//! INGEST-P-005/P-006: one coherent guarded/lazy-evaluation solution.
//!
//! `match` is uniformly lazy — only the taken arm evaluates — across bool,
//! integer, and finite subjects, while `select` stays strict (both sides
//! evaluate). Bool arms used to elaborate both sides through `Select`, so a
//! trapping projection in the untaken arm fired anyway; they now dispatch
//! through the same branch-chain shape as finite/integer arms.
//!
//! The shared solution: `next acc = match live { true => update, false =>
//! acc }` makes liveness explicit and machine-enforced. Dead lanes neither
//! execute the update (no trap, no dead-lane arithmetic) nor clobber the
//! carried state, and deleting the keep arm is a compile error (MNE140):
//! the guard cannot silently disappear the way a dropped
//! `select(..., keep)` could.

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

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

/// Lazy bool branches plus guarded carried-state updates agree on every
/// executable backend. `*_escape` must trap: laziness skips untaken arms,
/// it never suppresses a taken trap.
#[test]
fn guarded_match_agrees_on_every_executable_backend() {
    let source = example("source/pressure-guarded-match.mncs");
    let corpus = example("execution/pressure-guarded-match-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_eq!(
            code,
            Some(0),
            "{backend}: corpus must execute; stderr={stderr}; result={result:#}"
        );
        let cases = result["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
        assert_eq!(cases.len(), 8, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            if id.ends_with("_escape") {
                assert_eq!(
                    case["status"], "runtime_failure",
                    "{backend} {id}: taken traps still fire; case={case:#}"
                );
            } else {
                assert_eq!(case["status"], "returned", "{backend} {id}: {case:#}");
                assert_eq!(
                    case["expectation_met"], true,
                    "{backend} {id}: value mismatch; returned={:#}",
                    case["returned"]
                );
            }
        }
    }
}

/// Deleting the keep arm from a guarded update is a compile error
/// (MNE140): exhaustiveness is what makes the guard impossible to drop
/// silently.
#[test]
fn deleted_keep_arm_is_a_compile_error() {
    let cases = [
        (
            "guard-without-keep",
            "mncs 0.13;\nmodule test.gm.nokeep;\nfn guarded(buf: [byte; 8], n: u64) -> (result: i64) {\n    let v: [byte; up_to 8] = buf[0..n];\n    iterate i over v carrying acc: i64 = 0 {\n        next acc = match i < n { true => acc + (v[i] as i64) };\n    }\n    return acc;\n}\n",
        ),
        (
            "branch-without-else",
            "mncs 0.13;\nmodule test.gm.noelse;\nfn branch(flag: bool, x: i64) -> (result: i64) {\n    return match flag { true => 10 / x };\n}\n",
        ),
    ];
    for (name, text) in cases {
        let dir = std::env::temp_dir().join(format!("mncs-pressure-gmatch-{name}"));
        std::fs::create_dir_all(&dir).expect("create workspace");
        let path = dir.join(format!("{name}.mncs"));
        std::fs::write(&path, text).expect("write case");
        let output = binary()
            .args(["source-study", &path.to_string_lossy()])
            .env("MNCS_LIBRARY_PATH", library(""))
            .output()
            .expect("run source-study");
        let result: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
        let errors: Vec<String> = result["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter(|d| d["severity"] == "error")
            .filter_map(|d| d["code"].as_str().map(str::to_owned))
            .collect();
        assert!(
            errors.iter().any(|candidate| candidate == "MNE140"),
            "{name}: expected MNE140 in {errors:?}"
        );
    }
}
