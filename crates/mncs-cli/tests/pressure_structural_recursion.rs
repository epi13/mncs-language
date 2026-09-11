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
//! closed with `budget_exhausted` instead of overflowing the native
//! stack). Uniform fuel seeding (RFC 0047 §5) extends the agreement to
//! explicit budgets: see `exhaustion.mncs` with the fuel corpora below —
//! budgeted cases run on all five backends, while the over-cap case runs
//! the native three via compile plus `experiment execute` (the reference
//! interpreters recurse on the host stack and cannot hold 1025
//! debug-build activations).
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

/// Fuel exhaustion is observably identical everywhere (RFC 0047 §5
/// uniform fuel): an explicit small budget exhausts with
/// `budget_exhausted` on all five executable backends while the
/// unbudgeted control still returns, and malformed budgets (zero, above
/// the model cap) are `invalid_request` on all five — exactly like the
/// reference interpreters classify them. The corpus keeps host stacks
/// shallow so debug builds survive on every backend.
#[test]
fn budgeted_fuel_exhaustion_agrees_across_executable_backends() {
    let source = example("source/recursion-rfc/exhaustion.mncs");
    let corpus = example("execution/recursion-fuel-budget-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run_experiment(&source, backend, &corpus);
        let cases = result["cases"].as_array().expect("cases array");
        assert_eq!(cases.len(), 4, "{backend}: four fuel cases");
        let by_id = |id: &str| {
            cases
                .iter()
                .find(|case_| case_["case_id"] == id)
                .unwrap_or_else(|| panic!("{backend}: missing case {id}"))
        };
        let control = by_id("shallow-control");
        assert_eq!(control["status"], "returned", "{backend}: control returns");
        assert_eq!(control["expectation_met"], true, "{backend}: control value");
        assert_eq!(control["status_met"], true, "{backend}: control status");
        let exhausted = by_id("budget-exhaustion");
        assert_eq!(
            exhausted["status"], "budget_exhausted",
            "{backend}: budgeted fuel must exhaust, not fail generically"
        );
        assert_eq!(
            exhausted["status_met"], true,
            "{backend}: exhaustion status"
        );
        for id in ["budget-zero-rejected", "budget-over-cap-rejected"] {
            let rejected = by_id(id);
            assert_eq!(
                rejected["status"], "invalid_request",
                "{backend}: malformed budget must fail closed as invalid_request"
            );
            assert_eq!(rejected["status_met"], true, "{backend}: {id} status");
        }
    }
}

/// At the true model cap — no explicit budget, a 1025-deep runtime tree
/// built by `spine` plus one manual wrap — the native backends fail
/// closed with `budget_exhausted`, the same code the reference
/// interpreters report, instead of overflowing the native stack or
/// collapsing into `runtime_failure`. Driven via compile plus
/// `experiment execute` (no translation validation, no reference
/// control): the reference interpreters recurse on the host stack and
/// cannot hold 1025 activations in a debug build, so they are excluded
/// here by construction, not by preference. See the run evidence record.
#[test]
fn cap_exhaustion_reports_budget_exhausted_on_native_backends() {
    let source = example("source/recursion-rfc/exhaustion.mncs");
    let corpus = example("execution/recursion-fuel-cap-corpus.json");
    for backend in ["mncs-c11", "mncs-llvm-ir", "mncs-cranelift"] {
        let dir = std::env::temp_dir().join(format!(
            "mncs-fuel-cap-{}-{}",
            backend.replace("mncs-", ""),
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create output dir");
        let compiled = binary()
            .args([
                "compile",
                &source,
                "--emit",
                "backend",
                "--target",
                backend,
                "--output-dir",
            ])
            .arg(&dir)
            .output()
            .expect("compile cap fixture");
        assert!(
            compiled.status.success(),
            "{backend}: cap fixture compiles: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let artifact = dir.join("backend.json");
        assert!(artifact.exists(), "{backend}: backend.json emitted");
        let executed = binary()
            .args(["experiment", "execute"])
            .arg(&artifact)
            .arg(&corpus)
            .output()
            .expect("execute cap artifact");
        assert!(
            executed.status.success(),
            "{backend}: execute exits 0: {}",
            String::from_utf8_lossy(&executed.stderr)
        );
        let observations: Value =
            serde_json::from_slice(&executed.stdout).expect("observations JSON");
        let cases = observations.as_array().expect("observations array");
        assert_eq!(cases.len(), 1, "{backend}: one cap case");
        assert_eq!(
            cases[0]["status"], "budget_exhausted",
            "{backend}: over-cap activation must exhaust, got {:#?}",
            cases[0]
        );
        assert_eq!(cases[0]["status_met"], true, "{backend}: cap status");
        let _ = std::fs::remove_dir_all(&dir);
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
