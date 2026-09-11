//! WEB-P-006: checked view-to-view narrowing.
//!
//! A view already in hand re-satisfies a narrower same-element expectation
//! (`[E; up_to A]` to `[E; up_to B]`, `B < A`) through an explicit runtime
//! span check on every executable backend: the narrow succeeds exactly when
//! the live span fits the new capacity and fails closed otherwise. No copy
//! is materialized. Widening, element changes, and pre-0.14 profiles keep
//! refusing exactly as before.

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
        "mncs-view-narrow-{}-{:?}",
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

/// Narrowed views read the narrowed span on every backend, including
/// literal ranges, dynamic ranges, call-argument position, chained
/// narrows, return position, and traversal over the narrowed span.
/// Escaping the new capacity fails deterministically everywhere.
#[test]
fn view_narrow_values_agree_on_every_backend() {
    let source = example("source/pressure-view-narrow.mncs");
    let corpus = example("execution/pressure-view-narrow-corpus.json");
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
        assert_eq!(cases.len(), 10, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            if id.ends_with("_escape") || id == "escape" {
                assert_eq!(
                    case["status"], "runtime_failure",
                    "{backend} {id}: escaping the narrowed capacity must fail deterministically; case={case:#}"
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

/// Widening, element changes, and pre-0.14 profiles keep refusing: the
/// relaxation is exactly view-to-narrower-view of the same element.
#[test]
fn view_narrow_refusals_stay_closed() {
    for (name, probe, code) in [
        (
            "widen",
            "mncs 0.14;\nmodule test.narrow.widen;\nfn probe(wide: [byte; up_to 64]) -> (result: [byte; up_to 1024]) {\n    return wide;\n}\n",
            "MNE103",
        ),
        (
            "element",
            "mncs 0.14;\nmodule test.narrow.elem;\nfn probe(wide: [byte; up_to 1024]) -> (result: [u64; up_to 64]) {\n    let narrow: [u64; up_to 64] = wide;\n    return narrow;\n}\n",
            "MNE115",
        ),
        (
            "profile",
            "mncs 0.13;\nmodule test.narrow.profile;\nfn read(window: [byte; up_to 64]) -> (result: u64) {\n    return (window[0] as u64);\n}\nfn probe(buf: [byte; up_to 1024]) -> (result: u64) {\n    let window: [byte; up_to 64] = buf[0..64];\n    return read(window);\n}\n",
            "MNE188",
        ),
        (
            "view-widen",
            "mncs 0.14;\nmodule test.narrow.viewwiden;\nfn probe(narrow: [byte; up_to 64]) -> (result: [byte; up_to 1024]) {\n    let wide: [byte; up_to 1024] = narrow;\n    return wide;\n}\n",
            "MNE115",
        ),
    ] {
        let diagnostics = study_diagnostics(probe);
        assert!(
            diagnostics.iter().any(|d| d["code"] == code),
            "{name}: expected {code}, got: {diagnostics:?}"
        );
    }
}
