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

/// Stage F: the `mncs-stdlib` checkout backing these tests: explicit
/// `MNCS_STDLIB_ROOT` wins, else the `mncs-stdlib` sibling checkout.
/// Fails closed with a clear message when absent.
fn stdlib_checkout_dir() -> String {
    // Test inputs need a real checkout: an explicitly empty variable
    // (the CLI's hermetic spelling) falls through to the sibling here.
    let explicit = std::env::var("MNCS_STDLIB_ROOT")
        .ok()
        .filter(|root| !root.trim().is_empty());
    let checkout =
        explicit.unwrap_or_else(|| format!("{}/../../../mncs-stdlib", env!("CARGO_MANIFEST_DIR")));
    assert!(
        std::path::Path::new(&checkout).is_dir(),
        "mncs-stdlib checkout missing at {checkout}; set MNCS_STDLIB_ROOT"
    );
    checkout
}

/// Stage F: standard-library sources now live in `mncs-stdlib/library/`.
fn stdlib_library_dir() -> String {
    let checkout = stdlib_checkout_dir();
    let dir = format!("{checkout}/library");
    assert!(
        std::path::Path::new(&dir).is_dir(),
        "mncs-stdlib checkout missing at {checkout}; set MNCS_STDLIB_ROOT"
    );
    dir
}

/// Stage F: stdlib-owned examples (fixtures/corpora) read across repos.
fn stdlib_example(name: &str) -> String {
    format!("{}/examples/{name}", stdlib_checkout_dir())
}

fn library_dir() -> String {
    stdlib_library_dir()
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
    stdlib_example("execution/sha256-corpus.json")
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
