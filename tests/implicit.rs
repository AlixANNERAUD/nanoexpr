use nanoexpr::{Error, ErrorKind, Evaluator, Functions, PercentSign, Span};

type Call = fn(&str, &[i64]) -> Option<Result<i64, ErrorKind>>;

fn context() -> ([(&'static str, i64); 3], Functions<Call>) {
    (
        [("x", 3), ("y", 5), ("e", 2)],
        Functions(|name, arguments| match (name, arguments) {
            ("square", [value]) => Some(Ok(value * value)),
            ("x", [value]) => Some(Ok(value + 100)),
            _ => None,
        }),
    )
}

fn implicit(input: &str) -> Result<i64, Error> {
    let context = context();
    Evaluator::<_>::new(&context)
        .implicit_multiplication(true)
        .evaluate(input)
}

fn explicit(input: &str) -> Result<i64, Error> {
    let context = context();
    Evaluator::<_>::new(&context).evaluate(input)
}

#[test]
fn operands_multiply() {
    for (input, expected) in [
        ("2x", 6),
        ("2 x", 6),
        ("x y", 15),
        ("xy", 0),
        ("2(3 + 4)", 14),
        ("(1 + 2)(3 + 4)", 21),
        ("(2)x", 6),
        ("3square(4)", 48),
        ("square(2)square(3)", 36),
        ("2x y", 30),
        ("2e", 4),
    ] {
        let expected = if input == "xy" {
            Err(Error::new(ErrorKind::UnknownVariable, Span::new(0, 2)))
        } else {
            Ok(expected)
        };
        assert_eq!(implicit(input), expected, "{input}");
    }
}

#[test]
fn precedence_is_that_of_explicit_multiplication() {
    for (input, expected) in [
        ("12/2x", 18),
        ("12/2*x", 18),
        ("2x^2", 18),
        ("-2x", -6),
        ("-2^2x", -12),
        ("1 + 2x", 7),
        ("2x mod 4", 2),
        ("x(2)", 102),
        ("2x(2)", 204),
        ("2 x (2)", 204),
        ("x < 2x", 1),
        ("0 ? 1 : 2x", 6),
        ("2-x", -1),
    ] {
        assert_eq!(implicit(input), Ok(expected), "{input}");
    }
}

#[test]
fn numbers_after_operands_stay_errors() {
    assert_eq!(
        implicit("2 3"),
        Err(Error::new(ErrorKind::UnexpectedToken, Span::new(2, 3)))
    );
    assert_eq!(
        implicit("x 2"),
        Err(Error::new(ErrorKind::UnexpectedToken, Span::new(2, 3)))
    );
    assert_eq!(
        implicit("(1)2"),
        Err(Error::new(ErrorKind::UnexpectedToken, Span::new(3, 4)))
    );
}

#[test]
fn errors_cover_the_product() {
    assert_eq!(
        implicit("1 + 2z"),
        Err(Error::new(ErrorKind::UnknownVariable, Span::new(5, 6)))
    );
    assert_eq!(
        implicit("1 + 9223372036854775807x"),
        Err(Error::new(ErrorKind::Overflow, Span::new(4, 24)))
    );
    assert_eq!(
        implicit("2(1"),
        Err(Error::new(ErrorKind::UnclosedParenthesis, Span::new(1, 2)))
    );
    assert_eq!(implicit("0 && 2(1 / 0)"), Ok(0));
}

#[test]
fn is_off_by_default() {
    assert_eq!(
        explicit("2x"),
        Err(Error::new(ErrorKind::UnexpectedToken, Span::new(1, 2)))
    );
    assert_eq!(
        explicit("2(3)"),
        Err(Error::new(ErrorKind::UnexpectedToken, Span::new(1, 2)))
    );
    assert_eq!(explicit("x(2)"), Ok(102));
}

#[test]
fn works_with_percentages() {
    let context = context();
    let calculator = Evaluator::<_>::new(&context)
        .implicit_multiplication(true)
        .percent_sign(PercentSign::Percentage);
    assert_eq!(calculator.evaluate::<i64>("200%x"), Ok(6));
    assert_eq!(
        calculator.evaluate::<i64>("x 7 mod 4"),
        Err(Error::new(ErrorKind::UnexpectedToken, Span::new(2, 3)))
    );
}

#[test]
fn depth_stays_bounded() {
    let context = context();
    let evaluator = Evaluator::<_>::new(&context)
        .implicit_multiplication(true)
        .max_depth(3);
    assert_eq!(evaluator.evaluate::<i64>("x x x x x x"), Ok(729));
    assert_eq!(
        evaluator.evaluate::<i64>("x(((1)))").unwrap_err().kind,
        ErrorKind::TooDeep
    );
}

#[cfg(feature = "libm")]
#[test]
fn calculator_constants() {
    let calculator = Evaluator::<_>::new(&nanoexpr::Math).implicit_multiplication(true);
    assert_eq!(
        calculator.evaluate::<f64>("2pi"),
        Ok(2.0 * core::f64::consts::PI)
    );
    assert_eq!(calculator.evaluate::<f64>("2e3"), Ok(2000.0));
    assert_eq!(
        calculator.evaluate::<f64>("2e"),
        Ok(2.0 * core::f64::consts::E)
    );
    assert_eq!(calculator.evaluate::<f64>("3sqrt(16)"), Ok(12.0));
    assert_eq!(calculator.evaluate::<f64>("-2^2"), Ok(-4.0));
    assert_eq!(
        calculator.evaluate::<f64>("2 pi r",),
        Err(Error::new(ErrorKind::UnknownVariable, Span::new(5, 6)))
    );
}
