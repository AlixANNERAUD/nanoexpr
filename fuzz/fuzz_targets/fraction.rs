#![no_main]

use core::cmp::Ordering;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use nanoexpr::{Decimal, ErrorKind, Exact, Fraction, Number};

#[derive(Arbitrary, Debug)]
enum Operand {
    Parts(bool, u64, u64),
    Small(bool, u8, u8),
    Integer(i64),
    Text(String),
    Decimal(i64, i8),
}

#[derive(Arbitrary, Debug)]
struct Input {
    left: Operand,
    right: Operand,
    text: String,
}

fn operand(operand: &Operand) -> Option<Fraction> {
    match operand {
        Operand::Parts(negative, numerator, denominator) => {
            Fraction::new(*negative, *numerator, *denominator).ok()
        }
        Operand::Small(negative, numerator, denominator) => {
            Fraction::new(*negative, u64::from(*numerator), u64::from(*denominator)).ok()
        }
        Operand::Integer(value) => Some(Fraction::from(*value)),
        Operand::Text(text) => text.parse().ok(),
        Operand::Decimal(mantissa, exponent) => {
            Decimal::<18, 9999>::new(*mantissa, i32::from(*exponent))
                .ok()
                .and_then(|decimal| Fraction::try_from(decimal).ok())
        }
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn valid(value: Fraction) {
    assert!(value.denominator() >= 1, "{value:?}");
    assert_eq!(gcd(value.numerator(), value.denominator()), 1, "{value:?}");
    assert!(!(value.is_negative() && value.is_zero()), "{value:?}");
    let text = value.to_string();
    assert_eq!(text.parse::<Fraction>(), Ok(value), "{text}");
    let _ = format!("{value:#} {value:>40} {value:?}");
    assert_eq!(value.negate().and_then(Number::negate), Ok(value));
    assert!(value.floor() <= value && value <= value.ceil(), "{value:?}");
    assert!(value.trunc().abs() <= value.abs(), "{value:?}");
    let _ = value.round();
    assert_eq!(value.sign().multiply(value.abs()), Ok(value));
    decimals(value);
}

fn decimals(value: Fraction) {
    if let Ok(decimal) = Decimal::<14, 99>::try_from(value) {
        let back = Fraction::try_from(decimal);
        if value.denominator() == 1 && value.numerator() < 10_u64.pow(14) {
            assert_eq!(back, Ok(value), "{value:?} as {decimal:?}");
        }
        if let Ok(back) = back {
            assert_eq!(
                Decimal::<14, 99>::try_from(back),
                Ok(decimal),
                "{value:?} as {decimal:?}"
            );
        }
        let expected = value.to_f64();
        let actual = decimal.to_f64();
        assert!(
            (actual - expected).abs() <= 1e-12 * expected.abs(),
            "{value:?} as {decimal:?}"
        );
    }
    let exact = Exact::<14, 99>::from(value);
    assert!(exact.is_exact());
    assert_eq!(exact.as_fraction(), Some(value));
    assert_eq!(exact.to_string(), value.to_string());
    if let Ok(decimal) = Decimal::<18, 9999>::try_from(value) {
        if Fraction::try_from(decimal) == Ok(value) {
            let left = Exact::<18, 9999>::from(value);
            let right = Exact::<18, 9999>::from(decimal);
            assert_eq!(
                left.compare(right),
                Ok(Ordering::Equal),
                "{value:?} and {decimal:?}"
            );
        }
    }
}

fn arithmetic(left: Fraction, right: Fraction) {
    let sum = left.add(right);
    assert_eq!(sum, right.add(left));
    let product = left.multiply(right);
    assert_eq!(product, left.multiply(right));
    let difference = left.subtract(right);
    let quotient = left.divide(right);
    let remainder = left.remainder(right);
    let power = left.power(right);
    for value in [sum, difference, product, quotient, remainder, power]
        .into_iter()
        .flatten()
    {
        valid(value);
    }
    if let Ok(sum) = sum {
        assert_eq!(
            sum.subtract(right),
            Ok(left),
            "{left:?} + {right:?} = {sum:?}"
        );
    }
    if let (Ok(product), false) = (product, right.is_zero()) {
        assert_eq!(
            product.divide(right),
            Ok(left),
            "{left:?} * {right:?} = {product:?}"
        );
    }
    if right.is_zero() {
        assert_eq!(quotient, Err(ErrorKind::DivisionByZero));
        assert_eq!(remainder, Err(ErrorKind::DivisionByZero));
    }
    if let Ok(remainder) = remainder {
        assert!(
            remainder.abs() < right.abs(),
            "{left:?} % {right:?} = {remainder:?}"
        );
        assert!(
            remainder.is_zero() || remainder.is_negative() == left.is_negative(),
            "{left:?} % {right:?} = {remainder:?}"
        );
    }
    if let Ok(quotient) = quotient {
        if let Ok(back) = quotient.multiply(right) {
            assert_eq!(back, left, "{left:?} / {right:?} = {quotient:?}");
        }
    }
    assert_eq!(left.cmp(&right), right.cmp(&left).reverse());
    assert_eq!(left.compare(right), Ok(left.cmp(&right)));
    if left < right {
        let (l, r) = (left.to_f64(), right.to_f64());
        assert!(l <= r + 4.0 * f64::EPSILON * r.abs(), "{left:?} < {right:?}");
    }
    assert_eq!(left.power(Fraction::ONE), Ok(left));
    assert_eq!(left.power(Fraction::ZERO), Ok(Fraction::ONE));
    if let Ok(power) = power {
        if right.is_integer() && !right.is_negative() && right.numerator() <= 64 {
            let mut expected = Ok(Fraction::ONE);
            for _ in 0..right.numerator() {
                expected = expected.and_then(|value: Fraction| value.multiply(left));
            }
            assert_eq!(Ok(power), expected, "{left:?} ^ {right:?}");
        }
    }
}

fn exact<const D: u32, const E: i32>(left: Exact<D, E>, right: Exact<D, E>) {
    let results = [
        left.add(right),
        left.subtract(right),
        left.multiply(right),
        left.divide(right),
        left.remainder(right),
        left.power(right),
        left.negate(),
    ];
    for value in results.into_iter().flatten() {
        let _ = format!("{value} {value:#} {value:.5} {value:?}");
        let _ = value.to_decimal();
        if let Some(fraction) = value.as_fraction() {
            valid(fraction);
        }
    }
    assert_eq!(results[0], right.add(left));
    assert_eq!(results[2], right.multiply(left));
    assert_eq!(
        left.compare(right).map(Ordering::reverse),
        right.compare(left)
    );
}

fuzz_target!(|input: Input| {
    let _ = input.text.parse::<Fraction>();
    let _ = <Fraction as Number>::parse(&input.text);
    let _ = <Exact as Number>::parse(&input.text);
    let _ = <Exact<3, 19> as Number>::parse(&input.text);
    let (Some(left), Some(right)) = (operand(&input.left), operand(&input.right)) else {
        return;
    };
    valid(left);
    valid(right);
    arithmetic(left, right);
    exact(Exact::<14, 99>::from(left), Exact::<14, 99>::from(right));
    exact(Exact::<3, 19>::from(left), Exact::<3, 19>::from(right));
    if let Ok(decimal) = Decimal::<14, 99>::try_from(right) {
        exact(Exact::from(left), Exact::from(decimal));
        exact(Exact::from(decimal), Exact::from(left));
    }
    if let Ok(decimal) = Decimal::<3, 19>::try_from(left) {
        exact(Exact::from(decimal), Exact::<3, 19>::from(right));
    }
});
