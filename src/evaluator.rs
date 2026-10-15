//! Evaluates expressions in a single pass, by precedence climbing.
//!
//! The value of each operand is computed as soon as it is parsed, so nothing is kept but the current token and
//! the values on the call stack. The operands `&&`, `||` and `? :` do not need are still parsed, to check their
//! syntax, but not computed.

use core::marker::PhantomData;

use crate::parser::{Backend, Binary, Parser, Syntax, Unary};
use crate::{Context, Error, ErrorKind, Number, Span};

/// How the `%` sign is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PercentSign {
    /// `7 % 3` is the remainder of `7 / 3`, like `7 mod 3`.
    #[default]
    Remainder,
    /// `50%` is `50 / 100`, as on a pocket calculator. `mod` remains the remainder.
    Percentage,
}

/// Evaluates expressions against a [`Context`], with configurable limits and syntax.
///
/// `&&`, `||` and `condition ? then : otherwise` short-circuit: the operand they do not need is parsed, so its
/// syntax is checked, but it is not computed and the context is not asked for its names. So `x != 0 && 1 / x > 2`
/// is `0` rather than a division by zero when `x` is zero, and `x == 0 ? 0 : 1 / x` is `0`.
///
/// Errors that depend only on the text (syntax errors, [`ErrorKind::InvalidNumber`], [`ErrorKind::TooDeep`] and
/// [`ErrorKind::TooManyArguments`]) are reported wherever they are, even in an operand that is not computed, and
/// before any other error: a malformed expression is reported as such whatever the values. Otherwise the first
/// error in evaluation order is reported.
///
/// `ARGUMENTS` is the most arguments a single function call can take. They are held in an array on the stack, so
/// a call uses `ARGUMENTS * size_of::<N>()` bytes of stack for each level of nesting.
///
/// ```
/// # #[cfg(feature = "libm")] {
/// use nanoexpr::{Evaluator, PercentSign};
///
/// let evaluator = Evaluator::<_, 2>::new(&[("price", 80.0)])
///     .max_depth(8)
///     .percent_sign(PercentSign::Percentage);
/// assert_eq!(evaluator.evaluate::<f64>("price * (1 - 25%)"), Ok(60.0));
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Evaluator<'c, C: ?Sized, const ARGUMENTS: usize = 8> {
    /// Resolves variables and functions.
    context: &'c C,
    /// Limits and syntax.
    syntax: Syntax,
}

impl<'c, C: ?Sized, const ARGUMENTS: usize> Evaluator<'c, C, ARGUMENTS> {
    /// Default limit of nested parentheses, calls and operators.
    pub const DEFAULT_MAX_DEPTH: usize = 32;

    /// Creates an evaluator that resolves names with `context`.
    pub const fn new(context: &'c C) -> Self {
        Self {
            context,
            syntax: Syntax {
                max_depth: Self::DEFAULT_MAX_DEPTH,
                percent_sign: PercentSign::Remainder,
                implicit_multiplication: false,
            },
        }
    }

    /// Sets how deeply parentheses, calls and operators can nest before [`ErrorKind::TooDeep`].
    ///
    /// Evaluation recurses once per level, so this bounds the stack it uses.
    pub const fn max_depth(mut self, max_depth: usize) -> Self {
        self.syntax.max_depth = max_depth;
        self
    }

    /// Sets how the `%` sign is read.
    pub const fn percent_sign(mut self, percent_sign: PercentSign) -> Self {
        self.syntax.percent_sign = percent_sign;
        self
    }

    /// Sets whether an operand directly followed by a name or a `(` multiplies them, as on a calculator.
    ///
    /// Off by default. When on, `2pi`, `2x`, `3sqrt(4)`, `2(3 + 4)`, `(1 + 2)(3 + 4)` and `x y` are products.
    /// Implicit multiplication has the precedence of `*`, as on TI calculators, so `1/2x` is `(1/2) * x` and
    /// `2x^2` is `2 * x^2`. A name directly followed by `(` remains a call: `x(2)` calls the function `x`. A number
    /// does not multiply the operand before it, so `2 3` and `x 2` stay errors, and `2e3` is still the number
    /// `2000` while `2e` is `2 * e`.
    ///
    /// ```
    /// # #[cfg(feature = "libm")] {
    /// use nanoexpr::{Evaluator, Math};
    ///
    /// let context = ([("x", 3.0)], Math);
    /// let calculator = Evaluator::<_>::new(&context).implicit_multiplication(true);
    /// assert_eq!(calculator.evaluate::<f64>("2x + 3(x - 1)"), Ok(12.0));
    /// assert_eq!(calculator.evaluate::<f64>("1/2x"), Ok(1.5));
    /// # }
    /// ```
    pub const fn implicit_multiplication(mut self, implicit_multiplication: bool) -> Self {
        self.syntax.implicit_multiplication = implicit_multiplication;
        self
    }

    /// Returns the limits and syntax settings.
    pub(crate) const fn syntax(&self) -> Syntax {
        self.syntax
    }

    /// Evaluates `input`.
    pub fn evaluate<N: Number>(&self, input: &str) -> Result<N, Error>
    where
        C: Context<N>,
    {
        let evaluation = Evaluation {
            context: self.context,
            skipping: false,
            failure: None,
            number: PhantomData,
        };
        let (value, evaluation) = Parser::<_, ARGUMENTS>::parse(input, self.syntax, evaluation)?;
        evaluation.failure.map_or(Ok(value), Err)
    }
}

/// Where an evaluation is in a `&&`, `||` or `? :`.
#[derive(Clone, Copy)]
struct Branch {
    /// Whether the left operand or condition is true, `None` when values are not computed.
    truth: Option<bool>,
    /// Whether values were not computed before the operation, to restore after a skipped operand.
    skipping: bool,
}

/// The [`Backend`] that computes each operation as soon as it is parsed.
///
/// Errors come in two sorts. Syntax errors, [`ErrorKind::TooDeep`], [`ErrorKind::TooManyArguments`] and
/// [`ErrorKind::InvalidNumber`] depend only on the text and stop the parsing at once, wherever they are. The others
/// depend on values or on the context: the first one is kept in [`Evaluation::failure`] and the rest of the input
/// is still parsed, without computing, so that a syntax error further on takes precedence.
struct Evaluation<'c, C: ?Sized, N> {
    /// Resolves variables and functions.
    context: &'c C,
    /// Whether values are not computed, in an operand that is not needed or after a [`Evaluation::failure`].
    skipping: bool,
    /// The first error that depends on values or on the context.
    failure: Option<Error>,
    /// The number type values are computed in.
    number: PhantomData<N>,
}

impl<N: Number, C: Context<N> + ?Sized> Evaluation<'_, C, N> {
    /// Records `error` as the [`Evaluation::failure`] and stops computing.
    #[cold]
    fn fail(&mut self, error: Error) -> N {
        self.failure = Some(error);
        self.skipping = true;
        N::default()
    }

    /// Returns the result of `operation`, or records its error at `span`.
    ///
    /// Returns a placeholder without running `operation` when values are not computed.
    fn compute(&mut self, span: Span, operation: impl FnOnce() -> Result<N, ErrorKind>) -> N {
        if self.skipping {
            return N::default();
        }
        match operation() {
            Ok(value) => value,
            Err(kind) => self.fail(Error::new(kind, span)),
        }
    }

    /// Returns whether `value` is true, that is not zero, or `None` when values are not computed or on error.
    fn truth(&mut self, value: N, span: Span) -> Option<bool> {
        if self.skipping {
            return None;
        }
        match value.is_zero() {
            Ok(zero) => Some(!zero),
            Err(kind) => {
                self.fail(Error::new(kind, span));
                None
            }
        }
    }

    /// Computes values again if they were computed before `branch`, unless an error happened since.
    fn restore(&mut self, branch: Branch) {
        self.skipping = branch.skipping || self.failure.is_some();
    }
}

impl<'i, N: Number, C: Context<N> + ?Sized> Backend<'i> for Evaluation<'_, C, N> {
    type Value = N;
    type Branch = Branch;

    fn number(&mut self, literal: &'i str, span: Span) -> Result<N, Error> {
        N::parse(literal).ok_or(Error::new(ErrorKind::InvalidNumber, span))
    }

    fn variable(&mut self, name: &'i str, span: Span) -> Result<N, Error> {
        if self.skipping {
            return Ok(N::default());
        }
        Ok(match self.context.variable(name) {
            Some(value) => value,
            None => self.fail(Error::new(ErrorKind::UnknownVariable, span)),
        })
    }

    fn call(
        &mut self,
        name: &'i str,
        name_span: Span,
        arguments: &[N],
        span: Span,
    ) -> Result<N, Error> {
        if self.skipping {
            return Ok(N::default());
        }
        Ok(match self.context.call(name, arguments) {
            Some(Ok(value)) => value,
            Some(Err(kind)) => self.fail(Error::new(kind, span)),
            None => self.fail(Error::new(ErrorKind::UnknownFunction, name_span)),
        })
    }

    fn unary(&mut self, operator: Unary, operand: N, span: Span) -> Result<N, Error> {
        Ok(self.compute(span, || operator.apply(operand)))
    }

    fn binary(&mut self, operator: Binary, left: N, right: N, span: Span) -> Result<N, Error> {
        Ok(self.compute(span, || operator.apply(left, right)))
    }

    fn logic(&mut self, or: bool, left: N, span: Span) -> Result<Branch, Error> {
        let branch = Branch {
            truth: self.truth(left, span),
            skipping: self.skipping,
        };
        if branch.truth == Some(or) {
            self.skipping = true;
        }
        Ok(branch)
    }

    fn logic_end(&mut self, or: bool, branch: Branch, right: N, span: Span) -> Result<N, Error> {
        let truth = if branch.truth == Some(or) {
            self.restore(branch);
            Some(or)
        } else {
            self.truth(right, span)
        };
        Ok(match truth {
            Some(truth) => self.compute(span, || N::from_bool(truth)),
            None => N::default(),
        })
    }

    fn condition(&mut self, condition: N, span: Span) -> Result<Branch, Error> {
        let branch = Branch {
            truth: self.truth(condition, span),
            skipping: self.skipping,
        };
        if branch.truth == Some(false) {
            self.skipping = true;
        }
        Ok(branch)
    }

    fn otherwise(&mut self, branch: Branch, _: Span) -> Result<Branch, Error> {
        self.restore(branch);
        if branch.truth == Some(true) {
            self.skipping = true;
        }
        Ok(branch)
    }

    fn conditional_end(&mut self, branch: Branch, then: N, otherwise: N) -> N {
        self.restore(branch);
        if branch.truth == Some(true) {
            then
        } else {
            otherwise
        }
    }
}

/// Evaluates `input`, resolving names with `context`.
///
/// ```
/// let context = [("price", 250), ("quantity", 4)];
/// assert_eq!(nanoexpr::evaluate_with("price * quantity - 100", &context), Ok(900));
/// ```
pub fn evaluate_with<N: Number, C: Context<N> + ?Sized>(
    input: &str,
    context: &C,
) -> Result<N, Error> {
    Evaluator::<C>::new(context).evaluate(input)
}
