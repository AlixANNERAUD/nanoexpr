#![no_std]
#![forbid(unsafe_code)]

mod context;
mod error;
mod evaluator;
mod lexer;
mod number;

pub use context::{Context, Functions, Variables};
pub use error::{Error, ErrorKind, Span};
pub use evaluator::{Evaluator, PercentSign, evaluate_with};
pub use number::Number;
