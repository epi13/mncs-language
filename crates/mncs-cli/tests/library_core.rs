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
        .output()
        .expect("run experiment");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let value: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("experiment JSON ({stderr}): {error}"));
    (output.status.code(), value, stderr)
}

fn run_library_experiment(
    source: &str,
    backend: &str,
    corpus: &str,
) -> (Option<i32>, Value, String) {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library(""))
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
        .expect("run library experiment");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let value: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("library experiment JSON ({stderr}): {error}"));
    (output.status.code(), value, stderr)
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

fn assert_value_agreement(
    backend: &str,
    code: Option<i32>,
    result: &Value,
    stderr: &str,
    expected_len: usize,
) {
    assert_eq!(
        code,
        Some(0),
        "{backend}: unexpected exit; stderr={stderr}; result={result:#}"
    );
    let cases = result["cases"]
        .as_array()
        .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
    assert_eq!(
        cases.len(),
        expected_len,
        "{backend}: case count {} != {expected_len}",
        cases.len()
    );
    for case in cases {
        let id = case["case_id"].as_str().unwrap_or("?");
        assert_eq!(
            case["status"], "returned",
            "{backend} {id}: status {:#}",
            case["status"]
        );
        if !case["status_met"].is_null() {
            assert_eq!(
                case["status_met"], true,
                "{backend} {id}: status_met is not true: {case:#}"
            );
        }
        assert_eq!(
            case["expectation_met"], true,
            "{backend} {id}: logical value mismatch; returned={:#}",
            case["returned"]
        );
    }
    let overall = result["status"].as_str().unwrap_or("");
    assert!(
        overall == "PASS" || overall == "UNKNOWN",
        "{backend}: overall status {overall} is not PASS or UNKNOWN"
    );
}

/// Every core module must pass its bounded corpus on the research bytecode
/// realization end to end.
#[test]
fn core_modules_pass_on_research_bytecode() {
    for module in ["status", "logic", "ordering"] {
        let (_, result, _) = run_experiment(
            &library(&format!("core/{module}.mncs")),
            "mncs-research-bytecode",
            &example(&format!("execution/library-core-{module}-corpus.json")),
        );
        assert_eq!(
            result["status"], "PASS",
            "core/{module}: {:#?}",
            result["cases"]
        );
        assert!(result["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|case_| case_["expectation_met"] == true));
    }
}

/// Scalar-only core modules must also pass on portable WASM. The status
/// module carries a record-typed parameter (`StatusPair`), which is outside
/// the WASM realization envelope; it must refuse explicitly rather than
/// approximate.
#[test]
fn scalar_core_modules_pass_on_portable_wasm() {
    for module in ["logic", "ordering"] {
        let (code, result, _) = run_experiment(
            &library(&format!("core/{module}.mncs")),
            "mncs-portable-wasm-mvp",
            &example(&format!("execution/library-core-{module}-corpus.json")),
        );
        assert_eq!(code, Some(0), "core/{module} exit code");
        assert_eq!(result["status"], "PASS", "core/{module}");
    }
}

/// The WASM realization now materializes records through linear memory, so
/// the record parameter must execute and agree with the bytecode realization
/// instead of being refused.
#[test]
fn wasm_executes_core_status_record_parameter() {
    let (code, result, _) = run_experiment(
        &library("core/status.mncs"),
        "mncs-portable-wasm-mvp",
        &example("execution/library-core-status-corpus.json"),
    );
    assert_eq!(code, Some(0), "core/status on wasm");
    assert_eq!(result["status"], "PASS", "{:#?}", result["diagnostics"]);
}

#[test]
fn status_summary_exposes_bounded_counts_and_conflict() {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library(""))
        .args([
            "execute",
            &library("core/status.mncs"),
            &example("execution/library-core-status-summary-request.json"),
        ])
        .output()
        .expect("run status summary");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let result: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("status summary JSON ({stderr}): {error}"));

    assert_eq!(output.status.code(), Some(0), "{stderr}: {result:#}");
    assert_eq!(result["status"], "returned");
    let fields = result["returned"][0]["record"]["fields"]
        .as_array()
        .expect("summary record fields");
    let by_name = fields
        .iter()
        .map(|field| (field[0].as_str().expect("field name"), &field[1]))
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(
        by_name["status"]["finite"]["variant_identity"],
        "mncs:0.2:finite-variant:mncs.core.status.v1::Status::FAIL"
    );
    assert_eq!(by_name["pass_count"]["integer"]["value"], 1);
    assert_eq!(by_name["fail_count"]["integer"]["value"], 1);
    assert_eq!(by_name["unknown_count"]["integer"]["value"], 1);
    assert_eq!(by_name["observed_count"]["integer"]["value"], 3);
    assert_eq!(by_name["valid"]["boolean"]["value"], true);
}

#[test]
fn json_cursor_preserves_bounded_validity_across_executable_backends() {
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_library_experiment(
            &example("source/json-cursor-probe.mncs"),
            backend,
            &example("execution/json-cursor-corpus.json"),
        );
        assert_eq!(code, Some(0), "{backend}: json cursor exit; {stderr}");
        assert_value_agreement(backend, code, &result, &stderr, 11);
    }
}

#[test]
fn json_cursor_retains_known_keys_through_the_wider_window() {
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_library_experiment(
            &example("source/json-cursor-probe.mncs"),
            backend,
            &example("execution/json-cursor-wide-key-corpus.json"),
        );
        assert_eq!(code, Some(0), "{backend}: wide-key cursor exit; {stderr}");
        assert_value_agreement(backend, code, &result, &stderr, 1);
    }
}

/// Differential binding against the frozen RAVEL consumer snapshot: the
/// consumer's local `dominate` and the library's `dominate` must agree over
/// the full 3x3 lattice on the bytecode realization until RFC 0014 linking
/// lets consumers import `mncs.core` directly.
#[test]
fn ravel_snapshot_agrees_with_core_status_lattice_on_bytecode() {
    let (_, snapshot_result, _) = run_experiment(
        &example("consumers/ravel-core-snapshot.mncs"),
        "mncs-research-bytecode",
        &example("execution/library-core-ravel-differential-corpus.json"),
    );
    let (_, core_result, _) = run_experiment(
        &library("core/status.mncs"),
        "mncs-research-bytecode",
        &example("execution/library-core-status-corpus.json"),
    );

    for result in [&snapshot_result, &core_result] {
        assert_eq!(
            result["status"], "PASS",
            "differential participant failed: {:#?}",
            result["cases"]
        );
    }
    // Case-by-case bounded agreement of the shared lattice join.
    let mut snapshot_cases = Vec::new();
    for case_ in snapshot_result["cases"].as_array().unwrap() {
        if case_["case_id"]
            .as_str()
            .unwrap()
            .starts_with("ravel-dominate-")
        {
            snapshot_cases.push((
                case_["case_id"].as_str().unwrap().to_owned(),
                case_["returned"][0]["finite"]["discriminant"]
                    .as_i64()
                    .unwrap(),
            ));
        }
    }
    let mut core_cases = Vec::new();
    for case_ in core_result["cases"].as_array().unwrap() {
        if case_["case_id"].as_str().unwrap().starts_with("dominate-") {
            core_cases.push((
                format!("ravel-{}", case_["case_id"].as_str().unwrap()),
                case_["returned"][0]["finite"]["discriminant"]
                    .as_i64()
                    .unwrap(),
            ));
        }
    }
    assert_eq!(snapshot_cases.len(), 9);
    assert_eq!(snapshot_cases, core_cases);
}

/// The frozen consumer snapshot is a witness of what RAVEL ships: its
/// canonical form must stay identical to the upstream file's canonical form
/// (fingerprint computed at tranche time from both files).
#[test]
fn ravel_snapshot_is_canonically_identical_to_upstream() {
    let mut paths = vec![example("consumers/ravel-core-snapshot.mncs")];
    // When the sibling checkout is available, compare against it directly.
    if let Ok(upstream) = std::env::var("RAVEL_UPSTREAM_CORE") {
        paths.push(upstream);
    }
    for path in paths {
        let output = binary()
            .args(["canonicalize", &path])
            .output()
            .expect("canonicalize");
        let value: Value = serde_json::from_slice(&output.stdout).expect("canonical JSON");
        assert_eq!(
            value["fingerprint"],
            "4bfaff838654b6a4264f6145900c9684b106e1ce6cc64ce297c53bb4a04a5370",
            "{path}"
        );
    }
}

/// The negative mutant launders UNKNOWN into PASS; the corpus must catch it
/// on exactly that lattice cell and fail the experiment overall.
#[test]
fn wrong_status_mutant_fails_on_the_unknown_unknown_cell() {
    let (code, result, _) = run_experiment(
        &example("source/library-core-status-wrong.mncs"),
        "mncs-research-bytecode",
        &example("execution/library-core-status-wrong-corpus.json"),
    );
    assert_eq!(code, Some(1));
    assert_eq!(result["status"], "FAIL");
    let failed: Vec<&str> = result["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case_| case_["expectation_met"] == false)
        .filter_map(|case_| case_["case_id"].as_str())
        .collect();
    assert_eq!(failed, vec!["dominate-unknown-unknown"]);
}

/// mncs.core.result: the standard Result shape with real reason payloads must
/// execute end to end on the bytecode realization; the overall status stays
/// UNKNOWN because the fixture uses checked arithmetic (honest obligations).
#[test]
fn core_result_module_executes_payload_sums_on_bytecode() {
    let (_, result, _) = run_experiment(
        &library("core/result.mncs"),
        "mncs-research-bytecode",
        &example("execution/library-core-result-corpus.json"),
    );
    assert_eq!(result["status"], "UNKNOWN", "{:#?}", result["cases"]);
    let cases = result["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    assert!(cases
        .iter()
        .all(|case_| case_["status"] == "returned" && case_["expectation_met"] == true));
}

/// Sequence-typed and view-typed library signatures execute as language-level
/// exports on every executable backend. This is the process-boundary ABI,
/// not the older scalar-wrapper workaround.
#[test]
fn sequence_typed_library_exports_agree_per_backend() {
    let source = library("core/sequences.mncs");
    let corpus = example("execution/library-core-sequences-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_value_agreement(backend, code, &result, &stderr, 9);
    }
}

/// Canonical encoding entry points return and accept `[byte; N]` across
/// every executable backend.
#[test]
fn encoding_sequence_results_agree_per_backend() {
    let source = library("std/encoding.mncs");
    let corpus = example("execution/library-std-encoding-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_value_agreement(backend, code, &result, &stderr, 4);
    }
}

/// Vector-typed library signatures cross as canonical cells, including a
/// vector result reconstructed from the arena image.
#[test]
fn vector_typed_library_exports_agree_per_backend() {
    let source = library("core/vector.mncs");
    let corpus = example("execution/library-core-vector-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_value_agreement(backend, code, &result, &stderr, 2);
    }
}
