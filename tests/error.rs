use nanoexpr::{Error, ErrorKind, Span};

const KINDS: [(ErrorKind, &str); 14] = [
    (ErrorKind::UnexpectedCharacter, "unexpected character"),
    (ErrorKind::UnexpectedToken, "unexpected token"),
    (ErrorKind::UnexpectedEnd, "unexpected end of expression"),
    (ErrorKind::UnclosedParenthesis, "unclosed parenthesis"),
    (ErrorKind::InvalidNumber, "invalid number"),
    (ErrorKind::UnknownVariable, "unknown variable"),
    (ErrorKind::UnknownFunction, "unknown function"),
    (ErrorKind::TooManyArguments, "too many arguments"),
    (ErrorKind::ArgumentCount, "wrong number of arguments"),
    (ErrorKind::DivisionByZero, "division by zero"),
    (ErrorKind::Domain, "operand out of domain"),
    (ErrorKind::Overflow, "overflow"),
    (ErrorKind::TooDeep, "expression nested too deeply"),
    (ErrorKind::Unsupported, "unsupported operation"),
];

#[test]
fn kinds_describe_themselves() {
    for (kind, description) in KINDS {
        assert_eq!(kind.description(), description);
        assert_eq!(kind.to_string(), description);
    }
}

#[test]
fn errors_display_their_span() {
    let error = Error::new(ErrorKind::Overflow, Span::new(3, 8));
    assert_eq!(error.to_string(), "overflow at 3..8");
    let source: &dyn core::error::Error = &error;
    assert!(source.source().is_none());
}
