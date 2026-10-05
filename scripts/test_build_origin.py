import importlib.util
import tempfile
import unittest
from pathlib import Path

MODULE = Path(__file__).resolve().parents[1] / "tools/runtime_provider.py"
SPEC = importlib.util.spec_from_file_location("language_runtime_provider", MODULE)
provider = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provider)


class ToolchainReceiptTests(unittest.TestCase):
    def test_exact_selected_tool_bytes_are_verified(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            tools = {}
            for name in ("cargo", "rustc"):
                path = root / name
                path.write_bytes((name + " selected bytes").encode())
                tools[name] = {"configured_path": str(path), "resolved_path": str(path.resolve()), "sha256": provider.sha(path)}
            self.assertEqual(provider.toolchain_mismatches({"toolchain_executables": tools}), [])
            (root / "rustc").write_bytes(b"different bytes")
            self.assertEqual(provider.toolchain_mismatches({"toolchain_executables": tools}), ["build-tool:rustc"])
            self.assertEqual(provider.toolchain_mismatches({}), ["toolchain-executable-identities"])


if __name__ == "__main__":
    unittest.main()
