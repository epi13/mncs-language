use std::{
    collections::BTreeSet,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use mncs_compiler::{bundle::pinned_bundle, ReferenceCompiler};
use mncs_embed::{Artifact, CallOptions, Grant, Session};
use mncs_model::{ArtifactRepresentation, ExecutionValue};
use mncs_syntax::{SourceArtifactKind, SourceEnvelope};
use serde_json::{json, Value};

const SOURCE: &str = r#"
mncs 0.18;
module app.process_lifecycle_embed;
use mncs.std.process.v1;

fn start_owned(request: ProcessRequest) -> (result: ProcessStartResult)
    capability process_capability
    effect process_run authorized_by process_capability
{
    return start(request);
}

fn inspect_owned(handle: ProcessHandle) -> (result: ProcessObservation)
    capability process_capability
    effect process_run authorized_by process_capability
{
    return observe(handle);
}

fn cancel_owned(handle: ProcessHandle) -> (result: ProcessObservation)
    capability process_capability
    effect process_run authorized_by process_capability
{
    return cancel(handle);
}

fn reap_owned(handle: ProcessHandle) -> (result: ProcessObservation)
    capability process_capability
    effect process_run authorized_by process_capability
{
    return reap(handle);
}
"#;

fn open_session() -> Session {
    let compiler = ReferenceCompiler::default();
    let envelope = SourceEnvelope::inline(
        SourceArtifactKind::Program,
        "process-lifecycle-embed",
        SOURCE,
    );
    let front_end = compiler.front_end_with_resolver(
        envelope,
        &pinned_bundle()
            .expect("standard-library bundle verifies")
            .resolver(),
    );
    assert!(
        front_end.is_valid(),
        "front end: {:?}",
        front_end.diagnostics
    );
    let program = front_end.program.expect("linked process program");
    let emit = [
        ArtifactRepresentation::Semantic,
        ArtifactRepresentation::Hir,
        ArtifactRepresentation::Ssa,
        ArtifactRepresentation::TargetLoweringPlan,
        ArtifactRepresentation::BackendArtifact,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let request = compiler
        .request_for_program_with_backend(&program, emit, "mncs-research-bytecode")
        .expect("research bytecode target");
    let compilation = compiler.compile(request, &program);
    assert!(
        matches!(
            compilation.status,
            mncs_model::CompilationStatus::Completed
                | mncs_model::CompilationStatus::CompletedWithUnresolvedObligations
        ),
        "compile status: {:?}",
        compilation.status
    );
    let artifact = compilation
        .emissions
        .backend
        .expect("backend artifact emitted");
    let artifact = Artifact::from_json(&serde_json::to_vec(&artifact).expect("artifact JSON"))
        .expect("artifact verifies");
    Session::open(artifact).expect("retained session opens")
}

fn byte_sequence(bytes: &[u8]) -> Value {
    json!({"sequence":{"values":bytes.iter().map(|byte| json!({"byte":{"value":byte}})).collect::<Vec<_>>()}})
}

fn integer(value: u64) -> Value {
    json!({"integer":{"value":value}})
}

fn process_request(program: &str, argv: &[&str], environment: &[(&str, &str)]) -> String {
    let resources = json!({
        "record": {
            "type": "ProcessResourceEnvelope",
            "fields": {
                "memory_high_bytes": integer(128 * 1024 * 1024),
                "memory_max_bytes": integer(256 * 1024 * 1024),
                "swap_max_bytes": integer(0),
                "has_swap_max": {"boolean":{"value":true}},
                "process_max": integer(32)
            }
        }
    });
    serde_json::to_string(&json!([{
        "record": {
            "type": "ProcessRequest",
            "fields": {
                "program": byte_sequence(program.as_bytes()),
                "argv": {"sequence":{"values":argv.iter().map(|arg| byte_sequence(arg.as_bytes())).collect::<Vec<_>>() }},
                "argv_count": integer(argv.len() as u64),
                "current_dir": byte_sequence(b""),
                "environment": {"sequence":{"values":environment.iter().map(|(key,value)| json!({"record":{"type":"EnvironmentEntry","fields":{"key":byte_sequence(key.as_bytes()),"value":byte_sequence(value.as_bytes())}}})).collect::<Vec<_>>() }},
                "environment_count": integer(environment.len() as u64),
                "resources": resources,
                "stdin": byte_sequence(b""),
                "stdout_limit": integer(1024),
                "stderr_limit": integer(1024),
                "deadline_ms": integer(10000)
            }
        }
    }]))
    .expect("request JSON")
}

fn process_grant(program: &str) -> CallOptions {
    let mut options = CallOptions::budgeted(8_192);
    options.grants.push(Grant {
        capability: "process_capability".to_owned(),
        locator: program.to_owned(),
        bytes: Vec::new(),
    });
    options
}

fn process_handle(start_result: &ExecutionValue) -> ExecutionValue {
    let ExecutionValue::Record { fields, .. } = start_result else {
        panic!("ProcessStartResult expected, got {start_result:?}");
    };
    assert!(matches!(
        field(fields, "has_handle"),
        Some(ExecutionValue::Boolean { value: true })
    ));
    match field(fields, "handle") {
        Some(handle @ ExecutionValue::Record { .. }) => handle.clone(),
        other => panic!("provider-issued process handle expected, got {other:?}"),
    }
}

fn field<'a>(fields: &'a [(String, ExecutionValue)], name: &str) -> Option<&'a ExecutionValue> {
    fields
        .iter()
        .find_map(|(field_name, value)| (field_name == name).then_some(value))
}

fn call_observation(
    session: &Session,
    function: &str,
    handle: ExecutionValue,
    options: &CallOptions,
) -> ExecutionValue {
    let output = session.call(
        "app.process_lifecycle_embed",
        function,
        vec![handle],
        options,
    );
    assert_eq!(
        output.status, "returned",
        "{function}: {:?}",
        output.failure_reason
    );
    assert_eq!(output.returned.len(), 1);
    output.returned.into_iter().next().expect("one result")
}

fn observation_status(observation: &ExecutionValue) -> i128 {
    let ExecutionValue::Record { fields, .. } = observation else {
        panic!("ProcessObservation expected, got {observation:?}");
    };
    let Some(ExecutionValue::Finite { discriminant, .. }) = field(fields, "status") else {
        panic!("typed ProcessStatus expected");
    };
    i128::from(*discriminant)
}

fn observation_bool(observation: &ExecutionValue, name: &str) -> bool {
    let ExecutionValue::Record { fields, .. } = observation else {
        panic!("ProcessObservation expected, got {observation:?}");
    };
    match field(fields, name) {
        Some(ExecutionValue::Boolean { value }) => *value,
        other => panic!("boolean field {name} expected, got {other:?}"),
    }
}

fn observation_unsigned(observation: &ExecutionValue, name: &str) -> Option<u64> {
    let ExecutionValue::Record { fields, .. } = observation else {
        panic!("ProcessObservation expected, got {observation:?}");
    };
    match field(fields, name) {
        Some(ExecutionValue::Integer { value, ty }) if !ty.signed && *value >= 0 => {
            u64::try_from(*value).ok()
        }
        _ => None,
    }
}

#[test]
fn retained_mncs_process_capability_starts_cancels_and_reaps_owned_work() {
    let session = open_session();
    let options = process_grant("/usr/bin/sleep");
    let started_at = Instant::now();
    let started = session
        .call_typed_json(
            "app.process_lifecycle_embed",
            "start_owned",
            &process_request("/usr/bin/sleep", &["30"], &[]),
            &options,
        )
        .expect("typed process request");
    assert_eq!(started.status, "returned", "{:?}", started.failure_reason);
    let handle = process_handle(&started.returned[0]);
    let first = call_observation(&session, "inspect_owned", handle.clone(), &options);
    assert_eq!(observation_status(&first), 0, "running status");
    let unrelated_session = open_session();
    let foreign = call_observation(
        &unrelated_session,
        "inspect_owned",
        handle.clone(),
        &options,
    );
    assert_eq!(
        observation_status(&foreign),
        7,
        "handle is session-scoped UNKNOWN"
    );
    assert!(!observation_bool(&foreign, "has_cleanup_result"));

    let requested_at = Instant::now();
    let cancel_result = call_observation(&session, "cancel_owned", handle.clone(), &options);
    assert!(
        observation_bool(&cancel_result, "cancellation_requested"),
        "cancel is an explicit request"
    );
    let cancel_request_ms = requested_at.elapsed().as_secs_f64() * 1000.0;
    let reaped = call_observation(&session, "reap_owned", handle, &options);
    let cancel_to_reap_ms = requested_at.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(observation_status(&reaped), 2, "cancelled status");
    assert!(observation_bool(&reaped, "cancellation_complete"));
    assert!(observation_bool(&reaped, "launcher_reaped"));
    assert!(observation_bool(&reaped, "cleanup_complete"));
    assert!(
        !observation_bool(&reaped, "success"),
        "cancelled work cannot be PASS"
    );
    eprintln!(
        "generic process lifecycle: start={} ms, cancel-request={} ms, cancel-to-reap={} ms, total={} ms",
        started_at.elapsed().as_secs_f64() * 1000.0,
        cancel_request_ms,
        cancel_to_reap_ms,
        started_at.elapsed().as_secs_f64() * 1000.0,
    );
}

#[test]
fn process_tree_parent_fixture() {
    if std::env::var_os("MNCS_OWNED_FIXTURE").is_none() {
        return;
    }
    let executable = std::env::current_exe().expect("test executable path");
    let _child = Command::new(executable)
        .args(["--exact", "process_tree_leaf_fixture", "--nocapture"])
        .env("MNCS_OWNED_FIXTURE", "1")
        .spawn()
        .expect("spawn process-tree leaf");
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn process_tree_leaf_fixture() {
    if std::env::var_os("MNCS_OWNED_FIXTURE").is_none() {
        return;
    }
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn cancellation_terminates_the_complete_owned_process_tree() {
    let session = open_session();
    let executable = std::env::current_exe()
        .expect("test executable path")
        .to_string_lossy()
        .into_owned();
    let options = process_grant(&executable);
    let started = session
        .call_typed_json(
            "app.process_lifecycle_embed",
            "start_owned",
            &process_request(
                &executable,
                &["--exact", "process_tree_parent_fixture", "--nocapture"],
                &[("MNCS_OWNED_FIXTURE", "1")],
            ),
            &options,
        )
        .expect("typed process-tree request");
    assert_eq!(started.status, "returned", "{:?}", started.failure_reason);
    let handle = process_handle(&started.returned[0]);
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut live = call_observation(&session, "inspect_owned", handle.clone(), &options);
    while observation_unsigned(&live, "process_peak").unwrap_or_default() < 2
        && Instant::now() < deadline
    {
        thread::sleep(Duration::from_millis(10));
        live = call_observation(&session, "inspect_owned", handle.clone(), &options);
    }
    assert!(
        observation_unsigned(&live, "process_peak").unwrap_or_default() >= 2,
        "the owned cgroup observed both parent and child: {live:?}"
    );
    let requested_at = Instant::now();
    call_observation(&session, "cancel_owned", handle.clone(), &options);
    let cancel_request_ms = requested_at.elapsed().as_secs_f64() * 1000.0;
    let reaped = call_observation(&session, "reap_owned", handle, &options);
    let cancel_to_reap_ms = requested_at.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(observation_status(&reaped), 2, "whole tree is cancelled");
    assert!(observation_bool(&reaped, "cleanup_complete"));
    assert!(observation_bool(&reaped, "tree_empty"));
    assert!(observation_bool(&reaped, "launcher_reaped"));
    assert!(!observation_bool(&reaped, "success"));
    eprintln!(
        "owned process tree cancellation: request={cancel_request_ms:.3} ms, request-to-tree-zero-and-reap={cancel_to_reap_ms:.3} ms"
    );
}
