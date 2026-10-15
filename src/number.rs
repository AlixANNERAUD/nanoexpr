use crate::ErrorKind;

/// A value type expressions are evaluated in.
///
/// Implemented for `f32` and `f64` with the `libm` feature, and for every primitive integer type. Implement it for
/// your own type, such as a fixed-point number, to evaluate expressions in it.
///
/// Each operation returns the [`ErrorKind`] to report instead of producing a silent `NaN`, infinity or wrapped value.
pub trait Number: Copy + Default {
    /// Parses a literal such as `42`, `3.5` or `1e3`, or returns `None` if the type cannot represent it.
    fn parse(literal: &str) -> Option<Self>;
    /// Returns `self + rhs`.
    fn add(self, rhs: Self) -> Result<Self, ErrorKind>;
    /// Returns `self - rhs`.
    fn subtract(self, rhs: Self) -> Result<Self, ErrorKind>;
    /// Returns `self * rhs`.
    fn multiply(self, rhs: Self) -> Result<Self, ErrorKind>;
    /// Returns `self / rhs`.
    fn divide(self, rhs: Self) -> Result<Self, ErrorKind>;
    /// Returns the remainder of `self / rhs`, with the sign of `self`.
    fn remainder(self, rhs: Self) -> Result<Self, ErrorKind>;
    /// Returns `self` raised to the power `rhs`.
    fn power(self, rhs: Self) -> Result<Self, ErrorKind>;
    /// Returns `-self`.
    fn negate(self) -> Result<Self, ErrorKind>;
}

#[cfg(feature = "libm")]
macro_rules! float {
    ($type:ty, $power:path) => {
        impl Number for $type {
            fn parse(literal: &str) -> Option<Self> {
                literal.parse().ok()
            }

            fn add(self, rhs: Self) -> Result<Self, ErrorKind> {
                check(self, rhs, self + rhs)
            }

            fn subtract(self, rhs: Self) -> Result<Self, ErrorKind> {
                check(self, rhs, self - rhs)
            }

            fn multiply(self, rhs: Self) -> Result<Self, ErrorKind> {
                check(self, rhs, self * rhs)
            }

            fn divide(self, rhs: Self) -> Result<Self, ErrorKind> {
                if rhs == 0.0 {
                    return Err(ErrorKind::DivisionByZero);
                }
                check(self, rhs, self / rhs)
            }

            fn remainder(self, rhs: Self) -> Result<Self, ErrorKind> {
                if rhs == 0.0 {
                    return Err(ErrorKind::DivisionByZero);
                }
                check(self, rhs, self % rhs)
            }

            fn power(self, rhs: Self) -> Result<Self, ErrorKind> {
                if self == 0.0 && rhs < 0.0 {
                    return Err(ErrorKind::DivisionByZero);
                }
                check(self, rhs, $power(self, rhs))
            }

            fn negate(self) -> Result<Self, ErrorKind> {
                Ok(-self)
            }
        }

        impl Checked for $type {
            fn is_nan(self) -> bool {
                <$type>::is_nan(self)
            }

            fn is_infinite(self) -> bool {
                <$type>::is_infinite(self)
            }
        }
    };
}

#[cfg(feature = "libm")]
trait Checked: Copy {
    fn is_nan(self) -> bool;
    fn is_infinite(self) -> bool;
}

#[cfg(feature = "libm")]
fn check<T: Checked>(left: T, right: T, result: T) -> Result<T, ErrorKind> {
    let finite = |value: T| !value.is_nan() && !value.is_infinite();
    if finite(left) && finite(right) {
        if result.is_nan() {
            return Err(ErrorKind::Domain);
        }
        if result.is_infinite() {
            return Err(ErrorKind::Overflow);
        }
    }
    Ok(result)
}

#[cfg(feature = "libm")]
float!(f32, libm::powf);
#[cfg(feature = "libm")]
float!(f64, libm::pow);

macro_rules! integer {
    ($($type:ty),*) => {$(
        impl Number for $type {
            fn parse(literal: &str) -> Option<Self> {
                literal.parse().ok()
            }

            fn add(self, rhs: Self) -> Result<Self, ErrorKind> {
                self.checked_add(rhs).ok_or(ErrorKind::Overflow)
            }

            fn subtract(self, rhs: Self) -> Result<Self, ErrorKind> {
                self.checked_sub(rhs).ok_or(ErrorKind::Overflow)
            }

            fn multiply(self, rhs: Self) -> Result<Self, ErrorKind> {
                self.checked_mul(rhs).ok_or(ErrorKind::Overflow)
            }

            fn divide(self, rhs: Self) -> Result<Self, ErrorKind> {
                if rhs == 0 {
                    return Err(ErrorKind::DivisionByZero);
                }
                self.checked_div(rhs).ok_or(ErrorKind::Overflow)
            }

            fn remainder(self, rhs: Self) -> Result<Self, ErrorKind> {
                if rhs == 0 {
                    return Err(ErrorKind::DivisionByZero);
                }
                self.checked_rem(rhs).ok_or(ErrorKind::Overflow)
            }

            #[allow(unused_comparisons)]
            fn power(self, rhs: Self) -> Result<Self, ErrorKind> {
                if rhs < 0 {
                    return Err(ErrorKind::Domain);
                }
                let exponent = u32::try_from(rhs).map_err(|_| ErrorKind::Overflow)?;
                self.checked_pow(exponent).ok_or(ErrorKind::Overflow)
            }

            fn negate(self) -> Result<Self, ErrorKind> {
                (0 as $type).checked_sub(self).ok_or(ErrorKind::Overflow)
            }
        }
    )*};
}

integer!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_report_overflow_and_division_by_zero() {
        assert_eq!(i8::MAX.add(1), Err(ErrorKind::Overflow));
        assert_eq!(i8::MIN.divide(-1), Err(ErrorKind::Overflow));
        assert_eq!(7_i32.divide(0), Err(ErrorKind::DivisionByZero));
        assert_eq!(7_i32.remainder(0), Err(ErrorKind::DivisionByZero));
        assert_eq!(1_u8.negate(), Err(ErrorKind::Overflow));
        assert_eq!(0_u8.negate(), Ok(0));
        assert_eq!(i8::MIN.negate(), Err(ErrorKind::Overflow));
    }

    #[test]
    fn integers_raise_to_non_negative_powers() {
        assert_eq!(2_i32.power(10), Ok(1024));
        assert_eq!(2_i32.power(-1), Err(ErrorKind::Domain));
        assert_eq!(2_i32.power(31), Err(ErrorKind::Overflow));
        assert_eq!(1_u64.power(u64::MAX), Err(ErrorKind::Overflow));
    }

    #[test]
    fn integers_reject_fractional_literals() {
        assert_eq!(i32::parse("42"), Some(42));
        assert_eq!(i32::parse("4.2"), None);
        assert_eq!(u8::parse("256"), None);
    }

    #[cfg(feature = "libm")]
    #[test]
    fn floats_report_errors_instead_of_special_values() {
        assert_eq!(1.0_f64.divide(0.0), Err(ErrorKind::DivisionByZero));
        assert_eq!(1.0_f64.remainder(0.0), Err(ErrorKind::DivisionByZero));
        assert_eq!(f64::MAX.multiply(2.0), Err(ErrorKind::Overflow));
        assert_eq!((-8.0_f64).power(0.5), Err(ErrorKind::Domain));
        assert_eq!(0.0_f64.power(-1.0), Err(ErrorKind::DivisionByZero));
        assert_eq!(2.0_f32.power(3.0), Ok(8.0));
        assert_eq!(f64::parse("1e3"), Some(1000.0));
    }

    #[cfg(feature = "libm")]
    #[test]
    fn floats_propagate_non_finite_operands() {
        assert_eq!(f64::INFINITY.add(1.0), Ok(f64::INFINITY));
        assert!(f64::NAN.add(1.0).unwrap().is_nan());
    }
}
