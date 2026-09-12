//! WEB-P-003: bounded bulk span copy as one total operation.
//!
//! `copy_span(dst, dst_at, src, src_at, len)` produces a new sequence equal
//! to `dst` outside the destination window and to the source window inside
//! it, on every executable backend. Out-of-window positions keep the
//! destination value, overlapping windows over the same value copy safely
//! in either direction (the source is only read), and any escaping or
//! wrapping window is a deterministic runtime failure. The source may be
//! exact or a view; the destination stays exact so the result keeps a
//! static bound.

use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library(name: &str) -> String {
    format!("{}/../../library/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn run_experiment(source: &str, backend: &str, corpus: &str) -> (Option<i32>, Value, String) {
    let output = binary()
        .args([
            "experiment",
            "run",
            source,
            "--backend",
            backend,
            "--corpus",
            corpus,
        ])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run experiment");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let value: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("experiment JSON ({stderr}): {error}"));
    (output.status.code(), value, stderr)
}

fn study_diagnostics(source_text: &str) -> Vec<Value> {
    let dir = std::env::temp_dir().join(format!(
        "mncs-span-copy-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("create workspace");
    let path = dir.join("probe.mncs");
    std::fs::write(&path, source_text).expect("write case");
    let output = binary()
        .args(["source-study", &path.to_string_lossy()])
        .output()
        .expect("run source-study");
    let result: Value = serde_json::from_slice(&output.stdout).expect("front-end JSON");
    result["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

/// Value cases agree on every backend, including view sources, static
/// windows, overlapping self-copies, zero-length copies, narrow
/// integer/boolean widths, and a bit-exact float window. The byte cases
/// also read both inputs back, so a backend that mutated an input cell
/// would fail the expectation.
#[test]
fn span_copy_values_agree_on_every_backend() {
    let source = example("source/pressure-span-copy.mncs");
    let corpus = example("execution/pressure-span-copy-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_eq!(
            code,
            Some(0),
            "{backend}: unexpected exit; stderr={stderr}; result={result:#}"
        );
        let cases = result["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
        assert_eq!(cases.len(), 16, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            if id.starts_with("trap_") {
                assert_eq!(
                    case["status"], "runtime_failure",
                    "{backend} {id}: escaping window must fail deterministically; case={case:#}"
                );
            } else {
                assert_eq!(
                    case["status"], "returned",
                    "{backend} {id}: status {case:#}"
                );
                assert_eq!(
                    case["expectation_met"], true,
                    "{backend} {id}: logical value mismatch; returned={:#}",
                    case["returned"]
                );
            }
        }
    }
}

/// Elaboration refuses each malformed shape with its own diagnostic:
/// pre-0.14 profile, wrong arity, non-sequence destination, view
/// destination, non-u64 positions, non-sequence source, element mismatch,
/// and provably out-of-range literal windows.
#[test]
fn span_copy_malformed_shapes_stay_closed() {
    for (name, probe, code) in [
        (
            "profile",
            "mncs 0.13;\nmodule test.copy.profile;\nfn probe(dst: [byte; 8], src: [byte; 8]) -> (result: [byte; 8]) {\n    return copy_span(dst, 0, src, 0, 8);\n}\n",
            "MNP207",
        ),
        (
            "arity",
            "mncs 0.14;\nmodule test.copy.arity;\nfn probe(dst: [byte; 8], src: [byte; 8]) -> (result: [byte; 8]) {\n    return copy_span(dst, 0, src, 0);\n}\n",
            "MNP208",
        ),
        (
            "dst-type",
            "mncs 0.14;\nmodule test.copy.dst;\nfn probe(dst: u64, src: [byte; 8]) -> (result: u64) {\n    return copy_span(dst, 0, src, 0, 8);\n}\n",
            "MNE264",
        ),
        (
            "dst-view",
            "mncs 0.14;\nmodule test.copy.dstview;\nfn probe(dst: [byte; up_to 8], src: [byte; 8]) -> (result: [byte; 8]) {\n    let out: [byte; 8] = copy_span(dst, 0, src, 0, 8);\n    return out;\n}\n",
            "MNE265",
        ),
        (
            "offset-type",
            "mncs 0.14;\nmodule test.copy.off;\nfn probe(dst: [byte; 8], src: [byte; 8], flag: bool) -> (result: [byte; 8]) {\n    return copy_span(dst, flag, src, 0, 8);\n}\n",
            "MNE266",
        ),
        (
            "src-type",
            "mncs 0.14;\nmodule test.copy.src;\nfn probe(dst: [byte; 8], n: u64) -> (result: [byte; 8]) {\n    return copy_span(dst, 0, n, 0, 8);\n}\n",
            "MNE267",
        ),
        (
            "element",
            "mncs 0.14;\nmodule test.copy.elem;\nfn probe(dst: [byte; 8], src: [u64; 8]) -> (result: [byte; 8]) {\n    return copy_span(dst, 0, src, 0, 8);\n}\n",
            "MNE268",
        ),
        (
            "static-oob",
            "mncs 0.14;\nmodule test.copy.oob;\nfn probe(dst: [byte; 8], src: [byte; 8]) -> (result: [byte; 8]) {\n    return copy_span(dst, 6, src, 0, 4);\n}\n",
            "MNE271",
        ),
    ] {
        let diagnostics = study_diagnostics(probe);
        assert!(
            diagnostics.iter().any(|d| d["code"] == code),
            "{name}: expected {code}, got: {diagnostics:?}"
        );
    }
}
