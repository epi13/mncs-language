//! Stage C2 exact negation: `neg(x)` is IEEE-754 negation on every
//! executable backend. A case is met only when the backend returned the
//! expected logical value with the expected status, not merely when it
//! ran. The signed-zero cases are the point: `0.0 - x` rounds `+0.0`
//! to `+0.0`, while `neg(+0.0)` is `-0.0` bit-exactly. The NaN case
//! carries no `expected` value: it is met when the backend reports
//! `runtime_failure` (the float trap rule), pinning the input guard on
//! every lowering. The subnormal case pins bit-exactness where a
//! converting implementation would lose it.

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

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

/// Require every case to report the expected status, and every valued
/// case to match the expected logical value bit-for-bit.
fn assert_value_agreement(
    backend: &str,
    code: Option<i32>,
    result: &Value,
    stderr: &str,
    expected_len: usize,
) {
    assert_eq!(
        code,
        Some(0),
        "{backend}: unexpected exit; stderr={stderr}; result={result:#}"
    );
    let cases = result["cases"]
        .as_array()
        .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
    assert_eq!(
        cases.len(),
        expected_len,
        "{backend}: case count {} != {expected_len}",
        cases.len()
    );
    for case in cases {
        let id = case["case_id"].as_str().unwrap_or("?");
        // Trap cases carry no expected value; their contract is the
        // trap status itself.
        let trap = case
            .get("expected")
            .is_none_or(|expected| expected.is_null());
        let expected_status = if trap { "runtime_failure" } else { "returned" };
        assert_eq!(
            case["status"], expected_status,
            "{backend} {id}: status {:#}",
            case["status"]
        );
        if !case["status_met"].is_null() {
            assert_eq!(
                case["status_met"], true,
                "{backend} {id}: status_met is not true: {case:#}"
            );
        }
        if trap {
            continue;
        }
        assert_eq!(
            case["expectation_met"], true,
            "{backend} {id}: logical value mismatch; returned={:#}",
            case["returned"]
        );
    }
    // Opaque float operands carry honest float-finite obligations, so
    // the overall status may be UNKNOWN even when every case is met.
    let overall = result["status"].as_str().unwrap_or("");
    assert!(
        overall == "PASS" || overall == "UNKNOWN",
        "{backend}: overall status {overall} is not PASS or UNKNOWN"
    );
}

#[test]
fn neg_agrees_per_backend() {
    let source = example("source/stage-c2-neg.mncs");
    let corpus = example("execution/stage-c2-neg-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_value_agreement(backend, code, &result, &stderr, 6);
    }
}

fn diagnostics(path: &str) -> Vec<String> {
    let output = binary()
        .args(["source-study", path])
        .output()
        .expect("run source-study");
    let study: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
    study["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|diag| diag["code"].as_str().map(str::to_owned))
        .collect()
}

/// A wrong-arity `neg` is refused with its own diagnostic (MNP212).
#[test]
fn neg_misuse_is_rejected() {
    let dir = std::env::temp_dir().join(format!(
        "mncs-neg-misuse-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create neg misuse workspace");
    let fixture = dir.join("neg-invalid-arity.mncs");
    std::fs::write(
        &fixture,
        "mncs 0.12;\n\nmodule probe.neg_misuse;\n\nfn bad(x: f64, y: f64) -> (result: f64) {\n    return neg(x, y);\n}\n",
    )
    .expect("write neg misuse fixture");
    let codes = diagnostics(&fixture.to_string_lossy());
    assert!(
        codes.contains(&"MNP212".to_owned()),
        "neg-invalid-arity: expected MNP212 in {codes:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
