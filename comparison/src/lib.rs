pub const ARITHMETIC: &str = "((5.0 / 8.0) + 18.0 ^ 0.5 * 10.0 ^ 5.0) / 23.0";
pub const FUNCTIONS: &str = "sin(0.5) * cos(0.5) + sin(1.5) / cos(1.5)";
pub const VARIABLES: &str = "x * x + 2.0 * x * y + y ^ 2.0";

pub trait Library {
    const NAME: &'static str;
    const FUNCTIONS: &'static str = FUNCTIONS;

    fn new() -> Self;
    fn evaluate(&mut self, input: &str) -> f64;
    fn variables(&mut self, x: f64, y: f64) -> f64;
}

#[cfg(feature = "nanoexpr")]
pub struct Nanoexpr;

#[cfg(feature = "nanoexpr")]
impl Library for Nanoexpr {
    const NAME: &'static str = "nanoexpr";

    fn new() -> Self {
        Self
    }

    fn evaluate(&mut self, input: &str) -> f64 {
        nanoexpr::evaluate(input).unwrap()
    }

    fn variables(&mut self, x: f64, y: f64) -> f64 {
        nanoexpr::evaluate_with(VARIABLES, &[("x", x), ("y", y)]).unwrap()
    }
}

#[cfg(feature = "meval")]
pub struct Meval(meval::Context<'static>);

#[cfg(feature = "meval")]
impl Library for Meval {
    const NAME: &'static str = "meval";

    fn new() -> Self {
        Self(meval::Context::new())
    }

    fn evaluate(&mut self, input: &str) -> f64 {
        meval::eval_str(input).unwrap()
    }

    fn variables(&mut self, x: f64, y: f64) -> f64 {
        self.0.var("x", x).var("y", y);
        meval::eval_str_with_context(VARIABLES, &self.0).unwrap()
    }
}

#[cfg(feature = "evalexpr")]
pub struct Evalexpr(evalexpr::HashMapContext);

#[cfg(feature = "evalexpr")]
impl Library for Evalexpr {
    const NAME: &'static str = "evalexpr";
    const FUNCTIONS: &'static str =
        "math::sin(0.5) * math::cos(0.5) + math::sin(1.5) / math::cos(1.5)";

    fn new() -> Self {
        Self(evalexpr::HashMapContext::new())
    }

    fn evaluate(&mut self, input: &str) -> f64 {
        evalexpr::eval_float(input).unwrap()
    }

    fn variables(&mut self, x: f64, y: f64) -> f64 {
        use evalexpr::{ContextWithMutableVariables, Value};
        self.0.set_value("x".into(), Value::Float(x)).unwrap();
        self.0.set_value("y".into(), Value::Float(y)).unwrap();
        evalexpr::eval_float_with_context(VARIABLES, &self.0).unwrap()
    }
}

#[cfg(feature = "fasteval")]
pub struct Fasteval;

#[cfg(feature = "fasteval")]
impl Library for Fasteval {
    const NAME: &'static str = "fasteval";

    fn new() -> Self {
        Self
    }

    fn evaluate(&mut self, input: &str) -> f64 {
        fasteval::ez_eval(input, &mut fasteval::EmptyNamespace).unwrap()
    }

    fn variables(&mut self, x: f64, y: f64) -> f64 {
        let mut namespace = |name: &str, _: Vec<f64>| match name {
            "x" => Some(x),
            "y" => Some(y),
            _ => None,
        };
        fasteval::ez_eval(VARIABLES, &mut namespace).unwrap()
    }
}

#[cfg(feature = "exmex")]
pub struct Exmex;

#[cfg(feature = "exmex")]
impl Library for Exmex {
    const NAME: &'static str = "exmex";

    fn new() -> Self {
        Self
    }

    fn evaluate(&mut self, input: &str) -> f64 {
        exmex::eval_str(input).unwrap()
    }

    fn variables(&mut self, x: f64, y: f64) -> f64 {
        use exmex::Express;
        exmex::parse::<f64>(VARIABLES)
            .unwrap()
            .eval(&[x, y])
            .unwrap()
    }
}

#[cfg(feature = "exp-rs")]
pub struct ExpRs;

#[cfg(feature = "exp-rs")]
impl Library for ExpRs {
    const NAME: &'static str = "exp-rs";

    fn new() -> Self {
        Self
    }

    fn evaluate(&mut self, input: &str) -> f64 {
        exp_rs::interp(input, None).unwrap()
    }

    fn variables(&mut self, x: f64, y: f64) -> f64 {
        let mut context = exp_rs::EvalContext::new();
        context.set_parameter("x", x).unwrap();
        context.set_parameter("y", y).unwrap();
        exp_rs::interp(VARIABLES, Some(std::rc::Rc::new(context))).unwrap()
    }
}

#[cfg(feature = "size")]
#[unsafe(no_mangle)]
pub extern "C" fn evaluate(pointer: *const u8, length: usize) -> f64 {
    let input =
        unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(pointer, length)) };
    #[cfg(feature = "nanoexpr")]
    return Nanoexpr::new().evaluate(input);
    #[cfg(feature = "meval")]
    return Meval::new().evaluate(input);
    #[cfg(feature = "evalexpr")]
    return Evalexpr::new().evaluate(input);
    #[cfg(feature = "fasteval")]
    return Fasteval::new().evaluate(input);
    #[cfg(feature = "exmex")]
    return Exmex::new().evaluate(input);
    #[cfg(feature = "exp-rs")]
    return ExpRs::new().evaluate(input);
}
