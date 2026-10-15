//! The [`Exact`] number type, which keeps results as fractions while they are exact, like recent calculators.

use core::cmp::Ordering;
use core::fmt;

use crate::{Decimal, ErrorKind, Fraction, Number};

/// A number that stays an exact [`Fraction`] while it can, and becomes a [`Decimal`] otherwise, like the results
/// of a TI-84 Plus CE or a Casio ClassWiz.
///
/// Operations on fractions give fractions when the exact result fits one. When it does not, because it overflows
/// 64 bits or is irrational, such as `sqrt(2)`, `sin(1)` or `2 ^ (1/2)`, the operation is computed again on the
/// operands as decimals, and the result is a `Decimal` of `DIGITS` digits. Constants such as `pi` and `e` are
/// decimals, and once a value is decimal, everything computed from it stays decimal, as on the calculators.
///
/// Number literals are exact when they fit a fraction, so `0.1 + 0.2` is `3/10`, as on a Casio ClassWiz. A TI-84
/// Plus CE would show `0.3`, keeping typed decimals as decimals. Literals that do not fit, such as `1e30`, are
/// decimals.
///
/// Errors are those of the exact computation, or of the decimal one when it is computed: `1/0` is a
/// [`ErrorKind::DivisionByZero`] error and `sqrt(-2)` a [`ErrorKind::Domain`] error, at the same spans as with
/// the other number types.
///
/// It displays fractions as `n/d`, or as mixed numbers such as `1 1/2` with the alternate flag `{:#}`, and decimals
/// as [`Decimal`] does, with as many significant digits as a precision such as `{:.10}` asks for. Use
/// [`Exact::to_decimal`] to show a fraction as a decimal.
///
/// ```
/// use nanoexpr::Exact;
///
/// let sum: Exact = nanoexpr::evaluate("1/3 + 1/6").unwrap();
/// assert!(sum.is_exact());
/// assert_eq!(sum.to_string(), "1/2");
/// assert_eq!(sum.to_decimal().to_string(), "0.5");
///
/// let root: Exact = nanoexpr::evaluate("sqrt(4/9)").unwrap();
/// assert_eq!(root.to_string(), "2/3");
///
/// let irrational: Exact = nanoexpr::evaluate("1/3 + sqrt(2)").unwrap();
/// assert!(!irrational.is_exact());
/// assert_eq!(irrational.to_string(), "1.7475468957064");
/// assert_eq!(format!("{irrational:.10}"), "1.747546896");
/// ```
///
/// Two values are equal when they have the same form and the same value, so the fraction `1/2` differs from the
/// decimal `0.5`, and these two are not ordered either. Other values order by value, exactly. Comparisons in
/// expressions, such as `1/2 == 0.5`, compare values whatever their forms.
///
/// `MAX_EXPONENT` must be at least 19, so that every fraction is also a decimal.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Exact<const DIGITS: u32 = 14, const MAX_EXPONENT: i32 = 99>(Form<DIGITS, MAX_EXPONENT>);

/// The form of an [`Exact`] number.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Form<const DIGITS: u32, const MAX_EXPONENT: i32> {
    /// An exact fraction.
    Fraction(Fraction),
    /// A decimal, rounded to `DIGITS` digits.
    Decimal(Decimal<DIGITS, MAX_EXPONENT>),
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> Exact<DIGITS, MAX_EXPONENT> {
    /// Fails to compile when the parameters are out of range.
    const VALID: () = assert!(
        MAX_EXPONENT >= 19,
        "Exact needs a maximum exponent of at least 19, to hold every fraction as a decimal"
    );

    /// Returns whether the number is an exact fraction.
    pub const fn is_exact(self) -> bool {
        matches!(self.0, Form::Fraction(_))
    }

    /// Returns the fraction, or `None` if the number is a decimal.
    pub const fn as_fraction(self) -> Option<Fraction> {
        match self.0 {
            Form::Fraction(fraction) => Some(fraction),
            Form::Decimal(_) => None,
        }
    }

    /// Returns the decimal, or `None` if the number is a fraction.
    pub const fn as_decimal(self) -> Option<Decimal<DIGITS, MAX_EXPONENT>> {
        match self.0 {
            Form::Fraction(_) => None,
            Form::Decimal(decimal) => Some(decimal),
        }
    }

    /// Returns the number as a decimal, rounding a fraction to `DIGITS` digits, half to even.
    pub fn to_decimal(self) -> Decimal<DIGITS, MAX_EXPONENT> {
        let () = Self::VALID;
        match self.0 {
            Form::Fraction(fraction) => Decimal::try_from(fraction).unwrap_or(Decimal::ZERO),
            Form::Decimal(decimal) => decimal,
        }
    }

    /// Applies an operation exactly when both operands are fractions, and in decimal otherwise or when the exact
    /// result is [`ErrorKind::Overflow`] or [`ErrorKind::Inexact`].
    fn apply(
        self,
        rhs: Self,
        exact: fn(Fraction, Fraction) -> Result<Fraction, ErrorKind>,
        decimal: fn(
            Decimal<DIGITS, MAX_EXPONENT>,
            Decimal<DIGITS, MAX_EXPONENT>,
        ) -> Result<Decimal<DIGITS, MAX_EXPONENT>, ErrorKind>,
    ) -> Result<Self, ErrorKind> {
        if let (Form::Fraction(left), Form::Fraction(right)) = (self.0, rhs.0) {
            match exact(left, right) {
                Err(ErrorKind::Overflow | ErrorKind::Inexact) => {}
                result => return result.map(Self::from),
            }
        }
        decimal(self.to_decimal(), rhs.to_decimal()).map(Self::from)
    }

    /// Returns `base^exponent` in decimal, as `numerator^exponent / denominator^exponent` times the power of the
    /// sign.
    ///
    /// Numerators and denominators of up to `DIGITS` digits are exact as decimals, so the rounding of a fraction
    /// such as `1/3` is not raised to the power with it. Fails when a part overflows, or when the sign has no power.
    fn power_parts(
        base: Fraction,
        exponent: Decimal<DIGITS, MAX_EXPONENT>,
    ) -> Result<Decimal<DIGITS, MAX_EXPONENT>, ErrorKind> {
        let part = |value: u64| {
            Decimal::<DIGITS, MAX_EXPONENT>::try_from(Fraction::from(value))
                .and_then(|part| part.power(exponent))
        };
        let magnitude = part(base.numerator())?.divide(part(base.denominator())?)?;
        if base.is_negative() {
            let sign = Decimal::<DIGITS, MAX_EXPONENT>::try_from(Fraction::from(-1_i64))?;
            magnitude.multiply(sign.power(exponent)?)
        } else {
            Ok(magnitude)
        }
    }

    /// Returns how the number orders against `rhs`, comparing values exactly whatever their forms.
    pub(crate) fn cmp_value(self, rhs: Self) -> Ordering {
        match (self.0, rhs.0) {
            (Form::Fraction(left), Form::Fraction(right)) => left.cmp(&right),
            (Form::Decimal(left), Form::Decimal(right)) => left.cmp(&right),
            (Form::Decimal(left), Form::Fraction(right)) => left.cmp_fraction(right),
            (Form::Fraction(left), Form::Decimal(right)) => right.cmp_fraction(left).reverse(),
        }
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> PartialOrd for Exact<DIGITS, MAX_EXPONENT> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let ordering = self.cmp_value(*other);
        (ordering != Ordering::Equal || self == other).then_some(ordering)
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> From<Fraction> for Exact<DIGITS, MAX_EXPONENT> {
    fn from(value: Fraction) -> Self {
        Self(Form::Fraction(value))
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> From<Decimal<DIGITS, MAX_EXPONENT>>
    for Exact<DIGITS, MAX_EXPONENT>
{
    fn from(value: Decimal<DIGITS, MAX_EXPONENT>) -> Self {
        Self(Form::Decimal(value))
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> Default for Exact<DIGITS, MAX_EXPONENT> {
    fn default() -> Self {
        Self::from(Fraction::ZERO)
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> Number for Exact<DIGITS, MAX_EXPONENT> {
    fn parse(literal: &str) -> Option<Self> {
        let () = Self::VALID;
        Fraction::parse(literal)
            .map(Self::from)
            .or_else(|| Decimal::parse(literal).map(Self::from))
    }

    fn add(self, rhs: Self) -> Result<Self, ErrorKind> {
        self.apply(rhs, Number::add, Number::add)
    }

    fn subtract(self, rhs: Self) -> Result<Self, ErrorKind> {
        self.apply(rhs, Number::subtract, Number::subtract)
    }

    fn multiply(self, rhs: Self) -> Result<Self, ErrorKind> {
        self.apply(rhs, Number::multiply, Number::multiply)
    }

    fn divide(self, rhs: Self) -> Result<Self, ErrorKind> {
        self.apply(rhs, Number::divide, Number::divide)
    }

    fn remainder(self, rhs: Self) -> Result<Self, ErrorKind> {
        self.apply(rhs, Number::remainder, Number::remainder)
    }

    /// Raises to the power exactly when possible, and in decimal otherwise, through the numerator and the
    /// denominator of a fraction base.
    fn power(self, rhs: Self) -> Result<Self, ErrorKind> {
        if let (Form::Fraction(base), Form::Fraction(exponent)) = (self.0, rhs.0) {
            match base.power(exponent) {
                Err(ErrorKind::Overflow | ErrorKind::Inexact) => {}
                result => return result.map(Self::from),
            }
        }
        let exponent = rhs.to_decimal();
        if let Form::Fraction(base) = self.0 {
            if let Ok(value) = Self::power_parts(base, exponent) {
                return Ok(Self::from(value));
            }
        }
        self.to_decimal().power(exponent).map(Self::from)
    }

    fn negate(self) -> Result<Self, ErrorKind> {
        Ok(match self.0 {
            Form::Fraction(fraction) => Self::from(fraction.negate()?),
            Form::Decimal(decimal) => Self::from(decimal.negate()?),
        })
    }

    fn compare(self, rhs: Self) -> Result<Ordering, ErrorKind> {
        Ok(self.cmp_value(rhs))
    }

    fn is_zero(self) -> Result<bool, ErrorKind> {
        Ok(match self.0 {
            Form::Fraction(fraction) => fraction.is_zero(),
            Form::Decimal(decimal) => decimal.is_zero(),
        })
    }

    fn from_bool(value: bool) -> Result<Self, ErrorKind> {
        Fraction::from_bool(value).map(Self::from)
    }

    /// Computes the factorial exactly while it fits a fraction, up to `20!`, and in decimal beyond.
    fn factorial(self) -> Result<Self, ErrorKind> {
        if let Form::Fraction(fraction) = self.0 {
            match fraction.factorial() {
                Err(ErrorKind::Overflow) => {}
                result => return result.map(Self::from),
            }
        }
        self.to_decimal().factorial().map(Self::from)
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> fmt::Display for Exact<DIGITS, MAX_EXPONENT> {
    /// Writes a fraction as [`Fraction`] does, honoring the alternate flag, and a decimal as [`Decimal`] does,
    /// honoring the precision.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Form::Fraction(fraction) => fmt::Display::fmt(&fraction, formatter),
            Form::Decimal(decimal) => fmt::Display::fmt(&decimal, formatter),
        }
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> fmt::Debug for Exact<DIGITS, MAX_EXPONENT> {
    /// Writes the form and the value, such as `Fraction(1/2)` or `Decimal(0.5)`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Form::Fraction(fraction) => write!(formatter, "Fraction({fraction})"),
            Form::Decimal(decimal) => write!(formatter, "Decimal({decimal})"),
        }
    }
}
