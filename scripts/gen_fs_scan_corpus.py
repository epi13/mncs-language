#!/usr/bin/env python3
"""Generate the granted-filesystem corpus for examples/source/fs-scan.mncs.

Expectations are derived from the committed fixture tree
(examples/fs-fixture) by an independent walk: byte-sorted relative
paths, kinds, and exact file bytes. The executor under test performs
its own walk; the corpus pins that the two agree on counts, names,
kinds, and chunked content (including multi-chunk files, short final
reads, and empty end-of-input reads).

Generation values are wall data and are NEVER pinned here: relational
generation behavior (stable across polls, moves on mutation) is proven
by crates/mncs-cli/tests/fs_effects.rs instead.
Run from the repository root:

    python3 scripts/gen_fs_scan_corpus.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")
FIXTURE = os.path.join(ROOT, "examples", "fs-fixture")

MODULE = "examples.fs_scan"


def uinteger(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": False}}}


def byte_seq(data: bytes):
    return {"sequence": {"values": [{"byte": {"value": b}} for b in data]}}


def walk():
    entries = []
    stack = [""]
    while stack:
        rel = stack.pop()
        path = os.path.join(FIXTURE, rel) if rel else FIXTURE
        children = sorted(os.listdir(path))
        for name in children:
            child_rel = f"{rel}/{name}" if rel else name
            full = os.path.join(path, name)
            if os.path.islink(full):
                kind = 2
            elif os.path.isdir(full):
                kind = 1
                stack.append(child_rel)
            elif os.path.isfile(full):
                kind = 0
            else:
                kind = 2
            entries.append((child_rel, kind))
    entries.sort(key=lambda item: item[0].encode())
    return entries


def list_effect():
    return [{"kind": "fs_list", "target": "dir_list", "capability": "fs_root"}]


def read_effect():
    return [{"kind": "fs_read", "target": "file_read", "capability": "fs_root"}]


def case(cid, function, arguments, expected, effects):
    return {
        "id": cid,
        "request": {
            "schema_version": "0.1",
            "target": {"module": MODULE, "function": function},
            "arguments": arguments,
            "step_budget": 131072,
        },
        "expected": expected,
        "expected_effects": effects,
        "prohibit_unexpected_effects": True,
    }


def main():
    entries = walk()
    rels = [rel for rel, _ in entries]
    kinds = [kind for _, kind in entries]
    cases = [case("entry-count", "entry_count", [], [uinteger(len(entries))], list_effect())]
    for index, (rel, kind) in enumerate(entries):
        cases.append(
            case(f"name-{index}", "entry_name", [uinteger(index)],
                 [byte_seq(rel.encode())], list_effect())
        )
        cases.append(
            case(f"kind-{index}", "entry_kind", [uinteger(index)],
                 [uinteger(kind)], list_effect())
        )
    # Chunked reads over the multi-chunk file sub/b.txt (156 bytes).
    blob_index = rels.index("sub/b.txt")
    blob = open(os.path.join(FIXTURE, "sub/b.txt"), "rb").read()
    assert len(blob) == 156, len(blob)
    cases.append(
        case("read-first-chunk", "read_chunk",
             [uinteger(blob_index), uinteger(0), uinteger(64)],
             [byte_seq(blob[0:64])], read_effect())
    )
    cases.append(
        case("read-second-chunk", "read_chunk",
             [uinteger(blob_index), uinteger(64), uinteger(64)],
             [byte_seq(blob[64:128])], read_effect())
    )
    cases.append(
        case("read-short-tail", "read_chunk",
             [uinteger(blob_index), uinteger(128), uinteger(64)],
             [byte_seq(blob[128:156])], read_effect())
    )
    cases.append(
        case("read-at-eof-is-empty", "read_chunk",
             [uinteger(blob_index), uinteger(156), uinteger(64)],
             [byte_seq(b"")], read_effect())
    )
    cases.append(
        case("read-past-eof-is-empty", "read_chunk",
             [uinteger(blob_index), uinteger(1000), uinteger(64)],
             [byte_seq(b"")], read_effect())
    )
    # Small-file exact read.
    alpha_index = rels.index("a.txt")
    alpha = open(os.path.join(FIXTURE, "a.txt"), "rb").read()
    cases.append(
        case("read-small-file", "read_chunk",
             [uinteger(alpha_index), uinteger(0), uinteger(64)],
             [byte_seq(alpha)], read_effect())
    )
    path = os.path.join(OUT, "fs-scan-corpus.json")
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(
            {"schema_version": "0.1", "name": "fs-scan", "cases": cases},
            handle,
            indent=1,
        )
        handle.write("\n")
    print(f"wrote {path} ({len(cases)} cases)")

if __name__ == "__main__":
    main()
