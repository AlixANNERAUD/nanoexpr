//! Evaluates expressions in a single pass, by precedence climbing.
//!
//! The value of each operand is computed as soon as it is parsed, so nothing is kept but the current token and
//! the values on the call stack.

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
            evaluator: self,
            number: PhantomData,
        };
        let value = parser.expression(0)?;
        match parser.token.kind {
            Kind::End => Ok(value),
            _ => Err(Error::new(ErrorKind::UnexpectedToken, parser.token.span)),
        }
    }
}

/// A binary operator, mapped to a [`Number`] operation.
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
}

/// Binding powers of `+` and `-`, on their left and right: the right one is higher, so they associate to the left.
const ADDITIVE: (u8, u8) = (1, 2);
/// Binding powers of `*`, `/`, `%` and `mod`, tighter than [`ADDITIVE`] and associating to the left.
const MULTIPLICATIVE: (u8, u8) = (3, 4);
/// Binding power of the operand of a unary `-` or `+`: looser than [`POWER`], so `-2^2` is `-(2^2)`.
const UNARY: u8 = 5;
/// Binding powers of `^`: the left one is higher, so it associates to the right and `2^3^2` is `2^(3^2)`.
const POWER: (u8, u8) = (8, 7);
/// Binding power of a postfix `%` percentage, the tightest of all.
const PERCENTAGE: u8 = 9;

/// The state of one evaluation.
struct Parser<'i, 'e, 'c, N, C: ?Sized, const ARGUMENTS: usize> {
    /// Source of the tokens after [`Parser::token`].
    lexer: Lexer<'i>,
    /// The token being looked at, not consumed yet.
    token: Token<'i>,
    /// Offset just past the last consumed token, where error spans end.
    end: usize,
    /// How many [`Parser::expression`] calls are running.
    depth: usize,
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

    /// Returns the binary operator of the current token and its binding powers, if it is one.
    fn infix(&self) -> Option<(Operator, (u8, u8))> {
        match self.token.kind {
            Kind::Plus => Some((Operator::Add, ADDITIVE)),
            Kind::Minus => Some((Operator::Subtract, ADDITIVE)),
            Kind::Star => Some((Operator::Multiply, MULTIPLICATIVE)),
            Kind::Slash => Some((Operator::Divide, MULTIPLICATIVE)),
            Kind::Percent if self.evaluator.percent_sign == PercentSign::Remainder => {
                Some((Operator::Remainder, MULTIPLICATIVE))
            }
            Kind::Name("mod") => Some((Operator::Remainder, MULTIPLICATIVE)),
            Kind::Caret => Some((Operator::Power, POWER)),
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
        loop {
            if self.token.kind == Kind::Percent
                && self.evaluator.percent_sign == PercentSign::Percentage
            {
                if PERCENTAGE < minimum {
                    break;
                }
                self.advance()?;
                let hundred = N::parse("100").ok_or(self.at(start, ErrorKind::Overflow))?;
                left = left.divide(hundred).map_err(|kind| self.at(start, kind))?;
                continue;
            }
            let Some((operator, (left_power, right_power))) = self.infix() else {
                break;
            };
            if left_power < minimum {
                break;
            }
            self.advance()?;
            let right = self.expression(right_power)?;
            left = match operator {
                Operator::Add => left.add(right),
                Operator::Subtract => left.subtract(right),
                Operator::Multiply => left.multiply(right),
                Operator::Divide => left.divide(right),
                Operator::Remainder => left.remainder(right),
                Operator::Power => left.power(right),
            }
            .map_err(|kind| self.at(start, kind))?;
        }
        self.depth -= 1;
        Ok(left)
    }

    /// Evaluates a single operand: a number, a variable, a call, a parenthesized expression or a unary operation.
    fn prefix(&mut self) -> Result<N, Error> {
        let token = self.advance()?;
        match token.kind {
            Kind::Number(literal) => {
                N::parse(literal).ok_or(Error::new(ErrorKind::InvalidNumber, token.span))
            }
            Kind::Name(name) if self.token.kind == Kind::LeftParenthesis => self.call(name, token),
            Kind::Name(name) => self
                .evaluator
                .context
                .variable(name)
                .ok_or(Error::new(ErrorKind::UnknownVariable, token.span)),
            Kind::LeftParenthesis => {
                let value = self.expression(0)?;
                self.close(token)?;
                Ok(value)
            }
            Kind::Minus => self
                .expression(UNARY)?
                .negate()
                .map_err(|kind| self.at(token.span.start, kind)),
            Kind::Plus => self.expression(UNARY),
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
        match self.evaluator.context.call(name, &arguments[..count]) {
            Some(result) => result.map_err(|kind| self.at(name_token.span.start, kind)),
            None => Err(Error::new(ErrorKind::UnknownFunction, name_token.span)),
        }
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
