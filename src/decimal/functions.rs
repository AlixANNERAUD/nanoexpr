//! The elementary functions of [`Decimal`], computed with 36 digits in [`Wide`] and rounded once.

use super::Decimal;
use super::wide::{self, Wide};
use crate::ErrorKind;

/// Returns `Ok(())` when `valid`, and a [`ErrorKind::Domain`] error otherwise.
fn domain(valid: bool) -> Result<(), ErrorKind> {
    if valid {
        Ok(())
    } else {
        Err(ErrorKind::Domain)
    }
}

impl<const DIGITS: u32, const MAX_EXPONENT: i32> Decimal<DIGITS, MAX_EXPONENT> {
    /// Returns the number as a [`Wide`], exactly.
    fn wide(self) -> Wide {
        Wide::new(
            self.negative,
            u128::from(self.coefficient),
            i64::from(self.exponent),
        )
    }

    /// Rounds a [`Wide`] to `DIGITS` digits.
    ///
    /// The 36 digits are first rounded to 32, so that results a few units of the 36th digit away from an exact
    /// value, such as `2.99…99` for `log2(8)`, round as that exact value does.
    fn from_wide(value: Wide) -> Result<Self, ErrorKind> {
        let (negative, coefficient, exponent) = value.parts();
        Self::from_parts(negative, (coefficient + 5000) / 10_000, exponent + 4, false)
    }

    /// Returns whether the number is an odd integer.
    fn is_odd(self) -> bool {
        let integer = match self.exponent {
            0 => u128::from(self.coefficient),
            exponent if exponent > 0 => 0,
            exponent => 10_u128
                .checked_pow(exponent.unsigned_abs())
                .map_or(0, |divisor| u128::from(self.coefficient) / divisor),
        };
        self.is_integer() && integer % 2 == 1
    }

    /// Returns `self` raised to the power `exponent`.
    ///
    /// Integer exponents up to 1024 multiply by squaring, exactly while the result fits 36 digits, and others
    /// compute `e^(exponent × ln |self|)`.
    pub(super) fn pow(self, exponent: Self) -> Result<Self, ErrorKind> {
        if self.is_zero() {
            return if exponent.negative {
                Err(ErrorKind::DivisionByZero)
            } else if exponent.is_zero() {
                Ok(Self::one())
            } else {
                Ok(Self::ZERO)
            };
        }
        if let Some(integer) = exponent
            .to_integer()
            .filter(|integer| integer.unsigned_abs() <= 1024)
        {
            let mut result = Wide::ONE;
            let mut base = self.wide();
            let mut remaining = integer.unsigned_abs();
            while remaining != 0 {
                if remaining & 1 == 1 {
                    result = result.mul(base);
                }
                base = base.mul(base);
                remaining >>= 1;
            }
            return Self::from_wide(if integer < 0 {
                Wide::ONE.div(result)
            } else {
                result
            });
        }
        domain(!self.negative || exponent.is_integer())?;
        let magnitude = wide::exp(exponent.wide().mul(wide::ln(self.wide().abs())));
        Self::from_wide(magnitude.signed(self.negative && exponent.is_odd()))
    }

    /// Returns the square root.
    pub(crate) fn sqrt(self) -> Result<Self, ErrorKind> {
        domain(!self.negative)?;
        Self::from_wide(self.wide().sqrt())
    }

    /// Returns the cube root.
    pub(crate) fn cbrt(self) -> Result<Self, ErrorKind> {
        if self.is_zero() {
            return Ok(self);
        }
        let root = wide::exp(wide::ln(self.wide().abs()).div(Wide::integer(3)));
        Self::from_wide(root.signed(self.negative))
    }

    /// Returns `e^self`.
    pub(crate) fn exp(self) -> Result<Self, ErrorKind> {
        Self::from_wide(wide::exp(self.wide()))
    }

    /// Returns the natural logarithm as a [`Wide`], or a domain error unless the number is positive.
    fn wide_ln(self) -> Result<Wide, ErrorKind> {
        domain(!self.negative && !self.is_zero())?;
        Ok(wide::ln(self.wide()))
    }

    /// Returns the natural logarithm.
    pub(crate) fn ln(self) -> Result<Self, ErrorKind> {
        Self::from_wide(self.wide_ln()?)
    }

    /// Returns the logarithm in base 2.
    pub(crate) fn log2(self) -> Result<Self, ErrorKind> {
        Self::from_wide(self.wide_ln()?.div(Wide::LN_2))
    }

    /// Returns the logarithm in base 10, exact for powers of 10.
    pub(crate) fn log10(self) -> Result<Self, ErrorKind> {
        domain(!self.negative && !self.is_zero())?;
        let (power, rest) = wide::ln_parts(self.wide());
        Self::from_wide(Wide::integer(power).add(rest.div(Wide::LN_10)))
    }

    /// Returns the logarithm in base `base`.
    pub(crate) fn log(self, base: Self) -> Result<Self, ErrorKind> {
        domain(base != Self::one())?;
        Self::from_wide(self.wide_ln()?.div(base.wide_ln()?))
    }

    /// Returns the sine and cosine as [`Wide`] numbers.
    ///
    /// Arguments of `10^343` and beyond, past the digits of 2/π kept for the argument reduction, are domain errors.
    fn sine_cosine(self) -> Result<(Wide, Wide), ErrorKind> {
        let (sine, cosine) =
            wide::sine_cosine(self.coefficient, self.exponent).ok_or(ErrorKind::Domain)?;
        Ok((sine.signed(self.negative), cosine))
    }

    /// Returns the sine.
    pub(crate) fn sin(self) -> Result<Self, ErrorKind> {
        Self::from_wide(self.sine_cosine()?.0)
    }

    /// Returns the cosine.
    pub(crate) fn cos(self) -> Result<Self, ErrorKind> {
        Self::from_wide(self.sine_cosine()?.1)
    }

    /// Returns the tangent.
    pub(crate) fn tan(self) -> Result<Self, ErrorKind> {
        let (sine, cosine) = self.sine_cosine()?;
        Self::from_wide(sine.div(cosine))
    }

    /// Returns `√((1 - x)(1 + x))`, the cosine of the inverse sine, or a domain error outside [-1, 1].
    fn complement(self) -> Result<Wide, ErrorKind> {
        domain(self.abs() <= Self::one())?;
        let x = self.wide();
        Ok(Wide::ONE.sub(x).mul(Wide::ONE.add(x)).sqrt())
    }

    /// Returns the inverse sine.
    pub(crate) fn asin(self) -> Result<Self, ErrorKind> {
        Self::from_wide(wide::atan2(self.wide(), self.complement()?))
    }

    /// Returns the inverse cosine.
    pub(crate) fn acos(self) -> Result<Self, ErrorKind> {
        Self::from_wide(wide::atan2(self.complement()?, self.wide()))
    }

    /// Returns the inverse tangent.
    pub(crate) fn atan(self) -> Result<Self, ErrorKind> {
        Self::from_wide(wide::atan(self.wide()))
    }

    /// Returns the angle of the point `(x, self)`.
    pub(crate) fn atan2(self, x: Self) -> Result<Self, ErrorKind> {
        Self::from_wide(wide::atan2(self.wide(), x.wide()))
    }

    /// Returns the hyperbolic sine.
    pub(crate) fn sinh(self) -> Result<Self, ErrorKind> {
        Self::from_wide(wide::sinh(self.wide()))
    }

    /// Returns the hyperbolic cosine.
    pub(crate) fn cosh(self) -> Result<Self, ErrorKind> {
        Self::from_wide(wide::cosh(self.wide()))
    }

    /// Returns the hyperbolic tangent.
    pub(crate) fn tanh(self) -> Result<Self, ErrorKind> {
        Self::from_wide(wide::tanh(self.wide()))
    }

    /// Returns the inverse hyperbolic sine, `ln(1 + u)` with `u = x + x²/(1 + √(1 + x²))` for `x = |self|`.
    pub(crate) fn asinh(self) -> Result<Self, ErrorKind> {
        let x = self.wide().abs();
        let square = x.mul(x);
        let u = x.add(square.div(Wide::ONE.add(Wide::ONE.add(square).sqrt())));
        Self::from_wide(wide::ln_1p(u).signed(self.negative))
    }

    /// Returns the inverse hyperbolic cosine, `ln(1 + u + √(u(u + 2)))` with `u = self - 1`.
    pub(crate) fn acosh(self) -> Result<Self, ErrorKind> {
        domain(self >= Self::one())?;
        let u = self.wide().sub(Wide::ONE);
        Self::from_wide(wide::ln_1p(u.add(u.mul(u.add(Wide::TWO)).sqrt())))
    }

    /// Returns the inverse hyperbolic tangent, `ln(1 + 2x/(1 - x)) / 2` for `x = |self|`.
    pub(crate) fn atanh(self) -> Result<Self, ErrorKind> {
        domain(self.abs() < Self::one())?;
        let x = self.wide().abs();
        let logarithm = wide::ln_1p(x.add(x).div(Wide::ONE.sub(x)));
        Self::from_wide(logarithm.mul(Wide::HALF).signed(self.negative))
    }

    /// Returns `√(self² + y²)`.
    pub(crate) fn hypot(self, y: Self) -> Result<Self, ErrorKind> {
        let (x, y) = (self.wide(), y.wide());
        Self::from_wide(x.mul(x).add(y.mul(y)).sqrt())
    }
}
