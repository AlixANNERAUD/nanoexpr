use crate::{Error, ErrorKind, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind<'a> {
    Number(&'a str),
    Name(&'a str),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LeftParenthesis,
    RightParenthesis,
    Comma,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Token<'a> {
    pub kind: Kind<'a>,
    pub span: Span,
}

pub(crate) struct Lexer<'a> {
    input: &'a str,
    position: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, position: 0 }
    }

    fn byte(&self, offset: usize) -> Option<u8> {
        self.input.as_bytes().get(self.position + offset).copied()
    }

    fn digits(&mut self) -> usize {
        let start = self.position;
        while self.byte(0).is_some_and(|byte| byte.is_ascii_digit()) {
            self.position += 1;
        }
        self.position - start
    }

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

    fn symbol(&mut self, kind: Kind<'a>, length: usize) -> Kind<'a> {
        self.position += length;
        kind
    }

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
            Some(_) => Err(ErrorKind::UnexpectedCharacter),
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
