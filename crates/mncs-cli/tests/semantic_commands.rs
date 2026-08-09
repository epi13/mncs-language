use std::process::Command;

use serde_json::Value;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn semantic_commands_emit_deterministic_machine_json() {
    let manifest = example("account-transfer.mncs.json");
    let canonical = binary()
        .args(["canonicalize", &manifest])
        .output()
        .expect("run canonicalize");
    assert!(canonical.status.success());
    let canonical_json: Value = serde_json::from_slice(&canonical.stdout).expect("canonical JSON");
    assert_eq!(canonical_json["schema_version"], "0.2");
    assert_eq!(canonical_json["fingerprint"].as_str().unwrap().len(), 64);

    let graph = binary()
        .args(["graph", &manifest])
        .output()
        .expect("run graph");
    assert!(graph.status.success());
    let graph_json: Value = serde_json::from_slice(&graph.stdout).expect("graph JSON");
    assert!(graph_json["nodes"].as_array().unwrap().len() >= 7);
    assert!(graph_json["edges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|edge| edge["kind"] == "supports_property"));
}

#[test]
fn diff_reports_changed_identity_and_invalidated_evidence() {
    let before = example("semantic-foundation/before.mncs.json");
    let after = example("semantic-foundation/after.mncs.json");
    let output = binary()
        .args(["diff", &before, &after])
        .output()
        .expect("run diff");
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).expect("diff JSON");
    assert!(!report["semantic"]["changed"].as_array().unwrap().is_empty());
    assert_eq!(
        report["invalidation"]["invalidated_evidence"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn evidence_check_marks_saved_manifest_stale_after_change() {
    let before = example("semantic-foundation/before.mncs.json");
    let after = example("semantic-foundation/after.mncs.json");
    let manifest = binary()
        .args(["evidence-manifest", &before])
        .output()
        .expect("run evidence manifest");
    assert!(manifest.status.success());
    let path = std::env::temp_dir().join(format!("mncs-evidence-{}.json", std::process::id()));
    std::fs::write(&path, manifest.stdout).expect("write temporary evidence manifest");
    let path_string = path.to_string_lossy().into_owned();
    let check = binary()
        .args(["evidence-check", &path_string, &after])
        .output()
        .expect("run evidence check");
    let _ = std::fs::remove_file(path);
    assert!(check.status.success());
    let report: Value = serde_json::from_slice(&check.stdout).expect("evidence report JSON");
    assert_eq!(report[0]["state"], "stale");
    assert!(!report[0]["invalidated_by"].as_array().unwrap().is_empty());
}

#[test]
fn invalid_semantic_input_keeps_rejection_exit_code() {
    let invalid = example("invalid-undeclared-effect.mncs.json");
    let output = binary()
        .args(["canonicalize", &invalid])
        .output()
        .expect("run invalid canonicalize");
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).expect("validation JSON");
    assert_eq!(report["valid"], false);
}

#[test]
fn ir_obligations_verifier_and_compare_commands_are_traceable() {
    let manifest = example("account-transfer.mncs.json");
    let ir = binary().args(["ir", &manifest]).output().expect("run IR");
    assert!(ir.status.success());
    let ir_json: Value = serde_json::from_slice(&ir.stdout).expect("IR JSON");
    assert_eq!(ir_json["schema_version"], "0.3");
    assert!(!ir_json["functions"][0]["blocks"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!ir_json["trace"]["entries"].as_array().unwrap().is_empty());

    let obligations = binary()
        .args(["obligations", &manifest])
        .output()
        .expect("run obligations");
    assert!(obligations.status.success());
    let obligation_json: Value =
        serde_json::from_slice(&obligations.stdout).expect("obligations JSON");
    assert!(obligation_json["obligations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["status"] == "PASS"));

    let verified = binary()
        .args(["verify", &manifest])
        .output()
        .expect("run verifier");
    assert!(verified.status.success());
    let verified_json: Value = serde_json::from_slice(&verified.stdout).expect("verifier JSON");
    assert!(verified_json
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["status"] == "PASS"));

    let before = example("semantic-foundation/before.mncs.json");
    let after = example("semantic-foundation/after.mncs.json");
    let compare = binary()
        .args(["compare", &before, &after])
        .output()
        .expect("run compare");
    assert!(compare.status.success());
    let compare_json: Value = serde_json::from_slice(&compare.stdout).expect("compare JSON");
    assert_eq!(compare_json["authority"]["authority_broadened"], false);
    assert!(!compare_json["semantic"]["changed_contracts"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn diagnose_exposes_failed_obligation_and_preserves_invalid_exit_status() {
    let invalid = example("invalid-undeclared-effect.mncs.json");
    let output = binary()
        .args(["diagnose", &invalid])
        .output()
        .expect("run diagnose");
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).expect("diagnostic JSON");
    assert_eq!(report["obligations"][0]["category"], "authority");
    assert!(report["obligations"][0]["obligation"].is_string());
    assert_eq!(report["causal_slices"].as_array().unwrap().len(), 1);
    assert_eq!(report["causal_slices"][0]["complete"], false);
}

#[test]
fn verify_does_not_treat_unknown_evidence_as_success() {
    let manifest = example("ir/unknown-contract-evidence.mncs.json");
    let output = binary()
        .args(["verify", &manifest])
        .output()
        .expect("run unresolved verify");
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).expect("verifier JSON");
    assert_eq!(report[0]["status"], "UNKNOWN");
}
