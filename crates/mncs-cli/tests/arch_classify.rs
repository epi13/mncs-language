//! Fabric P-014: variable-length bounded-text classification over
//! caller-supplied bytes with differing runtime lengths. One entrypoint
//! decides all seven architecture aliases (plus case variants and
//! unknowns) through total bounded scans with ASCII case folding — no heap
//! String, only borrowed byte views plus explicit lengths.

use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The classifier answers every alias, case variant, and unknown on all
/// five executable backends. Overall UNKNOWN comes only from honest
/// checked-arithmetic obligations; every behavioral case must be met.
#[test]
fn arch_classifier_answers_all_aliases_on_every_executable_backend() {
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let output = binary()
            .env("MNCS_LIBRARY_PATH", library_dir())
            .args([
                "experiment",
                "run",
                &example("source/arch/classify.mncs"),
                "--backend",
                backend,
                "--corpus",
                &example("execution/arch-classify-corpus.json"),
            ])
            .output()
            .expect("run arch-classify experiment");
        assert!(
            output.status.success(),
            "{backend}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
        let cases = result["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 35, "{backend}: corpus drift: {cases:?}");
        for case in cases {
            assert_eq!(case["status"], "returned", "{backend}: {case}");
            assert_eq!(case["expectation_met"], true, "{backend}: {case}");
        }
    }
}

/// Host-fed classification: mixed-case bytes cross the `host_read`
/// capability boundary from a granted file and classify by runtime length,
/// with the realized effect recorded. Without the grant the call fails
/// closed and produces no classification.
#[test]
fn host_fed_bytes_classify_by_runtime_length() {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "experiment",
            "run",
            &example("source/arch/host.mncs"),
            "--backend",
            "mncs-research-bytecode",
            "--corpus",
            &example("execution/arch-classify-host-corpus.json"),
            "--grant-read",
            &format!(
                "arch_reader={}/../../examples/execution/arch-blob.txt",
                env!("CARGO_MANIFEST_DIR")
            ),
        ])
        .output()
        .expect("run host-fed arch-classify experiment");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
    let cases = result["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0]["status"], "returned");
    assert_eq!(cases[0]["expectation_met"], true);
    assert_eq!(cases[0]["returned"][0]["integer"]["value"], 2);
    assert_eq!(cases[0]["effects_met"], true);
}

/// The folded and exact view matchers agree across the layered reference
/// executors, including the new equality entrypoints.
#[test]
fn text_equality_agrees_across_layers() {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "check-backend-execution",
            &format!("{}/std/text_scan.mncs", library_dir()),
            &example("execution/text-scan-corpus.json"),
        ])
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
