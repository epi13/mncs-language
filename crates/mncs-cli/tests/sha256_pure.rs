//! Index PRESS-006: SHA-256 as pure bounded MNCS with no grant and no
//! new trusted primitive.
//!
//! `mncs.std.sha256.v1` compresses, updates over chunked views, and
//! finalizes entirely in-language on the PRESS-004 bitwise substrate.
//! These tests prove the hashlib-oracle vectors agree on every
//! executable backend and across the layered reference executors, that
//! repeated runs converge, and that different chunkings of one input
//! converge to one digest.

use std::path::PathBuf;
use std::process::Command;

use serde_json::json;
use serde_json::Value;
use sha2::{Digest, Sha256};

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

fn byte_sequence(bytes: &[u8]) -> Value {
    json!({
        "sequence": {
            "values": bytes.iter().map(|byte| json!({"byte": {"value": byte}})).collect::<Vec<_>>()
        }
    })
}

struct TemporaryCorpus(PathBuf);

impl TemporaryCorpus {
    fn write(value: &Value) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mncs-sha256-digest-{}-{nonce}.json",
            std::process::id()
        ));
        std::fs::write(
            &path,
            serde_json::to_vec(value).expect("serialize SHA-256 corpus"),
        )
        .expect("write SHA-256 corpus");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TemporaryCorpus {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
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
        assert_eq!(cases.len(), 4, "{backend}: corpus drift");
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

/// The public one-shot function handles empty input, every SHA padding edge,
/// multiple blocks, and the full application-identity bound on all executable
/// backends. Rust's `sha2` implementation is the independent oracle.
#[test]
fn sha256_bounded_digest_matches_oracle_on_executable_backends() {
    let lengths = [0usize, 1, 55, 56, 63, 64, 65, 127, 128, 129, 1024];
    let cases = lengths
        .iter()
        .map(|length| {
            let input = (0..*length)
                .map(|index| ((index * 37 + 11) % 256) as u8)
                .collect::<Vec<_>>();
            let expected = Sha256::digest(&input);
            json!({
                "id": format!("sha256-digest-{length}"),
                "request": {
                    "schema_version": "0.1",
                    "target": {
                        "module": "mncs.std.sha256.v1",
                        "function": "digest_standard"
                    },
                    "arguments": [byte_sequence(&input)],
                    "step_budget": 8_000_000
                },
                "expected": [byte_sequence(&expected)]
            })
        })
        .collect::<Vec<_>>();
    let corpus = TemporaryCorpus::write(&json!({
        "schema_version": "0.1",
        "name": "sha256-bounded-digest",
        "cases": cases
    }));

    for backend in EXECUTABLE_BACKENDS {
        let output = binary()
            .env("MNCS_LIBRARY_PATH", library_dir())
            .args([
                "experiment",
                "run",
                &source(),
                "--backend",
                backend,
                "--corpus",
                corpus.path().to_str().expect("UTF-8 temporary corpus path"),
            ])
            .output()
            .expect("run bounded SHA-256 experiment");
        assert!(
            output.status.success(),
            "{backend}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
        let observations = result["cases"].as_array().expect("case observations");
        assert_eq!(observations.len(), lengths.len(), "{backend}: corpus drift");
        let mut maximum_steps = 0;
        for observation in observations {
            assert_eq!(
                observation["status"], "returned",
                "{backend}: {observation}"
            );
            assert_eq!(
                observation["expectation_met"], true,
                "{backend}: {observation}"
            );
            let steps = observation["steps"]
                .as_u64()
                .expect("reported backend steps");
            maximum_steps = maximum_steps.max(steps);
            if backend == "mncs-research-bytecode" {
                assert!(
                    steps <= 500_000,
                    "research-bytecode resource bound: {observation}"
                );
            }
        }
        eprintln!("{backend} maximum SHA-256 digest steps: {maximum_steps}");
    }
}
