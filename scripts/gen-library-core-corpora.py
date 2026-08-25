#!/usr/bin/env python3
"""Generate bounded corpora for mncs.core library modules and their consumers.

Emits ExecutionCorpus JSON (schema 0.1) matching mncs-model's serde format.
Run from the repository root:

    python3 scripts/gen-library-core-corpora.py
"""

import json
import os
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, "examples", "execution")


def integer(value, bits=64, signed=True):
    return {"integer": {"value": value, "type": {"bits": bits, "signed": signed}}}


def boolean(value):
    return {"boolean": {"value": value}}


def finite(module, type_name, variant_name, discriminant):
    return {
        "finite": {
            "type_identity": f"mncs:0.2:finite-type:{module}::{type_name}",
            "variant_identity": f"mncs:0.2:finite-variant:{module}::{type_name}::{variant_name}",
            "discriminant": discriminant,
        }
    }


def record(module, type_name, field_types, values):
    """field_types: [(name, type_name)] in any order; values: {name: encoded}."""
    joined = "".join(f"{n}:{t};" for n, t in sorted(field_types))
    digest = urllib.parse.quote(joined, safe="")
    return {
        "record": {
            "type_identity": f"mncs:0.2:record-type:{module}::{type_name}::{digest}",
            "name": type_name,
            "fields": [[name, values[name]] for name, _ in sorted(field_types)],
        }
    }


def case(case_id, module, function, arguments, expected=None, step_budget=1024):
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


CORE = "mncs.core.status.v1"
LOGIC = "mncs.core.logic.v1"
ORDERING = "mncs.core.ordering.v1"
RAVEL = "ravel.core.v1"

P = lambda m: finite(m, "Status", "PASS", 0)  # noqa: E731
F = lambda m: finite(m, "Status", "FAIL", 1)  # noqa: E731
U = lambda m: finite(m, "Status", "UNKNOWN", 2)  # noqa: E731

# Dominance join: FAIL >= UNKNOWN >= PASS.
JOIN = {
    ("PASS", "PASS"): "PASS",
    ("PASS", "FAIL"): "FAIL",
    ("PASS", "UNKNOWN"): "UNKNOWN",
    ("FAIL", "PASS"): "FAIL",
    ("FAIL", "FAIL"): "FAIL",
    ("FAIL", "UNKNOWN"): "FAIL",
    ("UNKNOWN", "PASS"): "UNKNOWN",
    ("UNKNOWN", "FAIL"): "FAIL",
    ("UNKNOWN", "UNKNOWN"): "UNKNOWN",
}
VARIANTS = ["PASS", "FAIL", "UNKNOWN"]


def status_cases():
    cases = []
    enc = {"PASS": P, "FAIL": F, "UNKNOWN": U}
    for left in VARIANTS:
        for right in VARIANTS:
            result = JOIN[(left, right)]
            cases.append(
                case(
                    f"dominate-{left.lower()}-{right.lower()}",
                    CORE,
                    "dominate",
                    [enc[left](CORE), enc[right](CORE)],
                    enc[result](CORE),
                )
            )
    for left in VARIANTS:
        for right in VARIANTS:
            result = JOIN[(left, right)]
            pair = record(
                CORE,
                "StatusPair",
                [("left", "Status"), ("right", "Status")],
                {"left": enc[left](CORE), "right": enc[right](CORE)},
            )
            cases.append(
                case(
                    f"combine-{left.lower()}-{right.lower()}",
                    CORE,
                    "combine",
                    [pair],
                    enc[result](CORE),
                )
            )
    decided = {"PASS": True, "FAIL": True, "UNKNOWN": False}
    for name in VARIANTS:
        cases.append(
            case(
                f"is-decided-{name.lower()}",
                CORE,
                "is_decided",
                [enc[name](CORE)],
                boolean(decided[name]),
            )
        )
    for left in VARIANTS:
        for right in VARIANTS:
            result = left == JOIN[(left, right)]
            cases.append(
                case(
                    f"dominates-{left.lower()}-{right.lower()}",
                    CORE,
                    "dominates",
                    [enc[left](CORE), enc[right](CORE)],
                    boolean(result),
                )
            )
    return cases


def logic_cases():
    cases = []

    def b(value):
        return boolean(value)

    cases.append(case("not-true", LOGIC, "bool_not", [b(True)], b(False)))
    cases.append(case("not-false", LOGIC, "bool_not", [b(False)], b(True)))
    for a in (False, True):
        for c in (False, True):
            tag = f"{str(a).lower()}-{str(c).lower()}"
            cases.append(case(f"and-{tag}", LOGIC, "bool_and", [b(a), b(c)], b(a and c)))
            cases.append(case(f"or-{tag}", LOGIC, "bool_or", [b(a), b(c)], b(a or c)))
            cases.append(
                case(f"implies-{tag}", LOGIC, "bool_implies", [b(a), b(c)], b((not a) or c))
            )
            cases.append(case(f"xor-{tag}", LOGIC, "bool_xor", [b(a), b(c)], b(a != c)))
    return cases


def ordering_cases():
    cases = []

    def i(value, bits):
        return integer(value, bits=bits)

    tables = [
        ("i64", 64, [
            (7, 11), (11, 7), (-4, -9), (-9, -4), (0, 0),
            (4611686018427387903, -4611686018427387904),
        ]),
        ("i32", 32, [
            (7, 11), (11, 7), (-4, -9), (-9, -4), (0, 0),
            (2147483647, -2147483648),
        ]),
    ]
    for suffix, bits, rows in tables:
        for a, b in rows:
            tag = f"{suffix}-{a}-vs-{b}".replace("-", "neg") if str(a).startswith("-") or str(b).startswith("-") else f"{suffix}-{a}-vs-{b}"
            cases.append(case(f"min-{suffix}-{a}-{b}".replace("--", "-"), ORDERING, f"min_{suffix}", [i(a, bits), i(b, bits)], i(min(a, b), bits)))
            cases.append(case(f"max-{suffix}-{a}-{b}".replace("--", "-"), ORDERING, f"max_{suffix}", [i(a, bits), i(b, bits)], i(max(a, b), bits)))
        clamps = [
            ("below", -50, -10, 10, -10),
            ("above", 99, -10, 10, 10),
            ("inside", 3, -10, 10, 3),
            ("at-lo", -10, -10, 10, -10),
            ("at-hi", 10, -10, 10, 10),
            ("degenerate", 5, 7, 7, 7),
        ]
        for name, value, lo, hi, expected in clamps:
            cases.append(
                case(
                    f"clamp-{name}-{suffix}",
                    ORDERING,
                    f"clamp_{suffix}",
                    [i(value, bits), i(lo, bits), i(hi, bits)],
                    i(expected, bits),
                )
            )
    return cases


def ravel_differential_cases():
    """Same dominate matrix, executed against the consumer snapshot module."""
    cases = []
    enc = {"PASS": P, "FAIL": F, "UNKNOWN": U}
    for left in VARIANTS:
        for right in VARIANTS:
            result = JOIN[(left, right)]
            cases.append(
                case(
                    f"ravel-dominate-{left.lower()}-{right.lower()}",
                    RAVEL,
                    "dominate",
                    [enc[left](RAVEL), enc[right](RAVEL)],
                    enc[result](RAVEL),
                )
            )
    return cases


def status_wrong_cases():
    """Corpus for the deliberately wrong mutant: (UNKNOWN, UNKNOWN) -> PASS."""
    cases = []
    enc = {"PASS": P, "FAIL": F, "UNKNOWN": U}
    for left in VARIANTS:
        for right in VARIANTS:
            result = JOIN[(left, right)]
            cases.append(
                case(
                    f"dominate-{left.lower()}-{right.lower()}",
                    "examples.library_core.status_wrong",
                    "dominate",
                    [enc[left]("examples.library_core.status_wrong"), enc[right]("examples.library_core.status_wrong")],
                    enc[result]("examples.library_core.status_wrong"),
                )
            )
    return cases


P06 = "examples.profile06_payload_sums"


def i32(value):
    return integer(value, bits=32)


def gate(floor):
    return record(
        P06,
        "Gate",
        [("floor", "i32")],
        {"floor": i32(floor)},
    )


def verdict(variant, discriminant, payload=None):
    value = {
        "type_identity": f"mncs:0.2:finite-type:{P06}::Verdict",
        "variant_identity": f"mncs:0.2:finite-variant:{P06}::Verdict::{variant}",
        "discriminant": discriminant,
    }
    if payload is not None:
        value["payload"] = [
            [name, i32(number)] for name, number in sorted(payload)
        ]
    return {"finite": value}


def profile06_payload_cases():
    """Payload-bearing sums: construction, binding patterns, boundaries."""
    return [
        case("classify-accept", P06, "classify", [i32(50), gate(40)], verdict("Accept", 0, [("score", 50)])),
        case("classify-block", P06, "classify", [i32(30), gate(40)], verdict("Block", 1, [("reason", -10)])),
        case("classify-boundary", P06, "classify", [i32(40), gate(40)], verdict("Accept", 0, [("score", 40)])),
        case("severity-accept", P06, "severity", [verdict("Accept", 0, [("score", 42)])], i32(42)),
        case("severity-block", P06, "severity", [verdict("Block", 1, [("reason", -9)])], i32(9)),
        case("severity-defer", P06, "severity", [verdict("Defer", 2)], i32(1)),
    ]




RESULT = "mncs.core.result.v1"


def i64(value):
    return integer(value, bits=64)


def result_value(variant, discriminant, payload=None):
    value = {
        "type_identity": f"mncs:0.2:finite-type:{RESULT}::Result",
        "variant_identity": f"mncs:0.2:finite-variant:{RESULT}::Result::{variant}",
        "discriminant": discriminant,
    }
    if payload is not None:
        value["payload"] = [[name, i64(number)] for name, number in sorted(payload)]
    return {"finite": value}


def gate64(floor):
    return record(RESULT, "Gate", [("floor", "i64")], {"floor": i64(floor)})


def result_cases():
    cases = [
        case("divide-ok", RESULT, "divide", [i64(84), i64(4)], result_value("Ok", 0, [("value", 21)])),
        case("divide-zero", RESULT, "divide", [i64(5), i64(0)], result_value("Err", 1, [("reason", 1)])),
        case("bounded-ok", RESULT, "bounded_divide", [i64(100), i64(5), gate64(10)], result_value("Ok", 0, [("value", 20)])),
        case("bounded-below-floor", RESULT, "bounded_divide", [i64(3), i64(5), gate64(10)], result_value("Err", 1, [("reason", 2)])),
        case("value-or-ok", RESULT, "value_or", [result_value("Ok", 0, [("value", 7)]), i64(-1)], i64(7)),
        case("value-or-err", RESULT, "value_or", [result_value("Err", 1, [("reason", 2)]), i64(-1)], i64(-1)),
        case("reason-of-ok", RESULT, "reason_of", [result_value("Ok", 0, [("value", 9)])], i64(0)),
        case("reason-of-err", RESULT, "reason_of", [result_value("Err", 1, [("reason", 2)])], i64(2)),
    ]
    return cases


def main():
    emit("library-core-status-corpus.json", status_cases())
    emit("library-core-logic-corpus.json", logic_cases())
    emit("library-core-ordering-corpus.json", ordering_cases())
    emit("library-core-ravel-differential-corpus.json", ravel_differential_cases())
    emit("library-core-status-wrong-corpus.json", status_wrong_cases())
    emit("profile06-payload-sums-corpus.json", profile06_payload_cases())
    emit("library-core-result-corpus.json", result_cases())


if __name__ == "__main__":
    main()
