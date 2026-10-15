# nanoexpr

[![crates.io](https://img.shields.io/crates/v/nanoexpr.svg)](https://crates.io/crates/nanoexpr)
[![docs.rs](https://docs.rs/nanoexpr/badge.svg)](https://docs.rs/nanoexpr)
[![CI](https://github.com/AlixANNERAUD/nanoexpr/actions/workflows/rust.yml/badge.svg)](https://github.com/AlixANNERAUD/nanoexpr/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/AlixANNERAUD/nanoexpr/blob/main/LICENSE)

A tiny math expression evaluator for embedded systems: `no_std`, **zero heap allocation**, single pass, with
pluggable number types, variables and functions.

```rust
assert_eq!(nanoexpr::evaluate::<f64>("2 * (3 + 4) ^ 2 / sqrt(16)"), Ok(24.5));
```

Use it for calculator apps, sensor formulas configured in the field (`raw * 0.0125 - 40`), thresholds and
conversions typed by users, or anywhere a whole scripting engine would be too much.

## Features

- **No heap, ever.** No `alloc`, no AST, no token buffer: the expression is computed while it is parsed. A test
  with a counting allocator checks it.
- **Bounded stack.** Nesting is limited (32 levels by default, configurable) so hostile input cannot overflow the
  stack, and call arguments live in a fixed-size array.
- **Any number type.** `f32`, `f64`, every primitive integer, or your own type (fixed-point, decimal...) through
  the `Number` trait. Integers report overflow instead of wrapping.
- **Variables and functions from your code.** The evaluator asks a `Context` for each name it meets: implement the
  trait, pass `(name, value)` pairs, or wrap closures. Contexts compose as tuples.
- **Scientific functions included.** With the default `libm` feature, `Math` provides `pi`, `e`, `sqrt`, `sin`,
  `log`, `min`, `max`, `clamp` and more for `f32` and `f64`.
- **Errors you can show.** A `Copy` error with a kind to match on (and translate) and the byte span of the input
  that caused it. Division by zero, domain errors and overflow are errors, not silent `NaN` or infinities.
- **Calculator-friendly syntax.** `^` and `**` are right-associative (`2^3^2 = 512`), `-2^2 = -4`, and `%` can be a
  percentage (`200 * 15% = 30`) instead of the remainder.
- **No dependencies** besides the optional `libm`, and no `unsafe` code.

## Usage

```toml
[dependencies]
nanoexpr = "0.1"
```

### Variables and functions

```rust
use nanoexpr::{ErrorKind, Functions, Math};

let variables = [("raw", 2150.0), ("offset", -40.0)];
let functions = Functions(|name: &str, arguments: &[f64]| match (name, arguments) {
    ("celsius", [kelvin]) => Some(Ok(kelvin - 273.15)),
    ("celsius", _) => Some(Err(ErrorKind::ArgumentCount)),
    _ => None,
});
let context = (variables, (functions, Math));

assert_eq!(nanoexpr::evaluate_with("raw * 0.0125 + offset", &context), Ok(-13.125));
assert_eq!(nanoexpr::evaluate_with("round(celsius(300))", &context), Ok(27.0));
```

For full control, implement `Context` on your own type:

```rust
use nanoexpr::Context;

struct Sensors {
    readings: [i32; 4],
}

impl Context<i32> for Sensors {
    fn variable(&self, name: &str) -> Option<i32> {
        let index: usize = name.strip_prefix("s")?.parse().ok()?;
        self.readings.get(index).copied()
    }
}

let sensors = Sensors { readings: [12, 30, 7, 1] };
assert_eq!(nanoexpr::evaluate_with("(s0 + s1 + s2) / 3", &sensors), Ok(16));
```

### Errors

```rust
use nanoexpr::{ErrorKind, Span};

let error = nanoexpr::evaluate::<f64>("1 + 4 / (2 - 2)").unwrap_err();
assert_eq!(error.kind, ErrorKind::DivisionByZero);
assert_eq!(error.span, Span::new(4, 15));
assert_eq!(error.to_string(), "division by zero at 4..15");
```

### Calculator mode and limits

```rust
use nanoexpr::{Evaluator, Math, PercentSign};

let calculator = Evaluator::<_, 4>::new(&Math)
    .percent_sign(PercentSign::Percentage)
    .max_depth(16);

assert_eq!(calculator.evaluate::<f64>("200 * 15%"), Ok(30.0));
assert_eq!(calculator.evaluate::<f64>("10 mod 4"), Ok(2.0));
```

The second generic parameter is the most arguments a call can take (8 by default).

## Syntax

From the loosest to the tightest binding:

| Operators | Meaning | Associativity |
|---|---|---|
| `+` `-` | Addition, subtraction | Left |
| `*` `/` `%` `mod` | Multiplication, division, remainder | Left |
| `-x` `+x` | Negation, unary plus | Prefix |
| `^` `**` | Power | Right |
| `x%` | Percentage, with `PercentSign::Percentage` | Postfix |
| `( )` `name(a, b)` | Grouping, function call | |

Numbers are written `42`, `3.14`, `.5`, `5.` or `1.5e-3`. Names start with a letter or `_` and continue with
letters, digits or `_`.

## Footprint

Sizes of a WebAssembly module exporting one evaluation function, built with `opt-level = "z"`, LTO and
`panic = "abort"`:

| Configuration | Size |
|---|---|
| `i32`, no `Math` | 5.7 KiB |
| `f64`, no `Math` | 28.7 KiB |
| `f64` with `Math` | 45.9 KiB |

Most of the `f64` size is `core`'s float parsing, which any crate reading float literals pays for.

## Comparison

How `nanoexpr` compares with other expression evaluators on crates.io, as of their latest release in October 2026:

| Crate | `no_std` | Heap allocation | Number types | Required dependencies | Latest release | License |
|---|---|---|---|---|---|---|
| **nanoexpr** | ✅ | **None** | `f32`, `f64`, all integers, your own | None (`libm` optional) | 2026 | MIT |
| [meval] 0.2.0 | ❌ | Yes | `f64` | `nom`, `fnv` | 2018 | Unlicense / MIT |
| [evalexpr] 13.1.0 | ❌ | Yes | Integers, floats, booleans, strings, tuples | None | 2025 | AGPL-3.0 |
| [fasteval] 0.2.4 | ❌ | Yes | `f64` | None | 2020 | MIT |
| [exmex] 0.21.0 | ❌ | Yes | Floats, custom operator sets | `regex`, `smallvec` | 2026 | MIT / Apache-2.0 |
| [exp-rs] 0.2.0 | ⚠️ ARM only, needs `alloc` | Yes, arena (`bumpalo`) | `f32` or `f64`, chosen at build time | `bitflags`, `bumpalo`, `critical-section`, `hash32`, `heapless`, `serde` | 2025 | MIT / Apache-2.0 |
| [maschen] 0.2.1 | ✅ | None without `std` | Not applicable: converts infix to postfix, does not evaluate | `thiserror` | 2024 | MIT |

And what each is best at:

| Crate | Pick it when you need |
|---|---|
| **nanoexpr** | Run-time math on a microcontroller or in WebAssembly, with predictable memory and errors to show users |
| [meval] | Quick `f64` evaluation on a desktop, binding expressions to closures |
| [evalexpr] | A small scripting language: booleans, strings, assignment, tuples (mind the AGPL license) |
| [fasteval] | Compiling an expression once and evaluating it many times, fast, on a desktop or server |
| [exmex] | Symbolic derivatives and custom operators |
| [exp-rs] | An embedded evaluator with a C API and an allocator you control |
| [maschen] | Building your own evaluator on top of a shunting-yard |

`nanoexpr` parses each time it evaluates. To evaluate the same formula millions of times on a desktop, a crate that
compiles expressions first, such as `fasteval` or `exmex`, will be faster.

[meval]: https://crates.io/crates/meval
[evalexpr]: https://crates.io/crates/evalexpr
[fasteval]: https://crates.io/crates/fasteval
[exmex]: https://crates.io/crates/exmex
[exp-rs]: https://crates.io/crates/exp-rs
[maschen]: https://crates.io/crates/maschen

## Performance

Each crate parses and evaluates three expressions from a string, once per call:

| Expression | Input |
|---|---|
| Arithmetic | `((5.0 / 8.0) + 18.0 ^ 0.5 * 10.0 ^ 5.0) / 23.0` |
| Functions | `sin(0.5) * cos(0.5) + sin(1.5) / cos(1.5)` (`math::sin` and `math::cos` for `evalexpr`) |
| Variables | `x * x + 2.0 * x * y + y ^ 2.0` with `x = 1.5` and `y = 2.5` |

Median time per evaluation:

| Crate | Arithmetic | Functions | Variables |
|---|---|---|---|
| **nanoexpr** | **270 ns** | **270 ns** | **140 ns** |
| meval 0.2.0 | 1340 ns | 1330 ns | 710 ns |
| evalexpr 13.1.0 | 2370 ns | 3310 ns | 2090 ns |
| fasteval 0.2.4 | 430 ns | 650 ns | 350 ns |
| exmex 0.21.0 | 3460 ns | 3570 ns | 6330 ns |
| exp-rs 0.2.0 | 3080 ns | 2940 ns | 3070 ns |

Heap use per evaluation, as allocations / bytes allocated / peak live bytes:

| Crate | Arithmetic | Functions | Variables |
|---|---|---|---|
| **nanoexpr** | **0 / 0 B / 0 B** | **0 / 0 B / 0 B** | **0 / 0 B / 0 B** |
| meval 0.2.0 | 36 / 6106 B / 3296 B | 43 / 4594 B / 2424 B | 18 / 2404 B / 1794 B |
| evalexpr 13.1.0 | 80 / 7440 B / 3264 B | 136 / 8826 B / 4008 B | 76 / 5416 B / 2146 B |
| fasteval 0.2.4 | 12 / 9166 B / 9166 B | 22 / 10191 B / 10027 B | 11 / 8259 B / 8259 B |
| exmex 0.21.0 | 2 / 2606 B / 2606 B | 2 / 2601 B / 2601 B | 4 / 2591 B / 2591 B |
| exp-rs 0.2.0 | Not measured | Not measured | Not measured |

Size of a WebAssembly module exporting one function that evaluates a string with each crate, built with
`opt-level = "z"`, LTO, `panic = "abort"` and symbols stripped. The baseline module only unwraps an `Option`, which
pulls in the standard library's panic and formatting code that every other module also contains:

| Crate | Size | Over baseline |
|---|---|---|
| Baseline | 15.1 KiB | |
| **nanoexpr** | **75.3 KiB** | **60.2 KiB** |
| meval 0.2.0 | 140.0 KiB | 124.8 KiB |
| evalexpr 13.1.0 | 192.7 KiB | 177.5 KiB |
| fasteval 0.2.4 | 128.1 KiB | 113.0 KiB |
| exmex 0.21.0 | 1015.2 KiB | 1000.1 KiB |
| exp-rs 0.2.0 | 172.5 KiB | 157.3 KiB |

These modules use `std`, unlike the `no_std` builds in [Footprint](#footprint).

Measured on an Apple M2 Pro with rustc 1.99.0, in October 2026. Every call parses the expression again: none is
compiled once and reused. `fasteval`, `exmex` and `meval` can compile an expression once and then evaluate it much
faster, so for a formula evaluated many times they can beat `nanoexpr`. Times vary by a few percent between runs. Heap
use is counted by a global allocator wrapper; `exp-rs` installs its own global allocator, so its heap use cannot be
measured that way. To reproduce, from the
[`comparison`](https://github.com/AlixANNERAUD/nanoexpr/tree/main/comparison) directory of the repository:

```sh
cargo run --release --features all --bin bench
cargo run --release --features exp-rs --bin bench
./size.sh
```

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `libm` | ✅ | `Number` for `f32` and `f64`, `Math` and `evaluate`, through the pure Rust [`libm`](https://crates.io/crates/libm) |

## License

[MIT](https://github.com/AlixANNERAUD/nanoexpr/blob/main/LICENSE)
