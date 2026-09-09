#!/usr/bin/env python3
"""Generate the cross-module nominal-type corpus.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde format
for examples/source/test/nominal/consumer.mncs (which imports
test.nominal.machine). Every expectation carries the DECLARING module's
nominal identities: the consumer must observe the same Phase/Transport
identities, not structural copies. Finite discriminants are declaration
order (stopped 0, playing 1, paused 2; play 0, pause 1, stop 2).
Run from the repository root:

    python3 scripts/gen_import_nominal_corpus.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution", "import-nominal-corpus.json")

MODULE = "test.nominal.consumer"
MACHINE = "test.nominal.machine"

PHASE_ID = f"mncs:0.2:finite-type:{MACHINE}::Phase"
EVT_ID = f"mncs:0.2:finite-type:{MACHINE}::Evt"
TRANSPORT_ID = (
    f"mncs:0.2:record-type:{MACHINE}::Transport::armed%3Abool%3Bphase%3APhase%3B"
)

PHASE = {"stopped": 0, "playing": 1, "paused": 2}


def boolean(value):
    return {"boolean": {"value": value}}


EVT = {"play": 0, "pause": 1, "stop": 2}


def phase(variant):
    return {
        "finite": {
            "type_identity": PHASE_ID,
            "variant_identity": f"mncs:0.2:finite-variant:{MACHINE}::Phase::{variant}",
            "discriminant": PHASE[variant],
            "payload": [],
        }
    }


def evt(variant):
    return {
        "finite": {
            "type_identity": EVT_ID,
            "variant_identity": f"mncs:0.2:finite-variant:{MACHINE}::Evt::{variant}",
            "discriminant": EVT[variant],
            "payload": [],
        }
    }


def transport(phase_variant, armed):
    return {
        "record": {
            "type_identity": TRANSPORT_ID,
            "name": "Transport",
            "fields": [
                ["armed", boolean(armed)],
                ["phase", phase(phase_variant)],
            ],
        }
    }


def case(case_id, function, arguments, expected, step_budget=8192):
    return {
        "id": case_id,
        "request": {
            "schema_version": "0.1",
            "step_budget": step_budget,
            "target": {"module": MODULE, "function": function},
            "arguments": arguments,
        },
        "expected_status": "returned",
        "expected": expected,
    }


def main():
    playing_transport = transport("playing", True)
    cases = [
        case("launch", "launch", [boolean(True)], [transport("playing", True)]),
        case("launch_unarmed", "launch", [boolean(False)],
             [transport("playing", False)]),
        case("launch_qualified", "launch_qualified", [boolean(True)],
             [transport("playing", True)]),
        case("describe_idle", "describe", [transport("stopped", True)],
             [boolean(False)]),
        case("describe_playing", "describe", [transport("playing", False)],
             [boolean(True)]),
        case("phase_of", "phase_of", [transport("paused", True)],
             [phase("paused")]),
        case("cycle", "cycle", [playing_transport, evt("pause")],
             [transport("stopped", False)]),
        case("disarm_after_stop", "disarm_after_stop", [playing_transport],
             [boolean(False)]),
        case("arm_then_playing", "arm_then_playing", [boolean(False)],
             [boolean(True)]),
        case("echo_it", "echo_it", [phase("playing")], [phase("playing")]),
    ]
    corpus = {"schema_version": "0.1", "name": "import-nominal", "cases": cases}
    with open(OUT, "w") as handle:
        json.dump(corpus, handle, indent=1)
        handle.write("\n")
    print(f"wrote {OUT} ({len(cases)} cases)")


if __name__ == "__main__":
    main()
