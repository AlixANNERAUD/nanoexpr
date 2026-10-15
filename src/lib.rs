#![no_std]
#![cfg_attr(feature = "libm", doc = include_str!("../README.md"))]
#![forbid(unsafe_code)]

mod context;
mod error;
mod evaluator;
mod lexer;
#[cfg(feature = "libm")]
mod math;
mod number;

pub use context::{Context, Functions, Variables};
pub use error::{Error, ErrorKind, Span};
pub use evaluator::{Evaluator, PercentSign, evaluate_with};
#[cfg(feature = "libm")]
pub use math::{Math, evaluate};
pub use number::Number;
