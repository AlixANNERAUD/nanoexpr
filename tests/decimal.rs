#![cfg(feature = "decimal")]

use nanoexpr::{Context, Decimal, ErrorKind, Fraction, Number};

fn decimal(text: &str) -> Decimal {
    text.parse()
        .unwrap_or_else(|error| panic!("{text}: {error}"))
}

fn expected(text: &str) -> Result<Decimal, ErrorKind> {
    match text {
        "Overflow" => Err(ErrorKind::Overflow),
        "DivisionByZero" => Err(ErrorKind::DivisionByZero),
        "Domain" => Err(ErrorKind::Domain),
        _ => Ok(decimal(text)),
    }
}

#[test]
fn arithmetic_matches_python_decimal() {
    let mut checked = 0;
    for line in include_str!("data/decimal.txt").lines() {
        let (operation, result) = line.split_once(" = ").unwrap();
        let actual = match operation.split(' ').collect::<Vec<_>>()[..] {
            [left, operator, right] => {
                let (left, right) = (decimal(left), decimal(right));
                match operator {
                    "+" => left.add(right),
                    "-" => left.subtract(right),
                    "*" => left.multiply(right),
                    "/" => left.divide(right),
                    "%" => left.remainder(right),
                    _ => unreachable!(),
                }
            }
            [literal] => literal.parse::<Decimal>().map_err(|error| error.kind),
            _ => unreachable!(),
        };
        assert_eq!(actual, expected(result), "{line}");
        checked += 1;
    }
    assert_eq!(checked, 4500);
}

fn matches_mpmath<const DIGITS: u32, const MAX_EXPONENT: i32>(data: &str) -> (usize, usize, usize) {
    let parse = |text: &str| -> Decimal<DIGITS, MAX_EXPONENT> {
        text.parse()
            .unwrap_or_else(|error| panic!("{text}: {error}"))
    };
    let one_ulp = |value: Decimal<DIGITS, MAX_EXPONENT>| {
        let exponent = if value.is_zero() {
            -MAX_EXPONENT
        } else {
            value.scientific_exponent() + 1 - DIGITS as i32
        };
        Decimal::new(1, exponent).unwrap()
    };
    let (mut checked, mut exact, mut close) = (0, 0, 0);
    for line in data.lines() {
        let (call, result) = line.split_once(" = ").unwrap();
        let (name, arguments) = call.split_once(' ').unwrap();
        let arguments: Vec<_> = arguments.split(' ').map(parse).collect();
        let actual = nanoexpr::Math.call(name, &arguments).unwrap();
        let expected = match result {
            "Overflow" => Err(ErrorKind::Overflow),
            "DivisionByZero" => Err(ErrorKind::DivisionByZero),
            "Domain" => Err(ErrorKind::Domain),
            _ => Ok(parse(result)),
        };
        checked += 1;
        match (actual, expected) {
            (Ok(actual), Ok(expected)) if actual == expected => exact += 1,
            (Ok(actual), Ok(expected)) => {
                let error = actual
                    .subtract(expected)
                    .and_then(|difference| difference.divide(one_ulp(expected)))
                    .unwrap()
                    .abs();
                assert!(
                    error <= Decimal::new(1, 0).unwrap(),
                    "{line}: got {actual}, {error} units in the last place away"
                );
                close += 1;
            }
            (actual, expected) => assert_eq!(actual, expected, "{line}"),
        }
    }
    (checked, exact, close)
}

/// Compares with the results of `uv run scripts/decimal-functions.py`.
#[test]
fn functions_match_mpmath() {
    let (checked, exact, close) =
        matches_mpmath::<14, 99>(include_str!("data/decimal-functions.txt"));
    println!(
        "14 digits: {checked} results, {exact} correctly rounded values, {close} one unit in the last place away"
    );
    assert_eq!(checked, 3985);
    assert!(
        close * 1000 <= checked,
        "{close} of {checked} values are not correctly rounded"
    );
    let (checked, exact, close) =
        matches_mpmath::<18, 9999>(include_str!("data/decimal-functions-18.txt"));
    println!(
        "18 digits: {checked} results, {exact} correctly rounded values, {close} one unit in the last place away"
    );
    assert_eq!(checked, 2151);
    assert!(
        close * 1000 <= checked,
        "{close} of {checked} values are not correctly rounded"
    );
}

fn evaluate(input: &str) -> Result<Decimal, ErrorKind> {
    nanoexpr::evaluate(input).map_err(|error| error.kind)
}

fn shown(input: &str) -> String {
    evaluate(input).unwrap().to_string()
}

fn calculator(input: &str) -> String {
    format!("{:.10}", evaluate(input).unwrap())
}

#[test]
fn decimal_fractions_are_exact() {
    assert_eq!(shown("0.1 + 0.2"), "0.3");
    assert_eq!(shown("1.1 * 1.1"), "1.21");
    assert_eq!(shown("0.3 - 0.1 - 0.2"), "0");
    assert_eq!(shown("19.99 * 3"), "59.97");
}

#[test]
fn display_hides_rounding_with_a_precision() {
    let one = evaluate("1 / 3 * 3").unwrap();
    assert_eq!(one.to_string(), "0.99999999999999");
    assert_eq!(format!("{one:.10}"), "1");
    assert_eq!(
        format!("{:.10}", evaluate("2 / 3").unwrap()),
        "0.6666666667"
    );
    assert_eq!(format!("{:.3}", evaluate("-9.9996").unwrap()), "-10");
    assert_eq!(format!("{:>8}", decimal("1.5")), "     1.5");
}

#[test]
fn display_switches_to_scientific_notation() {
    assert_eq!(decimal("123456789").to_string(), "123456789");
    assert_eq!(decimal("1e13").to_string(), "10000000000000");
    assert_eq!(decimal("1e14").to_string(), "1e14");
    assert_eq!(decimal("6.02e23").to_string(), "6.02e23");
    assert_eq!(decimal("0.00001").to_string(), "0.00001");
    assert_eq!(decimal("0.000001").to_string(), "1e-6");
    assert_eq!(decimal("-1.5e-99").to_string(), "-1.5e-99");
    assert_eq!(format!("{:.10}", decimal("12345678901")), "1.23456789e10");
    assert_eq!(<Decimal>::ZERO.to_string(), "0");
}

#[test]
fn range_is_bounded() {
    assert_eq!(
        evaluate("9.9999999999999e99 * 10"),
        Err(ErrorKind::Overflow)
    );
    assert_eq!(evaluate("1e-99 / 10"), Ok(<Decimal>::ZERO));
    assert_eq!(evaluate("1e100"), Err(ErrorKind::InvalidNumber));
    assert_eq!(
        "1e100".parse::<Decimal>().unwrap_err().kind,
        ErrorKind::Overflow
    );
    assert_eq!(
        "1.2.3".parse::<Decimal>().unwrap_err().kind,
        ErrorKind::InvalidNumber
    );
    assert_eq!(
        "".parse::<Decimal>().unwrap_err().kind,
        ErrorKind::InvalidNumber
    );
    assert_eq!(
        "e5".parse::<Decimal>().unwrap_err().kind,
        ErrorKind::InvalidNumber
    );
    assert_eq!(
        "1e".parse::<Decimal>().unwrap_err().kind,
        ErrorKind::InvalidNumber
    );
}

#[test]
fn tiny_differences_round_correctly() {
    assert_eq!(shown("1 - 1e-30"), "1");
    assert_eq!(shown("1 + 1e-30"), "1");
    assert_eq!(shown("1 - 1e-14"), "0.99999999999999");
    assert_eq!(shown("1e20 + 1 - 1e20"), "0");
}

#[test]
fn powers() {
    assert_eq!(shown("2 ^ 10"), "1024");
    assert_eq!(shown("2 ^ -2"), "0.25");
    assert_eq!(shown("(-2) ^ 3"), "-8");
    assert_eq!(shown("0 ^ 0"), "1");
    assert_eq!(shown("10 ^ -200"), "0");
    assert_eq!(shown("4 ^ 0.5"), "2");
    assert_eq!(evaluate("1.5 ^ 1e30"), Err(ErrorKind::Overflow));
    assert_eq!(shown("(-1) ^ 1e30 + 0.5 ^ 1e30"), "1");
    assert_eq!(evaluate("2 ^ 1000"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate("0 ^ -1"), Err(ErrorKind::DivisionByZero));
    assert_eq!(evaluate("(-8) ^ 0.5"), Err(ErrorKind::Domain));
}

#[test]
fn remainders() {
    assert_eq!(shown("7 mod 3"), "1");
    assert_eq!(shown("-7 mod 3"), "-1");
    assert_eq!(shown("7.5 mod 2"), "1.5");
    assert_eq!(shown("1 mod 3"), "1");
    assert_eq!(shown("1e50 mod 7"), "2");
    assert_eq!(evaluate("1 mod 0"), Err(ErrorKind::DivisionByZero));
}

#[test]
fn math_functions() {
    assert_eq!(shown("pi"), "3.1415926535898");
    assert_eq!(shown("e"), "2.718281828459");
    assert_eq!(shown("tau"), "6.2831853071796");
    assert_eq!(calculator("sqrt(2)"), "1.414213562");
    assert_eq!(shown("sqrt(2)"), "1.4142135623731");
    assert_eq!(shown("sqrt(16)"), "4");
    assert_eq!(calculator("sin(pi / 6)"), "0.5");
    assert_eq!(calculator("ln(e)"), "1");
    assert_eq!(calculator("log(1000)"), "3");
    assert_eq!(shown("abs(-2.5) + sign(-3) + sign(0)"), "1.5");
    assert_eq!(
        shown("floor(-2.5) + ceil(-2.5) + round(-2.5) + trunc(-2.5)"),
        "-10"
    );
    assert_eq!(
        shown("round(0.5) + round(0.49999999999999) + floor(0.001)"),
        "1"
    );
    assert_eq!(shown("floor(123456.7e-20)"), "0");
    assert_eq!(
        shown("min(3, 1.5, 2) + max(0.1, 0.2) + clamp(5, 0, 1)"),
        "2.7"
    );
    assert_eq!(calculator("pow(2, 0.5) ^ 2"), "2");
    assert_eq!(evaluate("sqrt(-1)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate("clamp(1, 2, 0)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate("exp(1000)"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate("sqrt(1, 2)"), Err(ErrorKind::ArgumentCount));
    assert_eq!(evaluate("min()"), Err(ErrorKind::ArgumentCount));
    assert_eq!(
        evaluate("hypot(1, 2, 3, 4, 5)"),
        Err(ErrorKind::ArgumentCount)
    );
    assert_eq!(evaluate("gamma(1)"), Err(ErrorKind::UnknownFunction));
}

#[test]
fn conversions() {
    assert_eq!(<Decimal>::try_from(-42_i64).unwrap().to_string(), "-42");
    assert_eq!(<Decimal>::try_from(0.1_f64).unwrap().to_string(), "0.1");
    assert_eq!(<Decimal>::try_from(f64::NAN), Err(ErrorKind::Domain));
    assert_eq!(<Decimal>::try_from(f64::INFINITY), Err(ErrorKind::Overflow));
    assert_eq!(<Decimal>::try_from(1e300), Err(ErrorKind::Overflow));
    assert_eq!(f64::from(decimal("-2.5")), -2.5);
    assert_eq!(
        Decimal::<14, 99>::new(1999, -2).unwrap().to_string(),
        "19.99"
    );
    assert!(decimal("-0").is_zero() && !decimal("-0").is_negative());
    assert_eq!(decimal("-3").abs(), decimal("3"));
    assert_eq!(decimal("123").scientific_exponent(), 2);
    assert_eq!(<Decimal>::default(), <Decimal>::ZERO);
}

#[test]
fn ordering() {
    let mut values = ["2", "-1e5", "0", "-3", "1e-5", "1e5", "-1e-5"].map(decimal);
    values.sort();
    assert_eq!(
        values.map(|value| value.to_string()),
        ["-100000", "-3", "-0.00001", "0", "0.00001", "2", "100000"]
    );
}

#[test]
fn other_precisions() {
    type Short = Decimal<6, 9>;
    let third: Short = nanoexpr::evaluate("1 / 3").unwrap();
    assert_eq!(third.to_string(), "0.333333");
    assert_eq!(
        nanoexpr::evaluate::<Short>("1e9 * 10").unwrap_err().kind,
        ErrorKind::Overflow
    );
    type Wide = Decimal<18, 9999>;
    let large: Wide = nanoexpr::evaluate("10 ^ 5000 / 3").unwrap();
    assert_eq!(large.to_string(), "3.33333333333333333e4999");
    let wide = |input: &str| nanoexpr::evaluate::<Wide>(input).map_err(|error| error.kind);
    assert_eq!(
        wide("exp(1000)").unwrap().to_string(),
        "1.97007111401704699e434"
    );
    assert_eq!(
        wide("ln(1e5000)").unwrap().to_string(),
        "11512.9254649702284"
    );
    assert_eq!(wide("log(1e5000)").unwrap().to_string(), "5000");
    assert_eq!(wide("exp(23026)"), Err(ErrorKind::Overflow));
    assert_eq!(wide("exp(-23030)"), Ok(Wide::ZERO));
    assert_eq!(
        wide("sin(1e340)").unwrap().to_string(),
        "-0.200751879558999975"
    );
    assert_eq!(
        wide("sin(9.99999999999999999e342)").unwrap().to_string(),
        "0.390489115761362875"
    );
    assert_eq!(wide("sin(1e343)"), Err(ErrorKind::Domain));
    assert_eq!(wide("cosh(1e9999)"), Err(ErrorKind::Overflow));
    assert_eq!(wide("tanh(-1e9999)").unwrap().to_string(), "-1");
    assert_eq!(
        wide("atan(1e9999) * 2").unwrap().to_string(),
        "3.14159265358979324"
    );
    let tiny: Short = nanoexpr::evaluate("sin(1e-9)").unwrap();
    assert_eq!(tiny.to_string(), "1e-9");
}

#[test]
fn functions_are_accurate_near_their_zeros() {
    assert_eq!(shown("sin(3.1415926535898)"), "-6.7615373566167e-15");
    assert_eq!(shown("sin(pi)"), "-6.7615373566167e-15");
    assert_eq!(shown("cos(1.5707963267949)"), "-3.3807686783084e-15");
    assert_eq!(shown("tan(1.5707963267949)"), "-2.9579071955327e14");
    assert_eq!(shown("sin(1e22)"), "-0.85220084976719");
    assert_eq!(shown("ln(1.0000000000001)"), "9.9999999999995e-14");
    assert_eq!(shown("acosh(1.0000000000001)"), "4.4721359549995e-7");
    assert_eq!(shown("sinh(1e-50) + asinh(1e-50) + atanh(1e-50)"), "3e-50");
    assert_eq!(shown("acos(0.99999999999999)"), "1.4142135623731e-7");
}

#[test]
fn exact_results_are_exact() {
    assert_eq!(
        shown("sqrt(16) + cbrt(-27) + log2(1024) + log(1e-50) + log(8, 2)"),
        "-36"
    );
    assert_eq!(
        shown("exp(0) + ln(1) + sin(0) + cos(0) + atan(0) + asin(0) + acos(1)"),
        "2"
    );
    assert_eq!(shown("hypot(3, 4) + 4 ^ 1.5 + 0.25 ^ 0.5"), "13.5");
    assert_eq!(shown("5 ^ 21"), "4.7683715820312e14");
    assert_eq!(shown("1.0000001 ^ 1024"), "1.0001024052379");
}

#[test]
fn functions_report_domain_errors() {
    for input in [
        "sqrt(-1)",
        "ln(0)",
        "ln(-1)",
        "log2(0)",
        "log10(-2)",
        "log(2, 1)",
        "log(2, 0)",
        "log(0, 2)",
        "asin(1.1)",
        "acos(-1.0000000000001)",
        "acosh(0.99999999999999)",
        "atanh(1)",
        "atanh(-1)",
        "(-8) ^ 0.5",
    ] {
        assert_eq!(evaluate(input), Err(ErrorKind::Domain), "{input}");
    }
    assert_eq!(evaluate("0 ^ -0.5"), Err(ErrorKind::DivisionByZero));
    assert_eq!(evaluate("sinh(300)"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate("exp(-300)"), Ok(<Decimal>::ZERO));
    assert_eq!(evaluate("(-8) ^ 1e20"), Err(ErrorKind::Overflow));
    assert_eq!(shown("(-2) ^ 3 + (-0.5) ^ 1e20"), "-8");
}

fn fraction_of(text: &str) -> Option<Fraction> {
    text.parse::<Decimal>().unwrap().to_fraction(9999)
}

fn fraction_shown(text: &str) -> Option<(String, String)> {
    fraction_of(text).map(|fraction| (fraction.to_string(), format!("{fraction:#}")))
}

fn pair(improper: &str, mixed: &str) -> Option<(String, String)> {
    Some((improper.into(), mixed.into()))
}

#[test]
fn finds_simple_fractions() {
    assert_eq!(fraction_shown("0"), pair("0", "0"));
    assert_eq!(fraction_shown("0.5"), pair("1/2", "1/2"));
    assert_eq!(fraction_shown("0.33333333333333"), pair("1/3", "1/3"));
    assert_eq!(fraction_shown("0.66666666666667"), pair("2/3", "2/3"));
    assert_eq!(fraction_shown("-3.5"), pair("-7/2", "-3 1/2"));
    assert_eq!(fraction_shown("0.99999999999999"), pair("1", "1"));
    assert_eq!(fraction_shown("0.3"), pair("3/10", "3/10"));
    assert_eq!(
        fraction_shown("1.0001000100010e-4"),
        pair("1/9999", "1/9999")
    );
    assert_eq!(fraction_shown("123456789"), pair("123456789", "123456789"));
    assert_eq!(fraction_shown("-42"), pair("-42", "-42"));
}

#[test]
fn rejects_numbers_without_a_close_fraction() {
    assert_eq!(fraction_of("3.1415926535898"), None);
    assert_eq!(fraction_of("1.4142135623731"), None);
    assert_eq!(fraction_of("9.9930048965724e-5"), None);
    assert_eq!(fraction_of("1e30"), None);
    assert_eq!(fraction_of("1e-50"), None);
    assert_eq!(fraction_of("0.33333333333336"), None);
}

#[test]
fn honours_the_denominator_bound() {
    let third: Decimal = "0.33333333333333".parse().unwrap();
    assert_eq!(third.to_fraction(2), None);
    assert_eq!(
        third.to_fraction(3),
        Some(Fraction {
            negative: false,
            numerator: 1,
            denominator: 3,
        })
    );
    let two: Decimal = "2".parse().unwrap();
    assert_eq!(two.to_fraction(0), None);
    let half = Decimal::<10, 99>::new(5, -1).unwrap();
    assert_eq!(
        half.to_fraction(u64::MAX).map(|f| f.to_string()),
        Some("1/2".into())
    );
}
