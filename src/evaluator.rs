//! Evaluates expressions in a single pass, by precedence climbing.
//!
//! The value of each operand is computed as soon as it is parsed, so nothing is kept but the current token and
//! the values on the call stack. The operands `&&`, `||` and `? :` do not need are still parsed, to check their
//! syntax, but not computed.

use core::cmp::Ordering;
use core::marker::PhantomData;

use crate::lexer::{Kind, Lexer, Token};
use crate::{Context, Error, ErrorKind, Number, Span};

/// How the `%` sign is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PercentSign {
    /// `7 % 3` is the remainder of `7 / 3`, like `7 mod 3`.
    #[default]
    Remainder,
    /// `50%` is `50 / 100`, as on a pocket calculator. `mod` remains the remainder.
    Percentage,
}

/// Evaluates expressions against a [`Context`], with configurable limits and syntax.
///
/// `&&`, `||` and `condition ? then : otherwise` short-circuit: the operand they do not need is parsed, so its
/// syntax is checked, but it is not computed and the context is not asked for its names. So `x != 0 && 1 / x > 2`
/// is `0` rather than a division by zero when `x` is zero, and `x == 0 ? 0 : 1 / x` is `0`.
///
/// Errors that depend only on the text (syntax errors, [`ErrorKind::InvalidNumber`], [`ErrorKind::TooDeep`] and
/// [`ErrorKind::TooManyArguments`]) are reported wherever they are, even in an operand that is not computed, and
/// before any other error: a malformed expression is reported as such whatever the values. Otherwise the first
/// error in evaluation order is reported.
///
/// `ARGUMENTS` is the most arguments a single function call can take. They are held in an array on the stack, so
/// a call uses `ARGUMENTS * size_of::<N>()` bytes of stack for each level of nesting.
///
/// ```
/// # #[cfg(feature = "libm")] {
/// use nanoexpr::{Evaluator, PercentSign};
///
/// let evaluator = Evaluator::<_, 2>::new(&[("price", 80.0)])
///     .max_depth(8)
///     .percent_sign(PercentSign::Percentage);
/// assert_eq!(evaluator.evaluate::<f64>("price * (1 - 25%)"), Ok(60.0));
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Evaluator<'c, C: ?Sized, const ARGUMENTS: usize = 8> {
    /// Resolves variables and functions.
    context: &'c C,
    /// Most levels of nesting before [`ErrorKind::TooDeep`].
    max_depth: usize,
    /// How `%` is read.
    percent_sign: PercentSign,
}

impl<'c, C: ?Sized, const ARGUMENTS: usize> Evaluator<'c, C, ARGUMENTS> {
    /// Default limit of nested parentheses, calls and operators.
    pub const DEFAULT_MAX_DEPTH: usize = 32;

    /// Creates an evaluator that resolves names with `context`.
    pub const fn new(context: &'c C) -> Self {
        Self {
            context,
            max_depth: Self::DEFAULT_MAX_DEPTH,
            percent_sign: PercentSign::Remainder,
        }
    }

    /// Sets how deeply parentheses, calls and operators can nest before [`ErrorKind::TooDeep`].
    ///
    /// Evaluation recurses once per level, so this bounds the stack it uses.
    pub const fn max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Sets how the `%` sign is read.
    pub const fn percent_sign(mut self, percent_sign: PercentSign) -> Self {
        self.percent_sign = percent_sign;
        self
    }

    /// Evaluates `input`.
    pub fn evaluate<N: Number>(&self, input: &str) -> Result<N, Error>
    where
        C: Context<N>,
    {
        let mut lexer = Lexer::new(input);
        let token = lexer.next()?;
        let mut parser = Parser::<N, C, ARGUMENTS> {
            lexer,
            token,
            end: 0,
            depth: 0,
            skipping: false,
            failure: None,
            evaluator: self,
            number: PhantomData,
        };
        let value = parser.expression(0)?;
        match parser.token.kind {
            Kind::End => parser.failure.map_or(Ok(value), Err),
            _ => Err(Error::new(ErrorKind::UnexpectedToken, parser.token.span)),
        }
    }
}

/// An infix or postfix operator.
#[derive(Clone, Copy)]
enum Operator {
    /// `+`
    Add,
    /// `-`
    Subtract,
    /// `*`
    Multiply,
    /// `/`
    Divide,
    /// `%` or `mod`
    Remainder,
    /// `^` or `**`
    Power,
    /// `<`
    Less,
    /// `<=`
    LessEqual,
    /// `>`
    Greater,
    /// `>=`
    GreaterEqual,
    /// `==`
    Equal,
    /// `!=`
    NotEqual,
    /// `&&`, which skips its right operand when the left one is false.
    And,
    /// `||`, which skips its right operand when the left one is true.
    Or,
    /// `? :`, which has a middle operand and skips the branch not taken.
    Conditional,
    /// A postfix `%` percentage, which has no right operand.
    Percentage,
}

impl Operator {
    /// Applies an operator that computes its result from two operands to `left` and `right`.
    fn apply<N: Number>(self, left: N, right: N) -> Result<N, ErrorKind> {
        let ordering = |accept: fn(Ordering) -> bool| {
            left.compare(right)
                .and_then(|ordering| N::from_bool(accept(ordering)))
        };
        match self {
            Self::Add => left.add(right),
            Self::Subtract => left.subtract(right),
            Self::Multiply => left.multiply(right),
            Self::Divide => left.divide(right),
            Self::Remainder => left.remainder(right),
            Self::Power => left.power(right),
            Self::Less => ordering(Ordering::is_lt),
            Self::LessEqual => ordering(Ordering::is_le),
            Self::Greater => ordering(Ordering::is_gt),
            Self::GreaterEqual => ordering(Ordering::is_ge),
            Self::Equal => ordering(Ordering::is_eq),
            Self::NotEqual => ordering(Ordering::is_ne),
            Self::And | Self::Or | Self::Conditional | Self::Percentage => {
                Err(ErrorKind::Unsupported)
            }
        }
    }
}

/// Binding powers of the conditional `? :`, the loosest operator: the left one is higher, so it associates to the
/// right and `a ? b : c ? d : e` is `a ? b : (c ? d : e)`.
const CONDITIONAL: (u8, u8) = (2, 1);
/// Binding powers of `||`, associating to the left.
const OR: (u8, u8) = (3, 4);
/// Binding powers of `&&`, tighter than [`OR`] as in C.
const AND: (u8, u8) = (5, 6);
/// Binding powers of `==` and `!=`, looser than [`RELATIONAL`] as in C, so `a < b == c < d` compares two
/// comparisons.
const EQUALITY: (u8, u8) = (7, 8);
/// Binding powers of `<`, `<=`, `>` and `>=`, looser than arithmetic.
const RELATIONAL: (u8, u8) = (9, 10);
/// Binding powers of `+` and `-`, on their left and right: the right one is higher, so they associate to the left.
const ADDITIVE: (u8, u8) = (11, 12);
/// Binding powers of `*`, `/`, `%` and `mod`, tighter than [`ADDITIVE`] and associating to the left.
const MULTIPLICATIVE: (u8, u8) = (13, 14);
/// Binding power of the operand of a unary `-`, `+` or `!`: looser than [`POWER`], so `-2^2` is `-(2^2)`.
const UNARY: u8 = 15;
/// Binding powers of `^`: the left one is higher, so it associates to the right and `2^3^2` is `2^(3^2)`.
const POWER: (u8, u8) = (18, 17);
/// Binding power of a postfix `%` percentage, the tightest of all.
const PERCENTAGE: u8 = 19;

/// The state of one evaluation.
///
/// Errors come in two sorts. Syntax errors, [`ErrorKind::TooDeep`], [`ErrorKind::TooManyArguments`] and
/// [`ErrorKind::InvalidNumber`] depend only on the text and are returned at once, wherever they are. The others
/// depend on values or on the context: the first one is kept in [`Parser::failure`] and the rest of the input is
/// still parsed, without computing, so that a syntax error further on takes precedence.
struct Parser<'i, 'e, 'c, N, C: ?Sized, const ARGUMENTS: usize> {
    /// Source of the tokens after [`Parser::token`].
    lexer: Lexer<'i>,
    /// The token being looked at, not consumed yet.
    token: Token<'i>,
    /// Offset just past the last consumed token, where error spans end.
    end: usize,
    /// How many [`Parser::expression`] calls are running.
    depth: usize,
    /// Whether values are not computed, in a branch not taken or after a [`Parser::failure`].
    skipping: bool,
    /// The first error that depends on values or on the context.
    failure: Option<Error>,
    /// Limits, syntax and context.
    evaluator: &'e Evaluator<'c, C, ARGUMENTS>,
    /// The number type values are computed in.
    number: PhantomData<N>,
}

impl<'i, N: Number, C: Context<N> + ?Sized, const ARGUMENTS: usize>
    Parser<'i, '_, '_, N, C, ARGUMENTS>
{
    /// Consumes the current token, returns it and reads the next one.
    fn advance(&mut self) -> Result<Token<'i>, Error> {
        let token = self.token;
        self.end = token.span.end;
        self.token = self.lexer.next()?;
        Ok(token)
    }

    /// Creates an error of `kind` spanning from `start` to the end of the last consumed token.
    fn at(&self, start: usize, kind: ErrorKind) -> Error {
        Error::new(kind, Span::new(start, self.end))
    }

    /// Records `error` as the [`Parser::failure`] and stops computing.
    #[cold]
    fn fail(&mut self, error: Error) -> N {
        self.failure = Some(error);
        self.skipping = true;
        N::default()
    }

    /// Returns the result of `operation`, or records its error spanning from `start`.
    ///
    /// Returns a placeholder without running `operation` when values are not computed.
    fn compute(&mut self, start: usize, operation: impl FnOnce() -> Result<N, ErrorKind>) -> N {
        if self.skipping {
            return N::default();
        }
        match operation() {
            Ok(value) => value,
            Err(kind) => self.fail(self.at(start, kind)),
        }
    }

    /// Returns whether `value` is true, that is not zero, or `None` when values are not computed or on error.
    fn truth(&mut self, start: usize, value: N) -> Option<bool> {
        if self.skipping {
            return None;
        }
        match value.is_zero() {
            Ok(zero) => Some(!zero),
            Err(kind) => {
                self.fail(self.at(start, kind));
                None
            }
        }
    }

    /// Parses an expression like [`Parser::expression`] without computing it nor asking the context anything.
    fn skip(&mut self, minimum: u8) -> Result<N, Error> {
        let skipping = core::mem::replace(&mut self.skipping, true);
        let outcome = self.expression(minimum);
        self.skipping = skipping;
        outcome.map(|_| N::default())
    }

    /// Returns the infix or postfix operator of the current token and its binding powers, if it is one.
    fn infix(&self) -> Option<(Operator, (u8, u8))> {
        match self.token.kind {
            Kind::Plus => Some((Operator::Add, ADDITIVE)),
            Kind::Minus => Some((Operator::Subtract, ADDITIVE)),
            Kind::Star => Some((Operator::Multiply, MULTIPLICATIVE)),
            Kind::Slash => Some((Operator::Divide, MULTIPLICATIVE)),
            Kind::Percent => Some(match self.evaluator.percent_sign {
                PercentSign::Remainder => (Operator::Remainder, MULTIPLICATIVE),
                PercentSign::Percentage => (Operator::Percentage, (PERCENTAGE, PERCENTAGE)),
            }),
            Kind::Name("mod") => Some((Operator::Remainder, MULTIPLICATIVE)),
            Kind::Caret => Some((Operator::Power, POWER)),
            Kind::Less => Some((Operator::Less, RELATIONAL)),
            Kind::LessEqual => Some((Operator::LessEqual, RELATIONAL)),
            Kind::Greater => Some((Operator::Greater, RELATIONAL)),
            Kind::GreaterEqual => Some((Operator::GreaterEqual, RELATIONAL)),
            Kind::Equal => Some((Operator::Equal, EQUALITY)),
            Kind::NotEqual => Some((Operator::NotEqual, EQUALITY)),
            Kind::And => Some((Operator::And, AND)),
            Kind::Or => Some((Operator::Or, OR)),
            Kind::Question => Some((Operator::Conditional, CONDITIONAL)),
            _ => None,
        }
    }

    /// Evaluates operands joined by operators that bind at least as tightly as `minimum`.
    ///
    /// Starts with a [`Parser::prefix`] operand, then applies each following operator whose left binding power is
    /// at least `minimum`, evaluating its right operand with the operator's right binding power.
    fn expression(&mut self, minimum: u8) -> Result<N, Error> {
        if self.depth == self.evaluator.max_depth {
            return Err(Error::new(ErrorKind::TooDeep, self.token.span));
        }
        self.depth += 1;
        let start = self.token.span.start;
        let mut left = self.prefix()?;
        while let Some((operator, (left_power, right_power))) = self.infix() {
            if left_power < minimum {
                break;
            }
            self.advance()?;
            left = match operator {
                Operator::Percentage => self.compute(start, || {
                    left.divide(N::parse("100").ok_or(ErrorKind::Overflow)?)
                }),
                Operator::Conditional => self.conditional(start, left)?,
                Operator::And | Operator::Or => self.logic(start, operator, left, right_power)?,
                _ => {
                    let right = self.expression(right_power)?;
                    self.compute(start, || operator.apply(left, right))
                }
            };
        }
        self.depth -= 1;
        Ok(left)
    }

    /// Evaluates the right operand of `&&` or `||` if `left` does not decide the result, and skips it otherwise.
    fn logic(
        &mut self,
        start: usize,
        operator: Operator,
        left: N,
        right_power: u8,
    ) -> Result<N, Error> {
        let decided = match (operator, self.truth(start, left)) {
            (Operator::And, Some(false)) => Some(false),
            (Operator::Or, Some(true)) => Some(true),
            _ => None,
        };
        if let Some(result) = decided {
            self.skip(right_power)?;
            return Ok(self.compute(start, || N::from_bool(result)));
        }
        let right = self.expression(right_power)?;
        Ok(match self.truth(start, right) {
            Some(result) => self.compute(start, || N::from_bool(result)),
            None => N::default(),
        })
    }

    /// Evaluates the branch of `condition ? then : otherwise` that `condition` selects, after the `?`, and skips
    /// the other.
    fn conditional(&mut self, start: usize, condition: N) -> Result<N, Error> {
        let truth = self.truth(start, condition);
        let then = if truth == Some(false) {
            self.skip(0)?
        } else {
            self.expression(0)?
        };
        match self.token.kind {
            Kind::Colon => self.advance().map(drop)?,
            Kind::End => return Err(Error::new(ErrorKind::UnexpectedEnd, self.token.span)),
            _ => return Err(Error::new(ErrorKind::UnexpectedToken, self.token.span)),
        }
        let otherwise = if truth == Some(true) {
            self.skip(CONDITIONAL.1)?
        } else {
            self.expression(CONDITIONAL.1)?
        };
        Ok(if truth == Some(true) { then } else { otherwise })
    }

    /// Evaluates a single operand: a number, a variable, a call, a parenthesized expression or a unary operation.
    fn prefix(&mut self) -> Result<N, Error> {
        let token = self.advance()?;
        let start = token.span.start;
        match token.kind {
            Kind::Number(literal) => {
                N::parse(literal).ok_or(Error::new(ErrorKind::InvalidNumber, token.span))
            }
            Kind::Name(name) if self.token.kind == Kind::LeftParenthesis => self.call(name, token),
            Kind::Name(_) if self.skipping => Ok(N::default()),
            Kind::Name(name) => Ok(match self.evaluator.context.variable(name) {
                Some(value) => value,
                None => self.fail(Error::new(ErrorKind::UnknownVariable, token.span)),
            }),
            Kind::LeftParenthesis => {
                let value = self.expression(0)?;
                self.close(token)?;
                Ok(value)
            }
            Kind::Minus => {
                let operand = self.expression(UNARY)?;
                Ok(self.compute(start, || operand.negate()))
            }
            Kind::Plus => self.expression(UNARY),
            Kind::Not => {
                let operand = self.expression(UNARY)?;
                Ok(match self.truth(start, operand) {
                    Some(truth) => self.compute(start, || N::from_bool(!truth)),
                    None => N::default(),
                })
            }
            Kind::End => Err(Error::new(ErrorKind::UnexpectedEnd, token.span)),
            _ => Err(Error::new(ErrorKind::UnexpectedToken, token.span)),
        }
    }

    /// Consumes the `)` matching `opening`.
    fn close(&mut self, opening: Token<'i>) -> Result<(), Error> {
        match self.token.kind {
            Kind::RightParenthesis => self.advance().map(drop),
            Kind::End => Err(Error::new(ErrorKind::UnclosedParenthesis, opening.span)),
            _ => Err(Error::new(ErrorKind::UnexpectedToken, self.token.span)),
        }
    }

    /// Evaluates the arguments of a call to `name` and asks the context to call it.
    ///
    /// Kept out of line so that the arguments array only takes stack space in calls, not in every operand.
    #[inline(never)]
    fn call(&mut self, name: &str, name_token: Token<'i>) -> Result<N, Error> {
        let opening = self.advance()?;
        let mut arguments = [N::default(); ARGUMENTS];
        let mut count = 0;
        if self.token.kind != Kind::RightParenthesis {
            loop {
                let start = self.token.span.start;
                let value = self.expression(0)?;
                let slot = arguments
                    .get_mut(count)
                    .ok_or(self.at(start, ErrorKind::TooManyArguments))?;
                *slot = value;
                count += 1;
                if self.token.kind != Kind::Comma {
                    break;
                }
                self.advance()?;
            }
        }
        self.close(opening)?;
        if self.skipping {
            return Ok(N::default());
        }
        Ok(
            match self.evaluator.context.call(name, &arguments[..count]) {
                Some(Ok(value)) => value,
                Some(Err(kind)) => self.fail(self.at(name_token.span.start, kind)),
                None => self.fail(Error::new(ErrorKind::UnknownFunction, name_token.span)),
            },
        )
    }
}

/// Evaluates `input`, resolving names with `context`.
///
/// ```
/// let context = [("price", 250), ("quantity", 4)];
/// assert_eq!(nanoexpr::evaluate_with("price * quantity - 100", &context), Ok(900));
/// ```
pub fn evaluate_with<N: Number, C: Context<N> + ?Sized>(
    input: &str,
    context: &C,
) -> Result<N, Error> {
    Evaluator::<C>::new(context).evaluate(input)
}
