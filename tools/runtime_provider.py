#!/usr/bin/env python3
"""Selected Stage-0/reference executable build and source-origin inspection."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def digest(value):
    return hashlib.sha256(encoded(value)).hexdigest()


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def toolchain_mismatches(configuration):
    tools = configuration.get("toolchain_executables") if isinstance(configuration, dict) else None
    if not isinstance(tools, dict) or set(tools) != {"cargo", "rustc"}:
        return ["toolchain-executable-identities"]
    mismatches = []
    for name, identity in tools.items():
        try:
            if not isinstance(identity, dict):
                raise ValueError("invalid tool identity")
            configured_path = identity["configured_path"]
            configured = Path(configured_path)
            if not configured.is_absolute() and configured.parent == Path("."):
                configured = Path(shutil.which(configured_path) or configured_path)
            configured = configured.resolve(strict=True)
            resolved = Path(identity["resolved_path"]).resolve(strict=True)
            if configured != resolved or sha(resolved) != identity["sha256"]:
                mismatches.append("build-tool:" + name)
        except (OSError, KeyError, TypeError, ValueError):
            mismatches.append("build-tool:" + name)
    return mismatches


def inspect(executable: Path, *, expected_checkout: Path | None = None):
    executable = Path(executable).resolve()
    result = subprocess.run([str(executable), "--build-info"], capture_output=True, text=True, timeout=15, check=False)
    if result.returncode:
        return {"schema_version": "mncs.language.selected-runtime/1", "executable": str(executable),
                "executable_sha256": sha(executable), "build_origin": {"status": "unknown",
                "reason": "selected executable has no readable embedded build receipt"}}
    try:
        embedded = json.loads(result.stdout)
    except ValueError as error:
        raise RuntimeError("selected reference executable returned malformed build evidence") from error
    receipt = embedded.get("receipt") if isinstance(embedded, dict) else None
    mismatches = []
    if (not isinstance(receipt, dict) or receipt.get("schema_version") != "mncs.language-build-receipt/1"
            or embedded.get("identity") != "sha256:" + digest(receipt)):
        mismatches.append("receipt-integrity")
        receipt = receipt if isinstance(receipt, dict) else {}
    inputs = receipt.get("source_inputs", {})
    checkout = Path(receipt.get("checkout", "")).resolve()
    if expected_checkout is not None and checkout != Path(expected_checkout).resolve():
        mismatches.append("selected-checkout")
    if not isinstance(inputs, dict):
        mismatches.append("source-input-table")
        inputs = {}
    for relative, expected in inputs.items():
        path = (checkout / relative).resolve()
        try:
            if not path.is_relative_to(checkout) or not path.is_file() or sha(path) != expected:
                mismatches.append(relative)
        except OSError:
            mismatches.append(relative)
    if digest(inputs) != receipt.get("source_inputs_identity"):
        mismatches.append("source-input-identity")
    try:
        revision = subprocess.run(["git", "-C", str(checkout), "rev-parse", "HEAD"], capture_output=True, text=True, timeout=5, check=True).stdout.strip()
        status = subprocess.run(["git", "-C", str(checkout), "status", "--porcelain=v1", "--untracked-files=all"], capture_output=True, text=True, timeout=10, check=True).stdout.splitlines()
        changed = {line[3:].rsplit(" -> ", 1)[-1] for line in status if len(line) >= 4}
        dirty_inputs = {path: identity for path, identity in inputs.items() if path in changed}
        if revision != receipt.get("source_revision"):
            mismatches.append("source-revision")
        if digest(dirty_inputs) != receipt.get("dirty_content_identity") or len(dirty_inputs) != receipt.get("dirty_input_count"):
            mismatches.append("dirty-checkout-identity")
    except (OSError, subprocess.SubprocessError):
        mismatches.append("checkout-state-unavailable")
    mismatches.extend(toolchain_mismatches(receipt.get("build_configuration")))
    executable_sha = sha(executable)
    return {"schema_version": "mncs.language.selected-runtime/1", "executable": str(executable),
            "executable_sha256": executable_sha,
            "build_origin": {"status": "stale-inputs" if mismatches else "matches-embedded-inputs",
                "receipt_schema": receipt.get("schema_version"), "receipt_identity": embedded.get("identity"),
                "source_revision": receipt.get("source_revision"),
                "source_inputs_identity": receipt.get("source_inputs_identity"),
                "source_input_count": len(inputs), "dirty_content_identity": receipt.get("dirty_content_identity"),
                "dirty_input_count": receipt.get("dirty_input_count"),
                "build_configuration": receipt.get("build_configuration", {}),
                "mismatch_count": len(mismatches), "mismatches": mismatches[:32],
                "assurance": receipt.get("assurance", "unknown"),
                "runtime_build_identity": digest({"kind": "mncs.language.runtime-build/1",
                    "receipt_identity": embedded.get("identity"), "executable_sha256": executable_sha})}}


def build(executable: Path, cargo: str):
    expected = (ROOT / "target/release/mncs").resolve()
    if Path(executable).resolve() != expected:
        raise RuntimeError("provider build is bound to target/release/mncs")
    result = subprocess.run([cargo, "build", "--offline", "--locked", "--release", "-p", "mncs-cli"],
        cwd=ROOT, capture_output=True, text=True, timeout=1800, check=False,
        env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"})
    if result.returncode:
        raise RuntimeError("selected Stage-0/reference build failed: " + result.stderr[-3000:])
    runtime = inspect(expected)
    if runtime.get("build_origin", {}).get("status") != "matches-embedded-inputs":
        origin = runtime.get("build_origin", {})
        raise RuntimeError("rebuilt Stage-0/reference executable does not match its embedded inputs: "
                           + json.dumps(origin.get("mismatches", []), sort_keys=True))
    return {"schema_version": "mncs.language.build-operation/1", "status": "built-and-verified-locally",
            "build_command": [cargo, "build", "--offline", "--locked", "--release", "-p", "mncs-cli"],
            "runtime_identity": runtime, "build_stderr_tail": result.stderr[-1000:]}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("inspect", "build"))
    parser.add_argument("--executable", default=str(ROOT / "target/release/mncs"))
    parser.add_argument("--cargo", default=os.environ.get("CARGO", "cargo"))
    args = parser.parse_args(argv)
    try:
        value = inspect(Path(args.executable), expected_checkout=ROOT) if args.operation == "inspect" else build(Path(args.executable), args.cargo)
        print(json.dumps(value, sort_keys=True))
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(json.dumps({"schema_version": "mncs.language.selected-runtime/1", "status": "error", "reason": str(error)}))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
