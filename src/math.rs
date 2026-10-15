//! The [`Math`] context of constants and scientific functions, and [`evaluate`] that uses it.

#[cfg(feature = "exact")]
use core::cmp::Ordering;

#[cfg(feature = "decimal")]
use crate::Decimal;
#[cfg(feature = "exact")]
use crate::Exact;
#[cfg(feature = "fraction")]
use crate::Fraction;
use crate::{Context, Error, ErrorKind, Evaluator, Number};

/// The usual constants and functions of a scientific calculator, for `f32` and `f64` with the `libm` feature, for
/// `Decimal` with the `decimal` feature, the exact ones for `Fraction` with the `fraction` feature, and both for
/// `Exact` with the `exact` feature.
///
/// | Name | Meaning |
/// |---|---|
/// | `pi`, `PI`, `tau`, `e`, `E` | Constants |
/// | `sqrt`, `cbrt`, `abs`, `sign` | Roots, absolute value, sign (`-1`, `0` or `1`) |
/// | `exp`, `ln`, `log2`, `log10` | Exponential and logarithms |
/// | `log(x)`, `log(x, base)` | Logarithm in base 10, or in `base` |
/// | `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2(y, x)` | Trigonometry, in radians |
/// | `sinh`, `cosh`, `tanh`, `asinh`, `acosh`, `atanh` | Hyperbolic functions |
/// | `floor`, `ceil`, `round`, `trunc` | Rounding |
/// | `pow(x, y)`, `hypot(x, y)` | Power, length of the hypotenuse |
/// | `min(...)`, `max(...)`, `clamp(x, low, high)` | Comparisons, `min` and `max` take one argument or more |
///
/// Operands outside a function's domain, such as `sqrt(-1)` or `ln(0)`, give [`ErrorKind::Domain`], and finite
/// operands whose result is infinite give [`ErrorKind::Overflow`].
///
/// `f32` and `f64` call the functions of [`libm`](https://crates.io/crates/libm). `Decimal` computes every function
/// in decimal with 36 significant digits, then rounds once to its own digits: results are correctly rounded except
/// in rare cases, which are one unit in the last place away. Results beyond its exponent range are
/// [`ErrorKind::Overflow`] errors and results too small for it become zero. Trigonometric functions of `Decimal`
/// arguments of `10^343` and beyond are [`ErrorKind::Domain`] errors.
///
/// `Fraction` never approximates: it has no constants, and only the functions whose results are exact fractions.
/// `abs`, `sign`, rounding, `min`, `max` and `clamp` always are, and `sqrt`, `cbrt`, `hypot` and `pow` are when
/// their result is rational, such as `sqrt(4/9) = 2/3`. Other results, and every other function, are
/// [`ErrorKind::Inexact`] errors.
///
/// `Exact` computes a function exactly when its arguments are fractions and its result is exact, and with the
/// `Decimal` function otherwise. Its constants are decimals, and `min`, `max` and `clamp` return one of their
/// arguments as it is, comparing values exactly.
///
/// Combine it with your own context to add variables or functions, yours taking precedence:
///
/// ```
/// # #[cfg(feature = "libm")] {
/// use nanoexpr::Math;
///
/// let context = ([("r", 2.0)], Math);
/// assert_eq!(nanoexpr::evaluate_with("pi * r ^ 2", &context), Ok(core::f64::consts::PI * 4.0));
/// # }
/// ```
///
/// ```
/// # #[cfg(feature = "decimal")] {
/// use nanoexpr::Decimal;
///
/// let sine: Decimal = nanoexpr::evaluate("sin(pi)").unwrap();
/// assert_eq!(sine.to_string(), "-6.7615373566167e-15");
/// # }
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Math;

/// The names of the functions of [`Math`], which take a wrong number of arguments when no other arm matches.
const FUNCTIONS: [&str; 31] = [
    "sqrt", "cbrt", "abs", "sign", "exp", "ln", "log2", "log10", "log", "sin", "cos", "tan",
    "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh", "asinh", "acosh", "atanh", "floor",
    "ceil", "round", "trunc", "pow", "hypot", "min", "max", "clamp",
];

/// Returns whether `value` is neither `NaN` nor infinite.
#[cfg(feature = "libm")]
fn finite<T: Into<f64>>(value: T) -> bool {
    value.into().is_finite()
}

/// Implements [`Context`] for a float type on [`Math`].
///
/// `$module` gathers wrappers named after the functions they implement, which call the `libm` function of the
/// right precision: `unary` ones take one argument and `binary` ones two.
#[cfg(feature = "libm")]
macro_rules! math {
    (
        $type:ident in $module:ident,
        unary: [$($unary:ident = $unary_libm:ident),* $(,)?],
        binary: [$($binary:ident = $binary_libm:ident),* $(,)?] $(,)?
    ) => {
        #[doc = concat!("`libm` functions for `", stringify!($type), "`, under the names expressions use.")]
        mod $module {
            $(
            #[doc = concat!("Calls `libm::", stringify!($unary_libm), "`.")]
            pub(super) fn $unary(x: $type) -> $type {
                libm::$unary_libm(x)
            })*
            $(
            #[doc = concat!("Calls `libm::", stringify!($binary_libm), "`.")]
            pub(super) fn $binary(x: $type, y: $type) -> $type {
                libm::$binary_libm(x, y)
            })*
        }

        impl Context<$type> for Math {
            fn variable(&self, name: &str) -> Option<$type> {
                use core::$type::consts;
                match name {
                    "pi" | "PI" => Some(consts::PI),
                    "tau" => Some(consts::TAU),
                    "e" | "E" => Some(consts::E),
                    _ => None,
                }
            }

            fn call(&self, name: &str, arguments: &[$type]) -> Option<Result<$type, ErrorKind>> {
                use $module::*;
                let domain = |valid: bool, value: $type| if valid { Ok(value) } else { Err(ErrorKind::Domain) };
                let result = match (name, arguments) {
                    ("sqrt", &[x]) => domain(x >= 0.0, sqrt(x)),
                    ("cbrt", &[x]) => Ok(cbrt(x)),
                    ("abs", &[x]) => Ok(x.abs()),
                    ("sign", &[x]) => domain(!x.is_nan(), if x == 0.0 { 0.0 } else { x.signum() }),
                    ("exp", &[x]) => Ok(exp(x)),
                    ("ln", &[x]) => domain(x > 0.0, ln(x)),
                    ("log2", &[x]) => domain(x > 0.0, log2(x)),
                    ("log10" | "log", &[x]) => domain(x > 0.0, log10(x)),
                    ("log", &[x, base]) => domain(x > 0.0 && base > 0.0 && base != 1.0, ln(x) / ln(base)),
                    ("sin", &[x]) => Ok(sin(x)),
                    ("cos", &[x]) => Ok(cos(x)),
                    ("tan", &[x]) => Ok(tan(x)),
                    ("asin", &[x]) => domain((-1.0..=1.0).contains(&x), asin(x)),
                    ("acos", &[x]) => domain((-1.0..=1.0).contains(&x), acos(x)),
                    ("atan", &[x]) => Ok(atan(x)),
                    ("atan2", &[y, x]) => Ok(atan2(y, x)),
                    ("sinh", &[x]) => Ok(sinh(x)),
                    ("cosh", &[x]) => Ok(cosh(x)),
                    ("tanh", &[x]) => Ok(tanh(x)),
                    ("asinh", &[x]) => Ok(asinh(x)),
                    ("acosh", &[x]) => domain(x >= 1.0, acosh(x)),
                    ("atanh", &[x]) => domain(x > -1.0 && x < 1.0, atanh(x)),
                    ("floor", &[x]) => Ok(floor(x)),
                    ("ceil", &[x]) => Ok(ceil(x)),
                    ("round", &[x]) => Ok(round(x)),
                    ("trunc", &[x]) => Ok(trunc(x)),
                    ("pow", &[x, y]) => return Some(x.power(y)),
                    ("hypot", &[x, y]) => Ok(hypot(x, y)),
                    ("min", &[first, ref rest @ ..]) => Ok(rest.iter().fold(first, |a, b| a.min(*b))),
                    ("max", &[first, ref rest @ ..]) => Ok(rest.iter().fold(first, |a, b| a.max(*b))),
                    ("clamp", &[x, low, high]) => domain(low <= high, x.max(low).min(high)),
                    _ if FUNCTIONS.contains(&name) => Err(ErrorKind::ArgumentCount),
                    _ => return None,
                };
                Some(result.and_then(|value| {
                    if !finite(value) && arguments.iter().all(|argument| finite(*argument)) {
                        Err(if value.is_nan() { ErrorKind::Domain } else { ErrorKind::Overflow })
                    } else {
                        Ok(value)
                    }
                }))
            }
        }
    };
}

#[cfg(feature = "libm")]
math!(
    f32 in single,
    unary: [
        sqrt = sqrtf, cbrt = cbrtf, exp = expf, ln = logf, log2 = log2f, log10 = log10f,
        sin = sinf, cos = cosf, tan = tanf, asin = asinf, acos = acosf, atan = atanf,
        sinh = sinhf, cosh = coshf, tanh = tanhf, asinh = asinhf, acosh = acoshf, atanh = atanhf,
        floor = floorf, ceil = ceilf, round = roundf, trunc = truncf,
    ],
    binary: [atan2 = atan2f, hypot = hypotf],
);

#[cfg(feature = "libm")]
math!(
    f64 in double,
    unary: [
        sqrt = sqrt, cbrt = cbrt, exp = exp, ln = log, log2 = log2, log10 = log10,
        sin = sin, cos = cos, tan = tan, asin = asin, acos = acos, atan = atan,
        sinh = sinh, cosh = cosh, tanh = tanh, asinh = asinh, acosh = acosh, atanh = atanh,
        floor = floor, ceil = ceil, round = round, trunc = trunc,
    ],
    binary: [atan2 = atan2, hypot = hypot],
);

/// Evaluates `input` with the constants and functions of [`Math`].
///
/// ```
/// # #[cfg(feature = "libm")] {
/// assert_eq!(nanoexpr::evaluate::<f64>("sqrt(16) + 2 ^ 3"), Ok(12.0));
/// assert_eq!(nanoexpr::evaluate::<f32>("max(1, cos(0), 0.5)"), Ok(1.0));
/// # }
/// ```
pub fn evaluate<N: Number>(input: &str) -> Result<N, Error>
where
    Math: Context<N>,
{
    Evaluator::<Math>::new(&Math).evaluate(input)
}

#[cfg(feature = "decimal")]
impl<const DIGITS: u32, const MAX_EXPONENT: i32> Context<Decimal<DIGITS, MAX_EXPONENT>> for Math {
    fn variable(&self, name: &str) -> Option<Decimal<DIGITS, MAX_EXPONENT>> {
        let literal = match name {
            "pi" | "PI" => "3.14159265358979323846264338327950288",
            "tau" => "6.28318530717958647692528676655900577",
            "e" | "E" => "2.71828182845904523536028747135266250",
            _ => return None,
        };
        Number::parse(literal)
    }

    fn call(
        &self,
        name: &str,
        arguments: &[Decimal<DIGITS, MAX_EXPONENT>],
    ) -> Option<Result<Decimal<DIGITS, MAX_EXPONENT>, ErrorKind>> {
        let result = match (name, arguments) {
            ("abs", &[x]) => Ok(x.abs()),
            ("sign", &[x]) if x.is_zero() => Ok(x),
            ("sign", &[x]) => Decimal::new(if x.is_negative() { -1 } else { 1 }, 0),
            ("floor", &[x]) => x.floor(),
            ("ceil", &[x]) => x.ceil(),
            ("round", &[x]) => x.round(),
            ("trunc", &[x]) => x.trunc(),
            ("pow", &[x, y]) => x.power(y),
            ("sqrt", &[x]) => x.sqrt(),
            ("cbrt", &[x]) => x.cbrt(),
            ("exp", &[x]) => x.exp(),
            ("ln", &[x]) => x.ln(),
            ("log2", &[x]) => x.log2(),
            ("log10" | "log", &[x]) => x.log10(),
            ("log", &[x, base]) => x.log(base),
            ("sin", &[x]) => x.sin(),
            ("cos", &[x]) => x.cos(),
            ("tan", &[x]) => x.tan(),
            ("asin", &[x]) => x.asin(),
            ("acos", &[x]) => x.acos(),
            ("atan", &[x]) => x.atan(),
            ("atan2", &[y, x]) => y.atan2(x),
            ("sinh", &[x]) => x.sinh(),
            ("cosh", &[x]) => x.cosh(),
            ("tanh", &[x]) => x.tanh(),
            ("asinh", &[x]) => x.asinh(),
            ("acosh", &[x]) => x.acosh(),
            ("atanh", &[x]) => x.atanh(),
            ("hypot", &[x, y]) => x.hypot(y),
            ("min", &[first, ref rest @ ..]) => Ok(rest.iter().fold(first, |a, b| a.min(*b))),
            ("max", &[first, ref rest @ ..]) => Ok(rest.iter().fold(first, |a, b| a.max(*b))),
            ("clamp", &[x, low, high]) if low <= high => Ok(x.clamp(low, high)),
            ("clamp", &[_, _, _]) => Err(ErrorKind::Domain),
            _ if FUNCTIONS.contains(&name) => Err(ErrorKind::ArgumentCount),
            _ => return None,
        };
        Some(result)
    }
}

/// The functions of [`Math`] whose results are irrational for most arguments, which a [`Fraction`] does not compute,
/// with the number of arguments they take.
#[cfg(feature = "fraction")]
const INEXACT: [(&str, usize); 21] = [
    ("exp", 1),
    ("ln", 1),
    ("log2", 1),
    ("log10", 1),
    ("log", 1),
    ("log", 2),
    ("sin", 1),
    ("cos", 1),
    ("tan", 1),
    ("asin", 1),
    ("acos", 1),
    ("atan", 1),
    ("atan2", 2),
    ("sinh", 1),
    ("cosh", 1),
    ("tanh", 1),
    ("asinh", 1),
    ("acosh", 1),
    ("atanh", 1),
    ("sqrt", 1),
    ("cbrt", 1),
];

#[cfg(feature = "fraction")]
impl Context<Fraction> for Math {
    fn call(&self, name: &str, arguments: &[Fraction]) -> Option<Result<Fraction, ErrorKind>> {
        let result = match (name, arguments) {
            ("abs", &[x]) => Ok(x.abs()),
            ("sign", &[x]) => Ok(x.sign()),
            ("floor", &[x]) => Ok(x.floor()),
            ("ceil", &[x]) => Ok(x.ceil()),
            ("round", &[x]) => Ok(x.round()),
            ("trunc", &[x]) => Ok(x.trunc()),
            ("pow", &[x, y]) => x.power(y),
            ("sqrt", &[x]) => x.root(2),
            ("cbrt", &[x]) => x.root(3),
            ("hypot", &[x, y]) => x
                .multiply(x)
                .and_then(|square| square.add(y.multiply(y)?))
                .and_then(|sum| sum.root(2)),
            ("min", &[first, ref rest @ ..]) => Ok(rest.iter().fold(first, |a, b| a.min(*b))),
            ("max", &[first, ref rest @ ..]) => Ok(rest.iter().fold(first, |a, b| a.max(*b))),
            ("clamp", &[x, low, high]) if low <= high => Ok(x.clamp(low, high)),
            ("clamp", &[_, _, _]) => Err(ErrorKind::Domain),
            _ if INEXACT.contains(&(name, arguments.len())) => Err(ErrorKind::Inexact),
            _ if FUNCTIONS.contains(&name) => Err(ErrorKind::ArgumentCount),
            _ => return None,
        };
        Some(result)
    }
}

#[cfg(feature = "exact")]
impl<const DIGITS: u32, const MAX_EXPONENT: i32> Context<Exact<DIGITS, MAX_EXPONENT>> for Math {
    fn variable(&self, name: &str) -> Option<Exact<DIGITS, MAX_EXPONENT>> {
        Context::<Decimal<DIGITS, MAX_EXPONENT>>::variable(self, name).map(Exact::from)
    }

    fn call(
        &self,
        name: &str,
        arguments: &[Exact<DIGITS, MAX_EXPONENT>],
    ) -> Option<Result<Exact<DIGITS, MAX_EXPONENT>, ErrorKind>> {
        let pick = |first, rest: &[Exact<DIGITS, MAX_EXPONENT>], keep: Ordering| {
            rest.iter().fold(first, |chosen, candidate| {
                if candidate.cmp_value(chosen) == keep {
                    *candidate
                } else {
                    chosen
                }
            })
        };
        match (name, arguments) {
            ("min", &[first, ref rest @ ..]) => return Some(Ok(pick(first, rest, Ordering::Less))),
            ("max", &[first, ref rest @ ..]) => {
                return Some(Ok(pick(first, rest, Ordering::Greater)));
            }
            ("clamp", &[x, low, high]) => {
                return Some(
                    match (low.cmp_value(high), x.cmp_value(low), x.cmp_value(high)) {
                        (Ordering::Greater, _, _) => Err(ErrorKind::Domain),
                        (_, Ordering::Less, _) => Ok(low),
                        (_, _, Ordering::Greater) => Ok(high),
                        _ => Ok(x),
                    },
                );
            }
            _ => {}
        }
        if arguments.len() > 3 {
            return FUNCTIONS
                .contains(&name)
                .then_some(Err(ErrorKind::ArgumentCount));
        }
        let mut fractions = [Fraction::ZERO; 3];
        let mut exact = true;
        for (slot, argument) in fractions.iter_mut().zip(arguments) {
            match argument.as_fraction() {
                Some(fraction) => *slot = fraction,
                None => exact = false,
            }
        }
        if exact {
            match Context::<Fraction>::call(self, name, &fractions[..arguments.len()]) {
                Some(Err(ErrorKind::Overflow | ErrorKind::Inexact)) | None => {}
                result => return result.map(|result| result.map(Exact::from)),
            }
        }
        let mut decimals = [Decimal::ZERO; 3];
        for (slot, argument) in decimals.iter_mut().zip(arguments) {
            *slot = argument.to_decimal();
        }
        Context::<Decimal<DIGITS, MAX_EXPONENT>>::call(self, name, &decimals[..arguments.len()])
            .map(|result| result.map(Exact::from))
    }
}
