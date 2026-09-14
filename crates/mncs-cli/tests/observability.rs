use std::process::Command;

use serde_json::Value;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn observe(capture: &str, extra: &[&str]) -> Value {
    let mut command = binary();
    command.args([
        "observe",
        &example("source/observability-nested.mncs"),
        &example("execution/observability-nested-request.json"),
        "--capture",
        capture,
    ]);
    command.args(extra);
    let output = command.output().expect("run observe");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("observe JSON")
}

#[test]
fn native_observation_keeps_semantics_separate_from_capture_identity() {
    let none = observe("none", &[]);
    let failure_only = observe("failure-only", &[]);
    let bounded = observe("bounded", &["--max-events", "512", "--max-values", "128"]);
    let diagnostic = observe(
        "diagnostic",
        &["--max-events", "512", "--max-values", "128"],
    );

    assert_eq!(
        bounded["schema_version"],
        "mncs.execution-observation-command/1"
    );
    assert_eq!(bounded["execution"]["status"], "returned");
    assert_eq!(bounded["execution"]["returned"][0]["integer"]["value"], 42);
    assert_eq!(none["execution"], bounded["execution"]);
    assert_eq!(bounded["execution"], diagnostic["execution"]);
    assert_eq!(
        bounded["observation"]["execution_identity"],
        diagnostic["observation"]["execution_identity"]
    );
    assert_ne!(
        bounded["observation"]["identity"],
        diagnostic["observation"]["identity"]
    );
    assert_eq!(none["observation"]["completeness"]["status"], "disabled");
    assert_eq!(
        failure_only["observation"]["completeness"]["status"],
        "not_captured"
    );
    assert!(none["observation"]["events"].as_array().unwrap().is_empty());
    assert!(none["observation"]["values"].as_array().unwrap().is_empty());
}

#[test]
fn compiler_source_map_and_runtime_stream_join_nested_calls_without_inference() {
    let observed = observe("bounded", &["--max-events", "512", "--max-values", "128"]);
    let source_map = observed["source_map"].as_object().expect("source map");
    assert_eq!(source_map["schema_version"], "mncs.execution-source-map/1");
    assert!(!source_map["identity"].as_str().unwrap().is_empty());

    let operations = source_map["operations"].as_array().unwrap();
    assert!(!operations.is_empty());
    assert!(operations.iter().all(|operation| {
        operation["synthetic"] == false && operation["source_span"].is_object()
    }));
    let operation_ids = operations
        .iter()
        .map(|operation| operation["identity"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();

    let observation = observed["observation"].as_object().unwrap();
    assert_eq!(
        observation["schema_version"],
        "mncs.execution-observation/1"
    );
    assert_eq!(observation["completeness"]["status"], "complete");
    let frames = observation["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 2);
    let root = frames
        .iter()
        .find(|frame| frame["parent"].is_null())
        .unwrap();
    let child = frames
        .iter()
        .find(|frame| !frame["parent"].is_null())
        .unwrap();
    assert_eq!(child["parent"], root["identity"]);
    assert!(child["call_operation"].is_string());
    assert_eq!(child["depth"], 1);

    let events = observation["events"].as_array().unwrap();
    let operation_event = events
        .iter()
        .find(|event| event["kind"] == "operation_result")
        .expect("native operation result");
    assert_eq!(operation_event["status"], "completed");
    let operation_id = operation_event["operation"].as_str().unwrap();
    assert!(operation_ids.contains(operation_id));
    let mapped = operations
        .iter()
        .find(|operation| operation["identity"] == operation_event["operation"])
        .unwrap();
    assert_eq!(
        mapped["correspondence"],
        "semantic-operation-to-source-span"
    );
    assert!(mapped["source_span"]["line"].as_u64().unwrap() >= 1);

    let values = observation["values"].as_array().unwrap();
    assert!(values.iter().all(|value| value["type_name"].is_string()));
    assert!(values.iter().any(|value| value["origin"].is_string()));
    assert!(events.iter().any(|event| {
        event["inputs"]
            .as_array()
            .is_some_and(|inputs| !inputs.is_empty())
            && event["outputs"]
                .as_array()
                .is_some_and(|outputs| !outputs.is_empty())
    }));
}

#[test]
fn native_observation_reports_explicit_bounds_and_effect_lineage() {
    let bounded = observe("bounded", &["--max-events", "2", "--max-values", "1"]);
    assert_eq!(
        bounded["observation"]["completeness"]["status"],
        "truncated"
    );
    assert!(
        bounded["observation"]["completeness"]["dropped_events"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        bounded["observation"]["completeness"]["dropped_values"]
            .as_u64()
            .unwrap()
            > 0
    );

    let mut command = binary();
    let output = command
        .args([
            "observe",
            &example("executable/effectful-record.mncs.json"),
            &example("execution/effect-record-request.json"),
            "--capture",
            "bounded",
        ])
        .output()
        .expect("run effect observation");
    assert!(output.status.success());
    let effect: Value = serde_json::from_slice(&output.stdout).expect("effect observation JSON");
    let effects = effect["observation"]["effects"].as_array().unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0]["replayability"], "lineage_only");
    assert_eq!(effects[0]["status"], "realized_or_recorded");
    assert!(effect["observation"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["kind"] == "effect_invoke"));
    assert!(effect["observation"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["kind"] == "effect_result"));
}
