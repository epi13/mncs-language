#!/usr/bin/env python3
"""Generate the byte-exact corpus for mncs.std.json_emit.v1.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde format.
Expected writers are computed independently here in Python: the golden
bytes are the contract, not the module's own output. Every expectation
carries the full 64-byte buffer plus len/depth/first/ok so capacity,
closure, and failure behavior are pinned, not just the happy-path prefix.
Run from the repository root:

    python3 scripts/gen_json_emit_corpus.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")

MODULE = "mncs.std.json_emit.v1"
# Record identities carry the canonical (name-sorted) field shape, matching
# what the compiler reports for returned records.
REC_ID = (
    f"mncs:0.2:record-type:{MODULE}::Writer"
    "::buf%3A%5Bbyte%3B%2064%5D%3Bdepth%3Au64%3Bfirst%3Abool%3Blen%3Au64%3Bok%3Abool%3B"
)


def byte(value):
    return {"byte": {"value": value}}


def uinteger(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": False}}}


def boolean(value):
    return {"boolean": {"value": value}}


def integer(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": True}}}


def view(text):
    return {"sequence": {"values": [byte(b) for b in text]}}


def exact(text, width):
    assert len(text) <= width, (text, width)
    return {"sequence": {"values": [byte(b) for b in text] + [byte(0)] * (width - len(text))}}


def writer(expected: bytes, depth=0, first=False, ok=True):
    assert len(expected) <= 64, len(expected)
    buf = list(expected) + [0] * (64 - len(expected))
    return {
        "record": {
            "type_identity": REC_ID,
            "name": "Writer",
            "fields": [
                ["buf", {"sequence": {"values": [byte(b) for b in buf]}}],
                ["depth", uinteger(depth)],
                ["first", boolean(first)],
                ["len", uinteger(len(expected))],
                ["ok", boolean(ok)],
            ],
        }
    }


def writer_overflow(attempted: bytes, depth=0):
    """Capacity failure: the first 64 bytes land, len sticks at 64, ok is false."""
    buf = list(attempted[:64])
    return {
        "record": {
            "type_identity": REC_ID,
            "name": "Writer",
            "fields": [
                ["buf", {"sequence": {"values": [byte(b) for b in buf]}}],
                ["depth", uinteger(depth)],
                ["first", boolean(False)],
                ["len", uinteger(64)],
                ["ok", boolean(False)],
            ],
        }
    }


def case(case_id, function, arguments, expected, step_budget=16384):
    return {
        "id": case_id,
        "request": {
            "schema_version": "0.1",
            "target": {"module": MODULE, "function": function},
            "arguments": list(arguments),
            "step_budget": step_budget,
        },
        "expected": [expected],
    }


ROW = b'{"model":"qwen3:8b","score":820,"ok":true}'
ARR = b'[1,-2,300]'
ESC = b'"a\\"b\\\\c\\u0001"'
MINUS_ONE = b'[-1]'
MIN_I64 = b'[-9223372036854775808]'
MAX_I64 = b'[9223372036854775807]'
ZERO = b'[0]'
NULL = b'null'
OVERFLOW_ATTEMPTED = b'[' + b'-9223372036854775808,' * 3 + b'-9223372036'

cases = [
    case("metrics-row", "candidate_metrics_row",
         [exact(b"qwen3:8b", 8), uinteger(8), integer(820), boolean(True)],
         writer(ROW)),
    case("int-array", "candidate_array",
         [{"sequence": {"values": [integer(1), integer(-2), integer(300)]}}],
         writer(ARR)),
    case("escaped-string", "candidate_escaped",
         [exact(b'a"b\\c\x01', 8), uinteger(6)], writer(ESC)),
    case("null", "candidate_null", [], writer(NULL)),
    case("stray-end-fails-closed", "candidate_stray_end", [],
         writer(b"", depth=0, first=True, ok=False)),
    # Three minima fill exactly 64 bytes: the capacity boundary holds.
    case("boundary-exact", "candidate_array",
         [{"sequence": {"values": [integer(-9223372036854775808)] * 3}}],
         writer(b'[-9223372036854775808,-9223372036854775808,-9223372036854775808]')),
    # A 64-byte string needs 66 bytes with quotes: capacity poisons.
    case("overflow-poisons", "candidate_long_string",
         [exact(b"a" * 64, 64), uinteger(64)],
         writer_overflow(b'"' + b"a" * 64 + b'"')),
]

corpus = {"schema_version": "0.1", "name": "json-emit", "cases": cases}

with open(os.path.join(OUT, "json-emit-corpus.json"), "w") as handle:
    json.dump(corpus, handle, indent=1)
    handle.write("\n")

print(f"wrote {len(cases)} cases")
