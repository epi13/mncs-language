use std::{
    collections::BTreeSet,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use mncs_compiler::{bundle::pinned_bundle, ReferenceCompiler};
use mncs_embed::{Artifact, CallOptions, Grant, Session};
use mncs_model::{
    ArtifactRepresentation, ExecutionValue, MAX_SEQUENCE_BOUND, PROCESS_LAUNCH_VECTOR_CAPACITY,
};
use mncs_syntax::{SourceArtifactKind, SourceEnvelope};
use serde_json::{json, Value};

const SOURCE: &str = r#"
mncs 0.19;
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

fn byte_string(bytes: &[u8]) -> Value {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or_default();
        let third = chunk.get(2).copied().unwrap_or_default();
        encoded.push(ALPHABET[(first >> 2) as usize] as char);
        encoded.push(ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(ALPHABET[(third & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    json!({"byte_string":{"base64":encoded}})
}

fn integer(value: u64) -> Value {
    json!({"integer":{"value":value}})
}

fn process_request(program: &str, argv: &[&str], environment: &[(&str, &str)]) -> String {
    process_request_with_envelope(
        program,
        argv,
        environment,
        128 * 1024 * 1024,
        256 * 1024 * 1024,
        0,
        true,
        32,
        1024,
        1024,
    )
}

fn process_request_with_stdin(
    program: &str,
    argv: &[&str],
    environment: &[(&str, &str)],
    stdin: &[u8],
) -> String {
    let mut request: Value = serde_json::from_str(&process_request(program, argv, environment))
        .expect("process request JSON");
    let chunk_bytes = MAX_SEQUENCE_BOUND as usize;
    let initial_len = stdin.len().min(chunk_bytes);
    let chunks = stdin[initial_len..]
        .chunks(chunk_bytes)
        .map(byte_string)
        .collect::<Vec<_>>();
    request[0]["record"]["fields"]["stdin"] = byte_string(&stdin[..initial_len]);
    request[0]["record"]["fields"]["stdin_chunks"] = json!({"sequence":{"values":chunks}});
    request[0]["record"]["fields"]["stdin_chunk_count"] = integer(chunks.len() as u64);
    serde_json::to_string(&request).expect("process request JSON with stdin")
}

#[allow(clippy::too_many_arguments)]
fn process_request_with_envelope(
    program: &str,
    argv: &[&str],
    environment: &[(&str, &str)],
    memory_high_bytes: u64,
    memory_max_bytes: u64,
    swap_max_bytes: u64,
    has_swap_max: bool,
    process_max: u64,
    stdout_limit: u64,
    stderr_limit: u64,
) -> String {
    let resources = json!({
        "record": {
            "type": "ProcessResourceEnvelope",
            "fields": {
                "memory_high_bytes": integer(memory_high_bytes),
                "memory_max_bytes": integer(memory_max_bytes),
                "swap_max_bytes": integer(swap_max_bytes),
                "has_swap_max": {"boolean":{"value":has_swap_max}},
                "process_max": integer(process_max)
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
                "stdin_chunks": {"sequence":{"values":[]}},
                "stdin_chunk_count": integer(0),
                "stdout_limit": integer(stdout_limit),
                "stderr_limit": integer(stderr_limit),
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

fn observation_signed(observation: &ExecutionValue, name: &str) -> Option<i64> {
    let ExecutionValue::Record { fields, .. } = observation else {
        panic!("ProcessObservation expected, got {observation:?}");
    };
    match field(fields, name) {
        Some(ExecutionValue::Integer { value, ty }) if ty.signed => i64::try_from(*value).ok(),
        _ => None,
    }
}

fn observe_until_terminal(
    session: &Session,
    handle: &ExecutionValue,
    options: &CallOptions,
    count: usize,
) -> ExecutionValue {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let observed = call_observation(session, "inspect_owned", handle.clone(), options);
        if observation_status(&observed) != 0 {
            return observed;
        }
        assert!(Instant::now() < deadline, "{count} argv never left Running");
        thread::sleep(Duration::from_millis(10));
    }
}

fn observation_bytes_len(observation: &ExecutionValue, name: &str) -> usize {
    let ExecutionValue::Record { fields, .. } = observation else {
        panic!("ProcessObservation expected, got {observation:?}");
    };
    match field(fields, name) {
        Some(ExecutionValue::Sequence { values }) => values.len(),
        other => panic!("byte-sequence field {name} expected, got {other:?}"),
    }
}

fn observation_bytes(observation: &ExecutionValue, name: &str) -> Vec<u8> {
    let ExecutionValue::Record { fields, .. } = observation else {
        panic!("ProcessObservation expected, got {observation:?}");
    };
    let Some(ExecutionValue::Sequence { values }) = field(fields, name) else {
        panic!("byte-sequence field {name} expected");
    };
    values
        .iter()
        .map(|value| match value {
            ExecutionValue::Byte { value } => {
                u8::try_from(*value).unwrap_or_else(|_| panic!("byte in {name} is in range"))
            }
            other => panic!("byte value in {name} expected, got {other:?}"),
        })
        .collect()
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

#[cfg(target_os = "linux")]
#[test]
fn mncs_process_effect_reports_tiny_cgroup_memory_exhaustion() {
    let session = open_session();
    let options = process_grant("/usr/bin/python3");
    let program = "data = bytearray(64 * 1024 * 1024)\nfor offset in range(0, len(data), 4096):\n    data[offset] = 1\n";
    let started = session
        .call_typed_json(
            "app.process_lifecycle_embed",
            "start_owned",
            &process_request_with_envelope(
                "/usr/bin/python3",
                &["-c", program],
                &[],
                0,
                32 * 1024 * 1024,
                0,
                true,
                8,
                1024,
                1024,
            ),
            &options,
        )
        .expect("typed bounded-memory process request");
    assert_eq!(started.status, "returned", "{:?}", started.failure_reason);
    let handle = process_handle(&started.returned[0]);
    let completed = call_observation(&session, "reap_owned", handle, &options);

    eprintln!(
        "tiny cgroup exhaustion observation: status={}, memory_max_events={:?}, oom_events={:?}, oom_kill_events={:?}, memory_peak_bytes={:?}, observation_complete={}, cleanup_complete={}, tree_empty={}, launcher_reaped={}",
        observation_status(&completed),
        observation_unsigned(&completed, "memory_max_events"),
        observation_unsigned(&completed, "oom_events"),
        observation_unsigned(&completed, "oom_kill_events"),
        observation_unsigned(&completed, "memory_peak_bytes"),
        observation_bool(&completed, "observation_complete"),
        observation_bool(&completed, "cleanup_complete"),
        observation_bool(&completed, "tree_empty"),
        observation_bool(&completed, "launcher_reaped"),
    );
    assert_eq!(
        observation_status(&completed),
        4,
        "ResourceExhausted status"
    );
    assert!(!observation_bool(&completed, "success"));
    assert!(observation_bool(&completed, "has_memory_peak_bytes"));
    assert!(
        observation_unsigned(&completed, "memory_peak_bytes").unwrap_or(u64::MAX)
            <= 32 * 1024 * 1024
    );
    assert!(
        observation_unsigned(&completed, "memory_peak_bytes").unwrap_or_default()
            >= 16 * 1024 * 1024,
        "the provider reports the enforced memory peak"
    );
    assert!(observation_bool(&completed, "tree_empty"));
    assert!(observation_bool(&completed, "launcher_reaped"));
    assert!(observation_bool(&completed, "cleanup_complete"));
    eprintln!(
        "tiny cgroup exhaustion: memory_peak={} bytes, memory_high_events={}, memory_max_events={}, oom_kill_events={}",
        observation_unsigned(&completed, "memory_peak_bytes").unwrap_or_default(),
        observation_unsigned(&completed, "memory_high_events").unwrap_or_default(),
        observation_unsigned(&completed, "memory_max_events").unwrap_or_default(),
        observation_unsigned(&completed, "oom_kill_events").unwrap_or_default(),
    );
}

#[test]
fn process_launch_vectors_accept_capacity_and_reject_capacity_plus_one() {
    assert_eq!(
        PROCESS_LAUNCH_VECTOR_CAPACITY, 256,
        "runtime agrees with the stdlib launch-vector declaration"
    );
    let session = open_session();
    let options = process_grant("/bin/true");
    // Acceptance ladder: 0, 1, 16, 24, and 256 arguments pass the typed
    // boundary, start, and reach a terminal lifecycle state. Instantly
    // exiting processes may legitimately observe Unknown when the provider
    // never goes ready, so the ladder asserts boundary acceptance plus
    // termination; healthy execution is proven separately below.
    for count in [0usize, 1, 16, 24, PROCESS_LAUNCH_VECTOR_CAPACITY] {
        let args: Vec<String> = (0..count).map(|index| format!("arg{index:03}")).collect();
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let started = session
            .call_typed_json(
                "app.process_lifecycle_embed",
                "start_owned",
                &process_request("/bin/true", &refs, &[]),
                &options,
            )
            .unwrap_or_else(|error| panic!("{count} argv accepted: {error:?}"));
        assert_eq!(
            started.status, "returned",
            "{count} argv: {:?}",
            started.failure_reason
        );
        let handle = process_handle(&started.returned[0]);
        let observed = observe_until_terminal(&session, &handle, &options, count);
        assert_ne!(observation_status(&observed), 0, "{count} argv terminates");
        let reaped = call_observation(&session, "reap_owned", handle, &options);
        assert_ne!(observation_status(&reaped), 0, "{count} argv reaps");
    }
    // One healthy end-to-end execution through the same path.
    let sleep_options = process_grant("/usr/bin/sleep");
    let started = session
        .call_typed_json(
            "app.process_lifecycle_embed",
            "start_owned",
            &process_request("/usr/bin/sleep", &["0.2"], &[]),
            &sleep_options,
        )
        .expect("sleep starts");
    let handle = process_handle(&started.returned[0]);
    let observed = observe_until_terminal(&session, &handle, &sleep_options, 1);
    assert_eq!(observation_status(&observed), 1, "sleep exits");
    assert!(observation_bool(&observed, "success"));
    let reaped = call_observation(&session, "reap_owned", handle, &sleep_options);
    assert_eq!(observation_status(&reaped), 1, "sleep reaps");
    // One past capacity is rejected at the typed boundary, never executed.
    let over_args: Vec<String> = (0..=PROCESS_LAUNCH_VECTOR_CAPACITY)
        .map(|index| format!("arg{index:03}"))
        .collect();
    let over_refs: Vec<&str> = over_args.iter().map(String::as_str).collect();
    let rejected = session
        .call_typed_json(
            "app.process_lifecycle_embed",
            "start_owned",
            &process_request("/bin/true", &over_refs, &[]),
            &options,
        )
        .expect_err("257 argv rejected");
    assert!(
        rejected.message.contains("capacity"),
        "capacity diagnosed: {rejected:?}"
    );
    // The environment vector shares the launch-vector capacity.
    let env: Vec<(String, String)> = (0..PROCESS_LAUNCH_VECTOR_CAPACITY)
        .map(|index| (format!("K{index:03}"), "v".to_owned()))
        .collect();
    let env_refs: Vec<(&str, &str)> = env
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let started = session
        .call_typed_json(
            "app.process_lifecycle_embed",
            "start_owned",
            &process_request("/bin/true", &[], &env_refs),
            &options,
        )
        .expect("256 environment entries accepted");
    let handle = process_handle(&started.returned[0]);
    let observed = observe_until_terminal(&session, &handle, &options, 0);
    assert_ne!(
        observation_status(&observed),
        0,
        "256 environment entries terminate"
    );
    let reaped = call_observation(&session, "reap_owned", handle, &options);
    assert_ne!(
        observation_status(&reaped),
        0,
        "256 environment entries reap"
    );
    let mut over_env = env_refs.clone();
    over_env.push(("K257", "v"));
    let rejected = session
        .call_typed_json(
            "app.process_lifecycle_embed",
            "start_owned",
            &process_request("/bin/true", &[], &over_env),
            &options,
        )
        .expect_err("257 environment entries rejected");
    assert!(
        rejected.message.contains("capacity"),
        "capacity diagnosed: {rejected:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn process_stdin_streams_the_full_mncs_capacity_without_loss_or_reordering() {
    let session = open_session();
    let options = process_grant("/usr/bin/python3");
    let chunk_bytes = MAX_SEQUENCE_BOUND as usize;
    let max_bytes = chunk_bytes * (PROCESS_LAUNCH_VECTOR_CAPACITY + 1);
    let at_limit = (0..max_bytes)
        .map(|index| ((index * 131 + (index / chunk_bytes) * 29 + 17) & 0xff) as u8)
        .collect::<Vec<_>>();
    let hash_program = "import hashlib, sys\nsys.stdout.write(hashlib.sha256(sys.stdin.buffer.read()).hexdigest() + '\\n')\n";
    let started = session
        .call_typed_json(
            "app.process_lifecycle_embed",
            "start_owned",
            &process_request_with_stdin("/usr/bin/python3", &["-c", hash_program], &[], &at_limit),
            &options,
        )
        .expect("stdin at the full bounded MNCS capacity is a valid typed request");
    assert_eq!(started.status, "returned", "{:?}", started.failure_reason);
    let handle = process_handle(&started.returned[0]);
    let completed = call_observation(&session, "reap_owned", handle, &options);
    assert!(observation_bool(&completed, "has_exit_code"));
    assert_eq!(observation_signed(&completed, "exit_code"), Some(0));
    assert_eq!(
        observation_bytes(&completed, "stdout"),
        b"3608c6208fa3a2cec7131f9eee1e150abb7de7f9d5944d7eed56f18856d99437\n",
        "the independent digest covers every byte in order across all chunks"
    );
    match observation_status(&completed) {
        1 => assert!(observation_bool(&completed, "success")),
        7 => {
            assert!(
                !observation_bool(&completed, "containment_supported"),
                "UNKNOWN is retained when the provider cannot prove containment"
            );
            assert!(!observation_bool(&completed, "observation_complete"));
        }
        status => {
            panic!("the child exits or remains explicitly UNKNOWN, got {status}: {completed:?}")
        }
    }
    assert!(observation_bool(&completed, "launcher_reaped"));
    assert!(observation_bool(&completed, "cleanup_complete"));
    assert!(observation_bool(&completed, "tree_empty"));

    let over_limit = vec![b'x'; max_bytes + 1];
    let rejected = session
        .call_typed_json(
            "app.process_lifecycle_embed",
            "start_owned",
            &process_request_with_stdin(
                "/usr/bin/python3",
                &["-c", hash_program],
                &[],
                &over_limit,
            ),
            &options,
        )
        .expect_err("stdin beyond the MNCS contract capacity is rejected before launch");
    assert!(
        rejected.message.contains("256")
            || rejected.message.contains("capacity")
            || rejected.message.contains("bound"),
        "the bounded chunk capacity is reported: {rejected:?}"
    );

    let inconsistent_count = process_request_with_stdin(
        "/usr/bin/python3",
        &["-c", hash_program],
        &[],
        &at_limit[..chunk_bytes + 1],
    );
    let mut inconsistent_count: Value =
        serde_json::from_str(&inconsistent_count).expect("chunked request JSON");
    inconsistent_count[0]["record"]["fields"]["stdin_chunk_count"] = integer(0);
    let inconsistent_count = serde_json::to_string(&inconsistent_count).expect("request JSON");
    let rejected = session
        .call_typed_json(
            "app.process_lifecycle_embed",
            "start_owned",
            &inconsistent_count,
            &options,
        )
        .expect("mismatched chunk count returns a structured request failure");
    assert_eq!(rejected.status, "invalid_request");
    let rejection_reason = rejected.failure_reason.as_deref().unwrap_or_default();
    assert!(
        rejection_reason.contains("stdin_chunk_count"),
        "the invalid chunk count is identified: {rejected:?}"
    );

    let mut empty_chunk: Value = serde_json::from_str(&process_request(
        "/usr/bin/python3",
        &["-c", hash_program],
        &[],
    ))
    .expect("process request JSON");
    empty_chunk[0]["record"]["fields"]["stdin_chunks"] =
        json!({"sequence":{"values":[byte_sequence(&[])]}});
    empty_chunk[0]["record"]["fields"]["stdin_chunk_count"] = integer(1);
    let empty_chunk = serde_json::to_string(&empty_chunk).expect("request JSON");
    let rejected = session
        .call_typed_json(
            "app.process_lifecycle_embed",
            "start_owned",
            &empty_chunk,
            &options,
        )
        .expect("empty continuation chunk returns a structured request failure");
    assert_eq!(rejected.status, "invalid_request");
    let rejection_reason = rejected.failure_reason.as_deref().unwrap_or_default();
    assert!(
        rejection_reason.contains("must not be empty"),
        "the empty chunk is diagnosed: {rejected:?}"
    );
}

#[test]
fn process_observation_carries_a_truncated_window_of_chatty_output() {
    // Output within the requested capture budget but beyond the 1KB typed
    // observation window terminates normally; the observation carries the
    // window with truncation reported instead of failing the execution.
    let session = open_session();
    let options = process_grant("/bin/echo");
    let args: Vec<String> = (0..256).map(|index| format!("word{index:03}")).collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let started = session
        .call_typed_json(
            "app.process_lifecycle_embed",
            "start_owned",
            &process_request_with_envelope(
                "/bin/echo",
                &refs,
                &[],
                128 * 1024 * 1024,
                256 * 1024 * 1024,
                0,
                true,
                32,
                65536,
                65536,
            ),
            &options,
        )
        .expect("chatty execution starts");
    assert_eq!(
        started.status, "returned",
        "chatty start: {:?}",
        started.failure_reason
    );
    let handle = process_handle(&started.returned[0]);
    let observed = observe_until_terminal(&session, &handle, &options, 256);
    let status = observation_status(&observed);
    assert_ne!(status, 0, "chatty execution terminates");
    assert_ne!(status, 5, "within-budget output is not OutputExhausted");
    assert!(
        observation_bool(&observed, "stdout_truncated"),
        "window truncation reported"
    );
    assert_eq!(
        observation_bytes_len(&observed, "stdout"),
        1024,
        "observation carries exactly the transport window"
    );
    let reaped = call_observation(&session, "reap_owned", handle, &options);
    assert_ne!(observation_status(&reaped), 0, "chatty execution reaps");
}
