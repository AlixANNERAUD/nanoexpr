#![cfg(feature = "decimal")]

use nanoexpr::{Decimal, ErrorKind, Number};

fn decimal(text: &str) -> Decimal {
    text.parse().unwrap_or_else(|error| panic!("{text}: {error}"))
}

fn expected(text: &str) -> Result<Decimal, ErrorKind> {
    match text {
        "Overflow" => Err(ErrorKind::Overflow),
        "DivisionByZero" => Err(ErrorKind::DivisionByZero),
        _ => Ok(decimal(text)),
    }
}

#[test]
fn arithmetic_matches_python_decimal() {
    let mut checked = 0;
    for line in include_str!("data/decimal.txt").lines() {
        let (operation, result) = line.split_once(" = ").unwrap();
        let actual = match operation.split(' ').collect::<Vec<_>>()[..] {
            [left, operator, right] => {
                let (left, right) = (decimal(left), decimal(right));
                match operator {
                    "+" => left.add(right),
                    "-" => left.subtract(right),
                    "*" => left.multiply(right),
                    "/" => left.divide(right),
                    "%" => left.remainder(right),
                    _ => unreachable!(),
                }
            }
            [literal] => literal.parse::<Decimal>().map_err(|error| error.kind),
            _ => unreachable!(),
        };
        assert_eq!(actual, expected(result), "{line}");
        checked += 1;
    }
    assert_eq!(checked, 4500);
}
