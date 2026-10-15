//! Parses expressions by precedence climbing, handing each operation to a [`Backend`] as soon as it is parsed.
//!
//! The parser checks the syntax, the nesting depth and the number of arguments of calls. What an operation does is
//! up to the backend: the evaluator computes it at once, the compiler appends an instruction to a program.

use core::cmp::Ordering;

use crate::lexer::{Kind, Lexer, Token};
use crate::{Error, ErrorKind, Number, PercentSign, Span};

/// The syntax settings shared by evaluation and compilation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Syntax {
    /// Most levels of nesting before [`ErrorKind::TooDeep`].
    pub max_depth: usize,
    /// How `%` is read.
    pub percent_sign: PercentSign,
    /// Whether an operand directly followed by a name or a `(` multiplies them.
    pub implicit_multiplication: bool,
}

/// An operator that computes its result from two operands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Binary {
    /// `+`
    Add,
    /// `-`
    Subtract,
    /// `*`, or a name or `(` directly after an operand with implicit multiplication on.
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
}

impl Binary {
    /// Applies the operator to `left` and `right`.
    pub fn apply<N: Number>(self, left: N, right: N) -> Result<N, ErrorKind> {
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
        }
    }
}

/// An operator that computes its result from one operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unary {
    /// A prefix `-`.
    Negate,
    /// A prefix `!`.
    Not,
    /// A postfix `%` percentage.
    Percentage,
}

impl Unary {
    /// Applies the operator to `operand`.
    pub fn apply<N: Number>(self, operand: N) -> Result<N, ErrorKind> {
        match self {
            Self::Negate => operand.negate(),
            Self::Not => N::from_bool(operand.is_zero()?),
            Self::Percentage => operand.divide(N::parse("100").ok_or(ErrorKind::Overflow)?),
        }
    }
}

/// An infix or postfix operator, as found after an operand.
#[derive(Clone, Copy)]
enum Operator {
    /// An operator of [`Binary`].
    Binary(Binary),
    /// A name or `(` directly after an operand, with implicit multiplication on: not consumed.
    Implicit,
    /// `&&`, which skips its right operand when the left one is false.
    And,
    /// `||`, which skips its right operand when the left one is true.
    Or,
    /// `? :`, which has a middle operand and skips the branch not taken.
    Conditional,
    /// A postfix `%` percentage, which has no right operand.
    Percentage,
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

/// What is done with each operation as soon as it is parsed.
///
/// Every method receives the span its errors cover. Errors returned by a backend stop the parsing at once.
pub(crate) trait Backend<'i> {
    /// What an operand turns into: its value, or nothing when compiling.
    type Value: Copy + Default;
    /// What [`Backend::logic`] and [`Backend::condition`] pass on to the end of the operation.
    type Branch: Copy;

    /// Handles a number literal.
    fn number(&mut self, literal: &'i str, span: Span) -> Result<Self::Value, Error>;
    /// Handles a variable.
    fn variable(&mut self, name: &'i str, span: Span) -> Result<Self::Value, Error>;
    /// Handles a call of `name`, whose own span is `name_span`, with `arguments`.
    fn call(
        &mut self,
        name: &'i str,
        name_span: Span,
        arguments: &[Self::Value],
        span: Span,
    ) -> Result<Self::Value, Error>;
    /// Handles a unary operation.
    fn unary(
        &mut self,
        operator: Unary,
        operand: Self::Value,
        span: Span,
    ) -> Result<Self::Value, Error>;
    /// Handles a binary operation.
    fn binary(
        &mut self,
        operator: Binary,
        left: Self::Value,
        right: Self::Value,
        span: Span,
    ) -> Result<Self::Value, Error>;
    /// Handles the left operand of `||` if `or`, or of `&&` otherwise, before the right operand is parsed. `span`
    /// covers the left operand and the operator.
    fn logic(&mut self, or: bool, left: Self::Value, span: Span) -> Result<Self::Branch, Error>;
    /// Ends `||` if `or`, or `&&` otherwise, after its right operand. `span` covers the whole operation.
    fn logic_end(
        &mut self,
        or: bool,
        branch: Self::Branch,
        right: Self::Value,
        span: Span,
    ) -> Result<Self::Value, Error>;
    /// Handles the condition of `? :`, before its first branch. `span` covers the condition and the `?`.
    fn condition(&mut self, condition: Self::Value, span: Span) -> Result<Self::Branch, Error>;
    /// Starts the second branch of `? :`, after the `:` at `span`.
    fn otherwise(&mut self, branch: Self::Branch, span: Span) -> Result<Self::Branch, Error>;
    /// Ends `? :` after its second branch.
    fn conditional_end(
        &mut self,
        branch: Self::Branch,
        then: Self::Value,
        otherwise: Self::Value,
    ) -> Self::Value;
}

/// Parses one expression, handing its operations to a [`Backend`].
pub(crate) struct Parser<'i, B, const ARGUMENTS: usize> {
    /// Source of the tokens after [`Parser::token`].
    lexer: Lexer<'i>,
    /// The token being looked at, not consumed yet.
    token: Token<'i>,
    /// Offset just past the last consumed token, where error spans end.
    end: usize,
    /// How many [`Parser::expression`] calls are running.
    depth: usize,
    /// Limits and syntax.
    syntax: Syntax,
    /// What is done with the operations.
    pub backend: B,
}

impl<'i, B: Backend<'i>, const ARGUMENTS: usize> Parser<'i, B, ARGUMENTS> {
    /// Parses the whole of `input` with `syntax`, handing its operations to `backend`.
    pub fn parse(input: &'i str, syntax: Syntax, backend: B) -> Result<(B::Value, B), Error> {
        let mut lexer = Lexer::new(input);
        let token = lexer.next()?;
        let mut parser = Self {
            lexer,
            token,
            end: 0,
            depth: 0,
            syntax,
            backend,
        };
        let value = parser.expression(0)?;
        match parser.token.kind {
            Kind::End => Ok((value, parser.backend)),
            _ => Err(Error::new(ErrorKind::UnexpectedToken, parser.token.span)),
        }
    }

    /// Consumes the current token, returns it and reads the next one.
    fn advance(&mut self) -> Result<Token<'i>, Error> {
        let token = self.token;
        self.end = token.span.end;
        self.token = self.lexer.next()?;
        Ok(token)
    }

    /// Returns the span from `start` to the end of the last consumed token.
    fn from(&self, start: usize) -> Span {
        Span::new(start, self.end)
    }

    /// Returns the infix or postfix operator of the current token and its binding powers, if it is one.
    fn infix(&self) -> Option<(Operator, (u8, u8))> {
        let binary = |operator, powers| Some((Operator::Binary(operator), powers));
        match self.token.kind {
            Kind::Plus => binary(Binary::Add, ADDITIVE),
            Kind::Minus => binary(Binary::Subtract, ADDITIVE),
            Kind::Star => binary(Binary::Multiply, MULTIPLICATIVE),
            Kind::Slash => binary(Binary::Divide, MULTIPLICATIVE),
            Kind::Percent => match self.syntax.percent_sign {
                PercentSign::Remainder => binary(Binary::Remainder, MULTIPLICATIVE),
                PercentSign::Percentage => Some((Operator::Percentage, (PERCENTAGE, PERCENTAGE))),
            },
            Kind::Name("mod") => binary(Binary::Remainder, MULTIPLICATIVE),
            Kind::Caret => binary(Binary::Power, POWER),
            Kind::Less => binary(Binary::Less, RELATIONAL),
            Kind::LessEqual => binary(Binary::LessEqual, RELATIONAL),
            Kind::Greater => binary(Binary::Greater, RELATIONAL),
            Kind::GreaterEqual => binary(Binary::GreaterEqual, RELATIONAL),
            Kind::Equal => binary(Binary::Equal, EQUALITY),
            Kind::NotEqual => binary(Binary::NotEqual, EQUALITY),
            Kind::And => Some((Operator::And, AND)),
            Kind::Or => Some((Operator::Or, OR)),
            Kind::Question => Some((Operator::Conditional, CONDITIONAL)),
            Kind::Name(_) | Kind::LeftParenthesis if self.syntax.implicit_multiplication => {
                Some((Operator::Implicit, MULTIPLICATIVE))
            }
            _ => None,
        }
    }

    /// Parses operands joined by operators that bind at least as tightly as `minimum`.
    ///
    /// Starts with a [`Parser::prefix`] operand, then handles each following operator whose left binding power is
    /// at least `minimum`, parsing its right operand with the operator's right binding power.
    fn expression(&mut self, minimum: u8) -> Result<B::Value, Error> {
        if self.depth == self.syntax.max_depth {
            return Err(Error::new(ErrorKind::TooDeep, self.token.span));
        }
        self.depth += 1;
        let start = self.token.span.start;
        let mut left = self.prefix()?;
        while let Some((operator, (left_power, right_power))) = self.infix() {
            if left_power < minimum {
                break;
            }
            if !matches!(operator, Operator::Implicit) {
                self.advance()?;
            }
            left = match operator {
                Operator::Binary(operator) => {
                    let right = self.expression(right_power)?;
                    self.backend
                        .binary(operator, left, right, self.from(start))?
                }
                Operator::Implicit => {
                    let right = self.expression(right_power)?;
                    self.backend
                        .binary(Binary::Multiply, left, right, self.from(start))?
                }
                Operator::Percentage => {
                    self.backend
                        .unary(Unary::Percentage, left, self.from(start))?
                }
                Operator::And | Operator::Or => {
                    let or = matches!(operator, Operator::Or);
                    let branch = self.backend.logic(or, left, self.from(start))?;
                    let right = self.expression(right_power)?;
                    self.backend
                        .logic_end(or, branch, right, self.from(start))?
                }
                Operator::Conditional => self.conditional(start, left)?,
            };
        }
        self.depth -= 1;
        Ok(left)
    }

    /// Parses the branches of `condition ? then : otherwise`, after the `?`.
    fn conditional(&mut self, start: usize, condition: B::Value) -> Result<B::Value, Error> {
        let branch = self.backend.condition(condition, self.from(start))?;
        let then = self.expression(0)?;
        let colon = match self.token.kind {
            Kind::Colon => self.advance()?,
            Kind::End => return Err(Error::new(ErrorKind::UnexpectedEnd, self.token.span)),
            _ => return Err(Error::new(ErrorKind::UnexpectedToken, self.token.span)),
        };
        let branch = self.backend.otherwise(branch, colon.span)?;
        let otherwise = self.expression(CONDITIONAL.1)?;
        Ok(self.backend.conditional_end(branch, then, otherwise))
    }

    /// Parses a single operand: a number, a variable, a call, a parenthesized expression or a unary operation.
    fn prefix(&mut self) -> Result<B::Value, Error> {
        let token = self.advance()?;
        let start = token.span.start;
        match token.kind {
            Kind::Number(literal) => self.backend.number(literal, token.span),
            Kind::Name(name) if self.token.kind == Kind::LeftParenthesis => self.call(name, token),
            Kind::Name(name) => self.backend.variable(name, token.span),
            Kind::LeftParenthesis => {
                let value = self.expression(0)?;
                self.close(token)?;
                Ok(value)
            }
            Kind::Minus => {
                let operand = self.expression(UNARY)?;
                self.backend.unary(Unary::Negate, operand, self.from(start))
            }
            Kind::Plus => self.expression(UNARY),
            Kind::Not => {
                let operand = self.expression(UNARY)?;
                self.backend.unary(Unary::Not, operand, self.from(start))
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

    /// Parses the arguments of a call to `name` and hands the call to the backend.
    ///
    /// Kept out of line so that the arguments array only takes stack space in calls, not in every operand.
    #[inline(never)]
    fn call(&mut self, name: &'i str, name_token: Token<'i>) -> Result<B::Value, Error> {
        let opening = self.advance()?;
        let mut arguments = [B::Value::default(); ARGUMENTS];
        let mut count = 0;
        if self.token.kind != Kind::RightParenthesis {
            loop {
                let start = self.token.span.start;
                let value = self.expression(0)?;
                let slot = arguments
                    .get_mut(count)
                    .ok_or(Error::new(ErrorKind::TooManyArguments, self.from(start)))?;
                *slot = value;
                count += 1;
                if self.token.kind != Kind::Comma {
                    break;
                }
                self.advance()?;
            }
        }
        self.close(opening)?;
        self.backend.call(
            name,
            name_token.span,
            &arguments[..count],
            self.from(name_token.span.start),
        )
    }
}
