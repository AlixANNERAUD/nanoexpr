//! A decimal floating-point number, the representation pocket calculators use.

use core::cmp::Ordering;
use core::fmt::{self, Write};
use core::str::FromStr;

use crate::{ErrorKind, Number};

mod fraction;
mod functions;
mod wide;

/// A decimal floating-point number, as pocket calculators compute with.
///
/// The value is a coefficient of `DIGITS` significant decimal digits times a power of ten, with a scientific exponent
/// between `-MAX_EXPONENT` and `MAX_EXPONENT`. The defaults, 14 digits and an exponent of ±99, are those of TI-83 and
/// TI-84 calculators.
///
/// Decimal fractions are exact, so `0.1 + 0.2` is `0.3`. Each operation rounds its exact result once to `DIGITS`
/// digits, half to even. Results beyond the exponent range are [`ErrorKind::Overflow`] errors, and results too
/// small for it become zero.
///
/// Showing fewer digits than are kept, as calculators do, hides the rounding of intermediate results: format with a
/// precision to round to that many significant digits.
///
/// ```
/// use nanoexpr::Decimal;
///
/// let third: Decimal = nanoexpr::evaluate("1 / 3").unwrap();
/// assert_eq!(third.to_string(), "0.33333333333333");
///
/// let one: Decimal = nanoexpr::evaluate("1 / 3 * 3").unwrap();
/// assert_eq!(one.to_string(), "0.99999999999999");
/// assert_eq!(format!("{one:.10}"), "1");
///
/// let sum: Decimal = nanoexpr::evaluate("0.1 + 0.2").unwrap();
/// assert_eq!(sum.to_string(), "0.3");
/// ```
///
/// `DIGITS` must be between 1 and 18, and `MAX_EXPONENT` between 1 and 9999.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Decimal<const DIGITS: u32 = 14, const MAX_EXPONENT: i32 = 99> {
    /// Whether the number is below zero, never set for zero.
    negative: bool,
    /// Exactly `DIGITS` digits, or zero.
    coefficient: u64,
    /// Power of ten the coefficient is multiplied by, zero for zero.
    exponent: i32,
}

/// Returns `10^exponent`, for exponents up to 38.
const fn power_of_ten(exponent: u32) -> u128 {
    10_u128.pow(exponent)
}

/// Returns how many decimal digits `value` has, zero for zero.
const fn digit_count(value: u128) -> u32 {
    match value.checked_ilog10() {
        Some(logarithm) => logarithm + 1,
        None => 0,
    }
}

/// How [`Decimal::integer`] rounds to an integer.
#[derive(Clone, Copy)]
enum Rounding {
    /// Toward zero.
    Toward,
    /// Toward negative infinity.
    Down,
    /// Toward positive infinity.
    Up,
    /// To the nearest, half away from zero.
    Nearest,
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> Decimal<DIGITS, MAX_EXPONENT> {
    /// Fails to compile when the parameters are out of range.
    const VALID: () = assert!(
        DIGITS >= 1 && DIGITS <= 18 && MAX_EXPONENT >= 1 && MAX_EXPONENT <= 9999,
        "Decimal needs 1 to 18 digits and a maximum exponent of 1 to 9999"
    );

    /// Zero.
    pub const ZERO: Self = Self {
        negative: false,
        coefficient: 0,
        exponent: 0,
    };

    /// Creates `mantissa × 10^exponent`, rounded to `DIGITS` digits.
    ///
    /// ```
    /// use nanoexpr::Decimal;
    ///
    /// let price = Decimal::<14, 99>::new(1999, -2).unwrap();
    /// assert_eq!(price.to_string(), "19.99");
    /// ```
    pub fn new(mantissa: i64, exponent: i32) -> Result<Self, ErrorKind> {
        Self::from_parts(
            mantissa < 0,
            u128::from(mantissa.unsigned_abs()),
            i64::from(exponent),
            false,
        )
    }

    /// Rounds `coefficient × 10^exponent` to `DIGITS` digits, half to even.
    ///
    /// `sticky` tells that the exact value is slightly above `coefficient × 10^exponent`, for operations that had to
    /// drop nonzero digits below the ones they kept. Callers keep at least two digits more than `DIGITS` when they
    /// set it, so that it only breaks ties.
    fn from_parts(
        negative: bool,
        mut coefficient: u128,
        mut exponent: i64,
        sticky: bool,
    ) -> Result<Self, ErrorKind> {
        let () = Self::VALID;
        if coefficient == 0 {
            return Ok(Self::ZERO);
        }
        let digits = digit_count(coefficient);
        if digits > DIGITS {
            let dropped = digits - DIGITS;
            let divisor = power_of_ten(dropped);
            let remainder = coefficient % divisor;
            let half = divisor / 2;
            coefficient /= divisor;
            exponent += i64::from(dropped);
            if remainder > half || (remainder == half && (sticky || coefficient % 2 == 1)) {
                coefficient += 1;
                if coefficient == power_of_ten(DIGITS) {
                    coefficient /= 10;
                    exponent += 1;
                }
            }
        } else {
            let missing = DIGITS - digits;
            coefficient *= power_of_ten(missing);
            exponent -= i64::from(missing);
        }
        let scientific = exponent + i64::from(DIGITS) - 1;
        if scientific > i64::from(MAX_EXPONENT) {
            return Err(ErrorKind::Overflow);
        }
        if scientific < -i64::from(MAX_EXPONENT) {
            return Ok(Self::ZERO);
        }
        Ok(Self {
            negative,
            coefficient: coefficient as u64,
            exponent: exponent as i32,
        })
    }

    /// One.
    fn one() -> Self {
        Self {
            negative: false,
            coefficient: power_of_ten(DIGITS - 1) as u64,
            exponent: 1 - DIGITS as i32,
        }
    }

    /// Returns whether the number is zero.
    pub const fn is_zero(self) -> bool {
        self.coefficient == 0
    }

    /// Returns whether the number is below zero.
    pub const fn is_negative(self) -> bool {
        self.negative
    }

    /// Returns the absolute value.
    pub const fn abs(self) -> Self {
        Self {
            negative: false,
            ..self
        }
    }

    /// Returns the exponent of the number in scientific notation, such as 2 for `123`, or zero for zero.
    pub const fn scientific_exponent(self) -> i32 {
        if self.is_zero() {
            0
        } else {
            self.exponent + DIGITS as i32 - 1
        }
    }

    /// Returns the closest `f64`.
    pub fn to_f64(self) -> f64 {
        let mut buffer = Buffer::<48>::new();
        let sign = if self.negative { "-" } else { "" };
        let _ = write!(buffer, "{sign}{}e{}", self.coefficient, self.exponent);
        buffer.as_str().parse().unwrap_or(f64::NAN)
    }

    /// Returns `value` rounded to `DIGITS` digits.
    ///
    /// `NaN` is a [`ErrorKind::Domain`] error and infinities and values beyond the exponent range are
    /// [`ErrorKind::Overflow`] errors.
    pub fn from_f64(value: f64) -> Result<Self, ErrorKind> {
        if value.is_nan() {
            return Err(ErrorKind::Domain);
        }
        if value.is_infinite() {
            return Err(ErrorKind::Overflow);
        }
        let mut buffer = Buffer::<64>::new();
        let _ = write!(buffer, "{:.36e}", value.abs());
        let magnitude = Self::parse_magnitude(buffer.as_str()).ok_or(ErrorKind::Overflow)??;
        Ok(if value < 0.0 {
            magnitude.flip()
        } else {
            magnitude
        })
    }

    /// Returns the number with the opposite sign, keeping zero positive.
    const fn flip(self) -> Self {
        Self {
            negative: !self.negative && !self.is_zero(),
            ..self
        }
    }

    /// Parses an unsigned literal such as `12`, `.5`, `3.` or `1.5e-3`.
    ///
    /// Returns `None` for malformed text, or the rounded value or its overflow.
    fn parse_magnitude(text: &str) -> Option<Result<Self, ErrorKind>> {
        let bytes = text.as_bytes();
        let mut coefficient: u128 = 0;
        let mut digits = 0;
        let mut exponent: i64 = 0;
        let mut sticky = false;
        let mut fraction = false;
        let mut seen = false;
        let mut index = 0;
        while let Some(&byte) = bytes.get(index) {
            match byte {
                b'0'..=b'9' => {
                    seen = true;
                    let digit = u128::from(byte - b'0');
                    if digits < 38 {
                        coefficient = coefficient * 10 + digit;
                        if coefficient != 0 {
                            digits += 1;
                        }
                        if fraction {
                            exponent -= 1;
                        }
                    } else {
                        if !fraction {
                            exponent += 1;
                        }
                        sticky |= digit != 0;
                    }
                }
                b'.' if !fraction => fraction = true,
                b'e' | b'E' if seen => {
                    exponent += Self::parse_exponent(bytes.get(index + 1..)?)?;
                    break;
                }
                _ => return None,
            }
            index += 1;
        }
        seen.then(|| Self::from_parts(false, coefficient, exponent, sticky))
    }

    /// Parses the digits after an `e`, with an optional sign, saturating far beyond any exponent range.
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

    /// Returns the sum of the two numbers.
    fn sum(self, rhs: Self) -> Result<Self, ErrorKind> {
        if self.is_zero() {
            return Ok(rhs);
        }
        if rhs.is_zero() {
            return Ok(self);
        }
        let (large, small) = if self.exponent >= rhs.exponent {
            (self, rhs)
        } else {
            (rhs, self)
        };
        let difference = (large.exponent - small.exponent) as u32;
        let subtract = large.negative != small.negative;
        if difference > DIGITS + 2 {
            let shifted = u128::from(large.coefficient) * 1000;
            let exponent = i64::from(large.exponent) - 3;
            let coefficient = if subtract { shifted - 1 } else { shifted };
            return Self::from_parts(large.negative, coefficient, exponent, true);
        }
        let aligned = u128::from(large.coefficient) * power_of_ten(difference);
        let other = u128::from(small.coefficient);
        let exponent = i64::from(small.exponent);
        if !subtract {
            return Self::from_parts(large.negative, aligned + other, exponent, false);
        }
        match aligned.cmp(&other) {
            Ordering::Greater => Self::from_parts(large.negative, aligned - other, exponent, false),
            Ordering::Less => Self::from_parts(small.negative, other - aligned, exponent, false),
            Ordering::Equal => Ok(Self::ZERO),
        }
    }

    /// Returns whether the number is an integer.
    fn is_integer(self) -> bool {
        self.exponent >= 0
            || (self.exponent.unsigned_abs() <= 19
                && u128::from(self.coefficient) % power_of_ten(self.exponent.unsigned_abs()) == 0)
    }

    /// Returns the number as an `i64`, if it is an integer that fits.
    fn to_integer(self) -> Option<i64> {
        if !self.is_integer() {
            return None;
        }
        let magnitude = if self.exponent >= 0 {
            u128::from(self.coefficient)
                .checked_mul(10_u128.checked_pow(self.exponent.unsigned_abs())?)?
        } else {
            u128::from(self.coefficient) / power_of_ten(self.exponent.unsigned_abs())
        };
        let magnitude = i64::try_from(magnitude).ok()?;
        Some(if self.negative { -magnitude } else { magnitude })
    }

    /// Rounds the number to an integer.
    fn integer(self, rounding: Rounding) -> Result<Self, ErrorKind> {
        if self.exponent >= 0 {
            return Ok(self);
        }
        let shift = self.exponent.unsigned_abs();
        let (whole, rest) = if shift > DIGITS {
            (0, u128::from(self.coefficient))
        } else {
            let divisor = power_of_ten(shift);
            (
                u128::from(self.coefficient) / divisor,
                u128::from(self.coefficient) % divisor,
            )
        };
        let half = if shift > DIGITS {
            0
        } else {
            power_of_ten(shift) / 2
        };
        let away = match rounding {
            Rounding::Toward => false,
            Rounding::Down => self.negative && rest != 0,
            Rounding::Up => !self.negative && rest != 0,
            Rounding::Nearest => shift <= DIGITS && rest >= half,
        };
        Self::from_parts(self.negative, whole + u128::from(away), 0, false)
    }

    /// Rounds toward negative infinity.
    pub fn floor(self) -> Result<Self, ErrorKind> {
        self.integer(Rounding::Down)
    }

    /// Rounds toward positive infinity.
    pub fn ceil(self) -> Result<Self, ErrorKind> {
        self.integer(Rounding::Up)
    }

    /// Rounds to the nearest integer, half away from zero.
    pub fn round(self) -> Result<Self, ErrorKind> {
        self.integer(Rounding::Nearest)
    }

    /// Rounds toward zero.
    pub fn trunc(self) -> Result<Self, ErrorKind> {
        self.integer(Rounding::Toward)
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> Default for Decimal<DIGITS, MAX_EXPONENT> {
    fn default() -> Self {
        Self::ZERO
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> Number for Decimal<DIGITS, MAX_EXPONENT> {
    fn parse(literal: &str) -> Option<Self> {
        Self::parse_magnitude(literal)?.ok()
    }

    fn add(self, rhs: Self) -> Result<Self, ErrorKind> {
        self.sum(rhs)
    }

    fn subtract(self, rhs: Self) -> Result<Self, ErrorKind> {
        self.sum(rhs.flip())
    }

    fn multiply(self, rhs: Self) -> Result<Self, ErrorKind> {
        Self::from_parts(
            self.negative != rhs.negative,
            u128::from(self.coefficient) * u128::from(rhs.coefficient),
            i64::from(self.exponent) + i64::from(rhs.exponent),
            false,
        )
    }

    fn divide(self, rhs: Self) -> Result<Self, ErrorKind> {
        if rhs.is_zero() {
            return Err(ErrorKind::DivisionByZero);
        }
        let numerator = u128::from(self.coefficient) * power_of_ten(DIGITS + 2);
        let denominator = u128::from(rhs.coefficient);
        Self::from_parts(
            self.negative != rhs.negative,
            numerator / denominator,
            i64::from(self.exponent) - i64::from(rhs.exponent) - i64::from(DIGITS + 2),
            numerator % denominator != 0,
        )
    }

    fn remainder(self, rhs: Self) -> Result<Self, ErrorKind> {
        if rhs.is_zero() {
            return Err(ErrorKind::DivisionByZero);
        }
        if self.exponent < rhs.exponent {
            return Ok(self);
        }
        let divisor = u128::from(rhs.coefficient);
        let mut remainder = u128::from(self.coefficient) % divisor;
        for _ in rhs.exponent..self.exponent {
            remainder = remainder * 10 % divisor;
        }
        Self::from_parts(self.negative, remainder, i64::from(rhs.exponent), false)
    }

    fn power(self, rhs: Self) -> Result<Self, ErrorKind> {
        self.pow(rhs)
    }

    fn negate(self) -> Result<Self, ErrorKind> {
        Ok(self.flip())
    }

    fn compare(self, rhs: Self) -> Result<Ordering, ErrorKind> {
        Ok(self.cmp(&rhs))
    }

    fn is_zero(self) -> Result<bool, ErrorKind> {
        Ok(Self::is_zero(self))
    }

    fn from_bool(value: bool) -> Result<Self, ErrorKind> {
        Ok(if value { Self::one() } else { Self::ZERO })
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> PartialOrd for Decimal<DIGITS, MAX_EXPONENT> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> Ord for Decimal<DIGITS, MAX_EXPONENT> {
    fn cmp(&self, other: &Self) -> Ordering {
        let sign = |value: &Self| match (value.is_zero(), value.negative) {
            (true, _) => 0,
            (false, true) => -1,
            (false, false) => 1,
        };
        sign(self).cmp(&sign(other)).then_with(|| {
            let magnitude = self
                .exponent
                .cmp(&other.exponent)
                .then(self.coefficient.cmp(&other.coefficient));
            if self.negative {
                magnitude.reverse()
            } else {
                magnitude
            }
        })
    }
}

/// The error of parsing a [`Decimal`] from text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParseDecimalError {
    /// [`ErrorKind::InvalidNumber`] for malformed text, or [`ErrorKind::Overflow`] for a value beyond the range.
    pub kind: ErrorKind,
}

impl fmt::Display for ParseDecimalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(formatter)
    }
}

impl core::error::Error for ParseDecimalError {}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> FromStr for Decimal<DIGITS, MAX_EXPONENT> {
    type Err = ParseDecimalError;

    /// Parses a number such as `-12.5` or `6.02e23`, rounded to `DIGITS` digits.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (negative, magnitude) = match text.as_bytes().first() {
            Some(b'-') => (true, &text[1..]),
            Some(b'+') => (false, &text[1..]),
            _ => (false, text),
        };
        let value = Self::parse_magnitude(magnitude)
            .ok_or(ParseDecimalError {
                kind: ErrorKind::InvalidNumber,
            })?
            .map_err(|kind| ParseDecimalError { kind })?;
        Ok(if negative { value.flip() } else { value })
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> TryFrom<i64> for Decimal<DIGITS, MAX_EXPONENT> {
    type Error = ErrorKind;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::new(value, 0)
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> TryFrom<f64> for Decimal<DIGITS, MAX_EXPONENT> {
    type Error = ErrorKind;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::from_f64(value)
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> From<Decimal<DIGITS, MAX_EXPONENT>> for f64 {
    fn from(value: Decimal<DIGITS, MAX_EXPONENT>) -> Self {
        value.to_f64()
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> fmt::Display for Decimal<DIGITS, MAX_EXPONENT> {
    /// Writes the number with all its significant digits, or with as many as the precision asks for, rounded half
    /// away from zero.
    ///
    /// Numbers are written positionally when their scientific exponent is between -5 and the number of digits
    /// shown, and in scientific notation otherwise, such as `6.02e23`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let shown = formatter.precision().map_or(DIGITS, |precision| {
            u32::try_from(precision).unwrap_or(DIGITS).clamp(1, DIGITS)
        });
        let mut coefficient = self.coefficient;
        let mut exponent = self.scientific_exponent();
        if shown < DIGITS && !self.is_zero() {
            let divisor = power_of_ten(DIGITS - shown) as u64;
            let remainder = coefficient % divisor;
            coefficient /= divisor;
            if remainder >= divisor / 2 {
                coefficient += 1;
                if u128::from(coefficient) == power_of_ten(shown) {
                    coefficient /= 10;
                    exponent += 1;
                }
            }
        }
        let mut digits = Buffer::<20>::new();
        let _ = write!(digits, "{coefficient}");
        let digits = digits.as_str().trim_end_matches('0');
        let digits = if digits.is_empty() { "0" } else { digits };
        let mut text = Buffer::<48>::new();
        if (-5..i32::try_from(shown).unwrap_or(i32::MAX)).contains(&exponent) {
            if exponent < 0 {
                text.write_str("0.")?;
                for _ in 1..exponent.unsigned_abs() {
                    text.write_char('0')?;
                }
                text.write_str(digits)?;
            } else {
                let whole = exponent.unsigned_abs() as usize + 1;
                if digits.len() > whole {
                    let (integer, fraction) = digits.split_at(whole);
                    write!(text, "{integer}.{fraction}")?;
                } else {
                    text.write_str(digits)?;
                    for _ in digits.len()..whole {
                        text.write_char('0')?;
                    }
                }
            }
        } else {
            let (first, rest) = digits.split_at(1);
            text.write_str(first)?;
            if !rest.is_empty() {
                write!(text, ".{rest}")?;
            }
            write!(text, "e{exponent}")?;
        }
        formatter.pad_integral(!self.negative, "", text.as_str())
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> fmt::Debug for Decimal<DIGITS, MAX_EXPONENT> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

/// A fixed-size text buffer, to format without allocating.
struct Buffer<const SIZE: usize> {
    /// The written bytes, then unused space.
    bytes: [u8; SIZE],
    /// How many bytes are written.
    length: usize,
}

impl<const SIZE: usize> Buffer<SIZE> {
    /// Creates an empty buffer.
    const fn new() -> Self {
        Self {
            bytes: [0; SIZE],
            length: 0,
        }
    }

    /// Returns the written text.
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.length]).unwrap_or("")
    }
}

impl<const SIZE: usize> Write for Buffer<SIZE> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.length + text.len();
        self.bytes
            .get_mut(self.length..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
