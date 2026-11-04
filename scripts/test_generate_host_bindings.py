from __future__ import annotations

import unittest
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from generate_host_bindings import python_binding, rust_binding


ABI = {
    "schema_version": "0.1",
    "interface_identity": "a" * 64,
    "module": "probe.bindings.v1",
    "functions": {
        "feed": {
            "declaring_module": "probe.bindings.v1",
            "input_names": ["bytes", "state"],
            "inputs": [
                {"view": {"element": "byte", "capacity": 64}},
                {
                    "sequence": {
                        "element": {"integer": {"bits": 64, "signed": False}},
                        "length": 6,
                    }
                },
            ],
            "outputs": [
                {
                    "sequence": {
                        "element": {"integer": {"bits": 64, "signed": False}},
                        "length": 6,
                    }
                }
            ],
        }
    },
    "composites": {"probe.bindings.v1::Marker": {"finite": {"name": "Marker"}}},
}


class HostBindingGenerationTests(unittest.TestCase):
    def test_rust_binding_supports_bounded_view_and_multi_argument_callable(self) -> None:
        rendered = rust_binding(ABI)
        self.assertIn("pub fn feed(session: &Session, bytes: Vec<u8>, state: Vec<u64>", rendered)
        self.assertIn('{"byte": {"value": item}}', rendered)
        self.assertIn("typed_call(session, \"probe.bindings.v1\", \"feed\"", rendered)

    def test_python_binding_supports_byte_views_and_multi_argument_callable(self) -> None:
        rendered = python_binding(ABI)
        self.assertIn("def feed(self, bytes: bytes, state: tuple[Any, ...])", rendered)
        self.assertIn("typed_arguments = [_encode(argument) for argument in arguments]", rendered)
        self.assertIn("'call', str(self._config.source)", rendered)
        self.assertIn("return bytes(decoded) if element_descriptor == 'byte' else decoded", rendered)

    def test_rust_record_decoder_accepts_object_and_typed_pair_fields(self) -> None:
        rendered = rust_binding(ABI)
        self.assertIn("if let Some(fields) = fields.as_object()", rendered)
        self.assertIn("let pairs = fields.as_array()", rendered)
        self.assertIn("typed record contains a malformed field pair", rendered)
        self.assertIn("typed record contains duplicate field", rendered)

    def test_rust_binding_clones_opaque_json_returns_into_owned_values(self) -> None:
        abi = {
            **ABI,
            "functions": {
                **ABI["functions"],
                "opaque": {
                    "declaring_module": "probe.bindings.v1",
                    "inputs": [],
                    "outputs": [{"scalar": {"semantic_type": "opaque"}}],
                },
            },
        }
        rendered = rust_binding(abi)
        self.assertIn(
            "pub fn opaque(session: &Session, options: CallOptions) -> Result<serde_json::Value, EmbedError>",
            rendered,
        )
        self.assertIn("Ok((*&value).clone())", rendered)

    def test_python_record_decoder_rejects_malformed_and_duplicate_pairs(self) -> None:
        rendered = python_binding(ABI)
        self.assertIn("for pair in pairs:", rendered)
        self.assertIn("typed record contains a malformed field pair", rendered)
        self.assertIn("typed record contains duplicate field", rendered)


if __name__ == "__main__":
    unittest.main()
