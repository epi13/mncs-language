#!/usr/bin/env python3
"""Stage C1 oracle: bit-exact binary64 expectations for float arithmetic.

Python floats are IEEE-754 binary64 evaluated in double precision on the
host, so every finite `+ - * /`, comparison, and conversion below matches
what a conforming backend must produce bit-for-bit. The MNCS float trap
rule is applied in the oracle itself: any operation with a non-finite
input or a non-finite result expects `runtime_failure`, never a value.

Regenerate: python3 scripts/gen_stage_c1_float_corpus.py
Output: examples/execution/stage-c1-float-corpus.json
"""

import json
import math
import struct

MODULE = "examples.stage.floats"
OUT = "examples/execution/stage-c1-float-corpus.json"


def bits(value: float) -> int:
    return struct.unpack("<Q", struct.pack("<d", value))[0]


def farg(value: float) -> dict:
    return {"float": {"bits": bits(value), "type": {"bits": 64}}}


def fexp(value: float) -> list:
    return [{"float": {"bits": bits(value), "type": {"bits": 64}}}]


def iarg(value: int, width: int = 64, signed: bool = True) -> dict:
    return {"integer": {"value": value, "type": {"bits": width, "signed": signed}}}


def iexp(value: int, width: int = 64, signed: bool = True) -> list:
    return [{"integer": {"value": value, "type": {"bits": width, "signed": signed}}}]


def bexp(value: bool) -> list:
    return [{"boolean": {"value": value}}]


def case(id_, function, args, expected, status="returned", budget=256):
    request = {
        "schema_version": "0.1",
        "target": {"module": MODULE, "function": function},
        "arguments": args,
        "step_budget": budget,
    }
    body = {"id": id_, "request": request, "expected_status": status}
    if expected is not None:
        body["expected"] = expected
    return body


def trap(id_, function, args=None):
    return case(id_, function, args or [], None, status="runtime_failure")


def main() -> None:
    cases = []
    # Core arithmetic over spellable and bit-supplied operands. Negative
    # operands arrive as corpus bits (source literals have no sign).
    pairs = [(1.5, 2.25), (0.1, 0.2), (-3.5, 1.25), (100.0, -0.5)]
    for index, (a, b) in enumerate(pairs):
        cases.append(case(f"add-{index}", "fadd", [farg(a), farg(b)], fexp(a + b)))
        cases.append(case(f"sub-{index}", "fsub", [farg(a), farg(b)], fexp(a - b)))
        cases.append(case(f"mul-{index}", "fmul", [farg(a), farg(b)], fexp(a * b)))
        cases.append(case(f"div-{index}", "fdiv", [farg(a), farg(b)], fexp(a / b)))
    for index, x in enumerate([2.0, -1.5, 0.25]):
        cases.append(case(f"chained-{index}", "fchained", [farg(x)], fexp(x * x + x)))
    # All six comparisons, including negative zero (which equals zero).
    cmp_pairs = [(1.0, 2.0), (2.0, 1.0), (1.5, 1.5), (-0.0, 0.0), (-2.0, -1.0)]
    ops = {
        "flt": lambda a, b: a < b,
        "fle": lambda a, b: a <= b,
        "fgt": lambda a, b: a > b,
        "fge": lambda a, b: a >= b,
        "feq": lambda a, b: a == b,
        "fne": lambda a, b: a != b,
    }
    for fname, op in ops.items():
        for index, (a, b) in enumerate(cmp_pairs):
            cases.append(case(f"{fname}-{index}", fname, [farg(a), farg(b)], bexp(op(a, b))))
    # Conversions: truncation toward zero, IEEE rounding into binary64.
    cases.append(case("f2i-trunc", "f2i", [farg(305.9)], iexp(305)))
    cases.append(case("f2i-neg", "f2i", [farg(-3.2)], iexp(-3)))
    cases.append(case("i2f-small", "i2f", [iarg(42)], fexp(42.0)))
    cases.append(case("i2f-neg", "i2f", [iarg(-7)], fexp(-7.0)))
    cases.append(case("i2f-max", "i2f", [iarg(2**63 - 1)], fexp(float(2**63 - 1))))
    cases.append(case("f2u-trunc", "f2u", [farg(255.9)], iexp(255, signed=False)))
    cases.append(case("u2f-small", "u2f", [iarg(42, signed=False)], fexp(42.0)))
    cases.append(case("u2f-max", "u2f", [iarg(2**64 - 1, signed=False)], fexp(float(2**64 - 1))))
    # Conversion traps: out-of-range and negative-to-unsigned.
    cases.append(trap("f2i-overflow", "f2i", [farg(1e20)]))
    cases.append(trap("f2u-neg", "f2u", [farg(-1.0)]))
    cases.append(trap("f2u-overflow", "f2u", [farg(1e20)]))
    # Computed boundaries: 1e20 squared three times is 1e160 (finite);
    # a fourth squaring overflows and traps.
    big = ((1e20**2) ** 2) ** 2
    assert big == 1e160 and math.isfinite(big)
    cases.append(case("big-finite", "big", [], fexp(big)))
    cases.append(trap("overflow-traps", "overflow"))
    # Gradual underflow down to a subnormal, then to zero (no trap:
    # zero is finite).
    tiny = 1e-20
    # Mirror under() exactly: s2 = tiny^2, s4 = s2^2, s8 = s4^2, under = s8^2.
    s2 = tiny**2
    s4 = s2**2
    s8 = s4**2
    under = s8**2
    assert under == 1e-320 and 0.0 < under < 2.2250738585072014e-308
    assert math.isfinite(under)
    cases.append(case("tiny-spelled", "tiny", [], fexp(tiny)))
    cases.append(case("under-subnormal", "under", [], fexp(under)))
    cases.append(case("under-to-zero", "under_to_zero", [], fexp(0.0)))
    cases.append(case("neg-zero", "negzero", [], fexp(-0.0)))
    assert bits(-0.0) == 0x8000000000000000
    # Trap rule: zero divisors and zero-by-zero never produce a value.
    cases.append(trap("div-zero", "divzero", [farg(5.0)]))
    cases.append(trap("zero-by-zero", "zerozero"))
    cases.append(case("entry-main", "main", [], fexp(3.5)))
    corpus = {
        "schema_version": "0.2",
        "name": "stage-c1-float-arithmetic",
        "cases": cases,
        "properties": [],
    }
    with open(OUT, "w", encoding="utf-8") as handle:
        json.dump(corpus, handle, indent=2)
        handle.write("\n")
    print(f"wrote {OUT} with {len(cases)} cases")


if __name__ == "__main__":
    main()
