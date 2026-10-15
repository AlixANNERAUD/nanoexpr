use nanoexpr::{Error, ErrorKind, Evaluator, Functions, PercentSign, Span, evaluate_with};

fn integer(input: &str) -> Result<i64, Error> {
    evaluate_with(input, &())
}

fn error(input: &str) -> (ErrorKind, usize, usize) {
    let error = integer(input).unwrap_err();
    (error.kind, error.span.start, error.span.end)
}

#[test]
fn arithmetic() {
    assert_eq!(integer("2 + 3"), Ok(5));
    assert_eq!(integer("10 - 4 - 3"), Ok(3));
    assert_eq!(integer("6 * 7"), Ok(42));
    assert_eq!(integer("20 / 2 / 5"), Ok(2));
    assert_eq!(integer("7 % 3"), Ok(1));
    assert_eq!(integer("7 mod 3"), Ok(1));
}

#[test]
fn precedence() {
    assert_eq!(integer("2 + 3 * 4"), Ok(14));
    assert_eq!(integer("(2 + 3) * 4"), Ok(20));
    assert_eq!(integer("2 * 3 ^ 2"), Ok(18));
    assert_eq!(integer("((((1))))"), Ok(1));
}

#[test]
fn power_is_right_associative() {
    assert_eq!(integer("2 ^ 3 ^ 2"), Ok(512));
    assert_eq!(integer("2 ** 3 ** 2"), Ok(512));
}

#[test]
fn unary_binds_looser_than_power() {
    assert_eq!(integer("-2 ^ 2"), Ok(-4));
    assert_eq!(integer("(-2) ^ 2"), Ok(4));
    assert_eq!(integer("2 * -3"), Ok(-6));
    assert_eq!(integer("--3"), Ok(3));
    assert_eq!(integer("+3 - -3"), Ok(6));
    assert_eq!(integer("-2 * 3 + 1"), Ok(-5));
}

#[test]
fn variables_and_functions() {
    let variables = [("x", 4_i64), ("y", 5)];
    let functions = Functions(|name: &str, arguments: &[i64]| match (name, arguments) {
        ("max", [first, rest @ ..]) => Some(Ok(rest.iter().fold(*first, |a, b| a.max(*b)))),
        ("max", []) => Some(Err(ErrorKind::ArgumentCount)),
        ("zero", []) => Some(Ok(0)),
        _ => None,
    });
    let context = (variables, functions);
    assert_eq!(evaluate_with("x * y", &context), Ok(20));
    assert_eq!(evaluate_with("max(x, y * 2, 3)", &context), Ok(10));
    assert_eq!(evaluate_with("max(max(1, 2), x)", &context), Ok(4));
    assert_eq!(evaluate_with("zero() + 1", &context), Ok(1));
    assert_eq!(
        evaluate_with::<i64, _>("1 + max()", &context),
        Err(Error::new(ErrorKind::ArgumentCount, Span::new(4, 9)))
    );
}

#[test]
fn percentage() {
    let evaluator = Evaluator::<_>::new(&()).percent_sign(PercentSign::Percentage);
    assert_eq!(evaluator.evaluate::<i64>("50%"), Ok(0));
    assert_eq!(evaluator.evaluate::<i64>("200 * 50%"), Ok(0));
    assert_eq!(evaluator.evaluate::<i64>("5000% + 1"), Ok(51));
    assert_eq!(evaluator.evaluate::<i64>("7 mod 3"), Ok(1));
    assert_eq!(
        evaluator.evaluate::<i64>("7 % 3").unwrap_err().kind,
        ErrorKind::UnexpectedToken
    );
}

#[test]
fn syntax_errors() {
    assert_eq!(error(""), (ErrorKind::UnexpectedEnd, 0, 0));
    assert_eq!(error("2 +"), (ErrorKind::UnexpectedEnd, 3, 3));
    assert_eq!(error("2 + * 3"), (ErrorKind::UnexpectedToken, 4, 5));
    assert_eq!(error("(2 + 3"), (ErrorKind::UnclosedParenthesis, 0, 1));
    assert_eq!(error("2 + 3)"), (ErrorKind::UnexpectedToken, 5, 6));
    assert_eq!(error("()"), (ErrorKind::UnexpectedToken, 1, 2));
    assert_eq!(error("2 3"), (ErrorKind::UnexpectedToken, 2, 3));
    assert_eq!(error("1 # 2"), (ErrorKind::UnexpectedCharacter, 2, 3));
    assert_eq!(error("f(1, 2"), (ErrorKind::UnclosedParenthesis, 1, 2));
    assert_eq!(error("f(1 2)"), (ErrorKind::UnexpectedToken, 4, 5));
}

#[test]
fn resolution_errors() {
    assert_eq!(error("1 + x"), (ErrorKind::UnknownVariable, 4, 5));
    assert_eq!(error("1 + f(2)"), (ErrorKind::UnknownFunction, 4, 5));
    assert_eq!(error("1.5"), (ErrorKind::InvalidNumber, 0, 3));
}

#[test]
fn arithmetic_errors_cover_the_operation() {
    assert_eq!(error("1 + 4 / (2 - 2)"), (ErrorKind::DivisionByZero, 4, 15));
    assert_eq!(
        error("9223372036854775807 + 1"),
        (ErrorKind::Overflow, 0, 23)
    );
    assert_eq!(error("2 ^ -1"), (ErrorKind::Domain, 0, 6));
}

#[test]
fn depth_is_bounded() {
    let evaluator = Evaluator::<_>::new(&()).max_depth(3);
    assert_eq!(evaluator.evaluate::<i64>("((1))"), Ok(1));
    assert_eq!(
        evaluator.evaluate::<i64>("(((1)))").unwrap_err().kind,
        ErrorKind::TooDeep
    );
    assert_eq!(
        evaluator.evaluate::<i64>("---1").unwrap_err().kind,
        ErrorKind::TooDeep
    );
    let deep = "(".repeat(10_000) + "1" + &")".repeat(10_000);
    assert_eq!(integer(&deep).unwrap_err().kind, ErrorKind::TooDeep);
}

#[test]
fn arguments_are_bounded() {
    let sum = Functions(|_: &str, arguments: &[i64]| Some(Ok(arguments.iter().sum())));
    let evaluator = Evaluator::<_, 2>::new(&sum);
    assert_eq!(evaluator.evaluate::<i64>("f(1, 2)"), Ok(3));
    assert_eq!(
        evaluator.evaluate::<i64>("f(1, 2, 3)").unwrap_err(),
        Error::new(ErrorKind::TooManyArguments, Span::new(8, 9))
    );
}

#[test]
fn other_number_types() {
    assert_eq!(evaluate_with::<u8, _>("200 + 55", &()), Ok(255));
    assert_eq!(
        evaluate_with::<u8, _>("200 + 56", &()).unwrap_err().kind,
        ErrorKind::Overflow
    );
    assert_eq!(
        evaluate_with::<u8, _>("-1", &()).unwrap_err().kind,
        ErrorKind::Overflow
    );
    assert_eq!(
        evaluate_with::<i8, _>("-128", &()).unwrap_err().kind,
        ErrorKind::InvalidNumber
    );
    assert_eq!(evaluate_with::<i8, _>("-127 - 1", &()), Ok(-128));
}

#[cfg(feature = "libm")]
#[test]
fn floats() {
    assert_eq!(evaluate_with::<f64, _>("1 / 4 + .5e1", &()), Ok(5.25));
    assert_eq!(
        evaluate_with::<f32, _>("2 ^ 0.5 ^ 2", &()),
        Ok(2.0_f32.powf(0.25))
    );
    assert_eq!(
        evaluate_with::<f64, _>("1 / 0", &()).unwrap_err().kind,
        ErrorKind::DivisionByZero
    );
    let evaluator = Evaluator::<_>::new(&()).percent_sign(PercentSign::Percentage);
    assert_eq!(evaluator.evaluate::<f64>("200 * 15%"), Ok(30.0));
    assert_eq!(evaluator.evaluate::<f64>("-50%"), Ok(-0.5));
}

#[cfg(feature = "libm")]
#[test]
fn float_literals_beyond_the_range_are_invalid() {
    assert_eq!(
        evaluate_with::<f32, _>("2E223", &()).unwrap_err(),
        Error::new(ErrorKind::InvalidNumber, Span::new(0, 5))
    );
    assert_eq!(
        evaluate_with::<f64, _>("1 + 1e309", &()).unwrap_err(),
        Error::new(ErrorKind::InvalidNumber, Span::new(4, 9))
    );
    assert_eq!(evaluate_with::<f64, _>("1e-400", &()), Ok(0.0));
    assert_eq!(evaluate_with::<f32, _>("3.4e38", &()), Ok(3.4e38));
}
