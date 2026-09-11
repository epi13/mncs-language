//! WEB-P-012: bounded loops rebuilding large aggregate cells every trip.
//!
//! Total allocation (~50 MiB) exceeds every fixed backend arena, so only
//! loop-region reclamation keeps the peak bounded. The reclamation
//! backends (research interpreter, portable WASM) must return the exact
//! carried values; the bump-only native backends must report structured
//! `MNCS_RSRC_EXHAUSTED` budget exhaustion rather than trapping. The
//! small-footprint invariant case returns everywhere, pinning that
//! pre-loop composites read inside a loop survive the backedge reset,
//! and the nested case pins the inner/outer activation discipline.

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

fn u64_value(value: u64) -> Value {
    serde_json::json!([{"integer": {"value": value, "type": {"bits": 64, "signed": false}}}])
}

const RECLAIM_BACKENDS: [&str; 2] = ["mncs-research-bytecode", "mncs-portable-wasm-mvp"];

const NATIVE_BACKENDS: [&str; 3] = ["mncs-c11", "mncs-llvm-ir", "mncs-cranelift"];

/// Reclamation backends run the whole minimization and return the exact
/// carried values; the small invariant case returns on every backend.
#[test]
fn large_value_loops_return_where_reclamation_exists() {
    let source = example("source/pressure-loop-region.mncs");
    let corpus = example("execution/pressure-loop-region-corpus.json");
    let expectations = [
        ("carry", 1024_u64),
        ("nested", 2047_u64),
        ("invariant", 116736_u64),
        ("nested-loops", 1024_u64),
        ("boxed", 7_u64),
    ];
    for backend in RECLAIM_BACKENDS {
        let (_code, result, stderr) = run_experiment(&source, backend, &corpus);
        let cases = result["cases"].as_array().unwrap_or_else(|| {
            panic!("{backend}: missing cases; stderr={stderr} result={result:#}")
        });
        assert_eq!(cases.len(), 5, "{backend}: case count changed; {result:#}");
        for (id, expected) in expectations {
            let case = cases
                .iter()
                .find(|case| case["case_id"] == id)
                .unwrap_or_else(|| panic!("{backend}: missing case {id}"));
            assert_eq!(
                case["status_met"], true,
                "{backend} {id}: status not met; case={case:#}"
            );
            assert_eq!(case["status"], "returned", "{backend} {id}: {case:#}");
            assert_eq!(
                case["returned"],
                u64_value(expected),
                "{backend} {id}: wrong value; {case:#}"
            );
        }
    }
    // The small-footprint cases fit every arena, so they return on the
    // native backends too, with identical values: the invariant case pins
    // that pre-loop composites survive the backedge reset, and the boxed
    // case pins payload-bearing finite carriage through every lowering.
    for backend in NATIVE_BACKENDS {
        let (_code, result, stderr) = run_experiment(&source, backend, &corpus);
        let cases = result["cases"].as_array().unwrap_or_else(|| {
            panic!("{backend}: missing cases; stderr={stderr} result={result:#}")
        });
        for (id, expected) in [("invariant", 116736_u64), ("boxed", 7_u64)] {
            let case = cases
                .iter()
                .find(|case| case["case_id"] == id)
                .unwrap_or_else(|| panic!("{backend}: missing {id} case"));
            assert_eq!(
                case["status_met"], true,
                "{backend} {id}: status not met; case={case:#}"
            );
            assert_eq!(case["status"], "returned", "{backend} {id}: {case:#}");
            assert_eq!(
                case["returned"],
                u64_value(expected),
                "{backend} {id}: wrong value; {case:#}"
            );
        }
    }
}

/// Bump-only native backends report structured resource exhaustion — with
/// the stable diagnostic, never a trap — on the loops whose total
/// allocation exceeds their arena.
#[test]
fn large_value_loops_exhaust_natives_structurally() {
    let source = example("source/pressure-loop-region.mncs");
    let corpus = example("execution/pressure-loop-region-corpus.json");
    for backend in NATIVE_BACKENDS {
        let (_code, result, stderr) = run_experiment(&source, backend, &corpus);
        let cases = result["cases"].as_array().unwrap_or_else(|| {
            panic!("{backend}: missing cases; stderr={stderr} result={result:#}")
        });
        for id in ["carry", "nested", "nested-loops"] {
            let case = cases
                .iter()
                .find(|case| case["case_id"] == id)
                .unwrap_or_else(|| panic!("{backend}: missing case {id}"));
            assert_eq!(
                case["status"], "budget_exhausted",
                "{backend} {id}: expected structured exhaustion, not a trap or return; case={case:#}"
            );
            let reason = case["failure_reason"].as_str().unwrap_or("");
            assert!(
                reason.contains("MNCS_RSRC_EXHAUSTED"),
                "{backend} {id}: exhaustion lacks the stable resource diagnostic; reason={reason:#}"
            );
        }
    }
}
