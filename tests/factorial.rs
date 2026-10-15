#![cfg(any(feature = "libm", feature = "fraction"))]

use nanoexpr::{
    Context, Error, ErrorKind, Evaluator, Instruction, Math, Number, PercentSign, Program, Span,
};

fn evaluate<N: Number>(input: &str) -> Result<N, ErrorKind>
where
    Math: Context<N>,
{
    nanoexpr::evaluate(input).map_err(|error| error.kind)
}

#[cfg(feature = "fraction")]
fn shown<N: Number + core::fmt::Display>(input: &str) -> String
where
    Math: Context<N>,
{
    evaluate::<N>(input)
        .unwrap_or_else(|error| panic!("{input}: {error}"))
        .to_string()
}

#[test]
fn integers() {
    assert_eq!(0_i64.factorial(), Ok(1));
    assert_eq!(1_i64.factorial(), Ok(1));
    assert_eq!(5_i64.factorial(), Ok(120));
    assert_eq!(20_i64.factorial(), Ok(2_432_902_008_176_640_000));
    assert_eq!(21_i64.factorial(), Err(ErrorKind::Overflow));
    assert_eq!(34_u128.factorial().map(|value| value > 0), Ok(true));
    assert_eq!(35_u128.factorial(), Err(ErrorKind::Overflow));
    assert_eq!(5_u8.factorial(), Ok(120));
    assert_eq!(6_u8.factorial(), Err(ErrorKind::Overflow));
    assert_eq!(u8::MAX.factorial(), Err(ErrorKind::Overflow));
    assert_eq!((-1_i32).factorial(), Err(ErrorKind::Domain));
    assert_eq!(i64::MIN.factorial(), Err(ErrorKind::Domain));
    assert_eq!(i64::MAX.factorial(), Err(ErrorKind::Overflow));
}

#[cfg(feature = "libm")]
#[test]
fn floats() {
    assert_eq!(evaluate::<f64>("fact(0)"), Ok(1.0));
    assert_eq!(evaluate::<f64>("fact(1)"), Ok(1.0));
    assert_eq!(evaluate::<f64>("fact(5)"), Ok(120.0));
    assert_eq!(evaluate::<f64>("fact(20)"), Ok(2_432_902_008_176_640_000.0));
    let largest = evaluate::<f64>("fact(170)").unwrap();
    assert!((largest / 7.257_415_615_307_999e306 - 1.0).abs() < 1e-14);
    assert_eq!(evaluate::<f64>("fact(171)"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate::<f64>("fact(1e300)"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate::<f32>("fact(34)").map(f32::is_finite), Ok(true));
    assert_eq!(evaluate::<f32>("fact(35)"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate::<f64>("fact(-1)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<f64>("fact(-0.5)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<f64>("fact(2.5)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<f64>("fact(170.5)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<f64>("fact(-0)"), Ok(1.0));
    assert_eq!(f64::NAN.factorial(), Err(ErrorKind::Domain));
    assert_eq!(f64::INFINITY.factorial(), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<f64>("fact(200.5)"), Err(ErrorKind::Domain));
    assert_eq!(f64::NEG_INFINITY.factorial(), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<f64>("fact()"), Err(ErrorKind::ArgumentCount));
    assert_eq!(evaluate::<f64>("fact(1, 2)"), Err(ErrorKind::ArgumentCount));
    assert_eq!(
        nanoexpr::evaluate::<f64>("1 + fact(-1)"),
        Err(Error::new(ErrorKind::Domain, Span::new(4, 12)))
    );
}

#[cfg(feature = "decimal")]
#[test]
fn decimals() {
    use nanoexpr::Decimal;
    assert_eq!(shown::<Decimal>("fact(0)"), "1");
    assert_eq!(shown::<Decimal>("fact(1)"), "1");
    assert_eq!(shown::<Decimal>("fact(5)"), "120");
    assert_eq!(shown::<Decimal>("fact(17)"), "3.55687428096e14");
    assert_eq!(shown::<Decimal>("fact(20)"), "2.4329020081766e18");
    assert_eq!(shown::<Decimal>("fact(28)"), "3.0488834461171e29");
    assert_eq!(shown::<Decimal>("fact(46)"), "5.5026221598121e57");
    assert_eq!(shown::<Decimal>("fact(69)"), "1.7112245242814e98");
    assert_eq!(evaluate::<Decimal>("fact(70)"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate::<Decimal>("fact(1e99)"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate::<Decimal>("fact(-1)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<Decimal>("fact(3.5)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<Decimal>("fact(1e-99)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<Decimal>("fact(-1e99)"), Err(ErrorKind::Domain));
    let wide = |n: u32| {
        format!("{n}")
            .parse::<Decimal<18, 9999>>()
            .unwrap()
            .factorial()
            .map(|value| value.to_string())
    };
    assert_eq!(wide(25), Ok("1.5511210043330986e25".to_string()));
    assert_eq!(wide(3248), Ok("1.97363425308604253e9997".to_string()));
    assert_eq!(wide(3249), Err(ErrorKind::Overflow));
}

#[cfg(feature = "fraction")]
#[test]
fn fractions() {
    use nanoexpr::Fraction;
    assert_eq!(shown::<Fraction>("fact(0)"), "1");
    assert_eq!(shown::<Fraction>("fact(5)"), "120");
    assert_eq!(shown::<Fraction>("fact(20)"), "2432902008176640000");
    assert_eq!(shown::<Fraction>("fact(10/2)"), "120");
    assert_eq!(evaluate::<Fraction>("fact(21)"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate::<Fraction>("fact(-1)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<Fraction>("fact(1/2)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<Fraction>("fact(7/2)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<Fraction>("fact(55/2)"), Err(ErrorKind::Domain));
}

#[cfg(feature = "exact")]
#[test]
fn exact_numbers() {
    use nanoexpr::Exact;
    let value = |input: &str| evaluate::<Exact>(input).unwrap();
    assert!(value("fact(20)").is_exact());
    assert_eq!(value("fact(20)").to_string(), "2432902008176640000");
    assert!(!value("fact(21)").is_exact());
    assert_eq!(value("fact(21)").to_string(), "5.1090942171709e19");
    assert_eq!(value("fact(69)").to_string(), "1.7112245242814e98");
    assert_eq!(value("fact(round(pi))").to_string(), "6");
    assert!(!value("fact(round(pi))").is_exact());
    assert_eq!(evaluate::<Exact>("fact(70)"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate::<Exact>("fact(-1)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<Exact>("fact(1/2)"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<Exact>("fact(pi)"), Err(ErrorKind::Domain));
}

fn integer(input: &str) -> Result<i64, Error> {
    Evaluator::<()>::new(&()).evaluate(input)
}

fn calculator(input: &str) -> Result<i64, Error> {
    let context = [("x", 3), ("y", 4)];
    Evaluator::<_>::new(&context)
        .implicit_multiplication(true)
        .percent_sign(PercentSign::Percentage)
        .evaluate(input)
}

#[test]
fn postfix_operator() {
    for (input, expected) in [
        ("0!", 1),
        ("1!", 1),
        ("5!", 120),
        ("20!", 2_432_902_008_176_640_000),
        ("5 !", 120),
        ("(1 + 2)!", 6),
        ("2 * 3!", 12),
        ("3! * 2", 12),
        ("3! + 4!", 30),
        ("2^3!", 64),
        ("2 ** 3!", 64),
        ("3!^2", 36),
        ("-3!", -6),
        ("--3!", 6),
        ("3!!", 720),
        ("(3!)!", 720),
        ("!0", 1),
        ("!3!", 0),
        ("!0!", 0),
        ("!!3", 1),
        ("10 mod 3!", 4),
        ("3! > 5", 1),
        ("0! ? 3! : 4!", 6),
    ] {
        assert_eq!(integer(input), Ok(expected), "{input}");
    }
}

#[test]
fn postfix_errors_cover_the_operand_and_the_sign() {
    for (input, kind, start, end) in [
        ("(-1)!", ErrorKind::Domain, 0, 5),
        ("1 + (0 - 2)!", ErrorKind::Domain, 4, 12),
        ("21!", ErrorKind::Overflow, 0, 3),
        ("2 * 21 !", ErrorKind::Overflow, 4, 8),
        ("-21!", ErrorKind::Overflow, 1, 4),
        ("2^21!", ErrorKind::Overflow, 2, 5),
        ("(-1)! + 1 / 0", ErrorKind::Domain, 0, 5),
        ("3!!!", ErrorKind::Overflow, 0, 4),
        ("!", ErrorKind::UnexpectedEnd, 1, 1),
        ("3 + !", ErrorKind::UnexpectedEnd, 5, 5),
        ("!3 !2", ErrorKind::UnexpectedToken, 4, 5),
        ("3!2", ErrorKind::UnexpectedToken, 2, 3),
        ("5!==120", ErrorKind::UnexpectedCharacter, 3, 4),
        ("5! = 120", ErrorKind::UnexpectedCharacter, 3, 4),
    ] {
        assert_eq!(
            integer(input),
            Err(Error::new(kind, Span::new(start, end))),
            "{input}"
        );
    }
}

#[test]
fn not_equal_is_read_first() {
    assert_eq!(integer("5!=3"), Ok(1));
    assert_eq!(integer("5!=5"), Ok(0));
    assert_eq!(integer("5! == 120"), Ok(1));
    assert_eq!(integer("(5!)==120"), Ok(1));
    assert_eq!(integer("3!!=720"), Ok(1));
    assert_eq!(integer("3!! == 720"), Ok(1));
}

#[test]
fn skipped_operands_are_not_computed() {
    for (input, expected) in [
        ("0 && (-1)!", 0),
        ("1 || 21!", 1),
        ("0 ? 21! : 2", 2),
        ("1 ? 2 : (-1)!", 2),
        ("0 && 3!!!", 0),
    ] {
        assert_eq!(integer(input), Ok(expected), "{input}");
    }
}

#[test]
fn implicit_multiplication_and_percentages() {
    for (input, expected) in [
        ("3!x", 18),
        ("3! x", 18),
        ("2x!", 12),
        ("x!y", 24),
        ("x!y!", 144),
        ("3!(2)", 12),
        ("2(3)!", 12),
        ("(1 + 1)(2)!", 4),
        ("200%!", 2),
        ("5!%", 1),
    ] {
        assert_eq!(calculator(input), Ok(expected), "{input}");
    }
    assert_eq!(
        calculator("3!2"),
        Err(Error::new(ErrorKind::UnexpectedToken, Span::new(2, 3)))
    );
    assert_eq!(
        calculator("x !2"),
        Err(Error::new(ErrorKind::UnexpectedToken, Span::new(3, 4)))
    );
    assert_eq!(
        integer("3!x"),
        Err(Error::new(ErrorKind::UnexpectedToken, Span::new(2, 3)))
    );
}

#[test]
fn long_chains_do_not_nest() {
    let ones = "1".to_string() + &"!".repeat(10_000);
    assert_eq!(integer(&ones), Ok(1));
    let threes = "3".to_string() + &"!".repeat(10_000);
    assert_eq!(
        integer(&threes),
        Err(Error::new(ErrorKind::Overflow, Span::new(0, 4)))
    );
}

#[test]
fn compiled_programs() {
    let mut code = [Instruction::default(); 16];
    let program: Program<i64> = nanoexpr::compile("x! + 1", &mut code).unwrap();
    assert_eq!(program.evaluate(&[("x", 5)]), Ok(121));
    assert_eq!(program.evaluate(&[("x", 0)]), Ok(2));
    assert_eq!(
        program.evaluate(&[("x", -1)]),
        Err(Error::new(ErrorKind::Domain, Span::new(0, 2)))
    );
    assert_eq!(
        program.evaluate(&[("x", 21)]),
        Err(Error::new(ErrorKind::Overflow, Span::new(0, 2)))
    );
    let length = |input, code: &mut [Instruction<'_, i64>]| {
        nanoexpr::compile::<i64, 8>(input, code).map(|program| program.len())
    };
    assert_eq!(length("3!! + 2^3!", &mut code), Ok(1));
    assert_eq!(length("(-1)!", &mut code), Ok(2));
    let program: Program<i64> = nanoexpr::compile("x && (-1)!", &mut code).unwrap();
    assert_eq!(program.evaluate(&[("x", 0)]), Ok(0));
    assert_eq!(
        program.evaluate(&[("x", 1)]),
        Err(Error::new(ErrorKind::Domain, Span::new(5, 10)))
    );
}

#[cfg(feature = "libm")]
#[test]
fn postfix_floats() {
    assert_eq!(nanoexpr::evaluate::<f64>("170! == fact(170)"), Ok(1.0));
    assert_eq!(nanoexpr::evaluate::<f64>("5! / 4!"), Ok(5.0));
    assert_eq!(evaluate::<f64>("171!"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate::<f64>("2.5!"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<f64>("pi!"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<f32>("35!"), Err(ErrorKind::Overflow));
}

#[cfg(feature = "decimal")]
#[test]
fn postfix_decimals() {
    use nanoexpr::Decimal;
    assert_eq!(shown::<Decimal>("69!"), "1.7112245242814e98");
    assert_eq!(shown::<Decimal>("69! / 68!"), "69");
    assert_eq!(evaluate::<Decimal>("70!"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate::<Decimal>("0.5!"), Err(ErrorKind::Domain));
}

#[cfg(feature = "fraction")]
#[test]
fn postfix_fractions() {
    use nanoexpr::Fraction;
    assert_eq!(shown::<Fraction>("20! / 18!"), "380");
    assert_eq!(shown::<Fraction>("(4/2)!"), "2");
    assert_eq!(evaluate::<Fraction>("21!"), Err(ErrorKind::Overflow));
    assert_eq!(evaluate::<Fraction>("(1/2)!"), Err(ErrorKind::Domain));
}

#[cfg(feature = "exact")]
#[test]
fn postfix_exact_numbers() {
    use nanoexpr::Exact;
    let value = |input: &str| evaluate::<Exact>(input).unwrap();
    assert!(value("20!").is_exact());
    assert!(!value("21!").is_exact());
    assert_eq!(value("21!").to_string(), "5.1090942171709e19");
    assert_eq!(value("21! == fact(21)").to_string(), "1");
    assert_eq!(evaluate::<Exact>("(1/2)!"), Err(ErrorKind::Domain));
    assert_eq!(evaluate::<Exact>("70!"), Err(ErrorKind::Overflow));
}
