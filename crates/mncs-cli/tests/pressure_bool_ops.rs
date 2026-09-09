//! CP-0004: boolean equality and negation are first-class total operations.
//!
//! `bool == bool` / `bool != bool` elaborate to `BooleanCompare` and prefix
//! `!bool` to `BooleanNot` (Profile 0.13). Neither is a frontend rewrite, so
//! every executable backend observes the same operation and agrees bit for
//! bit. The corpus pins literals, arguments, precedence (`!a == b` is
//! `(!a) == b`), double negation, composition with `&&`/`||`, negation in
//! condition position (the compiler's `done = advance == false` workload),
//! and a comparison consumed as a match scrutinee. Negative cases pin the
//! fail-closed edges: mixed bool/integer equality (MNE119), ordering on
//! bools (MNE121), `!` on a non-bool (MNE181), and a dangling `!` (MNP064).

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
fn bool_equality_and_negation_agree_per_backend() {
    let source = example("source/pressure-bool-ops.mncs");
    let corpus = example("execution/pressure-bool-ops-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_eq!(
            code,
            Some(0),
            "{backend}: unexpected exit; stderr={stderr}; result={result:#}"
        );
        let overall = result["status"].as_str().unwrap_or("");
        assert!(
            overall == "PASS" || overall == "UNKNOWN",
            "{backend}: overall status {overall} is not PASS or UNKNOWN"
        );
        let cases = result["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
        assert_eq!(cases.len(), 17, "{backend}: case count");
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

/// The widened surface stays fail-closed: no implicit bool/integer
/// coercion, no ordering on bools, no negation of non-bools, and a
/// dangling `!` is a parse error rather than a silent drop.
#[test]
fn bool_operator_rejections_stay_closed() {
    let cases = [
        (
            "mixed-eq",
            "mncs 0.10;\nmodule test.bool.neg_mixed_eq;\nfn probe(a: bool, b: u64) -> (result: bool) {\n    return a == b;\n}\n",
            "MNE119",
        ),
        (
            "mixed-ne",
            "mncs 0.10;\nmodule test.bool.neg_mixed_ne;\nfn probe(a: bool, b: u64) -> (result: bool) {\n    return a != b;\n}\n",
            "MNE119",
        ),
        (
            "ordering",
            "mncs 0.10;\nmodule test.bool.neg_ordering;\nfn probe(a: bool, b: bool) -> (result: bool) {\n    return a < b;\n}\n",
            "MNE121",
        ),
        (
            "not-int",
            "mncs 0.13;\nmodule test.bool.neg_not_int;\nfn probe(a: u64) -> (result: bool) {\n    return !a;\n}\n",
            "MNE181",
        ),
        (
            "dangling",
            "mncs 0.10;\nmodule test.bool.neg_dangling;\nfn probe(a: bool) -> (result: bool) {\n    return !;\n}\n",
            "MNP064",
        ),
    ];
    for (name, text, code) in cases {
        let dir = std::env::temp_dir().join(format!("mncs-pressure-bool-{name}"));
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
