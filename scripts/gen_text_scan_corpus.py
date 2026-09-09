#!/usr/bin/env python3
"""Generate the bounded corpus for mncs.std.text_scan.v1.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde format.
Every operation is covered over byte-exact vectors: literal-term detection
(including code-fence ```` ``` ````), prefix/suffix, first-index search with
the -1 absent sentinel, deterministic ordering in all five outcome classes,
byte counting, and word scanning.
Run from the repository root:

    python3 scripts/gen_text_scan_corpus.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")

MODULE = "mncs.std.text_scan.v1"


def boolean(value):
    return {"boolean": {"value": value}}


def integer(value, bits=64, signed=True):
    return {"integer": {"value": value, "type": {"bits": bits, "signed": signed}}}


def uinteger(value):
    return {"integer": {"value": value, "type": {"bits": 64, "signed": False}}}


def byte(value):
    return {"byte": {"value": value}}


def view(text):
    return {"sequence": {"values": [byte(b) for b in text.encode("ascii")]}}


def exact(text, width):
    raw = list(text.encode("ascii"))
    assert len(raw) <= width, (text, width)
    return {"sequence": {"values": [byte(b) for b in raw] + [byte(0)] * (width - len(raw))}}


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


HAY = "ab```cd"
HAY_LEN = uinteger(len(HAY))
FENCE = exact("```", 4)
FENCE_LEN = uinteger(3)
AB = exact("ab", 4)
AB_LEN = uinteger(2)
CD = exact("cd", 4)
NOPE = exact("zz", 4)
NOPE_LEN = uinteger(2)

WORDS = "hi there"
WORDS_LEN = uinteger(len(WORDS))
THE = exact("the", 4)
THE_LEN = uinteger(3)

cases = [
    # Literal-term detection over a code fence.
    case("contains-fence", "candidate_contains",
         [view(HAY), HAY_LEN, FENCE, FENCE_LEN], boolean(True)),
    case("contains-absent", "candidate_contains",
         [view(HAY), HAY_LEN, NOPE, NOPE_LEN], boolean(False)),
    case("contains-empty-needle", "candidate_contains",
         [view(HAY), HAY_LEN, exact("", 4), uinteger(0)], boolean(True)),
    # Prefix / suffix.
    case("starts-with-ab", "candidate_starts_with",
         [view(HAY), HAY_LEN, AB, AB_LEN], boolean(True)),
    case("starts-with-cd", "candidate_starts_with",
         [view(HAY), HAY_LEN, CD, AB_LEN], boolean(False)),
    case("ends-with-cd", "candidate_ends_with",
         [view(HAY), HAY_LEN, CD, AB_LEN], boolean(True)),
    case("ends-with-ab", "candidate_ends_with",
         [view(HAY), HAY_LEN, AB, AB_LEN], boolean(False)),
    case("ends-with-overlong", "candidate_ends_with",
         [view("ab"), uinteger(2), FENCE, FENCE_LEN], boolean(False)),
    # First index with the -1 absent sentinel.
    case("find-fence", "candidate_find",
         [view(HAY), HAY_LEN, FENCE, FENCE_LEN], integer(2)),
    case("find-absent", "candidate_find",
         [view(HAY), HAY_LEN, NOPE, NOPE_LEN], integer(-1)),
    case("find-the", "candidate_find",
         [view(WORDS), WORDS_LEN, THE, THE_LEN], integer(3)),
    # Deterministic ordering: less, greater, equal, prefix-shorter,
    # prefix-longer.
    case("compare-less", "candidate_compare",
         [exact("abc", 8), uinteger(3), exact("abd", 8), uinteger(3)], integer(-1)),
    case("compare-greater", "candidate_compare",
         [exact("abd", 8), uinteger(3), exact("abc", 8), uinteger(3)], integer(1)),
    case("compare-equal", "candidate_compare",
         [exact("abc", 8), uinteger(3), exact("abc", 8), uinteger(3)], integer(0)),
    case("compare-prefix-shorter", "candidate_compare",
         [exact("ab", 8), uinteger(2), exact("abc", 8), uinteger(3)], integer(-1)),
    case("compare-prefix-longer", "candidate_compare",
         [exact("abc", 8), uinteger(3), exact("ab", 8), uinteger(2)], integer(1)),
    # Byte counting and word scanning.
    case("count-fence-bytes", "candidate_count_byte",
         [view(HAY), HAY_LEN, byte(ord("`"))], uinteger(3)),
    case("count-absent-byte", "candidate_count_byte",
         [view(HAY), HAY_LEN, byte(ord("z"))], uinteger(0)),
    case("count-words-two", "candidate_count_words",
         [view(WORDS), WORDS_LEN], uinteger(2)),
    case("count-words-one", "candidate_count_words",
         [view(HAY), HAY_LEN], uinteger(1)),
    case("count-words-empty", "candidate_count_words",
         [view(""), uinteger(0)], uinteger(0)),
]

corpus = {"schema_version": "0.1", "name": "text-scan", "cases": cases}

with open(os.path.join(OUT, "text-scan-corpus.json"), "w") as handle:
    json.dump(corpus, handle, indent=1)
    handle.write("\n")

print(f"wrote {len(cases)} cases")
