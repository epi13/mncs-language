#!/usr/bin/env python3
"""Generate the bounded corpus for the host-capability boundary.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde format.
The grant file is read here so expectations track its exact bytes; the
corpus itself carries no file content. Effect expectations pin the
realized host_read observation (kind/target/capability) with prohibition
of anything unexpected.
Run from the repository root:

    python3 scripts/gen_host_read_corpus.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")

MODULE = "examples.host_read_scan"
NOTE = open(os.path.join(OUT, "host-read-note.txt"), "rb").read()


def byte(value):
    return {"byte": {"value": value}}


def uinteger(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": False}}}


def boolean(value):
    return {"boolean": {"value": value}}


def exact(text, width):
    assert len(text) <= width, (text, width)
    return {"sequence": {"values": [byte(b) for b in text] + [byte(0)] * (width - len(text))}}


def host_read_effect():
    return {
        "expected_effects": [
            {"kind": "host_read", "target": "blob_read", "capability": "note_reader"}
        ],
        "prohibit_unexpected_effects": True,
    }


def case(case_id, function, arguments, expected, step_budget=8192):
    request = {
        "schema_version": "0.1",
        "target": {"module": MODULE, "function": function},
        "arguments": list(arguments),
        "step_budget": step_budget,
    }
    body = {
        "id": case_id,
        "request": request,
        "expected": [expected],
    }
    body.update(host_read_effect())
    return body


cases = [
    # The grant file contains b"task"; the miss needle is absent.
    case("hit", "has_term", [exact(b"task", 4), uinteger(4)], boolean(True)),
    case("miss", "has_term", [exact(b"zzzz", 4), uinteger(4)], boolean(False)),
    case("blob-len", "blob_len", [], uinteger(len(NOTE))),
]

corpus = {"schema_version": "0.1", "name": "host-read-scan", "cases": cases}

with open(os.path.join(OUT, "host-read-scan-corpus.json"), "w") as handle:
    json.dump(corpus, handle, indent=1)
    handle.write("\n")

print(f"wrote {len(cases)} cases against {len(NOTE)} grant bytes")
