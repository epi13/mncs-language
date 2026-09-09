//! P-010 embedding evidence: a retained in-process session answers
//! repeated calls against one verified artifact with millisecond-scale
//! marginal cost, reports the exact digest executed, refuses tampered
//! artifacts, and serves the stable C ABI without Rust layout.

use std::ffi::{CStr, CString};
use std::time::Instant;

use mncs_embed::{Artifact, CallOptions, Session};

const SOURCE: &str = "mncs 0.6;\nmodule probe.embed;\nfn decide(x: i64) -> (result: i64) {\n    if x >= 100 {\n        return 1;\n    }\n    if x >= 10 {\n        return 2;\n    }\n    return 3;\n}\n";

fn i64_arg(value: i64) -> String {
    format!(
        "[{{\"integer\": {{\"value\": {value}, \"type\": {{\"bits\": 64, \"signed\": true}}}}}}]"
    )
}

fn open_session() -> (Session, String) {
    let artifact =
        Artifact::from_source(SOURCE, "mncs-research-bytecode").expect("compile probe artifact");
    let digest = artifact.digest().to_owned();
    let session = Session::open(artifact).expect("open session");
    assert!(
        session.reused(),
        "research artifact must prepare reusable state"
    );
    assert_eq!(session.digest(), digest);
    (session, digest)
}

/// Repeated calls against one loaded artifact agree verdict-for-verdict
/// with fresh expectations and carry the executed digest on every call.
#[test]
fn repeated_calls_agree_and_carry_digest() {
    let (session, digest) = open_session();
    for (input, expected) in [(150, 1), (50, 2), (5, 3), (100, 1), (10, 2), (9, 3)] {
        let output = session
            .call_json(
                "probe.embed",
                "decide",
                &i64_arg(input),
                &CallOptions::budgeted(8_192),
            )
            .expect("call");
        assert_eq!(output.status, "returned", "input {input}");
        assert_eq!(output.artifact_sha256, digest);
        assert!(output.reused_session);
        let value = match &output.returned[..] {
            [mncs_model::ExecutionValue::Integer { value, .. }] => *value,
            other => panic!("integer verdict expected, got {other:?}"),
        };
        assert_eq!(value, expected as i128, "input {input}");
    }
}

/// A tampered artifact (one payload hex digit flipped) is refused at
/// open: the identity no longer validates, so no session ever executes
/// substituted bytes.
#[test]
fn tampered_artifact_is_refused_at_open() {
    let artifact =
        Artifact::from_source(SOURCE, "mncs-research-bytecode").expect("compile probe artifact");
    let mut document: serde_json::Value =
        serde_json::from_slice(&artifact.to_json_bytes()).expect("artifact JSON");
    let hex = document["bytes_hex"]
        .as_str()
        .expect("bytes_hex")
        .to_owned();
    let mut chars: Vec<char> = hex.chars().collect();
    let last = chars.len() - 1;
    chars[last] = if chars[last] == '0' { '1' } else { '0' };
    document["bytes_hex"] = serde_json::Value::String(chars.into_iter().collect());
    let tampered = serde_json::to_vec(&document).expect("tampered JSON");
    let error = match Artifact::from_json(&tampered) {
        Ok(_) => panic!("tampered artifact must be refused"),
        Err(error) => error,
    };
    assert!(
        error.code == "invalid_identity" || error.code == "invalid_artifact",
        "unexpected code: {error:?}"
    );
}

/// Structured failures cross the boundary: an unknown entrypoint reports
/// `invalid_request` with a reason, never a value and never a trap.
#[test]
fn unknown_entrypoint_reports_structured_failure() {
    let (session, _) = open_session();
    let output = session
        .call_json(
            "probe.embed",
            "no_such_function",
            &i64_arg(1),
            &CallOptions::budgeted(8_192),
        )
        .expect("call");
    assert_eq!(output.status, "invalid_request");
    assert!(output.returned.is_empty());
    assert!(output.failure_reason.is_some());
}

/// Repeated-call latency on an already-loaded artifact: the subprocess
/// pilot paid ~280 ms fixed per batch; the embedded path must be
/// millisecond-scale per call.
#[test]
fn repeated_call_latency_is_millisecond_scale() {
    let (session, _) = open_session();
    let options = CallOptions::budgeted(8_192);
    // Warm up once so setup costs stay out of the measurement.
    session
        .call_json("probe.embed", "decide", &i64_arg(42), &options)
        .expect("warmup");
    let calls = 200;
    let mut samples = Vec::with_capacity(calls);
    for i in 0..calls {
        let started = Instant::now();
        let output = session
            .call_json("probe.embed", "decide", &i64_arg(i as i64), &options)
            .expect("call");
        assert_eq!(output.status, "returned");
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let p50 = samples[samples.len() / 2];
    let p99 = samples[samples.len() * 99 / 100];
    eprintln!("embed latency ms over {calls} calls: mean={mean:.3} p50={p50:.3} p99={p99:.3}");
    assert!(
        mean < 50.0,
        "mean per-call latency must be millisecond-scale, got {mean:.3} ms"
    );
}

/// The C ABI round-trips without Rust layout: open from frozen bytes,
/// read identity, call an entrypoint, free everything.
#[test]
fn c_abi_round_trip_without_rust_layout() {
    use mncs_embed::{
        mncs_last_error, mncs_response_free, mncs_response_text, mncs_session_call,
        mncs_session_close, mncs_session_info, mncs_session_open,
    };

    let artifact =
        Artifact::from_source(SOURCE, "mncs-research-bytecode").expect("compile probe artifact");
    let frozen = artifact.to_json_bytes();
    let expected_digest = artifact.digest().to_owned();

    let read_text = |response: *mut mncs_embed::CallResponse| -> String {
        assert!(!response.is_null(), "null response");
        let text = unsafe { CStr::from_ptr(mncs_response_text(response)) }
            .to_str()
            .expect("utf8")
            .to_owned();
        unsafe { mncs_response_free(response) };
        text
    };

    let handle = unsafe { mncs_session_open(frozen.as_ptr(), frozen.len()) };
    assert!(!handle.is_null(), "open failed: {:?}", unsafe {
        CStr::from_ptr(mncs_last_error())
    });
    let info: serde_json::Value =
        serde_json::from_str(&read_text(unsafe { mncs_session_info(handle) })).expect("info JSON");
    assert_eq!(info["artifact_sha256"], expected_digest);
    assert_eq!(info["reused_session"], true);

    let module = CString::new("probe.embed").unwrap();
    let function = CString::new("decide").unwrap();
    let args = CString::new(i64_arg(150)).unwrap();
    let output: serde_json::Value = serde_json::from_str(&read_text(unsafe {
        mncs_session_call(
            handle,
            module.as_ptr(),
            function.as_ptr(),
            args.as_ptr(),
            std::ptr::null(),
            8_192,
        )
    }))
    .expect("call JSON");
    assert_eq!(output["status"], "returned");
    assert_eq!(output["artifact_sha256"], expected_digest);
    assert_eq!(output["returned"][0]["integer"]["value"], 1);

    unsafe { mncs_session_close(handle) };
}
