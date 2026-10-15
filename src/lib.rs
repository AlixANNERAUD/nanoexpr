#![no_std]
#![forbid(unsafe_code)]

mod context;
mod error;
mod lexer;
mod number;

pub use context::{Context, Functions, Variables};
pub use error::{Error, ErrorKind, Span};
pub use number::Number;
