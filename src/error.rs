//! Errors and the spans of input they point to.

use core::fmt;

/// Byte range of the input an [`Error`] points to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// Offset of the first byte.
    pub start: usize,
    /// Offset one past the last byte.
    pub end: usize,
}

impl Span {
    /// Creates a span from `start` to `end`.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// What went wrong while evaluating an expression.
///
/// Match on it to show your own, possibly translated, message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A character that is not part of the syntax.
    UnexpectedCharacter,
    /// A token where it cannot appear, such as `2 + * 3`.
    UnexpectedToken,
    /// The input ends where an operand or a closing parenthesis is expected.
    UnexpectedEnd,
    /// A `(` without its matching `)`.
    UnclosedParenthesis,
    /// A number literal the number type cannot represent.
    InvalidNumber,
    /// A name the context does not know as a variable.
    UnknownVariable,
    /// A name the context does not know as a function.
    UnknownFunction,
    /// A call with more arguments than the evaluator can hold.
    TooManyArguments,
    /// A call with the wrong number of arguments for its function.
    ArgumentCount,
    /// A division or remainder by zero.
    DivisionByZero,
    /// An operand outside the domain of an operation, such as `sqrt(-1)`.
    Domain,
    /// A result the number type cannot represent.
    Overflow,
    /// Parentheses, calls or operators nested deeper than the evaluator allows.
    TooDeep,
    /// An operation the number type does not implement, such as a comparison of a custom [`Number`] type that
    /// does not override [`Number::compare`].
    ///
    /// [`Number`]: crate::Number
    /// [`Number::compare`]: crate::Number::compare
    Unsupported,
    /// An expression that needs more instructions, or more pending values, than the buffer or the stack of its
    /// compiled [`Program`] can hold.
    ///
    /// [`Program`]: crate::Program
    TooLong,
}

impl ErrorKind {
    /// Short English description of the error.
    pub const fn description(self) -> &'static str {
        match self {
            Self::UnexpectedCharacter => "unexpected character",
            Self::UnexpectedToken => "unexpected token",
            Self::UnexpectedEnd => "unexpected end of expression",
            Self::UnclosedParenthesis => "unclosed parenthesis",
            Self::InvalidNumber => "invalid number",
            Self::UnknownVariable => "unknown variable",
            Self::UnknownFunction => "unknown function",
            Self::TooManyArguments => "too many arguments",
            Self::ArgumentCount => "wrong number of arguments",
            Self::DivisionByZero => "division by zero",
            Self::Domain => "operand out of domain",
            Self::Overflow => "overflow",
            Self::TooDeep => "expression nested too deeply",
            Self::Unsupported => "unsupported operation",
            Self::TooLong => "expression too long",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.description())
    }
}

/// An evaluation error and the part of the input that caused it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Error {
    /// What went wrong.
    pub kind: ErrorKind,
    /// Where it went wrong.
    pub span: Span,
}

impl Error {
    /// Creates an error of `kind` at `span`.
    pub const fn new(kind: ErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}..{}",
            self.kind, self.span.start, self.span.end
        )
    }
}

impl core::error::Error for Error {}
