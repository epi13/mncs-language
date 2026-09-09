#!/usr/bin/env python3
"""Generate pressure corpora from independent oracles (Python stdlib only)."""
import json
import struct


def fbits(x: float) -> int:
    return struct.unpack("<Q", struct.pack("<d", x))[0]


def farg(bits: int) -> dict:
    return {"float": {"bits": bits, "type": {"bits": 64}}}


def fexp(bits: int) -> list:
    return [{"float": {"bits": bits, "type": {"bits": 64}}}]


def iarg(value: int, signed: bool) -> dict:
    return {"integer": {"value": value, "type": {"bits": 64, "signed": signed}}}


def iexp(value: int, signed: bool) -> list:
    return [{"integer": {"value": value, "type": {"bits": 64, "signed": signed}}}]


def barg(value: bool) -> dict:
    return {"boolean": {"value": value}}


def bexp(value: bool) -> list:
    return [{"boolean": {"value": value}}]


def left_fold(xs):
    s = 0.0
    for x in xs:
        s = s + x
    return s


# --- f64 arrays ---
fcases = []


def fcase(cid, func, xs):
    args = [farg(fbits(x)) for x in xs]
    exp = fexp(fbits(left_fold(xs)))
    fcases.append(
        {
            "id": cid,
            "request": {
                "schema_version": "0.1",
                "target": {"module": "examples.pressure.f64arrays", "function": func},
                "arguments": args,
                "step_budget": 4096,
            },
            "expected": exp,
        }
    )


fcase("f2-basic", "entry_f2", [1.0, 2.0])
fcase("f4-basic", "entry_f4", [1.0, 2.0, 3.0, 4.0])
fcase("f6-basic", "entry_f6", [1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
fcase("f8-basic", "entry_f8", [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0])
fcase("local6-basic", "entry_local6", [1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
fcase("frac-f4", "entry_f4", [0.1, 0.2, 0.3, 0.4])
fcase("large-loop6", "entry_local6", [1e16, 1e16, -1e16, 2.0, -1e16, 1.0])
# Negative zero: bits 0x8000000000000000 + 0x0 -> +0.0
negzero = struct.unpack("<d", struct.pack("<Q", 0x8000000000000000))[0]
fcase("negzero-f2", "entry_f2", [negzero, 0.0])
# Subnormal: bits 1 + bits 1 = bits 2 (9.8e-324), still finite
sub1 = struct.unpack("<d", struct.pack("<Q", 1))[0]
fcase("subnormal-f2", "entry_f2", [sub1, sub1])
# Indexed read: a[0] + a[3]
fcases.append(
    {
        "id": "index4",
        "request": {
            "schema_version": "0.1",
            "target": {"module": "examples.pressure.f64arrays", "function": "entry_index4"},
            "arguments": [farg(fbits(x)) for x in [10.0, 20.0, 30.0, 40.0]],
            "step_budget": 4096,
        },
        "expected": fexp(fbits(50.0)),
    }
)

with open("examples/execution/pressure-f64-arrays-corpus.json", "w") as f:
    json.dump({"schema_version": "0.1", "name": "pressure-f64-arrays", "cases": fcases}, f, indent=1)

# --- u64 high-bit ---
MAX = 18446744073709551615
P63 = 9223372036854775808
ucases = []


def ucase(cid, func, args, exp, signed=False, trap=False):
    c = {
        "id": cid,
        "request": {
            "schema_version": "0.1",
            "target": {"module": "examples.pressure.u64highbit", "function": func},
            "arguments": args,
            "step_budget": 4096,
        },
    }
    if not trap:
        c["expected"] = exp
    ucases.append(c)


ucase("add-0-max", "entry_add", [iarg(0, False), iarg(MAX, False)], iexp(MAX, False))
ucase("add-p63-1", "entry_add", [iarg(P63, False), iarg(1, False)], iexp(P63 + 1, False))
ucase("add-max-0", "entry_add", [iarg(MAX, False), iarg(0, False)], iexp(MAX, False))
ucase("sub-max-max", "entry_sub", [iarg(MAX, False), iarg(MAX, False)], iexp(0, False))
ucase("sub-p63-p63m1", "entry_sub", [iarg(P63, False), iarg(P63 - 1, False)], iexp(1, False))
ucase("wrap-max-1", "entry_wrap", [iarg(MAX, False), iarg(1, False)], iexp(0, False))
ucase(
    "ge-max-p63",
    "entry_ge",
    [iarg(MAX, False), iarg(P63, False)],
    bexp(True),
)
ucase(
    "addm-0-0-max",
    "entry_addm",
    [iarg(0, False), iarg(0, False), iarg(MAX, False)],
    iexp(0, False),
)
ucase(
    "minmod",
    "entry_minmod",
    [iarg(-9223372036854775808, True), iarg(-1, True)],
    iexp(0, True),
)

with open("examples/execution/pressure-u64-highbit-corpus.json", "w") as f:
    json.dump({"schema_version": "0.1", "name": "pressure-u64-highbit", "cases": ucases}, f, indent=1)

# --- symbols ---
scases = []


def scase(cid, func, args, exp):
    scases.append(
        {
            "id": cid,
            "request": {
                "schema_version": "0.1",
                "target": {"module": "examples.pressure.symbols", "function": func},
                "arguments": args,
                "step_budget": 4096,
            },
            "expected": exp,
        }
    )


scase("trunc-5", "entry_trunc", [iarg(5, True)], iexp(5, True))
scase("min-3-7", "entry_min", [iarg(3, True), iarg(7, True)], iexp(3, True))
scase("exp-9", "entry_exp", [iarg(9, True)], iexp(9, True))

with open("examples/execution/pressure-symbol-hygiene-corpus.json", "w") as f:
    json.dump({"schema_version": "0.1", "name": "pressure-symbol-hygiene", "cases": scases}, f, indent=1)

print(f"wrote {len(fcases)} f64, {len(ucases)} u64, {len(scases)} symbol cases")
