//! RFC 0047 (structural recursion): admission plus five-backend execution.
//!
//! The four positive shapes in `examples/source/recursion-rfc/` elaborate
//! without `MNE130` on Source Profile 0.13: each recursive call consumes a
//! match-bound structural descendant of a finite first parameter, the
//! frontend records a `structural-decrease` claim per call site, and the
//! kernel re-derives the param-first projection chain. The execution probe
//! (`recursion-probe.mncs` plus `recursion-rfc-corpus.json`) pins that
//! admitted recursion actually runs — with identical values on all five
//! executable backends, where every backend enforces the same static
//! call-depth ceiling (incoming depth above `MODEL_MAX_CALL_DEPTH` fails
//! closed as a runtime failure instead of overflowing the native stack).
//!
//! The six negative shapes stay rejected with `MNE130` in every tranche:
//! numeric countdown, mutual recursion, root calls, alias calls,
//! reconstructed values, and shadow traps carry no structural evidence.

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

fn error_codes(source: &str) -> Vec<String> {
    let output = binary()
        .args(["source-study", source])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run source-study");
    let result: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
    result["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|d| d["severity"] == "error")
        .filter_map(|d| d["code"].as_str().map(str::to_owned))
        .collect()
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

/// Admitted positives: structural-decrease shapes elaborate with no cycle
/// diagnostic on Profile 0.13. `MNE130` must be absent; any other error
/// still fails the test via the exact-equality assertion below.
#[test]
fn admitted_structural_shapes_have_no_cycle_diagnostic() {
    let positives = [
        "depth",
        "single-descendant",
        "nested-descendant",
        "arg-position",
    ];
    for name in positives {
        let source = example(&format!("source/recursion-rfc/{name}.mncs"));
        let errors = error_codes(&source);
        assert!(
            !errors.iter().any(|code| code == "MNE130"),
            "{name}: admitted shape must not report MNE130, got {errors:?}"
        );
        assert_eq!(errors, Vec::<String>::new(), "{name}: unexpected errors");
    }
}

/// Admitted recursion executes identically on all five executable backends:
/// node counts, leaf-plus-accumulator counts, left-spine depth, and the
/// tag-observing counter agree everywhere, including the native JIT and
/// ahead-of-time backends that enforce call-depth fuel at the machine
/// boundary.
#[test]
fn admitted_recursion_agrees_across_executable_backends() {
    let source = example("source/recursion-rfc/recursion-probe.mncs");
    let corpus = example("execution/recursion-rfc-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run_experiment(&source, backend, &corpus);
        assert_eq!(cases_met(&result), 8, "{backend}: {:#?}", result["cases"]);
    }
}

/// Permanent negatives: shapes without structural evidence that must stay
/// rejected in every tranche (MNE130 today and after admission exists).
#[test]
fn non_structural_cycles_stay_rejected() {
    let negatives = [
        "root-call",
        "alias-call",
        "numeric-countdown",
        "mutual",
        "reconstructed",
        "shadow-trap",
    ];
    for name in negatives {
        let source = example(&format!("source/recursion-rfc/{name}.mncs"));
        let errors = error_codes(&source);
        assert!(
            errors.iter().any(|code| code == "MNE130"),
            "{name}: expected MNE130 in {errors:?}"
        );
    }
}
