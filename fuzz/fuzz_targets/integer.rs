#![no_main]

use std::fmt::{Display, Write};

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use nanoexpr::{ErrorKind, Evaluator, Number, PercentSign};

#[derive(Arbitrary, Debug, Clone, Copy)]
enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Modulo,
    Caret,
    DoubleStar,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    And,
    Or,
}

#[derive(Debug)]
enum Tree {
    Literal(u128),
    Negate(Box<Tree>),
    Plus(Box<Tree>),
    Not(Box<Tree>),
    Factorial(Box<Tree>),
    Binary(Operator, Box<Tree>, Box<Tree>),
    Conditional(Box<Tree>, Box<Tree>, Box<Tree>),
}

fn tree(input: &mut Unstructured<'_>, depth: u32) -> arbitrary::Result<Tree> {
    let choice = if depth == 0 {
        0
    } else {
        input.int_in_range(0..=4)?
    };
    Ok(match choice {
        0 => Tree::Literal(input.arbitrary()?),
        1 => Tree::Negate(Box::new(tree(input, depth - 1)?)),
        2 => Tree::Plus(Box::new(tree(input, depth - 1)?)),
        3 => Tree::Factorial(Box::new(tree(input, depth - 1)?)),
        _ => Tree::Binary(
            input.arbitrary()?,
            Box::new(tree(input, depth - 1)?),
            Box::new(tree(input, depth - 1)?),
        ),
    })
}

trait Integer: Number + Display + PartialOrd + std::fmt::Debug + TryFrom<u128> {
    const MAX: Self;
    fn checked_add(self, rhs: Self) -> Option<Self>;
    fn checked_sub(self, rhs: Self) -> Option<Self>;
    fn checked_mul(self, rhs: Self) -> Option<Self>;
    fn checked_div(self, rhs: Self) -> Option<Self>;
    fn checked_rem(self, rhs: Self) -> Option<Self>;
    fn checked_pow(self, rhs: u32) -> Option<Self>;
    fn zero(self) -> bool;
    fn is_negative(self) -> bool;
    fn exponent(self) -> Option<u32>;
}

macro_rules! integer {
    ($($type:ty),*) => {$(
        impl Integer for $type {
            const MAX: Self = <$type>::MAX;
            fn checked_add(self, rhs: Self) -> Option<Self> { <$type>::checked_add(self, rhs) }
            fn checked_sub(self, rhs: Self) -> Option<Self> { <$type>::checked_sub(self, rhs) }
            fn checked_mul(self, rhs: Self) -> Option<Self> { <$type>::checked_mul(self, rhs) }
            fn checked_div(self, rhs: Self) -> Option<Self> { <$type>::checked_div(self, rhs) }
            fn checked_rem(self, rhs: Self) -> Option<Self> { <$type>::checked_rem(self, rhs) }
            fn checked_pow(self, rhs: u32) -> Option<Self> { <$type>::checked_pow(self, rhs) }
            fn zero(self) -> bool { self == 0 }
            #[allow(unused_comparisons)]
            fn is_negative(self) -> bool { self < 0 }
            fn exponent(self) -> Option<u32> { u32::try_from(self).ok() }
        }
    )*};
}

integer!(i8, u8, i16, i32, u32, i64, u64, i128, u128, isize, usize);

fn literal<N: Integer>(value: u128) -> N {
    N::try_from(value)
        .or_else(|_| N::try_from(value % 100))
        .ok()
        .unwrap_or(N::MAX)
}

fn render<N: Integer>(tree: &Tree, text: &mut String, spaced: bool) {
    let space = if spaced { " " } else { "" };
    match tree {
        Tree::Literal(value) => write!(text, "{}", literal::<N>(*value)).unwrap(),
        Tree::Negate(operand) => {
            text.push_str("(-");
            render::<N>(operand, text, spaced);
            text.push(')');
        }
        Tree::Plus(operand) => {
            text.push_str("(+");
            render::<N>(operand, text, spaced);
            text.push(')');
        }
        Tree::Not(operand) => {
            text.push_str("(!");
            render::<N>(operand, text, spaced);
            text.push(')');
        }
        Tree::Factorial(operand) => {
            text.push('(');
            render::<N>(operand, text, spaced);
            write!(text, "{space}!)").unwrap();
        }
        Tree::Conditional(condition, then, otherwise) => {
            text.push('(');
            render::<N>(condition, text, spaced);
            write!(text, "{space}?{space}").unwrap();
            render::<N>(then, text, spaced);
            write!(text, "{space}:{space}").unwrap();
            render::<N>(otherwise, text, spaced);
            text.push(')');
        }
        Tree::Binary(operator, left, right) => {
            text.push('(');
            render::<N>(left, text, spaced);
            let symbol = match operator {
                Operator::Add => "+",
                Operator::Subtract => "-",
                Operator::Multiply => "*",
                Operator::Divide => "/",
                Operator::Remainder => "%",
                Operator::Modulo => " mod ",
                Operator::Caret => "^",
                Operator::DoubleStar => "**",
                Operator::Less => "<",
                Operator::LessEqual => "<=",
                Operator::Greater => ">",
                Operator::GreaterEqual => ">=",
                Operator::Equal => "==",
                Operator::NotEqual => "!=",
                Operator::And => "&&",
                Operator::Or => "||",
            };
            write!(text, "{space}{symbol}{space}").unwrap();
            render::<N>(right, text, spaced);
            text.push(')');
        }
    }
}

fn boolean<N: Integer>(value: bool) -> N {
    literal(u128::from(value))
}

fn compute<N: Integer>(tree: &Tree) -> Result<N, ErrorKind> {
    match tree {
        Tree::Literal(value) => Ok(literal(*value)),
        Tree::Negate(operand) => {
            let value = compute::<N>(operand)?;
            N::default().checked_sub(value).ok_or(ErrorKind::Overflow)
        }
        Tree::Plus(operand) => compute(operand),
        Tree::Not(operand) => Ok(boolean(compute::<N>(operand)?.zero())),
        Tree::Factorial(operand) => {
            let value = compute::<N>(operand)?;
            if value.is_negative() {
                return Err(ErrorKind::Domain);
            }
            let mut product = boolean::<N>(true);
            let mut factor = boolean::<N>(false);
            while factor < value {
                factor = factor
                    .checked_add(boolean(true))
                    .ok_or(ErrorKind::Overflow)?;
                product = product.checked_mul(factor).ok_or(ErrorKind::Overflow)?;
            }
            Ok(product)
        }
        Tree::Conditional(condition, then, otherwise) => {
            if compute::<N>(condition)?.zero() {
                compute(otherwise)
            } else {
                compute(then)
            }
        }
        Tree::Binary(Operator::And, left, right) => {
            if compute::<N>(left)?.zero() {
                return Ok(boolean(false));
            }
            Ok(boolean(!compute::<N>(right)?.zero()))
        }
        Tree::Binary(Operator::Or, left, right) => {
            if !compute::<N>(left)?.zero() {
                return Ok(boolean(true));
            }
            Ok(boolean(!compute::<N>(right)?.zero()))
        }
        Tree::Binary(operator, left, right) => {
            let left = compute::<N>(left)?;
            let right = compute::<N>(right)?;
            match operator {
                Operator::Add => left.checked_add(right).ok_or(ErrorKind::Overflow),
                Operator::Subtract => left.checked_sub(right).ok_or(ErrorKind::Overflow),
                Operator::Multiply => left.checked_mul(right).ok_or(ErrorKind::Overflow),
                Operator::Divide | Operator::Remainder | Operator::Modulo if right.zero() => {
                    Err(ErrorKind::DivisionByZero)
                }
                Operator::Divide => left.checked_div(right).ok_or(ErrorKind::Overflow),
                Operator::Remainder | Operator::Modulo => {
                    left.checked_rem(right).ok_or(ErrorKind::Overflow)
                }
                Operator::Caret | Operator::DoubleStar if right.is_negative() => {
                    Err(ErrorKind::Domain)
                }
                Operator::Caret | Operator::DoubleStar => right
                    .exponent()
                    .and_then(|exponent| left.checked_pow(exponent))
                    .ok_or(ErrorKind::Overflow),
                Operator::Less => Ok(boolean(left < right)),
                Operator::LessEqual => Ok(boolean(left <= right)),
                Operator::Greater => Ok(boolean(left > right)),
                Operator::GreaterEqual => Ok(boolean(left >= right)),
                Operator::Equal => Ok(boolean(left == right)),
                Operator::NotEqual => Ok(boolean(left != right)),
                Operator::And | Operator::Or => unreachable!(),
            }
        }
    }
}

fn check<N: Integer>(tree: &Tree, spaced: bool) {
    let mut text = String::new();
    render::<N>(tree, &mut text, spaced);
    let expected = compute::<N>(tree);
    let actual = Evaluator::<(), 8>::new(&())
        .max_depth(1000)
        .percent_sign(PercentSign::Remainder)
        .evaluate::<N>(&text)
        .map_err(|error| error.kind);
    assert_eq!(actual, expected, "{text}");
}

fuzz_target!(|data: &[u8]| {
    let mut input = Unstructured::new(data);
    let Ok(spaced) = input.arbitrary::<bool>() else {
        return;
    };
    let Ok(tree) = tree(&mut input, 12) else {
        return;
    };
    check::<i8>(&tree, spaced);
    check::<u8>(&tree, spaced);
    check::<i16>(&tree, spaced);
    check::<i32>(&tree, spaced);
    check::<u32>(&tree, spaced);
    check::<i64>(&tree, spaced);
    check::<u64>(&tree, spaced);
    check::<i128>(&tree, spaced);
    check::<u128>(&tree, spaced);
    check::<isize>(&tree, spaced);
    check::<usize>(&tree, spaced);
});
