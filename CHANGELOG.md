# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0](https://github.com/AlixANNERAUD/nanoexpr/releases/tag/v0.1.0) - 2026-10-15

### Added

- add the error type
- add the number trait for floats and integers
- add the context trait for variables and functions
- add a byte-based lexer
- add a single-pass evaluator
- add the math context with scientific functions
- add c bindings ([#6](https://github.com/AlixANNERAUD/nanoexpr/pull/6))
- add a decimal number type like pocket calculators use ([#7](https://github.com/AlixANNERAUD/nanoexpr/pull/7))

### Documentation

- keep the examples building without libm
- add a readme with usage, footprint and a comparison with other crates
- use the readme as crate documentation
- link the license with an absolute url
- document the lexer
- document the evaluator internals
- document the number implementations
- document the math context internals
- describe the context and error modules
- compare speed, heap use and size with other crates ([#5](https://github.com/AlixANNERAUD/nanoexpr/pull/5))
- make the readme welcoming for new users and search engines ([#9](https://github.com/AlixANNERAUD/nanoexpr/pull/9))

### Miscellaneous

- initialize the crate
- cover the evaluator syntax, errors and limits
- check that evaluation never allocates
- warn on missing documentation
- warn on undocumented private items
- upload coverage to codecov ([#3](https://github.com/AlixANNERAUD/nanoexpr/pull/3))
