use std::cell::Cell;

use nanoexpr::{Context, Error, ErrorKind, Evaluator, Functions, Span, evaluate_with};

fn integer(input: &str) -> Result<i64, Error> {
    evaluate_with(input, &())
}

fn error(input: &str) -> (ErrorKind, usize, usize) {
    let error = integer(input).unwrap_err();
    (error.kind, error.span.start, error.span.end)
}

#[test]
fn comparisons_give_one_or_zero() {
    for (input, expected) in [
        ("1 < 2", 1),
        ("2 < 2", 0),
        ("2 <= 2", 1),
        ("3 <= 2", 0),
        ("3 > 2", 1),
        ("2 > 2", 0),
        ("2 >= 2", 1),
        ("1 >= 2", 0),
        ("2 == 2", 1),
        ("2 == 3", 0),
        ("2 != 3", 1),
        ("2 != 2", 0),
        ("-3 < -2", 1),
    ] {
        assert_eq!(integer(input), Ok(expected), "{input}");
    }
}

#[test]
fn logic_operators_treat_zero_as_false() {
    for (input, expected) in [
        ("1 && 1", 1),
        ("5 && -2", 1),
        ("1 && 0", 0),
        ("0 && 1", 0),
        ("0 || 0", 0),
        ("0 || 7", 1),
        ("7 || 0", 1),
        ("!0", 1),
        ("!3", 0),
        ("!!3", 1),
        ("1 ? 10 : 20", 10),
        ("0 ? 10 : 20", 20),
        ("-1 ? 10 : 20", 10),
    ] {
        assert_eq!(integer(input), Ok(expected), "{input}");
    }
}

#[test]
fn precedence_follows_c() {
    for (input, expected) in [
        ("1 + 2 < 4", 1),
        ("2 * 3 > 5", 1),
        ("1 < 2 == 3 < 4", 1),
        ("1 < 2 == 3 > 4", 0),
        ("3 > 2 > 1", 0),
        ("1 == 1 && 2 == 3", 0),
        ("1 || 0 && 0", 1),
        ("(1 || 0) && 0", 0),
        ("0 && 1 || 1", 1),
        ("!0 + 1", 2),
        ("!(0 + 1)", 0),
        ("!2 ^ 0", 0),
        ("-!0", -1),
        ("!0 == 1", 1),
        ("1 || 0 ? 10 : 20", 10),
        ("0 ? 1 : 0 ? 2 : 3", 3),
        ("1 ? 0 ? 4 : 5 : 6", 5),
        ("0 ? 1 : 2 + 3", 5),
        ("1 ? 2 : 3 + 10", 2),
        ("(1 ? 2 : 3) + 10", 12),
        ("2 + (0 ? 1 : 2) * 3", 8),
        ("1 ? 2 : 0 || 1", 2),
        ("0 ? 2 : 0 || 1", 1),
    ] {
        assert_eq!(integer(input), Ok(expected), "{input}");
    }
}

#[test]
fn conditionals_nest_in_arguments() {
    let sum = Functions(|_: &str, arguments: &[i64]| Some(Ok(arguments.iter().sum())));
    assert_eq!(evaluate_with("f(1 ? 2 : 3, 0 ? 4 : 5)", &sum), Ok(7));
    assert_eq!(evaluate_with("f(1 > 0, 2)", &sum), Ok(3));
}

#[test]
fn short_circuit_skips_errors() {
    let context = [("x", 0_i64), ("y", 4)];
    let evaluate = |input| evaluate_with::<i64, _>(input, &context);
    assert_eq!(evaluate("x != 0 && 1 / x > 2"), Ok(0));
    assert_eq!(evaluate("x == 0 || 1 / x > 2"), Ok(1));
    assert_eq!(evaluate("x == 0 ? 0 : 1 / x"), Ok(0));
    assert_eq!(evaluate("x != 0 ? 1 / x : y"), Ok(4));
    assert_eq!(evaluate("0 && unknown(missing)"), Ok(0));
    assert_eq!(evaluate("1 || missing"), Ok(1));
    assert_eq!(evaluate("1 ? y : missing + 9223372036854775807 * 2"), Ok(4));
    assert_eq!(evaluate("0 && (1 / 0 || 1 / 0)"), Ok(0));
    assert_eq!(
        evaluate("x != 0 || 1 / x > 2"),
        Err(Error::new(ErrorKind::DivisionByZero, Span::new(10, 15)))
    );
    assert_eq!(
        evaluate("x ? 1 : 1 / x"),
        Err(Error::new(ErrorKind::DivisionByZero, Span::new(8, 13)))
    );
}

#[test]
fn skipped_branches_do_not_ask_the_context() {
    struct Counting(Cell<usize>);

    impl Context<i64> for Counting {
        fn variable(&self, _: &str) -> Option<i64> {
            self.0.set(self.0.get() + 1);
            Some(1)
        }

        fn call(&self, _: &str, _: &[i64]) -> Option<Result<i64, ErrorKind>> {
            self.0.set(self.0.get() + 1);
            Some(Ok(1))
        }
    }

    let context = Counting(Cell::new(0));
    assert_eq!(evaluate_with("0 && a + f(b, c)", &context), Ok(0));
    assert_eq!(evaluate_with("1 || a", &context), Ok(1));
    assert_eq!(evaluate_with("1 ? 2 : g(a)", &context), Ok(2));
    assert_eq!(context.0.get(), 0);
    assert_eq!(evaluate_with("0 ? a : f(b)", &context), Ok(1));
    assert_eq!(context.0.get(), 2);
}

#[test]
fn skipped_branches_still_check_the_text() {
    assert_eq!(error("0 && (1 +"), (ErrorKind::UnexpectedEnd, 9, 9));
    assert_eq!(error("1 || 2 3"), (ErrorKind::UnexpectedToken, 7, 8));
    assert_eq!(error("1 ? 2 : )"), (ErrorKind::UnexpectedToken, 8, 9));
    assert_eq!(error("0 && 1.5"), (ErrorKind::InvalidNumber, 5, 8));
    assert_eq!(
        error("0 && f(1 # 2)"),
        (ErrorKind::UnexpectedCharacter, 9, 10)
    );
    let evaluator = Evaluator::<_, 2>::new(&()).max_depth(4);
    assert_eq!(
        evaluator.evaluate::<i64>("0 && f(1, 2, 3)").unwrap_err(),
        Error::new(ErrorKind::TooManyArguments, Span::new(13, 14))
    );
    assert_eq!(
        evaluator
            .evaluate::<i64>("0 && ((((1))))")
            .unwrap_err()
            .kind,
        ErrorKind::TooDeep
    );
}

#[test]
fn syntax_errors_take_precedence_over_evaluation_errors() {
    assert_eq!(error("1 / 0 + )"), (ErrorKind::UnexpectedToken, 8, 9));
    assert_eq!(error("x + 1.5"), (ErrorKind::InvalidNumber, 4, 7));
    assert_eq!(error("x + (1"), (ErrorKind::UnclosedParenthesis, 4, 5));
    assert_eq!(error("1 / 0 + x"), (ErrorKind::DivisionByZero, 0, 5));
    assert_eq!(error("x + 1 / 0"), (ErrorKind::UnknownVariable, 0, 1));
}

#[test]
fn conditional_syntax_errors() {
    assert_eq!(error("1 ? 2"), (ErrorKind::UnexpectedEnd, 5, 5));
    assert_eq!(error("1 ? 2 3"), (ErrorKind::UnexpectedToken, 6, 7));
    assert_eq!(error("1 ? : 2"), (ErrorKind::UnexpectedToken, 4, 5));
    assert_eq!(error("1 : 2"), (ErrorKind::UnexpectedToken, 2, 3));
    assert_eq!(error("?"), (ErrorKind::UnexpectedToken, 0, 1));
    assert_eq!(error("1 ? 2 : "), (ErrorKind::UnexpectedEnd, 8, 8));
    assert_eq!(error("1 <"), (ErrorKind::UnexpectedEnd, 3, 3));
    assert_eq!(error("!"), (ErrorKind::UnexpectedEnd, 1, 1));
    assert_eq!(error("1 && && 1"), (ErrorKind::UnexpectedToken, 5, 7));
    assert_eq!(error("1 = 1"), (ErrorKind::UnexpectedCharacter, 2, 3));
}

#[cfg(feature = "libm")]
#[test]
fn thresholds_and_rules() {
    let reading = |temperature: f64, humidity: f64| {
        let context = [("temperature", temperature), ("humidity", humidity)];
        evaluate_with::<f64, _>("temperature > 30 && humidity < 40", &context)
    };
    assert_eq!(reading(35.0, 20.0), Ok(1.0));
    assert_eq!(reading(35.0, 60.0), Ok(0.0));
    assert_eq!(reading(25.0, 20.0), Ok(0.0));
}

#[cfg(feature = "libm")]
#[test]
fn floats_reject_nan_in_comparisons_and_logic() {
    let context = [("nan", f64::NAN), ("x", 2.5)];
    let evaluate = |input| evaluate_with::<f64, _>(input, &context);
    assert_eq!(evaluate("x >= 2.5"), Ok(1.0));
    assert_eq!(evaluate("0.1 + 0.2 == 0.3"), Ok(0.0));
    assert_eq!(evaluate("x > 2 ? x * 2 : 0"), Ok(5.0));
    assert_eq!(
        evaluate("1 + (nan < 1)"),
        Err(Error::new(ErrorKind::Domain, Span::new(5, 12)))
    );
    assert_eq!(evaluate("nan == nan").unwrap_err().kind, ErrorKind::Domain);
    assert_eq!(
        evaluate("!nan"),
        Err(Error::new(ErrorKind::Domain, Span::new(0, 4)))
    );
    assert_eq!(
        evaluate("nan && 1"),
        Err(Error::new(ErrorKind::Domain, Span::new(0, 6)))
    );
    assert_eq!(
        evaluate("nan ? 1 : 2"),
        Err(Error::new(ErrorKind::Domain, Span::new(0, 5)))
    );
    assert_eq!(evaluate("0 && nan < 1"), Ok(0.0));
}

#[cfg(feature = "decimal")]
#[test]
fn decimals_compare_exactly() {
    use nanoexpr::Decimal;
    let evaluate = |input| nanoexpr::evaluate::<Decimal>(input).map(|value| value.to_string());
    assert_eq!(evaluate("0.1 + 0.2 == 0.3"), Ok("1".into()));
    assert_eq!(evaluate("-0.5 < 0.25 && !0"), Ok("1".into()));
    assert_eq!(evaluate("1 / 3 * 3 >= 1"), Ok("0".into()));
    assert_eq!(evaluate("0 ? 1 / 0 : 2.5"), Ok("2.5".into()));
}

#[test]
fn other_number_types() {
    assert_eq!(evaluate_with::<u8, _>("200 > 100", &()), Ok(1));
    assert_eq!(
        evaluate_with::<u8, _>("0 ? 1 : 0 - 1", &())
            .unwrap_err()
            .kind,
        ErrorKind::Overflow
    );
    assert_eq!(evaluate_with::<i128, _>("!(3 != 3)", &()), Ok(1));
}
