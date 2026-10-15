#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use nanoexpr::{Context, Decimal, ErrorKind, Math, Number};

#[derive(Arbitrary, Debug)]
enum Operand {
    Parts(i64, i32),
    Text(String),
    Float(f64),
}

#[derive(Arbitrary, Debug)]
struct Input {
    left: Operand,
    right: Operand,
    precision: Option<u16>,
    width: u8,
    max_denominator: u64,
    text: String,
    float: f64,
}

fn operand<const D: u32, const E: i32>(operand: &Operand) -> Option<Decimal<D, E>> {
    match operand {
        Operand::Parts(mantissa, exponent) => Decimal::new(*mantissa, *exponent).ok(),
        Operand::Text(text) => text.parse().ok(),
        Operand::Float(value) => Decimal::try_from(*value).ok(),
    }
}

fn round_trips<const D: u32, const E: i32>(value: Decimal<D, E>) {
    let text = value.to_string();
    let parsed: Decimal<D, E> = text
        .parse()
        .unwrap_or_else(|error| panic!("{value:?} shown as {text:?} does not parse: {error}"));
    assert_eq!(parsed, value, "{text:?}");
    assert_eq!(value.is_negative(), text.starts_with('-'));
    let negated = value.negate().unwrap();
    assert_eq!(negated.negate().unwrap(), value);
    assert!(!negated.is_negative() || !value.is_negative());
    if D <= 15 {
        assert_eq!(Decimal::<D, E>::try_from(value.to_f64()), Ok(value));
    }
}

fn format<const D: u32, const E: i32>(value: Decimal<D, E>, precision: Option<u16>, width: u8) {
    let width = usize::from(width);
    let text = match precision {
        Some(precision) => {
            let precision = usize::from(precision);
            let text = format!("{value:>width$.precision$}");
            let _ = format!("{value:<+0width$.precision$}");
            let shown = format!("{value:.precision$}");
            match shown.parse::<Decimal<D, E>>() {
                Ok(rounded) if precision as u32 >= D => assert_eq!(rounded, value, "{shown:?}"),
                Ok(_) => {}
                Err(error) if error.kind == ErrorKind::Overflow && (precision as u32) < D => {}
                Err(error) => panic!("{value:?} shown as {shown:?}: {error}"),
            }
            text
        }
        None => format!("{value:^width$}"),
    };
    assert!(text.chars().count() >= width);
    let _ = format!("{value:?}");
}

fn representable(value: f64) -> bool {
    value.is_normal() && value.abs() < 1e300 && value.abs() > 1e-300
}

fn close(actual: f64, expected: f64, digits: u32) -> bool {
    if !representable(expected) || !representable(actual) {
        return true;
    }
    ((actual - expected) / expected).abs() <= 10f64.powi(2 - digits as i32).max(1e-13)
}

const UNARY: [&str; 25] = [
    "abs", "sign", "floor", "ceil", "round", "trunc", "sqrt", "cbrt", "exp", "ln", "log2", "log10",
    "log", "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "asinh", "acosh",
    "atanh",
];

const BINARY: [&str; 6] = ["pow", "log", "atan2", "hypot", "min", "max"];

fn compared(name: &str, arguments: &[f64]) -> bool {
    let inside = |low: f64, high: f64| arguments.iter().all(|x| *x > low && *x < high);
    match name {
        "tan" | "pow" | "log" | "floor" | "ceil" | "round" | "trunc" | "sign" => false,
        "asin" | "acos" | "atanh" => inside(-0.999, 0.999),
        "acosh" => inside(1.001, 1e3),
        "ln" | "log2" | "log10" => inside(1e-300, 1e300),
        _ => inside(-1e3, 1e3),
    }
}

fn functions<const D: u32, const E: i32>(left: Decimal<D, E>, right: Decimal<D, E>) {
    let (l, r) = (left.to_f64(), right.to_f64());
    let (one, two, one_float, two_float) = ([left], [left, right], [l], [l, r]);
    let calls = UNARY
        .iter()
        .map(|name| (*name, &one[..], &one_float[..]))
        .chain(BINARY.iter().map(|name| (*name, &two[..], &two_float[..])));
    for (name, arguments, floats) in calls {
        let Some(result) = Math.call(name, arguments) else {
            panic!("{name} is unknown");
        };
        let Ok(value) = result else {
            continue;
        };
        round_trips(value);
        let exact = arguments
            .iter()
            .all(|argument| argument.is_zero() || representable(argument.to_f64()));
        if D < 14 || !exact || !compared(name, floats) {
            continue;
        }
        let Some(Ok(expected)) = Context::<f64>::call(&Math, name, floats) else {
            continue;
        };
        let actual = value.to_f64();
        assert!(
            (actual - expected).abs() <= 1e-7 * expected.abs().max(1.0),
            "{name}{arguments:?} = {value:?}, expected {expected}"
        );
    }
    let _ = Math.call("clamp", &[left, right, left]);
    let _ = Math.call("clamp", &[left, left, right]);
    let _ = Math.call("min", &[left, right, left]);
    let _ = Math.call("sqrt", &[left, right]);
}

fn fraction<const D: u32, const E: i32>(value: Decimal<D, E>, max_denominator: u64) {
    let Some(fraction) = value.to_fraction(max_denominator) else {
        return;
    };
    assert!(
        fraction.denominator() >= 1 && fraction.denominator() <= max_denominator,
        "{value:?} is {fraction:?}"
    );
    let (mut a, mut b) = (fraction.numerator(), fraction.denominator());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    assert!(
        a == 1 || fraction.numerator() == 0,
        "{value:?} is {fraction:?}, not in lowest terms"
    );
    assert!(fraction.numerator() != 0 || !fraction.is_negative());
    assert_eq!(
        fraction.is_negative(),
        value.is_negative() && fraction.numerator() != 0
    );
    let _ = format!("{fraction} {fraction:#}");
    let exact = fraction.numerator() as f64 / fraction.denominator() as f64;
    if representable(value.to_f64()) {
        assert!(
            close(exact, value.abs().to_f64(), D.min(12)),
            "{value:?} is {fraction:?}"
        );
    }
}

fn check<const D: u32, const E: i32>(input: &Input) {
    let _ = input.text.parse::<Decimal<D, E>>();
    let _ = <Decimal<D, E> as Number>::parse(&input.text);
    if let Ok(value) = Decimal::<D, E>::try_from(input.float) {
        round_trips(value);
        format(value, input.precision, input.width);
    }
    let (Some(left), Some(right)) = (operand::<D, E>(&input.left), operand::<D, E>(&input.right))
    else {
        return;
    };
    for value in [left, right] {
        fraction(value, input.max_denominator);
        fraction(value, 9999);
        round_trips(value);
        format(value, input.precision, input.width);
        let _ = (value.floor(), value.ceil(), value.round(), value.trunc());
        let _ = (value.abs(), value.scientific_exponent(), f64::from(value));
    }
    functions(left, right);
    let results = [
        left.add(right),
        left.subtract(right),
        left.multiply(right),
        left.divide(right),
        left.remainder(right),
        left.power(right),
    ];
    for value in results.iter().flatten() {
        round_trips(*value);
        format(*value, input.precision, input.width);
    }
    assert_eq!(left.add(right), right.add(left));
    assert_eq!(left.multiply(right), right.multiply(left));
    assert_eq!(left.subtract(left), Ok(Decimal::ZERO));
    assert_eq!(left.cmp(&right), right.cmp(&left).reverse());
    assert_eq!(
        left.cmp(&right) == core::cmp::Ordering::Equal,
        left == right
    );
    if right.is_zero() {
        assert_eq!(results[3], Err(ErrorKind::DivisionByZero));
        assert_eq!(results[4], Err(ErrorKind::DivisionByZero));
    }
    let (l, r) = (left.to_f64(), right.to_f64());
    if !representable(l) || !representable(r) {
        return;
    }
    if let Ok(product) = results[2] {
        assert!(
            close(product.to_f64(), l * r, D),
            "{left:?} * {right:?} = {product:?}"
        );
    }
    if let Ok(quotient) = results[3] {
        assert!(
            close(quotient.to_f64(), l / r, D),
            "{left:?} / {right:?} = {quotient:?}"
        );
    }
    if let Ok(remainder) = results[4] {
        assert!(
            remainder.abs() < right.abs() || right.is_zero(),
            "{left:?} % {right:?} = {remainder:?}"
        );
    }
}

fuzz_target!(|input: Input| {
    check::<14, 99>(&input);
    check::<3, 5>(&input);
    check::<1, 1>(&input);
    check::<15, 300>(&input);
    check::<18, 9999>(&input);
});
