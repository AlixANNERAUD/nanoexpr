use crate::ErrorKind;

/// Resolves the variables and functions an expression refers to.
///
/// The evaluator never stores names or values: it asks the context each time it meets one. Both methods default
/// to knowing nothing, so implement only what you need.
///
/// Contexts compose: a tuple `(first, second)` asks `first`, then `second`. Slices and arrays of `(name, value)`
/// pairs are contexts of variables, and [`Variables`] and [`Functions`] turn closures into contexts.
///
/// ```
/// use nanoexpr::{Context, ErrorKind};
///
/// struct Sensor {
///     raw: i32,
/// }
///
/// impl Context<i32> for Sensor {
///     fn variable(&self, name: &str) -> Option<i32> {
///         (name == "raw").then_some(self.raw)
///     }
///
///     fn call(&self, name: &str, arguments: &[i32]) -> Option<Result<i32, ErrorKind>> {
///         match (name, arguments) {
///             ("double", [value]) => Some(Ok(value * 2)),
///             ("double", _) => Some(Err(ErrorKind::ArgumentCount)),
///             _ => None,
///         }
///     }
/// }
///
/// assert_eq!(nanoexpr::evaluate_with("double(raw) + 1", &Sensor { raw: 20 }), Ok(41));
/// ```
pub trait Context<N> {
    /// Returns the value of the variable `name`, or `None` if it does not exist.
    fn variable(&self, name: &str) -> Option<N> {
        let _ = name;
        None
    }

    /// Calls the function `name`, or returns `None` if it does not exist.
    ///
    /// Return `Some(Err(ErrorKind::ArgumentCount))` when the function exists but `arguments` has the wrong length.
    fn call(&self, name: &str, arguments: &[N]) -> Option<Result<N, ErrorKind>> {
        let _ = (name, arguments);
        None
    }
}

impl<N> Context<N> for () {}

impl<N, C: Context<N> + ?Sized> Context<N> for &C {
    fn variable(&self, name: &str) -> Option<N> {
        (**self).variable(name)
    }

    fn call(&self, name: &str, arguments: &[N]) -> Option<Result<N, ErrorKind>> {
        (**self).call(name, arguments)
    }
}

impl<N, A: Context<N>, B: Context<N>> Context<N> for (A, B) {
    fn variable(&self, name: &str) -> Option<N> {
        self.0.variable(name).or_else(|| self.1.variable(name))
    }

    fn call(&self, name: &str, arguments: &[N]) -> Option<Result<N, ErrorKind>> {
        self.0
            .call(name, arguments)
            .or_else(|| self.1.call(name, arguments))
    }
}

impl<N: Copy> Context<N> for [(&str, N)] {
    fn variable(&self, name: &str) -> Option<N> {
        self.iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, value)| *value)
    }
}

impl<N: Copy, const LENGTH: usize> Context<N> for [(&str, N); LENGTH] {
    fn variable(&self, name: &str) -> Option<N> {
        self.as_slice().variable(name)
    }
}

/// A context of variables resolved by a closure.
///
/// ```
/// use nanoexpr::Variables;
///
/// let readings = [12, 30, 7];
/// let context = Variables(|name: &str| {
///     let index: usize = name.strip_prefix("sensor")?.parse().ok()?;
///     readings.get(index).copied()
/// });
/// assert_eq!(nanoexpr::evaluate_with("sensor0 + sensor2", &context), Ok(19));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Variables<F>(pub F);

impl<N, F: Fn(&str) -> Option<N>> Context<N> for Variables<F> {
    fn variable(&self, name: &str) -> Option<N> {
        (self.0)(name)
    }
}

/// A context of functions resolved by a closure.
///
/// ```
/// use nanoexpr::{ErrorKind, Functions};
///
/// let context = Functions(|name: &str, arguments: &[i64]| match name {
///     "sum" => Some(Ok(arguments.iter().sum())),
///     _ => None,
/// });
/// assert_eq!(nanoexpr::evaluate_with("sum(1, 2, 3)", &context), Ok(6));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Functions<F>(pub F);

impl<N, F: Fn(&str, &[N]) -> Option<Result<N, ErrorKind>>> Context<N> for Functions<F> {
    fn call(&self, name: &str, arguments: &[N]) -> Option<Result<N, ErrorKind>> {
        (self.0)(name, arguments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_resolve_variables() {
        let pairs = [("x", 1), ("y", 2)];
        assert_eq!(pairs.variable("y"), Some(2));
        assert_eq!(pairs.variable("z"), None);
        assert_eq!(pairs.as_slice().variable("x"), Some(1));
    }

    #[test]
    fn tuples_ask_the_first_context_first() {
        let context = ([("x", 1)], [("x", 2), ("y", 3)]);
        assert_eq!(context.variable("x"), Some(1));
        assert_eq!(context.variable("y"), Some(3));
        assert_eq!(context.call("f", &[]), None);
    }

    #[test]
    fn unit_knows_nothing() {
        assert_eq!(Context::<i32>::variable(&(), "x"), None);
        assert_eq!(Context::<i32>::call(&(), "f", &[1]), None);
    }
}
