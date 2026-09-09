#!/usr/bin/env python3
"""Generate the bounded corpus for the wall-clock capability boundary.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde format.
Expectations pin only relational booleans (deadline 0 is always past,
1e12 ms is always future) and synthetic-argument pure results from
`mncs.std.clock.v1`; absolute instants are never pinned, since the
executor observes its own wall clock. Effect expectations pin the
realized clock_read observation (kind/target/capability) with prohibition
of anything unexpected.
Run from the repository root:

    python3 scripts/gen_clock_corpus.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")

MODULE = "examples.clock_scan"


def uinteger(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": False}}}


def boolean(value):
    return {"boolean": {"value": value}}


def clock_effect():
    return {
        "expected_effects": [
            {"kind": "clock_read", "target": "clock_read", "capability": "ticker"}
        ],
        "prohibit_unexpected_effects": True,
    }


def no_effects():
    return {
        "expected_effects": [],
        "prohibit_unexpected_effects": True,
    }


def case(case_id, function, arguments, expected, effects, step_budget=8192):
    body = {
        "id": case_id,
        "request": {
            "schema_version": "0.1",
            "target": {"module": MODULE, "function": function},
            "arguments": list(arguments),
            "step_budget": step_budget,
        },
        "expected": [expected],
    }
    body.update(effects)
    return body


cases = [
    # Deadline 0 is before every observable instant: always expired.
    case("past-expired", "past_expired", [], boolean(True), clock_effect()),
    # 1e15 ms (~31 millennia) is after every observable instant: never expired.
    case("future-pending", "future_pending", [], boolean(False), clock_effect()),
    # Pure stdlib relations over synthetic arguments: byte-exact.
    case("elapsed-known", "elapsed_known", [], uinteger(100), no_effects()),
    case("elapsed-floored", "elapsed_floored", [], uinteger(0), no_effects()),
    case("fresh-window", "fresh_window", [], boolean(True), no_effects()),
    case("stale-window", "stale_window", [], boolean(False), no_effects()),
]

corpus = {"schema_version": "0.1", "name": "clock-scan", "cases": cases}

with open(os.path.join(OUT, "clock-scan-corpus.json"), "w") as handle:
    json.dump(corpus, handle, indent=1)
    handle.write("\n")

print(f"wrote {len(cases)} cases")
