#![no_main]

use core::ffi::{c_char, c_void};
use core::ptr;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use nanoexpr_c::{
    Error, Options, Status, nanoexpr_default_options, nanoexpr_evaluate, nanoexpr_evaluate_with,
    nanoexpr_status_description,
};

const STATUSES: [Status; 16] = [
    Status::Ok,
    Status::UnexpectedCharacter,
    Status::UnexpectedToken,
    Status::UnexpectedEnd,
    Status::UnclosedParenthesis,
    Status::InvalidNumber,
    Status::UnknownVariable,
    Status::UnknownFunction,
    Status::TooManyArguments,
    Status::ArgumentCount,
    Status::DivisionByZero,
    Status::Domain,
    Status::Overflow,
    Status::TooDeep,
    Status::InvalidArgument,
    Status::UnknownError,
];

#[derive(Arbitrary, Debug)]
struct Input {
    bytes: Vec<u8>,
    null_input: bool,
    null_result: bool,
    null_error: bool,
    null_options: bool,
    variable: bool,
    function: bool,
    math: bool,
    percentage: bool,
    implicit_multiplication: bool,
    max_depth: u8,
    status: u8,
    value: f64,
}

struct UserData {
    status: Status,
    value: f64,
}

unsafe extern "C" fn variable(
    user_data: *mut c_void,
    name: *const c_char,
    name_length: usize,
    value: *mut f64,
) -> bool {
    let user_data = unsafe { &*user_data.cast::<UserData>() };
    let name = unsafe { core::slice::from_raw_parts(name.cast::<u8>(), name_length) };
    assert!(core::str::from_utf8(name).is_ok());
    if name.first() == Some(&b'v') {
        unsafe { value.write(user_data.value) };
        true
    } else {
        false
    }
}

unsafe extern "C" fn function(
    user_data: *mut c_void,
    name: *const c_char,
    name_length: usize,
    arguments: *const f64,
    argument_count: usize,
    result: *mut f64,
) -> Status {
    let user_data = unsafe { &*user_data.cast::<UserData>() };
    let _ = unsafe { core::slice::from_raw_parts(name.cast::<u8>(), name_length) };
    assert!(argument_count <= nanoexpr_c::MAX_ARGUMENTS);
    let arguments = unsafe { core::slice::from_raw_parts(arguments, argument_count) };
    unsafe { result.write(arguments.iter().sum::<f64>() + user_data.value) };
    user_data.status
}

fuzz_target!(|input: Input| {
    for status in STATUSES {
        assert!(!nanoexpr_status_description(status).is_null());
    }
    let mut user_data = UserData {
        status: STATUSES[usize::from(input.status) % STATUSES.len()],
        value: input.value,
    };
    let mut options = nanoexpr_default_options();
    options = Options {
        variable: input.variable.then_some(variable as _),
        function: input.function.then_some(function as _),
        user_data: (&raw mut user_data).cast(),
        math: input.math,
        percentage: input.percentage,
        implicit_multiplication: input.implicit_multiplication,
        max_depth: if input.max_depth == u8::MAX {
            options.max_depth
        } else {
            usize::from(input.max_depth)
        },
    };
    let text: *const c_char = if input.null_input {
        ptr::null()
    } else {
        input.bytes.as_ptr().cast()
    };
    let mut result = f64::NAN;
    let mut error = Error {
        kind: Status::UnknownError,
        start: usize::MAX,
        end: usize::MAX,
    };
    let result_pointer = if input.null_result {
        ptr::null_mut()
    } else {
        &raw mut result
    };
    let error_pointer = if input.null_error {
        ptr::null_mut()
    } else {
        &raw mut error
    };
    let options_pointer = if input.null_options {
        ptr::null()
    } else {
        &raw const options
    };
    let plain =
        unsafe { nanoexpr_evaluate(text, input.bytes.len(), result_pointer, error_pointer) };
    check(&input, plain, &error, false);
    let with = unsafe {
        nanoexpr_evaluate_with(
            text,
            input.bytes.len(),
            options_pointer,
            result_pointer,
            error_pointer,
        )
    };
    check(&input, with, &error, input.null_options);
});

fn check(input: &Input, status: Status, error: &Error, null_options: bool) {
    if !input.null_error {
        assert_eq!(error.kind, status);
        assert!(error.start <= error.end, "{error:?}");
        assert!(error.end <= input.bytes.len(), "{error:?}");
    }
    if input.null_input || input.null_result || null_options {
        assert_eq!(status, Status::InvalidArgument);
    }
}
