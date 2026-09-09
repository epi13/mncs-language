//! Fabric pressure cheap fixes: P-001 mixed-width diagnostics, P-002 ABI
//! mismatch diagnostics, P-003 corpus lint, P-013 caller-judged empty
//! expectations. Each test pins the repaired behavior against the current
//! tree so the language cannot regress without an external reproducer.

use std::process::Command;

use serde_json::Value;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn workspace(name: &str) -> std::path::PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "mncs-fabric-pressure-{}-{}-{:?}-{}",
        name,
        std::process::id(),
        std::thread::current().id(),
        unique
    ));
    std::fs::create_dir_all(&dir).expect("create workspace");
    dir
}

fn study_diagnostics(source_text: &str) -> Vec<Value> {
    let dir = workspace("study");
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

fn run_experiment(source: &str, corpus: &Value) -> (bool, Value) {
    let dir = workspace("experiment");
    let program = dir.join("probe.mncs");
    let corpus_path = dir.join("corpus.json");
    std::fs::write(&program, source).expect("write program");
    std::fs::write(
        &corpus_path,
        serde_json::to_string(corpus).expect("corpus JSON"),
    )
    .expect("write corpus");
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "experiment",
            "run",
            &program.to_string_lossy(),
            "--backend",
            "mncs-research-bytecode",
            "--corpus",
            &corpus_path.to_string_lossy(),
        ])
        .output()
        .expect("run experiment");
    let value: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
    (output.status.success(), value)
}

fn lint_corpus(source: &str, corpus: &Value) -> (bool, Value) {
    let dir = workspace("lint");
    let program = dir.join("probe.mncs");
    let corpus_path = dir.join("corpus.json");
    std::fs::write(&program, source).expect("write program");
    std::fs::write(
        &corpus_path,
        serde_json::to_string(corpus).expect("corpus JSON"),
    )
    .expect("write corpus");
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "corpus",
            "lint",
            &program.to_string_lossy(),
            &corpus_path.to_string_lossy(),
        ])
        .output()
        .expect("run corpus lint");
    let value: Value = serde_json::from_slice(&output.stdout).expect("lint JSON");
    (output.status.success(), value)
}

const RECORD_PROBE: &str = "mncs 0.6;\nmodule probe.p002;\nrecord R { x: i64 }\nfn f(env: R) -> (result: i64) {\n    return env.x;\n}\n";

fn record_argument(type_identity: &str) -> Value {
    serde_json::json!({
        "record": {
            "type_identity": type_identity,
            "name": "R",
            "fields": [["x", {"integer": {"value": 1, "type": {"bits": 64, "signed": true}}}]]
        }
    })
}

fn record_request(type_identity: &str) -> Value {
    serde_json::json!({
        "schema_version": "0.1",
        "target": {"module": "probe.p002", "function": "f"},
        "arguments": [record_argument(type_identity)],
        "step_budget": 8192
    })
}

const CANONICAL_IDENTITY: &str = "mncs:0.2:record-type:probe.p002::R::x%3Ai64%3B";
const TYPO_IDENTITY: &str = "mncs:0.2:record-type:probe.p002:R:x%3Ai64%3B";

/// P-001: `i32 <= i64` stays refused (explicit numeric semantics preserved)
/// but the diagnostic names both widths and suggests the `as` conversion.
#[test]
fn p001_mixed_width_diagnostic_names_types_and_suggests_as() {
    let diagnostics = study_diagnostics(
        "mncs 0.6;\nmodule probe.p001;\nrecord R { small_i32: i32, big_i64: i64 }\nfn c(env: R) -> (result: bool) {\n    return env.small_i32 <= env.big_i64;\n}\n",
    );
    let refusal = diagnostics
        .iter()
        .find(|d| d["code"] == "MNE119")
        .expect("MNE119 must still refuse mixed-width comparison");
    let message = refusal["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("i32") && message.contains("i64"),
        "MNE119 must name both operand types, got: {message}"
    );
    assert!(
        message.contains("as"),
        "MNE119 must suggest the explicit conversion, got: {message}"
    );
}

/// P-001 negative: same-width comparisons still elaborate, and the suggested
/// explicit conversion resolves the refusal.
#[test]
fn p001_same_width_and_explicit_conversion_still_elaborate() {
    let clean = study_diagnostics(
        "mncs 0.6;\nmodule probe.p001ok;\nrecord R { a: i64, b: i64 }\nfn c(env: R) -> (result: bool) {\n    return env.a <= env.b;\n}\n",
    );
    let codes: Vec<_> = clean.iter().filter(|d| d["severity"] == "error").collect();
    assert!(
        codes.is_empty(),
        "same-width comparison regressed: {codes:?}"
    );
    let converted = study_diagnostics(
        "mncs 0.7;\nmodule probe.p001conv;\nrecord R { small_i32: i32, big_i64: i64 }\nfn c(env: R) -> (result: bool) {\n    return (env.small_i32 as i64) <= env.big_i64;\n}\n",
    );
    let codes: Vec<_> = converted
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert!(
        codes.is_empty(),
        "explicit `as` conversion must resolve MNE119, got: {codes:?}"
    );
}

/// P-002: a `::` identity typo surfaces expected vs received identities,
/// the function name, and the argument index without dumping artifacts.
#[test]
fn p002_abi_mismatch_reports_expected_and_received_identities() {
    let corpus = serde_json::json!({
        "schema_version": "0.1",
        "name": "p002",
        "cases": [{
            "id": "bad-identity",
            "request": record_request(TYPO_IDENTITY),
            "expected": [{"integer": {"value": 1, "type": {"bits": 64, "signed": true}}}]
        }]
    });
    let (success, result) = run_experiment(RECORD_PROBE, &corpus);
    assert!(!success, "typo'd identity must still fail: {result}");
    let reason = result["cases"][0]["failure_reason"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        reason.contains(CANONICAL_IDENTITY),
        "diagnostic must carry the expected identity, got: {reason}"
    );
    assert!(
        reason.contains(TYPO_IDENTITY),
        "diagnostic must carry the received identity, got: {reason}"
    );
    assert!(
        reason.contains("probe.p002::f") || reason.contains("probe.p002"),
        "diagnostic must name the entrypoint, got: {reason}"
    );
    assert!(
        reason.contains("index 0"),
        "diagnostic must name the argument position, got: {reason}"
    );
}

/// P-003: `mncs corpus lint` catches the typo'd identity before execution
/// and passes a well-formed corpus.
#[test]
fn p003_corpus_lint_catches_typo_before_execution() {
    let bad = serde_json::json!({
        "schema_version": "0.1",
        "name": "p002",
        "cases": [{
            "id": "bad-identity",
            "request": record_request(TYPO_IDENTITY),
            "expected": [{"integer": {"value": 1, "type": {"bits": 64, "signed": true}}}]
        }]
    });
    let (success, report) = lint_corpus(RECORD_PROBE, &bad);
    assert!(!success, "lint must fail the typo'd corpus: {report}");
    let errors = report["cases"][0]["errors"].to_string();
    assert!(
        errors.contains(CANONICAL_IDENTITY) && errors.contains(TYPO_IDENTITY),
        "lint must pinpoint both identities, got: {errors}"
    );
    let good = serde_json::json!({
        "schema_version": "0.1",
        "name": "p002",
        "cases": [{
            "id": "good",
            "request": record_request(CANONICAL_IDENTITY),
            "expected": [{"integer": {"value": 1, "type": {"bits": 64, "signed": true}}}]
        }]
    });
    let (success, report) = lint_corpus(RECORD_PROBE, &good);
    assert!(success, "lint must pass the well-formed corpus: {report}");
    assert_eq!(report["cases"][0]["ok"], true);
}

/// P-013: missing and empty `expected` are both caller-judged; a genuinely
/// wrong expectation still fails.
#[test]
fn p013_empty_expected_is_caller_judged() {
    let corpus = serde_json::json!({
        "schema_version": "0.1",
        "name": "p013",
        "cases": [
            {"id": "empty-expected", "request": record_request(CANONICAL_IDENTITY), "expected": []},
            {"id": "omitted-expected", "request": record_request(CANONICAL_IDENTITY)}
        ]
    });
    let (success, result) = run_experiment(RECORD_PROBE, &corpus);
    assert!(success, "caller-judged cases must pass: {result}");
    for case in result["cases"].as_array().unwrap() {
        assert_eq!(case["status"], "returned", "{case}");
        assert!(
            case.get("expectation_met").is_none(),
            "caller-judged cases carry no judgment: {case}"
        );
    }
    let wrong = serde_json::json!({
        "schema_version": "0.1",
        "name": "p013neg",
        "cases": [{
            "id": "wrong",
            "request": record_request(CANONICAL_IDENTITY),
            "expected": [{"integer": {"value": 2, "type": {"bits": 64, "signed": true}}}]
        }]
    });
    let (success, result) = run_experiment(RECORD_PROBE, &wrong);
    assert!(!success, "wrong expectations must still fail: {result}");
    assert_eq!(result["cases"][0]["expectation_met"], false);
}
