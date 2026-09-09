#!/usr/bin/env python3
"""Generate the bounded corpus for mncs.std.task.v1.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde format.
Task records carry the finite phase inline; expected phases are ordinary
finite values with their own identities. Cases cover the happy path
(start, step, step, finish), the operator-abort path (start, step, step,
cancel, terminate cleanly), and the invalid-transition witness.
Run from the repository root:

    python3 scripts/gen_task_corpus.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")

MODULE = "mncs.std.task.v1"
REC_ID = (
    f"mncs:0.2:record-type:{MODULE}::Task"
    "::cancel_requested%3Abool%3Bphase%3APhase%3Bstep_budget%3Au64%3Bsteps_taken%3Au64%3B"
)
PHASE_ID = f"mncs:0.2:finite-type:{MODULE}::Phase"


def uinteger(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": False}}}


def integer(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": True}}}


def boolean(value):
    return {"boolean": {"value": value}}


def phase(variant, discriminant):
    return {
        "finite": {
            "type_identity": PHASE_ID,
            "variant_identity": f"{PHASE_ID.replace('finite-type', 'finite-variant')}::{variant}",
            "discriminant": discriminant,
        }
    }


def task(cancel_requested, variant, discriminant, budget, taken):
    return {
        "record": {
            "type_identity": REC_ID,
            "name": "Task",
            "fields": [
                ["cancel_requested", boolean(cancel_requested)],
                ["phase", phase(variant, discriminant)],
                ["step_budget", uinteger(budget)],
                ["steps_taken", uinteger(taken)],
            ],
        }
    }


def case(case_id, function, arguments, expected, step_budget=4096):
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


cases = [
    case("lifecycle-completes", "candidate_lifecycle", [uinteger(3)],
         task(False, "DONE", 3, 3, 3)),
    case("lifecycle-empty-budget", "candidate_lifecycle", [uinteger(0)],
         task(False, "DONE", 3, 0, 0)),
    case("cancel-terminates-cleanly", "candidate_cancel", [uinteger(8)],
         task(True, "CANCELLED", 2, 8, 2)),
    case("invalid-transitions-rejected", "candidate_invalid", [], integer(0)),
]

corpus = {"schema_version": "0.1", "name": "task", "cases": cases}

with open(os.path.join(OUT, "task-corpus.json"), "w") as handle:
    json.dump(corpus, handle, indent=1)
    handle.write("\n")

print(f"wrote {len(cases)} cases")
