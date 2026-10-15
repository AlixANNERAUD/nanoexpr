#![no_std]
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod context;
#[cfg(feature = "decimal")]
mod decimal;
mod error;
mod evaluator;
#[cfg(feature = "fraction")]
mod fraction;
mod lexer;
#[cfg(any(feature = "libm", feature = "fraction"))]
mod math;
mod number;
mod parser;
mod program;

pub use context::{Context, Functions, Variables};
#[cfg(feature = "decimal")]
pub use decimal::{Decimal, ParseDecimalError};
pub use error::{Error, ErrorKind, Span};
pub use evaluator::{Evaluator, PercentSign, evaluate_with};
#[cfg(feature = "fraction")]
pub use fraction::{Fraction, ParseFractionError};
#[cfg(any(feature = "libm", feature = "fraction"))]
pub use math::{Math, evaluate};
pub use number::Number;
pub use program::{Instruction, Program, compile};
