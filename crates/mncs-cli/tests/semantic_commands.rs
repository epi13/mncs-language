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
