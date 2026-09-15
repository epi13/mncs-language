#!/usr/bin/env python3
"""Determinism and stale-source tests for family provider metadata."""

from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path

import generate_family_provider_metadata as generator


ROOT = Path(__file__).resolve().parents[1]


class FamilyProviderMetadataTests(unittest.TestCase):
    def test_checked_in_output_is_current(self) -> None:
        rendered = generator.render(
            ROOT / "family-provider-source-v1.json",
            ROOT / "family-semantic-contracts-v1.json",
        )
        self.assertEqual(rendered, (ROOT / "family-provider-metadata-v1.json").read_text(encoding="utf-8"))

    def test_source_change_changes_identity_and_old_output_is_stale(self) -> None:
        source = json.loads((ROOT / "family-provider-source-v1.json").read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory(prefix="mncs-language-provider-test-") as directory:
            root = Path(directory)
            source_path = root / "family-provider-source-v1.json"
            evidence = root / "crates/mncs-cli/src/main.rs"
            evidence.parent.mkdir(parents=True)
            shutil.copyfile(ROOT / "crates/mncs-cli/src/main.rs", evidence)
            source_path.write_text(json.dumps(source), encoding="utf-8")
            first = generator.render(source_path, ROOT / "family-semantic-contracts-v1.json")
            evidence.write_text(evidence.read_text(encoding="utf-8") + "\n", encoding="utf-8")
            evidence_changed = generator.render(source_path, ROOT / "family-semantic-contracts-v1.json")
            self.assertNotEqual(
                json.loads(first)["authority"]["interface_identity"],
                json.loads(evidence_changed)["authority"]["interface_identity"],
            )
            source["providers"][0]["contract_revision"] = "0.18"
            source_path.write_text(json.dumps(source), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "declaration provides are stale"):
                generator.render(source_path, ROOT / "family-semantic-contracts-v1.json")
            declaration = json.loads((ROOT / "family-semantic-contracts-v1.json").read_text(encoding="utf-8"))
            declaration["provides"][0]["contract_revision"] = "0.18"
            declaration_path = root / "family-semantic-contracts-v1.json"
            declaration_path.write_text(json.dumps(declaration), encoding="utf-8")
            second = generator.render(source_path, declaration_path)
        self.assertNotEqual(first, second)
        self.assertNotEqual(
            json.loads(first)["authority"]["interface_identity"],
            json.loads(second)["authority"]["interface_identity"],
        )


if __name__ == "__main__":
    unittest.main()
