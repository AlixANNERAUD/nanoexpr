#![cfg(feature = "fraction")]

use std::collections::HashSet;

use nanoexpr::{Error, ErrorKind, Fraction, Number, Span};

fn fraction(text: &str) -> Fraction {
    text.parse()
        .unwrap_or_else(|error| panic!("{text}: {error}"))
}

fn evaluate(input: &str) -> Result<Fraction, ErrorKind> {
    nanoexpr::evaluate(input).map_err(|error| error.kind)
}

fn shown(input: &str) -> String {
    evaluate(input)
        .unwrap_or_else(|error| panic!("{input}: {error}"))
        .to_string()
}

#[test]
fn expressions_match_python_fractions() {
    let (mut checked, mut values) = (0, 0);
    for line in include_str!("data/fraction.txt").lines() {
        let (input, result) = line.split_once(" = ").unwrap();
        let expected = match result {
            "Overflow" => Err(ErrorKind::Overflow),
            "DivisionByZero" => Err(ErrorKind::DivisionByZero),
            _ => Ok(fraction(result)),
        };
        assert_eq!(evaluate(input), expected, "{line}");
        checked += 1;
        values += usize::from(expected.is_ok());
    }
    assert_eq!((checked, values), (4000, 2871));
}

#[test]
fn arithmetic_is_exact() {
    assert_eq!(shown("1/3 + 1/6"), "1/2");
    assert_eq!(shown("2/3 * 3"), "2");
    assert_eq!(shown("1/3 * 3"), "1");
    assert_eq!(shown("0.1 + 0.2"), "3/10");
    assert_eq!(shown("1/3 - 1/2"), "-1/6");
    assert_eq!(shown("(1/7) / (2/21)"), "3/2");
    assert_eq!(shown("7/2 % 1"), "1/2");
    assert_eq!(shown("-7/2 % 1"), "-1/2");
    assert_eq!(shown("(7/2) % (-2/3)"), "1/6");
    assert_eq!(shown("1/2 % 3"), "1/2");
    assert_eq!(shown("-(1/3)"), "-1/3");
    assert_eq!(shown("1/3 - 1/3"), "0");
    assert_eq!(shown("1/4 == 0.25"), "1");
    assert_eq!(shown("1/3 < 0.33333333333333"), "0");
    assert_eq!(shown("1/3 > 0.3 && 1/3 < 0.34"), "1");
}

#[test]
fn literals_are_exact() {
    assert_eq!(shown("0.75"), "3/4");
    assert_eq!(shown("1e-3"), "1/1000");
    assert_eq!(shown("1.5e-3"), "3/2000");
    assert_eq!(shown(".5"), "1/2");
    assert_eq!(shown("5."), "5");
    assert_eq!(shown("2.50000000000000000000000000000000000000000"), "5/2");
    assert_eq!(
        shown("0.00000000000000000000000000000000000000001e40"),
        "1/10"
    );
    assert_eq!(shown("1e19"), "10000000000000000000");
    assert_eq!(shown("18446744073709551615"), "18446744073709551615");
    assert_eq!(shown("0e-1000000"), "0");
    for invalid in ["1e20", "18446744073709551616", "1e-20", "1e-999999"] {
        assert_eq!(
            nanoexpr::evaluate::<Fraction>(invalid),
            Err(Error::new(
                ErrorKind::InvalidNumber,
                Span::new(0, invalid.len())
            )),
            "{invalid}"
        );
    }
}

#[test]
fn results_are_reduced() {
    let value = evaluate("6/8 + 2/8").unwrap();
    assert_eq!((value.numerator(), value.denominator()), (1, 1));
    let value = Fraction::new(true, 150, 1000).unwrap();
    assert_eq!(
        (value.is_negative(), value.numerator(), value.denominator()),
        (true, 3, 20)
    );
    assert_eq!(
        Fraction::new(true, 0, 5).unwrap(),
        Fraction::ZERO,
        "zero is never negative"
    );
    assert_eq!(Fraction::new(false, 1, 0), Err(ErrorKind::DivisionByZero));
    assert_eq!(Fraction::from(-4_i64), fraction("-8/2"));
    assert_eq!(Fraction::from(u64::MAX).to_string(), "18446744073709551615");
    assert_eq!(Fraction::from(i64::MIN).to_string(), "-9223372036854775808");
}

#[test]
fn overflow_is_detected_at_the_limits() {
    let max = "18446744073709551615";
    assert_eq!(evaluate(&format!("{max} + 1")), Err(ErrorKind::Overflow));
    assert_eq!(evaluate(&format!("-{max} - 1")), Err(ErrorKind::Overflow));
    assert_eq!(shown(&format!("{max} - 1 + 1")), max);
    assert_eq!(shown(&format!("-{max} + {max}")), "0");
    assert_eq!(evaluate(&format!("{max} * 2")), Err(ErrorKind::Overflow));
    assert_eq!(
        evaluate(&format!("1 / {max} / 2")),
        Err(ErrorKind::Overflow)
    );
    assert_eq!(shown(&format!("1 / {max} * 2")), format!("2/{max}"));
    assert_eq!(
        evaluate(&format!("1 / {max} + 1 / ({max} - 1)")),
        Err(ErrorKind::Overflow)
    );
    assert_eq!(
        shown(&format!("{max} / ({max} - 1) - 1")),
        "1/18446744073709551614"
    );
    assert_eq!(shown(&format!("({max} / 3) * (3 / {max})")), "1");
    assert_eq!(evaluate("2 ^ 64"), Err(ErrorKind::Overflow));
    assert_eq!(shown("2 ^ 63 - 1 + 2 ^ 63"), max);
    assert_eq!(evaluate("2 ^ (-64)"), Err(ErrorKind::Overflow));
    assert_eq!(shown("1 ^ 1e19 + (-1) ^ 1e19 + 0 ^ 1e19"), "2");
    assert_eq!(shown("(-1) ^ 18446744073709551615"), "-1");
    assert_eq!(evaluate("2 ^ 1e19"), Err(ErrorKind::Overflow));
}

#[test]
fn powers_are_exact_when_rational() {
    assert_eq!(shown("(2/3) ^ 3"), "8/27");
    assert_eq!(shown("(2/3) ^ (-2)"), "9/4");
    assert_eq!(shown("(-2/3) ^ 3"), "-8/27");
    assert_eq!(shown("(-2) ^ (-1)"), "-1/2");
    assert_eq!(shown("0 ^ 0"), "1");
    assert_eq!(shown("(4/9) ^ (1/2)"), "2/3");
    assert_eq!(shown("8 ^ (1/3)"), "2");
    assert_eq!(shown("8 ^ (2/3)"), "4");
    assert_eq!(shown("8 ^ (-2/3)"), "1/4");
    assert_eq!(shown("(-8) ^ (1/3)"), "-2");
    assert_eq!(shown("(-8) ^ (2/3)"), "4");
    assert_eq!(shown("0 ^ (1/2)"), "0");
    assert_eq!(shown("1 ^ (1/1000)"), "1");
    assert_eq!(shown("(2 ^ 63) ^ (1/63)"), "2");
    assert_eq!(
        shown("18446744073709551615 ^ (1/1)"),
        "18446744073709551615"
    );
    assert_eq!(shown("(4294967295 ^ 2) ^ 0.5"), "4294967295");
    assert_eq!(evaluate("2 ^ (1/2)"), Err(ErrorKind::Inexact));
    assert_eq!(evaluate("(2 ^ 63) ^ (1/64)"), Err(ErrorKind::Inexact));
    assert_eq!(evaluate("(4/5) ^ (1/2)"), Err(ErrorKind::Inexact));
    assert_eq!(evaluate("(-4) ^ (1/2)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate("0 ^ (-1)"), Err(ErrorKind::DivisionByZero));
    assert_eq!(evaluate("0 ^ (-1/2)"), Err(ErrorKind::DivisionByZero));
}

#[test]
fn division_by_zero() {
    assert_eq!(evaluate("1 / 0"), Err(ErrorKind::DivisionByZero));
    assert_eq!(evaluate("1 / (1/2 - 0.5)"), Err(ErrorKind::DivisionByZero));
    assert_eq!(evaluate("1 % 0"), Err(ErrorKind::DivisionByZero));
    assert_eq!(
        nanoexpr::evaluate::<Fraction>("1 + 2 / (1/3 - 1/3)"),
        Err(Error::new(ErrorKind::DivisionByZero, Span::new(4, 19)))
    );
}

#[test]
fn math_functions_are_exact() {
    assert_eq!(shown("abs(-3/4) + sign(-2/3) * 10"), "-37/4");
    assert_eq!(shown("sign(0)"), "0");
    assert_eq!(shown("floor(7/2)"), "3");
    assert_eq!(shown("floor(-7/2)"), "-4");
    assert_eq!(shown("ceil(7/2)"), "4");
    assert_eq!(shown("ceil(-7/2)"), "-3");
    assert_eq!(shown("trunc(-7/2)"), "-3");
    assert_eq!(shown("round(5/2)"), "3");
    assert_eq!(shown("round(-5/2)"), "-3");
    assert_eq!(shown("round(7/3)"), "2");
    assert_eq!(shown("round(-8/3)"), "-3");
    assert_eq!(
        shown("floor(18446744073709551615 / 2)"),
        "9223372036854775807"
    );
    assert_eq!(
        shown("ceil(18446744073709551615 / 2)"),
        "9223372036854775808"
    );
    assert_eq!(shown("sqrt(4/9)"), "2/3");
    assert_eq!(shown("cbrt(-27/8)"), "-3/2");
    assert_eq!(shown("hypot(3/5, 4/5)"), "1");
    assert_eq!(shown("pow(4/9, -1/2)"), "3/2");
    assert_eq!(shown("min(1/2, 1/3, 2/3) + max(1/2, 1/3)"), "5/6");
    assert_eq!(shown("clamp(5/2, 0, 1) + clamp(1/3, 0, 1)"), "4/3");
    assert_eq!(evaluate("clamp(1, 1, 0)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate("sqrt(-4)"), Err(ErrorKind::Domain));
    for inexact in [
        "sqrt(2)",
        "cbrt(2)",
        "hypot(1, 1)",
        "exp(0)",
        "ln(1)",
        "log(100)",
        "log(8, 2)",
        "sin(0)",
        "atan2(1, 1)",
        "atanh(0)",
    ] {
        assert_eq!(evaluate(inexact), Err(ErrorKind::Inexact), "{inexact}");
    }
    assert_eq!(evaluate("sin(1, 2)"), Err(ErrorKind::ArgumentCount));
    assert_eq!(evaluate("sqrt()"), Err(ErrorKind::ArgumentCount));
    assert_eq!(evaluate("min()"), Err(ErrorKind::ArgumentCount));
    assert_eq!(evaluate("pi"), Err(ErrorKind::UnknownVariable));
    assert_eq!(evaluate("gamma(1)"), Err(ErrorKind::UnknownFunction));
}

#[test]
fn display_and_parsing() {
    assert_eq!(fraction("3/4").to_string(), "3/4");
    assert_eq!(fraction("-5/4").to_string(), "-5/4");
    assert_eq!(format!("{:#}", fraction("-5/4")), "-1 1/4");
    assert_eq!(format!("{:#}", fraction("3/4")), "3/4");
    assert_eq!(format!("{:#}", fraction("8/4")), "2");
    assert_eq!(fraction("7").to_string(), "7");
    assert_eq!(fraction("+7").to_string(), "7");
    assert_eq!(fraction("1.5/2").to_string(), "3/4");
    assert_eq!(fraction("0.125").to_string(), "1/8");
    assert_eq!(fraction("-0").to_string(), "0");
    for (text, kind) in [
        ("", ErrorKind::InvalidNumber),
        ("-", ErrorKind::InvalidNumber),
        ("1/", ErrorKind::InvalidNumber),
        ("1/-2", ErrorKind::InvalidNumber),
        ("1 / 2", ErrorKind::InvalidNumber),
        ("1/2/3", ErrorKind::InvalidNumber),
        ("x", ErrorKind::InvalidNumber),
        ("1/0", ErrorKind::DivisionByZero),
        ("1e30", ErrorKind::Overflow),
    ] {
        assert_eq!(
            text.parse::<Fraction>().map_err(|error| error.kind),
            Err(kind),
            "{text}"
        );
    }
    assert_eq!(
        "1/0".parse::<Fraction>().unwrap_err().to_string(),
        "division by zero"
    );
}

#[test]
fn ordering_and_hashing() {
    let mut values = ["1/2", "-1/3", "0", "1/3", "-2", "5/2", "-1/2"].map(fraction);
    values.sort();
    assert_eq!(
        values.map(|value| value.to_string()),
        ["-2", "-1/2", "-1/3", "0", "1/3", "1/2", "5/2"]
    );
    let set: HashSet<Fraction> = ["1/2", "2/4", "0.5", "-0"].map(fraction).into();
    assert_eq!(set.len(), 2);
    assert_eq!(Fraction::default(), Fraction::ZERO);
    assert!(fraction("18446744073709551615/2") > fraction("18446744073709551614/2"));
    assert!(fraction("1/18446744073709551615") < fraction("1/18446744073709551614"));
}

#[test]
fn conversions() {
    assert_eq!(f64::from(fraction("-3/4")), -0.75);
    assert_eq!(fraction("1/3").to_f64(), 1.0 / 3.0);
    assert_eq!(Fraction::ONE.add(Fraction::ONE), Ok(fraction("2")));
    assert_eq!(Fraction::from_bool(true), Ok(Fraction::ONE));
    assert_eq!(Fraction::parse("1/2"), None);
}

#[cfg(feature = "decimal")]
#[test]
fn decimal_conversions() {
    use nanoexpr::Decimal;

    let decimal = |text: &str| text.parse::<Decimal>().unwrap();
    assert_eq!(
        <Decimal>::try_from(fraction("1/3")).unwrap().to_string(),
        "0.33333333333333"
    );
    assert_eq!(
        <Decimal>::try_from(fraction("2/3")).unwrap().to_string(),
        "0.66666666666667"
    );
    assert_eq!(
        <Decimal>::try_from(fraction("-18446744073709551615"))
            .unwrap()
            .to_string(),
        "-1.844674407371e19"
    );
    assert_eq!(
        <Decimal>::try_from(fraction("1/18446744073709551615"))
            .unwrap()
            .to_string(),
        "5.4210108624275e-20"
    );
    assert_eq!(
        Decimal::<6, 9>::try_from(fraction("10000000000")),
        Err(ErrorKind::Overflow)
    );
    assert_eq!(
        Decimal::<6, 9>::try_from(fraction("1/10000000000")),
        Ok(Decimal::ZERO)
    );
    let rounded = |text: &str| {
        Decimal::<6, 99>::try_from(fraction(text))
            .unwrap()
            .to_string()
    };
    assert_eq!(rounded("1234565/100"), "12345.6");
    assert_eq!(rounded("1234575/100"), "12345.8");
    assert_eq!(rounded("1234565001/100000"), "12345.7");
    assert_eq!(Fraction::try_from(decimal("0.125")), Ok(fraction("1/8")));
    assert_eq!(
        Fraction::try_from(decimal("-0.33333333333333")),
        Ok(fraction("-33333333333333/100000000000000"))
    );
    assert_eq!(
        Fraction::try_from(decimal("1.5e18")),
        Ok(fraction("1500000000000000000"))
    );
    assert_eq!(
        Fraction::try_from(decimal("1e20")),
        Err(ErrorKind::Overflow)
    );
    assert_eq!(
        Fraction::try_from(decimal("1.2345678901234e-10")),
        Err(ErrorKind::Overflow)
    );
    assert_eq!(Fraction::try_from(decimal("0")), Ok(Fraction::ZERO));
    let third: Decimal = nanoexpr::evaluate("1/3").unwrap();
    assert_eq!(third.to_fraction(9999), Some(fraction("1/3")));
}
