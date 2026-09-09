use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn source() -> String {
    example("source/host-read-scan.mncs")
}

fn corpus() -> String {
    example("execution/host-read-scan-corpus.json")
}

fn grant() -> String {
    example("execution/host-read-note.txt")
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

fn run_with(args: &[String]) -> (bool, Value) {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(args)
        .output()
        .expect("run experiment");
    let value: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
    (output.status.success(), value)
}

fn grant_args(capability: &str, path: &str) -> Vec<String> {
    vec!["--grant-read".to_owned(), format!("{capability}={path}")]
}

fn run_args(extra: &[String]) -> Vec<String> {
    let mut args = vec![
        "experiment".to_owned(),
        "run".to_owned(),
        source(),
        "--backend".to_owned(),
        "mncs-research-bytecode".to_owned(),
        "--corpus".to_owned(),
        corpus(),
    ];
    args.extend(extra.iter().cloned());
    args
}

/// Unauthorized `host_read()` shapes are rejected at elaboration: no
/// declaration, doubled declarations, and call-site arguments.
#[test]
fn host_read_authority_gaps_are_rejected() {
    let cases = [
        ("source/host-read-invalid-missing-decl.mncs", "MNE235"),
        ("source/host-read-invalid-double.mncs", "MNE236"),
        ("source/host-read-invalid-arity.mncs", "MNP193"),
    ];
    for (fixture, expected_code) in cases {
        let codes = diagnostics(&example(fixture));
        assert!(
            codes.contains(&expected_code.to_owned()),
            "{fixture}: expected {expected_code} in {codes:?}"
        );
    }
}

/// A real MNCS program obtains bounded bytes through the capability
/// boundary (HARNESS-PRESSURE-004): granted bytes flow into the program,
/// text scanning composes over them, and the realized effect is recorded
/// with kind/target/capability while unexpected effects are prohibited.
#[test]
fn granted_bytes_flow_and_effects_are_recorded() {
    let (success, result) = run_with(&run_args(&grant_args("note_reader", &grant())));
    assert!(success, "granted run exits success: {result}");
    for case in result["cases"].as_array().unwrap() {
        assert_eq!(case["status"], "returned", "{case}");
        assert_eq!(case["expectation_met"], true, "{case}");
        assert_eq!(case["effects_met"], true, "{case}");
    }
}

/// Without a grant the call fails closed: Unsupported, never a value, and
/// no effect is recorded.
#[test]
fn host_read_without_grant_fails_closed() {
    let (success, result) = run_with(&run_args(&[]));
    assert!(!success, "ungranted run must not succeed");
    for case in result["cases"].as_array().unwrap() {
        assert_eq!(case["status"], "unsupported", "{case}");
        // The corpus declares the granted expectation, so the ungranted
        // run reports it unmet (false): no value was produced, and the
        // failure reason names the missing realize policy/grant.
        assert_eq!(case["expectation_met"], false, "{case}");
    }
}

/// A grant for the wrong capability does not satisfy the declared
/// authority: InvalidRequest, never a value.
#[test]
fn wrong_capability_grant_is_rejected() {
    let (success, result) = run_with(&run_args(&grant_args("other_reader", &grant())));
    assert!(!success, "wrong-capability run must not succeed");
    for case in result["cases"].as_array().unwrap() {
        assert_eq!(case["status"], "invalid_request", "{case}");
    }
}

/// Grant files beyond the 64-byte bound are refused before any execution.
#[test]
fn oversize_grant_is_refused() {
    let big = std::env::temp_dir().join(format!("mncs-grant-big-{}", std::process::id()));
    std::fs::write(&big, vec![b'x'; 65]).expect("write oversize grant");
    let extra = grant_args("note_reader", &big.display().to_string());
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(run_args(&extra))
        .output()
        .expect("run experiment");
    std::fs::remove_file(&big).ok();
    assert_eq!(output.status.code(), Some(2), "oversize grant exits 2");
}

/// A missing grant file is refused before any execution.
#[test]
fn missing_grant_file_is_refused() {
    let extra = grant_args("note_reader", "/nonexistent/mncs-grant.txt");
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(run_args(&extra))
        .output()
        .expect("run experiment");
    assert_eq!(output.status.code(), Some(2), "missing grant exits 2");
}

/// The portable WASM backend cannot realize host calls: it refuses at
/// lowering and never manufactures PASS evidence.
#[test]
fn wasm_backend_refuses_host_calls() {
    let extra = grant_args("note_reader", &grant());
    let mut args = vec![
        "experiment".to_owned(),
        "run".to_owned(),
        source(),
        "--backend".to_owned(),
        "mncs-portable-wasm-mvp".to_owned(),
        "--corpus".to_owned(),
        corpus(),
    ];
    args.extend(extra);
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(args)
        .output()
        .expect("run wasm experiment");
    assert!(!output.status.success(), "wasm run must not succeed");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("host calls are unsupported"),
        "wasm refusal names host calls: {text:?}"
    );
}
