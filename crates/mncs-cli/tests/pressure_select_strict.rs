//! ENG-PRESSURE-0022: `select` evaluates both arms.
//!
//! Strictness is semantic: `select(true, 7, boom(0))` must trap with the
//! discarded arm's failure on every backend rather than returning 7. A
//! backend that silently went lazy would return the kept arm and break the
//! total-arms-guards idiom the engine relies on. The corpus pins the trap
//! in both arm positions plus a non-trapping control on every executable
//! backend.

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
fn select_discarded_arm_trap_is_observed_per_backend() {
    let source = example("source/pressure-select-strict.mncs");
    let corpus = example("execution/pressure-select-strict-corpus.json");
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
            let want_status = match id {
                "control" => "returned",
                _ => "runtime_failure",
            };
            assert_eq!(
                case["status"], want_status,
                "{backend} {id}: status {case:#}"
            );
            if id == "control" {
                assert_eq!(
                    case["expectation_met"], true,
                    "{backend} {id}: logical value mismatch; returned={:#}",
                    case["returned"]
                );
            }
        }
    }
}
