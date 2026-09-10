//! Index PRESS-003/007: granted-filesystem enumeration and chunked reads.
//!
//! The `fs_*` intrinsics move discovery snapshots from host code into
//! MNCS values: a program with a grant for a fixture root enumerates the
//! tree in canonical byte order and chunk-reads files by entry index,
//! with provenance carrying the snapshot identity and generation. Tests
//! prove the granted flow on the research backend, fail-closed negatives,
//! explicit refusal elsewhere, determinism across runs and creation
//! orders, empty trees, generation movement on mutation, stale-index and
//! directory/symlink refusals, and body/SSA layered agreement.

use std::process::Command;

use serde_json::{json, Value};

fn clean(path: String) -> String {
    // Lexically normalize `..` segments: some host paths reject them.
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        if segment == ".." {
            parts.pop();
        } else {
            parts.push(segment);
        }
    }
    parts.join("/")
}

fn library_dir() -> String {
    clean(format!("{}/../../library", env!("CARGO_MANIFEST_DIR")))
}

fn example(name: &str) -> String {
    clean(format!(
        "{}/../../examples/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn source() -> String {
    example("source/fs-scan.mncs")
}

fn corpus() -> String {
    example("execution/fs-scan-corpus.json")
}

fn fixture_root() -> String {
    let root = example("fs-fixture");
    // The corpus pins an empty directory entry (`empty`), which git cannot
    // track: a fresh clone lacks it, so ensure it exists before granting
    // the root. Creating an already-present empty directory is a no-op.
    let _ = std::fs::create_dir_all(format!("{root}/empty"));
    root
}

fn diagnostics(path: &str) -> Vec<String> {
    let output = binary()
        .args(["source-study", path])
        .output()
        .expect("run source-study");
    let study: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
    study["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|diag| diag["code"].as_str().map(str::to_owned))
        .collect()
}

fn u64_arg(value: u64) -> Value {
    json!({"integer": {"value": value, "type": {"bits": 64, "signed": false}}})
}

/// Fast-path single call with explicit grants (body reference executor).
fn run_execute(function: &str, args: Vec<Value>, grants: Vec<Value>) -> Value {
    use std::io::Write;
    let request = json!({
        "schema_version": "0.1",
        "target": {"module": "examples.fs_scan", "function": function},
        "arguments": args,
        "step_budget": 100000,
        "policy": {"effects": "realize"},
        "host_grants": grants,
    });
    let mut child = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["execute", &source(), "/dev/stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn execute");
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
    let output = child.wait_with_output().expect("run execute");
    // `execute` exits nonzero for non-returned statuses by design
    // (invalid_request, unsupported, ...): the status lives in the JSON.
    let result: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "{function}: no result JSON (rc={:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    result
}

fn fs_grant(root: &std::path::Path) -> Value {
    json!({"capability": "fs_root", "locator": root.to_string_lossy(), "bytes": []})
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

/// Unauthorized shapes are rejected at elaboration with dedicated codes.
#[test]
fn fs_authority_gaps_are_rejected() {
    let cases = [
        ("source/fs-invalid-missing-decl.mncs", "MNE257"),
        ("source/fs-invalid-double.mncs", "MNE258"),
        ("source/fs-invalid-read-missing.mncs", "MNE259"),
        ("source/fs-invalid-arity.mncs", "MNP205"),
        ("source/fs-invalid-profile.mncs", "MNP204"),
        ("source/fs-invalid-index-type.mncs", "MNE262"),
    ];
    for (fixture, expected_code) in cases {
        let codes = diagnostics(&example(fixture));
        assert!(
            codes.contains(&expected_code.to_owned()),
            "{fixture}: expected {expected_code} in {codes:?}"
        );
    }
}

/// The granted corpus flow returns exact values with recorded effects.
#[test]
fn granted_enumeration_matches_fixture() {
    let grant = format!("fs_root={}", fixture_root());
    let (ok, result) = run_experiment("mncs-research-bytecode", &["--grant-fs".to_owned(), grant]);
    assert!(ok, "granted run must succeed: {result}");
    let cases = result["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 19, "corpus drift");
    for case in cases {
        assert_eq!(case["status"], "returned", "{case}");
        assert_eq!(case["expectation_met"], true, "{case}");
        assert_eq!(case["effects_met"], true, "{case}");
    }
}

/// Without a grant the calls fail closed: Unsupported, never a value.
#[test]
fn fs_without_grant_fails_closed() {
    let (ok, result) = run_experiment("mncs-research-bytecode", &[]);
    assert!(!ok, "ungranted run must not succeed");
    for case in result["cases"].as_array().unwrap() {
        assert_eq!(case["status"], "unsupported", "{case}");
        let effects = case
            .get("effects")
            .and_then(|effects| effects.as_array())
            .map(Vec::len)
            .unwrap_or(0);
        assert_eq!(effects, 0, "{case}");
    }
}

/// A grant for the wrong capability, or a grant pointing at a missing
/// root, fails the call closed with InvalidRequest.
#[test]
fn fs_wrong_grant_or_missing_root_is_invalid() {
    let wrong = run_execute(
        "entry_count",
        vec![],
        vec![json!({"capability": "other", "locator": fixture_root(), "bytes": []})],
    );
    assert_eq!(wrong["status"], "invalid_request", "{wrong}");
    assert!(wrong["returned"].as_array().unwrap().is_empty());

    let missing = run_execute(
        "entry_count",
        vec![],
        vec![json!({"capability": "fs_root", "locator": "/nonexistent-mncs-fs-root", "bytes": []})],
    );
    assert_eq!(missing["status"], "invalid_request", "{missing}");
}

/// Non-realizing backends refuse filesystem calls explicitly at
/// lowering instead of faking support.
#[test]
fn unrealizing_backends_refuse_fs_explicitly() {
    for backend in [
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let grant = format!("fs_root={}", fixture_root());
        let output = binary()
            .env("MNCS_LIBRARY_PATH", library_dir())
            .args([
                "experiment",
                "run",
                &source(),
                "--backend",
                backend,
                "--corpus",
                &corpus(),
                "--grant-fs",
                &grant,
            ])
            .output()
            .expect("run experiment");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        assert!(
            stdout.contains("host calls are unsupported")
                || stderr.contains("host calls are unsupported"),
            "{backend}: refusal must be explicit: {stdout:.400} {stderr:.400}"
        );
    }
}

/// Repeated runs converge byte-exactly, including snapshot provenance.
#[test]
fn fs_listings_are_deterministically_repeatable() {
    let root = std::path::PathBuf::from(fixture_root());
    let first = run_execute("entry_name", vec![u64_arg(3)], vec![fs_grant(&root)]);
    let second = run_execute("entry_name", vec![u64_arg(3)], vec![fs_grant(&root)]);
    assert_eq!(first["status"], "returned");
    assert_eq!(first["returned"], second["returned"]);
    assert_eq!(first["effects"], second["effects"]);
}

/// Creation/enumeration order cannot leak: two trees with identical
/// content built in opposite orders list identically.
#[test]
fn fs_listing_is_independent_of_creation_order() {
    let base = std::env::temp_dir().join(format!(
        "mncs-fs-order-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let forward = base.join("forward");
    let reverse = base.join("reverse");
    let files: Vec<(&str, &[u8])> = vec![
        ("z.txt", b"zeta" as &[u8]),
        ("a.txt", b"alpha"),
        ("sub/m.txt", b"mu"),
        ("sub/a.txt", b"inner-alpha"),
    ];
    for (dir, order) in [(&forward, true), (&reverse, false)] {
        let mut items = files.clone();
        if !order {
            items.reverse();
        }
        for (rel, bytes) in &items {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).expect("mkdirs");
            std::fs::write(&path, bytes).expect("write");
        }
    }
    let list_all = |dir: &std::path::Path| -> Value {
        let grant = fs_grant(dir);
        let count = run_execute("entry_count", vec![], vec![grant.clone()]);
        assert_eq!(count["status"], "returned");
        let n = count["returned"][0]["integer"]["value"].as_u64().unwrap();
        let mut names = Vec::new();
        for index in 0..n {
            let name = run_execute("entry_name", vec![u64_arg(index)], vec![grant.clone()]);
            assert_eq!(name["status"], "returned");
            let bytes: Vec<u8> = name["returned"][0]["sequence"]["values"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| entry["byte"]["value"].as_u64().unwrap() as u8)
                .collect();
            names.push(String::from_utf8(bytes).unwrap());
        }
        json!({"count": n, "names": names})
    };
    let first = list_all(&forward);
    let second = list_all(&reverse);
    assert_eq!(first, second);
    assert_eq!(
        first["names"],
        json!(["a.txt", "sub", "sub/a.txt", "sub/m.txt", "z.txt"])
    );
}

/// An empty granted root lists zero entries.
#[test]
fn fs_empty_root_lists_zero() {
    let dir = std::env::temp_dir().join(format!(
        "mncs-fs-empty-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("mkdirs");
    let result = run_execute("entry_count", vec![], vec![fs_grant(&dir)]);
    assert_eq!(result["status"], "returned");
    assert_eq!(result["returned"][0]["integer"]["value"], 0);
}

/// Generation is stable across polls and moves on mutation: the
/// watch-hint contract.
#[test]
fn fs_generation_moves_on_mutation() {
    let dir = std::env::temp_dir().join(format!(
        "mncs-fs-gen-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("mkdirs");
    std::fs::write(dir.join("a.txt"), b"one").expect("write");
    let grant = fs_grant(&dir);
    let first = run_execute("generation", vec![], vec![grant.clone()]);
    let second = run_execute("generation", vec![], vec![grant.clone()]);
    assert_eq!(first["status"], "returned");
    assert_eq!(first["returned"], second["returned"], "quiet polls agree");
    std::fs::write(dir.join("b.txt"), b"two").expect("mutate");
    let third = run_execute("generation", vec![], vec![grant.clone()]);
    assert_eq!(third["status"], "returned");
    assert_ne!(
        first["returned"], third["returned"],
        "mutation must move the generation"
    );
}

/// Stale indexes fail closed with a re-list hint; directory reads are
/// refused; symlinks list as `other` and reads through them are refused.
#[test]
fn fs_stale_and_nonfile_reads_are_refused() {
    let root = std::path::PathBuf::from(fixture_root());
    let grant = fs_grant(&root);
    let stale = run_execute("entry_name", vec![u64_arg(99)], vec![grant.clone()]);
    assert_eq!(stale["status"], "invalid_request", "{stale}");

    // Index 1 is the `empty` directory: reads through it are refused.
    let dir_read = run_execute(
        "read_chunk",
        vec![u64_arg(1), u64_arg(0), u64_arg(64)],
        vec![grant.clone()],
    );
    assert_eq!(dir_read["status"], "invalid_request", "{dir_read}");

    #[cfg(unix)]
    {
        let dir = std::env::temp_dir().join(format!(
            "mncs-fs-link-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("mkdirs");
        std::fs::write(dir.join("real.txt"), b"real").expect("write");
        std::os::unix::fs::symlink(dir.join("real.txt"), dir.join("link.txt")).expect("symlink");
        let link_grant = fs_grant(&dir);
        let count = run_execute("entry_count", vec![], vec![link_grant.clone()]);
        assert_eq!(count["status"], "returned");
        // link.txt sorts before real.txt: kinds are other then file.
        let kinds: Vec<u64> = [0, 1]
            .iter()
            .map(|index| {
                let kind = run_execute(
                    "entry_kind",
                    vec![u64_arg(*index)],
                    vec![link_grant.clone()],
                );
                kind["returned"][0]["integer"]["value"].as_u64().unwrap()
            })
            .collect();
        assert_eq!(kinds, vec![2, 0], "symlink lists as other: {kinds:?}");
        let link_read = run_execute(
            "read_chunk",
            vec![u64_arg(0), u64_arg(0), u64_arg(64)],
            vec![link_grant],
        );
        assert_eq!(link_read["status"], "invalid_request", "{link_read}");
    }
}

/// Large files never load fully: an 8 MiB sparse file chunk-reads at
/// its tail through the bounded seek path.
#[test]
fn fs_large_file_reads_are_bounded() {
    use std::io::{Seek, SeekFrom, Write};
    let dir = std::env::temp_dir().join(format!(
        "mncs-fs-big-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("mkdirs");
    let path = dir.join("big.bin");
    let mut file = std::fs::File::create(&path).expect("create");
    file.set_len(8 * 1024 * 1024).expect("sparse");
    file.seek(SeekFrom::End(-8)).expect("seek");
    file.write_all(b"ENDMARK!").expect("marker");
    drop(file);
    let grant = fs_grant(&dir);
    let count = run_execute("entry_count", vec![], vec![grant.clone()]);
    assert_eq!(count["status"], "returned");
    assert_eq!(count["returned"][0]["integer"]["value"], 1);
    let tail = run_execute(
        "read_chunk",
        vec![u64_arg(0), u64_arg(8 * 1024 * 1024 - 8), u64_arg(64)],
        vec![grant],
    );
    assert_eq!(tail["status"], "returned", "{tail}");
    let bytes: Vec<u8> = tail["returned"][0]["sequence"]["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["byte"]["value"].as_u64().unwrap() as u8)
        .collect();
    assert_eq!(bytes, b"ENDMARK!");
}

/// Body and SSA executors agree on filesystem calls: the layered
/// reference paths share one realization.
#[test]
fn fs_body_and_ssa_layers_agree() {
    let root = std::path::PathBuf::from(fixture_root());
    let grant = format!("fs_root={}", root.to_string_lossy());
    let (_, backend_result) =
        run_experiment("mncs-research-bytecode", &["--grant-fs".to_owned(), grant]);
    assert!(backend_result["cases"].as_array().unwrap().len() == 19);
    let corpus: Value = serde_json::from_str(&std::fs::read_to_string(corpus()).expect("corpus"))
        .expect("corpus JSON");
    for case in backend_result["cases"].as_array().unwrap() {
        let id = case["case_id"].as_str().unwrap();
        let spec = corpus["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == id)
            .expect("corpus case");
        let target = &spec["request"]["target"];
        let args = spec["request"]["arguments"].as_array().unwrap().to_vec();
        let direct = run_execute(
            target["function"].as_str().unwrap(),
            args,
            vec![fs_grant(&root)],
        );
        assert_eq!(direct["status"], "returned", "{id}: {direct}");
        assert_eq!(direct["returned"], case["returned"], "{id}: layer mismatch");
    }
}
