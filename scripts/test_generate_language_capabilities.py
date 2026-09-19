#!/usr/bin/env python3
"""Regression checks for the authoritative language capability projection."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
INDEX = ROOT / "docs/language-capabilities.json"


def main() -> None:
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_language_capabilities.py")], check=True)
    value = json.loads(INDEX.read_text(encoding="utf-8"))
    assert value["schema_version"] == "mncs.language-capabilities/1"
    assert value["current_profile"] == "0.18"
    assert value["library_modules"]
    assert value["examples"]
    assert any(module["imports"] for module in value["library_modules"])
    assert any(module["effects"] for module in value["library_modules"])
    assert any(item["kind"] == "projection_generator" for item in value["provenance"])
    assert value["compiler_inventory"]["schema_version"] == "mncs.language-inventory/1"
    assert value["compiler_inventory_identity"] == value["compiler_inventory"]["inventory_identity"]
    assert value["intrinsics"] == value["compiler_inventory"]["intrinsics"]
    assert all(not item["name"].startswith("elaborate_") for item in value["intrinsics"])
    assert all("inventory_identity" in module for module in value["library_modules"])
    assert value["projections"] == {
        "capsule": True,
        "delta": True,
        "effects": True,
        "module_dependencies": True,
        "profile_compatibility": True,
        "provenance": True,
        "symbol": True,
        "topic": True,
    }
    content_identity = value.pop("content_identity")
    canonical = json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")
    expected = "sha256:" + hashlib.sha256(canonical).hexdigest()
    assert content_identity == expected, (content_identity, expected)
    deltas = json.loads((ROOT / "docs/language-capability-deltas.json").read_text(encoding="utf-8"))
    assert deltas["schema_version"] == "mncs.language-capability-deltas/1"
    assert deltas["history_identity"].startswith("sha256:")
    print(f"language capability index verified: {content_identity}")


if __name__ == "__main__":
    main()
