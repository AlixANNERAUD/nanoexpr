#![cfg(any(feature = "libm", feature = "fraction"))]

use nanoexpr::{Context, ErrorKind, Math, Number};

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
    use nanoexpr::{Error, Span};
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
    assert_eq!(f64::INFINITY.factorial(), Err(ErrorKind::Overflow));
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
