#!/usr/bin/env python3
"""Generate mncs-language provider facts from its compiler-owned export manifest.

The source manifest is the narrow authoritative list of contracts emitted by
the compiler/runtime boundary.  This generator binds that list to its exact
content identity and checks it against the human-reviewable family
declaration.  It never infers consumer relationships or scrapes arbitrary
source text.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


SOURCE_SCHEMA = "mncs-language.family-provider-source/v1"
OUTPUT_SCHEMA = "commons.mncs.generated-provider-metadata/v1"
GENERATOR_VERSION = "mncs-language-provider-facts/0.1"


def canonical(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False
    ).encode("utf-8")


def digest(value: Any) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


def read_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {label} {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a JSON object")
    return value


def normalize_providers(value: Any, *, source_root: Path) -> list[dict[str, str]]:
    if not isinstance(value, list) or not value:
        raise ValueError("provider source must contain a non-empty providers array")
    normalized: list[dict[str, str]] = []
    identities: set[str] = set()
    for index, raw in enumerate(value):
        if not isinstance(raw, dict):
            raise ValueError(f"providers[{index}] must be an object")
        fields = ("contract_identity", "contract_revision", "exported_identity", "evidence")
        if any(not isinstance(raw.get(field), str) or not raw[field] for field in fields):
            raise ValueError(f"providers[{index}] is incomplete")
        identity = raw["contract_identity"]
        if identity in identities:
            raise ValueError(f"providers repeats {identity}")
        identities.add(identity)
        evidence = Path(raw["evidence"])
        if evidence.is_absolute() or ".." in evidence.parts:
            raise ValueError(f"providers[{index}].evidence must be a bounded relative path")
        if not (source_root / evidence).is_file():
            raise ValueError(f"providers[{index}].evidence does not exist: {raw['evidence']}")
        normalized.append({field: raw[field] for field in fields})
    return sorted(normalized, key=lambda item: item["contract_identity"])


def evidence_digests(providers: list[dict[str, str]], *, source_root: Path) -> dict[str, str]:
    output: dict[str, str] = {}
    for provider in providers:
        path = source_root / provider["evidence"]
        output[provider["evidence"]] = hashlib.sha256(path.read_bytes()).hexdigest()
    return dict(sorted(output.items()))


def render(source_path: Path, declaration_path: Path) -> str:
    source = read_object(source_path, "provider source")
    if source.get("schema_version") != SOURCE_SCHEMA:
        raise ValueError(f"provider source must be {SOURCE_SCHEMA}")
    repository_id = source.get("repository_id")
    if repository_id != "mncs-language":
        raise ValueError("provider source repository_id must be mncs-language")
    authority = source.get("authority")
    if not isinstance(authority, dict):
        raise ValueError("provider source authority must be an object")
    if authority.get("kind") != "compiler-owned-export-manifest":
        raise ValueError("provider source authority.kind is not compiler-owned")
    module_identity = authority.get("module_identity")
    if not isinstance(module_identity, str) or not module_identity:
        raise ValueError("provider source authority.module_identity is required")
    providers = normalize_providers(source.get("providers"), source_root=source_path.parent)
    evidence = evidence_digests(providers, source_root=source_path.parent)

    declaration = read_object(declaration_path, "family declaration")
    declared = declaration.get("provides")
    if not isinstance(declared, list):
        raise ValueError("family declaration provides must be an array")
    fields = ("contract_identity", "contract_revision", "exported_identity", "evidence")
    normalized_declared = []
    for index, raw in enumerate(declared):
        if not isinstance(raw, dict) or any(not isinstance(raw.get(field), str) for field in fields):
            raise ValueError(f"family declaration provides[{index}] is incomplete")
        normalized_declared.append({field: raw[field] for field in fields})
    if sorted(normalized_declared, key=lambda item: item["contract_identity"]) != providers:
        raise ValueError("family declaration provides are stale relative to the compiler-owned export manifest")

    interface_material = {
        "schema_version": SOURCE_SCHEMA,
        "repository_id": repository_id,
        "authority": authority,
        "providers": providers,
        "evidence_digests": evidence,
    }
    interface_identity = digest(interface_material)
    binding_identity = digest(
        {
            "generator_version": GENERATOR_VERSION,
            "interface_identity": interface_identity,
            "providers": providers,
        }
    )
    provider_fact_identity = digest(
        {
            "binding_language": "compiler-export-manifest",
            "generator_version": GENERATOR_VERSION,
            "module_identity": module_identity,
            "interface_identity": interface_identity,
            "providers": providers,
        }
    )
    output = {
        "schema_version": OUTPUT_SCHEMA,
        "repository_id": repository_id,
        "authority": {
            "kind": "language-owned-export-manifest",
            "module_identity": module_identity,
            "interface_identity": interface_identity,
            "generator_version": GENERATOR_VERSION,
            "binding_content_identity": binding_identity,
            "provider_fact_identity": provider_fact_identity,
            "evidence_digests": evidence,
        },
        "providers": providers,
    }
    return json.dumps(output, indent=2, ensure_ascii=False, sort_keys=False) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=Path("family-provider-source-v1.json"))
    parser.add_argument("--declaration", type=Path, default=Path("family-semantic-contracts-v1.json"))
    parser.add_argument("--output", type=Path, default=Path("family-provider-metadata-v1.json"))
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        rendered = render(args.source.resolve(), args.declaration.resolve())
        if args.check:
            current = args.output.read_text(encoding="utf-8")
            if current != rendered:
                print(f"stale provider metadata: regenerate {args.output}", file=sys.stderr)
                return 1
        else:
            args.output.write_text(rendered, encoding="utf-8")
    except (OSError, ValueError) as error:
        print(f"provider metadata error: {error}", file=sys.stderr)
        return 1
    print(digest(json.loads(rendered)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
