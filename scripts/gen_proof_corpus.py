#!/usr/bin/env python3
"""Generate the RFC 0007 proof-kernel execution corpus.

Each case describes a flat proof buffer plus the claim under test. Buffers
use the mncs.core.proof_term.v1 vocabulary; expectations are i64 verdict
codes from mncs.core.proof_check.v1 check_proof_code (0 PASS, 1 FAIL,
2 UNKNOWN). The same buffer descriptions feed the Rust reference checker
tests, so MNCS execution and the independent checker stay differentially
aligned by construction.
"""

import json
import sys

PROOF_TERM_MODULE = "mncs.core.proof_term.v1"
CELL_IDENTITY = (
    "mncs:0.2:record-type:mncs.core.proof_term.v1::Cell::"
    "arg0%3Ai64%3Barg1%3Ai64%3Barg2%3Ai64%3Barg3%3Ai64%3Btag%3ATermTag%3B"
)
TAG_IDENTITY = "mncs:0.2:finite-type:mncs.core.proof_term.v1::TermTag"
TAG_VARIANT = "mncs:0.2:finite-variant:mncs.core.proof_term.v1::TermTag::"

TAGS = [
    "Universe", "Var", "Pi", "Lambda", "Apply", "Nat", "Zero",
    "Succ", "Plus", "NatElim", "Eq", "Refl", "Unsupported",
]
TAG_INDEX = {name: position for position, name in enumerate(TAGS)}

I64 = {"bits": 64, "signed": True}


def cell(tag, a0=0, a1=0, a2=0, a3=0):
    assert tag in TAG_INDEX, tag
    finite = {
        "type_identity": TAG_IDENTITY,
        "variant_identity": TAG_VARIANT + tag,
        "discriminant": TAG_INDEX[tag],
    }
    fields = [
        ["arg0", {"integer": {"value": a0, "type": I64}}],
        ["arg1", {"integer": {"value": a1, "type": I64}}],
        ["arg2", {"integer": {"value": a2, "type": I64}}],
        ["arg3", {"integer": {"value": a3, "type": I64}}],
        ["tag", {"finite": finite}],
    ]
    return {"record": {"type_identity": CELL_IDENTITY, "name": "Cell", "fields": fields}}


def pad(cells):
    assert len(cells) <= 16
    return cells + [cell("Nat")] * (16 - len(cells))


def byte(value):
    return {"byte": {"value": value}}


def case(case_id, cells, count, proof, prop, expected):
    return {
        "id": case_id,
        "request": {
            "schema_version": "0.1",
            "target": {
                "module": "mncs.core.proof_check.v1",
                "function": "check_proof_code",
            },
            "arguments": [
                {"sequence": {"values": pad(cells)}},
                byte(count),
                byte(proof),
                byte(prop),
            ],
            "step_budget": 65536,
        },
        "expected_status": "returned",
        "expected": [{"integer": {"value": expected, "type": I64}}],
    }


def nat_literal(base, value):
    """Append Zero/Succ chain for `value` at index `base`; return cell list."""
    out = [cell("Zero")]
    for _ in range(value):
        out.append(cell("Succ", base + len(out) - 1))
    return out


CASES = [
    # Eq Nat Zero Zero proved by Refl: the minimal closed theorem.
    case(
        "refl-nat-zero",
        [cell("Nat"), cell("Zero"), cell("Eq", 0, 1, 1), cell("Refl", 1, 2)],
        4, 3, 2, 0,
    ),
    # The flagship tranche theorem: 2 + 3 = 5 computed by the kernel's
    # closed Plus primitive and witnessed by Refl. Literals share one Succ
    # chain so the whole proof fits the 16-cell buffer.
    case(
        "plus-two-three-is-five",
        [
            cell("Nat"),                       # 0
            cell("Zero"),                      # 1
            cell("Succ", 1),                   # 2 = 1
            cell("Succ", 2),                   # 3 = 2
            cell("Succ", 3),                   # 4 = 3
            cell("Succ", 4),                   # 5 = 4
            cell("Succ", 5),                   # 6 = 5
            cell("Plus", 3, 4),                # 7 = 2+3, value 5
            cell("Eq", 0, 7, 6),               # 8 Eq Nat (2+3) 5
            cell("Refl", 7, 8),                # 9 Refl
        ],
        10, 9, 8, 0,
    ),
    # Wrong equality proof: Refl(Zero) against Eq Nat Zero (Succ Zero).
    case(
        "refl-wrong-side",
        [
            cell("Nat"),                       # 0
            cell("Zero"),                      # 1
            cell("Succ", 1),                   # 2 = 1
            cell("Eq", 0, 1, 2),               # 3 Eq Nat 0 1
            cell("Refl", 1, 3),                # 4 Refl(Zero)
        ],
        5, 4, 3, 1,
    ),
    # Malformed buffer: forward reference breaks topological order.
    case(
        "forward-reference-rejected",
        [cell("Nat"), cell("Succ", 5)],
        2, 1, 0, 1,
    ),
    # Universe violation: Universe(3) has no type inside the hierarchy.
    case(
        "universe-ceiling-rejected",
        [cell("Universe", 3)],
        1, 0, 0, 1,
    ),
    # Dependent lambda over an Eq motive checks per-cell but the open proof
    # term stays UNKNOWN: assumption accounting is a future tranche.
    case(
        "dependent-lambda-stays-unknown",
        [
            cell("Universe", 0),               # 0 U(0)
            cell("Nat"),                       # 1 Nat
            cell("Pi", 1, 1),                  # 2 Nat -> Nat
            cell("Var", 2),                    # 3 x : Nat
            cell("Eq", 1, 3, 3),               # 4 Eq Nat x x
            cell("Pi", 4, 4),                  # 5 (x : Eq..) -> Eq..
            cell("Var", 5),                    # 6 p : Eq Nat x x
            cell("Lambda", 4, 6, 5),           # 7 fun p => p
        ],
        8, 7, 5, 2,
    ),
    # Pi formation over closed types is a clean PASS claim.
    case(
        "pi-formation",
        [
            cell("Universe", 0),               # 0 U(0)
            cell("Nat"),                       # 1 Nat
            cell("Pi", 1, 1),                  # 2 Nat -> Nat
        ],
        3, 2, 0, 0,
    ),
    # Proof for obligation A attached to obligation B: claiming Zero proves
    # an Eq proposition fails closed.
    case(
        "wrong-proof-shape",
        [cell("Nat"), cell("Zero"), cell("Eq", 0, 1, 1)],
        3, 1, 2, 1,
    ),
]

if __name__ == "__main__":
    corpus = {"schema_version": "0.1", "name": "proof-kernel", "cases": CASES}
    path = sys.argv[1] if len(sys.argv) > 1 else "-"
    text = json.dumps(corpus, indent=1) + "\n"
    if path == "-":
        sys.stdout.write(text)
    else:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text)
        print(f"wrote {len(CASES)} cases to {path}")
