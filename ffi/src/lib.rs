//! C bindings of [`nanoexpr`], evaluating expressions in `double` without allocating.
//!
//! The functions are declared for C and C++ in `include/nanoexpr.h`. Build this crate as a static library
//! (`libnanoexpr_c.a`) or a shared one, and link it to your program.
//!
//! The `std` feature, on by default, links the standard library for its panic handler. Without it the crate is
//! `no_std` and provides a panic handler that loops forever: build it for embedded targets with
//! `--no-default-features` and `panic = "abort"`. Evaluation does not panic on any input.

#![no_std]
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

#[cfg(feature = "std")]
extern crate std;

use core::cell::Cell;
use core::ffi::{CStr, c_char, c_void};
use core::{slice, str};

use nanoexpr::{Context, ErrorKind, Evaluator, Math, PercentSign, Span};

/// The most arguments a function call can take.
pub const MAX_ARGUMENTS: usize = 8;

/// Default limit of nested parentheses, calls and operators.
pub const DEFAULT_MAX_DEPTH: usize = Evaluator::<(), MAX_ARGUMENTS>::DEFAULT_MAX_DEPTH;

/// Outcome of an evaluation: `nanoexpr_status` in C.
///
/// Every [`ErrorKind`] has a status of the same name, in the same order.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// Success.
    Ok = 0,
    /// [`ErrorKind::UnexpectedCharacter`], also returned for input that is not valid UTF-8.
    UnexpectedCharacter,
    /// [`ErrorKind::UnexpectedToken`].
    UnexpectedToken,
    /// [`ErrorKind::UnexpectedEnd`].
    UnexpectedEnd,
    /// [`ErrorKind::UnclosedParenthesis`].
    UnclosedParenthesis,
    /// [`ErrorKind::InvalidNumber`].
    InvalidNumber,
    /// [`ErrorKind::UnknownVariable`].
    UnknownVariable,
    /// [`ErrorKind::UnknownFunction`].
    UnknownFunction,
    /// [`ErrorKind::TooManyArguments`].
    TooManyArguments,
    /// [`ErrorKind::ArgumentCount`].
    ArgumentCount,
    /// [`ErrorKind::DivisionByZero`].
    DivisionByZero,
    /// [`ErrorKind::Domain`].
    Domain,
    /// [`ErrorKind::Overflow`].
    Overflow,
    /// [`ErrorKind::TooDeep`].
    TooDeep,
    /// A pointer that must not be null is null.
    InvalidArgument,
    /// An error this version of the bindings does not know.
    UnknownError,
}

impl Status {
    /// Short English description of the status, as a C string.
    pub const fn description(self) -> &'static CStr {
        match self {
            Self::Ok => c"success",
            Self::UnexpectedCharacter => c"unexpected character",
            Self::UnexpectedToken => c"unexpected token",
            Self::UnexpectedEnd => c"unexpected end of expression",
            Self::UnclosedParenthesis => c"unclosed parenthesis",
            Self::InvalidNumber => c"invalid number",
            Self::UnknownVariable => c"unknown variable",
            Self::UnknownFunction => c"unknown function",
            Self::TooManyArguments => c"too many arguments",
            Self::ArgumentCount => c"wrong number of arguments",
            Self::DivisionByZero => c"division by zero",
            Self::Domain => c"operand out of domain",
            Self::Overflow => c"overflow",
            Self::TooDeep => c"expression nested too deeply",
            Self::InvalidArgument => c"invalid argument",
            Self::UnknownError => c"unknown error",
        }
    }
}

impl From<ErrorKind> for Status {
    fn from(kind: ErrorKind) -> Self {
        match kind {
            ErrorKind::UnexpectedCharacter => Self::UnexpectedCharacter,
            ErrorKind::UnexpectedToken => Self::UnexpectedToken,
            ErrorKind::UnexpectedEnd => Self::UnexpectedEnd,
            ErrorKind::UnclosedParenthesis => Self::UnclosedParenthesis,
            ErrorKind::InvalidNumber => Self::InvalidNumber,
            ErrorKind::UnknownVariable => Self::UnknownVariable,
            ErrorKind::UnknownFunction => Self::UnknownFunction,
            ErrorKind::TooManyArguments => Self::TooManyArguments,
            ErrorKind::ArgumentCount => Self::ArgumentCount,
            ErrorKind::DivisionByZero => Self::DivisionByZero,
            ErrorKind::Domain => Self::Domain,
            ErrorKind::Overflow => Self::Overflow,
            ErrorKind::TooDeep => Self::TooDeep,
            _ => Self::UnknownError,
        }
    }
}

/// An evaluation error and the byte range of the input that caused it: `nanoexpr_error` in C.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Error {
    /// What went wrong, [`Status::Ok`] on success.
    pub kind: Status,
    /// Offset of the first byte.
    pub start: usize,
    /// Offset one past the last byte.
    pub end: usize,
}

impl Error {
    /// Creates an error of `kind` at `span`.
    const fn new(kind: Status, span: Span) -> Self {
        Self {
            kind,
            start: span.start,
            end: span.end,
        }
    }
}

/// Resolves a variable: `nanoexpr_variable_callback` in C.
///
/// Receives the user data, the name (not NUL-terminated) and its length in bytes. Writes the value and returns
/// `true` if the variable exists, returns `false` otherwise.
pub type VariableCallback = unsafe extern "C" fn(
    user_data: *mut c_void,
    name: *const c_char,
    name_length: usize,
    value: *mut f64,
) -> bool;

/// Calls a function: `nanoexpr_function_callback` in C.
///
/// Receives the user data, the name (not NUL-terminated) and its length in bytes, and the arguments. Writes the
/// result and returns [`Status::Ok`], returns [`Status::UnknownFunction`] to let the built-in functions try, or
/// returns another status to fail the evaluation with it.
pub type FunctionCallback = unsafe extern "C" fn(
    user_data: *mut c_void,
    name: *const c_char,
    name_length: usize,
    arguments: *const f64,
    argument_count: usize,
    result: *mut f64,
) -> Status;

/// How to evaluate an expression: `nanoexpr_options` in C.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Resolves variables, before the built-in constants. May be null.
    pub variable: Option<VariableCallback>,
    /// Calls functions, before the built-in functions. May be null.
    pub function: Option<FunctionCallback>,
    /// Passed to both callbacks.
    pub user_data: *mut c_void,
    /// Whether the constants and functions of [`Math`] are available.
    pub math: bool,
    /// Whether `%` is a percentage, as in `200 * 15%`, rather than the remainder.
    pub percentage: bool,
    /// Most levels of nested parentheses, calls and operators.
    pub max_depth: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            variable: None,
            function: None,
            user_data: core::ptr::null_mut(),
            math: true,
            percentage: false,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }
}

/// A [`Context`] that asks the callbacks of [`Options`].
struct Callbacks<'o> {
    /// The callbacks and their user data.
    options: &'o Options,
    /// The status a function callback failed with, which [`ErrorKind`] may not be able to express.
    failure: Cell<Option<Status>>,
}

impl Context<f64> for Callbacks<'_> {
    fn variable(&self, name: &str) -> Option<f64> {
        let callback = self.options.variable?;
        let mut value = 0.0;
        let found = unsafe {
            callback(
                self.options.user_data,
                name.as_ptr().cast(),
                name.len(),
                &mut value,
            )
        };
        found.then_some(value)
    }

    fn call(&self, name: &str, arguments: &[f64]) -> Option<Result<f64, ErrorKind>> {
        let callback = self.options.function?;
        let mut result = 0.0;
        let status = unsafe {
            callback(
                self.options.user_data,
                name.as_ptr().cast(),
                name.len(),
                arguments.as_ptr(),
                arguments.len(),
                &mut result,
            )
        };
        match status {
            Status::Ok => Some(Ok(result)),
            Status::UnknownFunction => None,
            status => {
                self.failure.set(Some(status));
                Some(Err(ErrorKind::Domain))
            }
        }
    }
}

/// Evaluates `input` with `context` and the settings of `options`.
fn run<C: Context<f64>>(
    input: &str,
    context: &C,
    options: &Options,
) -> Result<f64, nanoexpr::Error> {
    Evaluator::<C, MAX_ARGUMENTS>::new(context)
        .max_depth(options.max_depth)
        .percent_sign(if options.percentage {
            PercentSign::Percentage
        } else {
            PercentSign::Remainder
        })
        .evaluate(input)
}

/// Evaluates `input` with `options`, reporting failures as [`Error`].
fn evaluate(input: &[u8], options: &Options) -> Result<f64, Error> {
    let input = str::from_utf8(input).map_err(|error| {
        let start = error.valid_up_to();
        let end = error
            .error_len()
            .map_or(input.len(), |length| start + length);
        Error::new(Status::UnexpectedCharacter, Span::new(start, end))
    })?;
    let callbacks = Callbacks {
        options,
        failure: Cell::new(None),
    };
    let outcome = if options.math {
        run(input, &(&callbacks, Math), options)
    } else {
        run(input, &callbacks, options)
    };
    outcome.map_err(|error| {
        let kind = callbacks.failure.get().unwrap_or(error.kind.into());
        Error::new(kind, error.span)
    })
}

/// Checks the pointers, evaluates and writes the outcome.
///
/// # Safety
///
/// See [`nanoexpr_evaluate_with`].
unsafe fn evaluate_into(
    input: *const c_char,
    length: usize,
    options: Option<&Options>,
    result: *mut f64,
    error: *mut Error,
) -> Status {
    let outcome = match options {
        Some(options) if !input.is_null() && !result.is_null() => evaluate(
            unsafe { slice::from_raw_parts(input.cast(), length) },
            options,
        ),
        _ => Err(Error::new(Status::InvalidArgument, Span::new(0, 0))),
    };
    let report = match outcome {
        Ok(value) => {
            unsafe { result.write(value) };
            Error::new(Status::Ok, Span::new(0, 0))
        }
        Err(report) => report,
    };
    if !error.is_null() {
        unsafe { error.write(report) };
    }
    report.kind
}

/// Returns the default options: no callbacks, built-in math functions, `%` as the remainder and a depth limit of
/// [`DEFAULT_MAX_DEPTH`].
#[unsafe(no_mangle)]
pub extern "C" fn nanoexpr_default_options() -> Options {
    Options::default()
}

/// Evaluates the `length` bytes at `input` with the default options.
///
/// On success, writes the value to `result` and returns [`Status::Ok`]. On failure, leaves `result` untouched and
/// returns the error. In both cases, writes the status and the span of the error to `error` unless it is null.
///
/// # Safety
///
/// `input` must point to `length` readable bytes, `result` to a writable `double`, and `error` to a writable
/// `nanoexpr_error` or be null. A null `input` or `result` returns [`Status::InvalidArgument`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nanoexpr_evaluate(
    input: *const c_char,
    length: usize,
    result: *mut f64,
    error: *mut Error,
) -> Status {
    unsafe { evaluate_into(input, length, Some(&Options::default()), result, error) }
}

/// Evaluates the `length` bytes at `input` with `options`.
///
/// Behaves as [`nanoexpr_evaluate`], resolving names with the callbacks of `options` first, then with the
/// built-in constants and functions if `options.math` is set.
///
/// # Safety
///
/// Besides the requirements of [`nanoexpr_evaluate`], `options` must point to a valid `nanoexpr_options` or be
/// null, which returns [`Status::InvalidArgument`]. Its callbacks must be safe to call with its `user_data`, must
/// return a valid `nanoexpr_status`, and must not unwind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nanoexpr_evaluate_with(
    input: *const c_char,
    length: usize,
    options: *const Options,
    result: *mut f64,
    error: *mut Error,
) -> Status {
    unsafe { evaluate_into(input, length, options.as_ref(), result, error) }
}

/// Returns a short English description of `status`, as a static NUL-terminated string.
#[unsafe(no_mangle)]
pub extern "C" fn nanoexpr_status_description(status: Status) -> *const c_char {
    status.description().as_ptr()
}

/// Loops forever, as there is nowhere to report a panic without the standard library.
#[cfg(not(any(test, feature = "std")))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
