#![cfg(feature = "exact")]

use nanoexpr::{Decimal, Error, ErrorKind, Exact, Fraction, Span};

fn evaluate(input: &str) -> Result<Exact, ErrorKind> {
    nanoexpr::evaluate(input).map_err(|error| error.kind)
}

fn exact(input: &str) -> String {
    let value = evaluate(input).unwrap_or_else(|error| panic!("{input}: {error}"));
    assert!(value.is_exact(), "{input} = {value:?} is not exact");
    value.to_string()
}

fn decimal(input: &str) -> String {
    let value = evaluate(input).unwrap_or_else(|error| panic!("{input}: {error}"));
    assert!(!value.is_exact(), "{input} = {value:?} is exact");
    value.to_string()
}

#[test]
fn fractions_stay_exact() {
    assert_eq!(exact("1/3 + 1/6"), "1/2");
    assert_eq!(exact("2/3 * 3"), "2");
    assert_eq!(exact("1/3 * 3"), "1");
    assert_eq!(exact("0.1 + 0.2"), "3/10");
    assert_eq!(exact("sqrt(4/9)"), "2/3");
    assert_eq!(exact("(4/9) ^ (1/2)"), "2/3");
    assert_eq!(exact("8 ^ (1/3)"), "2");
    assert_eq!(exact("(-8) ^ (1/3)"), "-2");
    assert_eq!(exact("abs(-3/4) + floor(7/2) + round(-5/2)"), "3/4");
    assert_eq!(exact("hypot(3, 4) / 10"), "1/2");
    assert_eq!(exact("7/2 % 1"), "1/2");
    assert_eq!(exact("-(1/3)"), "-1/3");
    assert_eq!(exact("1e19"), "10000000000000000000");
}

#[test]
fn inexact_results_become_decimals() {
    assert_eq!(decimal("sqrt(2)"), "1.4142135623731");
    assert_eq!(decimal("1/3 + sqrt(2)"), "1.7475468957064");
    assert_eq!(decimal("sin(1)"), "0.8414709848079");
    assert_eq!(decimal("pi"), "3.1415926535898");
    assert_eq!(decimal("2 * pi"), "6.2831853071796");
    assert_eq!(decimal("e - e"), "0");
    assert_eq!(decimal("2 ^ (1/2)"), "1.4142135623731");
    assert_eq!(decimal("pow(2, 1/2)"), "1.4142135623731");
    assert_eq!(decimal("ln(1)"), "0");
    assert_eq!(decimal("sqrt(2) ^ 2"), "2");
    assert_eq!(decimal("sqrt(2) * 0 + 1/2"), "0.5");
    assert_eq!(decimal("1e30 / 3"), "3.3333333333333e29");
    assert_eq!(decimal("1e-30 * 3"), "3e-30");
    assert_eq!(decimal("(2 ^ 62) / 3 * 7"), "1.0760600709664e19");
    assert_eq!(decimal("2 ^ 64"), "1.844674407371e19");
    assert_eq!(
        decimal("1 / 18446744073709551615 / 2"),
        "2.7105054312138e-20"
    );
    assert_eq!(decimal("(1/3) ^ 100"), "1.9403252174826e-48");
    assert_eq!(evaluate("2 ^ (2 ^ 62)"), Err(ErrorKind::Overflow));
    assert_eq!(exact("(-2/3) ^ 3"), "-8/27");
    assert_eq!(decimal("(-2/3) ^ 41"), "-6.0291817892111e-8");
    let power = evaluate("(-2/3) ^ 42").unwrap();
    assert!(!power.is_exact());
    assert_eq!(format!("{power:.13}"), "4.019454526141e-8");
}

#[test]
fn errors_and_spans_are_those_of_decimals() {
    for input in [
        "1 + 4 / (2 - 2)",
        "1 % (1/3 - 1/3)",
        "sqrt(-2)",
        "sqrt(-4/9)",
        "(-4) ^ (1/2)",
        "(-2) ^ (1/3)",
        "0 ^ (-1)",
        "ln(0)",
        "asin(2)",
        "10 ^ 100",
        "clamp(1, 2, 0)",
        "min()",
        "sqrt(1, 2, 3, 4)",
        "1 +",
        "(1",
        "gamma(1)",
        "x + 1",
        "1e1000000",
        "1 / 0 > 1 || 2",
    ] {
        let expected = nanoexpr::evaluate::<Decimal>(input).map(|_| ());
        let actual = nanoexpr::evaluate::<Exact>(input).map(|_| ());
        assert!(expected.is_err(), "{input}");
        assert_eq!(actual, expected, "{input}");
    }
    assert_eq!(
        nanoexpr::evaluate::<Exact>("1 + 4 / (2/3 - 2/3)"),
        Err(Error::new(ErrorKind::DivisionByZero, Span::new(4, 19)))
    );
}

#[test]
fn comparisons_are_exact() {
    assert_eq!(exact("1/2 == 0.5"), "1");
    assert_eq!(exact("1/3 == 0.33333333333333"), "0");
    assert_eq!(exact("1/3 > 0.33333333333333"), "1");
    assert_eq!(exact("sqrt(2) > 7/5 && sqrt(2) < 3/2"), "1");
    assert_eq!(exact("sqrt(4) == 2"), "1");
    assert_eq!(exact("1e30 > 18446744073709551615"), "1");
    assert_eq!(exact("-1e30 < -18446744073709551615"), "1");
    assert_eq!(exact("1e-30 < 1/18446744073709551615"), "1");
    assert_eq!(exact("-1e-30 > -1/18446744073709551615"), "1");
    assert_eq!(exact("0.5 * sqrt(4) == 1"), "1");
    assert_eq!(exact("sin(0) == 0"), "1");
    assert_eq!(exact("sqrt(2) ? 1/2 : 1/3"), "1/2");
    assert_eq!(exact("sin(0) ? 1/2 : 1/3"), "1/3");
}

#[test]
fn min_max_and_clamp_keep_their_arguments() {
    assert_eq!(exact("max(1/3, 0.2 * sqrt(1))"), "1/3");
    assert_eq!(decimal("max(1/3, sqrt(2))"), "1.4142135623731");
    assert_eq!(exact("min(1/2, sqrt(2), 1/3)"), "1/3");
    assert_eq!(exact("clamp(5, 1/3, sqrt(4) * 0 + 1/2)"), "1/2");
    assert_eq!(decimal("clamp(5, 1/3, sqrt(3))"), "1.7320508075689");
    assert_eq!(exact("clamp(sqrt(3), 1/3, 1/2)"), "1/2");
}

#[test]
fn display() {
    let third = evaluate("1/3").unwrap();
    assert_eq!(third.to_string(), "1/3");
    assert_eq!(format!("{third:.5}"), "1/3");
    assert_eq!(third.to_decimal().to_string(), "0.33333333333333");
    assert_eq!(format!("{:.5}", third.to_decimal()), "0.33333");
    let mixed = evaluate("-7/2").unwrap();
    assert_eq!(format!("{mixed:#}"), "-3 1/2");
    let root = evaluate("sqrt(2)").unwrap();
    assert_eq!(format!("{root:.10}"), "1.414213562");
    assert_eq!(format!("{root:#}"), "1.4142135623731");
    assert_eq!(
        format!("{third:?} {root:?}"),
        "Fraction(1/3) Decimal(1.4142135623731)"
    );
}

#[test]
fn accessors_and_conversions() {
    let half = evaluate("1/2").unwrap();
    assert_eq!(half.as_fraction(), Some("1/2".parse::<Fraction>().unwrap()));
    assert_eq!(half.as_decimal(), None);
    let decimal_half = evaluate("sqrt(1/4) * 0 + 0.5 * sin(pi / 2)").unwrap();
    assert!(!decimal_half.is_exact());
    assert_eq!(decimal_half.as_fraction(), None);
    assert_eq!(decimal_half.as_decimal(), Some("0.5".parse().unwrap()));
    assert_ne!(half, decimal_half);
    assert_eq!(half.partial_cmp(&decimal_half), None);
    let third = evaluate("1/3").unwrap();
    assert!(third < decimal_half && decimal_half > third && third <= third);
    assert_eq!(half.to_decimal(), decimal_half.to_decimal());
    assert_eq!(Exact::<14, 99>::from(Fraction::ONE).to_string(), "1");
    assert_eq!(Exact::<14, 99>::from(Decimal::ZERO).to_string(), "0");
    assert_eq!(Exact::<14, 99>::default(), Exact::from(Fraction::ZERO));
    let short: Exact<6, 19> = nanoexpr::evaluate("1/3 + sqrt(2)").unwrap();
    assert_eq!(short.to_string(), "1.74754");
    let big: Exact<6, 19> = nanoexpr::evaluate("18446744073709551615").unwrap();
    assert_eq!(big.to_decimal().to_string(), "1.84467e19");
}

#[test]
fn fraction_expressions_stay_exact_until_they_overflow() {
    let (mut checked, mut exact) = (0, 0);
    for line in include_str!("data/fraction.txt").lines() {
        let (input, result) = line.split_once(" = ").unwrap();
        let actual = evaluate(input);
        match result {
            "DivisionByZero" => assert_eq!(actual, Err(ErrorKind::DivisionByZero), "{line}"),
            "Overflow" => assert!(
                actual.is_err() || actual.is_ok_and(|value| !value.is_exact()),
                "{line}: {actual:?}"
            ),
            _ => {
                assert_eq!(
                    actual.map(|value| value.as_fraction()),
                    Ok(Some(result.parse().unwrap())),
                    "{line}"
                );
                exact += 1;
            }
        }
        checked += 1;
    }
    assert_eq!((checked, exact), (4000, 2871));
}
