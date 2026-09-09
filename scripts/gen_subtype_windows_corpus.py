#!/usr/bin/env python3
"""Generate the exact-to-view borrow corpus.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde format
for examples/source/subtype-windows.mncs: one shared little-endian reader
(mncs.std.encoding.v1) serves 44/46/22-byte exact windows through the
automatic exact-to-bounded-view borrow. Expectations are computed here from
the staged window bytes, independently of any backend.
Run from the repository root:

    python3 scripts/gen_subtype_windows_corpus.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution", "subtype-windows-corpus.json")

MODULE = "examples.subtype.windows"


def byte(value):
    return {"byte": {"value": value}}


def window(width):
    return {"sequence": {"values": [byte(i % 256) for i in range(width)]}}


def uinteger(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": False}}}


def u16_value(value):
    return {"integer": {"value": value, "type": {"bits": 16, "signed": False}}}


def u32_value(value):
    return {"integer": {"value": value, "type": {"bits": 32, "signed": False}}}


def le16(data, offset):
    return data[offset] + data[offset + 1] * 256


def le32(data, offset):
    return (
        data[offset]
        + data[offset + 1] * 256
        + data[offset + 2] * 65536
        + data[offset + 3] * 16777216
    )


def case(case_id, function, arguments, expected, step_budget=8192, module=MODULE):
    return {
        "id": case_id,
        "request": {
            "schema_version": "0.1",
            "step_budget": step_budget,
            "target": {"module": module, "function": function},
            "arguments": arguments,
        },
        "expected_status": "returned",
        "expected": expected,
    }


def main():
    header44 = [i % 256 for i in range(44)]
    window46 = [i % 256 for i in range(46)]
    eocd22 = [i % 256 for i in range(22)]
    cases = [
        case("read44", "read44", [window(44)], [u16_value(le16(header44, 20))]),
        case("read46", "read46", [window(46)], [u16_value(le16(window46, 20))]),
        case("read22", "read22", [window(22)], [u16_value(le16(eocd22, 8))]),
        case("sig32", "sig32", [window(46)], [u32_value(le32(window46, 0))]),
        case(
            "nested",
            "nested",
            [window(44)],
            [u16_value(le16(header44, le16(header44, 20) % 40))],
        ),
        case("via_call", "via_call", [window(44)], [u16_value(le16(header44, 20))]),
        case("via_let", "via_let", [window(44)], [u16_value(le16(header44, 0))]),
        case(
            "via_return",
            "via_return",
            [window(44)],
            [{"sequence": {"values": [byte(v) for v in header44]}}],
        ),
        # Direct stdlib reader calls over staged views (no borrow involved).
        case(
            "direct_u16",
            "read_u16_le",
            [{"sequence": {"values": [byte(10), byte(20), byte(30), byte(40)]}},
             uinteger(1)],
            [u16_value(20 + 30 * 256)],
            module="mncs.std.encoding.v1",
        ),
        case(
            "direct_u32",
            "read_u32_le",
            [{"sequence": {"values": [byte(10), byte(20), byte(30), byte(40)]}},
             uinteger(0)],
            [u32_value(10 + 20 * 256 + 30 * 65536 + 40 * 16777216)],
            module="mncs.std.encoding.v1",
        ),
    ]
    corpus = {"schema_version": "0.1", "name": "subtype-windows", "cases": cases}
    with open(OUT, "w") as handle:
        json.dump(corpus, handle, indent=1)
        handle.write("\n")
    print(f"wrote {OUT} ({len(cases)} cases)")


if __name__ == "__main__":
    main()
