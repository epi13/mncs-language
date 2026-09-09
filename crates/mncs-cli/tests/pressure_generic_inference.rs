//! ENG-PRESSURE-0019 (first slice): deterministic generic-argument
//! inference.
//!
//! Generic functions existed but every call needed explicit `<...>`
//! (`MNE220` otherwise). Inference now solves directly-constrained
//! parameters from the value arguments — `grow_fill(base, 7)` pins `W`
//! from the type of `base`, `first(42)` pins `T` — with no search:
//! constraints flow through sequence structure and direct generic
//! positions, caller-parameter forwarding infers the same
//! `ValueParam`/generic `Type` the explicit spelling produces, and every
//! parameter must end with exactly one answer. Ambiguous calls (missing or
//! conflicting constraints) keep `MNE220` with the culprits named, and
//! explicit `<...>` always remains. Generic *records* are the designed
//! follow-up, not this slice.

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
fn inferred_arguments_execute_per_backend() {
    let source = example("source/pressure-generics.mncs");
    let corpus = example("execution/pressure-generics-corpus.json");
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
        assert_eq!(cases.len(), 7, "{backend}: case count");
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

/// Ambiguity refuses usefully: unconstrained parameters are named as
/// missing, contradictory ones as conflicting, all under the standing
/// `MNE220` code. Explicit arguments still rescue every case (pinned by
/// the execution corpus above for the solvable shapes).
#[test]
fn ambiguous_inference_refuses_with_names() {
    let cases = [
        (
            "phantom",
            "mncs 0.10;\nmodule test.generics.neg_phantom;\nfn phantom<T>() -> (result: i64) {\n    return 0;\n}\nfn probe() -> (result: i64) {\n    return phantom();\n}\n",
            "cannot infer T",
        ),
        (
            "partial",
            "mncs 0.10;\nmodule test.generics.neg_partial;\nfn two<N: Nat, M: Nat>(a: [i64; N]) -> (result: i64) {\n    return a[0];\n}\nfn probe() -> (result: i64) {\n    let base: [i64; 8] = [0, 0, 0, 0, 0, 0, 0, 0];\n    return two(base);\n}\n",
            "cannot infer M",
        ),
        (
            "conflict",
            "mncs 0.10;\nmodule test.generics.neg_conflict;\nfn either<N: Nat>(a: [i64; N], b: [i64; N]) -> (result: i64) {\n    return a[0] +% b[0];\n}\nfn probe() -> (result: i64) {\n    let x: [i64; 8] = [0, 0, 0, 0, 0, 0, 0, 0];\n    let y: [i64; 4] = [0, 0, 0, 0];\n    return either(x, y);\n}\n",
            "conflicting arguments for N",
        ),
    ];
    for (name, text, fragment) in cases {
        let dir = std::env::temp_dir().join(format!("mncs-pressure-generics-{name}"));
        std::fs::create_dir_all(&dir).expect("create workspace");
        let path = dir.join(format!("{name}.mncs"));
        std::fs::write(&path, text).expect("write case");
        let output = binary()
            .args(["source-study", &path.to_string_lossy()])
            .env("MNCS_LIBRARY_PATH", library(""))
            .output()
            .expect("run source-study");
        let result: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
        let messages: Vec<String> = result["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter(|d| d["code"] == "MNE220")
            .filter_map(|d| d["message"].as_str().map(str::to_owned))
            .collect();
        assert!(
            messages.iter().any(|message| message.contains(fragment)),
            "{name}: expected MNE220 containing {fragment:?}, got {messages:?}"
        );
    }
}
