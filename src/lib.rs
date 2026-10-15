#![no_std]
#![forbid(unsafe_code)]

mod error;
mod number;

pub use error::{Error, ErrorKind, Span};
pub use number::Number;
