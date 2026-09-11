//! WEB-P-009: checked indices with discharge.
//!
//! `checked_index(sequence, index)` traps unless the candidate sits below
//! the sequence's runtime length and carries the candidate unchanged.
//! Projecting through exactly that value discharges the projection's
//! bounds obligation (`CheckedBound`); raw candidates and cross-sequence
//! uses keep an explicit runtime check. Values agree on every executable
//! backend, escapes fail deterministically everywhere, and malformed
//! shapes refuse with their own diagnostics.

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

fn study(source_text: &str) -> Value {
    let dir = std::env::temp_dir().join(format!(
        "mncs-checked-index-{}-{:?}",
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
    serde_json::from_slice(&output.stdout).expect("front-end JSON")
}

fn study_diagnostics(source_text: &str) -> Vec<Value> {
    study(source_text)["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn study_unresolved_obligations(source_text: &str) -> Vec<String> {
    study(source_text)["unresolved_obligations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|obligation| obligation.as_str().map(str::to_owned))
        .collect()
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

/// Value cases agree on every backend: exact-bound reads (including the
/// last valid index), aliasing, view spans, cross-sequence reads, literal
/// candidates, and wider elements. Escaping the checked length fails
/// deterministically everywhere.
#[test]
fn checked_index_values_agree_on_every_backend() {
    let source = example("source/pressure-checked-index.mncs");
    let corpus = example("execution/pressure-checked-index-corpus.json");
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
        assert_eq!(cases.len(), 12, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            if id.ends_with("_escape") {
                assert_eq!(
                    case["status"], "runtime_failure",
                    "{backend} {id}: escaping the checked length must fail deterministically; case={case:#}"
                );
            } else {
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
}

/// Discharge is observable in the study: the checked projection leaves no
/// unresolved obligation, while the raw projection and the
/// cross-sequence use each retain `sequence-index-bounds`.
#[test]
fn checked_index_discharge_is_observable() {
    let checked = "mncs 0.14;\nmodule test.chk.dis;\nfn probe(buf: [byte; 8], i: u64) -> (result: byte) {\n    let c: u64 = checked_index(buf, i);\n    return buf[c];\n}\n";
    assert_eq!(
        study_unresolved_obligations(checked),
        Vec::<String>::new(),
        "checked projection must discharge its bounds obligation"
    );
    let raw = "mncs 0.14;\nmodule test.chk.raw;\nfn probe(buf: [byte; 8], i: u64) -> (result: byte) {\n    return buf[i];\n}\n";
    assert!(
        study_unresolved_obligations(raw)
            .iter()
            .any(|obligation| obligation.contains("sequence-index-bounds")),
        "raw projection must retain its bounds obligation"
    );
    let crossed = "mncs 0.14;\nmodule test.chk.cross;\nfn probe(a: [byte; 8], b: [byte; 8], i: u64) -> (result: byte) {\n    let c: u64 = checked_index(a, i);\n    return b[c];\n}\n";
    assert!(
        study_unresolved_obligations(crossed)
            .iter()
            .any(|obligation| obligation.contains("sequence-index-bounds")),
        "a check against one sequence must not discharge another"
    );
}

/// Elaboration refuses each malformed shape with its own diagnostic:
/// pre-0.14 profile, wrong arity, non-sequence subject, non-u64
/// candidate, provably out-of-range literal candidate, and a checked
/// index used where another type is required.
#[test]
fn checked_index_malformed_shapes_stay_closed() {
    for (name, probe, code) in [
        (
            "profile",
            "mncs 0.13;\nmodule test.chk.profile;\nfn probe(buf: [byte; 8], i: u64) -> (result: byte) {\n    let c: u64 = checked_index(buf, i);\n    return buf[c];\n}\n",
            "MNP209",
        ),
        (
            "arity",
            "mncs 0.14;\nmodule test.chk.arity;\nfn probe(buf: [byte; 8], i: u64) -> (result: byte) {\n    let c: u64 = checked_index(buf, i, buf);\n    return buf[c];\n}\n",
            "MNP210",
        ),
        (
            "subject-type",
            "mncs 0.14;\nmodule test.chk.subject;\nfn probe(n: u64, i: u64) -> (result: u64) {\n    let c: u64 = checked_index(n, i);\n    return c;\n}\n",
            "MNE273",
        ),
        (
            "candidate-type",
            "mncs 0.14;\nmodule test.chk.candidate;\nfn probe(buf: [byte; 8], flag: bool) -> (result: byte) {\n    let c: u64 = checked_index(buf, flag);\n    return buf[c];\n}\n",
            "MNE274",
        ),
        (
            "static-oob",
            "mncs 0.14;\nmodule test.chk.oob;\nfn probe(buf: [byte; 8]) -> (result: byte) {\n    let c: u64 = checked_index(buf, 8);\n    return buf[c];\n}\n",
            "MNE275",
        ),
        (
            "expected-type",
            "mncs 0.14;\nmodule test.chk.expected;\nfn probe(buf: [byte; 8], i: u64) -> (result: byte) {\n    let c: byte = checked_index(buf, i);\n    return buf[0];\n}\n",
            "MNE276",
        ),
    ] {
        let diagnostics = study_diagnostics(probe);
        assert!(
            diagnostics.iter().any(|d| d["code"] == code),
            "{name}: expected {code}, got: {diagnostics:?}"
        );
    }
}
