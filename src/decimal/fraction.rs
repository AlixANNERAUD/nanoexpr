//! Fractions that decimal numbers are close to, like the `►Frac` key of TI calculators.

use core::fmt;

use super::{Decimal, power_of_ten};

/// A fraction in lowest terms, such as `-7/2`, as found by [`Decimal::to_fraction`].
///
/// It displays as `7/2`, as `3` when its denominator is 1, or as a mixed number such as `3 1/2` with the alternate
/// flag `{:#}`.
///
/// ```
/// use nanoexpr::Fraction;
///
/// let fraction = Fraction { negative: true, numerator: 7, denominator: 2 };
/// assert_eq!(fraction.to_string(), "-7/2");
/// assert_eq!(format!("{fraction:#}"), "-3 1/2");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fraction {
    /// Whether the fraction is below zero, never set for zero.
    pub negative: bool,
    /// The numerator, without its sign.
    pub numerator: u64,
    /// The denominator, at least 1.
    pub denominator: u64,
}

impl fmt::Display for Fraction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            formatter.write_str("-")?;
        }
        let denominator = self.denominator.max(1);
        let whole = self.numerator / denominator;
        let rest = self.numerator % denominator;
        if rest == 0 {
            write!(formatter, "{whole}")
        } else if formatter.alternate() && whole > 0 {
            write!(formatter, "{whole} {rest}/{denominator}")
        } else {
            write!(formatter, "{}/{denominator}", self.numerator)
        }
    }
}

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
            return (max_denominator >= 1).then_some(Fraction {
                negative,
                numerator,
                denominator: 1,
            });
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
                return Some(Fraction {
                    negative: negative && numerator != 0,
                    numerator: u64::try_from(numerator).ok()?,
                    denominator: u64::try_from(denominator).ok()?,
                });
            }
        }
        None
    }
}

/// Returns `10^exponent`, or `None` beyond `u128`.
fn power_of_ten_checked(exponent: u32) -> Option<u128> {
    (exponent <= 38).then(|| power_of_ten(exponent))
}
