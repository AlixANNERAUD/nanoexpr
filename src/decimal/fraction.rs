//! Conversions between [`Decimal`] and [`Fraction`], such as the fraction a decimal is close to, like the `►Frac`
//! key of TI calculators.

#[cfg(feature = "exact")]
use core::cmp::Ordering;

use super::{Decimal, digit_count, power_of_ten};
use crate::{ErrorKind, Fraction};

impl<const DIGITS: u32, const MAX_EXPONENT: i32> Decimal<DIGITS, MAX_EXPONENT> {
    /// Returns the simplest fraction with a denominator up to `max_denominator` that the number is, to within one
    /// unit of its last digit, or `None` if there is none, as the `►Frac` key of a calculator.
    ///
    /// TI-83 and TI-84 calculators allow denominators up to 9999. The tolerance makes up for the rounding of
    /// intermediate results, so `1 / 3 * 3`, which is `0.99999999999999`, is `1`. Irrational numbers such as `pi`
    /// have no such fraction, and neither do integers beyond `u64`. For a float, convert it first with
    /// [`Decimal::from_f64`].
    ///
    /// ```
    /// use nanoexpr::Decimal;
    ///
    /// let sum: Decimal = nanoexpr::evaluate("1 / 3 + 1 / 6").unwrap();
    /// assert_eq!(sum.to_fraction(9999).unwrap().to_string(), "1/2");
    ///
    /// let improper: Decimal = nanoexpr::evaluate("-0.25 - 13 / 4").unwrap();
    /// let fraction = improper.to_fraction(9999).unwrap();
    /// assert_eq!(fraction.to_string(), "-7/2");
    /// assert_eq!(format!("{fraction:#}"), "-3 1/2");
    ///
    /// let pi: Decimal = nanoexpr::evaluate("pi").unwrap();
    /// assert_eq!(pi.to_fraction(9999), None);
    /// ```
    pub fn to_fraction(self, max_denominator: u64) -> Option<Fraction> {
        let negative = self.negative;
        let coefficient = u128::from(self.coefficient);
        if self.exponent >= 0 {
            let scale = power_of_ten_checked(self.exponent.unsigned_abs())?;
            let numerator = u64::try_from(coefficient.checked_mul(scale)?).ok()?;
            return Fraction::new(negative, numerator, 1)
                .ok()
                .filter(|_| max_denominator >= 1);
        }
        let scale = power_of_ten_checked(self.exponent.unsigned_abs())?;
        let bound = u128::from(max_denominator);
        let (mut previous, mut current) = ((0_u128, 1_u128), (1_u128, 0_u128));
        let (mut dividend, mut divisor) = (coefficient, scale);
        while divisor != 0 {
            let term = dividend / divisor;
            let denominator = term.checked_mul(current.1)?.checked_add(previous.1)?;
            if denominator > bound {
                return None;
            }
            let numerator = term.checked_mul(current.0)?.checked_add(previous.0)?;
            (previous, current) = (current, (numerator, denominator));
            (dividend, divisor) = (divisor, dividend % divisor);
            let difference = numerator
                .checked_mul(scale)?
                .abs_diff(coefficient.checked_mul(denominator)?);
            if difference <= denominator {
                return Fraction::new(
                    negative,
                    u64::try_from(numerator).ok()?,
                    u64::try_from(denominator).ok()?,
                )
                .ok();
            }
        }
        None
    }
}

/// Returns `10^exponent`, or `None` beyond `u128`.
fn power_of_ten_checked(exponent: u32) -> Option<u128> {
    (exponent <= 38).then(|| power_of_ten(exponent))
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> Decimal<DIGITS, MAX_EXPONENT> {
    /// Returns `numerator / denominator`, below zero if `negative`, correctly rounded to `DIGITS` digits.
    ///
    /// The quotient is computed by long division until it has two digits more than `DIGITS`, and the remainder
    /// left breaks ties, so the result is rounded once, half to even.
    fn from_ratio(negative: bool, numerator: u128, denominator: u128) -> Result<Self, ErrorKind> {
        if denominator == 0 {
            return Err(ErrorKind::DivisionByZero);
        }
        let mut coefficient = numerator / denominator;
        let mut remainder = numerator % denominator;
        let mut exponent: i64 = 0;
        while remainder != 0 && digit_count(coefficient) < DIGITS + 2 {
            let Some(scaled) = remainder.checked_mul(10) else {
                break;
            };
            coefficient = coefficient * 10 + scaled / denominator;
            remainder = scaled % denominator;
            exponent -= 1;
        }
        Self::from_parts(negative, coefficient, exponent, remainder != 0)
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> TryFrom<Fraction>
    for Decimal<DIGITS, MAX_EXPONENT>
{
    type Error = ErrorKind;

    /// Returns the fraction correctly rounded to `DIGITS` digits, half to even.
    ///
    /// Fractions beyond the exponent range, which takes a `MAX_EXPONENT` below 19, are [`ErrorKind::Overflow`]
    /// errors, or zero when they are too small.
    fn try_from(value: Fraction) -> Result<Self, Self::Error> {
        Self::from_ratio(
            value.is_negative(),
            u128::from(value.numerator()),
            u128::from(value.denominator()),
        )
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> TryFrom<Decimal<DIGITS, MAX_EXPONENT>>
    for Fraction
{
    type Error = ErrorKind;

    /// Returns the exact value of the decimal, such as `1/8` for `0.125`, or an [`ErrorKind::Overflow`] error if
    /// its numerator or denominator does not fit.
    ///
    /// To find the simple fraction a rounded result such as `0.33333333333333` stands for, use
    /// [`Decimal::to_fraction`] instead.
    fn try_from(value: Decimal<DIGITS, MAX_EXPONENT>) -> Result<Self, Self::Error> {
        Self::from_decimal(
            value.negative,
            u128::from(value.coefficient),
            i64::from(value.exponent),
        )
    }
}

#[cfg(feature = "exact")]
impl<const DIGITS: u32, const MAX_EXPONENT: i32> Decimal<DIGITS, MAX_EXPONENT> {
    /// Returns how the decimal orders against `fraction`, exactly.
    ///
    /// The magnitudes compare as `coefficient × denominator × 10^exponent` against the numerator, whose product
    /// fits 128 bits, so a power of ten that overflows decides on its own.
    pub(crate) fn cmp_fraction(self, fraction: Fraction) -> Ordering {
        let sign = |negative: bool, zero: bool| match (zero, negative) {
            (true, _) => 0,
            (false, true) => -1,
            (false, false) => 1,
        };
        let sign_order = sign(self.negative, self.is_zero())
            .cmp(&sign(fraction.is_negative(), fraction.is_zero()));
        if sign_order != Ordering::Equal || self.is_zero() {
            return sign_order;
        }
        let scaled = u128::from(self.coefficient) * u128::from(fraction.denominator());
        let numerator = u128::from(fraction.numerator());
        let shift = self.exponent.unsigned_abs();
        let scale = (shift <= 38).then(|| power_of_ten(shift));
        let magnitude = if self.exponent >= 0 {
            scale
                .and_then(|scale| scaled.checked_mul(scale))
                .map_or(Ordering::Greater, |left| left.cmp(&numerator))
        } else {
            scale
                .and_then(|scale| numerator.checked_mul(scale))
                .map_or(Ordering::Less, |right| scaled.cmp(&right))
        };
        if self.negative {
            magnitude.reverse()
        } else {
            magnitude
        }
    }
}
