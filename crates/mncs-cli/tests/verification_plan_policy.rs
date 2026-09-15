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
    let change_class = [
        "implementation",
        "public_contract",
        "shared_type",
        "parser_semantics",
        "serialization_format",
        "effect_semantics",
        "abi_boundary",
        "canonical_fixture",
        "language_profile",
        "cross_repository_contract",
    ][usize::try_from(args[4]).expect("change class code")];
    let boolean = |value: i64| json!({"boolean": {"value": value == 1}});
    let arguments = vec![json!({
        "record": {
            "type": "SelectionInput",
            "fields": {
                "cross_repository": boolean(args[0]),
                "impact_complete": boolean(args[1]),
                "unknown_root": boolean(args[2]),
                "truncated": boolean(args[3]),
                "change_class": {"finite": {"type": "ChangeClass", "variant": change_class}},
                "high_connectivity": boolean(args[5]),
                "shared_type": boolean(args[6]),
                "effect_semantics": boolean(args[7]),
                "abi_boundary": boolean(args[8]),
                "public_contract": boolean(args[9]),
                "direct_dependents": boolean(args[10]),
                "selected_tests": {"integer": {"value": args[11]}},
            }
        }
    })];
    std::fs::write(
        &request,
        serde_json::to_vec(&json!({
            "schema_version": "0.1",
            "target": {
                "module": "mncs.family.verification_plan.v1",
                "function": "choose"
            },
            "typed_arguments": arguments,
            "step_budget": 4096
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
        "policy execution failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
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
        .map(|pair| &pair[1])
        .expect("policy field")
}

fn finite_variant(value: &Value, field_name: &str) -> String {
    field(value, field_name)["finite"]["variant_identity"]
        .as_str()
        .expect("finite variant identity")
        .rsplit("::")
        .next()
        .expect("finite variant name")
        .to_owned()
}

fn reasons(value: &Value) -> Vec<String> {
    field(value, "reasons")["sequence"]["values"]
        .as_array()
        .expect("reasons sequence")
        .iter()
        .map(|reason| {
            reason["finite"]["variant_identity"]
                .as_str()
                .expect("reason variant identity")
                .rsplit("::")
                .next()
                .expect("reason variant name")
                .to_owned()
        })
        .filter(|reason| reason != "none")
        .collect()
}

fn sufficient(value: &Value) -> bool {
    field(value, "sufficient_local")["boolean"]["value"]
        .as_bool()
        .expect("sufficiency")
}

#[test]
fn native_policy_keeps_direct_dependents_narrow() {
    let result = run([0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1]);
    assert_eq!(result["status"], "returned");
    assert_eq!(finite_variant(&result, "level"), "direct_dependents");
    assert_eq!(reasons(&result), vec!["direct_dependents_affected"]);
    assert!(sufficient(&result));
}

#[test]
fn native_policy_promotes_public_contracts_to_repository_proof() {
    let result = run([0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1]);
    assert_eq!(result["status"], "returned");
    assert_eq!(finite_variant(&result, "level"), "repository_canonical");
    assert_eq!(reasons(&result), vec!["public_contract_changed"]);
    assert!(!sufficient(&result));
}

#[test]
fn native_policy_routes_cross_repository_changes_to_family() {
    let result = run([0, 1, 0, 0, 9, 0, 0, 0, 0, 0, 0, 1]);
    assert_eq!(result["status"], "returned");
    assert_eq!(finite_variant(&result, "level"), "family");
    assert_eq!(reasons(&result), vec!["cross_repository_contract_changed"]);
    assert!(!sufficient(&result));
}
