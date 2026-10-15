use nanoexpr::{
    Context, Error, ErrorKind, Evaluator, Functions, Instruction, Number, PercentSign, Program,
    Span,
};

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const NUMBERS: &[&str] = &[
    "0",
    "1",
    "2",
    "3",
    "10",
    "100",
    "0.5",
    "1e3",
    "2.5e-1",
    ".5",
    "256",
    "9223372036854775807",
];
const NAMES: &[&str] = &["x", "y", "z", "big", "unknown", "e", "mod"];
const BINARY: &[&str] = &[
    "+", "-", "*", "/", "%", "mod", "^", "**", "<", "<=", ">", ">=", "==", "!=", "&&", "||",
];
const NOISE: &[&str] = &[
    "(", ")", "?", ":", ",", "+", "*", "!", "&&", "=", "<", "1", "x", "f(",
];

fn space(random: &mut Random, output: &mut String) {
    match random.below(4) {
        0 => {}
        1 | 2 => output.push(' '),
        _ => output.push_str("  "),
    }
}

fn expression(random: &mut Random, depth: usize, output: &mut String) {
    let leaf = depth == 0 || random.below(3) == 0;
    if leaf {
        match random.below(3) {
            0 => output.push_str(random.pick(NAMES)),
            _ => output.push_str(random.pick(NUMBERS)),
        }
        return;
    }
    match random.below(11) {
        0..=2 => {
            expression(random, depth - 1, output);
            space(random, output);
            output.push_str(random.pick(BINARY));
            space(random, output);
            expression(random, depth - 1, output);
        }
        3 => {
            output.push_str(random.pick(&["-", "+", "!"]));
            expression(random, depth - 1, output);
        }
        4 => {
            output.push('(');
            expression(random, depth - 1, output);
            output.push(')');
        }
        5 => {
            output.push_str(random.pick(&["f", "g", "h", "fail", "missing", "x"]));
            output.push('(');
            let count = random.below(5);
            for index in 0..count {
                if index > 0 {
                    output.push(',');
                    space(random, output);
                }
                expression(random, depth - 1, output);
            }
            output.push(')');
        }
        6 | 7 => {
            expression(random, depth - 1, output);
            space(random, output);
            output.push('?');
            space(random, output);
            expression(random, depth - 1, output);
            space(random, output);
            output.push(':');
            space(random, output);
            expression(random, depth - 1, output);
        }
        8 => {
            expression(random, depth - 1, output);
            space(random, output);
            expression(random, depth - 1, output);
        }
        9 => {
            expression(random, depth - 1, output);
            output.push('%');
        }
        _ => {
            expression(random, depth - 1, output);
            space(random, output);
            output.push_str(random.pick(&["&&", "||"]));
            space(random, output);
            expression(random, depth - 1, output);
        }
    }
}

fn input(random: &mut Random) -> String {
    let mut output = String::new();
    let depth = random.below(7);
    expression(random, depth, &mut output);
    if random.below(5) == 0 {
        let mut position = random.below(output.len() + 1);
        while !output.is_char_boundary(position) {
            position -= 1;
        }
        if random.below(2) == 0 && position < output.len() {
            output.remove(position);
        } else {
            output.insert_str(position, random.pick(NOISE));
        }
    }
    output
}

fn functions<N: Number + PartialOrd>(name: &str, arguments: &[N]) -> Option<Result<N, ErrorKind>> {
    let zero = N::default();
    match (name, arguments) {
        ("f", [first, rest @ ..]) => {
            Some(rest.iter().try_fold(*first, |sum, value| sum.add(*value)))
        }
        ("f", []) => Some(Err(ErrorKind::ArgumentCount)),
        ("g", [value]) if *value < zero => Some(Err(ErrorKind::Domain)),
        ("g", [value]) => Some(Ok(*value)),
        ("h", []) => Some(N::from_bool(true)),
        ("fail", _) => Some(Err(ErrorKind::Overflow)),
        _ => None,
    }
}

fn compare<N, C, const ARGUMENTS: usize>(
    evaluator: &Evaluator<'_, C, ARGUMENTS>,
    context: &C,
    input: &str,
    same: impl Fn(&N, &N) -> bool,
) -> bool
where
    N: Number + core::fmt::Debug,
    C: Context<N>,
{
    let direct = evaluator.evaluate::<N>(input);
    let mut code = [Instruction::default(); 1024];
    let compiled = evaluator
        .compile::<N, 512>(input, &mut code)
        .and_then(|program| program.evaluate(context));
    let equal = match (&direct, &compiled) {
        (Ok(left), Ok(right)) => same(left, right),
        (Err(left), Err(right)) => left == right,
        _ => false,
    };
    assert!(
        equal,
        "{input:?}: evaluated {direct:?}, compiled {compiled:?}"
    );
    direct.is_ok()
}

fn differential<N, const ARGUMENTS: usize>(
    variables: [(&str, N); 4],
    seed: u64,
    same: impl Fn(&N, &N) -> bool + Copy,
) where
    N: Number + PartialOrd + core::fmt::Debug,
{
    let context = (variables, Functions(functions::<N>));
    let mut random = Random(seed);
    let mut successes = 0;
    let mut total = 0;
    for percent_sign in [PercentSign::Remainder, PercentSign::Percentage] {
        for implicit_multiplication in [false, true] {
            for max_depth in [4, 32] {
                let evaluator = Evaluator::<_, ARGUMENTS>::new(&context)
                    .percent_sign(percent_sign)
                    .implicit_multiplication(implicit_multiplication)
                    .max_depth(max_depth);
                for _ in 0..3_000 {
                    let input = input(&mut random);
                    successes += usize::from(compare(&evaluator, &context, &input, same));
                    total += 1;
                }
            }
        }
    }
    assert!(successes * 10 > total, "{successes} of {total} succeeded");
}

#[test]
fn compiled_integers_match_evaluation() {
    let variables = [("x", 3_i64), ("y", 0), ("z", -2), ("big", i64::MAX)];
    differential::<i64, 8>(variables, 0x9e37_79b9_7f4a_7c15, i64::eq);
    differential::<i64, 2>(variables, 0x2545_f491_4f6c_dd1d, i64::eq);
}

#[cfg(feature = "libm")]
#[test]
fn compiled_floats_match_evaluation() {
    let variables = [("x", 3.0), ("y", 0.0), ("z", -2.5), ("big", f64::NAN)];
    differential::<f64, 8>(variables, 0xdead_beef_cafe_f00d, |left, right| {
        left.to_bits() == right.to_bits()
    });
}

#[cfg(feature = "decimal")]
#[test]
fn compiled_decimals_match_evaluation() {
    use nanoexpr::Decimal;
    let number = |text: &str| text.parse::<Decimal>().unwrap();
    let variables = [
        ("x", number("3")),
        ("y", number("0")),
        ("z", number("-2.5")),
        ("big", number("9e99")),
    ];
    differential::<Decimal, 8>(variables, 0x0123_4567_89ab_cdef, |left, right| {
        left == right
    });
}

#[cfg(feature = "fraction")]
#[test]
fn compiled_fractions_match_evaluation() {
    use nanoexpr::Fraction;
    let number = |text: &str| text.parse::<Fraction>().unwrap();
    let variables = [
        ("x", number("3")),
        ("y", number("0")),
        ("z", number("-5/2")),
        ("big", number("18446744073709551615")),
    ];
    differential::<Fraction, 8>(variables, 0x0f1e_2d3c_4b5a_6978, Fraction::eq);
}

#[cfg(feature = "exact")]
#[test]
fn compiled_exact_numbers_match_evaluation() {
    use nanoexpr::{Exact, Number};
    let number = |text: &str| Exact::parse(text).unwrap();
    let variables = [
        ("x", number("3")),
        ("y", number("0")),
        ("z", number("2.5")),
        ("big", number("9e99")),
    ];
    differential::<Exact, 8>(variables, 0x5851_f42d_4c95_7f2d, |left, right| {
        left == right
    });
}

#[test]
fn programs_are_reusable() {
    let mut code = [Instruction::default(); 16];
    let program: Program<i64> = nanoexpr::compile("x != 0 && 100 / x > 2", &mut code).unwrap();
    assert_eq!(program.evaluate(&[("x", 0)]), Ok(0));
    assert_eq!(program.evaluate(&[("x", 10)]), Ok(1));
    assert_eq!(program.evaluate(&[("x", 50)]), Ok(0));
    assert_eq!(
        program.evaluate(&[("y", 1)]),
        Err(Error::new(ErrorKind::UnknownVariable, Span::new(0, 1)))
    );
    assert_eq!(program.len(), 10);
    assert!(!program.is_empty());
    assert_eq!(program.stack(), 2);
}

#[test]
fn compilation_reports_errors_of_the_text() {
    let mut code = [Instruction::<i64>::default(); 16];
    let error = |input| {
        nanoexpr::compile::<i64, 8>(input, &mut [Instruction::default(); 16])
            .map(|program| program.len())
            .unwrap_err()
    };
    assert_eq!(
        error("1 +"),
        Error::new(ErrorKind::UnexpectedEnd, Span::new(3, 3))
    );
    assert_eq!(
        error("0 && 1.5"),
        Error::new(ErrorKind::InvalidNumber, Span::new(5, 8))
    );
    assert_eq!(
        error("1 / 0 + )"),
        Error::new(ErrorKind::UnexpectedToken, Span::new(8, 9))
    );
    assert!(nanoexpr::compile::<i64, 8>("1 / 0", &mut code).is_ok());
}

#[test]
fn small_buffers_are_too_long() {
    let mut code = [Instruction::default(); 3];
    assert_eq!(
        nanoexpr::compile::<i64, 8>("x + y", &mut code).map(|program| program.len()),
        Ok(3)
    );
    assert_eq!(
        nanoexpr::compile::<i64, 8>("x + y + z", &mut code).map(|program| program.len()),
        Err(Error::new(ErrorKind::TooLong, Span::new(8, 9)))
    );
    let mut code = [Instruction::default(); 32];
    assert_eq!(
        nanoexpr::compile::<i64, 2>("x + y * z", &mut code).map(|program| program.len()),
        Err(Error::new(ErrorKind::TooLong, Span::new(8, 9)))
    );
    assert_eq!(
        nanoexpr::compile::<i64, 3>("x + y * z", &mut code).map(|program| program.stack()),
        Ok(3)
    );
    assert_eq!(
        nanoexpr::compile::<i64, 0>("1", &mut code).map(|program| program.len()),
        Err(Error::new(ErrorKind::TooLong, Span::new(0, 1)))
    );
}

#[test]
fn constants_are_folded() {
    let mut code = [Instruction::default(); 32];
    let length = |input, code: &mut [Instruction<'_, i64>]| {
        nanoexpr::compile::<i64, 8>(input, code).map(|program| program.len())
    };
    assert_eq!(length("1 + 2 * -3 ^ 2", &mut code), Ok(1));
    assert_eq!(length("x * (60 * 60)", &mut code), Ok(3));
    assert_eq!(length("1 / 0", &mut code), Ok(3));
    assert_eq!(length("(x ? 1 : 2) + 3", &mut code), Ok(7));
    assert_eq!(length("x && 1 + 2", &mut code), Ok(4));
    let program: Program<i64> = nanoexpr::compile("2 * 3 + (x ? 1 : 2) * 10", &mut code).unwrap();
    assert_eq!(program.evaluate(&[("x", 1)]), Ok(16));
    assert_eq!(program.evaluate(&[("x", 0)]), Ok(26));
    let program: Program<i64> = nanoexpr::compile("x + 1 / 0", &mut code).unwrap();
    assert_eq!(
        program.evaluate(&[("x", 1)]),
        Err(Error::new(ErrorKind::DivisionByZero, Span::new(4, 9)))
    );
}

#[test]
fn conditionals_count_one_pending_value() {
    let mut code = [Instruction::default(); 32];
    let program = nanoexpr::compile::<i64, 1>("1 ? 2 : 3 ? 4 : 5", &mut code).unwrap();
    assert_eq!(program.evaluate(&()), Ok(2));
    let program = nanoexpr::compile::<i64, 1>("0 || 1 && 0 || 2", &mut code).unwrap();
    assert_eq!(program.stack(), 1);
    assert_eq!(program.evaluate(&()), Ok(1));
}
