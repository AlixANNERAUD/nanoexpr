//! The [`Fraction`] number type, which computes with rational numbers exactly.

use core::cmp::Ordering;
use core::fmt;
use core::str::FromStr;

use crate::{ErrorKind, Number};

/// An exact rational number, such as `-7/2`, always in lowest terms.
///
/// The numerator and the denominator are each at most `u64::MAX`, the numerator holding the sign, so the range is
/// symmetric and negation never fails. Operations compute exactly with 128-bit integers, then reduce: a result
/// whose numerator or denominator does not fit is an [`ErrorKind::Overflow`] error, never a rounded value.
///
/// Expressions written with `/` build fractions, so `1/3 + 1/6` is `1/2`. Literals are exact too: `0.75` is `3/4`
/// and `1.5e-3` is `3/2000`. Literals that do not fit, or have more than 38 significant digits, are invalid
/// numbers.
///
/// Raising to an integer power is exact. Raising to a fractional power `p/q` is exact when the `q`-th root is
/// rational, such as `(4/9)^(1/2) = 2/3` or `(-8)^(1/3) = -2`, and an [`ErrorKind::Inexact`] error otherwise, as
/// `2^(1/2)` is. Even roots of negative numbers are [`ErrorKind::Domain`] errors.
///
/// It displays as `7/2`, as `3` when its denominator is 1, or as a mixed number such as `3 1/2` with the alternate
/// flag `{:#}`.
///
/// ```
/// # #[cfg(feature = "fraction")] {
/// use nanoexpr::Fraction;
///
/// let sum: Fraction = nanoexpr::evaluate("1/3 + 1/6").unwrap();
/// assert_eq!(sum.to_string(), "1/2");
///
/// let total: Fraction = nanoexpr::evaluate("0.1 + 0.2 + 3").unwrap();
/// assert_eq!(total.to_string(), "33/10");
/// assert_eq!(format!("{total:#}"), "3 3/10");
///
/// let root: Fraction = nanoexpr::evaluate("sqrt(4/9)").unwrap();
/// assert_eq!(root, "2/3".parse().unwrap());
/// # }
/// ```
///
/// Equality, ordering and hashing compare values, as every value has a single representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fraction {
    /// Whether the fraction is below zero, never set for zero.
    negative: bool,
    /// The numerator, without its sign.
    numerator: u64,
    /// The denominator, at least 1.
    denominator: u64,
}

/// Returns the greatest common divisor of `a` and `b`, or the other one when one of them is zero.
pub(crate) const fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Returns `base^exponent`, or an [`ErrorKind::Overflow`] error if it does not fit a `u64`.
fn power(base: u64, exponent: u64) -> Result<u64, ErrorKind> {
    match base {
        0 => Ok(u64::from(exponent == 0)),
        1 => Ok(1),
        _ => u32::try_from(exponent)
            .ok()
            .and_then(|exponent| base.checked_pow(exponent))
            .ok_or(ErrorKind::Overflow),
    }
}

/// Returns the integer `degree`-th root of `value`, or `None` if `value` is not a perfect power.
fn root(value: u64, degree: u64) -> Option<u64> {
    if value < 2 {
        return Some(value);
    }
    let degree = u32::try_from(degree).ok().filter(|&degree| degree < 64)?;
    let (mut low, mut high) = (1_u64, 1_u64 << 32);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        match middle.checked_pow(degree) {
            Some(result) if result <= value => low = middle,
            _ => high = middle - 1,
        }
    }
    (low.checked_pow(degree) == Some(value)).then_some(low)
}

impl Fraction {
    /// Zero.
    pub const ZERO: Self = Self {
        negative: false,
        numerator: 0,
        denominator: 1,
    };

    /// One.
    pub const ONE: Self = Self {
        negative: false,
        numerator: 1,
        denominator: 1,
    };

    /// Creates `numerator / denominator`, below zero if `negative`, reduced to lowest terms.
    ///
    /// A zero `denominator` is a [`ErrorKind::DivisionByZero`] error.
    ///
    /// ```
    /// use nanoexpr::Fraction;
    ///
    /// let fraction = Fraction::new(true, 14, 4).unwrap();
    /// assert_eq!(fraction.to_string(), "-7/2");
    /// assert_eq!((fraction.numerator(), fraction.denominator()), (7, 2));
    /// ```
    pub fn new(negative: bool, numerator: u64, denominator: u64) -> Result<Self, ErrorKind> {
        Self::reduce(negative, u128::from(numerator), u128::from(denominator))
    }

    /// Reduces `numerator / denominator`, below zero if `negative`, to lowest terms.
    ///
    /// A zero `denominator` is a [`ErrorKind::DivisionByZero`] error, and a reduced numerator or denominator
    /// beyond `u64` an [`ErrorKind::Overflow`] error.
    pub(crate) fn reduce(
        negative: bool,
        numerator: u128,
        denominator: u128,
    ) -> Result<Self, ErrorKind> {
        if denominator == 0 {
            return Err(ErrorKind::DivisionByZero);
        }
        let divisor = gcd(numerator, denominator);
        let numerator = u64::try_from(numerator / divisor).map_err(|_| ErrorKind::Overflow)?;
        let denominator = u64::try_from(denominator / divisor).map_err(|_| ErrorKind::Overflow)?;
        Ok(Self {
            negative: negative && numerator != 0,
            numerator,
            denominator,
        })
    }

    /// Returns `mantissa × 10^exponent`, below zero if `negative`, exactly.
    ///
    /// Values whose reduced numerator or denominator does not fit are [`ErrorKind::Overflow`] errors.
    pub(crate) fn from_decimal(
        negative: bool,
        mantissa: u128,
        exponent: i64,
    ) -> Result<Self, ErrorKind> {
        if mantissa == 0 {
            return Ok(Self::ZERO);
        }
        if exponent >= 0 {
            let scale = u32::try_from(exponent)
                .ok()
                .and_then(|exponent| 10_u128.checked_pow(exponent))
                .and_then(|scale| scale.checked_mul(mantissa))
                .ok_or(ErrorKind::Overflow)?;
            return Self::reduce(negative, scale, 1);
        }
        let (mut numerator, mut twos, mut fives) =
            (mantissa, exponent.unsigned_abs(), exponent.unsigned_abs());
        while twos > 0 && numerator % 2 == 0 {
            numerator /= 2;
            twos -= 1;
        }
        while fives > 0 && numerator % 5 == 0 {
            numerator /= 5;
            fives -= 1;
        }
        let denominator = u32::try_from(twos)
            .ok()
            .and_then(|twos| 2_u128.checked_pow(twos))
            .zip(
                u32::try_from(fives)
                    .ok()
                    .and_then(|fives| 5_u128.checked_pow(fives)),
            )
            .and_then(|(twos, fives)| twos.checked_mul(fives))
            .ok_or(ErrorKind::Overflow)?;
        Self::reduce(negative, numerator, denominator)
    }

    /// Returns whether the fraction is below zero.
    pub const fn is_negative(self) -> bool {
        self.negative
    }

    /// Returns whether the fraction is zero.
    pub const fn is_zero(self) -> bool {
        self.numerator == 0
    }

    /// Returns whether the fraction is an integer, that is whether its denominator is 1.
    pub const fn is_integer(self) -> bool {
        self.denominator == 1
    }

    /// Returns the numerator, without its sign.
    pub const fn numerator(self) -> u64 {
        self.numerator
    }

    /// Returns the denominator, at least 1.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Returns the absolute value.
    pub const fn abs(self) -> Self {
        Self {
            negative: false,
            ..self
        }
    }

    /// Returns `-1`, `0` or `1`, the sign of the fraction.
    pub const fn sign(self) -> Self {
        if self.is_zero() {
            Self::ZERO
        } else {
            Self {
                negative: self.negative,
                ..Self::ONE
            }
        }
    }

    /// Returns `1 / self`, or a [`ErrorKind::DivisionByZero`] error for zero.
    fn reciprocal(self) -> Result<Self, ErrorKind> {
        if self.is_zero() {
            return Err(ErrorKind::DivisionByZero);
        }
        Ok(Self {
            negative: self.negative,
            numerator: self.denominator,
            denominator: self.numerator,
        })
    }

    /// Returns the integer `magnitude`, below zero if `negative`.
    fn integer(negative: bool, magnitude: u64) -> Self {
        Self {
            negative: negative && magnitude != 0,
            numerator: magnitude,
            denominator: 1,
        }
    }

    /// Rounds the magnitude down, or up if `up`, to an integer of the same sign.
    fn round_magnitude(self, up: bool) -> Self {
        let whole = self.numerator / self.denominator;
        let away = up && self.numerator % self.denominator != 0;
        Self::integer(self.negative, whole + u64::from(away))
    }

    /// Rounds toward negative infinity.
    pub fn floor(self) -> Self {
        self.round_magnitude(self.negative)
    }

    /// Rounds toward positive infinity.
    pub fn ceil(self) -> Self {
        self.round_magnitude(!self.negative)
    }

    /// Rounds toward zero.
    pub fn trunc(self) -> Self {
        self.round_magnitude(false)
    }

    /// Rounds to the nearest integer, half away from zero.
    pub fn round(self) -> Self {
        let remainder = self.numerator % self.denominator;
        let up = u128::from(remainder) * 2 >= u128::from(self.denominator);
        self.round_magnitude(up)
    }

    /// Returns `self` raised to the integer power `exponent`, or to its opposite if `reciprocal`.
    fn power_integer(self, reciprocal: bool, exponent: u64) -> Result<Self, ErrorKind> {
        let base = if reciprocal && exponent != 0 {
            self.reciprocal()?
        } else {
            self
        };
        Ok(Self {
            negative: base.negative && exponent % 2 == 1,
            numerator: power(base.numerator, exponent)?,
            denominator: power(base.denominator, exponent)?,
        })
    }

    /// Returns the real `degree`-th root, exactly.
    ///
    /// Even roots of negative numbers are [`ErrorKind::Domain`] errors, and irrational roots
    /// [`ErrorKind::Inexact`] errors.
    pub(crate) fn root(self, degree: u64) -> Result<Self, ErrorKind> {
        if self.negative && degree % 2 == 0 {
            return Err(ErrorKind::Domain);
        }
        Ok(Self {
            negative: self.negative,
            numerator: root(self.numerator, degree).ok_or(ErrorKind::Inexact)?,
            denominator: root(self.denominator, degree).ok_or(ErrorKind::Inexact)?,
        })
    }

    /// Returns the closest `f64`, exactly rounded when the numerator and the denominator are below 2^53, and within
    /// two units in the last place otherwise.
    pub fn to_f64(self) -> f64 {
        let magnitude = self.numerator as f64 / self.denominator as f64;
        if self.negative { -magnitude } else { magnitude }
    }

    /// Returns the numerator and the denominator of each operand, as 128-bit integers.
    fn wide(self, rhs: Self) -> [u128; 4] {
        [
            u128::from(self.numerator),
            u128::from(self.denominator),
            u128::from(rhs.numerator),
            u128::from(rhs.denominator),
        ]
    }

    /// Returns the sum of the two fractions.
    ///
    /// Both numerators are brought to the least common multiple of the denominators, whose terms then fit 127 bits
    /// unless the denominators are coprime, in which case a sum beyond 128 bits cannot reduce to a fitting result.
    fn sum(self, rhs: Self) -> Result<Self, ErrorKind> {
        let [a, b, c, d] = self.wide(rhs);
        let divisor = gcd(b, d);
        let left = a * (d / divisor);
        let right = c * (b / divisor);
        let (negative, numerator) = if self.negative == rhs.negative {
            (
                self.negative,
                left.checked_add(right).ok_or(ErrorKind::Overflow)?,
            )
        } else if left >= right {
            (self.negative, left - right)
        } else {
            (rhs.negative, right - left)
        };
        Self::reduce(negative, numerator, b / divisor * d)
    }

    /// Returns the product of the two fractions, reducing across them first so that nothing overflows.
    fn product(self, rhs: Self) -> Result<Self, ErrorKind> {
        let [a, b, c, d] = self.wide(rhs);
        let (first, second) = (gcd(a, d).max(1), gcd(c, b).max(1));
        Self::reduce(
            self.negative != rhs.negative,
            (a / first) * (c / second),
            (b / second) * (d / first),
        )
    }
}

impl Default for Fraction {
    fn default() -> Self {
        Self::ZERO
    }
}

impl Number for Fraction {
    fn parse(literal: &str) -> Option<Self> {
        Self::parse_magnitude(literal).ok()
    }

    fn add(self, rhs: Self) -> Result<Self, ErrorKind> {
        self.sum(rhs)
    }

    fn subtract(self, rhs: Self) -> Result<Self, ErrorKind> {
        self.sum(Self {
            negative: !rhs.negative && !rhs.is_zero(),
            ..rhs
        })
    }

    fn multiply(self, rhs: Self) -> Result<Self, ErrorKind> {
        self.product(rhs)
    }

    fn divide(self, rhs: Self) -> Result<Self, ErrorKind> {
        self.product(rhs.reciprocal()?)
    }

    fn remainder(self, rhs: Self) -> Result<Self, ErrorKind> {
        if rhs.is_zero() {
            return Err(ErrorKind::DivisionByZero);
        }
        let [a, b, c, d] = self.wide(rhs);
        Self::reduce(self.negative, (a * d) % (c * b), b * d)
    }

    fn power(self, rhs: Self) -> Result<Self, ErrorKind> {
        if rhs.is_integer() {
            return self.power_integer(rhs.negative, rhs.numerator);
        }
        if self.is_zero() && rhs.negative {
            return Err(ErrorKind::DivisionByZero);
        }
        self.root(rhs.denominator)?
            .power_integer(rhs.negative, rhs.numerator)
    }

    fn negate(self) -> Result<Self, ErrorKind> {
        Ok(Self {
            negative: !self.negative && !self.is_zero(),
            ..self
        })
    }

    fn compare(self, rhs: Self) -> Result<Ordering, ErrorKind> {
        Ok(self.cmp(&rhs))
    }

    fn is_zero(self) -> Result<bool, ErrorKind> {
        Ok(Self::is_zero(self))
    }

    fn from_bool(value: bool) -> Result<Self, ErrorKind> {
        Ok(if value { Self::ONE } else { Self::ZERO })
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Fraction {
    fn cmp(&self, other: &Self) -> Ordering {
        let sign = |value: &Self| match (value.is_zero(), value.negative) {
            (true, _) => 0,
            (false, true) => -1,
            (false, false) => 1,
        };
        sign(self).cmp(&sign(other)).then_with(|| {
            let [a, b, c, d] = self.wide(*other);
            let magnitude = (a * d).cmp(&(c * b));
            if self.negative {
                magnitude.reverse()
            } else {
                magnitude
            }
        })
    }
}

impl Fraction {
    /// Parses an unsigned literal such as `12`, `.5`, `3.` or `1.5e-3`, exactly.
    ///
    /// Malformed text is an [`ErrorKind::InvalidNumber`] error, and values that do not fit, or have more than 38
    /// significant digits, are [`ErrorKind::Overflow`] errors.
    fn parse_magnitude(text: &str) -> Result<Self, ErrorKind> {
        let bytes = text.as_bytes();
        let mut mantissa: u128 = 0;
        let mut zeros: u32 = 0;
        let mut exponent: i64 = 0;
        let mut fraction = false;
        let mut seen = false;
        let mut index = 0;
        while let Some(&byte) = bytes.get(index) {
            match byte {
                b'0'..=b'9' => {
                    seen = true;
                    if fraction {
                        exponent -= 1;
                    }
                    let digit = u128::from(byte - b'0');
                    if digit == 0 {
                        zeros = zeros.saturating_add(1);
                    } else if mantissa == 0 {
                        mantissa = digit;
                        zeros = 0;
                    } else {
                        mantissa = 10_u128
                            .checked_pow(zeros + 1)
                            .and_then(|scale| mantissa.checked_mul(scale))
                            .and_then(|scaled| scaled.checked_add(digit))
                            .ok_or(ErrorKind::Overflow)?;
                        zeros = 0;
                    }
                }
                b'.' if !fraction => fraction = true,
                b'e' | b'E' if seen => {
                    exponent += bytes
                        .get(index + 1..)
                        .and_then(parse_exponent)
                        .ok_or(ErrorKind::InvalidNumber)?;
                    break;
                }
                _ => return Err(ErrorKind::InvalidNumber),
            }
            index += 1;
        }
        if !seen {
            return Err(ErrorKind::InvalidNumber);
        }
        if mantissa == 0 {
            return Ok(Self::ZERO);
        }
        Self::from_decimal(false, mantissa, exponent + i64::from(zeros))
    }
}

/// Parses the digits after an `e`, with an optional sign, saturating far beyond any useful exponent.
fn parse_exponent(bytes: &[u8]) -> Option<i64> {
    let (negative, digits) = match bytes.split_first()? {
        (b'-', rest) => (true, rest),
        (b'+', rest) => (false, rest),
        _ => (false, bytes),
    };
    if digits.is_empty() {
        return None;
    }
    let mut value: i64 = 0;
    for &byte in digits {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = (value * 10 + i64::from(byte - b'0')).min(1_000_000);
    }
    Some(if negative { -value } else { value })
}

/// The error of parsing a [`Fraction`] from text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParseFractionError {
    /// [`ErrorKind::InvalidNumber`] for malformed text, [`ErrorKind::Overflow`] for a value that does not fit, or
    /// [`ErrorKind::DivisionByZero`] for a zero denominator.
    pub kind: ErrorKind,
}

impl fmt::Display for ParseFractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(formatter)
    }
}

impl core::error::Error for ParseFractionError {}

impl FromStr for Fraction {
    type Err = ParseFractionError;

    /// Parses a fraction such as `3/4`, `-5/4` or `7`, whose numerator and denominator may also be decimals such as
    /// `1.5/2`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = |kind| ParseFractionError { kind };
        let (negative, magnitude) = match text.as_bytes().first() {
            Some(b'-') => (true, &text[1..]),
            Some(b'+') => (false, &text[1..]),
            _ => (false, text),
        };
        let (numerator, denominator) = match magnitude.split_once('/') {
            Some((numerator, denominator)) => (numerator, Some(denominator)),
            None => (magnitude, None),
        };
        let mut value = Self::parse_magnitude(numerator).map_err(error)?;
        if let Some(denominator) = denominator {
            let denominator = Self::parse_magnitude(denominator).map_err(error)?;
            value = value.divide(denominator).map_err(error)?;
        }
        if negative {
            value = value.negate().map_err(error)?;
        }
        Ok(value)
    }
}

impl From<u64> for Fraction {
    fn from(value: u64) -> Self {
        Self::integer(false, value)
    }
}

impl From<i64> for Fraction {
    fn from(value: i64) -> Self {
        Self::integer(value < 0, value.unsigned_abs())
    }
}

impl From<Fraction> for f64 {
    fn from(value: Fraction) -> Self {
        value.to_f64()
    }
}

impl fmt::Display for Fraction {
    /// Writes `n/d`, `n` for integers, or with the alternate flag `{:#}` a mixed number such as `-3 1/2`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            formatter.write_str("-")?;
        }
        let whole = self.numerator / self.denominator;
        let rest = self.numerator % self.denominator;
        if rest == 0 {
            write!(formatter, "{whole}")
        } else if formatter.alternate() && whole > 0 {
            write!(formatter, "{whole} {rest}/{}", self.denominator)
        } else {
            write!(formatter, "{}/{}", self.numerator, self.denominator)
        }
    }
}
