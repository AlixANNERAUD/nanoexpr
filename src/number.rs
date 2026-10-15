//! The [`Number`] trait and its implementations for primitive types.

use core::cmp::Ordering;

use crate::ErrorKind;

/// A value type expressions are evaluated in.
///
/// Implemented for `f32` and `f64` with the `libm` feature, and for every primitive integer type. Implement it for
/// your own type, such as a fixed-point number, to evaluate expressions in it.
///
/// Each operation returns the [`ErrorKind`] to report instead of producing a silent `NaN`, infinity or wrapped value.
///
/// Comparisons (`<`, `==`...) and logic operators (`&&`, `||`, `!`, `? :`) need [`Number::compare`],
/// [`Number::is_zero`] and [`Number::from_bool`]. They have default implementations, so a type that only
/// implements arithmetic keeps working: override [`Number::compare`] to support comparisons and logic, as the
/// other two are derived from it.
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

    /// Returns how `self` orders against `rhs`.
    ///
    /// Return an error for values that cannot be ordered, such as [`ErrorKind::Domain`] for a float `NaN`. The
    /// default returns [`ErrorKind::Unsupported`].
    fn compare(self, rhs: Self) -> Result<Ordering, ErrorKind> {
        let _ = rhs;
        Err(ErrorKind::Unsupported)
    }

    /// Returns whether `self` is zero, which logic operators read as false.
    ///
    /// The default compares `self` with [`Default::default`], which it assumes to be zero.
    fn is_zero(self) -> Result<bool, ErrorKind> {
        self.compare(Self::default())
            .map(|ordering| ordering == Ordering::Equal)
    }

    /// Returns `1` for `true` and `0` for `false`, the results of comparisons and logic operators.
    ///
    /// The default parses the literals `1` and `0`, and returns [`ErrorKind::Unsupported`] if that fails.
    fn from_bool(value: bool) -> Result<Self, ErrorKind> {
        Self::parse(if value { "1" } else { "0" }).ok_or(ErrorKind::Unsupported)
    }

    /// Returns the factorial `self!`, the product `1 * 2 * ... * self`, with `0! = 1`.
    ///
    /// Return [`ErrorKind::Domain`] when `self` is negative or not an integer, and [`ErrorKind::Overflow`] when
    /// the result does not fit. The default counts from zero to `self` with [`Number::add`], [`Number::compare`]
    /// and [`Number::from_bool`], multiplying as it goes, so it needs [`Number::compare`] and rounds each product
    /// as [`Number::multiply`] does. It gives up with [`ErrorKind::Overflow`] after 10 000 multiplications, so a
    /// type with a huge or unbounded range cannot compute for ever. Override it to round only once, as `Decimal`
    /// does.
    fn factorial(self) -> Result<Self, ErrorKind> {
        factorial(self)
    }
}

/// Most multiplications [`factorial`] does before giving up with [`ErrorKind::Overflow`].
///
/// Built-in number types overflow long before, at `171!` for `f64` or `70!` for the default `Decimal`. The limit
/// only stops number types of your own with a huge or unbounded range from computing for ever.
pub(crate) const MAX_FACTORIAL: usize = 10_000;

/// Returns `n!`, the product `1 * 2 * ... * n`, with `0! = 1`.
///
/// Counts from zero to `n` with [`Number::add`] and [`Number::compare`], multiplying as it goes, so the results of
/// integer types and `Fraction` are exact, and `Decimal` and floats round each product as their multiplication
/// does. Fails with [`ErrorKind::Domain`] when `n` is negative or not an integer, and with [`ErrorKind::Overflow`]
/// when a product does not fit or `n` is above [`MAX_FACTORIAL`].
fn factorial<N: Number>(n: N) -> Result<N, ErrorKind> {
    let one = N::from_bool(true)?;
    let mut count = N::from_bool(false)?;
    let mut product = one;
    let mut ordering = count.compare(n)?;
    let mut steps = 0;
    while ordering == Ordering::Less {
        if steps == MAX_FACTORIAL {
            return Err(ErrorKind::Overflow);
        }
        steps += 1;
        count = count.add(one)?;
        ordering = count.compare(n)?;
        if ordering == Ordering::Greater {
            break;
        }
        product = product.multiply(count)?;
    }
    match ordering {
        Ordering::Equal => Ok(product),
        _ => Err(ErrorKind::Domain),
    }
}

/// Implements [`Number`] and [`Checked`] for a float type, raising to a power with the `libm` function `$power`.
#[cfg(feature = "libm")]
macro_rules! float {
    ($type:ty, $power:path) => {
        impl Number for $type {
            fn parse(literal: &str) -> Option<Self> {
                literal
                    .parse()
                    .ok()
                    .filter(|value: &$type| value.is_finite())
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

            fn compare(self, rhs: Self) -> Result<Ordering, ErrorKind> {
                self.partial_cmp(&rhs).ok_or(ErrorKind::Domain)
            }

            fn is_zero(self) -> Result<bool, ErrorKind> {
                if self.is_nan() {
                    return Err(ErrorKind::Domain);
                }
                Ok(self == 0.0)
            }

            fn from_bool(value: bool) -> Result<Self, ErrorKind> {
                Ok(if value { 1.0 } else { 0.0 })
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

/// The float classification [`check`] needs, shared by `f32` and `f64`.
#[cfg(feature = "libm")]
trait Checked: Copy {
    /// Returns whether the value is `NaN`.
    fn is_nan(self) -> bool;
    /// Returns whether the value is an infinity.
    fn is_infinite(self) -> bool;
}

/// Turns a `NaN` or infinite `result` of finite operands into an error.
///
/// A `NaN` is a [`ErrorKind::Domain`] error and an infinity an [`ErrorKind::Overflow`]. Results of operands that
/// are already `NaN` or infinite are passed through unchanged.
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

/// Implements [`Number`] for integer types with checked arithmetic, so overflows are errors instead of wrapping.
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

            fn compare(self, rhs: Self) -> Result<Ordering, ErrorKind> {
                Ok(self.cmp(&rhs))
            }

            fn is_zero(self) -> Result<bool, ErrorKind> {
                Ok(self == 0)
            }

            fn from_bool(value: bool) -> Result<Self, ErrorKind> {
                Ok(Self::from(value))
            }
        }
    )*};
}

integer!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

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

    #[test]
    fn integers_compare_and_convert_booleans() {
        assert_eq!(3_u8.compare(4), Ok(Ordering::Less));
        assert_eq!((-1_i64).compare(-1), Ok(Ordering::Equal));
        assert_eq!(0_u128.is_zero(), Ok(true));
        assert_eq!((-5_i8).is_zero(), Ok(false));
        assert_eq!(i16::from_bool(true), Ok(1));
        assert_eq!(usize::from_bool(false), Ok(0));
    }

    #[cfg(feature = "libm")]
    #[test]
    fn floats_compare_and_reject_nan() {
        assert_eq!(1.5_f64.compare(-2.0), Ok(Ordering::Greater));
        assert_eq!((-0.0_f32).compare(0.0), Ok(Ordering::Equal));
        assert_eq!(f64::NAN.compare(1.0), Err(ErrorKind::Domain));
        assert_eq!(1.0_f32.compare(f32::NAN), Err(ErrorKind::Domain));
        assert_eq!((-0.0_f64).is_zero(), Ok(true));
        assert_eq!(f64::INFINITY.is_zero(), Ok(false));
        assert_eq!(f32::NAN.is_zero(), Err(ErrorKind::Domain));
        assert_eq!(f64::from_bool(true), Ok(1.0));
        assert_eq!(f32::from_bool(false), Ok(0.0));
    }

    /// A number that implements only the required methods of [`Number`].
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    struct Plain(i32);

    impl Number for Plain {
        fn parse(literal: &str) -> Option<Self> {
            literal.parse().ok().map(Self)
        }

        fn add(self, rhs: Self) -> Result<Self, ErrorKind> {
            Ok(Self(self.0 + rhs.0))
        }

        fn subtract(self, rhs: Self) -> Result<Self, ErrorKind> {
            Ok(Self(self.0 - rhs.0))
        }

        fn multiply(self, rhs: Self) -> Result<Self, ErrorKind> {
            Ok(Self(self.0 * rhs.0))
        }

        fn divide(self, rhs: Self) -> Result<Self, ErrorKind> {
            Ok(Self(self.0 / rhs.0))
        }

        fn remainder(self, rhs: Self) -> Result<Self, ErrorKind> {
            Ok(Self(self.0 % rhs.0))
        }

        fn power(self, rhs: Self) -> Result<Self, ErrorKind> {
            Ok(Self(self.0.pow(rhs.0.unsigned_abs())))
        }

        fn negate(self) -> Result<Self, ErrorKind> {
            Ok(Self(-self.0))
        }
    }

    #[test]
    fn defaults_report_unsupported_comparisons() {
        assert_eq!(Plain(1).compare(Plain(2)), Err(ErrorKind::Unsupported));
        assert_eq!(Plain(1).is_zero(), Err(ErrorKind::Unsupported));
        assert_eq!(Plain::from_bool(true), Ok(Plain(1)));
        assert_eq!(Plain::from_bool(false), Ok(Plain(0)));
        assert_eq!(Plain(3).factorial(), Err(ErrorKind::Unsupported));
    }

    /// A number that never overflows, saturating at `u64::MAX` instead.
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    struct Saturating(u64);

    impl Number for Saturating {
        fn parse(literal: &str) -> Option<Self> {
            literal.parse().ok().map(Self)
        }

        fn add(self, rhs: Self) -> Result<Self, ErrorKind> {
            Ok(Self(self.0.saturating_add(rhs.0)))
        }

        fn subtract(self, rhs: Self) -> Result<Self, ErrorKind> {
            Ok(Self(self.0.saturating_sub(rhs.0)))
        }

        fn multiply(self, rhs: Self) -> Result<Self, ErrorKind> {
            Ok(Self(self.0.saturating_mul(rhs.0)))
        }

        fn divide(self, rhs: Self) -> Result<Self, ErrorKind> {
            Ok(Self(self.0 / rhs.0))
        }

        fn remainder(self, rhs: Self) -> Result<Self, ErrorKind> {
            Ok(Self(self.0 % rhs.0))
        }

        fn power(self, rhs: Self) -> Result<Self, ErrorKind> {
            Ok(Self(self.0.saturating_pow(rhs.0 as u32)))
        }

        fn negate(self) -> Result<Self, ErrorKind> {
            Ok(self)
        }

        fn compare(self, rhs: Self) -> Result<Ordering, ErrorKind> {
            Ok(self.0.cmp(&rhs.0))
        }
    }

    #[test]
    fn factorials_stop_after_a_bounded_number_of_steps() {
        assert_eq!(Saturating(0).factorial(), Ok(Saturating(1)));
        assert_eq!(Saturating(4).factorial(), Ok(Saturating(24)));
        let last = MAX_FACTORIAL as u64;
        assert_eq!(Saturating(last).factorial(), Ok(Saturating(u64::MAX)));
        assert_eq!(Saturating(last + 1).factorial(), Err(ErrorKind::Overflow));
        assert_eq!(Saturating(u64::MAX).factorial(), Err(ErrorKind::Overflow));
    }

    #[cfg(feature = "libm")]
    #[test]
    fn floats_propagate_non_finite_operands() {
        assert_eq!(f64::INFINITY.add(1.0), Ok(f64::INFINITY));
        assert!(f64::NAN.add(1.0).unwrap().is_nan());
    }
}
