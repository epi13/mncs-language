use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

/// Bounded canonical JSON emission (HARNESS-PRESSURE-009): the `json_emit`
/// writer produces byte-exact golden output — metrics rows, arrays,
/// escapes, null, capacity poisoning, fail-closed mismatched ends — on
/// both executable backends.
#[test]
fn json_emit_produces_golden_bytes_on_both_backends() {
    let source = format!("{}/std/json_emit.mncs", library_dir());
    let corpus = example("execution/json-emit-corpus.json");
    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let output = binary()
            .env("MNCS_LIBRARY_PATH", library_dir())
            .args([
                "experiment",
                "run",
                &source,
                "--backend",
                backend,
                "--corpus",
                &corpus,
            ])
            .output()
            .expect("run json-emit experiment");
        assert!(
            output.status.success(),
            "{backend}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
        // Overall UNKNOWN comes only from honest division obligations in
        // decimal rendering; every golden vector must still be met.
        for case in result["cases"].as_array().unwrap() {
            assert_eq!(case["status"], "returned", "{backend}: {case}");
            assert_eq!(case["expectation_met"], true, "{backend}: {case}");
        }
    }
}

/// Emission round-trips through the existing scanner: an emitted metrics
/// row scans as one complete value with the expected summary.
#[test]
fn json_emit_roundtrips_through_scanner_on_both_backends() {
    let source = example("source/json-emit-roundtrip.mncs");
    let corpus = example("execution/json-emit-roundtrip-corpus.json");
    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let output = binary()
            .env("MNCS_LIBRARY_PATH", library_dir())
            .args([
                "experiment",
                "run",
                &source,
                "--backend",
                backend,
                "--corpus",
                &corpus,
            ])
            .output()
            .expect("run round-trip experiment");
        assert!(
            output.status.success(),
            "{backend}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
        for case in result["cases"].as_array().unwrap() {
            assert_eq!(case["status"], "returned", "{backend}: {case}");
            assert_eq!(case["expectation_met"], true, "{backend}: {case}");
        }
    }
}
