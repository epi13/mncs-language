//! STORE-P-0012 / P2-002: safe static view-capacity widening (Profile 0.15).
//!
//! A `[T; up_to M]` value satisfies a `[T; up_to N]` expectation when
//! `M <= N` with the same element type: no copy, no runtime check — the
//! descriptor is identical and only the static capacity fact widens at the
//! use site (`let` bindings, call arguments including imported-module calls
//! and generic specializations, returns in both statement and bodyless-tail
//! forms, call results, and field projections). Checked narrowing (0.14)
//! stays distinct: narrowing verifies the live span, widening never does.
//!
//! The value corpus below pins identical behavior on every executable
//! backend (lengths prove the span is untouched; the u64 element read
//! proves no re-slicing); the refusal suite pins the closed negatives.

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
        "mncs-view-widen-{}-{:?}",
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

/// Widened uses evaluate identically on every backend: argument, let,
/// statement-return, tail-return, call-result, field-projection, imported,
/// generic-specialized, chained, and non-byte-element widening.
#[test]
fn view_widening_values_agree_on_every_backend() {
    let source = example("source/pressure-view-widen.mncs");
    let corpus = example("execution/pressure-view-widen-corpus.json");
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
        assert_eq!(cases.len(), 11, "{backend}: case count");
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

/// Widening refuses exactly the non-widening shapes on 0.15: element
/// mismatches, exact/view confusion in either direction, and capacities in
/// the wrong direction (which is narrowing, not widening). Each probe must
/// fail with a type diagnostic and none may elaborate.
#[test]
fn view_widening_refusals_stay_closed() {
    for (name, probe, codes) in [
        (
            "element",
            "mncs 0.15;\nmodule test.widen.elem;\nfn take(view: [u64; up_to 64]) -> (result: u64) {\n    return view.len;\n}\nfn probe(v: [byte; up_to 8]) -> (result: u64) {\n    return take(v);\n}\n",
            vec!["MNE117", "MNE133"],
        ),
        (
            "exact-into-small-view",
            "mncs 0.15;\nmodule test.widen.exact;\nfn take(view: [byte; up_to 4]) -> (result: u64) {\n    return view.len;\n}\nfn probe(x: [byte; 8]) -> (result: u64) {\n    return take(x);\n}\n",
            vec!["MNE133"],
        ),
        (
            "view-into-exact",
            "mncs 0.15;\nmodule test.widen.intoexact;\nfn take(view: [byte; 8]) -> (result: u64) {\n    return view.len;\n}\nfn probe(v: [byte; up_to 8]) -> (result: u64) {\n    return take(v);\n}\n",
            vec!["MNE117", "MNE133"],
        ),
        // Note: `[byte; up_to 64]` into `[byte; up_to 8]` is narrowing,
        // not widening: 0.14 synthesizes a checked ViewNarrow there, so it
        // succeeds by design and is pinned by the narrowing corpus instead.
        (
            "return-element",
            "mncs 0.15;\nmodule test.widen.retelem;\nfn probe(v: [byte; up_to 8]) -> (result: [u64; up_to 64]) {\n    let w: [u64; up_to 64] = v;\n    return w;\n}\n",
            vec!["MNE115"],
        ),
    ] {
        let diagnostics = study_diagnostics(probe);
        let found: Vec<String> = diagnostics
            .iter()
            .filter_map(|diagnostic| {
                diagnostic["code"].as_str().map(str::to_owned)
            })
            .collect();
        assert!(
            codes.iter().any(|code| found.iter().any(|have| have == code)),
            "{name}: expected one of {codes:?}, got: {found:?}"
        );
    }
}

/// Below 0.15 the widening shape refuses exactly as before (MNE117 at the
/// name use, MNE133 at the call): the profile gate, not the relation,
/// decides admission.
#[test]
fn view_widening_refuses_below_0_15() {
    let probe = "mncs 0.14;\nmodule test.widen.gate;\nfn take(view: [byte; up_to 64]) -> (result: u64) {\n    return view.len;\n}\nfn probe(v: [byte; up_to 8]) -> (result: u64) {\n    return take(v);\n}\n";
    let diagnostics = study_diagnostics(probe);
    let found: Vec<String> = diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic["code"].as_str().map(str::to_owned))
        .collect();
    assert!(
        found.contains(&"MNE117".to_owned()) && found.contains(&"MNE133".to_owned()),
        "0.14 keeps both historical refusals, got: {found:?}"
    );
}
