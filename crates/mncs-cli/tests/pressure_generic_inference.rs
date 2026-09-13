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
            "mncs 0.13;\nmodule test.generics.neg_phantom;\nfn phantom<T>() -> (result: i64) {\n    return 0;\n}\nfn probe() -> (result: i64) {\n    return phantom();\n}\n",
            "cannot infer T",
        ),
        (
            "partial",
            "mncs 0.13;\nmodule test.generics.neg_partial;\nfn two<N: Nat, M: Nat>(a: [i64; N]) -> (result: i64) {\n    return a[0];\n}\nfn probe() -> (result: i64) {\n    let base: [i64; 8] = [0, 0, 0, 0, 0, 0, 0, 0];\n    return two(base);\n}\n",
            "cannot infer M",
        ),
        (
            "conflict",
            "mncs 0.13;\nmodule test.generics.neg_conflict;\nfn either<N: Nat>(a: [i64; N], b: [i64; N]) -> (result: i64) {\n    return a[0] +% b[0];\n}\nfn probe() -> (result: i64) {\n    let x: [i64; 8] = [0, 0, 0, 0, 0, 0, 0, 0];\n    let y: [i64; 4] = [0, 0, 0, 0];\n    return either(x, y);\n}\n",
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

/// View capacities constrain inference exactly like exact bounds: `[T; up_to
/// N]` against `[T; up_to 4]` pins `N` to 4, and a generic caller forwards
/// its own capacity parameter. (Regression root: `first_of_generic(view)`
/// in `mncs.core.sequences.v1` refused with `MNE220` until capacities
/// constrained.)
#[test]
fn view_capacity_inference_accepts() {
    let text = "mncs 0.13;\nmodule test.generics.view_capacity;\nfn head_or<N: Nat>(view: [i64; up_to N], fallback: i64) -> (result: i64) {\n    if view.len == 0 {\n        return fallback;\n    }\n    return view[0];\n}\nfn outer<M: Nat>(view: [i64; up_to M], fallback: i64) -> (result: i64) {\n    return head_or(view, fallback);\n}\nfn ident<T>(value: T) -> (result: T) {\n    return value;\n}\nfn wrap<U>(value: U) -> (result: U) {\n    return ident(value);\n}\nfn mixed<N: Nat, M: Nat>(flex: [i64; N], fixed: [i64; M], scale: i64) -> (result: i64) {\n    return flex[0] +% fixed[0] +% scale;\n}\nfn mixed_caller<K: Nat>(flex: [i64; K], scale: i64) -> (result: i64) {\n    let fixed: [i64; 8] = [0, 0, 0, 0, 0, 0, 0, 0];\n    return mixed(flex, fixed, scale);\n}\nfn probe() -> (result: i64) {\n    let buf: [i64; 4] = [1, 2, 3, 4];\n    let view: [i64; up_to 4] = buf[0..4];\n    return head_or(view, -1) +% outer(view, -1) +% wrap(1) +% mixed_caller(buf, 2);\n}\n";
    let dir = std::env::temp_dir().join("mncs-pressure-generics-view-capacity");
    std::fs::create_dir_all(&dir).expect("create workspace");
    let path = dir.join("view_capacity.mncs");
    std::fs::write(&path, text).expect("write case");
    let output = binary()
        .args(["source-study", &path.to_string_lossy()])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run source-study");
    let result: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
    let elaboration_errors: Vec<String> = result["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|d| {
            d["code"]
                .as_str()
                .is_some_and(|code| code.starts_with("MNE"))
        })
        .filter_map(|d| d["message"].as_str().map(str::to_owned))
        .collect();
    assert!(
        elaboration_errors.is_empty(),
        "view capacity inference elaborates without errors, got {elaboration_errors:?}"
    );
}
