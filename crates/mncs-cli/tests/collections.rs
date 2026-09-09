//! Index PRESS-014/015: chunked-text cursors, deterministic sort/dedup,
//! and bounded relation closure as language-owned contracts.
//!
//! `mncs.std.chunk.v1`, `mncs.std.sort.v1`, and `mncs.std.relation.v1`
//! move line-boundary scanning, canonical ordering, and transitive
//! reachability from host control planes into total bounded MNCS
//! functions. These tests prove the witnesses agree on every executable
//! backend and across the layered reference executors, and that
//! repeated runs converge.

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

const MODULES: [(&str, &str, usize); 3] = [
    ("std/chunk.mncs", "execution/chunk-corpus.json", 3),
    ("std/sort.mncs", "execution/sort-corpus.json", 3),
    ("std/relation.mncs", "execution/relation-corpus.json", 5),
];

fn run(source: &str, corpus: &str, backend: &str) -> Value {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "experiment",
            "run",
            source,
            "--backend",
            backend,
            "--corpus",
            corpus,
        ])
        .output()
        .expect("run experiment");
    assert!(
        output.status.success(),
        "{backend} {source}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("result JSON")
}

fn assert_all_met(result: &Value, backend: &str, source: &str, expected_cases: usize) {
    let cases = result["cases"].as_array().unwrap();
    assert_eq!(
        cases.len(),
        expected_cases,
        "{backend} {source}: corpus drift"
    );
    for case in cases {
        assert_eq!(case["status"], "returned", "{backend} {source}: {case}");
        assert_eq!(case["expectation_met"], true, "{backend} {source}: {case}");
    }
}

/// Chunk cursors, sort/dedup, and relation closure agree on every
/// executable backend.
#[test]
fn collection_contracts_agree_on_every_executable_backend() {
    for (module, corpus, count) in MODULES {
        let source = format!("{}/{}", library_dir(), module);
        for backend in EXECUTABLE_BACKENDS {
            let result = run(&source, &example(corpus), backend);
            assert_all_met(&result, backend, module, count);
        }
    }
}

/// The collection contracts agree across the layered reference
/// executors.
#[test]
fn collection_contracts_agree_across_layers() {
    for (module, corpus, _) in MODULES {
        let output = binary()
            .env("MNCS_LIBRARY_PATH", library_dir())
            .args([
                "check-backend-execution",
                &format!("{}/{}", library_dir(), module),
                &example(corpus),
            ])
            .output()
            .expect("run layered check");
        assert!(
            output.status.success(),
            "{module}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("layered JSON");
        assert_eq!(result["status"], "consistent_over_corpus", "{module}");
        assert_eq!(result["mismatching_cases"], 0, "{module}");
    }
}

/// Repeated runs converge to identical witnesses.
#[test]
fn collection_witnesses_are_deterministically_repeatable() {
    for (module, corpus, _) in MODULES {
        let source = format!("{}/{}", library_dir(), module);
        let first = run(&source, &example(corpus), "mncs-research-bytecode");
        let second = run(&source, &example(corpus), "mncs-research-bytecode");
        let witness = |result: &Value| {
            result["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|case| {
                    (
                        case["case_id"].as_str().unwrap().to_owned(),
                        case["returned"].clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(witness(&first), witness(&second), "{module}");
    }
}
