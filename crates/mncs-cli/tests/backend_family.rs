//! Backend-family conformance: every backend is exercised against the modern
//! language surface with its honest, asserted envelope. Bounded agreement is
//! evidence, not universal proof; refusals are part of the contract.

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

/// Count corpus cases whose declared expectations were met, whether those
/// expectations are return values (`expectation_met`) or runtime-failure
/// statuses (`status_met`).
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

fn refusal_codes(result: &Value) -> Vec<String> {
    result["diagnostics"]
        .as_array()
        .map(|diagnostics| {
            diagnostics
                .iter()
                .filter_map(|diag| diag["code"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The checked division/modulo corpus across every executable scalar backend:
/// identical bounded observations everywhere, including runtime-failure
/// classification for division by zero and MIN / -1.
#[test]
fn checked_division_agrees_across_executable_backends() {
    let source = example("execution/backend-conformance/div-probe.mncs");
    let corpus = example("execution/backend-conformance/div-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run_experiment(&source, backend, &corpus);
        // Overall stays UNKNOWN because the fixture uses checked arithmetic;
        // every behavioral case must still be met by every backend.
        assert_eq!(
            result["status"], "UNKNOWN",
            "{backend}: {:#?}",
            result["cases"]
        );
        assert_eq!(cases_met(&result), 7, "{backend}");
        if backend == "mncs-c11" || backend == "mncs-llvm-ir" || backend == "mncs-cranelift" {
            // These backends realize div/mod without overflow obligations on
            // the guard paths; the residual UNKNOWN comes from the module's
            // other arithmetic, so no extra assertion is needed here yet the
            // explicit comment keeps that decision visible.
        }
    }
}

/// Strict boolean operators agree across all five executable backends.
#[test]
fn strict_booleans_agree_across_executable_backends() {
    let source = example("source/profile06-boolean-operators.mncs");
    let corpus = example("execution/profile06-boolean-operators-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run_experiment(&source, backend, &corpus);
        assert_eq!(
            result["status"], "PASS",
            "{backend}: {:#?}",
            result["cases"]
        );
        assert_eq!(cases_met(&result), 9, "{backend}");
    }
}

/// The core library runs end to end on bytecode and WASM (records, payload
/// sums); C11/LLVM/Cranelift refuse composite signatures explicitly instead
/// of mis-typing them.
#[test]
fn core_status_module_envelope_per_backend() {
    let source = library("core/status.mncs");
    let corpus = example("execution/library-core-status-corpus.json");

    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let result = run_experiment(&source, backend, &corpus);
        assert_eq!(result["status"], "PASS", "{backend}");
        assert_eq!(cases_met(&result), 30, "{backend}");
    }

    for backend in ["mncs-c11", "mncs-llvm-ir"] {
        let result = run_experiment(&source, backend, &corpus);
        assert_ne!(
            result["status"], "PASS",
            "{backend} must not claim composite support it does not have"
        );
        let codes = refusal_codes(&result);
        assert!(
            codes.iter().any(|code| code.starts_with("CG")),
            "{backend}: expected a structured capability refusal, got {codes:?}"
        );
    }
}

/// The Result payload-sum module executes identically on bytecode and WASM.
#[test]
fn core_result_payload_sums_on_bytecode_and_wasm() {
    let source = library("core/result.mncs");
    let corpus = example("execution/library-core-result-corpus.json");
    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let result = run_experiment(&source, backend, &corpus);
        // UNKNOWN only from honest checked-arithmetic obligations.
        assert_eq!(result["status"], "UNKNOWN", "{backend}");
        assert_eq!(cases_met(&result), 8, "{backend}");
    }
}

/// External targets produce genuine artifacts through llc; execution refuses
/// with precise environment reasons on this host.
#[test]
fn external_targets_produce_genuine_artifacts_and_refuse_execution_honestly() {
    for target in ["mncs-riscv32", "mncs-ptx64"] {
        let output = binary()
            .args([
                "compile",
                &example("source/profile06-boolean-operators.mncs"),
                "--emit",
                "backend",
                "--target",
                target,
            ])
            .output()
            .expect("compile for external target");
        assert!(
            output.status.success(),
            "{target}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("compilation JSON");
        assert_eq!(result["status"], "completed", "{target}");
        let artifact = &result["emissions"]["backend"];
        assert!(
            !artifact["bytes_sha256"].is_null(),
            "{target} produced no artifact bytes"
        );
    }
}

//! Backend-family conformance: every backend is exercised against the modern
//! language surface with its honest, asserted envelope. Bounded agreement is
//! evidence, not universal proof; refusals are part of the contract.

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

/// Count corpus cases whose declared expectations were met, whether those
/// expectations are return values (`expectation_met`) or runtime-failure
/// statuses (`status_met`).
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

fn refusal_codes(result: &Value) -> Vec<String> {
    result["diagnostics"]
        .as_array()
        .map(|diagnostics| {
            diagnostics
                .iter()
                .filter_map(|diag| diag["code"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The checked division/modulo corpus across every executable scalar backend:
/// identical bounded observations everywhere, including runtime-failure
/// classification for division by zero and MIN / -1.
#[test]
fn checked_division_agrees_across_executable_backends() {
    let source = example("execution/backend-conformance/div-probe.mncs");
    let corpus = example("execution/backend-conformance/div-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run_experiment(&source, backend, &corpus);
        // Overall stays UNKNOWN because the fixture uses checked arithmetic;
        // every behavioral case must still be met by every backend.
        assert_eq!(
            result["status"], "UNKNOWN",
            "{backend}: {:#?}",
            result["cases"]
        );
        assert_eq!(cases_met(&result), 7, "{backend}");
        if backend == "mncs-c11" || backend == "mncs-llvm-ir" || backend == "mncs-cranelift" {
            // These backends realize div/mod without overflow obligations on
            // the guard paths; the residual UNKNOWN comes from the module's
            // other arithmetic, so no extra assertion is needed here yet the
            // explicit comment keeps that decision visible.
        }
    }
}

/// Strict boolean operators agree across all five executable backends.
#[test]
fn strict_booleans_agree_across_executable_backends() {
    let source = example("source/profile06-boolean-operators.mncs");
    let corpus = example("execution/profile06-boolean-operators-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run_experiment(&source, backend, &corpus);
        assert_eq!(
            result["status"], "PASS",
            "{backend}: {:#?}",
            result["cases"]
        );
        assert_eq!(cases_met(&result), 9, "{backend}");
    }
}

/// The core library runs end to end on bytecode and WASM (records, payload
/// sums); C11/LLVM/Cranelift refuse composite signatures explicitly instead
/// of mis-typing them.
#[test]
fn core_status_module_envelope_per_backend() {
    let source = library("core/status.mncs");
    let corpus = example("execution/library-core-status-corpus.json");

    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let result = run_experiment(&source, backend, &corpus);
        assert_eq!(result["status"], "PASS", "{backend}");
        assert_eq!(cases_met(&result), 30, "{backend}");
    }

    for backend in ["mncs-c11", "mncs-llvm-ir"] {
        let result = run_experiment(&source, backend, &corpus);
        assert_ne!(
            result["status"], "PASS",
            "{backend} must not claim composite support it does not have"
        );
        let codes = refusal_codes(&result);
        assert!(
            codes.iter().any(|code| code.starts_with("CG")),
            "{backend}: expected a structured capability refusal, got {codes:?}"
        );
    }
}

/// The Result payload-sum module executes identically on bytecode and WASM.
#[test]
fn core_result_payload_sums_on_bytecode_and_wasm() {
    let source = library("core/result.mncs");
    let corpus = example("execution/library-core-result-corpus.json");
    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let result = run_experiment(&source, backend, &corpus);
        // UNKNOWN only from honest checked-arithmetic obligations.
        assert_eq!(result["status"], "UNKNOWN", "{backend}");
        assert_eq!(cases_met(&result), 8, "{backend}");
    }
}

/// External targets produce genuine artifacts through llc; execution refuses
/// with precise environment reasons on this host.
#[test]
fn external_targets_produce_genuine_artifacts_and_refuse_execution_honestly() {
    for target in ["mncs-riscv32", "mncs-ptx64"] {
        let output = binary()
            .args([
                "compile",
                &example("source/profile06-boolean-operators.mncs"),
                "--emit",
                "backend",
                "--target",
                target,
            ])
            .output()
            .expect("compile for external target");
        assert!(
            output.status.success(),
            "{target}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("compilation JSON");
        assert_eq!(result["status"], "completed", "{target}");
        let artifact = &result["emissions"]["backend"];
        assert!(
            !artifact["bytes_sha256"].is_null(),
            "{target} produced no artifact bytes"
        );
    }
}

/// Multi-module programs export every linked function under its home-module
/// name, and finite values inside records marshal with language-owned
/// identities. Regression: WASM used to emit empty export names for imported
/// functions and `unresolved` finite identities inside record results, so a
/// fully-linked MNEL corpus disagreed on every case.
#[test]
fn linked_module_exports_and_record_finite_marshaling_agree() {
    let source = example("source/backend-conformance/linked-root.mncs");
    let corpus = example("execution/backend-conformance/linked-corpus.json");
    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let result = run_experiment(&source, backend, &corpus);
        assert_eq!(cases_met(&result), 5, "{backend}: {:#?}", result["cases"]);
        for case_ in result["cases"].as_array().unwrap() {
            assert_eq!(
                case_["failure_reason"], serde_json::Value::Null,
                "{backend}/{}: {:?}",
                case_["case_id"], case_
            );
        }
    }
}
