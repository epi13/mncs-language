//! Tranche A (P1-001/P1-002/P1-003, P2-005): durable filesystem
//! mutation behind the granted root.
//!
//! The `fs_write` family (create, positioned write, append, mkdir,
//! delete, same-dir atomic rename, sync barrier) executes on the
//! research bytecode backend with an explicit `--grant-fs` grant: a
//! nine-case fixture-anchored lifecycle grows and patches a fixture
//! file, seals it, stages and publishes new entries, and retires them,
//! with every case returning under the translation validator's
//! independent observed replay (overall status PASS). A second,
//! host-threaded lifecycle proves the store-driver index-threading
//! pattern across both layers. Compiled backends refuse the
//! all-effectful module whole-program with explicit diagnostics and
//! never touch the destination; ungranted runs fail closed as
//! Unsupported; ill-formed mutations (double create, path names,
//! sparse gaps, wild indices, non-empty dirs, dir barriers) refuse as
//! InvalidRequest and leave no trace. Body and SSA executors agree
//! case-by-case on twin-root replays.

use std::process::Command;

use serde_json::{json, Value};

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn source() -> String {
    example("source/pressure-fs-mutation.mncs")
}

fn corpus() -> String {
    example("execution/pressure-fs-mutation-corpus.json")
}

fn u64_arg(value: u64) -> Value {
    json!({"integer": {"value": value, "type": {"bits": 64, "signed": false}}})
}

fn view_arg(bytes: &[u8]) -> Value {
    json!({"sequence": {"values": bytes.iter().map(|byte| json!({"byte": {"value": *byte}})).collect::<Vec<_>>()}})
}

fn workspace(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mncs-fs-mutation-{}-{}-{:?}",
        tag,
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create workspace");
    dir
}

/// Host fixtures for the experiment corpus: `aa-probe` (8-byte file)
/// and `aa-stage` (empty directory) sort first, so fixture indices 0/1
/// stay put while lifecycle entries land after them. Fixtures make
/// every corpus case independently validatable: the translation
/// validator replays each case alone under the observed (intent-only)
/// policy on the pristine tree, where a refused case can never agree —
/// so indexed cases only ever address fixture entries.
fn fixture_workspace(tag: &str) -> std::path::PathBuf {
    let dir = workspace(tag);
    std::fs::write(dir.join("aa-probe"), b"PROBE123").expect("fixture file");
    std::fs::create_dir_all(dir.join("aa-stage")).expect("fixture dir");
    dir
}

/// One direct call against the body (`execute`) or SSA (`execute-ssa`)
/// reference executor with an explicit grant set.
fn run_direct(command: &str, function: &str, args: Vec<Value>, root: &std::path::Path) -> Value {
    use std::io::Write;
    let request = json!({
        "schema_version": "0.1",
        "target": {"module": "pressure.fs_mutation", "function": function},
        "arguments": args,
        "step_budget": 100000,
        "policy": {"effects": "realize"},
        "host_grants": [{"capability": "fs_root", "locator": root.to_string_lossy(), "bytes": []}],
    });
    let mut child = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([command, &source(), "/dev/stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn direct execute");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(
            serde_json::to_string(&request)
                .expect("request JSON")
                .as_bytes(),
        )
        .expect("write request");
    let output = child.wait_with_output().expect("run direct execute");
    // Direct execution exits nonzero for non-returned statuses by
    // design (invalid_request, ...): the status lives in the JSON.
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "{command} {function}: no result JSON (rc={:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn run_experiment(backend: &str, extra: &[String]) -> (bool, Value) {
    let mut args = vec![
        "experiment".to_owned(),
        "run".to_owned(),
        source(),
        "--backend".to_owned(),
        backend.to_owned(),
        "--corpus".to_owned(),
        corpus(),
    ];
    args.extend(extra.iter().cloned());
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(&args)
        .output()
        .expect("run experiment");
    let ok = output.status.success();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let result: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| json!({"_stderr": stderr, "_rc": output.status.code()}));
    (ok, result)
}

fn returned_u64(case: &Value) -> u64 {
    case["returned"][0]["integer"]["value"]
        .as_u64()
        .unwrap_or_else(|| panic!("u64 return: {case:#}"))
}

/// The granted lifecycle runs end to end on the research backend: all
/// nine cases return with expectations met (overall status PASS, so the
/// translation validation's independent observed replay agrees
/// layer-by-layer), mutation effects carry the `fs_write` kind with
/// post-state provenance, and the tree ends as designed (`aa-stage`
/// plus the published `w-final` carrying the patched probe bytes).
#[test]
fn granted_mutation_lifecycle_returns_and_retires() {
    let root = fixture_workspace("lifecycle");
    let grant = format!("fs_root={}", root.to_string_lossy());
    let (ok, result) = run_experiment("mncs-research-bytecode", &["--grant-fs".to_owned(), grant]);
    assert!(ok, "granted run must succeed: {result:#}");
    assert_eq!(result["status"], "PASS", "experiment status: {result:#}");
    let cases = result["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 9, "corpus drift");
    for case in cases {
        let id = case["case_id"].as_str().unwrap_or("?");
        assert_eq!(case["status"], "returned", "{id}: {case:#}");
        assert_eq!(case["expectation_met"], true, "{id}: {case:#}");
        let effects = case["effects"].as_array().expect("effects");
        assert_eq!(effects.len(), 1, "{id}: one effect per call");
        let expected_kind = if id == "read-probe" {
            "fs_read"
        } else {
            "fs_write"
        };
        assert_eq!(effects[0]["kind"], expected_kind, "{id}: effect kind");
        assert_eq!(effects[0]["capability"], "fs_root", "{id}: capability");
        let provenance = effects[0]["provenance"].as_str().unwrap_or("");
        assert!(
            provenance.contains("grant:") && provenance.contains("gen:"),
            "{id}: provenance carries grant and post-state generation: {provenance}"
        );
    }
    // The read-back case is the byte-exact content proof: the fixture
    // "PROBE123" grown by [7,7] and patched at 0 with [9].
    let read_back = cases
        .iter()
        .find(|case| case["case_id"] == "read-probe")
        .expect("read-probe");
    let bytes: Vec<u8> = read_back["returned"][0]["sequence"]["values"]
        .as_array()
        .expect("sequence")
        .iter()
        .map(|entry| entry["byte"]["value"].as_u64().expect("byte") as u8)
        .collect();
    assert_eq!(
        bytes,
        vec![9, 82, 79, 66, 69, 49, 50, 51, 7, 7],
        "content proof"
    );
    // Designed end-state: the stage dir survives, the published file
    // carries the patched probe bytes, everything staged is retired.
    assert!(root.join("aa-stage").is_dir(), "fixture dir survives");
    assert_eq!(
        std::fs::read(root.join("w-final")).expect("published bytes"),
        vec![9, 82, 79, 66, 69, 49, 50, 51, 7, 7],
        "published content"
    );
    assert!(!root.join("aa-probe").exists(), "probe was published away");
    assert!(!root.join("chunk").exists(), "staged chunk retired");
    assert!(!root.join("temp").exists(), "stage dir retired");
}

/// Body and SSA executors agree on every corpus case: replaying the
/// corpus order through `execute` and `execute-ssa` on twin fixture
/// roots yields the corpus expectations on both layers.
#[test]
fn mutation_layers_agree_body_and_ssa() {
    let corpus_text = std::fs::read_to_string(corpus()).expect("corpus");
    let corpus: Value = serde_json::from_str(&corpus_text).expect("corpus JSON");
    for command in ["execute", "execute-ssa"] {
        let root = fixture_workspace(command);
        for case in corpus["cases"].as_array().expect("cases") {
            let id = case["id"].as_str().unwrap_or("?");
            let function = case["request"]["target"]["function"]
                .as_str()
                .expect("function");
            let args: Vec<Value> = case["request"]["arguments"]
                .as_array()
                .expect("args")
                .to_vec();
            let direct = run_direct(command, function, args, &root);
            assert_eq!(direct["status"], "returned", "{command} {id}: {direct:#}");
            assert_eq!(
                direct["returned"], case["expected"],
                "{command} {id}: layer mismatch"
            );
        }
        // Twin roots converge to the designed end-state on both layers.
        assert!(
            root.join("aa-stage").is_dir(),
            "{command}: fixture dir survives"
        );
        assert_eq!(
            std::fs::read(root.join("w-final")).expect("published bytes"),
            vec![9, 82, 79, 66, 69, 49, 50, 51, 7, 7],
            "{command}: published content"
        );
    }
}

/// The store-driver pattern — host-side index threading across calls —
/// agrees byte-for-byte between the layers: stage, grow, patch, publish
/// via atomic rename, barrier, read back at the NEW index, retire both
/// entries, ending empty. This is the lifecycle the translation
/// validator cannot replay independently (sequential indices), so direct
/// two-layer replay is its proof.
#[test]
fn threaded_lifecycle_agrees_across_layers() {
    let script: Vec<(&str, Vec<Value>, Value)> = vec![
        (
            "stage",
            vec![view_arg(b"chunk"), view_arg(&[1, 2, 3, 4])],
            u64_arg(0),
        ),
        ("make_dir", vec![view_arg(b"temp")], u64_arg(1)),
        ("grow", vec![u64_arg(0), view_arg(&[5, 6])], u64_arg(2)),
        (
            "patch",
            vec![u64_arg(0), u64_arg(0), view_arg(&[9])],
            u64_arg(1),
        ),
        ("publish", vec![u64_arg(0), view_arg(b"final")], u64_arg(0)),
        ("barrier", vec![u64_arg(0)], u64_arg(1)),
        (
            "read_back",
            vec![u64_arg(0), u64_arg(0), u64_arg(64)],
            view_arg(&[9, 2, 3, 4, 5, 6]),
        ),
        ("retire", vec![u64_arg(1)], u64_arg(1)),
        ("retire", vec![u64_arg(0)], u64_arg(0)),
    ];
    let mut observed: Vec<(String, Value)> = Vec::new();
    for command in ["execute", "execute-ssa"] {
        let root = workspace(&format!("threaded-{command}"));
        for (step, (function, args, expected)) in script.iter().enumerate() {
            let direct = run_direct(command, function, args.clone(), &root);
            assert_eq!(
                direct["status"], "returned",
                "{command} step {step} {function}: {direct:#}"
            );
            assert_eq!(
                direct["returned"],
                Value::Array(vec![expected.clone()]),
                "{command} step {step} {function}: value mismatch"
            );
            if command == "execute" {
                observed.push((function.to_string(), direct["returned"].clone()));
            } else {
                assert_eq!(
                    direct["returned"], observed[step].1,
                    "layer mismatch at step {step} {function}"
                );
            }
        }
        let remaining: Vec<_> = std::fs::read_dir(&root).expect("read root").collect();
        assert!(remaining.is_empty(), "{command}: tree ends empty");
    }
}

/// Refused mutations fail closed as InvalidRequest and leave no trace:
/// double create, path-shaped names, sparse-gap writes, wild indices,
/// non-empty directory deletes, symlink handling, and directory
/// barriers. The byte-exact survivor check proves refused writes never
/// land.
#[test]
fn mutation_refusals_fail_closed_without_trace() {
    let root = workspace("refusals");
    // Setup: victim file [1] at index 0.
    let created = run_direct(
        "execute",
        "stage",
        vec![view_arg(b"victim"), view_arg(&[1])],
        &root,
    );
    assert_eq!(created["status"], "returned", "{created:#}");
    assert_eq!(returned_u64(&created), 0);
    // Double create refuses; original bytes intact.
    let refused = run_direct(
        "execute",
        "stage",
        vec![view_arg(b"victim"), view_arg(&[2])],
        &root,
    );
    assert_eq!(refused["status"], "invalid_request", "{refused:#}");
    assert_eq!(std::fs::read(root.join("victim")).expect("read"), vec![1]);
    // Path-shaped name refuses; nothing is created anywhere.
    let refused = run_direct(
        "execute",
        "stage",
        vec![view_arg(b"sub/escape"), view_arg(&[1])],
        &root,
    );
    assert_eq!(refused["status"], "invalid_request", "{refused:#}");
    assert!(!root.join("sub").exists() && !root.join("escape").exists());
    // Sparse-gap write refuses; file intact.
    let refused = run_direct(
        "execute",
        "patch",
        vec![u64_arg(0), u64_arg(99), view_arg(&[1])],
        &root,
    );
    assert_eq!(refused["status"], "invalid_request", "{refused:#}");
    assert_eq!(std::fs::read(root.join("victim")).expect("read"), vec![1]);
    // Wild indices refuse on every indexed entrypoint.
    for (function, args) in [
        ("grow", vec![u64_arg(7), view_arg(&[1])]),
        ("patch", vec![u64_arg(7), u64_arg(0), view_arg(&[1])]),
        ("retire", vec![u64_arg(7)]),
        ("publish", vec![u64_arg(7), view_arg(b"elsewhere")]),
        ("barrier", vec![u64_arg(7)]),
    ] {
        let refused = run_direct("execute", function, args, &root);
        assert_eq!(
            refused["status"], "invalid_request",
            "{function}: {refused:#}"
        );
    }
    // Directories: barrier refuses, non-empty delete refuses, empty
    // delete reports kind 1.
    let dir = run_direct("execute", "make_dir", vec![view_arg(b"d")], &root);
    assert_eq!(dir["status"], "returned", "{dir:#}");
    let refused = run_direct(
        "execute",
        "barrier",
        vec![u64_arg(returned_u64(&dir))],
        &root,
    );
    assert_eq!(refused["status"], "invalid_request", "{refused:#}");
    std::fs::write(root.join("d").join("inner"), b"x").expect("fixture");
    let refused = run_direct(
        "execute",
        "retire",
        vec![u64_arg(returned_u64(&dir))],
        &root,
    );
    assert_eq!(refused["status"], "invalid_request", "{refused:#}");
    assert!(root.join("d").join("inner").exists());
    std::fs::remove_file(root.join("d").join("inner")).expect("cleanup");
    let removed = run_direct(
        "execute",
        "retire",
        vec![u64_arg(returned_u64(&dir))],
        &root,
    );
    assert_eq!(removed["status"], "returned", "{removed:#}");
    assert_eq!(returned_u64(&removed), 1);
}

/// Without a grant every mutation fails closed as Unsupported and the
/// granted tree is never touched.
#[test]
fn mutations_without_grant_fail_closed() {
    let root = fixture_workspace("ungranted");
    let (ok, result) = run_experiment("mncs-research-bytecode", &[]);
    assert!(!ok, "ungranted run must not succeed");
    for case in result["cases"].as_array().expect("cases") {
        assert_eq!(case["status"], "unsupported", "{case:#}");
        let effects = case
            .get("effects")
            .and_then(|effects| effects.as_array())
            .map(Vec::len)
            .unwrap_or(0);
        assert_eq!(effects, 0, "{case:#}");
    }
    // Fixtures byte-identical: no ungranted call mutated anything.
    assert_eq!(
        std::fs::read(root.join("aa-probe")).expect("probe intact"),
        b"PROBE123"
    );
    assert!(root.join("aa-stage").is_dir(), "stage dir intact");
    let entries: Vec<_> = std::fs::read_dir(&root).expect("read root").collect();
    assert_eq!(entries.len(), 2, "nothing created without a grant");
}

const COMPILED_BACKENDS: [&str; 4] = [
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

/// The all-effectful mutation module refuses whole-program on every
/// compiled backend with explicit host-call diagnostics; the grant is
/// presented but the destination stays untouched.
#[test]
fn compiled_backends_refuse_the_mutation_module() {
    for backend in COMPILED_BACKENDS {
        let root = workspace(backend);
        let grant = format!("fs_root={}", root.to_string_lossy());
        let (ok, result) = run_experiment(backend, &["--grant-fs".to_owned(), grant]);
        assert!(!ok, "{backend}: all-effectful module must refuse");
        let text = serde_json::to_string(&result).unwrap_or_default();
        assert!(
            text.contains("host calls are unsupported"),
            "{backend}: refusal must cite host calls: {text:.600}"
        );
        let remaining: Vec<_> = std::fs::read_dir(&root).expect("read root").collect();
        assert!(remaining.is_empty(), "{backend}: destination untouched");
    }
}
