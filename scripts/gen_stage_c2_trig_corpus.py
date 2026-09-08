#!/usr/bin/env python3
"""Stage C2 oracle: bit-exact binary64 expectations for sin/cos.

Python floats are IEEE-754 binary64 and CPython's math.sin/math.cos call
the host libm directly, so every finite expectation below matches what a
conforming backend must produce bit-for-bit: MNCS routes `sin`/`cos` to
the same host libm on every executable backend. The MNCS float trap
rule is applied in the oracle itself: any intrinsic with a non-finite
operand expects `runtime_failure`, never a value. (Finite operands
always yield results in [-1, 1], so the post-call guard only fires when
the operand guard already did.)

Regenerate: python3 scripts/gen_stage_c2_trig_corpus.py
Output: examples/execution/stage-c2-trig-corpus.json
"""

import json
import math
import struct

MODULE = "examples.stage.trig"
OUT = "examples/execution/stage-c2-trig-corpus.json"


def bits(value: float) -> int:
    return struct.unpack("<Q", struct.pack("<d", value))[0]


def farg(value: float) -> dict:
    return {"float": {"bits": bits(value), "type": {"bits": 64}}}


def fexp(value: float) -> list:
    return [{"float": {"bits": bits(value), "type": {"bits": 64}}}]


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
    # Quadrant coverage, negatives (corpus bits; source literals have no
    # sign), negative zero, near-multiples of pi, and large magnitudes
    # that stress argument reduction.
    angles = [
        0.0,
        0.5,
        1.0,
        -1.0,
        0.1,
        2.5,
        -2.5,
        -0.0,
        math.pi / 2,
        math.pi,
        3.0,
        10.0,
        -10.0,
        1e10,
        -1e10,
    ]
    for index, x in enumerate(angles):
        cases.append(case(f"sin-{index}", "fsin", [farg(x)], fexp(math.sin(x))))
        cases.append(case(f"cos-{index}", "fcos", [farg(x)], fexp(math.cos(x))))
    assert bits(math.sin(-0.0)) == 0x8000000000000000
    # Chained, nested, and identity forms over a smaller angle set.
    # The oracle mirrors source evaluation order exactly.
    chained = [0.5, 1.0, -2.5, 10.0]
    for index, x in enumerate(chained):
        cases.append(
            case(f"sincos-{index}", "sincos", [farg(x)], fexp(math.sin(x) + math.cos(x)))
        )
        cases.append(
            case(
                f"sinofcos-{index}",
                "sin_of_cos",
                [farg(x)],
                fexp(math.sin(math.cos(x))),
            )
        )
        s = math.sin(x)
        c = math.cos(x)
        cases.append(
            case(f"pythag-{index}", "pythag", [farg(x)], fexp(s * s + c * c))
        )
    # Trap rule: non-finite operands never produce a value. Corpus bits
    # can carry infinities and NaN directly; every backend guards the
    # operand before the libm call.
    cases.append(trap("sin-posinf", "fsin", [farg(math.inf)]))
    cases.append(trap("sin-neginf", "fsin", [farg(-math.inf)]))
    cases.append(trap("cos-posinf", "fcos", [farg(math.inf)]))
    cases.append(trap("cos-neginf", "fcos", [farg(-math.inf)]))
    cases.append(trap("sin-nan", "fsin", [farg(math.nan)]))
    cases.append(trap("cos-nan", "fcos", [farg(math.nan)]))
    cases.append(
        case("entry-main", "main", [], fexp(math.sin(0.5) + math.cos(0.5)))
    )
    corpus = {
        "schema_version": "0.2",
        "name": "stage-c2-trig",
        "cases": cases,
        "properties": [],
    }
    with open(OUT, "w", encoding="utf-8") as handle:
        json.dump(corpus, handle, indent=2)
        handle.write("\n")
    print(f"wrote {OUT} with {len(cases)} cases")


if __name__ == "__main__":
    main()
