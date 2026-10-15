#![no_std]
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod context;
#[cfg(feature = "decimal")]
mod decimal;
mod error;
mod evaluator;
mod lexer;
#[cfg(feature = "libm")]
mod math;
mod number;

pub use context::{Context, Functions, Variables};
#[cfg(feature = "decimal")]
pub use decimal::{Decimal, ParseDecimalError};
pub use error::{Error, ErrorKind, Span};
pub use evaluator::{Evaluator, PercentSign, evaluate_with};
#[cfg(feature = "libm")]
pub use math::{Math, evaluate};
pub use number::Number;
