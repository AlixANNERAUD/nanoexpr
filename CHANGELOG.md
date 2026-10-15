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

### Documentation

- keep the examples building without libm
- add a readme with usage, footprint and a comparison with other crates
- use the readme as crate documentation
- link the license with an absolute url

### Miscellaneous

- initialize the crate
- cover the evaluator syntax, errors and limits
- check that evaluation never allocates
- warn on missing documentation
