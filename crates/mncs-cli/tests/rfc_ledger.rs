//! RFC ledger conformance: the machine-readable ledger stays well-formed,
//! evidence-backed, and consistent with the MNCS-native status gates, and
//! the status corpus passes on every executable backend.

use std::process::Command;

use serde_json::Value;

fn workspace(name: &str) -> String {
    format!("{}/../../{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn ledger() -> Value {
    let text = std::fs::read_to_string(workspace("rfcs/conformance-ledger.json"))
        .expect("read conformance ledger");
    serde_json::from_str(&text).expect("parse conformance ledger")
}

fn is_gap_reference(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 7
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5].is_ascii_uppercase()
        && !bytes[6..].is_empty()
        && bytes[6..].iter().all(u8::is_ascii_digit)
}

#[test]
fn ledger_covers_every_rfc_with_valid_states() {
    let document = ledger();
    assert_eq!(document["schema_version"], "0.1");
    let design: Vec<String> = document["design_states"]
        .as_array()
        .expect("design states")
        .iter()
        .map(|state| state.as_str().expect("state").to_owned())
        .collect();
    let implementation: Vec<String> = document["implementation_states"]
        .as_array()
        .expect("implementation states")
        .iter()
        .map(|state| state.as_str().expect("state").to_owned())
        .collect();
    let entries = document["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 46, "ledger must cover RFC 0001 through 0046");
    for (position, entry) in entries.iter().enumerate() {
        let expected = format!("{:04}", position + 1);
        assert_eq!(
            entry["number"].as_str().unwrap_or("?"),
            expected,
            "ledger entries must stay in numeric order"
        );
        assert!(
            design.contains(&entry["design_status"].as_str().unwrap_or("?").to_owned()),
            "RFC {expected}: design status outside the vocabulary"
        );
        assert!(
            implementation.contains(
                &entry["implementation_status"]
                    .as_str()
                    .unwrap_or("?")
                    .to_owned()
            ),
            "RFC {expected}: implementation status outside the vocabulary"
        );
        let criteria = entry["acceptance_criteria"].as_array().expect("criteria");
        assert!(
            !criteria.is_empty(),
            "RFC {expected}: no acceptance criteria"
        );
        for criterion in criteria {
            let state = criterion["state"].as_str().unwrap_or("?");
            assert!(
                ["satisfied", "partial", "unsatisfied"].contains(&state),
                "RFC {expected}: criterion state outside the vocabulary"
            );
            if state == "unsatisfied" && criterion.get("excused").is_none() {
                assert!(
                    criterion["evidence"]
                        .as_array()
                        .map(|evidence| evidence.is_empty())
                        .unwrap_or(true),
                    "RFC {expected}: unsatisfied criterion must carry no evidence"
                );
            }
            for evidence in criterion["evidence"]
                .as_array()
                .cloned()
                .unwrap_or_default()
            {
                let path = evidence.as_str().unwrap_or("?");
                if is_gap_reference(path) {
                    continue;
                }
                assert!(
                    std::path::Path::new(&workspace(path)).exists(),
                    "RFC {expected}: evidence path missing: {path}"
                );
            }
        }
    }
}

#[test]
fn ledger_rfc0007_tally_matches_the_executable_gate() {
    let document = ledger();
    let entries = document["entries"].as_array().expect("entries");
    let rfc0007 = entries
        .iter()
        .find(|entry| entry["number"] == "0007")
        .expect("RFC 0007 entry");
    assert_eq!(rfc0007["design_status"], "DRAFT");
    assert_eq!(rfc0007["implementation_status"], "BOUNDED_IMPLEMENTATION");
    let criteria = rfc0007["acceptance_criteria"].as_array().expect("criteria");
    let count = |state: &str| {
        criteria
            .iter()
            .filter(|criterion| criterion["state"] == state)
            .count() as i64
    };
    let excused = criteria
        .iter()
        .filter(|criterion| criterion.get("excused") == Some(&Value::Bool(true)))
        .count() as i64;
    // These numbers are duplicated in mncs.family.rfc_status.v1
    // `rfc0007_tally`; this test fails if the ledger drifts from the gate.
    assert_eq!(criteria.len(), 20);
    assert_eq!(count("satisfied"), 18);
    assert_eq!(count("partial"), 1);
    assert_eq!(excused, 1);
}

fn run_status_corpus(backend: &str) -> Value {
    let output = binary()
        .env(
            "MNCS_LIBRARY_PATH",
            format!("{}/../../library/", env!("CARGO_MANIFEST_DIR")),
        )
        .args([
            "experiment",
            "run",
            &workspace("library/family/rfc_status.mncs"),
            "--backend",
            backend,
            "--corpus",
            &workspace("examples/execution/rfc-status-corpus.json"),
        ])
        .output()
        .expect("run status corpus");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_eq!(output.status.code(), Some(0), "{backend}: exit; {stderr}");
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{backend}: status JSON ({stderr}): {error}"))
}

#[test]
fn status_gates_pass_on_every_backend() {
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run_status_corpus(backend);
        assert_eq!(result["status"], "PASS", "{backend}: overall status");
        for case in result["cases"].as_array().expect("cases") {
            assert_eq!(
                case["expectation_met"],
                true,
                "{backend} {}: gate verdict mismatch",
                case["case_id"].as_str().unwrap_or("?")
            );
        }
    }
}
