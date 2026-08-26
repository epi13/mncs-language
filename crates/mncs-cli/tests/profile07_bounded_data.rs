//! Source Profile 0.7 bounded-data substrate: differential backend agreement,
//! negative fixtures, and honest capability-envelope refusals.

use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library(name: &str) -> String {
    format!("{}/../../library/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn source_fixture(name: &str) -> String {
    format!(
        "{}/../../examples/source/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
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
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run experiment");
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("experiment JSON for {backend}: {error}"))
}

fn cases_met(result: &Value) -> usize {
    result["cases"]
        .as_array()
        .map(|cases| {
            cases
                .iter()
                .filter(|case_| case_["expectation_met"] == true || case_["status_met"] == true)
                .count()
        })
        .unwrap_or(0)
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

/// The bounded-data flagship program produces identical bounded observations
/// on all five executable backends. Overall status is UNKNOWN, honestly,
/// because runtime-checked view ranges and dynamic indexes retain unresolved
/// obligations; the backend-agreement validation itself must PASS.
#[test]
fn bounded_data_flagship_agrees_per_backend() {
    let source = example("source/profile07-bounded-data.mncs");
    let corpus = example("execution/profile07-bounded-data-corpus.json");

    for backend in EXECUTABLE_BACKENDS {
        let result = run_experiment(&source, backend, &corpus);
        assert_eq!(cases_met(&result), 5, "{backend}");
        assert_eq!(result["status"], "UNKNOWN", "{backend}");
        let validation = &result["translation_validations"][0];
        assert_eq!(
            validation["judgement"], "PASS",
            "{backend}: layered body/SSA/backend agreement"
        );
    }
}

/// The serialization consumer binds mncs.std.encoding and mncs.core.bytes
/// through elaboration-time imports and agrees across all five backends.
#[test]
fn serialization_groundwork_agrees_per_backend() {
    let source = example("source/profile07-serialization.mncs");
    let corpus = example("execution/profile07-serialization-corpus.json");

    for backend in EXECUTABLE_BACKENDS {
        let result = run_experiment(&source, backend, &corpus);
        assert_eq!(cases_met(&result), 2, "{backend}");
        let validation = &result["translation_validations"][0];
        assert_eq!(
            validation["judgement"], "PASS",
            "{backend}: layered agreement for the encoding consumer"
        );
    }
}

/// Negative fixtures fail closed at elaboration with precise codes.
#[test]
fn profile07_negative_fixtures_are_rejected_with_coded_diagnostics() {
    struct Case {
        fixture: &'static str,
        code: &'static str,
    }
    let cases = [
        Case {
            fixture: "invalid-static-index-out-of-range.mncs",
            code: "MNE192",
        },
        Case {
            fixture: "invalid-underfilled-literal.mncs",
            code: "MNE184",
        },
        Case {
            fixture: "invalid-overfilled-literal.mncs",
            code: "MNE184",
        },
        Case {
            fixture: "invalid-byte-arithmetic.mncs",
            code: "MNE191",
        },
        Case {
            fixture: "invalid-sequence-type-in-06.mncs",
            code: "MNP145",
        },
    ];
    for case in &cases {
        let path = source_fixture(&format!("profile07/{}", case.fixture));
        let output = binary()
            .args(["validate", &path])
            .output()
            .expect("validate");
        let diagnostics: Value = serde_json::from_slice(&output.stdout).expect("diagnostics JSON");
        let codes: Vec<&str> = diagnostics
            .as_array()
            .expect("diagnostics list")
            .iter()
            .filter_map(|diag| diag["code"].as_str())
            .collect();
        assert!(
            !codes.is_empty(),
            "{}: expected rejection, got none",
            case.fixture
        );
        assert!(
            codes.contains(&case.code),
            "{}: expected {}, got {codes:?}",
            case.fixture,
            case.code
        );
    }
}

/// An inverted view range elaborates (endpoints are runtime values) but fails
/// deterministically at runtime on every backend that realizes views.
#[test]
fn inverted_view_range_fails_at_runtime_consistently() {
    let source = example("source/profile07/invalid-view-range-inverted.mncs");
    let corpus = example("execution/profile07-invalid-view-range-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let result = run_experiment(&source, backend, &corpus);
        let case_status = result["cases"][0]["status"]
            .as_str()
            .unwrap_or_else(|| panic!("{backend}: missing case status"));
        assert_eq!(
            case_status, "runtime_failure",
            "{backend}: inverted range must fail deterministically"
        );
    }
}
