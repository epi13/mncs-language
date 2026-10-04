use std::process::Command;

use serde_json::Value;

/// Stage F: the `mncs-stdlib` checkout backing these tests: explicit
/// `MNCS_STDLIB_ROOT` wins, else the `mncs-stdlib` sibling checkout.
/// Fails closed with a clear message when absent.
fn stdlib_checkout_dir() -> String {
    // Test inputs need a real checkout: an explicitly empty variable
    // (the CLI's hermetic spelling) falls through to the sibling here.
    let explicit = std::env::var("MNCS_STDLIB_ROOT")
        .ok()
        .filter(|root| !root.trim().is_empty());
    let checkout =
        explicit.unwrap_or_else(|| format!("{}/../../../mncs-stdlib", env!("CARGO_MANIFEST_DIR")));
    assert!(
        std::path::Path::new(&checkout).is_dir(),
        "mncs-stdlib checkout missing at {checkout}; set MNCS_STDLIB_ROOT"
    );
    checkout
}

/// Stage F: standard-library sources now live in `mncs-stdlib/library/`.
fn stdlib_library_dir() -> String {
    let checkout = stdlib_checkout_dir();
    let dir = format!("{checkout}/library");
    assert!(
        std::path::Path::new(&dir).is_dir(),
        "mncs-stdlib checkout missing at {checkout}; set MNCS_STDLIB_ROOT"
    );
    dir
}

/// Stage F: stdlib-owned examples (fixtures/corpora) read across repos.
fn stdlib_example(name: &str) -> String {
    format!("{}/examples/{name}", stdlib_checkout_dir())
}

fn library_dir() -> String {
    stdlib_library_dir()
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

/// Deterministic milli-scale scoring (HARNESS-PRESSURE-002): the `fixed`
/// module blends, margins, and clamps with byte-exact expectations on both
/// executable backends, including true saturation at the i64 bounds.
#[test]
fn fixed_scores_execute_and_agree_on_both_backends() {
    let source = format!("{}/std/fixed.mncs", library_dir());
    let corpus = format!(
        "{}/examples/execution/fixed-corpus.json",
        stdlib_checkout_dir()
    );
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
            .expect("run fixed experiment");
        assert!(
            output.status.success(),
            "{backend}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
        // Saturating intents discharge every obligation: overall PASS.
        assert_eq!(result["status"], "PASS", "{backend}: {result}");
        for case in result["cases"].as_array().unwrap() {
            assert_eq!(case["status"], "returned", "{backend}: {case}");
            assert_eq!(case["expectation_met"], true, "{backend}: {case}");
        }
    }
}

/// The fixed-point operations agree across the layered reference executors.
#[test]
fn fixed_scores_agree_across_layers() {
    let source = format!("{}/std/fixed.mncs", library_dir());
    let corpus = format!(
        "{}/examples/execution/fixed-corpus.json",
        stdlib_checkout_dir()
    );
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["check-backend-execution", &source, &corpus])
        .output()
        .expect("run layered fixed check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("layered JSON");
    assert_eq!(result["status"], "consistent_over_corpus");
    assert_eq!(result["mismatching_cases"], 0);
}
