//! CP-0010: total scalar dispatch over integer subjects.
//!
//! `match` over an integer accepts integer literal arms (including negative
//! patterns on signed types) plus exactly one required `_` default arm.
//! Exhaustiveness is decided at elaboration — duplicates (MNE139), missing
//! or duplicated defaults (MNE140/MNE139), arms after the default (MNE139),
//! out-of-range literals (MNE145), and non-scalar arms on integer subjects
//! (MNE138) all fail closed — and lowering reuses the finite-match
//! branch-chain shape, so every executable backend agrees without any
//! backend-side exhaustiveness logic. Scalar patterns on finite/bool
//! subjects stay rejected (MNE138), and a variant literally named `_`
//! keeps its historical meaning on finite subjects.

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
fn scalar_match_dispatch_agrees_per_backend() {
    let source = example("source/pressure-scalar-match.mncs");
    let corpus = example("execution/pressure-scalar-match-corpus.json");
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
        assert_eq!(cases.len(), 20, "{backend}: case count");
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

/// Totality is an elaboration property: every way to be non-total or
/// ill-typed fails closed with a precise diagnostic.
#[test]
fn scalar_match_totality_stays_closed() {
    let cases = [
        (
            "dup-literal",
            "mncs 0.13;\nmodule test.sm.neg_dup;\nfn probe(x: u64) -> (result: u64) {\n    return match x { 0 => 1, 0 => 2, _ => 3 };\n}\n",
            "MNE139",
        ),
        (
            "dup-default",
            "mncs 0.13;\nmodule test.sm.neg_dup_default;\nfn probe(x: u64) -> (result: u64) {\n    return match x { 0 => 1, _ => 2, _ => 3 };\n}\n",
            "MNE139",
        ),
        (
            "after-default",
            "mncs 0.13;\nmodule test.sm.neg_after_default;\nfn probe(x: u64) -> (result: u64) {\n    return match x { _ => 2, 0 => 1 };\n}\n",
            "MNE139",
        ),
        (
            "missing-default",
            "mncs 0.13;\nmodule test.sm.neg_missing;\nfn probe(x: u64) -> (result: u64) {\n    return match x { 0 => 1, 1 => 2 };\n}\n",
            "MNE140",
        ),
        (
            "out-of-range",
            "mncs 0.13;\nmodule test.sm.neg_range;\nfn probe(x: u8) -> (result: u64) {\n    return match x { 256 => 1, _ => 0 };\n}\n",
            "MNE145",
        ),
        (
            "negative-unsigned",
            "mncs 0.13;\nmodule test.sm.neg_unsigned;\nfn probe(x: u64) -> (result: u64) {\n    return match x { -1 => 1, _ => 0 };\n}\n",
            "MNE145",
        ),
        (
            "variant-on-int",
            "mncs 0.13;\nmodule test.sm.neg_variant;\nfn probe(x: u64) -> (result: u64) {\n    return match x { Zero => 1, _ => 0 };\n}\n",
            "MNE138",
        ),
        (
            "scalar-on-finite",
            "mncs 0.13;\nmodule test.sm.neg_finite;\nenum E { A, B }\nfn probe(e: E) -> (result: u64) {\n    return match e { 0 => 1, A => 2, B => 3 };\n}\n",
            "MNE138",
        ),
        (
            "scalar-on-bool",
            "mncs 0.13;\nmodule test.sm.neg_bool;\nfn probe(b: bool) -> (result: u64) {\n    return match b { 0 => 1, true => 2, false => 3 };\n}\n",
            "MNE138",
        ),
        (
            "missing-arrow",
            "mncs 0.13;\nmodule test.sm.neg_arrow;\nfn probe(x: u64) -> (result: u64) {\n    return match x { 0 1 => 2, _ => 3 };\n}\n",
            "MNP083",
        ),
        (
            "missing-comma",
            "mncs 0.13;\nmodule test.sm.neg_comma;\nfn probe(x: u64) -> (result: u64) {\n    return match x { 0 => 1 1 => 2, _ => 3 };\n}\n",
            "MNP192",
        ),
    ];
    for (name, text, code) in cases {
        let dir = std::env::temp_dir().join(format!("mncs-pressure-smatch-{name}"));
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

/// A variant literally named `_` keeps its historical meaning on finite
/// subjects: scalar-pattern parsing must not reinterpret existing matches.
#[test]
fn underscore_variant_keeps_meaning_on_finite() {
    let dir = std::env::temp_dir().join("mncs-pressure-smatch-underscore");
    std::fs::create_dir_all(&dir).expect("create workspace");
    let path = dir.join("underscore.mncs");
    std::fs::write(
        &path,
        "mncs 0.10;\nmodule test.sm.underscore;\nenum E { _, B }\nfn probe(e: E) -> (result: u64) {\n    return match e { _ => 1, B => 0 };\n}\n",
    )
    .expect("write case");
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
        errors.is_empty(),
        "underscore variant regressed: {errors:?}"
    );
}
