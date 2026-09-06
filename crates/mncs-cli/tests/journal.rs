//! Journal admission-gate conformance: the MNCS-native Journal decision core
//! stays valid and its evidence/trust/chain/render verdicts agree on every
//! executable backend.
//!
//! The corpus expectations encode the governing principle directly: models
//! may interpret (hints are observable), validators attest, policy decides.
//! `reject-model-only` and `model-never-admits` must keep passing — a model
//! opinion alone can never produce `Admit`.

use std::process::Command;

use serde_json::Value;

fn workspace(name: &str) -> String {
    format!("{}/../../{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn run_journal_corpus(backend: &str) -> Value {
    let output = binary()
        .env(
            "MNCS_LIBRARY_PATH",
            format!("{}/../../library/", env!("CARGO_MANIFEST_DIR")),
        )
        .args([
            "experiment",
            "run",
            &workspace("library/family/journal.mncs"),
            "--backend",
            backend,
            "--corpus",
            &workspace("examples/execution/journal-corpus.json"),
        ])
        .output()
        .expect("run journal corpus");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_eq!(output.status.code(), Some(0), "{backend}: exit; {stderr}");
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{backend}: journal JSON ({stderr}): {error}"))
}

#[test]
fn journal_module_validates() {
    let output = binary()
        .env(
            "MNCS_LIBRARY_PATH",
            format!("{}/../../library/", env!("CARGO_MANIFEST_DIR")),
        )
        .args(["validate", &workspace("library/family/journal.mncs")])
        .output()
        .expect("validate journal module");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let report: Value = serde_json::from_str(&stdout).expect("journal validation report is JSON");
    assert_eq!(report["valid"], true, "journal module invalid: {report:#}");
}

#[test]
fn journal_gates_pass_on_every_backend() {
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run_journal_corpus(backend);
        assert_eq!(result["status"], "PASS", "{backend}: overall status");
        let cases = result["cases"].as_array().expect("cases");
        assert_eq!(cases.len(), 35, "{backend}: case count");
        for case in cases {
            assert_eq!(
                case["expectation_met"],
                true,
                "{backend} {}: gate verdict mismatch",
                case["case_id"].as_str().unwrap_or("?")
            );
        }
    }
}

#[test]
fn journal_model_opinion_never_admits() {
    // Targeted pin: even the most journal-worthy narrator opinion evaluates
    // to `false` for admission authority, on the reference backend.
    let result = run_journal_corpus("mncs-research-bytecode");
    let cases = result["cases"].as_array().expect("cases");
    for wanted in [
        "model-never-admits",
        "model-narrator-no-admit",
        "reject-model-only",
    ] {
        let found = cases
            .iter()
            .find(|case| case["case_id"].as_str().unwrap_or("?") == wanted)
            .unwrap_or_else(|| panic!("missing case {wanted}"));
        assert_eq!(
            found["expectation_met"], true,
            "{wanted}: model-authority boundary moved"
        );
    }
}
