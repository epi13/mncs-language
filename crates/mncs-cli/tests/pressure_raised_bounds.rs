//! ENG-PRESSURE-0004 / ENG-PRESSURE-0018: raised boundedness ceilings.
//!
//! `iterate ... up_to 32` and the flat 64-sequence ceiling forced helper
//! chaining and unnatural framebuffers. Both ceilings were policy, not
//! architecture: every backend lowers bounded iteration to a native loop
//! (C/LLVM/Cranelift emit a program-counter state machine, WASM uses real
//! `loop` opcodes), so compile-time cost is O(1) in the bound; the
//! reference interpreter retires roughly eight steps per iteration
//! (measured), so a 1024-loop costs ~8K steps against the 8M budget; and
//! 1024 keeps one i64 axis to 8 KiB on stack backends inside the 16 MiB
//! composite arena. The single named constants now read 1024, enforced at
//! the same sites, with the two-level nesting cap retained. This stress
//! pins counter loops and flat/nested sequences far past the old walls on
//! every executable backend, plus the still-closed over-bound refusal.

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

#[test]
fn raised_bounds_hold_stress_per_backend() {
    let source = example("source/pressure-bounds.mncs");
    let corpus = example("execution/pressure-bounds-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_eq!(
            code,
            Some(0),
            "{backend}: unexpected exit; stderr={stderr}; result={result:#}"
        );
        assert_eq!(result["status"], "PASS", "{backend}: overall {result:#}");
        let cases = result["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
        assert_eq!(cases.len(), 8, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
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

/// The ceilings moved, they did not vanish: 1025-element sequences and
/// `up_to 1025` loops still fail closed with the standing diagnostics.
#[test]
fn over_new_bounds_still_refused() {
    let long_literal = vec!["0"; 1025].join(", ");
    let cases = [
        (
            "sequence",
            format!(
                "mncs 0.10;\nmodule test.bounds.neg_seq;\nfn probe() -> (result: i64) {{\n    let v: [i64; 1025] = [{long_literal}];\n    return v[0];\n}}\n"
            ),
            "MNE105",
        ),
        (
            "iteration",
            "mncs 0.10;\nmodule test.bounds.neg_iter;\nfn probe() -> (result: i64) {\n    iterate i up_to 1025 carrying acc: i64 = 0 {\n        next acc = acc +% 1;\n    }\n    return acc;\n}\n"
                .to_owned(),
            "MNE142",
        ),
    ];
    for (name, text, code) in cases {
        let dir = std::env::temp_dir().join(format!("mncs-pressure-bounds-{name}"));
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
            errors.iter().any(|candidate| candidate == code),
            "{name}: expected {code} in {errors:?}"
        );
    }
}
