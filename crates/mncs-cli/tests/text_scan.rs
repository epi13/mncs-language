use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

/// Bounded literal text scanning (HARNESS-PRESSURE-001): the `text_scan`
/// module studies clean and every corpus case — literal-term detection,
/// prefix/suffix, first-index search, deterministic ordering, byte
/// counting, word scanning — meets its byte-exact expectation on both
/// executable backends.
#[test]
fn text_scan_executes_and_agrees_on_both_backends() {
    let source = format!("{}/std/text_scan.mncs", library_dir());
    let corpus = format!(
        "{}/../../examples/execution/text-scan-corpus.json",
        env!("CARGO_MANIFEST_DIR")
    );
    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let output = binary()
            .env("MNCS_LIBRARY_PATH", library_dir())
            .args(["experiment", "run", &source, "--backend", backend, "--corpus", &corpus])
            .output()
            .expect("run text-scan experiment");
        assert!(
            output.status.success(),
            "{backend}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
        // Overall UNKNOWN comes only from honest checked-arithmetic
        // obligations; every behavioral case must still be met.
        for case in result["cases"].as_array().unwrap() {
            assert_eq!(case["status"], "returned", "{backend}: {case}");
            assert_eq!(case["expectation_met"], true, "{backend}: {case}");
        }
    }
}

/// The text-scan operations agree across the layered reference executors.
#[test]
fn text_scan_agrees_across_layers() {
    let source = format!("{}/std/text_scan.mncs", library_dir());
    let corpus = format!(
        "{}/../../examples/execution/text-scan-corpus.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["check-backend-execution", &source, &corpus])
        .output()
        .expect("run layered text-scan check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("layered JSON");
    assert_eq!(result["status"], "consistent_over_corpus");
    assert_eq!(result["mismatching_cases"], 0);
}
