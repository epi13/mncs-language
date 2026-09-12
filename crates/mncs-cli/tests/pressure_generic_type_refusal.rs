//! Numerics P-004: generic records and enums are refused precisely.
//!
//! `record Box<N: Nat>` and `enum Outcome<N: Nat>` used to die in a
//! four-diagnostic cascade (`MNP123`/`MNP072` plus `MNP127`/`MNP074`,
//! `MNP128`/`MNP075`, `MNP006`, `MNP007`) that named everything except
//! the real rule. The parser now reuses the shared generic-parameter
//! machinery, refuses once at the `<` (`MNP213` for records, `MNP214`
//! for enums), and skips the balanced body so a following declaration
//! still parses. The generic-types feature itself stays open language
//! design; these tests pin the diagnostic half.

use std::process::Command;

use serde_json::Value;

fn study_diagnostics(source_text: &str) -> Vec<Value> {
    let dir = std::env::temp_dir().join(format!(
        "mncs-generic-type-{}-{:?}",
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

/// A generic record is one `MNP213` naming the rule; the following
/// function still parses (no cascade, no `MNP006`/`MNP007`).
#[test]
fn generic_record_names_the_refusal_once() {
    let diagnostics = study_diagnostics(
        "mncs 0.16;\n\nmodule box.probe;\n\nrecord Box<N: Nat> {\n    lane: [f64; N],\n}\n\nfn demo() -> (result: i64) {\n    return 0;\n}\n",
    );
    assert_eq!(
        diagnostics.len(),
        1,
        "expected a single diagnostic; got {diagnostics:#?}"
    );
    assert_eq!(diagnostics[0]["code"], "MNP213");
    let message = diagnostics[0]["message"].as_str().unwrap_or("");
    assert!(
        message.contains("not supported") && message.contains("type parameters"),
        "MNP213 names the rule; got {message:?}"
    );
}

/// A generic enum is one `MNP214` naming the rule; the following
/// function still parses (no cascade, no `MNP006`/`MNP007`).
#[test]
fn generic_enum_names_the_refusal_once() {
    let diagnostics = study_diagnostics(
        "mncs 0.16;\n\nmodule out.probe;\n\nenum Outcome<N: Nat> {\n    Value { value: [f64; N] },\n    Empty,\n}\n\nfn demo() -> (result: i64) {\n    return 1;\n}\n",
    );
    assert_eq!(
        diagnostics.len(),
        1,
        "expected a single diagnostic; got {diagnostics:#?}"
    );
    assert_eq!(diagnostics[0]["code"], "MNP214");
    let message = diagnostics[0]["message"].as_str().unwrap_or("");
    assert!(
        message.contains("not supported") && message.contains("type parameters"),
        "MNP214 names the rule; got {message:?}"
    );
}
