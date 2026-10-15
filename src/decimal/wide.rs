//! An extended-precision decimal float, and the elementary functions [`Decimal`](super::Decimal) computes with it.

use core::cmp::Ordering;

/// Significant digits of a [`Wide`] coefficient.
const PRECISION: u32 = 36;

/// The smallest coefficient of a nonzero [`Wide`], `10^35`.
const SMALLEST: u128 = 10_u128.pow(PRECISION - 1);

/// The base of the limbs the argument reduction of trigonometric functions multiplies, `10^18`.
const LIMB: u128 = 1_000_000_000_000_000_000;

/// The largest exponent of a [`Wide`]: larger values saturate to it, and values below `10^-LIMIT` become zero.
const LIMIT: i64 = 1_000_000;

/// The first 432 digits of 2/π after the decimal point, 18 per limb.
const TWO_OVER_PI: [u64; 24] = [
    636619772367581343,
    75535053490057448,
    137838582961825794,
    990669376235587190,
    536906140360455211,
    65012343824291370,
    907031832147571647,
    384458314611511869,
    642926799356916959,
    867749636310292310,
    985587701230754869,
    571584869590646773,
    449560966894516047,
    329520456890799022,
    863761847560347610,
    695824481957643747,
    751376342114892399,
    785773600994689390,
    957838443593292387,
    132299624667945851,
    218797794608751526,
    299146267856964155,
    983496557394439935,
    472396799849771502,
];

/// A decimal float of 36 significant digits and a wide exponent range, which the elementary functions compute in
/// before rounding once to a [`Decimal`](super::Decimal).
///
/// Operations truncate their results, so each one is accurate to about one unit of the 36th digit.
#[derive(Clone, Copy)]
pub(super) struct Wide {
    /// Whether the number is below zero, never set for zero.
    negative: bool,
    /// Exactly 36 digits, or zero.
    coefficient: u128,
    /// Power of ten the coefficient is multiplied by, zero for zero.
    exponent: i32,
}

impl Wide {
    /// Zero.
    pub(super) const ZERO: Self = Self::constant(0, 0);
    /// One.
    pub(super) const ONE: Self = Self::constant(SMALLEST, -35);
    /// Two.
    pub(super) const TWO: Self = Self::constant(2 * SMALLEST, -35);
    /// One half.
    pub(super) const HALF: Self = Self::constant(5 * SMALLEST, -36);
    /// One quarter.
    const QUARTER: Self = Self::constant(25 * SMALLEST / 10, -36);
    /// One sixty-fourth.
    const SIXTY_FOURTH: Self = Self::constant(15625 * (SMALLEST / 10_000), -37);
    /// One tenth.
    const TENTH: Self = Self::constant(SMALLEST, -36);
    /// One million.
    const MILLION: Self = Self::constant(SMALLEST, -29);
    /// π.
    const PI: Self = Self::constant(314159265358979323846264338327950288, -35);
    /// π/2.
    const HALF_PI: Self = Self::constant(157079632679489661923132169163975144, -35);
    /// π/4.
    const QUARTER_PI: Self = Self::constant(785398163397448309615660845819875721, -36);
    /// The square root of 2.
    const SQRT_2: Self = Self::constant(141421356237309504880168872420969808, -35);
    /// The square root of 1/2.
    const SQRT_HALF: Self = Self::constant(707106781186547524400844362104849039, -36);
    /// The coefficient of the square root of 10.
    const SQRT_10: u128 = 316227766016837933199889354443271853;
    /// The natural logarithm of 10.
    pub(super) const LN_10: Self = Self::constant(230258509299404568401799145468436421, -35);
    /// The natural logarithm of 2.
    pub(super) const LN_2: Self = Self::constant(693147180559945309417232121458176568, -36);

    /// Creates a positive number from a coefficient that is already normalized.
    const fn constant(coefficient: u128, exponent: i32) -> Self {
        Self {
            negative: false,
            coefficient,
            exponent,
        }
    }

    /// Creates `coefficient × 10^exponent`, truncated to 36 digits.
    ///
    /// Exponents beyond the range saturate, and numbers too small for it become zero.
    pub(super) fn new(negative: bool, mut coefficient: u128, mut exponent: i64) -> Self {
        let Some(logarithm) = coefficient.checked_ilog10() else {
            return Self::ZERO;
        };
        let digits = logarithm + 1;
        if digits > PRECISION {
            coefficient /= 10_u128.pow(digits - PRECISION);
            exponent += i64::from(digits - PRECISION);
        } else {
            coefficient *= 10_u128.pow(PRECISION - digits);
            exponent -= i64::from(PRECISION - digits);
        }
        if exponent < -LIMIT {
            return Self::ZERO;
        }
        Self {
            negative,
            coefficient,
            exponent: exponent.min(LIMIT) as i32,
        }
    }

    /// Creates an integer.
    pub(super) fn integer(value: i64) -> Self {
        Self::new(value < 0, u128::from(value.unsigned_abs()), 0)
    }

    /// Returns the sign, the 36-digit coefficient and the exponent.
    pub(super) fn parts(self) -> (bool, u128, i64) {
        (self.negative, self.coefficient, i64::from(self.exponent))
    }

    /// Returns whether the number is zero.
    pub(super) fn is_zero(self) -> bool {
        self.coefficient == 0
    }

    /// Returns the number with the opposite sign, keeping zero positive.
    pub(super) fn negate(self) -> Self {
        Self {
            negative: !self.negative && !self.is_zero(),
            ..self
        }
    }

    /// Returns the absolute value.
    pub(super) fn abs(self) -> Self {
        Self {
            negative: false,
            ..self
        }
    }

    /// Returns the number with the sign of `negative` applied.
    pub(super) fn signed(self, negative: bool) -> Self {
        if negative { self.negate() } else { self }
    }

    /// Compares the absolute values.
    fn compare_magnitude(self, other: Self) -> Ordering {
        match (self.is_zero(), other.is_zero()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => {
                (self.exponent, self.coefficient).cmp(&(other.exponent, other.coefficient))
            }
        }
    }

    /// Returns the sum.
    pub(super) fn add(self, other: Self) -> Self {
        if self.is_zero() {
            return other;
        }
        if other.is_zero() {
            return self;
        }
        let (large, small) = if self.exponent >= other.exponent {
            (self, other)
        } else {
            (other, self)
        };
        let shift = large.exponent.abs_diff(small.exponent);
        if shift > PRECISION + 1 {
            return large;
        }
        let aligned = large.coefficient * 10;
        let other = if shift == 0 {
            small.coefficient * 10
        } else {
            small.coefficient / 10_u128.pow(shift - 1)
        };
        let exponent = i64::from(large.exponent) - 1;
        if large.negative == small.negative {
            Self::new(large.negative, aligned + other, exponent)
        } else if aligned >= other {
            Self::new(large.negative, aligned - other, exponent)
        } else {
            Self::new(small.negative, other - aligned, exponent)
        }
    }

    /// Returns the difference.
    pub(super) fn sub(self, other: Self) -> Self {
        self.add(other.negate())
    }

    /// Returns the product, from the four products of the halves of the 36-digit coefficients.
    pub(super) fn mul(self, other: Self) -> Self {
        let (left_high, left_low) = (self.coefficient / LIMB, self.coefficient % LIMB);
        let (right_high, right_low) = (other.coefficient / LIMB, other.coefficient % LIMB);
        let middle = left_high * right_low + left_low * right_high + left_low * right_low / LIMB;
        Self::new(
            self.negative != other.negative,
            left_high * right_high * 10 + middle / (LIMB / 10),
            i64::from(self.exponent) + i64::from(other.exponent) + 35,
        )
    }

    /// Returns the quotient by long division, two digits at a time, saturating when dividing by zero.
    pub(super) fn div(self, other: Self) -> Self {
        let negative = self.negative != other.negative;
        if self.is_zero() {
            return Self::ZERO;
        }
        if other.is_zero() {
            return Self::new(negative, 1, LIMIT);
        }
        let mut quotient = 0;
        let mut remainder = self.coefficient;
        for _ in 0..19 {
            quotient = quotient * 100 + remainder / other.coefficient;
            remainder = remainder % other.coefficient * 100;
        }
        Self::new(
            negative,
            quotient,
            i64::from(self.exponent) - i64::from(other.exponent) - 36,
        )
    }

    /// Returns the quotient by a small positive integer.
    fn div_integer(self, divisor: u128) -> Self {
        Self::new(
            self.negative,
            (self.coefficient * 100)
                .checked_div(divisor)
                .unwrap_or_default(),
            i64::from(self.exponent) - 2,
        )
    }

    /// Returns the number times `10^power`.
    fn scale(self, power: i64) -> Self {
        Self::new(
            self.negative,
            self.coefficient,
            i64::from(self.exponent).saturating_add(power),
        )
    }

    /// Returns the square root of the absolute value: the integer square root of the coefficient, refined by one
    /// step of Newton's method.
    pub(super) fn sqrt(self) -> Self {
        if self.is_zero() {
            return Self::ZERO;
        }
        let odd = self.exponent.rem_euclid(2) == 1;
        let radicand = if odd {
            self.coefficient * 10
        } else {
            self.coefficient
        };
        let estimate = Self::new(
            false,
            integer_square_root(radicand),
            (i64::from(self.exponent) - i64::from(odd)) / 2,
        );
        estimate.add(self.abs().div(estimate)).mul(Self::HALF)
    }

    /// Returns the nearest integer, saturating far beyond the range of `i64`.
    fn nearest_integer(self) -> i64 {
        let shift = -i64::from(self.exponent);
        let magnitude = if self.is_zero() {
            0
        } else if shift <= 0 {
            u128::MAX
        } else if shift > 37 {
            0
        } else {
            let divisor = 10_u128.pow(shift as u32);
            self.coefficient / divisor + u128::from(self.coefficient % divisor * 2 >= divisor)
        };
        let magnitude = i64::try_from(magnitude).unwrap_or(i64::MAX);
        if self.negative { -magnitude } else { magnitude }
    }
}

/// Returns the integer square root, one bit at a time.
fn integer_square_root(value: u128) -> u128 {
    let mut bit = 1_u128 << 126;
    while bit > value {
        bit >>= 2;
    }
    let mut rest = value;
    let mut root = 0;
    while bit != 0 {
        if rest >= root + bit {
            rest -= root + bit;
            root = (root >> 1) + bit;
        } else {
            root >>= 1;
        }
        bit >>= 2;
    }
    root
}

/// Returns whether adding `term` to `sum` no longer changes its 36 digits.
fn negligible(term: Wide, sum: Wide) -> bool {
    term.is_zero() || i64::from(term.exponent) + i64::from(PRECISION) < i64::from(sum.exponent)
}

/// Returns `z + z³/3 + z⁵/5 + …`, the inverse hyperbolic tangent, or `z - z³/3 + z⁵/5 - …`, the inverse tangent,
/// when `alternating`, for `|z|` up to about 0.2.
fn odd_series(z: Wide, alternating: bool) -> Wide {
    let square = z.mul(z).signed(alternating);
    let mut power = z;
    let mut sum = z;
    for index in 1..48 {
        power = power.mul(square);
        let term = power.div_integer(2 * index + 1);
        if negligible(term, sum) {
            break;
        }
        sum = sum.add(term);
    }
    sum
}

/// Returns `first - first·square/(n(n+1)) + …`, the Taylor series of the sine when `index` is 2 and of the cosine
/// when it is 1, for `square` up to (π/4)².
fn trigonometric_series(first: Wide, square: Wide, mut index: u128) -> Wide {
    let mut term = first;
    let mut sum = first;
    for _ in 0..24 {
        term = term
            .mul(square)
            .div_integer(index)
            .div_integer(index + 1)
            .negate();
        index += 2;
        if negligible(term, sum) {
            break;
        }
        sum = sum.add(term);
    }
    sum
}

/// Returns `(fraction, power)` such that `e^x = (1 + fraction) × 10^power`, with `fraction` accurate to all its
/// digits when `x` is tiny.
///
/// Subtracting the nearest multiple of ln 10 leaves at most 1.16, divided by 64 for a short Taylor series, whose
/// result is squared back six times with `(1 + f)² = 1 + f(f + 2)`.
pub(super) fn exp_parts(x: Wide) -> (Wide, i64) {
    if x.compare_magnitude(Wide::MILLION) != Ordering::Less {
        return if x.negative {
            (Wide::ONE.negate(), 0)
        } else {
            (Wide::ZERO, LIMIT)
        };
    }
    let power = x.div(Wide::LN_10).nearest_integer();
    let reduced = x
        .sub(Wide::integer(power).mul(Wide::LN_10))
        .mul(Wide::SIXTY_FOURTH);
    let mut fraction = reduced;
    let mut term = reduced;
    for index in 2..32 {
        term = term.mul(reduced).div_integer(index);
        if negligible(term, fraction) {
            break;
        }
        fraction = fraction.add(term);
    }
    for _ in 0..6 {
        fraction = fraction.mul(fraction.add(Wide::TWO));
    }
    (fraction, power)
}

/// Returns `e^x`.
pub(super) fn exp(x: Wide) -> Wide {
    let (fraction, power) = exp_parts(x);
    Wide::ONE.add(fraction).scale(power)
}

/// Returns `(power, rest)` such that `ln x = power × ln 10 + rest` for a positive `x`, with `rest` accurate to all
/// its digits when `x` is close to 1.
///
/// The coefficient is brought between 1/√2 and √2 by powers of 10 and 2, then `ln v = 2 atanh((v - 1)/(v + 1))`.
pub(super) fn ln_parts(x: Wide) -> (i64, Wide) {
    let mut power = i64::from(x.exponent) + 35;
    let mut value = Wide::constant(x.coefficient, -35);
    if x.coefficient > Wide::SQRT_10 {
        value.exponent = -36;
        power += 1;
    }
    let mut twos = 0;
    for _ in 0..2 {
        if value.compare_magnitude(Wide::SQRT_2) == Ordering::Greater {
            value = value.mul(Wide::HALF);
            twos += 1;
        } else if value.compare_magnitude(Wide::SQRT_HALF) == Ordering::Less {
            value = value.add(value);
            twos -= 1;
        }
    }
    let series = odd_series(value.sub(Wide::ONE).div(value.add(Wide::ONE)), false);
    (
        power,
        Wide::LN_2.mul(Wide::integer(twos)).add(series.add(series)),
    )
}

/// Returns `ln x` for a positive `x`.
pub(super) fn ln(x: Wide) -> Wide {
    let (power, rest) = ln_parts(x);
    Wide::LN_10.mul(Wide::integer(power)).add(rest)
}

/// Returns `ln(1 + u)`, accurate to all its digits when `u` is tiny.
pub(super) fn ln_1p(u: Wide) -> Wide {
    if u.compare_magnitude(Wide::QUARTER) == Ordering::Less {
        let series = odd_series(u.div(Wide::TWO.add(u)), false);
        series.add(series)
    } else {
        ln(Wide::ONE.add(u))
    }
}

/// Returns the 18 digits of 2/π after the decimal point starting at the 1-based position `index`, with zeros
/// before the point.
fn two_over_pi(index: i64) -> u128 {
    let limb = |position: i64| {
        usize::try_from(position)
            .ok()
            .and_then(|position| TWO_OVER_PI.get(position))
            .map_or(0, |&limb| u128::from(limb))
    };
    let position = index - 1;
    let offset = position.rem_euclid(18) as u32;
    let first = position.div_euclid(18);
    limb(first) % 10_u128.pow(18 - offset) * 10_u128.pow(offset)
        + limb(first + 1) / 10_u128.pow(18 - offset)
}

/// Returns `(r, k)` such that `coefficient × 10^exponent = k·π/2 + r` with `|r| ≤ π/4`, or `None` past the digits
/// of 2/π stored.
///
/// Multiplying by 2/π only needs its digits from about the exponent on, since earlier ones add multiples of 4 to
/// `k`: a window of 108 of them gives 90 digits of fraction, enough for `r` to keep 36 digits when the argument is
/// within 10^-50 of a multiple of π/2.
fn reduce(coefficient: u64, exponent: i32) -> Option<(Wide, u128)> {
    let exponent = i64::from(exponent);
    if exponent + 90 > 18 * TWO_OVER_PI.len() as i64 {
        return None;
    }
    let mut product = [0_u128; 7];
    let mut carry = 0;
    for index in (0..6).rev() {
        let value =
            u128::from(coefficient) * two_over_pi(exponent - 17 + 18 * index as i64) + carry;
        product[index + 1] = value % LIMB;
        carry = value / LIMB;
    }
    let mut quadrant = product[1] % 4;
    let fraction = &mut product[2..];
    let complement = fraction[0] >= LIMB / 2;
    if complement {
        quadrant += 1;
        let mut borrow = 0;
        for limb in fraction.iter_mut().rev() {
            let value = *limb + borrow;
            (*limb, borrow) = if value == 0 {
                (0, 0)
            } else {
                (LIMB - value, 1)
            };
        }
    }
    let Some(start) = fraction.iter().position(|&limb| limb != 0) else {
        return Some((Wide::ZERO, quadrant));
    };
    let limb = |index: usize| fraction.get(index).copied().unwrap_or(0);
    let weight = -18 * start as i64;
    let reduced = Wide::new(false, limb(start) * LIMB + limb(start + 1), weight - 36)
        .add(Wide::new(false, limb(start + 2), weight - 54));
    Some((reduced.mul(Wide::HALF_PI).signed(complement), quadrant))
}

/// Returns the sine and cosine of `coefficient × 10^exponent`, or `None` from `10^343` on, past the digits of 2/π
/// stored.
pub(super) fn sine_cosine(coefficient: u64, exponent: i32) -> Option<(Wide, Wide)> {
    let x = Wide::new(false, u128::from(coefficient), i64::from(exponent));
    if x.exponent + 35 > 342 {
        return None;
    }
    let (reduced, quadrant) = if x.compare_magnitude(Wide::QUARTER_PI) == Ordering::Less {
        (x, 0)
    } else {
        reduce(coefficient, exponent)?
    };
    let square = reduced.mul(reduced);
    let sine = trigonometric_series(reduced, square, 2);
    let cosine = trigonometric_series(Wide::ONE, square, 1);
    Some(match quadrant % 4 {
        0 => (sine, cosine),
        1 => (cosine, sine.negate()),
        2 => (sine.negate(), cosine.negate()),
        _ => (cosine.negate(), sine),
    })
}

/// Returns the inverse tangent, from `π/2 - atan(1/x)` above 1 and three halvings
/// `atan x = 2 atan(x / (1 + √(1 + x²)))` down to 0.1, before the series.
pub(super) fn atan(x: Wide) -> Wide {
    let invert = x.compare_magnitude(Wide::ONE) == Ordering::Greater;
    let mut z = if invert {
        Wide::ONE.div(x.abs())
    } else {
        x.abs()
    };
    let mut factor = 1;
    for _ in 0..3 {
        if z.compare_magnitude(Wide::TENTH) == Ordering::Greater {
            z = z.div(Wide::ONE.add(Wide::ONE.add(z.mul(z)).sqrt()));
            factor *= 2;
        }
    }
    let angle = odd_series(z, true).mul(Wide::integer(factor));
    let angle = if invert {
        Wide::HALF_PI.sub(angle)
    } else {
        angle
    };
    angle.signed(x.negative)
}

/// Returns the angle of the point `(x, y)`, between -π and π.
pub(super) fn atan2(y: Wide, x: Wide) -> Wide {
    if x.is_zero() {
        return if y.is_zero() {
            Wide::ZERO
        } else {
            Wide::HALF_PI.signed(y.negative)
        };
    }
    let angle = atan(y.div(x));
    if !x.negative {
        angle
    } else if y.negative {
        angle.sub(Wide::PI)
    } else {
        angle.add(Wide::PI)
    }
}

/// Returns the hyperbolic sine, from `e^x - e^-x = f + f/(1 + f)` with `f = e^x - 1` near zero.
pub(super) fn sinh(x: Wide) -> Wide {
    let (fraction, power) = exp_parts(x.abs());
    let twice = if power == 0 {
        fraction.add(fraction.div(Wide::ONE.add(fraction)))
    } else {
        let exponential = Wide::ONE.add(fraction).scale(power);
        exponential.sub(Wide::ONE.div(exponential))
    };
    twice.mul(Wide::HALF).signed(x.negative)
}

/// Returns the hyperbolic cosine.
pub(super) fn cosh(x: Wide) -> Wide {
    let exponential = exp(x.abs());
    exponential.add(Wide::ONE.div(exponential)).mul(Wide::HALF)
}

/// Returns the hyperbolic tangent, from `f/(f + 2)` with `f = e^2x - 1` near zero.
pub(super) fn tanh(x: Wide) -> Wide {
    let (fraction, power) = exp_parts(x.abs().add(x.abs()));
    let magnitude = if power == 0 {
        fraction.div(fraction.add(Wide::TWO))
    } else {
        let exponential = Wide::ONE.add(fraction).scale(power);
        exponential.sub(Wide::ONE).div(exponential.add(Wide::ONE))
    };
    magnitude.signed(x.negative)
}
