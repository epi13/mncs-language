//! CP-0013: `next` is a contextual field/member name, not a global keyword.
//!
//! `next` opens the iteration-step clause (`next state = expr;`) and names
//! carried state only through that step grammar, so it stays reserved
//! exactly there. In field positions — record declarations and literals,
//! finite payload declarations and constructions, `.` projections, match
//! payload bindings — it is an ordinary member name. The corpus pins
//! construction, projection, payload construction and binding, an
//! unmodified step clause, and both coexisting in one body on every
//! executable backend. Negative cases pin the reservation that remains:
//! a `let` binding or carried state still may not be named `next`, a
//! missing step clause is still diagnosed, and a missing `:` after a
//! `next` field is still diagnosed.

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

#[test]
fn next_as_field_name_agrees_per_backend() {
    let source = example("source/pressure-next-field.mncs");
    let corpus = example("execution/pressure-next-field-corpus.json");
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
        assert_eq!(cases.len(), 6, "{backend}: case count");
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

/// The reservation that remains: `next` still cannot bind a value (`let`,
/// carried state) and malformed step/field syntax still diagnoses precisely.
#[test]
fn next_reservation_edges_stay_closed() {
    let cases = [
        (
            "let-binding",
            "mncs 0.10;\nmodule test.nextfield.neg_let;\nfn probe() -> (result: u64) {\n    let next: u64 = 0;\n    return next;\n}\n",
            "MNP050",
        ),
        (
            "carried-state",
            "mncs 0.10;\nmodule test.nextfield.neg_state;\nfn probe() -> (result: u64) {\n    iterate i up_to 4 carrying next: u64 = 0 {\n        next next = next + 1;\n    }\n    return next;\n}\n",
            "MNP096",
        ),
        (
            "missing-step",
            "mncs 0.10;\nmodule test.nextfield.neg_step;\nfn probe() -> (result: u64) {\n    iterate i up_to 4 carrying acc: u64 = 0 {\n        let z: u64 = acc;\n    }\n    return acc;\n}\n",
            "MNP101",
        ),
        (
            "missing-colon",
            "mncs 0.10;\nmodule test.nextfield.neg_colon;\nrecord R { next u64 }\nfn probe(v: R) -> (result: u64) {\n    return v.next;\n}\n",
            "MNP125",
        ),
    ];
    for (name, text, code) in cases {
        let dir = std::env::temp_dir().join(format!("mncs-pressure-next-{name}"));
        std::fs::create_dir_all(&dir).expect("create workspace");
        let path = dir.join(format!("{name}.mncs"));
        std::fs::write(&path, text).expect("write case");
        let output = binary()
            .args(["source-study", &path.to_string_lossy()])
            .env("MNCS_LIBRARY_PATH", library(""))
            .output()
            .expect("run source-study");
        let result: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
        let errors: Vec<String> = result["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter(|d| d["severity"] == "error")
            .filter_map(|d| d["code"].as_str().map(str::to_owned))
            .collect();
        assert!(
            errors.iter().any(|candidate| candidate == code),
            "{name}: expected {code} in {errors:?}"
        );
    }
}
