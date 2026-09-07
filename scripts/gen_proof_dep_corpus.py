#!/usr/bin/env python3
"""Generate the RFC 0007 tranche 0.2 (genuine dependency) execution corpora.

Buffers use the mncs.core.proof_dep.v2 vocabulary (flat `[Cell; 32]`
binder-edge graphs with explicit levels). The main corpus expectations are
verdict codes from check_proof_code (0 PASS, 1 FAIL, 2 UNKNOWN), head tags
from probe_eval_code (0..12, 100 abstain, 101 malformed), verdicts from
probe_defeq_code (0/1/2, 3 malformed), assumption codes from
assumptions_code (base-33 diagnostic projection, -1 when not PASS), or
canonical assumption sets from assumption_set (exact, authority-grade).

The same buffer descriptions feed the Rust reference checker
(`proof_dep.rs`), so MNCS execution and the independent checker stay
differentially aligned by construction.

A second corpus (proof-dep-admission) covers the authoritative admission
and binding policy in mncs.core.proof_admit.v1 (seal_binding,
binding_authorizes, binding_reusable). Those semantics are MNCS-only BY
DESIGN — no second implementation re-decides them, so the Rust checker
does not parse that file; it is executed on all five backends as
MNCS-authoritative evidence, and the Rust side covers it through real
MNCS execution plus corroboration in the admission integration tests.
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


# --- Tranche-0.2 hardening: canonical sets and MNCS admission ---

U64 = {"bits": 64, "signed": False}

ASSUMPTION_SET_IDENTITY = (
    "mncs:0.2:record-type:mncs.core.proof_dep.v2::AssumptionSet::"
    "carrier%3A%5Bi64%3B%2032%5D%3Bcount%3Ai64%3Bhyp%3A%5Bi64%3B%2032%5D%3B"
    "level%3A%5Bi64%3B%2032%5D%3Bvalid%3Abool%3B"
)

PROOF_BINDING_IDENTITY = (
    "mncs:0.2:record-type:mncs.core.proof_admit.v1::ProofBinding::"
    "assumptions%3Amncs%3A0.2%3Arecord-type%3Amncs.core.proof_dep.v2%3A%3A"
    "AssumptionSet%3A%3Acarrier%253A%255Bi64%253B%252032%255D%253Bcount%253A"
    "i64%253Bhyp%253A%255Bi64%253B%252032%255D%253Blevel%253A%255Bi64%253B"
    "%252032%255D%253Bvalid%253Abool%253B%3Bcells%3A%5Bmncs%3A0.2%3Arecord-"
    "type%3Amncs.core.proof_dep.v2%3A%3ACell%3A%3Aarg0%253Ai64%253Barg1%253A"
    "i64%253Barg2%253Ai64%253Barg3%253Ai64%253Btag%253ATermTag%253B%3B%2032"
    "%5D%3Bcount%3Abyte%3Bdep0%3A%5Bbyte%3B%2032%5D%3Bdep1%3A%5Bbyte%3B%2032"
    "%5D%3Bdep2%3A%5Bbyte%3B%2032%5D%3Bdep3%3A%5Bbyte%3B%2032%5D%3Bdep_count"
    "%3Au64%3Bkernel%3A%5Bbyte%3B%2032%5D%3Bkernel_len%3Au64%3Bobligation%3A"
    "%5Bbyte%3B%2032%5D%3Bproof%3A%5Bbyte%3B%2032%5D"
    "%3Bproof_index%3Abyte%3Bprop_index%3Abyte%3Bverdict%3Ai64%3B"
)

KERNEL_ID = list(b"mncs:proof-kernel:0.2")
assert len(KERNEL_ID) == 21, len(KERNEL_ID)


def sha256_digest(text):
    import hashlib
    return list(hashlib.sha256(text).digest())


# Opaque test digests: the MNCS admission functions treat the proof digest
# as exact-match identity bytes. These are NOT real artifact hashes (the
# real digest linkage is covered by the Rust admission tests over sealed
# artifacts); corpus reuse cases only need consistent vs mutated bytes.
DIGEST_A = list(range(1, 33))
DIGEST_B = [9] + list(range(2, 33))

# Obligation identities cross into MNCS as sha256 digests (real obligation
# identities exceed every bounded byte slot). Corpus cases use the true
# digests of their label strings, mutated by byte flips below.
OBLIGATION_FLAGSHIP = sha256_digest(b"obligation:plus-zero-right")
OBLIGATION_OPEN = sha256_digest(b"obligation:open-refl")
OBLIGATION_OTHER = sha256_digest(b"obligation:something-else")

DEP_PATTERN = list(range(101, 133))


def u64(value):
    assert value >= 0, value
    return {"integer": {"value": value, "type": U64}}


def boolean(value):
    return {"boolean": {"value": bool(value)}}


def bytes_seq(raw, width):
    assert len(raw) <= width, (len(raw), width)
    return {"sequence": {"values": [byte(b) for b in raw] + [byte(0)] * (width - len(raw))}}


def i64_seq(values):
    assert len(values) == CAPACITY, len(values)
    return {"sequence": {"values": [{"integer": {"value": v, "type": I64}} for v in values]}}


def assumption_set_value(uses, valid):
    """Theory-derived canonical set: uses is [(hyp, level, carrier)]."""
    hyp = [0] * CAPACITY
    level = [0] * CAPACITY
    carrier = [0] * CAPACITY
    for slot, (h, lv, ca) in enumerate(uses):
        hyp[slot] = h
        level[slot] = lv
        carrier[slot] = ca
    fields = [
        ["carrier", i64_seq(carrier)],
        ["count", {"integer": {"value": len(uses), "type": I64}}],
        ["hyp", i64_seq(hyp)],
        ["level", i64_seq(level)],
        ["valid", boolean(valid)],
    ]
    return {"record": {"type_identity": ASSUMPTION_SET_IDENTITY, "name": "AssumptionSet", "fields": fields}}


def proof_binding_value(cells32, count, proof_index, prop_index, digest, obligation,
                        kernel, verdict, uses, valid, dep_count=0, deps=None):
    """Theory-derived binding record: seals must match what seal_binding
    would package for the same inputs (verdict + canonical set)."""
    assert len(cells32) == CAPACITY
    assert len(digest) == 32 and len(obligation) == 32
    deps = deps or [[0] * 32] * 4
    assert len(deps) == 4 and all(len(d) == 32 for d in deps)
    fields = [
        ["assumptions", assumption_set_value(uses, valid)],
        ["cells", {"sequence": {"values": cells32}}],
        ["count", byte(count)],
        ["dep0", bytes_seq(deps[0], 32)],
        ["dep1", bytes_seq(deps[1], 32)],
        ["dep2", bytes_seq(deps[2], 32)],
        ["dep3", bytes_seq(deps[3], 32)],
        ["dep_count", u64(dep_count)],
        ["kernel", bytes_seq(kernel, 32)],
        ["kernel_len", u64(len(kernel))],
        ["obligation", bytes_seq(obligation, 32)],
        ["proof", bytes_seq(digest, 32)],
        ["proof_index", byte(proof_index)],
        ["prop_index", byte(prop_index)],
        ["verdict", {"integer": {"value": verdict, "type": I64}}],
    ]
    return {"record": {"type_identity": PROOF_BINDING_IDENTITY, "name": "ProofBinding", "fields": fields}}


def set_case(case_id, cells, count, proof, prop, uses, valid):
    req = dict(check_case(case_id, cells, count, proof, prop, 0)["request"])
    req["target"] = {"module": "mncs.core.proof_dep.v2", "function": "assumption_set"}
    return {
        "id": case_id,
        "request": req,
        "expected_status": "returned",
        "expected": [assumption_set_value(uses, valid)],
    }


def seal_arguments(cells, count, proof, prop, digest, obligation, kernel,
                   dep_count=0, deps=None):
    deps = deps or [[0] * 32] * 4
    assert len(digest) == 32 and len(obligation) == 32
    return [
        bytes_seq(digest, 32),
        bytes_seq(obligation, 32),
        bytes_seq(kernel, 32),
        u64(len(kernel)),
        {"sequence": {"values": pad(cells)}},
        byte(count),
        byte(proof),
        byte(prop),
        u64(dep_count),
        bytes_seq(deps[0], 32),
        bytes_seq(deps[1], 32),
        bytes_seq(deps[2], 32),
        bytes_seq(deps[3], 32),
    ]


def seal_case(case_id, cells, count, proof, prop, digest, obligation, kernel,
              verdict, uses, valid, dep_count=0, deps=None, budget=2000000):
    req = {
        "schema_version": "0.1",
        "target": {"module": "mncs.core.proof_admit.v1", "function": "seal_binding"},
        "arguments": seal_arguments(cells, count, proof, prop, digest, obligation, kernel, dep_count, deps),
        "step_budget": budget,
    }
    return {
        "id": case_id,
        "request": req,
        "expected_status": "returned",
        "expected": [proof_binding_value(pad(cells), count, proof, prop, digest,
                                         obligation, kernel, verdict, uses, valid,
                                         dep_count, deps)],
    }


def authorizes_case(case_id, binding):
    req = {
        "schema_version": "0.1",
        "target": {"module": "mncs.core.proof_admit.v1", "function": "binding_authorizes"},
        "arguments": [binding],
        "step_budget": 2000000,
    }
    return {
        "id": case_id,
        "request": req,
        "expected_status": "returned",
        "expected": [boolean(True)],
    }


def authorizes_false_case(case_id, binding):
    case = authorizes_case(case_id, binding)
    case["expected"] = [boolean(False)]
    return case


def reusable_case(case_id, binding, cells, count, proof, prop, digest, obligation,
                  kernel, dep_count=0, deps=None, expect=True, budget=4000000):
    deps = deps or [[0] * 32] * 4
    assert len(digest) == 32 and len(obligation) == 32
    req = {
        "schema_version": "0.1",
        "target": {"module": "mncs.core.proof_admit.v1", "function": "binding_reusable"},
        "arguments": [binding] + [
            bytes_seq(digest, 32),
            bytes_seq(obligation, 32),
            bytes_seq(kernel, 32),
            u64(len(kernel)),
            {"sequence": {"values": pad(cells)}},
            byte(count),
            byte(proof),
            byte(prop),
            u64(dep_count),
            bytes_seq(deps[0], 32),
            bytes_seq(deps[1], 32),
            bytes_seq(deps[2], 32),
            bytes_seq(deps[3], 32),
        ],
        "step_budget": budget,
    }
    return {
        "id": case_id,
        "request": req,
        "expected_status": "returned",
        "expected": [boolean(expect)],
    }


def closed_refl():
    # Nat, Zero, Eq(Nat, Zero, Zero), Refl(Zero): PASS.
    cells = [cell("Nat"), cell("Zero"), cell("Eq", 0, 1, 1), cell("Refl", 1)]
    return [check_case("closed-refl", cells, 4, 3, 2, 0)]


def flagship_buffer():
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
    return cells, 29, 23, 28


def flagship_plus_zero_right():
    cells, count, proof, prop = flagship_buffer()
    out = [check_case("flagship-plus-zero-right", cells, count, proof, prop, 0)]
    out.append(assumps_case("flagship-assumptions-empty", cells, count, proof, prop, 0))
    out.append(set_case("flagship-set-empty", cells, count, proof, prop, [], True))
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


def open_refl_buffer():
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
    return cells, 7, 5, 6


def assumption_cases():
    out = []
    cells, count, proof, prop = open_refl_buffer()
    out.append(check_case("assumption-open-refl", cells, count, proof, prop, 0))
    # One used hypothesis of level 0: digits over 7 cells give 33**5.
    out.append(assumps_case("assumption-used-code", cells, count, proof, prop, 33 ** 5))
    # Canonical exact set: hypothesis 1, level 0, carrier cell 0.
    out.append(set_case("assumption-used-set", cells, count, proof, prop, [(1, 0, 0)], True))
    return out


def unknown_buffer():
    # Opaque proof cell against Nat: the kernel abstains (UNKNOWN) instead
    # of deciding. Both implementations must agree on abstention.
    cells = [
        cell("Nat"),          # 0 prop
        cell("Unsupported", 0, 0, 0, 0),  # 1 opaque proof
    ]
    return cells, 2, 1, 0


def unknown_cases():
    out = []
    cells, count, proof, prop = unknown_buffer()
    out.append(check_case("unknown-opaque-proof", cells, count, proof, prop, 2))
    out.append(set_case("unknown-opaque-set", cells, count, proof, prop, [], False))
    return out


def admission_cases():
    """MNCS-authoritative admission/binding cases (admission corpus only).

    Every expectation is theory-derived from the calculus and the admit
    module: seal packages the kernel verdict with the canonical set;
    authorizes is PASS + expected kernel + valid set; reusable additionally
    requires exact presented identities, live cells, and dependency slots.
    Each mutation flips exactly one conjunct to false.
    """
    out = []
    fcells, fcount, fproof, fprop = flagship_buffer()
    ocells, ocount, oproof, oprop = open_refl_buffer()

    flagship_binding = proof_binding_value(
        pad(fcells), fcount, fproof, fprop, DIGEST_A, OBLIGATION_FLAGSHIP,
        KERNEL_ID, 0, [], True)
    open_binding = proof_binding_value(
        pad(ocells), ocount, oproof, oprop, DIGEST_A, OBLIGATION_OPEN,
        KERNEL_ID, 0, [(1, 0, 0)], True)

    # Seal packages verdict + canonical set (cheap open-refl buffer keeps
    # most of the five-backend sweep fast; the flagship seal pins the same
    # path at full theorem scale, as the Rust admission tests do over the
    # real artifact).
    out.append(seal_case("admit-seal-open", ocells, ocount, oproof, oprop,
                         DIGEST_A, OBLIGATION_OPEN, KERNEL_ID, 0, [(1, 0, 0)], True))
    out.append(seal_case("admit-seal-flagship", fcells, fcount, fproof, fprop,
                         DIGEST_A, OBLIGATION_FLAGSHIP, KERNEL_ID, 0, [], True,
                         budget=4000000))

    # Authorization: PASS + expected kernel + valid set authorizes; every
    # other combination refuses, and nothing upgrades FAIL/UNKNOWN.
    out.append(authorizes_case("admit-authorizes-pass", flagship_binding))
    out.append(authorizes_false_case(
        "admit-authorizes-fail-verdict",
        proof_binding_value(pad(fcells), fcount, fproof, fprop, DIGEST_A,
                            OBLIGATION_FLAGSHIP, KERNEL_ID, 1, [], False)))
    out.append(authorizes_false_case(
        "admit-authorizes-unknown-verdict",
        proof_binding_value(pad(fcells), fcount, fproof, fprop, DIGEST_A,
                            OBLIGATION_FLAGSHIP, KERNEL_ID, 2, [], False)))
    wrong_kernel = list(KERNEL_ID)
    wrong_kernel[0] = 110  # 'm' -> 'n': wrong kernel identity
    out.append(authorizes_false_case(
        "admit-authorizes-wrong-kernel",
        proof_binding_value(pad(fcells), fcount, fproof, fprop, DIGEST_A,
                            OBLIGATION_FLAGSHIP, wrong_kernel, 0, [], True)))
    out.append(authorizes_false_case(
        "admit-authorizes-invalid-set",
        proof_binding_value(pad(fcells), fcount, fproof, fprop, DIGEST_A,
                            OBLIGATION_FLAGSHIP, KERNEL_ID, 0, [], False)))

    # Reuse: exact match authorizes (closed flagship and open proof, the
    # latter also with one bound dependency slot).
    out.append(reusable_case("admit-reuse-flagship", flagship_binding,
                             fcells, fcount, fproof, fprop,
                             DIGEST_A, OBLIGATION_FLAGSHIP, KERNEL_ID))
    out.append(reusable_case("admit-reuse-open", open_binding,
                             ocells, ocount, oproof, oprop,
                             DIGEST_A, OBLIGATION_OPEN, KERNEL_ID))
    dep_binding = proof_binding_value(
        pad(ocells), ocount, oproof, oprop, DIGEST_A, OBLIGATION_OPEN,
        KERNEL_ID, 0, [(1, 0, 0)], True, dep_count=1, deps=[DEP_PATTERN, [0] * 32, [0] * 32, [0] * 32])
    out.append(reusable_case("admit-reuse-open-dep", dep_binding,
                             ocells, ocount, oproof, oprop,
                             DIGEST_A, OBLIGATION_OPEN, KERNEL_ID,
                             dep_count=1, deps=[DEP_PATTERN, [0] * 32, [0] * 32, [0] * 32]))

    # Mutations: each flips exactly one conjunct.
    out.append(reusable_case("admit-reject-digest", flagship_binding,
                             fcells, fcount, fproof, fprop,
                             DIGEST_B, OBLIGATION_FLAGSHIP, KERNEL_ID, expect=False))
    flipped_obligation = list(OBLIGATION_FLAGSHIP)
    flipped_obligation[0] = (flipped_obligation[0] + 1) % 256
    out.append(reusable_case("admit-reject-obligation", flagship_binding,
                             fcells, fcount, fproof, fprop,
                             DIGEST_A, flipped_obligation, KERNEL_ID, expect=False))
    out.append(reusable_case("admit-reject-obligation-swap", flagship_binding,
                             fcells, fcount, fproof, fprop,
                             DIGEST_A, OBLIGATION_OTHER, KERNEL_ID, expect=False))
    out.append(reusable_case("admit-reject-kernel", flagship_binding,
                             fcells, fcount, fproof, fprop,
                             DIGEST_A, OBLIGATION_FLAGSHIP, wrong_kernel, expect=False))
    drifted_cells = [dict(c) for c in fcells]
    drifted_record = dict(drifted_cells[3]["record"])
    drifted_fields = [list(f) for f in drifted_record["fields"]]
    for field in drifted_fields:
        if field[0] == "arg0":
            field[1] = {"integer": {"value": 9, "type": I64}}
    drifted_record["fields"] = drifted_fields
    drifted_cells[3]["record"] = drifted_record
    out.append(reusable_case("admit-reject-live-cells", flagship_binding,
                             drifted_cells, fcount, fproof, fprop,
                             DIGEST_A, OBLIGATION_FLAGSHIP, KERNEL_ID, expect=False))
    skewed_binding = proof_binding_value(
        pad(drifted_cells), fcount, fproof, fprop, DIGEST_A, OBLIGATION_FLAGSHIP,
        KERNEL_ID, 0, [], True)
    out.append(reusable_case("admit-reject-sealed-cells", skewed_binding,
                             fcells, fcount, fproof, fprop,
                             DIGEST_A, OBLIGATION_FLAGSHIP, KERNEL_ID, expect=False))
    out.append(reusable_case("admit-reject-count", flagship_binding,
                             fcells, fcount + 1, fproof, fprop,
                             DIGEST_A, OBLIGATION_FLAGSHIP, KERNEL_ID, expect=False))
    out.append(reusable_case("admit-reject-proof-index", flagship_binding,
                             fcells, fcount, fproof, 27,
                             DIGEST_A, OBLIGATION_FLAGSHIP, KERNEL_ID, expect=False))
    flipped_dep = list(DEP_PATTERN)
    flipped_dep[7] = (flipped_dep[7] + 1) % 256
    out.append(reusable_case("admit-reject-dep", dep_binding,
                             ocells, ocount, oproof, oprop,
                             DIGEST_A, OBLIGATION_OPEN, KERNEL_ID,
                             dep_count=1, deps=[flipped_dep, [0] * 32, [0] * 32, [0] * 32],
                             expect=False))
    out.append(reusable_case("admit-reject-dep-count", dep_binding,
                             ocells, ocount, oproof, oprop,
                             DIGEST_A, OBLIGATION_OPEN, KERNEL_ID,
                             dep_count=0, expect=False))
    mutated_set_binding = proof_binding_value(
        pad(ocells), ocount, oproof, oprop, DIGEST_A, OBLIGATION_OPEN,
        KERNEL_ID, 0, [(2, 0, 0)], True)
    out.append(reusable_case("admit-reject-set", mutated_set_binding,
                             ocells, ocount, oproof, oprop,
                             DIGEST_A, OBLIGATION_OPEN, KERNEL_ID, expect=False))
    return out


def adversarial_cases():
    out = []
    # Undeclared variable: resolution failure is FAIL.
    out.append(check_case("undeclared-var", [cell("Nat"), cell("Var", 7)], 2, 1, 0, 1))
    out.append(set_case("undeclared-var-set", [cell("Nat"), cell("Var", 7)], 2, 1, 0, [], False))
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
    cases.extend(unknown_cases())
    cases.extend(adversarial_cases())
    corpus = {"schema_version": "0.1", "name": "proof-dep", "cases": cases}
    text = json.dumps(corpus, indent=1) + "\n"
    admission = admission_cases()
    admission_corpus = {"schema_version": "0.1", "name": "proof-dep-admission", "cases": admission}
    admission_text = json.dumps(admission_corpus, indent=1) + "\n"
    if len(sys.argv) > 1:
        with open(sys.argv[1], "w") as handle:
            handle.write(text)
        print(f"wrote {len(cases)} cases to {sys.argv[1]}")
        if len(sys.argv) > 2:
            with open(sys.argv[2], "w") as handle:
                handle.write(admission_text)
            print(f"wrote {len(admission)} cases to {sys.argv[2]}")
        else:
            print(admission_text)
    else:
        print(text)


if __name__ == "__main__":
    main()
