#![no_main]

use std::fmt::Debug;

use libfuzzer_sys::fuzz_target;
use nanoexpr::{
    Context, Decimal, ErrorKind, Evaluator, Exact, Fraction, Functions, Math, Number, PercentSign,
    Span,
};

struct Settings {
    max_depth: usize,
    percent_sign: PercentSign,
    implicit_multiplication: bool,
}

fn functions<N: Number>(name: &str, arguments: &[N]) -> Option<Result<N, ErrorKind>> {
    match (name, arguments) {
        ("sum", _) => Some(
            arguments
                .iter()
                .try_fold(N::default(), |total, value| total.add(*value)),
        ),
        ("id", &[x]) => Some(Ok(x)),
        ("id", _) => Some(Err(ErrorKind::ArgumentCount)),
        ("fail", _) => Some(Err(ErrorKind::Domain)),
        _ => None,
    }
}

fn run<N: Number, C: Context<N>, const ARGUMENTS: usize>(
    input: &str,
    context: &C,
    settings: &Settings,
) -> Result<N, nanoexpr::Error> {
    let result = Evaluator::<C, ARGUMENTS>::new(context)
        .max_depth(settings.max_depth)
        .percent_sign(settings.percent_sign)
        .implicit_multiplication(settings.implicit_multiplication)
        .evaluate::<N>(input);
    if let Err(error) = result {
        let Span { start, end } = error.span;
        assert!(start <= end && end <= input.len(), "{error} in {input:?}");
        assert!(
            input.is_char_boundary(start) && input.is_char_boundary(end),
            "{error} in {input:?}"
        );
    }
    result
}

fn finite<N: Number + Into<f64>>(input: &str, settings: &Settings)
where
    Math: Context<N>,
{
    let result = Evaluator::<Math, 2>::new(&Math)
        .max_depth(settings.max_depth)
        .percent_sign(settings.percent_sign)
        .implicit_multiplication(settings.implicit_multiplication)
        .evaluate::<N>(input);
    if let Ok(value) = result {
        assert!(value.into().is_finite(), "{input:?}");
    }
}

fn every_arity<N: Number + Debug, C: Context<N>>(input: &str, context: &C, settings: &Settings) {
    let results = [
        run::<N, C, 0>(input, context, settings),
        run::<N, C, 1>(input, context, settings),
        run::<N, C, 2>(input, context, settings),
        run::<N, C, 8>(input, context, settings),
    ];
    for (index, result) in results.iter().enumerate() {
        if let Ok(value) = result {
            for larger in &results[index..] {
                assert_eq!(
                    format!("{larger:?}"),
                    format!("{:?}", Ok::<_, ()>(value)),
                    "{input:?}"
                );
            }
        }
    }
}

fn integer<N: Number + Debug>(input: &str, settings: &Settings, x: N, y: N) {
    let variables = [("x", x), ("y", y), ("zero", N::default())];
    every_arity::<N, _>(input, &(variables, Functions(functions::<N>)), settings);
    every_arity::<N, _>(input, &(), settings);
}

fn with_math<N: Number + Debug>(input: &str, settings: &Settings, x: N, y: N)
where
    Math: Context<N>,
{
    let variables = [("x", x), ("y", y), ("zero", N::default())];
    every_arity::<N, _>(
        input,
        &(variables, (Functions(functions::<N>), Math)),
        settings,
    );
    every_arity::<N, _>(input, &Math, settings);
    let _ = nanoexpr::evaluate::<N>(input);
}

fn decimal<const DIGITS: u32, const MAX_EXPONENT: i32>(input: &str, settings: &Settings) {
    let x = Decimal::<DIGITS, MAX_EXPONENT>::new(-1234567, -3).unwrap_or_default();
    let y = Decimal::<DIGITS, MAX_EXPONENT>::new(i64::MAX, 0).unwrap_or_default();
    with_math(input, settings, x, y);
}

fuzz_target!(|data: &[u8]| {
    let Some((&flags, input)) = data.split_first() else {
        return;
    };
    let Ok(input) = core::str::from_utf8(input) else {
        return;
    };
    let max_depth = match flags & 0b11 {
        0 => 0,
        1 => 1,
        2 => 4,
        _ => Evaluator::<(), 8>::DEFAULT_MAX_DEPTH,
    };
    let percent_sign = if flags & 0b100 == 0 {
        PercentSign::Remainder
    } else {
        PercentSign::Percentage
    };
    let settings = Settings {
        max_depth,
        percent_sign,
        implicit_multiplication: flags & 0b1000 != 0,
    };
    integer::<i8>(input, &settings, -128, 127);
    integer::<i64>(input, &settings, i64::MIN, i64::MAX);
    integer::<u32>(input, &settings, u32::MAX, 7);
    integer::<i128>(input, &settings, i128::MIN, 3);
    integer::<u8>(input, &settings, 255, 0);
    with_math::<f32>(input, &settings, f32::MAX, f32::MIN_POSITIVE);
    with_math::<f64>(input, &settings, f64::INFINITY, f64::NAN);
    with_math::<f64>(input, &settings, -0.0, 1e-310);
    finite::<f32>(input, &settings);
    finite::<f64>(input, &settings);
    decimal::<14, 99>(input, &settings);
    with_math::<Fraction>(
        input,
        &settings,
        Fraction::from(-7_i64),
        Fraction::new(false, 1, 3).unwrap(),
    );
    with_math::<Exact>(
        input,
        &settings,
        Exact::from(Fraction::new(true, 2, 3).unwrap()),
        Exact::from(Decimal::new(15, -1).unwrap()),
    );
    with_math::<Exact<3, 19>>(
        input,
        &settings,
        Exact::from(Fraction::from(u64::MAX)),
        Exact::from(Decimal::new(-1, -19).unwrap()),
    );
    decimal::<3, 5>(input, &settings);
    decimal::<1, 1>(input, &settings);
    decimal::<18, 9999>(input, &settings);
});
