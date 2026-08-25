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

#[test]
fn wasm_refuses_record_parameter_of_core_status_explicitly() {
    let (code, result, _) = run_experiment(
        &library("core/status.mncs"),
        "mncs-portable-wasm-mvp",
        &example("execution/library-core-status-corpus.json"),
    );
    assert_eq!(code, Some(1));
    assert_eq!(result["status"], "completed_with_unresolved_obligations");
    let codes: Vec<&str> = result["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|diag| diag["code"].as_str())
        .collect();
    assert!(
        codes.contains(&"CGN302"),
        "expected CGN302 refusal: {codes:?}"
    );
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
            "93813943182726e085bb361bf6de48a238dcca73b407e100d6b1da085277ed2a",
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
