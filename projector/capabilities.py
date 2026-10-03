"""Ambient semantic entry points for mncs-language capability projections.

These repo-owned renderers adapt the existing
``scripts/generate_language_capabilities.py`` to the projection protocol:
the generator is imported (never rewritten or forked) and its pure build
functions are reused. The wrappers return content instead of writing files
(only Environment writes claimed target bytes) and enforce validation
before any byte is returned.

Semantic checks here mirror ``scripts/test_generate_language_capabilities.py``,
which remains the authoritative regression suite in CI. Transport shape
checks live in the projection declarations.

Evaluation order matters for the delta journal: the
``mncs-language:language-capability-deltas`` declaration precedes
``mncs-language:language-capabilities`` in the projection inventory, and
explicit applies must keep that order. The journal renderer reads the
committed index as its before-state; if the index row is reconciled first
in the same pass, the transition is unobservable and the renderer fails
loudly (history linkage mismatch) instead of silently dropping it.
Recovery is operator-level: restore ``docs/language-capabilities.json``
from git HEAD and re-apply deltas-first.

The compiler binary (``target/debug/mncs`` or ``MNCS_LANGUAGE_BINARY``) is
a renderer-environment dependency, like the toolchain itself. Its emitted
identities are embedded in the output and linkage-checked here; a missing
or incompatible binary fails the render with the generator's own
diagnosis rather than producing output.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
from typing import Any

CHECKOUT = Path(__file__).resolve().parents[1]

# The generator's worker pool affects only parallelism (path-ordered map
# keeps output byte-stable; verified identical for 1, 2, and 4 workers).
# One worker keeps peak compiler-binary residency to a single process and
# still renders in ~66s, inside the render timeout; an explicitly
# configured value is always respected.
os.environ.setdefault("MNCS_INVENTORY_WORKERS", "1")

EXPECTED_PROJECTION_FLAGS = {
    "capsule": True,
    "delta": True,
    "effects": True,
    "module_dependencies": True,
    "profile_compatibility": True,
    "provenance": True,
    "symbol": True,
    "topic": True,
}


def _load_generator():
    """Import the semantic renderer without executing its file writes."""
    path = CHECKOUT / "scripts/generate_language_capabilities.py"
    spec = importlib.util.spec_from_file_location(
        "language_capability_generator", path
    )
    if spec is None or spec.loader is None:
        raise ValueError("capability generator module not loadable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _canonical_bytes(value: dict[str, Any]) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def _digest_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def _registry_current_version(model: dict[str, Any]) -> str | None:
    registry = (model.get("values") or {}).get("registry")
    if not isinstance(registry, list):
        return None
    for item in registry:
        if isinstance(item, dict) and item.get("status") == "current":
            version = item.get("version")
            return version if isinstance(version, str) else None
    return None


def _check(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError("capability semantic validation failed: " + message)


def _validate_index(index: dict[str, Any], model: dict[str, Any]) -> None:
    """Mirror the repository generator test, minus regeneration."""
    _check(index.get("schema_version") == "mncs.language-capabilities/1",
           "schema_version %r" % (index.get("schema_version"),))
    expected_profile = _registry_current_version(model)
    if expected_profile is not None:
        _check(index.get("current_profile") == expected_profile,
               "current_profile %r != registry current %r"
               % (index.get("current_profile"), expected_profile))
    modules = index.get("library_modules") or []
    examples = index.get("examples") or []
    _check(bool(modules), "empty library_modules")
    _check(bool(examples), "empty examples")
    _check(any(module.get("imports") for module in modules),
           "no module imports observed")
    _check(any(module.get("effects") for module in modules),
           "no module effects observed")
    provenance = index.get("provenance") or []
    _check(any(item.get("kind") == "projection_generator" for item in provenance),
           "projection_generator provenance missing")
    inventory = index.get("compiler_inventory") or {}
    _check(inventory.get("schema_version") == "mncs.language-inventory/1",
           "compiler inventory schema %r" % (inventory.get("schema_version"),))
    _check(index.get("compiler_inventory_identity") == inventory.get("inventory_identity"),
           "compiler inventory identity linkage broken")
    _check(index.get("intrinsics") == inventory.get("intrinsics"),
           "intrinsics diverge from compiler inventory")
    _check(all(not item.get("name", "").startswith("elaborate_")
               for item in index.get("intrinsics") or []),
           "private elaborate_* helper leaked into index")
    _check(all("inventory_identity" in module for module in modules),
           "module missing inventory_identity")
    _check(index.get("projections") == EXPECTED_PROJECTION_FLAGS,
           "projection flags %r" % (index.get("projections"),))
    content_identity = index.get("content_identity")
    rest = {key: value for key, value in index.items() if key != "content_identity"}
    _check(content_identity == _digest_bytes(_canonical_bytes(rest)),
           "content_identity does not match canonical bytes")


def _validate_history(history: dict[str, Any]) -> None:
    _check(history.get("schema_version") == "mncs.language-capability-deltas/1",
           "delta schema_version %r" % (history.get("schema_version"),))
    retention = int(history.get("retention", 8))
    deltas = history.get("deltas") or []
    _check(len(deltas) <= retention, "retention %d exceeded" % retention)
    rest = {key: value for key, value in history.items() if key != "history_identity"}
    _check(history.get("history_identity") == _digest_bytes(_canonical_bytes(rest)),
           "history_identity does not match canonical bytes")


def render_language_capabilities(model: dict[str, Any]) -> str:
    """Render the capability index from corpus sources via the generator."""
    generator = _load_generator()
    binary = generator.compiler_binary()
    index = generator.build_index(binary)
    _validate_index(index, model)
    return json.dumps(index, indent=2, sort_keys=True) + "\n"


def render_language_capability_deltas(model: dict[str, Any]) -> str:
    """Render the delta journal over the committed index before-state."""
    generator = _load_generator()
    # The committed journal is derived state, not a semantic subject (a
    # projection cannot observe its own output); read it as the history
    # base exactly like the generator's own delta writer does.
    history = {
        "schema_version": "mncs.language-capability-deltas/1",
        "retention": 8,
        "deltas": [],
    }
    history_path = CHECKOUT / "docs/language-capability-deltas.json"
    if history_path.is_file():
        on_disk = json.loads(history_path.read_text(encoding="utf-8"))
        if isinstance(on_disk, dict):
            history = dict(on_disk)
    before = None
    index_path = CHECKOUT / "docs/language-capabilities.json"
    if index_path.is_file():
        before = json.loads(index_path.read_text(encoding="utf-8"))
    if before is not None:
        entries = history.get("deltas") or []
        if entries:
            latest = entries[-1].get("current_content_identity")
            _check(latest == before.get("content_identity"),
                   "journal linkage broken: latest entry %r != committed index %r; "
                   "reconcile language-capability-deltas before "
                   "language-capabilities in the same pass" % (latest, before.get("content_identity")))
    binary = generator.compiler_binary()
    after = generator.build_index(binary)
    if before is not None and before.get("content_identity") != after.get("content_identity"):
        history["deltas"] = [
            *history.get("deltas", []),
            generator.build_delta(before, after),
        ][-int(history.get("retention", 8)):]
    rest = {key: value for key, value in history.items() if key != "history_identity"}
    history["history_identity"] = _digest_bytes(_canonical_bytes(rest))
    _validate_history(history)
    return json.dumps(history, indent=2, sort_keys=True) + "\n"
