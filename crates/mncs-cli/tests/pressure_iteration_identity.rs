//! CP-0009 (iteration identities): sequential loops may reuse a source
//! name; nested overlap may not.
//!
//! Iteration identities are unique over their live lexical scope. Two
//! sequential non-overlapping loops (traversal or counted) may share a
//! source-level index name; the recorded identity is hygienic (`i`, then
//! `i#2`) so proof-graph nodes, obligation subjects, and body validation
//! stay per-loop. A nested loop reusing a still-open identity stays
//! rejected (`MNE146`), and sequential carried-state reuse stays governed
//! by the rebinding guard (`MNE110`), which this tranche deliberately
//! leaves intact.

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

fn study(path: &std::path::Path) -> Value {
    let output = binary()
        .args(["source-study", &path.to_string_lossy()])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run source-study");
    serde_json::from_slice(&output.stdout).expect("study JSON")
}

fn error_codes(result: &Value) -> Vec<String> {
    result["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|d| d["severity"] == "error")
        .filter_map(|d| d["code"].as_str().map(str::to_owned))
        .collect()
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

#[test]
fn sequential_identity_reuse_agrees_per_backend() {
    let source = example("source/pressure-iteration-reuse.mncs");
    let corpus = example("execution/pressure-iteration-reuse-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_eq!(
            code,
            Some(0),
            "{backend}: unexpected exit; stderr={stderr}; result={result:#}"
        );
        let overall = result["status"].as_str().unwrap_or("");
        assert!(
            overall == "PASS" || overall == "UNKNOWN",
            "{backend}: overall status {overall} is not PASS or UNKNOWN"
        );
        let cases = result["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
        assert_eq!(cases.len(), 4, "{backend}: case count");
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

/// Elaboration is deterministic: the hygienic `name#2` mapping follows
/// source order, so two studies of the reuse corpus agree fingerprint-wise.
#[test]
fn reuse_elaboration_is_deterministic() {
    let source = example("source/pressure-iteration-reuse.mncs");
    let first = study(std::path::Path::new(&source));
    let second = study(std::path::Path::new(&source));
    assert!(
        error_codes(&first).is_empty(),
        "reuse corpus must elaborate cleanly: {:?}",
        error_codes(&first)
    );
    assert_eq!(
        first["semantic_fingerprint"], second["semantic_fingerprint"],
        "semantic fingerprints differ between identical runs"
    );
    assert_eq!(
        first["ssa_fingerprint"], second["ssa_fingerprint"],
        "ssa fingerprints differ between identical runs"
    );
}

/// The narrowed rule keeps its teeth: nested overlap is still `MNE146`,
/// and sequential carried-state reuse is still the rebinding guard's
/// `MNE110` (deliberately out of scope for this tranche).
#[test]
fn overlap_and_rebinding_edges_stay_closed() {
    let cases = [
        (
            "nested-same",
            "mncs 0.13;\nmodule test.iter.neg_nested;\nfn probe(a: [u64; 2], b: [u64; 2]) -> (result: u64) {\n    iterate i over a carrying x: u64 = 0 {\n        iterate i over b carrying y: u64 = x {\n            next y = y + 1;\n        }\n        next x = y;\n    }\n    return x;\n}\n",
            "MNE146",
        ),
        (
            "nested-counted-same",
            "mncs 0.13;\nmodule test.iter.neg_nested_counted;\nfn probe() -> (result: u64) {\n    iterate k up_to 2 carrying a: u64 = 0 {\n        iterate k up_to 2 carrying b: u64 = a {\n            next b = b + 1;\n        }\n        next a = b;\n    }\n    return a;\n}\n",
            "MNE146",
        ),
        (
            "state-reuse",
            "mncs 0.10;\nmodule test.iter.neg_state;\nfn probe() -> (result: u64) {\n    iterate i up_to 2 carrying x: u64 = 0 {\n        next x = x + 1;\n    }\n    iterate j up_to 2 carrying x: u64 = x {\n        next x = x + 1;\n    }\n    return x;\n}\n",
            "MNE110",
        ),
    ];
    for (name, text, code) in cases {
        let dir = std::env::temp_dir().join(format!("mncs-pressure-iter-{name}"));
        std::fs::create_dir_all(&dir).expect("create workspace");
        let path = dir.join(format!("{name}.mncs"));
        std::fs::write(&path, text).expect("write case");
        let errors = error_codes(&study(&path));
        assert!(
            errors.iter().any(|candidate| candidate == code),
            "{name}: expected {code} in {errors:?}"
        );
    }
}
