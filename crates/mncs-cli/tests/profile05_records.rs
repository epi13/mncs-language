use std::process::Command;

use mncs_model::{execute_ssa_module, execute_with_policy, ExecutionCorpus};
use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Profile 0.5 regression: `Name {` is shared between record literals and the
/// block-taking constructs (`match` scrutinee, `if` condition, bounded
/// iteration), and declarations may be interleaved. The fixture exercises all
/// of these together.
#[test]
fn profile05_interleaved_declarations_and_block_openers_parse() {
    let source =
        std::fs::read_to_string(example("source/profile05-interleaved-declarations.mncs")).unwrap();
    let envelope = mncs_syntax::SourceEnvelope::inline(
        mncs_syntax::SourceArtifactKind::Program,
        "profile05.interleaved",
        source,
    );
    let front_end = mncs_compiler::ReferenceCompiler::default().front_end(envelope);
    assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
}

/// Record values must round-trip through the execution boundary as returned
/// results and as record-typed function arguments, and the reference body
/// interpreter and independent SSA interpreter must agree.
#[test]
fn profile05_record_values_agree_across_reference_execution_paths() {
    let source =
        std::fs::read_to_string(example("source/profile05-interleaved-declarations.mncs")).unwrap();
    let envelope = mncs_syntax::SourceEnvelope::inline(
        mncs_syntax::SourceArtifactKind::Program,
        "profile05.interleaved",
        source,
    );
    let front_end = mncs_compiler::ReferenceCompiler::default().front_end(envelope);
    let program = front_end.program.expect("valid program");
    let ssa = program.lower_to_ssa().expect("lowerable to SSA");
    let corpus: ExecutionCorpus = serde_json::from_slice(
        &std::fs::read(example("execution/profile05-interleaved-corpus.json")).unwrap(),
    )
    .expect("valid corpus");

    for case in &corpus.cases {
        let body = execute_with_policy(&program, &case.request);
        assert_eq!(
            body.status,
            mncs_model::ExecutionStatus::Returned,
            "case {}: {:?}",
            case.id,
            body.failure
        );
        let ssa_result = execute_ssa_module(&program, &ssa, &case.request);
        assert_eq!(
            ssa_result.status,
            mncs_model::ExecutionStatus::Returned,
            "case {}: {:?}",
            case.id,
            ssa_result.failure
        );
        assert_eq!(
            body.returned, ssa_result.returned,
            "case {} disagrees between reference paths",
            case.id
        );
        if let Some(expected) = &case.expected {
            assert_eq!(&body.returned, expected, "case {} mismatch", case.id);
        }
    }
}

/// The same corpus must also pass through the sealed experiment path on a
/// real backend adapter, with every expectation met.
#[test]
fn profile05_record_results_pass_backend_experiment_expectations() {
    let output = Command::new(env!("CARGO_BIN_EXE_mncs"))
        .args([
            "experiment",
            "run",
            &example("source/profile05-interleaved-declarations.mncs"),
            "--backend",
            "mncs-research-bytecode",
            "--corpus",
            &example("execution/profile05-interleaved-corpus.json"),
        ])
        .output()
        .expect("run experiment");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("experiment JSON");
    assert_eq!(result["status"], "PASS");
    assert!(result["cases"]
        .as_array()
        .unwrap()
        .iter()
        .all(|case| case["expectation_met"] == true));
}
