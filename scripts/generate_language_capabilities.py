#!/usr/bin/env python3
"""Build the compact, revision-addressed MNCS language capability index.

The profile registry, library sources, compiler declarations, and executable
examples are the facts.  The small topic/capsule table is curated guidance
that points back to those facts; it is not a second language specification.
"""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REGISTRY = ROOT / "spec/source-profile-registry.json"
OUTPUT = ROOT / "docs/language-capabilities.json"


def digest_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def digest_file(path: Path) -> str:
    return digest_bytes(path.read_bytes())


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def source_facts(path: Path) -> dict[str, object]:
    text = path.read_text(encoding="utf-8")
    profile = re.search(r"^mncs\s+([^;]+);", text, re.MULTILINE)
    module = re.search(r"^module\s+([^;]+);", text, re.MULTILINE)
    declarations: list[dict[str, str]] = []
    patterns = (
        ("record", r"^record\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{"),
        ("enum", r"^enum\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{"),
        ("type", r"^type\s+([A-Za-z_][A-Za-z0-9_]*)\b"),
        ("function", r"^fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\("),
    )
    for kind, pattern in patterns:
        for match in re.finditer(pattern, text, re.MULTILINE):
            declarations.append({"kind": kind, "name": match.group(1)})
    declarations.sort(key=lambda item: (item["name"], item["kind"]))
    imports = sorted(
        {
            match.group(1).split(" as ", 1)[0].strip()
            for match in re.finditer(r"^\s*use\s+([^;]+);", text, re.MULTILINE)
        }
    )
    capabilities = sorted(
        set(re.findall(r"^\s*capability\s+([A-Za-z_][A-Za-z0-9_]*)\b", text, re.MULTILINE))
    )
    effects = [
        {
            "capability": match.group(2),
            "effect": match.group(1),
            "authorized_by": match.group(2),
        }
        for match in re.finditer(
            r"^\s*effect\s+([A-Za-z_][A-Za-z0-9_]*)\s+authorized_by\s+([A-Za-z_][A-Za-z0-9_]*)",
            text,
            re.MULTILINE,
        )
    ]
    effects.sort(key=lambda item: (item["effect"], item["capability"], item["authorized_by"]))
    return {
        "module": module.group(1) if module else None,
        "profile": profile.group(1) if profile else None,
        "path": path.relative_to(ROOT).as_posix(),
        "source_identity": digest_file(path),
        "symbols": declarations,
        "exports": [item["name"] for item in declarations],
        "imports": imports,
        "capabilities": capabilities,
        "effects": effects,
    }


def compiler_intrinsics() -> list[dict[str, str]]:
    facts: list[dict[str, str]] = []
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        text = path.read_text(encoding="utf-8", errors="replace")
        for name in sorted(set(re.findall(r"\b(elaborate_[A-Za-z0-9_]+)\b", text))):
            facts.append(
                {
                    "name": name,
                    "source": path.relative_to(ROOT).as_posix(),
                    "source_identity": digest_file(path),
                }
            )
    return facts


def canonical_examples() -> list[dict[str, object]]:
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
            examples.append(
                {
                    "identity": f"mncs.example/{identity}/1",
                    "path": relative,
                    "profile": re.search(
                        r"^mncs\s+([^;]+);", path.read_text(encoding="utf-8"), re.MULTILINE
                    ).group(1),
                    "topics": [topic],
                    "source_identity": digest_file(path),
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


def build_index() -> dict[str, object]:
    profiles = read_json(REGISTRY)
    current = next(item for item in profiles if item["status"] == "current")
    library = [source_facts(path) for path in sorted((ROOT / "library").rglob("*.mncs"))]
    provenance = [
        {
            "path": REGISTRY.relative_to(ROOT).as_posix(),
            "kind": "profile_registry",
            "source_identity": digest_file(REGISTRY),
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
    examples = canonical_examples()
    provenance.extend(
        {"path": item["path"], "kind": "canonical_example", "source_identity": item["source_identity"]}
        for item in examples
    )
    provenance.extend(
        {"path": item["source"], "kind": "compiler_source", "source_identity": item["source_identity"]}
        for item in compiler_intrinsics()
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
        "profiles": profiles,
        "library_modules": library,
        "intrinsics": compiler_intrinsics(),
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
            "syntax": ["module", "use", "record", "enum", "fn", "bounded sequences", "effects"],
            "types": ["nominal records", "finite enums", "bounded sequences", "views", "byte arrays"],
            "effects": ["declared capabilities", "structured_read", "structured_write", "artifact publication"],
            "standard_library_namespaces": ["mncs.core", "mncs.family", "mncs.jit", "mncs.std"],
            "evolution": "source facts evolve in place; persistent contracts carry explicit identities",
        },
    }
    canonical = json.dumps(index, sort_keys=True, separators=(",", ":")).encode("utf-8")
    index["content_identity"] = digest_bytes(canonical)
    return index


def main() -> None:
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(build_index(), indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(OUTPUT)


if __name__ == "__main__":
    main()
