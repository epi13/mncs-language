use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

/// Bounded task/cancellation seed (HARNESS-PRESSURE-007): the lifecycle
/// skeleton runs start/step/finish and start/step/cancel/terminate
/// through both executable backends with byte-exact task records, and the
/// invalid-transition witness reports all-rejected.
#[test]
fn task_lifecycle_executes_on_both_backends() {
    let source = format!("{}/std/task.mncs", library_dir());
    let corpus = format!(
        "{}/../../examples/execution/task-corpus.json",
        env!("CARGO_MANIFEST_DIR")
    );
    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let output = binary()
            .env("MNCS_LIBRARY_PATH", library_dir())
            .args(["experiment", "run", &source, "--backend", backend, "--corpus", &corpus])
            .output()
            .expect("run task experiment");
        assert!(
            output.status.success(),
            "{backend}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
        // Overall UNKNOWN comes only from honest iteration-cost
        // obligations; every lifecycle case must still be met.
        for case in result["cases"].as_array().unwrap() {
            assert_eq!(case["status"], "returned", "{backend}: {case}");
            assert_eq!(case["expectation_met"], true, "{backend}: {case}");
        }
    }
}

/// The task lifecycle agrees across the layered reference executors.
#[test]
fn task_lifecycle_agrees_across_layers() {
    let source = format!("{}/std/task.mncs", library_dir());
    let corpus = format!(
        "{}/../../examples/execution/task-corpus.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["check-backend-execution", &source, &corpus])
        .output()
        .expect("run layered task check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("layered JSON");
    assert_eq!(result["status"], "consistent_over_corpus");
    assert_eq!(result["mismatching_cases"], 0);
}
