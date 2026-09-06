#!/usr/bin/env python3
"""Generate the RFC status-gate execution corpus (mncs.family.rfc_status.v1)."""

import json
import sys
from urllib.parse import quote

MODULE = "mncs.family.rfc_status.v1"
STATUS_MODULE = "mncs.core.status.v1"
I64 = {"bits": 64, "signed": True}


def finite(module, type_name, variant, discriminant, payload=None):
    value = {
        "type_identity": f"mncs:0.2:finite-type:{module}::{type_name}",
        "variant_identity": f"mncs:0.2:finite-variant:{module}::{type_name}::{variant}",
        "discriminant": discriminant,
    }
    if payload is not None:
        value["payload"] = payload
    return {"finite": value}


def status_variant(name):
    return finite(STATUS_MODULE, "Status", name, {"PASS": 0, "FAIL": 1, "UNKNOWN": 2}[name])


IMPL = ["Absent", "Substrate", "Partial", "Bounded", "Experimental", "Complete"]


def impl_state(name):
    return finite(MODULE, "ImplState", name, IMPL.index(name))


TALLY_FIELDS = [
    ("evidence_ok", "bool"),
    ("excused", "i64"),
    ("partial", "i64"),
    ("required", "i64"),
    ("satisfied", "i64"),
]


def tally_identity():
    inner = "".join(f"{name}%3A{kind}%3B" for name, kind in TALLY_FIELDS)
    return f"mncs:0.2:record-type:{MODULE}::CriterionTally::{inner}"


def tally(required, satisfied, partial, excused, evidence_ok):
    return {
        "record": {
            "type_identity": tally_identity(),
            "name": "CriterionTally",
            "fields": [
                ["evidence_ok", {"boolean": {"value": evidence_ok}}],
                ["excused", {"integer": {"value": excused, "type": I64}}],
                ["partial", {"integer": {"value": partial, "type": I64}}],
                ["required", {"integer": {"value": required, "type": I64}}],
                ["satisfied", {"integer": {"value": satisfied, "type": I64}}],
            ],
        }
    }


def case(case_id, function, arguments, expected):
    return {
        "id": case_id,
        "request": {
            "schema_version": "0.1",
            "target": {"module": MODULE, "function": function},
            "arguments": arguments,
            "step_budget": 4096,
        },
        "expected_status": "returned",
        "expected": [expected],
    }


CASES = [
    case("exp-0007-tally", "check_rfc0007_experimental", [], status_variant("PASS")),
    case("impl-0007-tally", "check_rfc0007_implemented", [], status_variant("UNKNOWN")),
    case(
        "exp-no-evidence",
        "gate_experimental",
        [tally(20, 15, 4, 1, False)],
        status_variant("FAIL"),
    ),
    case(
        "exp-incomplete",
        "gate_experimental",
        [tally(20, 3, 4, 1, True)],
        status_variant("UNKNOWN"),
    ),
    case(
        "exp-overcount",
        "gate_experimental",
        [tally(20, 15, 6, 1, True)],
        status_variant("FAIL"),
    ),
    case(
        "impl-full",
        "gate_implemented",
        [tally(5, 5, 0, 0, True)],
        status_variant("PASS"),
    ),
    case(
        "impl-partial",
        "gate_implemented",
        [tally(5, 4, 1, 0, True)],
        status_variant("UNKNOWN"),
    ),
    case(
        "impl-excused",
        "gate_implemented",
        [tally(5, 4, 0, 1, True)],
        status_variant("UNKNOWN"),
    ),
    case(
        "trans-upgrade-pass",
        "transition_allowed",
        [impl_state("Partial"), impl_state("Experimental"), status_variant("PASS")],
        {"boolean": {"value": True}},
    ),
    case(
        "trans-upgrade-unknown",
        "transition_allowed",
        [impl_state("Partial"), impl_state("Experimental"), status_variant("UNKNOWN")],
        {"boolean": {"value": False}},
    ),
    case(
        "trans-downgrade",
        "transition_allowed",
        [impl_state("Experimental"), impl_state("Substrate"), status_variant("FAIL")],
        {"boolean": {"value": True}},
    ),
    case(
        "trans-same",
        "transition_allowed",
        [impl_state("Bounded"), impl_state("Bounded"), status_variant("UNKNOWN")],
        {"boolean": {"value": True}},
    ),
]

if __name__ == "__main__":
    corpus = {"schema_version": "0.1", "name": "rfc-status", "cases": CASES}
    path = sys.argv[1] if len(sys.argv) > 1 else "-"
    text = json.dumps(corpus, indent=1) + "\n"
    if path == "-":
        sys.stdout.write(text)
    else:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text)
        print(f"wrote {len(CASES)} cases to {path}")
