#!/usr/bin/env python3
"""Bounded MNCS Language Provider Protocol 0.1 adapter for Forge development."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

PROVIDER = {
    "id": "mncs-language-provider",
    "name": "mncs-language-provider",
    "identity": "mncs-language-provider-v1",
    "version": "0.1",
}
METHODS = [
    "mncs-language-rust-quality",
    "mncs-language-semantic-validation",
    "mncs-language-hir-trace-integrity",
    "mncs-language-ssa-integrity",
    "mncs-language-evidence-freshness",
    "mncs-language-execution-equivalence",
]
OUTPUT_LIMIT = 65_536
TIMEOUT_SECONDS = 30


def response(
    request: dict[str, Any],
    status: str,
    summary: str,
    *,
    witnesses: list[object] | None = None,
    limitations: list[str] | None = None,
    unsupported: list[str] | None = None,
    dependency_paths: list[str] | None = None,
    complete: bool = False,
) -> dict[str, object]:
    return {
        "protocol_version": "0.1",
        "type": "analysis_response",
        "request_id": request.get("request_id", "unknown"),
        "provider": PROVIDER,
        "status": status,
        "summary": summary,
        "witnesses": witnesses or [],
        "limitations": limitations or [],
        "extensions": {
            "unsupported_constructs": unsupported or [],
            # Forge 0.1 workflows read the shorter key while verifier runs
            # accept the protocol's explicit key. Keep both during the
            # transition so this adapter remains compatible with either path.
            "unsupported": unsupported or [],
            "mncs_forge": {
                "assumptions": [
                    "the checked repository fixtures represent the declared bounded subset"
                ],
                "dependency_envelope": {
                    "paths": dependency_paths or [],
                    "identities": {},
                    "complete": complete,
                },
            },
        },
    }


def run(command: list[str], *, timeout: int = TIMEOUT_SECONDS) -> tuple[bool, str]:
    try:
        completed = subprocess.run(
            command,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=timeout,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        return False, str(error)
    return completed.returncode == 0, completed.stdout[-OUTPUT_LIMIT:]


def parse_json_line(output: str) -> dict[str, Any]:
    decoder = json.JSONDecoder()
    start = 0
    while True:
        start = output.find("{", start)
        if start < 0:
            break
        try:
            parsed, _ = decoder.raw_decode(output[start:])
        except json.JSONDecodeError:
            start += 1
            continue
        if isinstance(parsed, dict):
            return parsed
        start += 1
    return {}


def rust_quality(request: dict[str, Any]) -> dict[str, object]:
    checks = [
        ["cargo", "fmt", "--all", "--", "--check"],
        ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
        ["cargo", "test", "--workspace"],
    ]
    witnesses = []
    for command in checks:
        ok, output = run(command)
        witnesses.append({"command": command, "passed": ok, "output_tail": output[-2000:]})
        if not ok:
            return response(
                request,
                "FAIL",
                "a declared Rust quality check failed",
                witnesses=witnesses,
                limitations=["failure is limited to the declared command and workspace state"],
                dependency_paths=["Cargo.toml", "Cargo.lock", "crates"],
                complete=True,
            )
    return response(
        request,
        "PASS",
        "all declared Rust quality checks passed",
        witnesses=witnesses,
        limitations=["quality checks do not establish independent evaluation or conformance"],
        dependency_paths=["Cargo.toml", "Cargo.lock", "crates"],
        complete=True,
    )


def semantic_validation(request: dict[str, Any]) -> dict[str, object]:
    valid, valid_output = run([
        "cargo", "run", "-q", "-p", "mncs-cli", "--", "validate",
        "examples/executable/checked-add.mncs.json",
    ])
    invalid, invalid_output = run([
        "cargo", "run", "-q", "-p", "mncs-cli", "--", "validate",
        "examples/executable/invalid-body-capability.mncs.json",
    ])
    rejected = not invalid
    witnesses = [
        {"fixture": "checked-add", "accepted": valid, "output_tail": valid_output[-1000:]},
        {"fixture": "invalid-body-capability", "rejected": rejected, "output_tail": invalid_output[-1000:]},
    ]
    return response(
        request,
        "PASS" if valid and rejected else "FAIL",
        "valid and invalid executable-body fixtures had the declared outcomes"
        if valid and rejected else "semantic-body fixture outcome did not match the declared expectation",
        witnesses=witnesses,
        dependency_paths=["examples/executable", "crates/mncs-model", "crates/mncs-cli"],
        complete=True,
    )


def hir_trace(request: dict[str, Any]) -> dict[str, object]:
    command = [
        "cargo", "run", "-q", "-p", "mncs-cli", "--", "ir",
        "examples/executable/checked-add.mncs.json",
    ]
    first_ok, first_output = run(command)
    second_ok, second_output = run(command)
    if not first_ok or not second_ok:
        return response(
            request,
            "UNKNOWN",
            "HIR could not be emitted for the supported fixture",
            limitations=["lowering execution was unavailable or malformed"],
            unsupported=["hir-emission-unavailable"],
            dependency_paths=["examples/executable/checked-add.mncs.json", "crates/mncs-model", "crates/mncs-cli"],
        )
    try:
        first = json.loads(first_output)
        second = json.loads(second_output)
        entries = first["trace"]["entries"]
        transformations = first["transformations"]
        operation_trace = any(":operation:" in item["semantic_identity"] for item in entries)
        deterministic = first == second
    except (KeyError, TypeError, json.JSONDecodeError):
        entries, transformations, operation_trace, deterministic = [], [], False, False
    passed = operation_trace and bool(transformations) and deterministic
    return response(
        request,
        "PASS" if passed else "FAIL",
        "HIR trace and transformation provenance are deterministic" if passed else "HIR trace integrity check failed",
        witnesses=[{"operation_trace": operation_trace, "transformation_count": len(transformations), "trace_count": len(entries), "deterministic": deterministic}],
        limitations=["trace integrity is provenance validation, not a proof of lowering correctness"],
        dependency_paths=["examples/executable/checked-add.mncs.json", "crates/mncs-model", "crates/mncs-cli"],
        complete=True,
    )


def ssa_integrity(request: dict[str, Any]) -> dict[str, object]:
    ok, output = run([
        "cargo", "run", "-q", "-p", "mncs-cli", "--", "ssa",
        "examples/executable/checked-add.mncs.json",
    ])
    if not ok:
        return response(
            request,
            "UNKNOWN",
            "SSA command is unavailable or could not validate the fixture",
            limitations=["SSA support is experimental and explicitly bounded"],
            unsupported=["ssa-unavailable"],
            dependency_paths=["examples/executable/checked-add.mncs.json", "crates/mncs-model", "crates/mncs-cli"],
        )
    try:
        artifact = json.loads(output)
        passed = artifact["schema_version"] == "0.4" and bool(artifact["functions"])
        witness = {"schema_version": artifact["schema_version"], "function_count": len(artifact["functions"])}
    except (KeyError, TypeError, json.JSONDecodeError):
        passed, witness = False, {"malformed": True}
    return response(
        request,
        "PASS" if passed else "FAIL",
        "SSA artifact passed structural provider checks" if passed else "SSA artifact was malformed",
        witnesses=[witness],
        limitations=["SSA structural validation does not establish formal compiler correctness"],
        dependency_paths=["examples/executable/checked-add.mncs.json", "crates/mncs-model", "crates/mncs-cli"],
        complete=True,
    )


def evidence_freshness(request: dict[str, Any]) -> dict[str, object]:
    manifest = "/tmp/mncs-language-forge-evidence.json"
    exported_ok, exported = run([
        "cargo", "run", "-q", "-p", "mncs-cli", "--", "evidence-manifest",
        "examples/semantic-foundation/before.mncs.json",
    ])
    if not exported_ok:
        return response(request, "UNKNOWN", "evidence manifest could not be exported", unsupported=["manifest-export"])
    try:
        Path(manifest).write_text(exported, encoding="utf-8")
    except OSError as error:
        return response(request, "UNKNOWN", "evidence fixture could not be staged", limitations=[str(error)])
    ok, output = run([
        "cargo", "run", "-q", "-p", "mncs-cli", "--", "evidence-check",
        manifest, "examples/semantic-foundation/after.mncs.json",
    ])
    try:
        report = json.loads(output)
        stale = any(item.get("state") == "stale" for item in report)
    except json.JSONDecodeError:
        stale = False
    passed = ok and stale
    return response(
        request,
        "PASS" if passed else "FAIL",
        "changed semantic dependencies make saved evidence stale" if passed else "evidence freshness check failed",
        witnesses=[{"stale_detected": stale}],
        limitations=["freshness check exercises the repository fixture and does not infer all dependencies"],
        dependency_paths=["examples/semantic-foundation", "crates/mncs-model", "crates/mncs-cli"],
        complete=True,
    )


def execution_equivalence(request: dict[str, Any]) -> dict[str, object]:
    baseline = "examples/execution/bounded-sum-baseline.mncs.json"
    equivalent = "examples/execution/bounded-sum-equivalent-refactor.mncs.json"
    regression = "examples/execution/bounded-sum-regression.mncs.json"
    corpus = "examples/execution/bounded-sum-corpus.json"
    equivalent_ok, equivalent_output = run([
        "cargo", "run", "-q", "-p", "mncs-cli", "--", "compare-execution",
        baseline, equivalent, corpus,
    ])
    regression_ok, regression_output = run([
        "cargo", "run", "-q", "-p", "mncs-cli", "--", "compare-execution",
        baseline, regression, corpus,
    ])
    equivalent_report = parse_json_line(equivalent_output)
    regression_report = parse_json_line(regression_output)
    equivalent_passed = (
        equivalent_report.get("status") == "equivalent_over_corpus"
        and equivalent_report.get("matching_cases") == equivalent_report.get("corpus_size")
    )
    regression_detected = (
        regression_report.get("status") == "mismatch_detected"
        and regression_report.get("mismatching_cases", 0) > 0
    )
    passed = equivalent_ok and not regression_ok and equivalent_passed and regression_detected
    return response(
        request,
        "PASS" if passed else "FAIL",
        "bounded corpus equivalence passed and the intentional regression was detected"
        if passed else "bounded execution equivalence check did not match the declared outcomes",
        witnesses=[
            {
                "fixture": "bounded-sum-equivalent-refactor",
                "process_passed": equivalent_ok,
                "status": equivalent_report.get("status"),
                "matching_cases": equivalent_report.get("matching_cases"),
                "corpus_size": equivalent_report.get("corpus_size"),
            },
            {
                "fixture": "bounded-sum-regression",
                "process_rejected": not regression_ok,
                "status": regression_report.get("status"),
                "mismatching_cases": regression_report.get("mismatching_cases"),
            },
        ],
        limitations=[
            "corpus equivalence is finite behavioral evidence, not universal equivalence",
            "reference execution is not production backend execution",
            "local Forge provider results are development evidence, not independent evaluation",
        ],
        dependency_paths=["examples/execution", "crates/mncs-model", "crates/mncs-cli"],
        complete=True,
    )


def structural_workflow(request: dict[str, Any]) -> dict[str, object]:
    results = [
        semantic_validation(request),
        hir_trace(request),
        ssa_integrity(request),
    ]
    statuses = {result["status"] for result in results}
    status = "FAIL" if "FAIL" in statuses else "UNKNOWN" if "UNKNOWN" in statuses else "PASS"
    return response(
        request,
        status,
        "bounded semantic, HIR, and SSA checks completed",
        witnesses=[
            {
                "summary": result.get("summary"),
                "status": result.get("status"),
                "witnesses": result.get("witnesses", []),
            }
            for result in results
        ],
        limitations=[
            "structural checks are bounded development evidence, not formal compiler verification"
        ],
        dependency_paths=["examples/executable", "crates/mncs-model", "crates/mncs-cli"],
        complete=True,
    )


def dispatch(request: dict[str, Any]) -> dict[str, object]:
    if request.get("type") == "capabilities":
        return {
            "protocol_version": "0.1",
            "type": "capabilities",
            "request_id": request.get("request_id", "unknown"),
            "provider": PROVIDER,
            "analyses": METHODS,
            "statuses": ["PASS", "FAIL", "UNKNOWN"],
            "cancellation": False,
            "health_checks": False,
            "extensions": {
                "supported_constructs": ["rust-workspace", "semantic-body", "high-level-ir", "verified-ssa", "evidence-freshness", "bounded-reference-execution", "corpus-differential-check"],
                "unsupported_constructs": ["formal-compiler-correctness", "backend-code-generation"],
                "limitations": ["normal local process; not a sandbox"],
            },
        }
    method = request.get("analysis")
    handlers = {
        "mncs-language-rust-quality": rust_quality,
        "build": rust_quality,
        "mncs-language-semantic-validation": semantic_validation,
        "inspection": semantic_validation,
        "bounded_structural_analysis": structural_workflow,
        "evidence_derivation": evidence_freshness,
        "mncs_bundle_validation": structural_workflow,
        "mncs-language-hir-trace-integrity": hir_trace,
        "mncs-language-ssa-integrity": ssa_integrity,
        "mncs-language-evidence-freshness": evidence_freshness,
        "mncs-language-execution-equivalence": execution_equivalence,
    }
    handler = handlers.get(method)
    if handler is None:
        return response(request, "UNKNOWN", "requested provider method is unsupported", unsupported=["unsupported-method"])
    return handler(request)


def main() -> int:
    try:
        request = json.loads(sys.stdin.readline())
        result = dispatch(request)
    except (json.JSONDecodeError, TypeError) as error:
        result = response({"request_id": "malformed"}, "UNKNOWN", "provider request was malformed", limitations=[str(error)], unsupported=["malformed-request"])
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
