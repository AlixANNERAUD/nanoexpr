#!/usr/bin/env python3
import decimal
import random
import sys
from pathlib import Path

DIGITS = 14
MAX_EXPONENT = 99
COUNT = 4000

context = decimal.Context(
    prec=DIGITS,
    rounding=decimal.ROUND_HALF_EVEN,
    Emax=MAX_EXPONENT,
    Emin=-MAX_EXPONENT,
    traps=[decimal.Overflow, decimal.DivisionByZero, decimal.InvalidOperation],
)


def operand(generator):
    digits = generator.randint(1, DIGITS)
    coefficient = generator.randrange(10 ** (digits - 1), 10**digits)
    if generator.random() < 0.1:
        coefficient = int("9" * digits)
    exponent = generator.randint(-MAX_EXPONENT, MAX_EXPONENT - digits + 1)
    if generator.random() < 0.5:
        exponent = generator.randint(-digits - 3, 3)
    sign = "-" if generator.random() < 0.5 else ""
    return decimal.Decimal(f"{sign}{coefficient}e{exponent}")


def outcome(operation, left, right):
    try:
        result = operation(left, right)
    except decimal.Overflow:
        return "Overflow"
    except decimal.DivisionByZero:
        return "DivisionByZero"
    except decimal.InvalidOperation:
        return None
    if result != 0 and result.adjusted() < -MAX_EXPONENT:
        return None
    return str(result)


def main():
    generator = random.Random(14)
    operations = {
        "+": context.add,
        "-": context.subtract,
        "*": context.multiply,
        "/": context.divide,
        "%": context.remainder,
    }
    lines = []
    while len(lines) < COUNT:
        symbol = generator.choice(list(operations))
        left = operand(generator)
        right = operand(generator)
        if generator.random() < 0.15:
            right = decimal.Decimal(f"{generator.choice('-+')}{left.copy_abs()}")
            right = right.scaleb(generator.randint(-16, 16))
        if generator.random() < 0.02:
            right = decimal.Decimal(0)
        if right != 0 and abs(right.adjusted()) > MAX_EXPONENT:
            continue
        result = outcome(operations[symbol], left, right)
        if result is not None:
            lines.append(f"{left} {symbol} {right} = {result}")
    for _ in range(500):
        digits = generator.randint(15, 40)
        literal = str(generator.randrange(10 ** (digits - 1), 10**digits))
        point = generator.randint(0, digits)
        literal = f"{literal[:point]}.{literal[point:]}e{generator.randint(-60, 60)}"
        lines.append(f"{literal} = {context.create_decimal(literal)}")
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "tests/data/decimal.txt")
    path.write_text("\n".join(lines) + "\n")


main()
