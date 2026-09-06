//! Semantic-conformance CLI coverage: contract discovery, deterministic case
//! generation, reference plus backend execution, mutation rejection, and
//! corpus emission for the existing experiment machinery.
//!
//! Vision laws are intentionally not executed here: the observer pipeline is
//! too step-heavy for dev-profile `cargo test` runs. They are covered by the
//! release CI gate and by committed evidence. Image laws do run here (about
//! a minute in dev) as well as in the release gate.

use std::process::Command;

use serde_json::Value;

fn workspace(name: &str) -> String {
    format!("{}/../../{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn conformance(args: &[&str]) -> (Option<i32>, Value, String) {
    let output = binary()
        .env(
            "MNCS_LIBRARY_PATH",
            format!("{}/../../library/", env!("CARGO_MANIFEST_DIR")),
        )
        .arg("conformance")
        .args(args)
        .output()
        .expect("run mncs conformance");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let report: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("conformance stdout is JSON ({stderr}): {error}"));
    (output.status.code(), report, stderr)
}

#[test]
fn arithmetic_contracts_pass_on_every_backend() {
    let (code, report, _) = conformance(&[
        &workspace("examples/source/semantic/arithmetic_contracts.mncs"),
        "--cases",
        "4",
        "--backends",
        "mncs-portable-wasm-mvp,mncs-research-bytecode,mncs-c11,mncs-llvm-ir,mncs-cranelift",
    ]);
    assert_eq!(code, Some(0));
    assert_eq!(report["schema_version"], "mncs.conformance-report/1");
    assert_eq!(report["summary"]["fail"], 0);
    assert_eq!(report["summary"]["unknown"], 0);
    assert_eq!(report["summary"]["unsupported"], 0);
    assert!(report["summary"]["pass"].as_u64().unwrap_or(0) > 0);
    assert!(!report["subject_fingerprint"]
        .as_str()
        .unwrap_or("")
        .is_empty());
    let predicates = report["predicates"].as_array().expect("predicates");
    assert_eq!(predicates.len(), 7);
    for predicate in predicates {
        assert_eq!(predicate["status"], "tested");
        for case in predicate["cases"].as_array().expect("cases") {
            assert_eq!(case["reference"]["verdict"], "pass", "{}", case["id"]);
            assert_eq!(case["determinism_stable"], true, "{}", case["id"]);
            // Five backend observations per case, all passing.
            let backends = case["backends"].as_array().expect("backends");
            assert_eq!(backends.len(), 5, "{}", case["id"]);
            for observation in backends {
                assert_eq!(
                    observation["verdict"], "pass",
                    "{}/{}",
                    case["id"], observation["backend"]
                );
            }
        }
    }
}

#[test]
fn mutant_implementation_is_rejected_with_failure_exit() {
    let (code, report, _) = conformance(&[
        &workspace("examples/source/semantic/arithmetic_contracts_mutant.mncs"),
        "--cases",
        "4",
        "--backends",
        "mncs-portable-wasm-mvp",
    ]);
    assert_eq!(code, Some(1));
    assert!(report["summary"]["fail"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn image_laws_pass_on_portable_backends() {
    let (code, report, _) = conformance(&[
        &workspace("library/core/image.mncs"),
        "--cases",
        "2",
        "--backends",
        "mncs-portable-wasm-mvp,mncs-research-bytecode",
        "--step-budget",
        "200000",
    ]);
    assert_eq!(code, Some(0));
    assert_eq!(report["summary"]["fail"], 0);
    let predicates = report["predicates"].as_array().expect("predicates");
    assert_eq!(predicates.len(), 9);
}

#[test]
fn image_mutant_is_rejected_with_failure_exit() {
    // Mutation sensitivity for the pixel laws without the full image-suite
    // cost: one case of the self-contained broken-shift law rejects in
    // seconds under dev. The full image suite stays release-gated above.
    let (code, report, _) = conformance(&[
        &workspace("examples/source/semantic/image_mutant.mncs"),
        "--cases",
        "1",
        "--backends",
        "mncs-research-bytecode",
        "--predicate",
        "law_mutant_shift",
    ]);
    assert_eq!(code, Some(1));
    assert!(report["summary"]["fail"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn vision_mutant_is_rejected_with_failure_exit() {
    // Mutation sensitivity for the observer laws: the broken-tracker law
    // rejects in under a minute under dev. The full vision suite stays
    // release-gated (see the module header).
    let (code, report, _) = conformance(&[
        &workspace("examples/source/semantic/vision_mutant.mncs"),
        "--cases",
        "1",
        "--backends",
        "mncs-research-bytecode",
        "--predicate",
        "law_mutant_track",
    ]);
    assert_eq!(code, Some(1));
    assert!(report["summary"]["fail"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn emitted_corpus_reuses_experiment_machinery() {
    let dir = std::env::temp_dir().join(format!("mncs-conformance-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let corpus_path = dir.join("arith-corpus.json");
    let evidenced_path = dir.join("arith-evidenced.json");
    let (_code, report, _) = conformance(&[
        &workspace("examples/source/semantic/arithmetic_contracts.mncs"),
        "--cases",
        "2",
        "--backends",
        "mncs-portable-wasm-mvp",
        "--emit-corpus",
        corpus_path.to_str().expect("corpus path"),
        "--attach-evidence",
        evidenced_path.to_str().expect("evidenced path"),
    ]);
    assert_eq!(report["summary"]["fail"], 0);
    let corpus: Value =
        serde_json::from_str(&std::fs::read_to_string(&corpus_path).expect("corpus written"))
            .expect("corpus is JSON");
    assert!(!corpus["cases"].as_array().unwrap_or(&Vec::new()).is_empty());
    // The attached conformance claims are what discharge the compilation
    // `contract-evidence-bound` obligations: the experiment must run on the
    // evidenced program, not the bare source, to reach PASS honestly.
    let evidenced: Value =
        serde_json::from_str(&std::fs::read_to_string(&evidenced_path).expect("evidenced written"))
            .expect("evidenced program is JSON");
    assert!(
        evidenced["functions"]
            .as_array()
            .expect("functions")
            .iter()
            .any(|function| !function["evidence"]
                .as_array()
                .unwrap_or(&Vec::new())
                .is_empty()),
        "conformance attached no evidence"
    );
    // The emitted corpus executes under the pre-existing experiment runner
    // with every case expecting `true`.
    let output = binary()
        .env(
            "MNCS_LIBRARY_PATH",
            format!("{}/../../library/", env!("CARGO_MANIFEST_DIR")),
        )
        .args([
            "experiment",
            "run",
            evidenced_path.to_str().expect("evidenced path"),
            "--backend",
            "mncs-portable-wasm-mvp",
            "--corpus",
            corpus_path.to_str().expect("corpus path"),
            "--output-dir",
            dir.join("result").to_str().expect("result dir"),
        ])
        .output()
        .expect("run emitted corpus");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    let result: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("result").join("result.json")).expect("result written"),
    )
    .expect("result is JSON");
    assert_eq!(result["status"], "PASS", "{result:#}");
}
