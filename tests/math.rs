#![cfg(feature = "libm")]

use core::f64::consts::{E, PI, TAU};

use nanoexpr::{Error, ErrorKind, Math, Span, evaluate, evaluate_with};

fn close(input: &str, expected: f64) {
    let actual = evaluate::<f64>(input).unwrap();
    assert!(
        (actual - expected).abs() < 1e-12,
        "{input} = {actual}, expected {expected}"
    );
}

fn kind(input: &str) -> ErrorKind {
    evaluate::<f64>(input).unwrap_err().kind
}

#[test]
fn constants() {
    close("pi", PI);
    close("PI", PI);
    close("tau", TAU);
    close("e", E);
    close("E", E);
}

#[test]
fn functions() {
    close("sqrt(16)", 4.0);
    close("cbrt(-27)", -3.0);
    close("abs(-5)", 5.0);
    close("sign(-3) + sign(0) * 10 + sign(2) * 100", 99.0);
    close("exp(0) + ln(e)", 2.0);
    close("log2(8) + log10(1000) + log(100)", 8.0);
    close("log(8, 2)", 3.0);
    close("sin(pi / 2) + cos(0) + tan(0)", 2.0);
    close("asin(1) + acos(1) + atan(0)", PI / 2.0);
    close("atan2(1, 1)", PI / 4.0);
    close(
        "sinh(0) + cosh(0) + tanh(0) + asinh(0) + acosh(1) + atanh(0)",
        1.0,
    );
    close("floor(2.7) + ceil(2.1) + round(2.5) + trunc(-2.7)", 6.0);
    close("pow(2, 10) + hypot(3, 4)", 1029.0);
    close("min(3, 1, 2) + max(3, 1, 2) + min(7)", 11.0);
    close("clamp(5, 0, 1) + clamp(-5, 0, 1) + clamp(0.5, 0, 1)", 1.5);
}

#[test]
fn single_precision() {
    assert_eq!(
        evaluate::<f32>("sqrt(2) ^ 2").map(|value| (value - 2.0).abs() < 1e-6),
        Ok(true)
    );
    assert_eq!(evaluate::<f32>("hypot(3, 4)"), Ok(5.0));
}

#[test]
fn domain_errors() {
    assert_eq!(kind("sqrt(-1)"), ErrorKind::Domain);
    assert_eq!(kind("ln(0)"), ErrorKind::Domain);
    assert_eq!(kind("log(-1)"), ErrorKind::Domain);
    assert_eq!(kind("log(8, 1)"), ErrorKind::Domain);
    assert_eq!(kind("asin(2)"), ErrorKind::Domain);
    assert_eq!(kind("acosh(0)"), ErrorKind::Domain);
    assert_eq!(kind("atanh(1)"), ErrorKind::Domain);
    assert_eq!(kind("clamp(0, 1, 0)"), ErrorKind::Domain);
}

#[test]
fn overflow() {
    assert_eq!(kind("exp(1000)"), ErrorKind::Overflow);
    assert_eq!(kind("sinh(1000)"), ErrorKind::Overflow);
}

#[test]
fn argument_count() {
    assert_eq!(
        evaluate::<f64>("1 + sqrt(1, 2)").unwrap_err(),
        Error::new(ErrorKind::ArgumentCount, Span::new(4, 14))
    );
    assert_eq!(kind("min()"), ErrorKind::ArgumentCount);
    assert_eq!(kind("atan2(1)"), ErrorKind::ArgumentCount);
}

#[test]
fn unknown_names() {
    assert_eq!(kind("x"), ErrorKind::UnknownVariable);
    assert_eq!(kind("gamma(1)"), ErrorKind::UnknownFunction);
}

#[test]
fn custom_context_takes_precedence() {
    let context = ([("e", 0.0), ("x", 2.0)], Math);
    assert_eq!(evaluate_with("e + x * pi", &context), Ok(2.0 * PI));
}
