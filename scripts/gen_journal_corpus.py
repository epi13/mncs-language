#!/usr/bin/env python3
"""Generate the Journal admission-gate execution corpus (mncs.family.journal.v1)."""

import json
import sys

MODULE = "mncs.family.journal.v1"
STATUS_MODULE = "mncs.core.status.v1"
I64 = {"bits": 64, "signed": True}
I32 = {"bits": 32, "signed": True}


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


TRUST_LEVELS = [
    "Observed",
    "LocallyProven",
    "IndependentlyConfirmed",
    "CommonsPromoted",
    "LegacyUnattested",
]

TRUST_EVENTS = ["LocalProof", "IndependentConfirmation", "CommonsPromotion"]

MODEL_ROLES = ["Classifier", "Summarizer", "Narrator", "DuplicateDetector", "Redactor"]


def trust_level(name):
    return finite(MODULE, "TrustLevel", name, TRUST_LEVELS.index(name))


def trust_event(name):
    return finite(MODULE, "TrustEvent", name, TRUST_EVENTS.index(name))


def model_role(name):
    return finite(MODULE, "ModelRole", name, MODEL_ROLES.index(name))


def boolean(value):
    return {"boolean": {"value": bool(value)}}


def integer(value, width):
    return {"integer": {"value": value, "type": width}}


def record_identity(name, fields):
    inner = "".join(f"{field}%3A{kind}%3B" for field, kind in fields)
    return f"mncs:0.2:record-type:{MODULE}::{name}::{inner}"


TALLY_FIELDS = [
    ("evidence_ok", "bool"),
    ("missing_refs", "i64"),
    ("required", "i64"),
    ("satisfied", "i64"),
    ("stale_refs", "i64"),
]


def tally(required, satisfied, missing, stale, evidence_ok):
    return {
        "record": {
            "type_identity": record_identity("EvidenceTally", TALLY_FIELDS),
            "name": "EvidenceTally",
            "fields": [
                ["evidence_ok", boolean(evidence_ok)],
                ["missing_refs", integer(missing, I64)],
                ["required", integer(required, I64)],
                ["satisfied", integer(satisfied, I64)],
                ["stale_refs", integer(stale, I64)],
            ],
        }
    }


CASE_FIELDS = [
    ("duplicate_known", "bool"),
    ("gate", "Status"),
    ("model_only", "bool"),
    ("validator", "Status"),
]


def admission_case(duplicate, gate, model_only, validator):
    return {
        "record": {
            "type_identity": record_identity("AdmissionCase", CASE_FIELDS),
            "name": "AdmissionCase",
            "fields": [
                ["duplicate_known", boolean(duplicate)],
                ["gate", status_variant(gate)],
                ["model_only", boolean(model_only)],
                ["validator", status_variant(validator)],
            ],
        }
    }


def verdict(name, reason=None):
    discriminant = {"Admit": 0, "Hold": 1, "Reject": 2}[name]
    payload = None
    if reason is not None:
        payload = [["reason", integer(reason, I32)]]
    return finite(MODULE, "AdmissionVerdict", name, discriminant, payload)


TRANSITION_FIELDS = [
    ("accepted", "bool"),
    ("following", "TrustLevel"),
    ("reason", "i32"),
]


def transition(accepted, following, reason):
    return {
        "record": {
            "type_identity": record_identity("TrustTransition", TRANSITION_FIELDS),
            "name": "TrustTransition",
            "fields": [
                ["accepted", boolean(accepted)],
                ["following", trust_level(following)],
                ["reason", integer(reason, I32)],
            ],
        }
    }


OPINION_FIELDS = [
    ("duplicate_hint", "bool"),
    ("journal_worthy", "bool"),
    ("role", "ModelRole"),
    ("sensitive_hint", "bool"),
]


def opinion(duplicate_hint, worthy, role, sensitive_hint):
    return {
        "record": {
            "type_identity": record_identity("ModelOpinion", OPINION_FIELDS),
            "name": "ModelOpinion",
            "fields": [
                ["duplicate_hint", boolean(duplicate_hint)],
                ["journal_worthy", boolean(worthy)],
                ["role", model_role(role)],
                ["sensitive_hint", boolean(sensitive_hint)],
            ],
        }
    }


SENSITIVITY_FIELDS = [
    ("redaction_reviewed", "bool"),
    ("security_detail", "bool"),
]


def sensitivity(reviewed, detail):
    return {
        "record": {
            "type_identity": record_identity("Sensitivity", SENSITIVITY_FIELDS),
            "name": "Sensitivity",
            "fields": [
                ["redaction_reviewed", boolean(reviewed)],
                ["security_detail", boolean(detail)],
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
    # Evidence gate: PASS only on complete, current, integrity-clean tallies.
    case("gate-full", "gate_admit", [tally(3, 3, 0, 0, True)], status_variant("PASS")),
    case(
        "gate-partial",
        "gate_admit",
        [tally(3, 1, 2, 0, True)],
        status_variant("UNKNOWN"),
    ),
    case(
        "gate-claimed-but-missing",
        "gate_admit",
        [tally(3, 3, 1, 0, True)],
        status_variant("UNKNOWN"),
    ),
    case(
        "gate-empty",
        "gate_admit",
        [tally(3, 0, 3, 0, True)],
        status_variant("UNKNOWN"),
    ),
    case(
        "gate-no-claim",
        "gate_admit",
        [tally(0, 0, 0, 0, True)],
        status_variant("UNKNOWN"),
    ),
    case(
        "gate-no-evidence",
        "gate_admit",
        [tally(3, 3, 0, 0, False)],
        status_variant("FAIL"),
    ),
    case(
        "gate-stale",
        "gate_admit",
        [tally(3, 3, 0, 1, True)],
        status_variant("FAIL"),
    ),
    case(
        "gate-overcount",
        "gate_admit",
        [tally(3, 4, 0, 0, True)],
        status_variant("FAIL"),
    ),
    case(
        "gate-negative",
        "gate_admit",
        [tally(3, -1, 0, 0, True)],
        status_variant("FAIL"),
    ),
    # Admission: validator attestation plus evidence gate; duplicates and
    # model-only claims are rejected even when every tally passes.
    case(
        "admit-clean",
        "admission_decision",
        [admission_case(False, "PASS", False, "PASS")],
        verdict("Admit"),
    ),
    case(
        "reject-validator",
        "admission_decision",
        [admission_case(False, "PASS", False, "FAIL")],
        verdict("Reject", 1),
    ),
    case(
        "reject-evidence",
        "admission_decision",
        [admission_case(False, "FAIL", False, "PASS")],
        verdict("Reject", 2),
    ),
    case(
        "reject-duplicate",
        "admission_decision",
        [admission_case(True, "PASS", False, "PASS")],
        verdict("Reject", 3),
    ),
    case(
        "reject-model-only",
        "admission_decision",
        [admission_case(False, "PASS", True, "PASS")],
        verdict("Reject", 4),
    ),
    case(
        "hold-unresolved-gate",
        "admission_decision",
        [admission_case(False, "UNKNOWN", False, "PASS")],
        verdict("Hold", 5),
    ),
    case(
        "hold-unresolved-validator",
        "admission_decision",
        [admission_case(False, "PASS", False, "UNKNOWN")],
        verdict("Hold", 5),
    ),
    # Trust promotion: stepwise upgrades need PASS; skips, quarantine, and
    # terminal states hold.
    case(
        "trust-first-proof",
        "trust_transition",
        [trust_level("Observed"), trust_event("LocalProof"), status_variant("PASS")],
        transition(True, "LocallyProven", 0),
    ),
    case(
        "trust-proof-needs-pass",
        "trust_transition",
        [trust_level("Observed"), trust_event("LocalProof"), status_variant("UNKNOWN")],
        transition(False, "LocallyProven", 2),
    ),
    case(
        "trust-skip-level",
        "trust_transition",
        [
            trust_level("Observed"),
            trust_event("IndependentConfirmation"),
            status_variant("PASS"),
        ],
        transition(False, "Observed", 1),
    ),
    case(
        "trust-quarantine",
        "trust_transition",
        [
            trust_level("LegacyUnattested"),
            trust_event("LocalProof"),
            status_variant("PASS"),
        ],
        transition(False, "LegacyUnattested", 3),
    ),
    case(
        "trust-terminal-stays",
        "trust_transition",
        [
            trust_level("CommonsPromoted"),
            trust_event("CommonsPromotion"),
            status_variant("UNKNOWN"),
        ],
        transition(True, "CommonsPromoted", 0),
    ),
    case(
        "trust-commons-step",
        "trust_transition",
        [
            trust_level("IndependentlyConfirmed"),
            trust_event("CommonsPromotion"),
            status_variant("PASS"),
        ],
        transition(True, "CommonsPromoted", 0),
    ),
    # History linkage and model authority need no host arguments.
    case("chain-genesis", "check_genesis_link", [], boolean(True)),
    case("chain-successor", "check_successor_link", [], boolean(True)),
    case("chain-reorder", "check_reorder_link", [], boolean(False)),
    case("chain-fork", "check_fork_link", [], boolean(False)),
    case("model-never-admits", "check_model_never_admits", [], boolean(True)),
    # Rendering guard: admitted non-sensitive content renders; unadmitted
    # content never renders; security detail needs redaction review.
    case(
        "render-admitted",
        "may_render_public",
        [sensitivity(False, False), status_variant("PASS")],
        boolean(True),
    ),
    case(
        "render-unadmitted",
        "may_render_public",
        [sensitivity(False, False), status_variant("UNKNOWN")],
        boolean(False),
    ),
    case(
        "render-security-unreviewed",
        "may_render_public",
        [sensitivity(False, True), status_variant("PASS")],
        boolean(False),
    ),
    case(
        "render-security-reviewed",
        "may_render_public",
        [sensitivity(True, True), status_variant("PASS")],
        boolean(True),
    ),
    # Model hints are observable facts, not authority.
    case(
        "hint-duplicate",
        "model_hint_duplicate",
        [opinion(True, False, "DuplicateDetector", False)],
        boolean(True),
    ),
    case(
        "hint-worthy",
        "model_hint_worthy",
        [opinion(False, True, "Classifier", False)],
        boolean(True),
    ),
    case(
        "hint-sensitive",
        "model_hint_sensitive",
        [opinion(False, False, "Redactor", True)],
        boolean(True),
    ),
    case(
        "model-narrator-no-admit",
        "model_may_admit",
        [opinion(False, True, "Narrator", False)],
        boolean(False),
    ),
]

if __name__ == "__main__":
    corpus = {"schema_version": "0.1", "name": "journal", "cases": CASES}
    path = sys.argv[1] if len(sys.argv) > 1 else "-"
    text = json.dumps(corpus, indent=1) + "\n"
    if path == "-":
        sys.stdout.write(text)
    else:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text)
        print(f"wrote {len(CASES)} cases to {path}")
