//! Index PRESS-006: SHA-256 as pure bounded MNCS with no grant and no
//! new trusted primitive.
//!
//! `mncs.std.sha256.v1` compresses, updates over chunked views, and
//! finalizes entirely in-language on the PRESS-004 bitwise substrate.
//! These tests prove the hashlib-oracle vectors agree on every
//! executable backend and across the layered reference executors, that
//! repeated runs converge, and that different chunkings of one input
//! converge to one digest.

use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn example(name: &str) -> String {
    let raw = format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"));
    let mut parts: Vec<&str> = Vec::new();
    for segment in raw.split('/') {
        if segment == ".." {
            parts.pop();
        } else {
            parts.push(segment);
        }
    }
    parts.join("/")
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

fn source() -> String {
    format!("{}/std/sha256.mncs", library_dir())
}

fn corpus() -> String {
    example("execution/sha256-corpus.json")
}

fn run(backend: &str) -> Value {
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
        ])
        .output()
        .expect("run experiment");
    assert!(
        output.status.success(),
        "{backend}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("result JSON")
}

/// Oracle vectors (NIST "abc" plus multi-block inputs past the old
/// 64-byte wall in two chunkings) agree on every executable backend
/// with no grant involved.
#[test]
fn sha256_vectors_agree_on_every_executable_backend() {
    for backend in EXECUTABLE_BACKENDS {
        let result = run(backend);
        let cases = result["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 3, "{backend}: corpus drift");
        for case in cases {
            assert_eq!(case["status"], "returned", "{backend}: {case}");
            assert_eq!(case["expectation_met"], true, "{backend}: {case}");
        }
    }
}

/// The digest agrees across the layered reference executors.
#[test]
fn sha256_agrees_across_layers() {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["check-backend-execution", &source(), &corpus()])
        .output()
        .expect("run layered check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("layered JSON");
    assert_eq!(result["status"], "consistent_over_corpus");
    assert_eq!(result["mismatching_cases"], 0);
}

/// Both chunkings of the multi-block input converge to one digest, and
/// repeated runs converge.
#[test]
fn sha256_chunkings_converge_deterministically() {
    let first = run("mncs-research-bytecode");
    let second = run("mncs-research-bytecode");
    let digest_of = |result: &Value, id: &str| {
        result["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["case_id"] == id)
            .unwrap()["returned"]
            .clone()
    };
    assert_eq!(
        digest_of(&first, "sha256-blocks-64-64-28"),
        digest_of(&first, "sha256-blocks-30-63-63"),
        "chunking independence"
    );
    assert_eq!(first, second, "repeatability");
}
