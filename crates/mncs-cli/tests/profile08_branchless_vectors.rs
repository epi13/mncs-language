//! Source Profile 0.8 branchless selection, semantic masks, and vectors.

use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn run_experiment(source: &str, backend: &str, corpus: &str) -> Value {
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
        .output()
        .expect("run experiment");
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("experiment JSON for {backend}: {error}"))
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

fn assert_corpus_agreement(source: &str, corpus: &str, expected_cases: usize) {
    for backend in EXECUTABLE_BACKENDS {
        let result = run_experiment(source, backend, corpus);
        let cases = result["cases"].as_array().expect("cases");
        assert_eq!(cases.len(), expected_cases, "{backend}");
        assert!(
            cases.iter().all(|case_| case_["expectation_met"] == true),
            "{backend}: {:?}",
            cases
                .iter()
                .filter(|case_| case_["expectation_met"] != true)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            result["translation_validations"][0]["judgement"], "PASS",
            "{backend}: layered body/SSA/backend agreement"
        );
    }
}

#[test]
fn scalar_branchless_selection_agrees_per_backend() {
    assert_corpus_agreement(
        &example("source/profile08-branchless.mncs"),
        &example("execution/profile08-branchless-corpus.json"),
        4,
    );
}

#[test]
fn masks_and_vectors_agree_per_backend() {
    assert_corpus_agreement(
        &example("source/profile08-vectors.mncs"),
        &example("execution/profile08-vectors-corpus.json"),
        12,
    );
}

#[test]
fn profile08_negative_fixtures_fail_closed() {
    let cases = [
        ("invalid-vector-type-in-07.mncs", "MNP163"),
        ("invalid-lane-out-of-range.mncs", "MNE207"),
        ("invalid-vector-lane-mismatch.mncs", "MNE210"),
        ("invalid-mask-lane-mismatch.mncs", "MNE214"),
        ("invalid-mask-sequence-element.mncs", "MNE105"),
    ];
    for (fixture, expected_code) in cases {
        let path = example(&format!("source/profile08/{fixture}"));
        let output = binary()
            .args(["validate", &path])
            .output()
            .expect("validate");
        let diagnostics: Value = serde_json::from_slice(&output.stdout).expect("diagnostics JSON");
        let codes = diagnostics
            .as_array()
            .expect("diagnostic list")
            .iter()
            .filter_map(|diagnostic| diagnostic["code"].as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&expected_code), "{fixture}: {codes:?}");
    }
}

#[test]
fn profile08_library_modules_validate() {
    for module in [
        "library/core/vector.mncs",
        "library/core/mask.mncs",
        "library/std/simd.mncs",
    ] {
        let path = format!("{}/../../{module}", env!("CARGO_MANIFEST_DIR"));
        let output = binary()
            .args(["validate", &path])
            .output()
            .expect("validate");
        let report: Value = serde_json::from_slice(&output.stdout).expect("validation report");
        assert_eq!(report["valid"], true, "{module}: {}", report["errors"]);
    }
}
