//! ENG-PRESSURE-0020: `[value; N]` repeat literals.
//!
//! The RFC-0016 multi-element slices (`[a, b, c]`, RFC 0016) only covered
//! spelled-out elements. Repeat literals desugar in elaboration to the same
//! `SequenceConstruct` the equivalent N-element literal produces: the
//! element elaborates once under the declared element type and the resolved
//! operand is duplicated N times, so evaluation happens a single time. A
//! repeat count that disagrees with the expected exact bound fails closed
//! (`MNE184`); a non-length count is rejected (`MNE256`). The corpus pins
//! repeat reads, repeat arithmetic, and repeat/literal equivalence on every
//! executable backend. A bare-identifier count (`[x; N]`, numerics P-003)
//! parses without cascade and fails once with a `MNE256` that names the
//! rule (counts must be Nat literals).

use std::process::Command;

use serde_json::Value;

fn study_diagnostics(source_text: &str) -> Vec<Value> {
    let dir = std::env::temp_dir().join(format!(
        "mncs-repeat-symbolic-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("create workspace");
    let path = dir.join("probe.mncs");
    std::fs::write(&path, source_text).expect("write case");
    let output = Command::new(env!("CARGO_BIN_EXE_mncs"))
        .args(["source-study", &path.to_string_lossy()])
        .output()
        .expect("run source-study");
    let result: Value = serde_json::from_slice(&output.stdout).expect("front-end JSON");
    result["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

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

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

#[test]
fn repeat_literals_match_equivalent_literals_per_backend() {
    let source = example("source/pressure-repeat.mncs");
    let corpus = example("execution/pressure-repeat-corpus.json");
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
        assert_eq!(cases.len(), 3, "{backend}: case count");
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

/// A symbolic repeat count names the rule exactly once: no `MNP203`
/// parse refusal, no `MNP016`/`MNP017`/`MNP006`/`MNP007` cascade — one
/// `MNE256` at the identifier spelling out that counts must be Nat
/// literals (numerics P-003 diagnostic half).
#[test]
fn symbolic_repeat_count_names_the_literal_rule_once() {
    let diagnostics = study_diagnostics(
        "mncs 0.16;\n\nmodule fill.probe;\n\nfn demo(x: f64) -> (result: [f64; 4]) {\n    return [x; N];\n}\n",
    );
    assert_eq!(
        diagnostics.len(),
        1,
        "expected a single diagnostic; got {diagnostics:#?}"
    );
    assert_eq!(diagnostics[0]["code"], "MNE256");
    let message = diagnostics[0]["message"].as_str().unwrap_or("");
    assert!(
        message.contains("symbolic repeat count") && message.contains("must be Nat literals"),
        "MNE256 names the rule; got {message:?}"
    );
}
