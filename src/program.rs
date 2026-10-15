//! Compiles an expression once into a [`Program`] of postfix instructions, to evaluate it many times.
//!
//! The instructions live in a slice the caller provides and the values in a fixed-size array, so neither compiling
//! nor running allocates.

use crate::parser::{Backend, Binary, Parser, Unary};
use crate::{Context, Error, ErrorKind, Evaluator, Number, Span};

/// One instruction of a [`Program`], borrowing the names of variables and functions from the expression.
///
/// Provide a slice of them to [`Evaluator::compile`], such as `[Instruction::default(); 64]`: an expression needs
/// about one instruction per number, name, operator and call, and one more per `&&`, `||` and `? :`.
#[derive(Debug, Clone, Copy)]
pub struct Instruction<'i, N>(Code<'i, N>);

impl<N: Default> Default for Instruction<'_, N> {
    fn default() -> Self {
        Self(Code::Number(N::default()))
    }
}

/// What an [`Instruction`] does.
///
/// The spans are those of the errors the instruction reports, the same as the evaluator reports.
#[derive(Debug, Clone, Copy)]
enum Code<'i, N> {
    /// Pushes a number.
    Number(N),
    /// Pushes the value of a variable.
    Variable(&'i str, Span),
    /// Replaces the arguments on top of the stack with the result of a call.
    Call {
        /// The name of the function, also the start of `span`.
        name: &'i str,
        /// How many arguments are on the stack.
        count: usize,
        /// The span of the whole call.
        span: Span,
    },
    /// Replaces the value on top of the stack with the result of an operation.
    Unary(Unary, Span),
    /// Replaces the two values on top of the stack with the result of an operation.
    Binary(Binary, Span),
    /// Pops the left operand of `||` if `or`, or of `&&` otherwise. If it decides the result, pushes it and jumps
    /// to `target`, past the right operand.
    Logic {
        /// Whether this is `||`.
        or: bool,
        /// Where the instructions after the operation start.
        target: usize,
        /// The span of an error reading the left operand.
        condition: Span,
        /// The span of the whole operation.
        span: Span,
    },
    /// Replaces the value on top of the stack with `1` if it is true, `0` otherwise.
    Truth(Span),
    /// Pops a condition and jumps to the given instruction if it is false.
    JumpUnless(usize, Span),
    /// Jumps to the given instruction.
    Jump(usize),
}

/// An expression compiled by [`Evaluator::compile`], to evaluate many times with different variables.
///
/// The variables and functions are resolved by the context at each evaluation, which gives the same results and
/// errors, with the same spans, as [`Evaluator::evaluate`] on the same expression and context. Errors that depend
/// only on the text, such as syntax errors, are reported by [`Evaluator::compile`] instead. Operations on numbers
/// alone, such as `2 * 3.5`, are computed once by the compilation, unless they fail.
///
/// `STACK` is the most values pending at once during an evaluation, such as the left operands of operators
/// waiting for their right one, or the arguments of calls. They are kept in an array on the call stack of
/// [`Program::evaluate`]. An expression that needs more fails to compile with [`ErrorKind::TooLong`].
///
/// ```
/// use nanoexpr::{Instruction, Program};
///
/// let mut code = [Instruction::default(); 16];
/// let program: Program<i64> = nanoexpr::compile("raw * 125 / 10000 - 40", &mut code).unwrap();
/// for (raw, celsius) in [(4000, 10), (4800, 20)] {
///     assert_eq!(program.evaluate(&[("raw", raw)]), Ok(celsius));
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Program<'b, 'i, N, const STACK: usize = 32> {
    /// The instructions, in the order they run without jumps.
    code: &'b [Instruction<'i, N>],
    /// The most values pending at once.
    stack: usize,
}

impl<N: Number, const STACK: usize> Program<'_, '_, N, STACK> {
    /// Returns how many instructions the program takes in its buffer.
    pub const fn len(&self) -> usize {
        self.code.len()
    }

    /// Returns `false`, as a program has at least one instruction.
    pub const fn is_empty(&self) -> bool {
        self.code.is_empty()
    }

    /// Returns the most values pending at once during an evaluation, which is at most `STACK`.
    pub const fn stack(&self) -> usize {
        self.stack
    }

    /// Evaluates the program, resolving names with `context`.
    pub fn evaluate<C: Context<N> + ?Sized>(&self, context: &C) -> Result<N, Error> {
        let mut stack = Stack::<N, STACK>::new();
        let mut position = 0;
        while let Some(Instruction(code)) = self.code.get(position) {
            position += 1;
            match *code {
                Code::Number(value) => stack.push(value),
                Code::Variable(name, span) => match context.variable(name) {
                    Some(value) => stack.push(value),
                    None => return Err(Error::new(ErrorKind::UnknownVariable, span)),
                },
                Code::Call { name, count, span } => {
                    let result = match context.call(name, stack.top(count)) {
                        Some(result) => result.map_err(|kind| Error::new(kind, span))?,
                        None => {
                            let name = Span::new(span.start, span.start + name.len());
                            return Err(Error::new(ErrorKind::UnknownFunction, name));
                        }
                    };
                    stack.drop(count);
                    stack.push(result);
                }
                Code::Unary(operator, span) => {
                    let operand = stack.pop();
                    stack.push(at(operator.apply(operand), span)?);
                }
                Code::Binary(operator, span) => {
                    let right = stack.pop();
                    let left = stack.pop();
                    stack.push(at(operator.apply(left, right), span)?);
                }
                Code::Logic {
                    or,
                    target,
                    condition,
                    span,
                } => {
                    if !at(stack.pop().is_zero(), condition)? == or {
                        stack.push(at(N::from_bool(or), span)?);
                        position = target;
                    }
                }
                Code::Truth(span) => {
                    let truth = !at(stack.pop().is_zero(), span)?;
                    stack.push(at(N::from_bool(truth), span)?);
                }
                Code::JumpUnless(target, span) => {
                    if at(stack.pop().is_zero(), span)? {
                        position = target;
                    }
                }
                Code::Jump(target) => position = target,
            }
        }
        Ok(stack.pop())
    }
}

/// Turns the error of `result` into an [`Error`] at `span`.
fn at<T>(result: Result<T, ErrorKind>, span: Span) -> Result<T, Error> {
    result.map_err(|kind| Error::new(kind, span))
}

/// The values pending during an evaluation.
///
/// Compilation checks that an expression never needs more than `STACK` values nor pops a value it did not push,
/// so the operations never fail. They still check their bounds, returning zeros rather than panicking.
struct Stack<N, const STACK: usize> {
    /// The values, the pending ones first.
    values: [N; STACK],
    /// How many values are pending.
    length: usize,
}

impl<N: Number, const STACK: usize> Stack<N, STACK> {
    /// Creates an empty stack.
    fn new() -> Self {
        Self {
            values: [N::default(); STACK],
            length: 0,
        }
    }

    /// Pushes `value`.
    fn push(&mut self, value: N) {
        if let Some(slot) = self.values.get_mut(self.length) {
            *slot = value;
            self.length += 1;
        }
    }

    /// Pops the value on top.
    fn pop(&mut self) -> N {
        self.length = self.length.saturating_sub(1);
        self.values.get(self.length).copied().unwrap_or_default()
    }

    /// Returns the `count` values on top, the deepest first.
    fn top(&self, count: usize) -> &[N] {
        self.values
            .get(self.length.saturating_sub(count)..self.length)
            .unwrap_or_default()
    }

    /// Drops the `count` values on top.
    fn drop(&mut self, count: usize) {
        self.length = self.length.saturating_sub(count);
    }
}

/// The [`Backend`] that appends each operation to a program.
struct Compilation<'b, 'i, N> {
    /// Where the instructions go.
    code: &'b mut [Instruction<'i, N>],
    /// How many instructions are written.
    length: usize,
    /// How many values are pending after the instructions written, when they run without jumping.
    depth: usize,
    /// The most values pending at once so far.
    stack: usize,
    /// The most values the program may have pending.
    capacity: usize,
    /// The position of the last jump target: constants before it cannot be folded into later operations.
    barrier: usize,
}

impl<'i, N> Compilation<'_, 'i, N> {
    /// Appends an instruction that pops `pops` values and pushes `pushes`, and returns its position.
    ///
    /// Fails with [`ErrorKind::TooLong`] at `span` if the buffer is full or the values pending would not fit.
    fn emit(
        &mut self,
        code: Code<'i, N>,
        pops: usize,
        pushes: usize,
        span: Span,
    ) -> Result<usize, Error> {
        let slot = self
            .code
            .get_mut(self.length)
            .ok_or(Error::new(ErrorKind::TooLong, span))?;
        *slot = Instruction(code);
        self.depth = self.depth.saturating_sub(pops) + pushes;
        if self.depth > self.capacity {
            return Err(Error::new(ErrorKind::TooLong, span));
        }
        self.stack = self.stack.max(self.depth);
        self.length += 1;
        Ok(self.length - 1)
    }

    /// Returns the last `COUNT` instructions if they are all numbers that no jump lands between.
    fn constants<const COUNT: usize>(&self) -> Option<[N; COUNT]>
    where
        N: Copy + Default,
    {
        let start = self.length.checked_sub(COUNT)?;
        if start < self.barrier {
            return None;
        }
        let mut values = [N::default(); COUNT];
        for (value, Instruction(code)) in values.iter_mut().zip(self.code.get(start..self.length)?)
        {
            match code {
                Code::Number(number) => *value = *number,
                _ => return None,
            }
        }
        Some(values)
    }

    /// Replaces the last `count` instructions, numbers, with the number `value`.
    fn fold(&mut self, count: usize, value: N, span: Span) -> Result<(), Error> {
        self.length -= count;
        self.depth = self.depth.saturating_sub(count);
        self.emit(Code::Number(value), 0, 1, span).map(drop)
    }

    /// Points the jump at `position` to the next instruction, and sets the span of a [`Code::Logic`] to `span`.
    fn land(&mut self, position: usize, span: Span) {
        let next = self.length;
        self.barrier = next;
        if let Some(Instruction(code)) = self.code.get_mut(position) {
            match code {
                Code::Logic {
                    target, span: end, ..
                } => {
                    *target = next;
                    *end = span;
                }
                Code::JumpUnless(target, _) | Code::Jump(target) => *target = next,
                _ => {}
            }
        }
    }
}

impl<'i, N: Number> Backend<'i> for Compilation<'_, 'i, N> {
    type Value = ();
    type Branch = usize;

    fn number(&mut self, literal: &'i str, span: Span) -> Result<(), Error> {
        let value = N::parse(literal).ok_or(Error::new(ErrorKind::InvalidNumber, span))?;
        self.emit(Code::Number(value), 0, 1, span).map(drop)
    }

    fn variable(&mut self, name: &'i str, span: Span) -> Result<(), Error> {
        self.emit(Code::Variable(name, span), 0, 1, span).map(drop)
    }

    fn call(&mut self, name: &'i str, _: Span, arguments: &[()], span: Span) -> Result<(), Error> {
        let count = arguments.len();
        self.emit(Code::Call { name, count, span }, count, 1, span)
            .map(drop)
    }

    fn unary(&mut self, operator: Unary, _: (), span: Span) -> Result<(), Error> {
        if let Some(Ok(value)) = self.constants().map(|[operand]| operator.apply(operand)) {
            return self.fold(1, value, span);
        }
        self.emit(Code::Unary(operator, span), 1, 1, span).map(drop)
    }

    fn binary(&mut self, operator: Binary, _: (), _: (), span: Span) -> Result<(), Error> {
        if let Some(Ok(value)) = self
            .constants()
            .map(|[left, right]| operator.apply(left, right))
        {
            return self.fold(2, value, span);
        }
        self.emit(Code::Binary(operator, span), 2, 1, span)
            .map(drop)
    }

    fn logic(&mut self, or: bool, _: (), span: Span) -> Result<usize, Error> {
        let code = Code::Logic {
            or,
            target: 0,
            condition: span,
            span,
        };
        self.emit(code, 1, 0, span)
    }

    fn logic_end(&mut self, _: bool, branch: usize, _: (), span: Span) -> Result<(), Error> {
        self.emit(Code::Truth(span), 1, 1, span)?;
        self.land(branch, span);
        Ok(())
    }

    fn condition(&mut self, _: (), span: Span) -> Result<usize, Error> {
        self.emit(Code::JumpUnless(0, span), 1, 0, span)
    }

    fn otherwise(&mut self, branch: usize, span: Span) -> Result<usize, Error> {
        let jump = self.emit(Code::Jump(0), 0, 0, span)?;
        self.land(branch, span);
        self.depth = self.depth.saturating_sub(1);
        Ok(jump)
    }

    fn conditional_end(&mut self, branch: usize, _: (), _: ()) {
        self.land(branch, Span::new(0, 0));
    }
}

impl<C: ?Sized, const ARGUMENTS: usize> Evaluator<'_, C, ARGUMENTS> {
    /// Compiles `input` into `code`, to evaluate it many times with [`Program::evaluate`].
    ///
    /// The program follows the syntax settings of this evaluator, but resolves names with the context given to
    /// [`Program::evaluate`] rather than with the context of the evaluator. Returns the errors that depend only on
    /// the text: syntax errors, [`ErrorKind::InvalidNumber`], [`ErrorKind::TooDeep`] and
    /// [`ErrorKind::TooManyArguments`], as [`Evaluator::evaluate`] would, and [`ErrorKind::TooLong`] when `code`
    /// or `STACK` is too small.
    ///
    /// ```
    /// # #[cfg(feature = "libm")] {
    /// use nanoexpr::{Evaluator, Instruction, Math, PercentSign, Program};
    ///
    /// let calculator = Evaluator::<_>::new(&Math).percent_sign(PercentSign::Percentage);
    /// let mut code = [Instruction::default(); 32];
    /// let program: Program<f64> = calculator.compile("price * (1 - discount%)", &mut code).unwrap();
    /// assert_eq!(program.evaluate(&[("price", 80.0), ("discount", 25.0)]), Ok(60.0));
    /// assert_eq!(program.evaluate(&[("price", 10.0), ("discount", 50.0)]), Ok(5.0));
    /// # }
    /// ```
    pub fn compile<'b, 'i, N: Number, const STACK: usize>(
        &self,
        input: &'i str,
        code: &'b mut [Instruction<'i, N>],
    ) -> Result<Program<'b, 'i, N, STACK>, Error> {
        let compilation = Compilation {
            code,
            length: 0,
            depth: 0,
            stack: 0,
            capacity: STACK,
            barrier: 0,
        };
        let ((), compilation) = Parser::<_, ARGUMENTS>::parse(input, self.syntax(), compilation)?;
        let code: &'b [Instruction<'i, N>] = compilation.code;
        Ok(Program {
            code: code.get(..compilation.length).unwrap_or_default(),
            stack: compilation.stack,
        })
    }
}

/// Compiles `input` into `code` with the default settings, to evaluate it many times with [`Program::evaluate`].
///
/// See [`Evaluator::compile`].
pub fn compile<'b, 'i, N: Number, const STACK: usize>(
    input: &'i str,
    code: &'b mut [Instruction<'i, N>],
) -> Result<Program<'b, 'i, N, STACK>, Error> {
    Evaluator::<()>::new(&()).compile(input, code)
}
