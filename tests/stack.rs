#![cfg(feature = "libm")]

use std::thread;

use nanoexpr::{Context, ErrorKind, Evaluator, Math, Number};

const STACK: usize = if cfg!(debug_assertions) {
    256 * 1024
} else {
    64 * 1024
};

fn nested(depth: usize) -> [String; 11] {
    [
        "(".repeat(depth) + "pi" + &")".repeat(depth),
        "-".repeat(depth) + "pi",
        "+".repeat(depth) + "pi",
        "2^".repeat(depth) + "pi",
        "1+(".repeat(depth) + "pi" + &")".repeat(depth),
        "max(".repeat(depth) + "pi" + &")".repeat(depth),
        "max(1,".repeat(depth) + "pi" + &")".repeat(depth),
        "abs(-(".repeat(depth) + "pi" + &"))".repeat(depth),
        "!".repeat(depth) + "pi",
        "1?".repeat(depth) + "pi" + &":0".repeat(depth),
        "1&&(".repeat(depth) + "pi" + &")".repeat(depth),
    ]
}

fn evaluate_deepest<N: Number + std::fmt::Debug + Send + 'static>()
where
    Math: Context<N>,
{
    thread::Builder::new()
        .stack_size(STACK)
        .spawn(|| {
            let evaluator = Evaluator::<Math>::new(&Math);
            for depth in 0..=Evaluator::<Math>::DEFAULT_MAX_DEPTH + 1 {
                for input in nested(depth) {
                    let _ = evaluator.evaluate::<N>(&input);
                }
            }
            for input in nested(10_000) {
                assert_eq!(
                    evaluator.evaluate::<N>(&input).unwrap_err().kind,
                    ErrorKind::TooDeep
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn default_depth_fits_a_small_stack_in_f64() {
    evaluate_deepest::<f64>();
}

#[cfg(feature = "decimal")]
#[test]
fn default_depth_fits_a_small_stack_in_decimal() {
    evaluate_deepest::<nanoexpr::Decimal>();
    evaluate_deepest::<nanoexpr::Decimal<18, 9999>>();
}

#[cfg(feature = "fraction")]
#[test]
fn default_depth_fits_a_small_stack_in_fraction() {
    evaluate_deepest::<nanoexpr::Fraction>();
}

#[cfg(feature = "exact")]
#[test]
fn default_depth_fits_a_small_stack_in_exact() {
    evaluate_deepest::<nanoexpr::Exact>();
}
