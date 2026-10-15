#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["mpmath"]
# ///
"""Writes the expected results of the Decimal math functions to tests/data/decimal-functions.txt, with 14 digits
and exponents up to 99, and to tests/data/decimal-functions-18.txt, with 18 digits and exponents up to 9999.

Run with `uv run scripts/decimal-functions.py`: mpmath computes each function with 300 digits, then Python's
decimal rounds the result half to even.
"""
import decimal
import random
import sys
from pathlib import Path

import mpmath

DIGITS = 14
MAX_EXPONENT = 99
COUNT = 120

mpmath.mp.dps = 300
context = decimal.Context()


def configure(digits, max_exponent, count):
    global DIGITS, MAX_EXPONENT, COUNT, context
    DIGITS, MAX_EXPONENT, COUNT = digits, max_exponent, count
    context = decimal.Context(
        prec=digits, rounding=decimal.ROUND_HALF_EVEN, Emax=decimal.MAX_EMAX, Emin=decimal.MIN_EMIN
    )


class DomainError(Exception):
    pass


def require(valid):
    if not valid:
        raise DomainError


def number(text):
    return context.create_decimal(text)


def big(value):
    return mpmath.mpf(str(value))


def logarithm(x):
    require(x > 0)
    return mpmath.log(big(x))


def power(x, y):
    if x == 0:
        if y < 0:
            return "DivisionByZero"
        return mpmath.mpf(1) if y == 0 else mpmath.mpf(0)
    integer = y == y.to_integral_value()
    require(x > 0 or integer)
    magnitude = mpmath.power(abs(big(x)), big(y))
    odd = integer and int(y) % 2 == 1
    return -magnitude if x < 0 and odd else magnitude


def logarithm_base(x, base):
    require(base != 1)
    return logarithm(x) / logarithm(base)


FUNCTIONS = {
    "sqrt": lambda x: (require(x >= 0), mpmath.sqrt(big(x)))[1],
    "cbrt": lambda x: mpmath.sign(big(x)) * mpmath.cbrt(abs(big(x))),
    "exp": lambda x: mpmath.exp(big(x)),
    "ln": logarithm,
    "log2": lambda x: logarithm(x) / mpmath.log(2),
    "log10": lambda x: logarithm(x) / mpmath.log(10),
    "log": logarithm_base,
    "sin": lambda x: mpmath.sin(big(x)),
    "cos": lambda x: mpmath.cos(big(x)),
    "tan": lambda x: mpmath.tan(big(x)),
    "asin": lambda x: (require(abs(x) <= 1), mpmath.asin(big(x)))[1],
    "acos": lambda x: (require(abs(x) <= 1), mpmath.acos(big(x)))[1],
    "atan": lambda x: mpmath.atan(big(x)),
    "atan2": lambda y, x: mpmath.atan2(big(y), big(x)),
    "sinh": lambda x: mpmath.sinh(big(x)),
    "cosh": lambda x: mpmath.cosh(big(x)),
    "tanh": lambda x: mpmath.tanh(big(x)),
    "asinh": lambda x: mpmath.asinh(big(x)),
    "acosh": lambda x: (require(x >= 1), mpmath.acosh(big(x)))[1],
    "atanh": lambda x: (require(abs(x) < 1), mpmath.atanh(big(x)))[1],
    "hypot": lambda x, y: mpmath.hypot(big(x), big(y)),
    "pow": power,
}


def outcome(name, arguments):
    try:
        value = FUNCTIONS[name](*arguments)
    except DomainError:
        return "Domain"
    if isinstance(value, str):
        return value
    try:
        result = context.create_decimal(mpmath.nstr(value, 60, min_fixed=1, max_fixed=0))
    except decimal.Overflow:
        return "Overflow"
    if result != 0 and result.adjusted() > MAX_EXPONENT:
        return "Overflow"
    if result == 0 or result.adjusted() < -MAX_EXPONENT:
        return "0"
    return str(result)


def random_number(generator, low, high, negative=True):
    digits = generator.randint(1, DIGITS)
    coefficient = generator.randrange(10 ** (digits - 1), 10**digits)
    exponent = generator.randint(low, high) - digits + 1
    sign = "-" if negative and generator.random() < 0.5 else ""
    return number(f"{sign}{coefficient}e{exponent}")


def multiples_of_half_pi(generator):
    for _ in range(40):
        k = generator.choice([1, 2, 3, 4, 5, 6, 7, 8, 100, 355, 1000, 10**6, 10**9, 10**12])
        k = k + generator.randint(0, 20)
        yield number(mpmath.nstr(k * mpmath.pi / 2, 40))


def near(generator, center, count):
    for _ in range(count):
        offset = number(f"{generator.randint(1, 9999)}e{generator.randint(-17, -4)}")
        yield context.add(center, offset) if generator.random() < 0.5 else context.subtract(center, offset)


def arguments(generator):
    wide = lambda: random_number(generator, -MAX_EXPONENT, MAX_EXPONENT)
    small = lambda: random_number(generator, -3, 2)
    for _ in range(COUNT):
        yield "sqrt", [random_number(generator, -MAX_EXPONENT, MAX_EXPONENT, False)]
        yield "cbrt", [wide()]
        yield "exp", [random_number(generator, -20, 2)]
        yield "ln", [random_number(generator, -MAX_EXPONENT, MAX_EXPONENT, False)]
        yield "log2", [random_number(generator, -MAX_EXPONENT, MAX_EXPONENT, False)]
        yield "log10", [random_number(generator, -MAX_EXPONENT, MAX_EXPONENT, False)]
        yield "log", [random_number(generator, -20, 20, False), random_number(generator, -5, 5, False)]
        yield "sin", [random_number(generator, -20, 30)]
        yield "cos", [random_number(generator, -20, 30)]
        yield "tan", [random_number(generator, -20, 30)]
        yield "asin", [random_number(generator, -20, 0)]
        yield "acos", [random_number(generator, -20, 0)]
        yield "atan", [wide()]
        yield "atan2", [random_number(generator, -10, 10), random_number(generator, -10, 10)]
        yield "sinh", [random_number(generator, -30, 2)]
        yield "cosh", [random_number(generator, -30, 2)]
        yield "tanh", [random_number(generator, -30, 2)]
        yield "asinh", [wide()]
        yield "acosh", [context.add(random_number(generator, 0, MAX_EXPONENT, False), 1)]
        yield "atanh", [random_number(generator, -20, -1)]
        yield "hypot", [wide(), wide()]
        yield "pow", [random_number(generator, -10, 10, False), small()]
        yield "pow", [small(), number(generator.randint(-40, 40))]
    for x in multiples_of_half_pi(generator):
        for name in ["sin", "cos", "tan"]:
            yield name, [x]
            yield name, [-x]
    for x in ["1e50", "1e99", "9.9999999999999e99", "123456789.12345e80", "1e22", "355", "1e-99", "0", "1e300",
              "9.87654321987654321e339"]:
        for name in ["sin", "cos", "tan"]:
            yield name, [number(x)]
    for x in near(generator, number(1), 30):
        for name in ["ln", "log2", "log10", "acosh", "asin", "acos", "atanh", "sqrt", "cbrt"]:
            yield name, [x]
    for x in near(generator, number(0), 30):
        for name in ["exp", "sinh", "tanh", "asinh", "atanh", "atan", "asin", "sin", "tan", "cbrt"]:
            yield name, [x]
    for x in near(generator, number(-1), 10):
        for name in ["asin", "acos", "atanh"]:
            yield name, [x]
    for x in near(generator, number(mpmath.nstr(MAX_EXPONENT * mpmath.log(10), 40)), 10):
        for name in ["exp", "sinh", "cosh"]:
            yield name, [x]
            yield name, [-x]
    for x in ["2", "4", "8", "1024", "1e10", "1e-50", "0.001", "1000", "10", "100000000000000"]:
        for name in ["ln", "log2", "log10", "sqrt", "cbrt"]:
            yield name, [number(x)]
    for x in ["0", "-1", "-1e-99", "1.0000000000001", "-1.0000000000001", "1", "-1", "300", "-300", "1e99"]:
        for name in FUNCTIONS:
            if name not in ["log", "atan2", "hypot", "pow"]:
                yield name, [number(x)]
    pairs = [
        ("0", "0"), ("0", "-1"), ("0", "1"), ("1", "0"), ("-1", "0"), ("-1", "-1e-50"), ("1e-50", "-1"), ("3", "4"),
        ("1e99", "1e99"), ("2", "0.5"), ("8", "1"), ("1", "10"), ("1e-99", "1e-99"), ("-1", "1"), ("1e50", "-1e-50"),
    ]
    for x, y in pairs:
        for name in ["atan2", "hypot", "pow", "log"]:
            yield name, [number(x), number(y)]
    for x, y in [("5", "21"), ("1.21", "2.5"), ("4", "1.5"), ("-2", "333"), ("0.5", "1e30"), ("1.5", "1e30"),
                 ("-1", "1e30"), ("10", "-99"), ("10", "-100"), ("27", "0.33333333333333"), ("2", "-0.5")]:
        yield "pow", [number(x), number(y)]


def main():
    directory = Path(sys.argv[1] if len(sys.argv) > 1 else "tests/data")
    for digits, max_exponent, count, name in [(14, 99, 120, "decimal-functions"), (18, 9999, 40, "decimal-functions-18")]:
        configure(digits, max_exponent, count)
        generator = random.Random(digits)
        lines = []
        for function, values in arguments(generator):
            if any(value != 0 and abs(value.adjusted()) > MAX_EXPONENT for value in values):
                continue
            lines.append(f"{function} {' '.join(map(str, values))} = {outcome(function, values)}")
        (directory / f"{name}.txt").write_text("\n".join(lines) + "\n")


main()
