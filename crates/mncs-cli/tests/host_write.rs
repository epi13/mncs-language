//! P-006 storage slice: bounded append-only `host_write(view)` behind an
//! explicit capability. Granted bytes land byte-exact in order with the
//! realized effect recorded; ungranted calls fail closed; non-realizing
//! backends refuse at lowering instead of faking support.

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
    example("source/host-write-append.mncs")
}

fn corpus() -> String {
    example("execution/host-write-append-corpus.json")
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

/// Unauthorized `host_write()` shapes are rejected at elaboration: no
/// declaration, doubled declarations, and call-site arity.
#[test]
fn host_write_authority_gaps_are_rejected() {
    let cases = [
        ("source/host-write-invalid-missing-decl.mncs", "MNE253"),
        ("source/host-write-invalid-double.mncs", "MNE254"),
        ("source/host-write-invalid-arity.mncs", "MNP202"),
    ];
    for (fixture, expected_code) in cases {
        let codes = diagnostics(&example(fixture));
        assert!(
            codes.contains(&expected_code.to_owned()),
            "{fixture}: expected {expected_code} in {codes:?}"
        );
    }
}

/// Granted appends land byte-exact and in corpus order, each case reports
/// its appended count, and every realized effect is recorded with
/// kind/target/capability while unexpected effects are prohibited.
///
/// `experiment run` replays realized effects once per validation layer
/// (body, SSA, backend), so the ledger holds each case's bytes once per
/// layer; the test pins the trailing single-execution order instead of
/// the absolute file length. Frozen `experiment execute` runs each case
/// exactly once (see the evidence record).
#[test]
fn granted_appends_land_in_order_with_effects_recorded() {
    let dir = std::env::temp_dir().join(format!(
        "mncs-host-write-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("create workspace");
    let ledger = dir.join("ledger.bin");
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "experiment",
            "run",
            &source(),
            "--backend",
            "mncs-research-bytecode",
            "--corpus",
            &corpus(),
            "--grant-write",
            &format!("ledger_writer={}", ledger.to_string_lossy()),
        ])
        .output()
        .expect("run experiment");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
    let counts: Vec<u64> = result["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            assert_eq!(case["status"], "returned", "{case}");
            assert_eq!(case["expectation_met"], true, "{case}");
            assert_eq!(case["effects_met"], true, "{case}");
            case["returned"][0]["integer"]["value"]
                .as_u64()
                .expect("u64 count")
        })
        .collect();
    assert_eq!(counts, vec![2, 2, 4]);
    let bytes = std::fs::read(&ledger).expect("read ledger");
    assert!(
        bytes.len() >= 8,
        "layered replay must retain at least one full pass: {bytes:?}"
    );
    assert_eq!(
        &bytes[bytes.len() - 8..],
        b"abcdefgh",
        "appends land in corpus order: {bytes:?}"
    );
}

/// Without a grant the call fails closed: Unsupported, never a value, no
/// file is created, and no effect is recorded.
#[test]
fn host_write_without_grant_fails_closed() {
    let dir = std::env::temp_dir().join(format!(
        "mncs-host-write-denied-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("create workspace");
    let ledger = dir.join("ledger.bin");
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "experiment",
            "run",
            &source(),
            "--backend",
            "mncs-research-bytecode",
            "--corpus",
            &corpus(),
        ])
        .output()
        .expect("run experiment");
    assert!(!output.status.success(), "ungranted run must not succeed");
    let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
    for case in result["cases"].as_array().unwrap() {
        assert_eq!(case["status"], "unsupported", "{case}");
        let effects = case
            .get("effects")
            .and_then(|effects| effects.as_array())
            .map(Vec::len)
            .unwrap_or(0);
        assert_eq!(effects, 0, "{case}");
    }
    assert!(
        !ledger.exists(),
        "fail-closed writes must not create the destination"
    );
}

/// Non-realizing backends refuse host calls explicitly at lowering: no
/// cases run and no destination file is created.
#[test]
fn unrealizing_backends_refuse_host_write_explicitly() {
    let dir = std::env::temp_dir().join(format!(
        "mncs-host-write-refused-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("create workspace");
    let ledger = dir.join("ledger.bin");
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "experiment",
            "run",
            &source(),
            "--backend",
            "mncs-portable-wasm-mvp",
            "--corpus",
            &corpus(),
            "--grant-write",
            &format!("ledger_writer={}", ledger.to_string_lossy()),
        ])
        .output()
        .expect("run experiment");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        stdout.contains("host calls are unsupported"),
        "refusal must be explicit: {stdout:.500}"
    );
    assert!(
        !ledger.exists(),
        "refused backends must not touch the destination"
    );
}
