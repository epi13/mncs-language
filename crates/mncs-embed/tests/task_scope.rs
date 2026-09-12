//! Index PRESS-001/002/008/012 + PRESS-010: structured task scopes as the
//! runtime realization of the `mncs.std.scope.v1` source contracts.
//!
//! A `TaskScope` runs many entrypoint calls of one verified artifact with
//! bounded parallelism, deterministic id-ordered merge, failure
//! aggregation by index (never by completion time), and cooperative
//! cancellation. The index-shaped benchmark proves reusable invocation is
//! millisecond-scale per call with artifact identity attached to every
//! result.

use std::time::Instant;

use mncs_embed::{Artifact, CallOptions, Grant, TaskScope, WorkItem};
use mncs_model::ExecutionValue;

const SOURCE: &str = "mncs 0.10;\nmodule probe.scope_tasks;\nfn classify(x: i64) -> (result: i64) {\n    if x >= 100 {\n        return 1;\n    }\n    if x >= 10 {\n        return 2;\n    }\n    return 3;\n}\nfn add_pair(a: i64, b: i64) -> (result: i64) {\n    return a + b;\n}\nfn blob_len() -> (result: u64)\n    capability probe_reader\n    effect host_read authorized_by probe_reader\n{\n    let blob: [byte; up_to 64] = host_read();\n    return blob.len;\n}\n";

fn i64_arg(value: i64) -> ExecutionValue {
    mncs_model::ExecutionValue::Integer {
        value: value as i128,
        ty: mncs_model::IntegerType {
            bits: 64,
            signed: true,
        },
    }
}

fn as_i64(value: &ExecutionValue) -> i128 {
    match value {
        ExecutionValue::Integer { value, .. } => *value,
        other => panic!("integer verdict expected, got {other:?}"),
    }
}

fn open_scope(capacity: usize) -> (TaskScope, String) {
    let artifact =
        Artifact::from_source(SOURCE, "mncs-research-bytecode").expect("compile scope artifact");
    assert_eq!(artifact.backend_name(), "mncs-research-bytecode");
    let digest = artifact.digest().to_owned();
    let scope = TaskScope::open(&artifact, capacity).expect("open scope");
    assert_eq!(scope.artifact_sha256(), digest);
    (scope, digest)
}

fn classify_item(value: i64) -> WorkItem {
    WorkItem::new(
        "probe.scope_tasks",
        "classify",
        vec![i64_arg(value)],
        CallOptions::budgeted(8_192),
    )
}

fn add_item(a: i64, b: i64) -> WorkItem {
    WorkItem::new(
        "probe.scope_tasks",
        "add_pair",
        vec![i64_arg(a), i64_arg(b)],
        CallOptions::budgeted(8_192),
    )
}

/// Task test: two independent tasks return distinct values, join collects
/// both by index, and every result carries the executed digest.
#[test]
fn concurrent_tasks_join_with_distinct_values() {
    let (scope, digest) = open_scope(2);
    let run = scope
        .run_concurrent(vec![classify_item(150), add_item(20, 22)])
        .expect("run");
    assert_eq!(run.first_failure, None);
    assert_eq!(run.executed, 2);
    assert_eq!(run.cancelled_count, 0);
    assert_eq!(run.results.len(), 2);
    assert_eq!(run.results[0].status, "returned");
    assert_eq!(run.results[1].status, "returned");
    assert_eq!(as_i64(&run.results[0].returned[0]), 1);
    assert_eq!(as_i64(&run.results[1].returned[0]), 42);
    for result in &run.results {
        assert!(!result.cancelled);
        assert_eq!(result.artifact_sha256, digest);
    }
}

/// The same work merged concurrently and sequentially agrees
/// verdict-for-verdict: merge order is a function of task ids, never of
/// thread scheduling.
#[test]
fn concurrent_merge_matches_sequential_batch() {
    let (scope, _) = open_scope(4);
    let items: Vec<WorkItem> = (0..64)
        .map(|i: i64| {
            if i % 3 == 0 {
                add_item(i, i * 2)
            } else {
                classify_item(i * 7 - 100)
            }
        })
        .collect();
    let concurrent = scope.run_concurrent(items.clone()).expect("concurrent");
    let batch = scope.run_batch(items).expect("batch");
    assert_eq!(concurrent.first_failure, None);
    assert_eq!(batch.first_failure, None);
    assert_eq!(concurrent.results.len(), batch.results.len());
    for (left, right) in concurrent.results.iter().zip(batch.results.iter()) {
        assert_eq!(left.index, right.index);
        assert_eq!(left.status, right.status);
        assert_eq!(left.returned, right.returned);
    }
}

/// Failure test: the failing index is reported (lowest failed index, not
/// fastest thread), the reason stays inspectable, and siblings that ran
/// still return — cancellation stays distinguishable from failure.
#[test]
fn failure_aggregates_by_index_with_inspectable_reason() {
    let (scope, digest) = open_scope(3);
    let mut items = vec![classify_item(150), add_item(1, 2), classify_item(5)];
    items[1] = WorkItem::new(
        "probe.scope_tasks",
        "no_such_function",
        vec![],
        CallOptions::budgeted(8_192),
    );
    let run = scope.run_concurrent(items).expect("run");
    assert_eq!(run.first_failure, Some(1));
    assert_eq!(run.results[0].status, "returned");
    assert_eq!(run.results[1].status, "invalid_request");
    assert_eq!(run.results[2].status, "returned");
    assert!(run.results[1].failure_reason.is_some());
    assert!(!run.results[1].cancelled);
    for result in &run.results {
        assert_eq!(result.artifact_sha256, digest);
    }
}

/// Cancellation test: a pre-cancelled scope executes nothing, publishes
/// no value, and counts every skipped task as cleanup evidence.
#[test]
fn pre_cancel_publishes_nothing_and_counts_cleanup() {
    let (scope, _) = open_scope(4);
    scope.cancel();
    let run = scope
        .run_concurrent(vec![classify_item(150), add_item(1, 2), classify_item(5)])
        .expect("run");
    assert_eq!(run.executed, 0);
    assert_eq!(run.cancelled_count, 3);
    assert_eq!(run.first_failure, None);
    assert_eq!(scope.cleanups(), 3);
    for result in &run.results {
        assert!(result.cancelled);
        assert_eq!(result.status, "cancelled");
        assert!(result.returned.is_empty());
        assert!(result.failure_reason.is_none());
    }
}

/// Filesystem grants travel per task: a scoped `fs_list_count` call over
/// a temporary root returns its entry count with the executed digest.
#[test]
fn fs_grant_lists_granted_root_per_task() {
    use mncs_embed::Grant;
    let dir = std::env::temp_dir().join(format!(
        "mncs-embed-fs-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(dir.join("sub")).expect("mkdirs");
    std::fs::write(dir.join("a.txt"), b"alpha").expect("write");
    std::fs::write(dir.join("sub/b.txt"), b"beta").expect("write");
    let artifact = Artifact::from_source(
        "mncs 0.12;\nmodule probe.fs_count;\nfn count() -> (result: u64)\n    capability fs_root\n    effect fs_list authorized_by fs_root\n{\n    return fs_list_count();\n}\n",
        "mncs-research-bytecode",
    )
    .expect("compile fs probe");
    let scope = TaskScope::open(&artifact, 2).expect("open");
    let item = WorkItem::new(
        "probe.fs_count",
        "count",
        vec![],
        mncs_embed::CallOptions {
            step_budget: 8_192,
            grants: vec![Grant::fs_root("fs_root", &dir.to_string_lossy())],
            type_arguments: Vec::new(),
        },
    );
    let run = scope.run_batch(vec![item]).expect("run");
    assert_eq!(run.first_failure, None);
    assert_eq!(run.results[0].status, "returned");
    let count = match &run.results[0].returned[..] {
        [mncs_model::ExecutionValue::Integer { value, .. }] => *value,
        other => panic!("u64 count expected, got {other:?}"),
    };
    // a.txt, sub, sub/b.txt in canonical order.
    assert_eq!(count, 3);
}

/// The C batch boundary executes many entrypoints in one crossing and
/// returns the outputs in request order.
#[test]
fn c_abi_batch_returns_outputs_in_order() {
    use std::ffi::{CStr, CString};
    let artifact = Artifact::from_source(SOURCE, "mncs-research-bytecode").expect("compile");
    let session = mncs_embed::Session::open(artifact).expect("open");
    let requests = CString::new(
        r#"[{"module":"probe.scope_tasks","function":"classify","args":[{"integer":{"value":150,"type":{"bits":64,"signed":true}}}],"step_budget":8192},{"module":"probe.scope_tasks","function":"add_pair","args":[{"integer":{"value":20,"type":{"bits":64,"signed":true}}},{"integer":{"value":22,"type":{"bits":64,"signed":true}}}],"step_budget":8192}]"#,
    )
    .expect("cstring");
    let response = unsafe {
        mncs_embed::mncs_session_call_batch(
            &session as *const mncs_embed::Session,
            requests.as_ptr(),
        )
    };
    assert!(!response.is_null(), "batch must answer");
    let text = unsafe {
        let ptr = mncs_embed::mncs_response_text(response as *const _);
        assert!(!ptr.is_null());
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    };
    let outputs: serde_json::Value = serde_json::from_str(&text).expect("outputs JSON");
    let outputs = outputs.as_array().expect("array");
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0]["status"], "returned");
    assert_eq!(outputs[1]["status"], "returned");
    assert_eq!(outputs[0]["returned"][0]["integer"]["value"], 1);
    assert_eq!(outputs[1]["returned"][0]["integer"]["value"], 42);
    assert_eq!(outputs[0]["artifact_sha256"], outputs[1]["artifact_sha256"]);
    unsafe { mncs_embed::mncs_response_free(response) };
}

/// Ownership test: after close, both run paths refuse — work cannot
/// escape its scope.
#[test]
fn closed_scope_refuses_further_runs() {
    let (scope, _) = open_scope(2);
    let first = scope
        .run_batch(vec![classify_item(150)])
        .expect("first run");
    assert_eq!(first.first_failure, None);
    scope.close();
    assert!(scope.is_closed());
    let batch = scope.run_batch(vec![classify_item(150)]);
    assert!(batch.is_err(), "batch after close must refuse");
    assert_eq!(batch.unwrap_err().code, "scope_closed");
    let concurrent = scope.run_concurrent(vec![classify_item(150)]);
    assert!(
        concurrent.is_err(),
        "concurrent run after close must refuse"
    );
}

/// Index-shaped benchmark: hundreds of mixed-entrypoint calls through one
/// scope, plus a granted host-effect call. Asserts millisecond-scale warm
/// calls with stable artifact identity on every result.
#[test]
fn index_shaped_workload_is_millisecond_scale() {
    let opened = Instant::now();
    let (scope, digest) = open_scope(8);
    let open_cost = opened.elapsed();

    // 600 mixed calls across two entrypoints, like one index build's
    // kernel-call shape (hundreds of small pure calls).
    let items: Vec<WorkItem> = (0..600)
        .map(|i: i64| {
            if i % 3 == 2 {
                add_item(i, 1)
            } else {
                classify_item(i - 200)
            }
        })
        .collect();

    let started = Instant::now();
    let run = scope.run_concurrent(items).expect("concurrent workload");
    let wall = started.elapsed();
    assert_eq!(run.first_failure, None, "workload must fully return");
    assert_eq!(run.executed, 600);
    // Spot-check values at both ends and the middle: merge is by index.
    assert_eq!(as_i64(&run.results[0].returned[0]), 3);
    assert_eq!(as_i64(&run.results[599].returned[0]), 600);
    assert_eq!(as_i64(&run.results[300].returned[0]), 1);
    for result in &run.results {
        assert_eq!(result.artifact_sha256, digest, "identity on every call");
    }

    // Warm-call latency on one session: the PRESS-010 acceptance shape.
    let session_artifact =
        Artifact::from_source(SOURCE, "mncs-research-bytecode").expect("artifact");
    let session = mncs_embed::Session::open(session_artifact).expect("session");
    let options = CallOptions::budgeted(8_192);
    let _warmup = session.call("probe.scope_tasks", "classify", vec![i64_arg(42)], &options);
    assert_eq!(_warmup.status, "returned");
    let calls = 200;
    let mut samples = Vec::with_capacity(calls);
    for i in 0..calls {
        let call_started = Instant::now();
        let output = session.call(
            "probe.scope_tasks",
            "classify",
            vec![i64_arg(i as i64)],
            &options,
        );
        samples.push(call_started.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(output.status, "returned");
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let p50 = samples[samples.len() / 2];
    let p99 = samples[(samples.len() * 99 / 100).min(samples.len() - 1)];

    // Granted host-effect call through the same scope path.
    let grant =
        Grant::read_bytes("probe_reader", "scope-bench", b"hello-scope".to_vec()).expect("grant");
    let effect_item = WorkItem::new(
        "probe.scope_tasks",
        "blob_len",
        vec![],
        CallOptions {
            step_budget: 8_192,
            grants: vec![grant],
            type_arguments: Vec::new(),
        },
    );
    let effect_run = scope.run_batch(vec![effect_item]).expect("effect call");
    assert_eq!(effect_run.results[0].status, "returned");

    eprintln!(
        "scope open: {open_cost:?}; 600-call concurrent wall: {wall:?}; warm mean {mean:.3} ms p50 {p50:.3} ms p99 {p99:.3} ms"
    );
    assert!(
        mean < 1.0,
        "warm mean {mean:.3} ms must stay millisecond-scale (subprocess is ~10 ms)"
    );
}
