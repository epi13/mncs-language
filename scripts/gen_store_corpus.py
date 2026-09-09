#!/usr/bin/env python3
"""Generate the backend corpus for mncs.std.store.v1.

Expectations are hand-derived from the contract math in
library/std/store.mncs and verified against the reference executor
before commit. Run from the repository root:

    python3 scripts/gen_store_corpus.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")

MODULE = "mncs.std.store.v1"


def i64(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": True}}}


def case(cid, function, expected):
    return {
        "id": cid,
        "request": {
            "schema_version": "0.1",
            "target": {"module": MODULE, "function": function},
            "arguments": [],
            "step_budget": 200000,
        },
        "expected": [i64(expected)],
    }


cases = [
    # Two writers race from base 0: one commits 11 at generation 1,
    # the stale write observes 1.
    case("cas-race", "candidate_cas_race", 11011),
    # The pinned handle reads generation 1 / value 11 after two more
    # publications; the released handle reports released.
    case("snapshot-isolation", "candidate_snapshot_isolation", 1113),
    # Retention scans with different pinned sets.
    case("reclaim", "candidate_reclaim", 40300),
    # PROCESS receipt without power-loss claims.
    case("durability-receipt", "candidate_durability_receipt", 111),
]


def main():
    path = os.path.join(OUT, "store-corpus.json")
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(
            {"schema_version": "0.1", "name": "store", "cases": cases},
            handle,
            indent=1,
        )
        handle.write("\n")
    print(f"wrote {path} ({len(cases)} cases)")


if __name__ == "__main__":
    main()
