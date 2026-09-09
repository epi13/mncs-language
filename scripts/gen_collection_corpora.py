#!/usr/bin/env python3
"""Generate backend corpora for chunk/sort/relation stdlib contracts.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde
format. Expectations are hand-derived from the contract math in
library/std/{chunk,sort,relation}.mncs and verified against the
reference executor before commit. Run from the repository root:

    python3 scripts/gen_collection_corpora.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")


def i64(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": True}}}


def u64(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": False}}}


def byte_seq(data: bytes):
    return {"sequence": {"values": [{"byte": {"value": b}} for b in data]}}


def u64_seq(values):
    return {"sequence": {"values": [u64(v) for v in values]}}


def case(cid, module, function, arguments, expected):
    return {
        "id": cid,
        "request": {
            "schema_version": "0.1",
            "target": {"module": module, "function": function},
            "arguments": arguments,
            "step_budget": 300000,
        },
        "expected": [i64(expected)],
    }


CHUNK = "mncs.std.chunk.v1"
chunk_cases = [
    # "ab\ncd": newline at 2; none from 3.
    case("find-newline", CHUNK, "candidate_find_newline",
         [byte_seq(b"ab\ncd"), u64(5)], 201),
    # "ab\ncd" from 0: span (0, 2), terminated, resume at 3.
    case("next-line", CHUNK, "candidate_next_line",
         [byte_seq(b"ab\ncd"), u64(5)], 2031),
    # 60-byte prefix + "ef\nxx": joined length 62, terminated.
    case("continue-line", CHUNK, "candidate_continue_line",
         [byte_seq(b"ef\nxx"), u64(5), u64(60)], 621),
]

SORT = "mncs.std.sort.v1"
sort_cases = [
    # [5,3,5,1,4,2,2,0] sorts ordered with 6 distinct.
    case("sort-dedup", SORT, "candidate_sort_dedup",
         [u64_seq([5, 3, 5, 1, 4, 2, 2, 0]), u64(8)], 16),
    # Reversed input converges: first 1, last 8.
    case("sort-reverse", SORT, "candidate_sort_reverse",
         [u64_seq([8, 7, 6, 5, 4, 3, 2, 1]), u64(8)], 18),
    # Empty window ordered with count 0; singleton distinct 1.
    case("sort-edges", SORT, "candidate_sort_edges",
         [u64(0), u64_seq([7, 0, 0, 0, 0, 0, 0, 0]), u64(1)], 1001),
]

RELATION = "mncs.std.relation.v1"
relation_cases = [
    case("chain", RELATION, "candidate_chain", [], 410400),
    case("diamond", RELATION, "candidate_diamond", [], 401040),
    case("cycle-dedup", RELATION, "candidate_cycle_dup", [], 301030),
    case("budget-truncation", RELATION, "candidate_budget", [], 31),
    case("missing-target-order", RELATION, "candidate_missing_order", [], 102029),
]


def main():
    for name, cases in (("chunk-corpus.json", chunk_cases),
                        ("sort-corpus.json", sort_cases),
                        ("relation-corpus.json", relation_cases)):
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
