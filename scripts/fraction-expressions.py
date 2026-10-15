#!/usr/bin/env python3
"""Writes random expressions and their exact values to tests/data/fraction.txt, to check the Fraction type.

Run with `uv run scripts/fraction-expressions.py`. Each line is `expression = numerator/denominator`, or the error
the expression gives: Python's `fractions` computes each operation exactly, and a result whose numerator or
denominator is beyond u64 is an overflow. The remainder takes the sign of the dividend, as in Rust.
"""
import random
import sys
from fractions import Fraction
from pathlib import Path

COUNT = 4000
LIMIT = 2**64 - 1


class Failure(Exception):
    pass


def checked(value):
    if abs(value.numerator) > LIMIT or value.denominator > LIMIT:
        raise Failure("Overflow")
    return value


def literal(generator):
    kind = generator.random()
    if kind < 0.4:
        return str(generator.randint(0, 12))
    if kind < 0.6:
        return str(generator.randint(0, 10 ** generator.randint(2, 19)))
    if kind < 0.7:
        return generator.choice(["18446744073709551615", "9223372036854775807", "4294967296", "4294967295"])
    if kind < 0.9:
        digits = str(generator.randint(1, 10 ** generator.randint(1, 8)))
        point = generator.randint(0, len(digits))
        return f"{digits[:point]}.{digits[point:]}"
    return f"{generator.randint(1, 99)}e{generator.randint(-12, 6)}"


def value_of(text):
    return checked(Fraction(text))


def expression(generator, depth):
    if depth == 0 or generator.random() < 0.25:
        text = literal(generator)
        return text, lambda: value_of(text)
    kind = generator.random()
    if kind < 0.1:
        inner, compute = expression(generator, depth - 1)
        return f"-({inner})", lambda: -compute()
    if kind < 0.2:
        base, compute = expression(generator, depth - 1)
        exponent = generator.randint(-4, 6)

        def power():
            value = compute()
            if value == 0 and exponent < 0:
                raise Failure("DivisionByZero")
            return checked(value**exponent)

        return f"({base}) ^ ({exponent})", power
    operator = generator.choice("+-*/%")
    left, compute_left = expression(generator, depth - 1)
    right, compute_right = expression(generator, depth - 1)

    def binary():
        a = compute_left()
        b = compute_right()
        if operator in "/%" and b == 0:
            raise Failure("DivisionByZero")
        if operator == "+":
            return checked(a + b)
        if operator == "-":
            return checked(a - b)
        if operator == "*":
            return checked(a * b)
        if operator == "/":
            return checked(a / b)
        quotient = a / b
        truncated = quotient.numerator // quotient.denominator
        if quotient < 0 and quotient.denominator != 1:
            truncated += 1
        return checked(a - b * truncated)

    return f"({left} {operator} {right})", binary


def main():
    generator = random.Random(2026)
    lines = []
    for _ in range(COUNT):
        text, compute = expression(generator, generator.randint(1, 5))
        try:
            value = compute()
            result = f"{value.numerator}/{value.denominator}"
        except Failure as failure:
            result = str(failure)
        lines.append(f"{text} = {result}")
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "tests/data/fraction.txt")
    path.write_text("\n".join(lines) + "\n")


main()
