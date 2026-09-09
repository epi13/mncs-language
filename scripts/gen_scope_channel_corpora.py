#!/usr/bin/env python3
"""Generate backend corpora for mncs.std.scope.v1 and mncs.std.channel.v1.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde format.
Every case calls a zero-argument `candidate_*` entry point and pins its
scalar witness (see library/std/scope.mncs and library/std/channel.mncs
for the packing contracts). Expectations are hand-derived from the
contract math and verified against the reference executor before commit.
Run from the repository root:

    python3 scripts/gen_scope_channel_corpora.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")


def integer(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": True}}}


def case(cid, module, function, expected):
    return {
        "id": cid,
        "request": {
            "schema_version": "0.1",
            "target": {"module": module, "function": function},
            "arguments": [],
            "step_budget": 131072,
        },
        "expected": [integer(expected)],
    }


SCOPE = "mncs.std.scope.v1"
scope_cases = [
    # Two tasks complete out of order; join packs by id: 11*100+22.
    case("task-join-is-id-ordered", SCOPE, "candidate_task_join", 1122),
    # Spawn accepted, then close; late spawn and settle rejected: 100.
    case("scope-close-rejects-escape", SCOPE, "candidate_scope_escape", 100),
    # FAILED(2) code 7 survives sibling CANCELLED(3); close accepted.
    case("failure-distinct-from-cancel", SCOPE, "candidate_failure", 20731),
    # Three cleanups run; post-cancel success rejected: 30.
    case("cancel-blocks-success", SCOPE, "candidate_cancel_cleanup", 30),
    # Only the two legal steps accepted: 100010.
    case("settle-rejections", SCOPE, "candidate_settle_rejected", 100010),
]

CHANNEL = "mncs.std.channel.v1"
channel_cases = [
    # 2 producers / 1 consumer / capacity 2: FIFO sum 36, backpressure,
    # close-once, drained close observed, no sentinel.
    case("bounded-flow-with-backpressure", CHANNEL, "candidate_channel_flow", 361121),
    # Empty recv is EMPTY; early close rejected and smuggles nothing.
    case("empty-and-early-close", CHANNEL, "candidate_channel_empty", 102),
    # Buffered item drains before the observed close; sends/done refused.
    case("send-after-close", CHANNEL, "candidate_send_closed", 11002),
]


def main():
    for name, cases in (("scope-corpus.json", scope_cases), ("channel-corpus.json", channel_cases)):
        path = os.path.join(OUT, name)
        with open(path, "w", encoding="utf-8") as handle:
            json.dump(
                {"schema_version": "0.1", "name": name.removesuffix("-corpus.json"), "cases": cases},
                handle,
                indent=1,
            )
            handle.write("\n")
        print(f"wrote {path} ({len(cases)} cases)")


if __name__ == "__main__":
    main()
