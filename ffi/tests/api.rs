use core::ffi::{CStr, c_char, c_void};
use core::{ptr, slice};

use nanoexpr::ErrorKind;
use nanoexpr_c::{
    DEFAULT_MAX_DEPTH, Error, MAX_ARGUMENTS, Options, Status, nanoexpr_default_options,
    nanoexpr_evaluate, nanoexpr_evaluate_with, nanoexpr_status_description,
};

fn evaluate(input: &[u8]) -> Result<f64, Error> {
    with(input, None)
}

fn with(input: &[u8], options: Option<&Options>) -> Result<f64, Error> {
    let mut result = f64::NAN;
    let mut error = Error {
        kind: Status::UnknownError,
        start: usize::MAX,
        end: usize::MAX,
    };
    let status = unsafe {
        match options {
            Some(options) => nanoexpr_evaluate_with(
                input.as_ptr().cast(),
                input.len(),
                options,
                &mut result,
                &mut error,
            ),
            None => nanoexpr_evaluate(input.as_ptr().cast(), input.len(), &mut result, &mut error),
        }
    };
    assert_eq!(status, error.kind);
    match status {
        Status::Ok => {
            assert_eq!((error.start, error.end), (0, 0));
            Ok(result)
        }
        _ => {
            assert!(result.is_nan());
            Err(error)
        }
    }
}

fn error(kind: Status, start: usize, end: usize) -> Result<f64, Error> {
    Err(Error { kind, start, end })
}

struct Sensors {
    temperature: f64,
    calls: usize,
}

unsafe fn name<'a>(name: *const c_char, length: usize) -> &'a [u8] {
    unsafe { slice::from_raw_parts(name.cast(), length) }
}

unsafe extern "C" fn variable(
    user_data: *mut c_void,
    name_pointer: *const c_char,
    name_length: usize,
    value: *mut f64,
) -> bool {
    let sensors = unsafe { &mut *user_data.cast::<Sensors>() };
    match unsafe { name(name_pointer, name_length) } {
        b"temperature" => {
            unsafe { value.write(sensors.temperature) };
            true
        }
        b"pi" => {
            unsafe { value.write(3.0) };
            true
        }
        _ => false,
    }
}

unsafe extern "C" fn function(
    user_data: *mut c_void,
    name_pointer: *const c_char,
    name_length: usize,
    arguments: *const f64,
    argument_count: usize,
    result: *mut f64,
) -> Status {
    let sensors = unsafe { &mut *user_data.cast::<Sensors>() };
    sensors.calls += 1;
    let arguments = unsafe { slice::from_raw_parts(arguments, argument_count) };
    match (unsafe { name(name_pointer, name_length) }, arguments) {
        (b"celsius", [kelvin]) => {
            unsafe { result.write(kelvin - 273.15) };
            Status::Ok
        }
        (b"celsius", _) => Status::ArgumentCount,
        (b"fail", _) => Status::InvalidArgument,
        _ => Status::UnknownFunction,
    }
}

fn options(sensors: &mut Sensors) -> Options {
    Options {
        variable: Some(variable),
        function: Some(function),
        user_data: ptr::from_mut(sensors).cast(),
        ..nanoexpr_default_options()
    }
}

#[test]
fn evaluates() {
    assert_eq!(evaluate(b"2 * (3 + 4) ^ 2 / sqrt(16)"), Ok(24.5));
    assert_eq!(evaluate(b"7 % 4"), Ok(3.0));
    assert_eq!(evaluate(b"max(1, 5, 3)"), Ok(5.0));
}

#[test]
fn reports_errors_with_spans() {
    assert_eq!(
        evaluate(b"1 + 4 / (2 - 2)"),
        error(Status::DivisionByZero, 4, 15)
    );
    assert_eq!(evaluate(b"2 + * 3"), error(Status::UnexpectedToken, 4, 5));
    assert_eq!(
        evaluate(b"(1 + 2"),
        error(Status::UnclosedParenthesis, 0, 1)
    );
    assert_eq!(evaluate(b"x + 1"), error(Status::UnknownVariable, 0, 1));
    assert_eq!(evaluate(b"f(1)"), error(Status::UnknownFunction, 0, 1));
    assert_eq!(evaluate(b"sqrt(-1)"), error(Status::Domain, 0, 8));
    assert_eq!(evaluate(b"1 $ 2"), error(Status::UnexpectedCharacter, 2, 3));
    assert_eq!(evaluate(b""), error(Status::UnexpectedEnd, 0, 0));
}

#[test]
fn reads_only_the_given_length() {
    assert_eq!(with(&b"1 + 2 garbage"[..5], None), Ok(3.0));
}

#[test]
fn rejects_invalid_utf8() {
    assert_eq!(
        evaluate(b"1 + \xff + 2"),
        error(Status::UnexpectedCharacter, 4, 5)
    );
    assert_eq!(
        evaluate(b"1 + \xe2\x82"),
        error(Status::UnexpectedCharacter, 4, 6)
    );
}

#[test]
fn applies_options() {
    let percentage = Options {
        percentage: true,
        ..Options::default()
    };
    assert_eq!(with(b"200 * 15%", Some(&percentage)), Ok(30.0));

    let shallow = Options {
        max_depth: 2,
        ..Options::default()
    };
    assert_eq!(with(b"((1))", Some(&shallow)), error(Status::TooDeep, 2, 3));

    let plain = Options {
        math: false,
        ..Options::default()
    };
    assert_eq!(
        with(b"sqrt(4)", Some(&plain)),
        error(Status::UnknownFunction, 0, 4)
    );
    assert_eq!(
        with(b"pi", Some(&plain)),
        error(Status::UnknownVariable, 0, 2)
    );
}

#[test]
fn calls_callbacks() {
    let mut sensors = Sensors {
        temperature: 300.0,
        calls: 0,
    };
    let options = options(&mut sensors);
    assert_eq!(
        with(b"round(celsius(temperature))", Some(&options)),
        Ok(27.0)
    );
    assert_eq!(with(b"pi", Some(&options)), Ok(3.0));
    assert_eq!(with(b"sqrt(16) + floor(e)", Some(&options)), Ok(6.0));
    assert_eq!(
        with(b"1 + celsius(1, 2)", Some(&options)),
        error(Status::ArgumentCount, 4, 17)
    );
    assert_eq!(
        with(b"fail()", Some(&options)),
        error(Status::InvalidArgument, 0, 6)
    );
    assert_eq!(
        with(b"nothing()", Some(&options)),
        error(Status::UnknownFunction, 0, 7)
    );
    assert_eq!(sensors.calls, 7);
}

#[test]
fn rejects_null_pointers() {
    let input = b"1";
    let mut result = 0.0;
    let mut error = Error {
        kind: Status::Ok,
        start: 1,
        end: 1,
    };
    let invalid = Error {
        kind: Status::InvalidArgument,
        start: 0,
        end: 0,
    };
    unsafe {
        assert_eq!(
            nanoexpr_evaluate(ptr::null(), 0, &mut result, &mut error),
            Status::InvalidArgument
        );
        assert_eq!(error, invalid);
        assert_eq!(
            nanoexpr_evaluate(input.as_ptr().cast(), 1, ptr::null_mut(), ptr::null_mut()),
            Status::InvalidArgument
        );
        assert_eq!(
            nanoexpr_evaluate_with(
                input.as_ptr().cast(),
                1,
                ptr::null(),
                &mut result,
                ptr::null_mut()
            ),
            Status::InvalidArgument
        );
        assert_eq!(
            nanoexpr_evaluate(input.as_ptr().cast(), 1, &mut result, ptr::null_mut()),
            Status::Ok
        );
    }
    assert_eq!(result, 1.0);
}

#[test]
fn defaults() {
    let options = nanoexpr_default_options();
    assert!(options.variable.is_none());
    assert!(options.function.is_none());
    assert!(options.user_data.is_null());
    assert!(options.math);
    assert!(!options.percentage);
    assert_eq!(options.max_depth, DEFAULT_MAX_DEPTH);
}

#[test]
fn describes_statuses() {
    let description = |status| unsafe { CStr::from_ptr(nanoexpr_status_description(status)) };
    assert_eq!(description(Status::Ok), c"success");
    assert_eq!(description(Status::DivisionByZero), c"division by zero");
    assert_eq!(description(Status::InvalidArgument), c"invalid argument");
    for kind in [
        ErrorKind::UnexpectedCharacter,
        ErrorKind::UnexpectedToken,
        ErrorKind::UnexpectedEnd,
        ErrorKind::UnclosedParenthesis,
        ErrorKind::InvalidNumber,
        ErrorKind::UnknownVariable,
        ErrorKind::UnknownFunction,
        ErrorKind::TooManyArguments,
        ErrorKind::ArgumentCount,
        ErrorKind::DivisionByZero,
        ErrorKind::Domain,
        ErrorKind::Overflow,
        ErrorKind::TooDeep,
    ] {
        assert_eq!(
            description(Status::from(kind)).to_str(),
            Ok(kind.description())
        );
    }
    assert_eq!(Status::TooDeep as i32, 13);
    assert_eq!(Status::UnknownError as i32, 15);
}

#[test]
fn header_matches() {
    let header = include_str!("../include/nanoexpr.h");
    let statuses = [
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
    let declared: Vec<&str> = header
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("NANOEXPR_") && !line.starts_with("NANOEXPR_H"))
        .map(|line| line.split([' ', ',']).next().unwrap())
        .collect();
    let expected: Vec<String> = statuses
        .iter()
        .enumerate()
        .map(|(index, status)| {
            assert_eq!(*status as usize, index);
            let mut name = String::from("NANOEXPR");
            for character in format!("{status:?}").chars() {
                if character.is_uppercase() {
                    name.push('_');
                }
                name.push(character.to_ascii_uppercase());
            }
            name
        })
        .collect();
    assert_eq!(declared, expected);
    assert!(header.contains(&format!("#define NANOEXPR_MAX_ARGUMENTS {MAX_ARGUMENTS}\n")));
    assert!(header.contains(&format!(
        "#define NANOEXPR_DEFAULT_MAX_DEPTH {DEFAULT_MAX_DEPTH}\n"
    )));
}
