#!/usr/bin/env python3
"""Generate bounded corpora for mncs.core.version and mncs.family.coherence.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde format.
Run from the repository root:

    python3 scripts/gen-family-coherence-corpora.py
"""

import itertools
import json
import os
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")

VERSION = "mncs.core.version.v1"
COHERENCE = "mncs.family.coherence.v01"
STATUS = "mncs.core.status.v1"


def integer(value, bits=32, signed=True):
    return {"integer": {"value": value, "type": {"bits": bits, "signed": signed}}}


def boolean(value):
    return {"boolean": {"value": value}}


def finite(module, type_name, variant_name, discriminant, payload=None):
    value = {
        "type_identity": f"mncs:0.2:finite-type:{module}::{type_name}",
        "variant_identity": f"mncs:0.2:finite-variant:{module}::{type_name}::{variant_name}",
        "discriminant": discriminant,
    }
    if payload is not None:
        value["payload"] = payload
    return {"finite": value}


def record(module, type_name, field_types, values):
    joined = "".join(f"{n}:{t};" for n, t in sorted(field_types))
    digest = urllib.parse.quote(joined, safe="")
    return {
        "record": {
            "type_identity": f"mncs:0.2:record-type:{module}::{type_name}::{digest}",
            "name": type_name,
            "fields": [[name, values[name]] for name, _ in sorted(field_types)],
        }
    }


def case(case_id, module, function, arguments, expected=None, step_budget=4096):
    entry = {
        "id": case_id,
        "request": {
            "schema_version": "0.1",
            "target": {"module": module, "function": function},
            "arguments": list(arguments),
            "step_budget": step_budget,
        },
    }
    if expected is not None:
        entry["expected"] = [expected] if not isinstance(expected, list) else expected
    return entry


def emit(name, cases):
    path = os.path.join(OUT, name)
    document = {"schema_version": "0.1", "name": name[: -len("-corpus.json")], "cases": cases}
    with open(path, "w") as handle:
        json.dump(document, handle, indent=1)
        handle.write("\n")
    print(f"wrote {path}: {len(cases)} cases")


def version(major, minor, patch):
    return record(
        VERSION,
        "Version",
        [("major", "i32"), ("minor", "i32"), ("patch", "i32")],
        {
            "major": integer(major),
            "minor": integer(minor),
            "patch": integer(patch),
        },
    )


ORDERING = {"Before": 0, "Same": 1, "After": 2}
ENVELOPES = ["Any", "Exact", "AtLeast", "CompatibleMinor"]

STATUS_VARIANTS = ["PASS", "FAIL", "UNKNOWN"]
STATUS_DISCRIMINANT = {"PASS": 0, "FAIL": 1, "UNKNOWN": 2}


def status(name):
    return finite(STATUS, "Status", name, STATUS_DISCRIMINANT[name])


def coherence_status(name):
    return finite(STATUS, "Status", name, STATUS_DISCRIMINANT[name])


CHECK_FIELDS = [(f"c{i:02d}", "Status") for i in range(1, 13)]


def check_vector(statuses):
    values = {}
    for index in range(12):
        slot = statuses[index] if index < len(statuses) else "PASS"
        values[f"c{index + 1:02d}"] = coherence_status(slot)
    return record(COHERENCE, "CheckVector", CHECK_FIELDS, values)


def join(left, right):
    if left == "FAIL" or right == "FAIL":
        return "FAIL"
    if left == "UNKNOWN" or right == "UNKNOWN":
        return "UNKNOWN"
    return "PASS"


def version_cases():
    versions = [
        (0, 0, 0),
        (0, 0, 1),
        (0, 1, 0),
        (0, 1, 4),
        (0, 2, 0),
        (1, 0, 0),
        (1, 0, 3),
        (1, 2, 0),
        (1, 2, 7),
        (2, 0, 0),
    ]

    def key(v):
        return (v[0], v[1], v[2])

    cases = []
    for left in versions:
        for right in versions:
            tag = f"v{left[0]}{left[1]}{left[2]}-v{right[0]}{right[1]}{right[2]}"
            if key(left) < key(right):
                order = "Before"
            elif key(left) == key(right):
                order = "Same"
            else:
                order = "After"

            def satisfies(envelope):
                if envelope == "Any":
                    return True
                if envelope == "Exact":
                    return order == "Same"
                if envelope == "AtLeast":
                    return order != "Before"
                return order != "Before" and left[0] == right[0]

            for envelope in ENVELOPES:
                cases.append(
                    case(
                        f"satisfies-{envelope.lower()}-{tag}",
                        VERSION,
                        "satisfies",
                        [
                            finite(VERSION, "Envelope", envelope, ENVELOPES.index(envelope)),
                            version(*left),
                            version(*right),
                        ],
                        boolean(satisfies(envelope)),
                    )
                )
            breaking = (
                left[0] != right[0]
                or (left[0] == 0 and left[1] != right[1])
                or (left[0] == 0 and left[1] == 0 and left != right)
            )
            cases.append(
                case(
                    f"breaking-{tag}",
                    VERSION,
                    "is_breaking_upgrade",
                    [version(*left), version(*right)],
                    boolean(breaking),
                )
            )
    return cases


def aggregate_cases():
    cases = []
    triples = list(itertools.product(STATUS_VARIANTS, repeat=3))
    for triple in triples:
        folded = join(join(triple[0], triple[1]), triple[2])
        tag = "-".join(name.lower() for name in triple)
        cases.append(
            case(
                f"aggregate-{tag}",
                COHERENCE,
                "aggregate_checks",
                [check_vector(list(triple))],
                coherence_status(folded),
            )
        )
    extreme_fail = ["FAIL"] * 12
    mixed = ["PASS"] * 5 + ["UNKNOWN"] + ["PASS"] * 6
    late_fail = ["PASS"] * 11 + ["FAIL"]
    for name, vector in [
        ("all-fail", extreme_fail),
        ("unknown-at-slot-06", mixed),
        ("fail-at-final-slot", late_fail),
        ("all-pass", ["PASS"] * 12),
    ]:
        folded = "PASS"
        for slot in vector:
            folded = join(folded, slot)
        cases.append(
            case(
                f"aggregate-{name}",
                COHERENCE,
                "aggregate_checks",
                [check_vector(vector)],
                coherence_status(folded),
            )
        )
    return cases


COMPAT_EVIDENCE = {"PASS": "Compatible", "UNKNOWN": None, "FAIL": None}


def verdict(variant, discriminant, reason=None):
    payload = [["reason", integer(reason)]] if reason is not None else None
    return finite(COHERENCE, "CoherenceVerdict", variant, discriminant, payload)


VERDICT_DISCRIMINANT = {"Compatible": 0, "Incompatible": 1, "Unresolved": 2}


def compatibility_cases():
    cases = []
    pairs = [
        ("exact-same", (1, 2, 3), (1, 2, 3), True),
        ("patch-behind", (1, 2, 3), (1, 2, 2), False),
        ("patch-ahead", (1, 2, 3), (1, 2, 9), True),
        ("minor-behind", (1, 2, 0), (1, 1, 9), False),
        ("minor-ahead", (1, 2, 0), (1, 3, 0), False),
        ("major-behind", (2, 0, 0), (1, 9, 9), False),
        ("zero-line-minor-behind", (0, 3, 0), (0, 2, 5), False),
        ("zero-line-equal", (0, 3, 1), (0, 3, 1), True),
    ]
    for evidence in STATUS_VARIANTS:
        for fingerprints in (True, False):
            for envelope in ENVELOPES:
                for name, bound, actual, in_range in pairs:
                    if envelope == "Any":
                        satisfied = True
                    elif envelope == "Exact":
                        satisfied = bound == actual
                    elif envelope == "AtLeast":
                        satisfied = tuple(actual) >= tuple(bound)
                    else:
                        satisfied = tuple(actual) >= tuple(bound) and bound[0] == actual[0]
                    if evidence == "FAIL":
                        expected = verdict("Incompatible", 1, 1)
                    elif evidence == "UNKNOWN":
                        expected = verdict("Unresolved", 2, 2)
                    elif not satisfied:
                        expected = verdict("Incompatible", 1, 3)
                    elif not fingerprints:
                        expected = verdict("Unresolved", 2, 4)
                    else:
                        expected = verdict("Compatible", 0)
                    claim = record(
                        COHERENCE,
                        "CompatibilityClaim",
                        [
                            ("envelope", "Envelope"),
                            ("bound_major", "i32"),
                            ("bound_minor", "i32"),
                            ("bound_patch", "i32"),
                            ("actual_major", "i32"),
                            ("actual_minor", "i32"),
                            ("actual_patch", "i32"),
                            ("fingerprints_match", "bool"),
                            ("evidence", "Status"),
                        ],
                        {
                            "envelope": finite(
                                VERSION, "Envelope", envelope, ENVELOPES.index(envelope)
                            ),
                            "bound_major": integer(bound[0]),
                            "bound_minor": integer(bound[1]),
                            "bound_patch": integer(bound[2]),
                            "actual_major": integer(actual[0]),
                            "actual_minor": integer(actual[1]),
                            "actual_patch": integer(actual[2]),
                            "fingerprints_match": boolean(fingerprints),
                            "evidence": coherence_status(evidence),
                        },
                    )
                    tag = f"{evidence.lower()}-fp{int(fingerprints)}-{envelope.lower()}-{name}"
                    cases.append(
                        case(
                            f"compat-{tag}",
                            COHERENCE,
                            "evaluate_compatibility",
                            [claim],
                            expected,
                        )
                    )
    return cases


IMPACT_FIELDS = [
    ("direct_consumer", "bool"),
    ("transitive_consumer", "bool"),
    ("manifest_found", "bool"),
    ("contract_declared", "bool"),
]

IMPACT_CLASSES = {
    "DefinitelyAffected": 0,
    "PotentiallyAffected": 1,
    "Unaffected": 2,
    "UnknownGap": 3,
}


def impact_cases():
    cases = []
    for direct, transitive, manifest, declared in itertools.product((False, True), repeat=4):
        if not manifest or not declared:
            expected = "UnknownGap"
        elif direct:
            expected = "DefinitelyAffected"
        elif transitive:
            expected = "PotentiallyAffected"
        else:
            expected = "Unaffected"
        observation = record(
            COHERENCE,
            "ImpactObservation",
            IMPACT_FIELDS,
            {
                "direct_consumer": boolean(direct),
                "transitive_consumer": boolean(transitive),
                "manifest_found": boolean(manifest),
                "contract_declared": boolean(declared),
            },
        )
        tag = f"d{int(direct)}t{int(transitive)}m{int(manifest)}c{int(declared)}"
        cases.append(
            case(
                f"impact-{tag}",
                COHERENCE,
                "classify_impact",
                [observation],
                finite(COHERENCE, "ImpactClass", expected, IMPACT_CLASSES[expected]),
            )
        )
    return cases


PROMOTION_STATES = {"Promote": 0, "Hold": 1, "Reject": 2}

PROMOTION_FIELDS = [
    ("integration", "Status"),
    ("identities_exact", "bool"),
    ("unresolved_unknowns", "i32"),
]


def promotion_cases():
    cases = []
    for integration in STATUS_VARIANTS:
        for identities in (False, True):
            for unknowns in (0, 1, 4):
                if integration == "FAIL":
                    expected = "Reject"
                elif integration == "UNKNOWN":
                    expected = "Hold"
                elif identities and unknowns == 0:
                    expected = "Promote"
                else:
                    expected = "Hold"
                subject = record(
                    COHERENCE,
                    "PromotionCase",
                    PROMOTION_FIELDS,
                    {
                        "integration": coherence_status(integration),
                        "identities_exact": boolean(identities),
                        "unresolved_unknowns": integer(unknowns),
                    },
                )
                tag = f"{integration.lower()}-exact{int(identities)}-u{unknowns}"
                cases.append(
                    case(
                        f"promotion-{tag}",
                        COHERENCE,
                        "promotion_decision",
                        [subject],
                        finite(COHERENCE, "PromotionDecision", expected, PROMOTION_STATES[expected]),
                    )
                )
    return cases


CHANGE_SET_STATES = [
    "Draft",
    "ImpactAnalyzed",
    "InProgress",
    "IntegrationCandidate",
    "VerifiedPass",
    "VerifiedFail",
    "Merged",
    "EpochPromoted",
    "Abandoned",
]
CHANGE_SET_EVENTS = [
    "AnalyzeImpact",
    "BeginImplementation",
    "SubmitCandidate",
    "RecordPass",
    "RecordFail",
    "Merge",
    "PromoteEpoch",
    "Abandon",
]

LEGAL = {
    ("Draft", "AnalyzeImpact"): "ImpactAnalyzed",
    ("Draft", "Abandon"): "Abandoned",
    ("ImpactAnalyzed", "BeginImplementation"): "InProgress",
    ("ImpactAnalyzed", "Abandon"): "Abandoned",
    ("InProgress", "SubmitCandidate"): "IntegrationCandidate",
    ("InProgress", "Abandon"): "Abandoned",
    ("IntegrationCandidate", "RecordPass"): "VerifiedPass",
    ("IntegrationCandidate", "RecordFail"): "VerifiedFail",
    ("IntegrationCandidate", "Abandon"): "Abandoned",
    ("VerifiedPass", "Merge"): "Merged",
    ("VerifiedPass", "Abandon"): "Abandoned",
    ("VerifiedFail", "BeginImplementation"): "InProgress",
    ("VerifiedFail", "Abandon"): "Abandoned",
    ("Merged", "PromoteEpoch"): "EpochPromoted",
}

TERMINAL = {"EpochPromoted", "Abandoned"}


def transition_cases():
    cases = []
    state_discriminant = {name: index for index, name in enumerate(CHANGE_SET_STATES)}
    event_discriminant = {name: index for index, name in enumerate(CHANGE_SET_EVENTS)}
    for state in CHANGE_SET_STATES:
        for event in CHANGE_SET_EVENTS:
            target = LEGAL.get((state, event))
            if target is not None:
                accepted = True
                following = target
                reason = 0
            else:
                accepted = False
                following = state
                reason = 0 if state in TERMINAL else 1
            expected = record(
                COHERENCE,
                "Transition",
                [("accepted", "bool"), ("following", "ChangeSetState"), ("reason", "i32")],
                {
                    "accepted": boolean(accepted),
                    "following": finite(
                        COHERENCE, "ChangeSetState", following, state_discriminant[following]
                    ),
                    "reason": integer(reason),
                },
            )
            cases.append(
                case(
                    f"transition-{state.lower()}-{event.lower()}",
                    COHERENCE,
                    "transition",
                    [
                        finite(COHERENCE, "ChangeSetState", state, state_discriminant[state]),
                        finite(COHERENCE, "ChangeSetEvent", event, event_discriminant[event]),
                    ],
                    expected,
                )
            )
    return cases


DRIFT_KINDS = {
    "Clean": 0,
    "UndeclaredSurfaceChange": 1,
    "FingerprintDrift": 2,
    "StaleReference": 3,
    "FixtureMismatch": 4,
}

DRIFT_FIELDS = [
    ("same_declared_version", "bool"),
    ("fingerprint_matches", "bool"),
    ("consumer_references_resolve", "bool"),
    ("fixtures_match_declaration", "bool"),
    ("surface_unchanged", "bool"),
]


def drift_cases():
    cases = []
    for same_version, fingerprints, references, fixtures, surface in itertools.product(
        (False, True), repeat=5
    ):
        if same_version and not surface:
            expected = "UndeclaredSurfaceChange"
        elif same_version and not fingerprints:
            expected = "FingerprintDrift"
        elif not references:
            expected = "StaleReference"
        elif not fixtures:
            expected = "FixtureMismatch"
        else:
            expected = "Clean"
        observation = record(
            COHERENCE,
            "DriftObservation",
            DRIFT_FIELDS,
            {
                "same_declared_version": boolean(same_version),
                "fingerprint_matches": boolean(fingerprints),
                "consumer_references_resolve": boolean(references),
                "fixtures_match_declaration": boolean(fixtures),
                "surface_unchanged": boolean(surface),
            },
        )
        tag = (
            f"v{int(same_version)}f{int(fingerprints)}"
            f"r{int(references)}x{int(fixtures)}s{int(surface)}"
        )
        cases.append(
            case(
                f"drift-{tag}",
                COHERENCE,
                "assess_drift",
                [observation],
                finite(COHERENCE, "DriftKind", expected, DRIFT_KINDS[expected]),
            )
        )
    return cases


def main():
    emit("family-version-corpus.json", version_cases())
    emit("family-coherence-aggregate-corpus.json", aggregate_cases())
    emit("family-coherence-compatibility-corpus.json", compatibility_cases())
    emit("family-coherence-impact-corpus.json", impact_cases())
    emit("family-coherence-promotion-corpus.json", promotion_cases())
    emit("family-coherence-transition-corpus.json", transition_cases())
    emit("family-coherence-drift-corpus.json", drift_cases())


if __name__ == "__main__":
    main()
