//! WEB-P-005: integer literals adapt symmetrically in binary arithmetic.
//!
//! A literal on the left adapts to the right operand's concrete type
//! exactly as a right-side literal adapts to the left's, so `1000 +% x`
//! means what `x +% 1000` means on every executable backend. Genuinely
//! mixed non-literal widths stay refused (MNE119), and an out-of-range
//! literal for a byte operand stays refused.

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
        "mncs-literal-symmetry-{}-{:?}",
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

/// Left-literal and right-literal forms return identical values on every
/// backend, including the wrapping/saturating spellings from the pressure.
#[test]
fn literal_operands_adapt_symmetrically_on_every_backend() {
    let source = example("source/pressure-literal-symmetry.mncs");
    let corpus = example("execution/pressure-literal-symmetry-corpus.json");
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
        assert_eq!(cases.len(), 10, "{backend}: case count");
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

/// Genuinely mixed non-literal widths stay refused, and an out-of-range
/// literal for a byte operand stays refused: adaptation never invents a
/// width the literal cannot inhabit.
#[test]
fn non_adaptable_mismatches_stay_closed() {
    let mixed = study_diagnostics(
        "mncs 0.13;\nmodule test.lit.mixed;\nfn probe(a: i32, b: u64) -> (result: u64) {\n    return 1 + a + b;\n}\n",
    );
    assert!(
        mixed.iter().any(|d| d["code"] == "MNE119"),
        "variable mixed widths must stay MNE119, got: {mixed:?}"
    );
    let narrow = study_diagnostics(
        "mncs 0.13;\nmodule test.lit.narrow;\nfn probe(b: byte) -> (result: byte) {\n    return 300 & b;\n}\n",
    );
    assert!(
        narrow
            .iter()
            .any(|d| d["code"] == "MNE118" || d["code"] == "MNE119"),
        "out-of-range byte literal must stay refused, got: {narrow:?}"
    );
}
