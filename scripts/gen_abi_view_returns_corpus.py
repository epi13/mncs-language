#!/usr/bin/env python3
"""Generate the sequence/view-return composability corpus.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde format
for examples/source/abi-view-returns.mncs. Every expectation is a logical
value computed here independently of any backend: backends must agree on
the value, not merely the status. Trap cases (out-of-bounds index,
reversed slice) carry no expectation; the harness pins their failure
status separately per backend.
Run from the repository root:

    python3 scripts/gen_abi_view_returns_corpus.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution", "abi-view-returns-corpus.json")
TRAP_OUT = os.path.join(ROOT, "examples", "execution", "abi-view-returns-traps-corpus.json")

MODULE = "examples.abi.view_returns"


def boolean(value):
    return {"boolean": {"value": value}}


def uinteger(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": False}}}


def byte(value):
    return {"byte": {"value": value}}


def byte_seq(values):
    return {"sequence": {"values": [byte(v) for v in values]}}


def u64_seq(values):
    return {"sequence": {"values": [uinteger(v) for v in values]}}


def case(case_id, function, arguments, expected, step_budget=4096):
    entry = {
        "id": case_id,
        "request": {
            "schema_version": "0.1",
            "step_budget": step_budget,
            "target": {"module": MODULE, "function": function},
            "arguments": arguments,
        },
    }
    if expected is not None:
        entry["expected_status"] = "returned"
        entry["expected"] = expected
    return entry


def digit_pair(value):
    return [((value // 10) % 10) + 48, (value % 10) + 48]


def main():
    bar, beat = 7, 3
    window = [5, 6, 7, 8]
    wide = [100, 200, 300, 400]
    cases = [
        # P-013 regression vectors: slices of a locally-built exact sequence.
        case("s02", "slice02", [uinteger(bar), uinteger(beat)],
             [byte_seq(digit_pair(bar))]),
        case("s13", "slice13", [uinteger(bar), uinteger(beat)],
             [byte_seq([bar % 10 + 48, 46])]),
        # Repeated call with new arguments (allocator reuse across calls).
        case("s02_repeat", "slice02", [uinteger(12), uinteger(34)],
             [byte_seq(digit_pair(12))]),
        # Slices of a host-staged (packed) parameter view.
        case("param_tail", "tail2", [byte_seq(window)], [byte_seq([6, 7])]),
        case("param_mid_even", "mid_even", [byte_seq(window)], [byte_seq([7, 8])]),
        case("param_head", "head2", [byte_seq(window)], [byte_seq([5, 6])]),
        case("param_empty", "empty_mid", [byte_seq(window)], [byte_seq([])]),
        # Exact sequence returns: length 1, 3, and the profile maximum.
        case("exact1", "exact1", [], [byte_seq([65])]),
        case("exact3", "exact3", [], [byte_seq([10, 20, 30])]),
        case("exact64", "exact64", [], [byte_seq(list(range(64)))]),
        # Maximum-bound view over a local exact sequence.
        case("full64", "full64", [], [byte_seq(list(range(64)))]),
        # Odd-start slice of a cell-backed (exact-derived) view.
        case("odd_cell_slice", "odd_cell_slice", [], [byte_seq([6, 7, 8])]),
        # Internal consumption of derived views at odd and even offsets.
        case("index_odd", "index_odd", [byte_seq(window)], [uinteger(6)]),
        case("index_odd_second", "index_odd_second", [byte_seq(window)],
             [uinteger(7)]),
        case("index_cell_odd", "index_cell_odd", [], [uinteger(8)]),
        # Non-byte element views (cell-strided on every backend).
        case("wide_slice", "wide_slice", [u64_seq(wide)], [u64_seq([200, 300])]),
        case("wide_exact", "wide_exact", [], [u64_seq(wide)]),
        # Exact sequence input (canonical cells staged by the host).
        case("exact_input", "exact_second", [byte_seq([9, 7, 5, 3])], [uinteger(7)]),
        # Scalar consumption through a dynamic index.
        case("pick", "pick", [byte_seq(window), uinteger(3)], [uinteger(8)]),
    ]
    # Trap shapes live in a separate corpus with no expectations; the harness
    # pins their runtime_failure status per backend instead of values.
    traps = [
        case("pick_oob", "pick", [byte_seq(window), uinteger(9)], None),
        case("rev_slice", "rev_slice", [byte_seq(window)], None),
    ]
    corpus = {"schema_version": "0.1", "name": "abi-view-returns", "cases": cases}
    with open(OUT, "w") as handle:
        json.dump(corpus, handle, indent=1)
        handle.write("\n")
    trap_corpus = {
        "schema_version": "0.1",
        "name": "abi-view-returns-traps",
        "cases": traps,
    }
    with open(TRAP_OUT, "w") as handle:
        json.dump(trap_corpus, handle, indent=1)
        handle.write("\n")
    print(f"wrote {OUT} ({len(cases)} cases)")
    print(f"wrote {TRAP_OUT} ({len(traps)} cases)")


if __name__ == "__main__":
    main()
