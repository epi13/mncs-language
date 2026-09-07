#!/usr/bin/env python3
"""Generate the RFC 0007 tranche 0.2 (genuine dependency) execution corpus.

Buffers use the mncs.core.proof_dep.v2 vocabulary (flat `[Cell; 32]`
binder-edge graphs with explicit levels). Expectations are verdict codes
from check_proof_code (0 PASS, 1 FAIL, 2 UNKNOWN), head tags from
probe_eval_code (0..12, 100 abstain, 101 malformed), verdicts from
probe_defeq_code (0/1/2, 3 malformed), or assumption codes from
assumptions_code (base-33, -1 when not PASS).

The same buffer descriptions feed the Rust reference checker
(`proof_dep.rs`), so MNCS execution and the independent checker stay
differentially aligned by construction.
"""

import json
import sys

CELL_IDENTITY = (
    "mncs:0.2:record-type:mncs.core.proof_dep.v2::Cell::"
    "arg0%3Ai64%3Barg1%3Ai64%3Barg2%3Ai64%3Barg3%3Ai64%3Btag%3ATermTag%3B"
)
TAG_IDENTITY = "mncs:0.2:finite-type:mncs.core.proof_dep.v2::TermTag"
TAG_VARIANT = "mncs:0.2:finite-variant:mncs.core.proof_dep.v2::TermTag::"

TAGS = [
    "Universe", "Var", "Pi", "Lam", "App", "Nat", "Zero",
    "Succ", "Plus", "NatElim", "Eq", "Refl", "Hyp", "Cong",
    "Unsupported",
]
TAG_INDEX = {name: position for position, name in enumerate(TAGS)}

I64 = {"bits": 64, "signed": True}
CAPACITY = 32


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
    assert len(cells) <= CAPACITY, len(cells)
    return cells + [cell("Nat")] * (CAPACITY - len(cells))


def byte(value):
    assert 0 <= value <= 255, value
    return {"byte": {"value": value}}


def check_case(case_id, cells, count, proof, prop, expected):
    return {
        "id": case_id,
        "request": {
            "schema_version": "0.1",
            "target": {
                "module": "mncs.core.proof_dep.v2",
                "function": "check_proof_code",
            },
            "arguments": [
                {"sequence": {"values": pad(cells)}},
                byte(count),
                byte(proof),
                byte(prop),
            ],
            "step_budget": 2000000,
        },
        "expected_status": "returned",
        "expected": [{"integer": {"value": expected, "type": I64}}],
    }


def eval_case(case_id, cells, count, index, expected):
    req = dict(check_case(case_id, cells, count, 0, 0, expected)["request"])
    req["target"] = {"module": "mncs.core.proof_dep.v2", "function": "probe_eval_code"}
    req["arguments"] = [
        {"sequence": {"values": pad(cells)}},
        byte(count),
        byte(index),
    ]
    return {
        "id": case_id,
        "request": req,
        "expected_status": "returned",
        "expected": [{"integer": {"value": expected, "type": I64}}],
    }


def defeq_case(case_id, cells, count, left, right, expected):
    req = dict(check_case(case_id, cells, count, 0, 0, expected)["request"])
    req["target"] = {"module": "mncs.core.proof_dep.v2", "function": "probe_defeq_code"}
    req["arguments"] = [
        {"sequence": {"values": pad(cells)}},
        byte(count),
        byte(left),
        byte(right),
    ]
    return {
        "id": case_id,
        "request": req,
        "expected_status": "returned",
        "expected": [{"integer": {"value": expected, "type": I64}}],
    }


def assumps_case(case_id, cells, count, proof, prop, expected):
    req = dict(check_case(case_id, cells, count, proof, prop, expected)["request"])
    req["target"] = {"module": "mncs.core.proof_dep.v2", "function": "assumptions_code"}
    return {
        "id": case_id,
        "request": req,
        "expected_status": "returned",
        "expected": [{"integer": {"value": expected, "type": I64}}],
    }


def closed_refl():
    # Nat, Zero, Eq(Nat, Zero, Zero), Refl(Zero): PASS.
    cells = [cell("Nat"), cell("Zero"), cell("Eq", 0, 1, 1), cell("Refl", 1)]
    return [check_case("closed-refl", cells, 4, 3, 2, 0)]


def flagship_plus_zero_right():
    # Pi (n : Nat). Eq Nat (Plus n Zero) n, proved by induction:
    #   motive = Lam Nat. Eq Nat (Plus Var1 Zero) (Var1)   [level 1]
    #   base   = Refl Zero : P Zero  (Plus Zero Zero unfolds to Zero)
    #   step   = Lam k. Lam ih. Cong ih  : Pi k. Pi ih. P (Succ k)
    #   proof  = Lam n. NatElim motive base step n         [level 0]
    # Open cones are trees (each variable use is its own cell); only closed
    # cells (Nat, Zero) are shared. Proposition binders use fresh levels.
    cells = [
        cell("Nat"),            # 0
        cell("Zero"),           # 1
        cell("Var", 1),         # 2 motive n (use A)
        cell("Plus", 2, 1),     # 3 Plus n Zero
        cell("Var", 1),         # 4 motive n (use B)
        cell("Eq", 0, 3, 4),    # 5 motive body
        cell("Lam", 0, 5, 1),   # 6 motive
        cell("Refl", 1),        # 7 base
        cell("Var", 2),         # 8 k (Plus-A)
        cell("Plus", 8, 1),     # 9 Plus k Zero
        cell("Succ", 9),        # 10 sA
        cell("Var", 2),         # 11 k (sB)
        cell("Succ", 11),       # 12 sB
        cell("Var", 2),         # 13 k (Plus-B)
        cell("Plus", 13, 1),    # 14 Plus-B
        cell("Var", 2),         # 15 k (P-k side)
        cell("Eq", 0, 14, 15),  # 16 P k
        cell("Var", 3),         # 17 ih
        cell("Cong", 17, 10, 12),  # 18 step body
        cell("Lam", 16, 18, 3), # 19 inner step
        cell("Lam", 0, 19, 2),  # 20 outer step
        cell("Var", 0),         # 21 target n
        cell("NatElim", 6, 7, 20, 21),  # 22
        cell("Lam", 0, 22, 0),  # 23 proof
        cell("Var", 4),         # 24 prop n (use A)
        cell("Plus", 24, 1),    # 25
        cell("Var", 4),         # 26 prop n (use B)
        cell("Eq", 0, 25, 26),  # 27
        cell("Pi", 0, 27, 4),   # 28 prop
    ]
    out = [check_case("flagship-plus-zero-right", cells, 29, 23, 28, 0)]
    out.append(assumps_case("flagship-assumptions-empty", cells, 29, 23, 28, 0))
    return out


def substitution_probes():
    out = []
    # Beta: (Lam x:Nat. x) Zero evaluates to Zero (value head 3).
    cells = [
        cell("Nat"),          # 0
        cell("Zero"),         # 1
        cell("Var", 5),       # 2 body
        cell("Lam", 0, 2, 5), # 3 function
        cell("App", 3, 1),    # 4 redex
    ]
    out.append(eval_case("beta-identity", cells, 5, 4, 3))
    # Constructor heads: Succ evaluates to a tag-4 node, and so does a
    # closed Plus (Succ-chain, not a collapsed literal).
    cells = [cell("Nat"), cell("Zero"), cell("Succ", 1)]
    out.append(eval_case("succ-head", cells, 3, 2, 4))
    cells = [
        cell("Nat"),        # 0
        cell("Zero"),       # 1
        cell("Succ", 1),    # 2 one
        cell("Plus", 2, 2), # 3 one plus one
    ]
    out.append(eval_case("plus-head", cells, 4, 3, 4))
    # Computation: Plus (Succ Zero) (Succ Zero) is definitionally 2, stated
    # as Succ (Succ Zero).
    cells = [
        cell("Nat"),       # 0
        cell("Zero"),      # 1
        cell("Succ", 1),   # 2 one
        cell("Succ", 2),   # 3 two (constructor form)
        cell("Plus", 2, 2),  # 4 one plus one
        cell("Succ", 2),   # 5 two again
    ]
    out.append(defeq_case("plus-one-plus-one", cells, 6, 4, 5, 0))
    # Capture-relevant resolution: Lam x. Lam y. x checks against
    # Pi x. Pi y. Nat (inner use resolves past the nearer binder).
    # Binder levels are globally unique (Barendregt): the inner Pi/Lam use
    # 6/7, the outer pair 5/8; defeq still relates them up to alpha.
    cells = [
        cell("Nat"),          # 0
        cell("Var", 5),       # 1 x (use)
        cell("Lam", 0, 1, 6), # 2 inner
        cell("Lam", 0, 2, 5), # 3 outer (proof)
        cell("Pi", 0, 0, 7),  # 4 inner type
        cell("Pi", 0, 4, 8),  # 5 outer type (prop)
    ]
    out.append(check_case("nested-lam-resolution", cells, 6, 3, 5, 0))
    return out


def elim_const_family():
    # Minimal dependent elimination over a closed target with a constant
    # motive (Lam Nat. Nat): exercises the full elim rule (domain, zero,
    # step under two rigids, target) at small resource scale. The step is
    # Lam k. Lam ih. ih; the proposition is Nat (P Zero).
    cells = [
        cell("Nat"),           # 0
        cell("Zero"),          # 1
        cell("Lam", 0, 0, 5),  # 2 motive (const Nat family)
        cell("Var", 7),        # 3 ih (use)
        cell("Lam", 0, 3, 7),  # 4 inner step
        cell("Lam", 0, 4, 6),  # 5 outer step
        cell("NatElim", 2, 1, 5, 1),  # 6 proof over Zero
    ]
    return [check_case("elim-const-family", cells, 7, 6, 0, 0)]


def assumption_cases():
    out = []
    # Open proof under one assumption: Refl x : Eq Nat x x with Hyp(0, Nat).
    # Each variable use is its own cell (open cones are trees).
    cells = [
        cell("Nat"),        # 0
        cell("Hyp", 0, 0),  # 1 assumption x : Nat (root)
        cell("Var", 0),     # 2 x (witness)
        cell("Var", 0),     # 3 x (left)
        cell("Var", 0),     # 4 x (right)
        cell("Refl", 2),    # 5 proof
        cell("Eq", 0, 3, 4),  # 6 prop
    ]
    out.append(check_case("assumption-open-refl", cells, 7, 5, 6, 0))
    # One used hypothesis of level 0: digits over 7 cells give 33**5.
    out.append(assumps_case("assumption-used-code", cells, 7, 5, 6, 33 ** 5))
    return out


def adversarial_cases():
    out = []
    # Undeclared variable: resolution failure is FAIL.
    out.append(check_case("undeclared-var", [cell("Nat"), cell("Var", 7)], 2, 1, 0, 1))
    # A declaration is not a term: naming a Hyp as proof is FAIL.
    out.append(check_case("hyp-as-proof", [cell("Nat"), cell("Hyp", 0, 0)], 2, 1, 0, 1))
    # A declaration is not a subterm: embedded Hyp is FAIL.
    out.append(check_case(
        "hyp-nonroot",
        [cell("Nat"), cell("Hyp", 0, 0), cell("Succ", 1)], 3, 2, 0, 1))
    # Assumption types must be closed: Hyp(3, Pi(Nat, Var 3)) is FAIL.
    out.append(check_case(
        "dependent-hyp",
        [cell("Nat"), cell("Var", 3), cell("Pi", 0, 1, 4), cell("Hyp", 3, 2)],
        4, 2, 2, 1))
    return out


def main():
    cases = []
    cases.extend(closed_refl())
    cases.extend(flagship_plus_zero_right())
    cases.extend(substitution_probes())
    cases.extend(elim_const_family())
    cases.extend(assumption_cases())
    cases.extend(adversarial_cases())
    corpus = {"schema_version": "0.1", "name": "proof-dep", "cases": cases}
    text = json.dumps(corpus, indent=1) + "\n"
    if len(sys.argv) > 1:
        with open(sys.argv[1], "w") as handle:
            handle.write(text)
        print(f"wrote {len(cases)} cases to {sys.argv[1]}")
    else:
        print(text)


if __name__ == "__main__":
    main()
