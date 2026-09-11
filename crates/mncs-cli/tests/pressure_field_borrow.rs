//! WEB-P-002: exact-array record fields borrow as bounded views.
//!
//! A projected `[byte; N]` field borrows as `[byte; up_to M]` (N <= M)
//! exactly like a named value does — same rule, same borrow machinery,
//! no copy. Both the direct field and the chained (`outer.inner.data`)
//! projection agree on every executable backend, while over-capacity
//! borrows and element mismatches stay refused.

use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library(name: &str) -> String {
    format!("{}/../../library/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn run_experiment(source: &str, backend: &str, corpus: &str) -> (Option<i32>, Value, String) {
    let output = binary()
        .args([
            "experiment",
            "run",
            source,
            "--backend",
            backend,
            "--corpus",
            corpus,
        ])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run experiment");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let value: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("experiment JSON ({stderr}): {error}"));
    (output.status.code(), value, stderr)
}

fn study_diagnostics(source_text: &str) -> Vec<Value> {
    let dir = std::env::temp_dir().join(format!(
        "mncs-field-borrow-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("create workspace");
    let path = dir.join("probe.mncs");
    std::fs::write(&path, source_text).expect("write case");
    let output = binary()
        .args(["source-study", &path.to_string_lossy()])
        .output()
        .expect("run source-study");
    let result: Value = serde_json::from_slice(&output.stdout).expect("front-end JSON");
    result["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

/// Field and chained borrows return the full-range view length on every
/// backend, identical to the direct borrow.
#[test]
fn projected_fields_borrow_as_views_on_every_backend() {
    let source = example("source/pressure-field-borrow.mncs");
    let corpus = example("execution/pressure-field-borrow-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_eq!(
            code,
            Some(0),
            "{backend}: unexpected exit; stderr={stderr}; result={result:#}"
        );
        let cases = result["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
        assert_eq!(cases.len(), 2, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            assert_eq!(
                case["status"], "returned",
                "{backend} {id}: status {case:#}"
            );
            assert_eq!(
                case["expectation_met"], true,
                "{backend} {id}: logical value mismatch; returned={:#}",
                case["returned"]
            );
        }
    }
}

/// Over-capacity borrows (N > M) and element mismatches stay refused:
/// the borrow rule widens positions, not bounds.
#[test]
fn unborrowable_projections_stay_closed() {
    let over_capacity = study_diagnostics(
        "mncs 0.13;\nmodule test.borrow.overcap;\nrecord Big { data: [byte; 16], len: u64 }\nfn take_view(buf: [byte; up_to 8], len: u64) -> (result: u64) {\n    return buf.len;\n}\nfn probe(wrap: Big) -> (result: u64) {\n    return take_view(wrap.data, wrap.len);\n}\n",
    );
    assert!(
        over_capacity
            .iter()
            .any(|d| d["code"] == "MNE163" && d["severity"] == "error"),
        "N > M field borrow must stay MNE163, got: {over_capacity:?}"
    );
    let element_mismatch = study_diagnostics(
        "mncs 0.13;\nmodule test.borrow.elem;\nrecord Numbers { data: [u64; 8], len: u64 }\nfn take_view(buf: [byte; up_to 1024], len: u64) -> (result: u64) {\n    return buf.len;\n}\nfn mismatch(wrap: Numbers) -> (result: u64) {\n    return take_view(wrap.data, wrap.len);\n}\n",
    );
    assert!(
        element_mismatch
            .iter()
            .any(|d| d["code"] == "MNE163" && d["severity"] == "error"),
        "element-mismatched field borrow must stay MNE163, got: {element_mismatch:?}"
    );
}
