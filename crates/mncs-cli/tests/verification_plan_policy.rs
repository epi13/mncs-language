//! The selective-development policy is executable MNCS, not a host-language
//! copy. These cases pin the bounded level/reason decisions used by Ravel.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{json, Value};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

static REQUEST_ID: AtomicUsize = AtomicUsize::new(0);

fn policy_source() -> String {
    format!(
        "{}/../../library/family/verification_plan.mncs",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn run(args: [i64; 12]) -> Value {
    let root = std::env::temp_dir().join(format!(
        "mncs-verification-plan-policy-{}-{}",
        std::process::id(),
        REQUEST_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create request directory");
    let request = root.join("request.json");
    let arguments = args
        .into_iter()
        .map(|value| json!({"integer": {"value": value, "type": {"bits": 32, "signed": true}}}))
        .collect::<Vec<_>>();
    std::fs::write(
        &request,
        serde_json::to_vec(&json!({
            "schema_version": "0.1",
            "target": {
                "module": "mncs.family.verification_plan.v1",
                "function": "select_codes"
            },
            "arguments": arguments,
            "step_budget": 512
        }))
        .expect("encode request"),
    )
    .expect("write request");
    let source = policy_source();
    let request_value = request.to_string_lossy().into_owned();
    let output = binary()
        .args(["execute", &source, &request_value])
        .output()
        .expect("execute native policy");
    assert!(
        output.status.success(),
        "policy execution failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("policy JSON")
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value["returned"][0]["record"]["fields"]
        .as_array()
        .expect("returned record fields")
        .iter()
        .find(|pair| pair[0] == name)
        .map(|pair| &pair[1]["integer"]["value"])
        .expect("policy field")
}

#[test]
fn native_policy_keeps_direct_dependents_narrow() {
    let result = run([0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1]);
    assert_eq!(result["status"], "returned");
    assert_eq!(field(&result, "level_code"), 1);
    assert_eq!(field(&result, "reason_mask"), 8192);
}

#[test]
fn native_policy_promotes_public_contracts_to_repository_proof() {
    let result = run([0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1]);
    assert_eq!(result["status"], "returned");
    assert_eq!(field(&result, "level_code"), 3);
    assert_eq!(field(&result, "reason_mask"), 16);
}

#[test]
fn native_policy_routes_cross_repository_changes_to_family() {
    let result = run([0, 1, 0, 0, 9, 0, 0, 0, 0, 0, 0, 1]);
    assert_eq!(result["status"], "returned");
    assert_eq!(field(&result, "level_code"), 4);
    assert_eq!(field(&result, "reason_mask"), 1);
}
