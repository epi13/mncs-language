#!/usr/bin/env python3
"""Build the compact, revision-addressed MNCS language capability index.

The profile registry, library sources, compiler declarations, and executable
examples are the facts.  The small topic/capsule table is curated guidance
that points back to those facts; it is not a second language specification.
"""

from __future__ import annotations

import hashlib
import json
import os
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
REGISTRY = ROOT / "spec/source-profile-registry.json"
OUTPUT = ROOT / "docs/language-capabilities.json"
DELTA_OUTPUT = ROOT / "docs/language-capability-deltas.json"


def digest_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def digest_file(path: Path) -> str:
    return digest_bytes(path.read_bytes())


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def compiler_binary() -> Path:
    candidates = []
    if value := os.environ.get("MNCS_LANGUAGE_BINARY"):
        candidates.append(Path(value))
    if value := os.environ.get("CARGO_TARGET_DIR"):
        target_root = Path(value)
        if not target_root.is_absolute():
            target_root = ROOT / target_root
        candidates.extend([target_root / "debug/mncs", target_root / "release/mncs"])
    candidates.extend(
        [
            ROOT / "target/debug/mncs",
            ROOT / "target/release/mncs",
        ]
    )
    for candidate in candidates:
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate
    raise RuntimeError(
        "compiler-owned language inventory requires a built mncs binary; "
        "set MNCS_LANGUAGE_BINARY or build target/debug/mncs"
    )


def compiler_command(binary: Path, *arguments: str) -> dict[str, object]:
    environment = os.environ.copy()
    library_root = str(ROOT / "library")
    configured = environment.get("MNCS_LIBRARY_PATH")
    environment["MNCS_LIBRARY_PATH"] = (
        f"{library_root}:{configured}" if configured else library_root
    )
    result = subprocess.run(
        [str(binary), *arguments],
        cwd=ROOT,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
    )
    try:
        document = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(
            f"compiler inventory command emitted invalid JSON: {result.stderr.strip()}"
        ) from error
    if result.returncode != 0:
        diagnostics = document.get("diagnostics", []) if isinstance(document, dict) else []
        raise RuntimeError(
            f"compiler inventory command failed for {arguments!r}: "
            f"{diagnostics or result.stderr.strip()}"
        )
    return document


def compiler_language_inventory(binary: Path) -> dict[str, object]:
    document = compiler_command(binary, "language-inventory")
    if document.get("schema_version") != "mncs.language-inventory/1":
        raise RuntimeError("compiler returned an unexpected language inventory schema")
    return document


def source_facts(path: Path, binary: Path) -> dict[str, object]:
    report = compiler_command(binary, "declaration-inventory", str(path), "--syntax-only")
    inventory = report.get("inventory")
    if not report.get("valid") or not isinstance(inventory, dict):
        raise RuntimeError(f"compiler could not inventory {path}: {report.get('diagnostics')}")
    declarations = inventory.get("declarations", [])
    callables = inventory.get("callables", [])
    symbols = [
        {
            "kind": declaration.get("kind"),
            "name": declaration.get("name"),
            "identity": declaration.get("identity"),
        }
        for declaration in declarations
        if declaration.get("kind") != "module"
    ]
    imports = sorted(
        declaration["name"]
        for declaration in declarations
        if declaration.get("kind") == "import"
    )
    capabilities = sorted(
        {
            capability
            for callable_ in callables
            for capability in callable_.get("capabilities", [])
        }
    )
    effects = sorted(
        {
            (
                effect.get("kind"),
                effect.get("target"),
                effect.get("capability"),
            )
            for callable_ in callables
            for effect in callable_.get("effects", [])
        }
    )
    return {
        "module": inventory.get("module"),
        "profile": inventory.get("source_profile"),
        "path": path.relative_to(ROOT).as_posix(),
        # Projection provenance is file-addressed so Doctor can verify
        # freshness without reimplementing compiler identity derivation.
        "source_identity": digest_file(path),
        "compiler_source_identity": inventory.get("source_artifact_identity"),
        "inventory_identity": inventory.get("inventory_identity"),
        "symbols": symbols,
        "exports": sorted(
            declaration["name"]
            for declaration in declarations
            if declaration.get("exported") and declaration.get("kind") != "module"
        ),
        "imports": imports,
        "capabilities": capabilities,
        "effects": [
            {"effect": kind, "target": target, "capability": capability}
            for kind, target, capability in effects
        ],
    }


def canonical_examples(binary: Path) -> list[dict[str, object]]:
    candidates = [
        ("bounded-collections", "examples/source/bounded-min.mncs", "bounded collections"),
        ("identity", "examples/source/identity.mncs", "identity"),
        ("effects", "examples/source/cre3-retry-authority.mncs", "effects"),
        ("structured-artifacts", "examples/source/structured-artifact.mncs", "structured artifacts"),
        ("applications", "examples/source/application-entry.mncs", "applications"),
    ]
    examples = []
    for identity, relative, topic in candidates:
        path = ROOT / relative
        if path.is_file():
            inventory = source_facts(path, binary)
            examples.append(
                {
                    "identity": f"mncs.example/{identity}/1",
                    "path": relative,
                    "profile": inventory["profile"],
                    "topics": [topic],
                    "source_identity": inventory["source_identity"],
                    "compiler_source_identity": inventory["compiler_source_identity"],
                    "inventory_identity": inventory["inventory_identity"],
                    "verification": "examples/source and compiler test suites",
                }
            )
    return examples


CURATED_TOPICS = [
    {
        "id": "bounded collections",
        "description": "Use statically bounded sequences, vectors, and builders so capacity is explicit.",
        "profiles": ["0.1"],
        "modules": ["mncs.core.sequences.v1", "mncs.core.vector.v1"],
        "guidance": ["Prefer the generic bounded builder over width-specific application helpers."],
    },
    {
        "id": "effects",
        "description": "Declare and preserve capability/effect authority at the source boundary.",
        "profiles": ["0.6", "0.18"],
        "modules": ["mncs.core.proof_admit.v1", "mncs.std.application.v1"],
        "guidance": ["An effect declaration is part of the semantic contract, not host-side metadata."],
    },
    {
        "id": "structured artifacts",
        "description": "Read, write, and publish typed artifacts through the structured artifact effects.",
        "profiles": ["0.18"],
        "modules": ["mncs.std.store.v1"],
        "guidance": ["Keep persistent schema identities separate from implementation filenames."],
    },
    {
        "id": "identity",
        "description": "Bind source, content, revision, evidence, and artifact identities explicitly.",
        "profiles": ["0.1"],
        "modules": ["mncs.core.identity.v1", "mncs.core.version.v1"],
        "guidance": ["Prefer identity-bearing records over ambient host state."],
    },
    {
        "id": "profiles",
        "description": "Select a language profile from the authoritative source-profile registry.",
        "profiles": ["0.18"],
        "modules": [],
        "guidance": ["Use the current profile unless a compatibility boundary explicitly requires an older profile."],
    },
]


def build_index(binary: Path) -> dict[str, object]:
    profiles = read_json(REGISTRY)
    current = next(item for item in profiles if item["status"] == "current")
    compiler_facts = compiler_language_inventory(binary)
    library_paths = sorted((ROOT / "library").rglob("*.mncs"))
    worker_count = max(
        1,
        min(
            len(library_paths),
            int(os.environ.get("MNCS_INVENTORY_WORKERS", "4")),
        ),
    )
    # Compiler calls are independent; map preserves path order so the
    # resulting projection remains byte-stable while large modules can be
    # inventoried without turning refreshes into a serial crawl.
    with ThreadPoolExecutor(max_workers=worker_count) as workers:
        library = list(workers.map(lambda path: source_facts(path, binary), library_paths))
    provenance = [
        {
            "path": "compiler:language-inventory",
            "kind": "compiler_language_inventory",
            "source_identity": compiler_facts["source_identity"],
            "inventory_identity": compiler_facts["inventory_identity"],
        }
    ]
    provenance.append(
        {
            "path": "scripts/generate_language_capabilities.py",
            "kind": "projection_generator",
            "source_identity": digest_file(ROOT / "scripts/generate_language_capabilities.py"),
        }
    )
    provenance.extend(
        {"path": item["path"], "kind": "library_module", "source_identity": item["source_identity"]}
        for item in library
    )
    examples = canonical_examples(binary)
    provenance.extend(
        {"path": item["path"], "kind": "canonical_example", "source_identity": item["source_identity"]}
        for item in examples
    )
    # De-duplicate compiler files and keep the projection deterministic.
    provenance = sorted(
        {item["path"]: item for item in provenance}.values(), key=lambda item: item["path"]
    )
    index: dict[str, object] = {
        "schema_version": "mncs.language-capabilities/1",
        "generator_identity": "mncs-language-capabilities-generator/1",
        "language": "MNCS",
        "current_profile": current["version"],
        "profile_registry_identity": digest_file(REGISTRY),
        "compiler_inventory_identity": compiler_facts["inventory_identity"],
        "compiler_inventory": compiler_facts,
        "profiles": profiles,
        "library_modules": library,
        "intrinsics": compiler_facts["intrinsics"],
        "topics": CURATED_TOPICS,
        "examples": examples,
        "provenance": provenance,
        "projections": {
            "capsule": True,
            "topic": True,
            "symbol": True,
            "delta": True,
            "provenance": True,
            "module_dependencies": True,
            "effects": True,
            "profile_compatibility": True,
        },
        "capsule": {
            "profile": current["version"],
            "syntax": compiler_facts["syntax_features"],
            "types": [item["name"] for item in compiler_facts["type_forms"]],
            "effects": compiler_facts["effects"],
            "standard_library_namespaces": ["mncs.core", "mncs.family", "mncs.jit", "mncs.std"],
            "evolution": "source facts evolve in place; persistent contracts carry explicit identities",
        },
    }
    canonical = json.dumps(index, sort_keys=True, separators=(",", ":")).encode("utf-8")
    index["content_identity"] = digest_bytes(canonical)
    return index


def _by_key(items: list[dict[str, object]], key: str) -> dict[str, dict[str, object]]:
    return {str(item[key]): item for item in items if key in item}


def _set_delta(before: set[str], after: set[str]) -> dict[str, list[str]]:
    return {"added": sorted(after - before), "removed": sorted(before - after)}


def build_delta(before: dict[str, object], after: dict[str, object]) -> dict[str, object]:
    before_modules = _by_key(before.get("library_modules", []), "module")
    after_modules = _by_key(after.get("library_modules", []), "module")
    module_changes = _set_delta(set(before_modules), set(after_modules))
    module_changes["changed"] = sorted(
        module
        for module in set(before_modules) & set(after_modules)
        if before_modules[module].get("inventory_identity")
        != after_modules[module].get("inventory_identity")
    )
    before_profiles = _by_key(before.get("profiles", []), "version")
    after_profiles = _by_key(after.get("profiles", []), "version")
    profile_changes = _set_delta(set(before_profiles), set(after_profiles))
    profile_changes["changed"] = sorted(
        profile
        for profile in set(before_profiles) & set(after_profiles)
        if before_profiles[profile] != after_profiles[profile]
    )
    before_intrinsics = _by_key(before.get("intrinsics", []), "name")
    after_intrinsics = _by_key(after.get("intrinsics", []), "name")
    intrinsic_changes = _set_delta(set(before_intrinsics), set(after_intrinsics))
    intrinsic_changes["changed"] = sorted(
        name
        for name in set(before_intrinsics) & set(after_intrinsics)
        if before_intrinsics[name] != after_intrinsics[name]
    )
    before_exports = {
        f"{item.get('module')}::{export}"
        for item in before.get("library_modules", [])
        for export in item.get("exports", [])
    }
    after_exports = {
        f"{item.get('module')}::{export}"
        for item in after.get("library_modules", [])
        for export in item.get("exports", [])
    }
    before_effects = set(before.get("compiler_inventory", {}).get("effects", []))
    after_effects = set(after.get("compiler_inventory", {}).get("effects", []))
    before_capabilities = set(before.get("compiler_inventory", {}).get("capabilities", []))
    after_capabilities = set(after.get("compiler_inventory", {}).get("capabilities", []))
    before_examples = _by_key(before.get("examples", []), "identity")
    after_examples = _by_key(after.get("examples", []), "identity")
    example_changes = _set_delta(set(before_examples), set(after_examples))
    example_changes["changed"] = sorted(
        identity
        for identity in set(before_examples) & set(after_examples)
        if before_examples[identity] != after_examples[identity]
    )
    return {
        "previous_content_identity": before.get("content_identity"),
        "current_content_identity": after.get("content_identity"),
        "from_profile": before.get("current_profile"),
        "to_profile": after.get("current_profile"),
        "profiles": profile_changes,
        "modules": module_changes,
        "exports": _set_delta(before_exports, after_exports),
        "intrinsics": intrinsic_changes,
        "effects": _set_delta(before_effects, after_effects),
        "capabilities": _set_delta(before_capabilities, after_capabilities),
        "canonical_examples": example_changes,
    }


def write_language_delta(before: dict[str, object] | None, after: dict[str, object]) -> None:
    history = read_json(DELTA_OUTPUT) if DELTA_OUTPUT.is_file() else {
        "schema_version": "mncs.language-capability-deltas/1",
        "retention": 8,
        "deltas": [],
    }
    if before and before.get("content_identity") != after.get("content_identity"):
        history["deltas"] = [
            *history.get("deltas", []),
            build_delta(before, after),
        ][-int(history.get("retention", 8)) :]
    canonical = json.dumps(
        {key: value for key, value in history.items() if key != "history_identity"},
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    history["history_identity"] = digest_bytes(canonical)
    DELTA_OUTPUT.write_text(json.dumps(history, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> None:
    binary = compiler_binary()
    before = read_json(OUTPUT) if OUTPUT.is_file() else None
    index = build_index(binary)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(index, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    write_language_delta(before, index)
    print(OUTPUT)


if __name__ == "__main__":
    main()
