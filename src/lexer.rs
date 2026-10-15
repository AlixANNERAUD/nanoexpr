//! Splits an expression into tokens, one at a time and without allocating.

use crate::{Error, ErrorKind, Span};

/// What a token is, borrowing its text from the input when it has any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind<'a> {
    /// A number literal, parsed later by the number type.
    Number(&'a str),
    /// A variable, a function or the `mod` keyword.
    Name(&'a str),
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `^` or `**`
    Caret,
    /// `(`
    LeftParenthesis,
    /// `)`
    RightParenthesis,
    /// `,`
    Comma,
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
    /// `&&`
    And,
    /// `||`
    Or,
    /// `!`, a logical not before an operand and a factorial after one.
    Not,
    /// `?`
    Question,
    /// `:`
    Colon,
    /// The end of the input, returned again on every later call.
    End,
}

/// A token and where it is in the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Token<'a> {
    /// What the token is.
    pub kind: Kind<'a>,
    /// The bytes of the input it covers.
    pub span: Span,
}

/// Reads tokens from an input on demand, keeping only a position in it.
pub(crate) struct Lexer<'a> {
    /// The whole expression.
    input: &'a str,
    /// Offset of the next byte to read.
    position: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a lexer at the start of `input`.
    pub fn new(input: &'a str) -> Self {
        Self { input, position: 0 }
    }

    /// Returns the byte `offset` bytes after the current position, if any.
    fn byte(&self, offset: usize) -> Option<u8> {
        self.input.as_bytes().get(self.position + offset).copied()
    }

    /// Skips ASCII digits and returns how many there were.
    fn digits(&mut self) -> usize {
        let start = self.position;
        while self.byte(0).is_some_and(|byte| byte.is_ascii_digit()) {
            self.position += 1;
        }
        self.position - start
    }

    /// Reads a number literal: digits with an optional fraction and an optional exponent.
    ///
    /// The exponent is only read when digits follow the `e`, so `2e` is the number `2` then the name `e`.
    fn number(&mut self) -> Result<Kind<'a>, ErrorKind> {
        let start = self.position;
        let mut digits = self.digits();
        if self.byte(0) == Some(b'.') {
            self.position += 1;
            digits += self.digits();
        }
        if digits == 0 {
            return Err(ErrorKind::UnexpectedCharacter);
        }
        if matches!(self.byte(0), Some(b'e' | b'E')) {
            let sign = usize::from(matches!(self.byte(1), Some(b'+' | b'-')));
            if self
                .byte(1 + sign)
                .is_some_and(|byte| byte.is_ascii_digit())
            {
                self.position += 1 + sign;
                self.digits();
            }
        }
        Ok(Kind::Number(&self.input[start..self.position]))
    }

    /// Reads a name made of ASCII letters, digits and `_`.
    fn name(&mut self) -> Kind<'a> {
        let start = self.position;
        while self
            .byte(0)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            self.position += 1;
        }
        Kind::Name(&self.input[start..self.position])
    }

    /// Skips a symbol of `length` bytes and returns its `kind`.
    fn symbol(&mut self, kind: Kind<'a>, length: usize) -> Kind<'a> {
        self.position += length;
        kind
    }

    /// Skips a symbol of two bytes and returns `double` if its second byte is `second`, or skips a symbol of one
    /// byte and returns `single` otherwise.
    fn either(&mut self, second: u8, double: Kind<'a>, single: Kind<'a>) -> Kind<'a> {
        if self.byte(1) == Some(second) {
            self.symbol(double, 2)
        } else {
            self.symbol(single, 1)
        }
    }

    /// Reads a comparison or logic operator starting with `byte`.
    ///
    /// Kept apart from [`Lexer::next`] so that arithmetic, the most common, is lexed as fast as without them.
    #[inline(never)]
    fn logic(&mut self, byte: u8) -> Result<Kind<'a>, ErrorKind> {
        match byte {
            b'<' => Ok(self.either(b'=', Kind::LessEqual, Kind::Less)),
            b'>' => Ok(self.either(b'=', Kind::GreaterEqual, Kind::Greater)),
            b'!' => Ok(self.either(b'=', Kind::NotEqual, Kind::Not)),
            b'=' if self.byte(1) == Some(b'=') => Ok(self.symbol(Kind::Equal, 2)),
            b'&' if self.byte(1) == Some(b'&') => Ok(self.symbol(Kind::And, 2)),
            b'|' if self.byte(1) == Some(b'|') => Ok(self.symbol(Kind::Or, 2)),
            b'?' => Ok(self.symbol(Kind::Question, 1)),
            b':' => Ok(self.symbol(Kind::Colon, 1)),
            _ => Err(ErrorKind::UnexpectedCharacter),
        }
    }

    /// Reads the next token, skipping whitespace before it.
    ///
    /// An unexpected character is reported with the span of the whole character, even when it is several bytes long.
    #[inline(always)]
    pub fn next(&mut self) -> Result<Token<'a>, Error> {
        while self.byte(0).is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.position += 1;
        }
        let start = self.position;
        let kind = match self.byte(0) {
            None => Ok(Kind::End),
            Some(b'0'..=b'9' | b'.') => self.number(),
            Some(b'a'..=b'z' | b'A'..=b'Z' | b'_') => Ok(self.name()),
            Some(b'+') => Ok(self.symbol(Kind::Plus, 1)),
            Some(b'-') => Ok(self.symbol(Kind::Minus, 1)),
            Some(b'*') if self.byte(1) == Some(b'*') => Ok(self.symbol(Kind::Caret, 2)),
            Some(b'*') => Ok(self.symbol(Kind::Star, 1)),
            Some(b'/') => Ok(self.symbol(Kind::Slash, 1)),
            Some(b'%') => Ok(self.symbol(Kind::Percent, 1)),
            Some(b'^') => Ok(self.symbol(Kind::Caret, 1)),
            Some(b'(') => Ok(self.symbol(Kind::LeftParenthesis, 1)),
            Some(b')') => Ok(self.symbol(Kind::RightParenthesis, 1)),
            Some(b',') => Ok(self.symbol(Kind::Comma, 1)),
            Some(byte) => self.logic(byte),
        };
        match kind {
            Ok(kind) => Ok(Token {
                kind,
                span: Span::new(start, self.position),
            }),
            Err(kind) => {
                let length = self.input[start..].chars().next().map_or(1, char::len_utf8);
                Err(Error::new(kind, Span::new(start, start + length)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_kinds(input: &str, expected: &[Kind<'_>]) {
        let mut lexer = Lexer::new(input);
        for kind in expected {
            assert_eq!(
                lexer.next().map(|token| token.kind),
                Ok(*kind),
                "in {input:?}"
            );
        }
        assert_eq!(
            lexer.next().map(|token| token.kind),
            Ok(Kind::End),
            "in {input:?}"
        );
    }

    #[test]
    fn operators() {
        assert_kinds(
            "+ - * / % ^ ** ( ) ,",
            &[
                Kind::Plus,
                Kind::Minus,
                Kind::Star,
                Kind::Slash,
                Kind::Percent,
                Kind::Caret,
                Kind::Caret,
                Kind::LeftParenthesis,
                Kind::RightParenthesis,
                Kind::Comma,
            ],
        );
    }

    #[test]
    fn comparison_and_logic_operators() {
        assert_kinds(
            "< <= > >= == != && || ! ? : !!x<=-1",
            &[
                Kind::Less,
                Kind::LessEqual,
                Kind::Greater,
                Kind::GreaterEqual,
                Kind::Equal,
                Kind::NotEqual,
                Kind::And,
                Kind::Or,
                Kind::Not,
                Kind::Question,
                Kind::Colon,
                Kind::Not,
                Kind::Not,
                Kind::Name("x"),
                Kind::LessEqual,
                Kind::Minus,
                Kind::Number("1"),
            ],
        );
    }

    #[test]
    fn halves_of_operators_are_unexpected() {
        for input in ["1 = 2", "1 & 2", "1 | 2"] {
            assert_eq!(
                Lexer::new(input).nth_error(),
                Error::new(ErrorKind::UnexpectedCharacter, Span::new(2, 3))
            );
        }
    }

    #[test]
    fn numbers() {
        assert_kinds(
            "42 3.14 .5 5. 1e3 2.5E-4 6e+2",
            &[
                Kind::Number("42"),
                Kind::Number("3.14"),
                Kind::Number(".5"),
                Kind::Number("5."),
                Kind::Number("1e3"),
                Kind::Number("2.5E-4"),
                Kind::Number("6e+2"),
            ],
        );
    }

    #[test]
    fn exponent_needs_digits() {
        assert_kinds("2e", &[Kind::Number("2"), Kind::Name("e")]);
        assert_kinds("2e+", &[Kind::Number("2"), Kind::Name("e"), Kind::Plus]);
    }

    #[test]
    fn names() {
        assert_kinds(
            "sin x_1 _tmp PI",
            &[
                Kind::Name("sin"),
                Kind::Name("x_1"),
                Kind::Name("_tmp"),
                Kind::Name("PI"),
            ],
        );
    }

    #[test]
    fn spans() {
        let mut lexer = Lexer::new("  12 +sqrt");
        assert_eq!(lexer.next().unwrap().span, Span::new(2, 4));
        assert_eq!(lexer.next().unwrap().span, Span::new(5, 6));
        assert_eq!(lexer.next().unwrap().span, Span::new(6, 10));
        assert_eq!(lexer.next().unwrap().span, Span::new(10, 10));
    }

    #[test]
    fn unexpected_characters() {
        assert_eq!(
            Lexer::new("1 # 2").nth_error(),
            Error::new(ErrorKind::UnexpectedCharacter, Span::new(2, 3))
        );
        assert_eq!(
            Lexer::new("é").nth_error(),
            Error::new(ErrorKind::UnexpectedCharacter, Span::new(0, 2))
        );
        assert_eq!(
            Lexer::new(" . ").nth_error(),
            Error::new(ErrorKind::UnexpectedCharacter, Span::new(1, 2))
        );
    }

    impl Lexer<'_> {
        fn nth_error(mut self) -> Error {
            loop {
                match self.next() {
                    Ok(Token {
                        kind: Kind::End, ..
                    }) => panic!("no error"),
                    Ok(_) => {}
                    Err(error) => return error,
                }
            }
        }
    }
}
