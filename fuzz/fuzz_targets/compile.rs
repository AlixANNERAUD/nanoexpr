#![no_main]

use std::fmt::Debug;

use libfuzzer_sys::fuzz_target;
use nanoexpr::{
    Context, Decimal, ErrorKind, Evaluator, Exact, Fraction, Functions, Instruction, Math, Number,
    PercentSign,
};

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

#[derive(Clone, Copy)]
struct Settings {
    max_depth: usize,
    percent_sign: PercentSign,
    implicit_multiplication: bool,
    capacity: usize,
}

fn evaluator<C: ?Sized, const ARGUMENTS: usize>(
    context: &C,
    settings: Settings,
) -> Evaluator<'_, C, ARGUMENTS> {
    Evaluator::<C, ARGUMENTS>::new(context)
        .max_depth(settings.max_depth)
        .percent_sign(settings.percent_sign)
        .implicit_multiplication(settings.implicit_multiplication)
}

fn same<N: Number + Debug, C: Context<N>, const ARGUMENTS: usize, const STACK: usize>(
    input: &str,
    settings: Settings,
    contexts: &[&C],
) {
    let mut code = vec![Instruction::<N>::default(); settings.capacity];
    let compiled =
        evaluator::<C, ARGUMENTS>(contexts[0], settings).compile::<N, STACK>(input, &mut code);
    match compiled {
        Ok(program) => {
            assert!(program.len() <= settings.capacity);
            assert!(program.stack() <= STACK);
            for context in contexts {
                let expected = evaluator::<C, ARGUMENTS>(context, settings).evaluate::<N>(input);
                let actual = program.evaluate(*context);
                assert_eq!(
                    format!("{actual:?}"),
                    format!("{expected:?}"),
                    "{input:?} compiled into {} instructions",
                    program.len()
                );
            }
        }
        Err(error) if error.kind == ErrorKind::TooLong => {}
        Err(error) => {
            for context in contexts {
                let expected = evaluator::<C, ARGUMENTS>(context, settings).evaluate::<N>(input);
                assert_eq!(
                    format!("{:?}", Err::<N, _>(error)),
                    format!("{expected:?}"),
                    "{input:?} does not compile"
                );
            }
        }
    }
}

fn check<N: Number + Debug, C: Context<N>>(input: &str, settings: Settings, contexts: &[&C]) {
    same::<N, C, 2, 4>(input, settings, contexts);
    same::<N, C, 8, 32>(input, settings, contexts);
}

fn typed<N: Number + Debug>(input: &str, settings: Settings, values: [N; 4])
where
    Math: Context<N>,
{
    let first = (
        [("x", values[0]), ("y", values[1])],
        (Functions(functions::<N>), Math),
    );
    let second = (
        [("x", values[2]), ("y", values[3])],
        (Functions(functions::<N>), Math),
    );
    check::<N, _>(input, settings, &[&first, &second]);
}

fn integer<N: Number + Debug>(input: &str, settings: Settings, values: [N; 4]) {
    let first = (
        [("x", values[0]), ("y", values[1])],
        Functions(functions::<N>),
    );
    let second = (
        [("x", values[2]), ("y", values[3])],
        Functions(functions::<N>),
    );
    check::<N, _>(input, settings, &[&first, &second]);
}

fn decimal<const D: u32, const E: i32>(input: &str, settings: Settings) {
    let value = |mantissa, exponent| Decimal::<D, E>::new(mantissa, exponent).unwrap_or_default();
    typed(
        input,
        settings,
        [
            value(-1234567, -3),
            value(0, 0),
            value(5, 0),
            value(i64::MAX, 0),
        ],
    );
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
        _ => 32,
    };
    let percent_sign = if flags & 0b100 == 0 {
        PercentSign::Remainder
    } else {
        PercentSign::Percentage
    };
    let capacity = match flags >> 4 {
        0 => 0,
        1 => 1,
        2 => 3,
        3 => 8,
        _ => 256,
    };
    let settings = Settings {
        max_depth,
        percent_sign,
        implicit_multiplication: flags & 0b1000 != 0,
        capacity,
    };
    integer::<i8>(input, settings, [-128, 127, 0, 2]);
    integer::<i64>(input, settings, [i64::MIN, i64::MAX, 0, -3]);
    integer::<u32>(input, settings, [u32::MAX, 7, 0, 1]);
    typed::<f32>(input, settings, [f32::MAX, f32::MIN_POSITIVE, 0.0, -2.5]);
    typed::<f64>(input, settings, [f64::INFINITY, f64::NAN, 0.0, 1e-310]);
    decimal::<14, 99>(input, settings);
    typed::<Fraction>(
        input,
        settings,
        [
            Fraction::from(-7_i64),
            Fraction::new(false, 1, 3).unwrap(),
            Fraction::ZERO,
            Fraction::from(u64::MAX),
        ],
    );
    typed::<Exact>(
        input,
        settings,
        [
            Exact::from(Fraction::new(true, 2, 3).unwrap()),
            Exact::from(Decimal::new(15, -1).unwrap()),
            Exact::default(),
            Exact::from(Decimal::new(i64::MAX, 80).unwrap()),
        ],
    );
    decimal::<3, 5>(input, settings);
    decimal::<18, 9999>(input, settings);
});
