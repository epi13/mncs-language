use std::process::Command;

use serde_json::Value;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn semantic_commands_emit_deterministic_machine_json() {
    let manifest = example("account-transfer.mncs.json");
    let canonical = binary()
        .args(["canonicalize", &manifest])
        .output()
        .expect("run canonicalize");
    assert!(canonical.status.success());
    let canonical_json: Value = serde_json::from_slice(&canonical.stdout).expect("canonical JSON");
    assert_eq!(canonical_json["schema_version"], "0.2");
    assert_eq!(canonical_json["fingerprint"].as_str().unwrap().len(), 64);

    let graph = binary()
        .args(["graph", &manifest])
        .output()
        .expect("run graph");
    assert!(graph.status.success());
    let graph_json: Value = serde_json::from_slice(&graph.stdout).expect("graph JSON");
    assert!(graph_json["nodes"].as_array().unwrap().len() >= 7);
    assert!(graph_json["edges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|edge| edge["kind"] == "supports_property"));
}

#[test]
fn compiler_architecture_exposes_the_complete_stage_ladder_and_current_gaps() {
    let output = binary()
        .arg("compiler-architecture")
        .output()
        .expect("run compiler architecture");
    assert!(output.status.success());
    let architecture: Value =
        serde_json::from_slice(&output.stdout).expect("compiler architecture JSON");
    assert_eq!(
        architecture["contract_id"],
        "mncs:language:compiler-stage-architecture:0.1"
    );
    assert_eq!(architecture["stages"].as_array().unwrap().len(), 12);
    let lexical = architecture["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|stage| stage["stage"] == "lexical_analysis")
        .unwrap();
    assert_eq!(lexical["availability"], "experimental");
    assert_eq!(lexical["integration"], "compiler_driver");
    let hir = architecture["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|stage| stage["stage"] == "high_level_ir")
        .unwrap();
    assert_eq!(hir["integration"], "compiler_driver");
}

#[test]
fn portable_backend_and_translation_validation_are_explicitly_bounded() {
    let program = example("executable/checked-add.mncs.json");
    let request = example("execution/checked-add-request.json");
    let executed = binary()
        .args(["execute-backend", &program, &request])
        .output()
        .expect("run execute-backend");
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    let result: Value = serde_json::from_slice(&executed.stdout).expect("backend result");
    assert_eq!(result["status"], "returned");
    assert_eq!(result["returned"][0]["integer"]["value"], 42);

    let compiled = binary()
        .args(["compile", &program, "--target", "portable-wasm"])
        .output()
        .expect("compile portable wasm");
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let compilation: Value = serde_json::from_slice(&compiled.stdout).expect("compile JSON");
    assert!(compilation["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|artifact| artifact["representation"] == "backend_artifact"));

    let fail = binary()
        .args([
            "validate-translation",
            "checked-elision",
            &program,
            &example("execution/checked-add-corpus.json"),
        ])
        .output()
        .expect("validate checked-elision");
    let judgement: Value = serde_json::from_slice(&fail.stdout).expect("validation JSON");
    assert_eq!(judgement["judgement"], "FAIL");
    assert!(judgement["counterexample"].is_object());
}

#[test]
fn source_profile_0_2_flagship_reaches_ssa() {
    let source = example("source/flagship.mncs");
    let output = binary()
        .args(["source-study", &source, "--node-id", "flagship-test"])
        .output()
        .expect("run flagship source-study");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let study: Value = serde_json::from_slice(&output.stdout).expect("flagship study");
    assert!(study["stage_fingerprints"]
        .as_object()
        .unwrap()
        .contains_key("ssa"));
}

#[test]
fn source_study_emits_the_front_end_and_hir_stage_chain() {
    let source = example("source/identity.mncs");
    let output = binary()
        .args(["source-study", &source, "--node-id", "cli-source-test"])
        .output()
        .expect("run source study");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let study: Value = serde_json::from_slice(&output.stdout).expect("source study JSON");
    let stages = study["stage_fingerprints"].as_object().unwrap();
    for stage in [
        "source",
        "lexical_tokens",
        "concrete_syntax_tree",
        "abstract_syntax_tree",
        "semantic",
        "semantic_graph",
        "identity_map",
        "validation",
        "hir",
        "ssa",
    ] {
        assert!(stages.contains_key(stage), "missing {stage}");
    }
    assert_eq!(study["pass_executions"][0]["pass_id"], "lex-source");
    assert_eq!(
        study["interpretation"],
        "observation_only_not_assurance_or_conformance"
    );
}

#[test]
fn diff_reports_changed_identity_and_invalidated_evidence() {
    let before = example("semantic-foundation/before.mncs.json");
    let after = example("semantic-foundation/after.mncs.json");
    let output = binary()
        .args(["diff", &before, &after])
        .output()
        .expect("run diff");
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).expect("diff JSON");
    assert!(!report["semantic"]["changed"].as_array().unwrap().is_empty());
    assert_eq!(
        report["invalidation"]["invalidated_evidence"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn evidence_check_marks_saved_manifest_stale_after_change() {
    let before = example("semantic-foundation/before.mncs.json");
    let after = example("semantic-foundation/after.mncs.json");
    let manifest = binary()
        .args(["evidence-manifest", &before])
        .output()
        .expect("run evidence manifest");
    assert!(manifest.status.success());
    let path = std::env::temp_dir().join(format!("mncs-evidence-{}.json", std::process::id()));
    std::fs::write(&path, manifest.stdout).expect("write temporary evidence manifest");
    let path_string = path.to_string_lossy().into_owned();
    let check = binary()
        .args(["evidence-check", &path_string, &after])
        .output()
        .expect("run evidence check");
    let _ = std::fs::remove_file(path);
    assert!(check.status.success());
    let report: Value = serde_json::from_slice(&check.stdout).expect("evidence report JSON");
    assert_eq!(report[0]["state"], "stale");
    assert!(!report[0]["invalidated_by"].as_array().unwrap().is_empty());
}

#[test]
fn invalid_semantic_input_keeps_rejection_exit_code() {
    let invalid = example("invalid-undeclared-effect.mncs.json");
    let output = binary()
        .args(["canonicalize", &invalid])
        .output()
        .expect("run invalid canonicalize");
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).expect("validation JSON");
    assert_eq!(report["valid"], false);
}

#[test]
fn ir_obligations_verifier_and_compare_commands_are_traceable() {
    let manifest = example("account-transfer.mncs.json");
    let ir = binary().args(["ir", &manifest]).output().expect("run IR");
    assert!(ir.status.success());
    let ir_json: Value = serde_json::from_slice(&ir.stdout).expect("IR JSON");
    assert_eq!(ir_json["schema_version"], "0.3");
    assert!(!ir_json["functions"][0]["blocks"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!ir_json["trace"]["entries"].as_array().unwrap().is_empty());

    let obligations = binary()
        .args(["obligations", &manifest])
        .output()
        .expect("run obligations");
    assert!(obligations.status.success());
    let obligation_json: Value =
        serde_json::from_slice(&obligations.stdout).expect("obligations JSON");
    assert!(obligation_json["obligations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["status"] == "PASS"));

    let verified = binary()
        .args(["verify", &manifest])
        .output()
        .expect("run verifier");
    assert!(verified.status.success());
    let verified_json: Value = serde_json::from_slice(&verified.stdout).expect("verifier JSON");
    assert!(verified_json
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["status"] == "PASS"));

    let before = example("semantic-foundation/before.mncs.json");
    let after = example("semantic-foundation/after.mncs.json");
    let compare = binary()
        .args(["compare", &before, &after])
        .output()
        .expect("run compare");
    assert!(compare.status.success());
    let compare_json: Value = serde_json::from_slice(&compare.stdout).expect("compare JSON");
    assert_eq!(compare_json["authority"]["authority_broadened"], false);
    assert!(!compare_json["semantic"]["changed_contracts"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn diagnose_exposes_failed_obligation_and_preserves_invalid_exit_status() {
    let invalid = example("invalid-undeclared-effect.mncs.json");
    let output = binary()
        .args(["diagnose", &invalid])
        .output()
        .expect("run diagnose");
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).expect("diagnostic JSON");
    assert_eq!(report["obligations"][0]["category"], "authority");
    assert!(report["obligations"][0]["obligation"].is_string());
    assert_eq!(report["causal_slices"].as_array().unwrap().len(), 1);
    assert_eq!(report["causal_slices"][0]["complete"], false);
}

#[test]
fn verify_does_not_treat_unknown_evidence_as_success() {
    let manifest = example("ir/unknown-contract-evidence.mncs.json");
    let output = binary()
        .args(["verify", &manifest])
        .output()
        .expect("run unresolved verify");
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).expect("verifier JSON");
    assert_eq!(report[0]["status"], "UNKNOWN");
}

#[test]
fn executable_body_trace_and_verifier_artifact_commands_are_bound() {
    let manifest = example("executable/checked-add.mncs.json");

    let body = binary()
        .args(["body", &manifest])
        .output()
        .expect("run body");
    assert!(body.status.success());
    let body_json: Value = serde_json::from_slice(&body.stdout).expect("body JSON");
    assert_eq!(body_json["schema_version"], "0.2");
    assert_eq!(body_json["functions"].as_array().unwrap().len(), 1);

    let ir = binary()
        .args(["ir", &manifest])
        .output()
        .expect("run body IR");
    assert!(ir.status.success());
    let ir_json: Value = serde_json::from_slice(&ir.stdout).expect("body IR JSON");
    assert!(!ir_json["transformations"].as_array().unwrap().is_empty());
    assert!(ir_json["trace"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["semantic_identity"]
            .as_str()
            .is_some_and(|identity| identity.contains(":operation:"))));

    let ssa = binary()
        .args(["ssa", &manifest])
        .output()
        .expect("run body SSA");
    assert!(ssa.status.success());
    let ssa_json: Value = serde_json::from_slice(&ssa.stdout).expect("SSA JSON");
    assert_eq!(ssa_json["schema_version"], "0.4");
    assert_eq!(ssa_json["functions"].as_array().unwrap().len(), 1);
    assert!(!ssa_json["trace"]["entries"].as_array().unwrap().is_empty());

    let effect_manifest = example("executable/effectful-record.mncs.json");
    let effect_ir = binary()
        .args(["ir", &effect_manifest])
        .output()
        .expect("run effect IR");
    assert!(effect_ir.status.success());
    let effect_json: Value = serde_json::from_slice(&effect_ir.stdout).expect("effect IR JSON");
    let effect_operation = effect_json["functions"][0]["blocks"][0]["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "Effect")
        .expect("effect operation");
    assert!(effect_operation["state_region"].is_string());
    assert_eq!(
        effect_operation["capability_uses"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let graph = binary()
        .args(["graph", &manifest])
        .output()
        .expect("run body graph");
    assert!(graph.status.success());
    let graph_json: Value = serde_json::from_slice(&graph.stdout).expect("body graph JSON");
    let operation_identity = graph_json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["kind"] == "operation")
        .and_then(|node| node["identity"].as_str())
        .expect("operation node")
        .to_owned();
    let slice = binary()
        .args(["slice", &manifest, &operation_identity])
        .output()
        .expect("run slice");
    assert!(slice.status.success());
    let slice_json: Value = serde_json::from_slice(&slice.stdout).expect("slice JSON");
    assert_eq!(slice_json["root"], operation_identity);
    assert!(slice_json["nodes"].as_array().unwrap().len() > 1);

    let request = binary()
        .args(["verifier-request", &manifest])
        .output()
        .expect("run verifier request");
    assert!(request.status.success());
    let requests: Value = serde_json::from_slice(&request.stdout).expect("request JSON");
    let request_value = requests
        .as_array()
        .unwrap()
        .first()
        .expect("request")
        .clone();
    let verified = binary()
        .args(["verify", &manifest])
        .output()
        .expect("run verify");
    assert_eq!(verified.status.code(), Some(1));
    let results: Value = serde_json::from_slice(&verified.stdout).expect("result JSON");
    let result_value = results.as_array().unwrap().first().expect("result").clone();
    let unique = format!("mncs-cli-{}", std::process::id());
    let request_path = std::env::temp_dir().join(format!("{unique}-request.json"));
    let result_path = std::env::temp_dir().join(format!("{unique}-result.json"));
    std::fs::write(&request_path, serde_json::to_vec(&request_value).unwrap()).unwrap();
    std::fs::write(&result_path, serde_json::to_vec(&result_value).unwrap()).unwrap();
    let request_path_string = request_path.to_string_lossy().into_owned();
    let result_path_string = result_path.to_string_lossy().into_owned();
    let imported = binary()
        .args([
            "verify-result",
            &manifest,
            &request_path_string,
            &result_path_string,
        ])
        .output()
        .expect("run verifier result import");
    let _ = std::fs::remove_file(request_path);
    let _ = std::fs::remove_file(result_path);
    assert_eq!(imported.status.code(), Some(1));
    let imported_json: Value = serde_json::from_slice(&imported.stdout).expect("imported JSON");
    assert_eq!(imported_json["status"], "UNKNOWN");
    assert_eq!(imported_json["freshness"], "current");
}

#[test]
fn candidate_evaluation_stays_isolated_and_rejects_protected_regression() {
    let baseline = example("semantic-foundation/before.mncs.json");
    let proposal = example("refinement/contract-repair.proposal.json");
    let output = binary()
        .args(["evaluate-candidate", &baseline, &proposal])
        .output()
        .expect("run candidate evaluation");
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).expect("candidate JSON");
    assert_eq!(
        report["protected_properties"]["results"][0]["status"],
        "regressed"
    );
    assert!(!report["candidate"]["delta"]["invalidated_evidence"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn bounded_execution_and_corpus_comparison_are_explicitly_limited() {
    let baseline = example("execution/bounded-sum-baseline.mncs.json");
    let request = example("execution/bounded-sum-one-request.json");
    let executed = binary()
        .args(["execute", &baseline, &request])
        .output()
        .expect("run bounded execution");
    assert!(executed.status.success());
    let result: Value = serde_json::from_slice(&executed.stdout).expect("execution JSON");
    assert_eq!(result["schema_version"], "0.1");
    assert_eq!(result["status"], "returned");
    assert_eq!(result["returned"][0]["integer"]["value"], 1);
    assert!(!result["trace"].as_array().unwrap().is_empty());

    let executed_ssa = binary()
        .args(["execute-ssa", &baseline, &request])
        .output()
        .expect("run bounded SSA execution");
    assert!(executed_ssa.status.success());
    let ssa_result: Value =
        serde_json::from_slice(&executed_ssa.stdout).expect("SSA execution JSON");
    assert_eq!(ssa_result["schema_version"], "0.1");
    assert_eq!(ssa_result["status"], "returned");
    assert_eq!(ssa_result["returned"][0]["integer"]["value"], 1);
    assert!(ssa_result["ssa_module_identity"].is_string());
    assert!(ssa_result["hir_fingerprint"].is_string());

    let effect_manifest = example("executable/effectful-record.mncs.json");
    let effect_request = example("execution/effect-record-request.json");
    let effect = binary()
        .args(["execute", &effect_manifest, &effect_request])
        .output()
        .expect("run recorded effect");
    assert!(effect.status.success());
    let effect_result: Value = serde_json::from_slice(&effect.stdout).expect("effect JSON");
    assert_eq!(effect_result["status"], "returned");
    assert_eq!(effect_result["effects"].as_array().unwrap().len(), 1);

    let equivalent = example("execution/bounded-sum-equivalent-refactor.mncs.json");
    let regression = example("execution/bounded-sum-regression.mncs.json");
    let corpus = example("execution/bounded-sum-corpus.json");
    let lowering = binary()
        .args(["check-lowering-execution", &baseline, &corpus])
        .output()
        .expect("run body SSA conformance");
    assert!(lowering.status.success());
    let lowering_report: Value = serde_json::from_slice(&lowering.stdout).expect("lowering JSON");
    assert_eq!(lowering_report["status"], "consistent_over_corpus");
    assert_eq!(lowering_report["matching_cases"], 7);
    assert_eq!(lowering_report["mismatching_cases"], 0);

    let pass = binary()
        .args(["compare-execution", &baseline, &equivalent, &corpus])
        .output()
        .expect("run equivalent corpus comparison");
    assert!(pass.status.success());
    let pass_report: Value = serde_json::from_slice(&pass.stdout).expect("comparison JSON");
    assert_eq!(pass_report["status"], "equivalent_over_corpus");
    assert_eq!(pass_report["matching_cases"], 7);

    let diff = binary()
        .args(["diff", &baseline, &equivalent])
        .output()
        .expect("run execution fixture diff");
    assert!(diff.status.success());
    let diff_report: Value = serde_json::from_slice(&diff.stdout).expect("fixture diff JSON");
    assert!(!diff_report["semantic"]["changed"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!diff_report["invalidation"]["invalidated_evidence"]
        .as_array()
        .unwrap()
        .is_empty());

    let failed = binary()
        .args(["compare-execution", &baseline, &regression, &corpus])
        .output()
        .expect("run regression corpus comparison");
    assert_eq!(failed.status.code(), Some(1));
    let failed_report: Value = serde_json::from_slice(&failed.stdout).expect("comparison JSON");
    assert_eq!(failed_report["status"], "mismatch_detected");
    assert!(!failed_report["mismatches"].as_array().unwrap().is_empty());
}

#[test]
fn bounded_execution_exhausts_its_model_budget_without_hanging() {
    let baseline = example("execution/bounded-sum-baseline.mncs.json");
    let request = example("execution/bounded-sum-one-request.json");
    let mut request_json: Value =
        serde_json::from_slice(&std::fs::read(request).expect("read execution request"))
            .expect("request JSON");
    request_json["step_budget"] = 1.into();
    let path =
        std::env::temp_dir().join(format!("mncs-execution-budget-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(&request_json).unwrap()).expect("write request");
    let path_string = path.to_string_lossy().into_owned();
    let output = binary()
        .args(["execute", &baseline, &path_string])
        .output()
        .expect("run budgeted execution");
    let _ = std::fs::remove_file(path);
    assert_eq!(output.status.code(), Some(1));
    let result: Value = serde_json::from_slice(&output.stdout).expect("execution JSON");
    assert_eq!(result["status"], "budget_exhausted");
}

#[test]
fn compile_emits_the_existing_hir_ssa_and_evidence_chain() {
    let manifest = example("executable/checked-add.mncs.json");
    let compiled = binary()
        .args(["compile", &manifest, "--emit", "semantic,hir,ssa,evidence"])
        .output()
        .expect("run compiler");
    assert!(compiled.status.success());
    let result: Value = serde_json::from_slice(&compiled.stdout).expect("compiler JSON");
    assert_eq!(result["status"], "completed_with_unresolved_obligations");
    assert!(!result["evidence"]["unresolved_obligations"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(result["evidence"]["transformation_edges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|edge| edge["status"] == "UNKNOWN"));

    let direct_hir = binary().args(["ir", &manifest]).output().expect("run HIR");
    let direct_ssa = binary().args(["ssa", &manifest]).output().expect("run SSA");
    assert_eq!(
        result["emissions"]["hir"],
        serde_json::from_slice::<Value>(&direct_hir.stdout).unwrap()
    );
    assert_eq!(
        result["emissions"]["ssa"],
        serde_json::from_slice::<Value>(&direct_ssa.stdout).unwrap()
    );
}

#[test]
fn compile_is_deterministic_and_respects_requested_files() {
    let manifest = example("executable/checked-add.mncs.json");
    let output_dir =
        std::env::temp_dir().join(format!("mncs-compiler-output-{}", std::process::id()));
    let output_path = output_dir.to_string_lossy().into_owned();
    let first = binary()
        .args([
            "compile",
            &manifest,
            "--emit",
            "hir,ssa,evidence",
            "--output-dir",
            &output_path,
        ])
        .output()
        .expect("run first compiler invocation");
    let second = binary()
        .args(["compile", &manifest, "--emit", "hir,ssa,evidence"])
        .output()
        .expect("run second compiler invocation");
    assert!(first.status.success());
    assert!(second.status.success());
    assert_eq!(first.stdout, second.stdout);
    assert!(output_dir.join("hir.json").is_file());
    assert!(output_dir.join("ssa.json").is_file());
    assert!(output_dir.join("evidence.json").is_file());
    assert!(!output_dir.join("semantic.json").exists());
    std::fs::remove_dir_all(&output_dir).expect("remove exact temporary compiler output");
}

#[test]
fn invalid_compile_is_structured_and_never_emits_ssa() {
    let manifest = example("executable/invalid-body-capability.mncs.json");
    let output = binary()
        .args(["compile", &manifest, "--emit", "hir,ssa,evidence"])
        .output()
        .expect("run invalid compiler input");
    assert_eq!(output.status.code(), Some(1));
    let result: Value = serde_json::from_slice(&output.stdout).expect("compiler JSON");
    assert_eq!(result["status"], "failed");
    assert!(result["artifacts"].as_array().unwrap().is_empty());
    assert!(result["emissions"]["ssa"].is_null());
    assert!(result["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .all(|diagnostic| diagnostic["kind"] == "semantic_invalidity"));
}

#[test]
fn compiler_study_separates_host_observations_from_portable_fingerprints() {
    let manifest = example("execution/bounded-sum-baseline.mncs.json");
    let output = binary()
        .args(["compiler-study", &manifest, "--node-id", "test-node"])
        .output()
        .expect("run compiler study");
    assert!(output.status.success());
    let study: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
    assert_eq!(study["node"]["node_identity"], "test-node");
    assert!(study["run_identity"].is_string());
    assert!(study["compilation_request_identity"].is_string());
    for field in ["semantic_fingerprint", "hir_fingerprint", "ssa_fingerprint"] {
        assert_eq!(study[field].as_str().unwrap().len(), 64);
        assert!(study["invariants"]["expected_equal"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == field));
    }
    assert!(study["invariants"]["expected_distinct"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "compilation_request_identity"));
}
