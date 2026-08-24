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
fn language_experiment_runs_two_backend_adapters_and_compares_semantics() {
    let source = example("source/bounded-min.mncs");
    let corpus = example("execution/bounded-min-corpus.json");
    let temp = std::env::temp_dir().join(format!("mncs-experiment-test-{}", std::process::id()));
    let wasm_dir = temp.join("wasm");
    let bytecode_dir = temp.join("bytecode");
    for (backend, output_dir) in [
        ("portable-wasm", &wasm_dir),
        ("research-bytecode", &bytecode_dir),
    ] {
        let output = binary()
            .args([
                "experiment",
                "run",
                &source,
                "--backend",
                backend,
                "--corpus",
                &corpus,
                "--output-dir",
                output_dir.to_str().unwrap(),
            ])
            .output()
            .expect("run language experiment");
        assert!(
            output.status.success(),
            "{}: {}",
            backend,
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("experiment JSON");
        assert_eq!(result["status"], "UNKNOWN");
        assert_eq!(result["translation_validations"][0]["judgement"], "PASS");
        assert_eq!(result["cases"].as_array().unwrap().len(), 3);
        assert_eq!(
            result["definition"]["realization"]["acceptable_backends"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
    let comparison = binary()
        .args([
            "experiment",
            "compare",
            wasm_dir.join("result.json").to_str().unwrap(),
            bytecode_dir.join("result.json").to_str().unwrap(),
        ])
        .output()
        .expect("compare backend experiments");
    assert!(comparison.status.success());
    let report: Value = serde_json::from_slice(&comparison.stdout).expect("comparison JSON");
    assert_eq!(report["same_semantics"], true);
    assert_eq!(report["same_ssa"], true);
    assert_eq!(report["same_backend"], false);
    assert_eq!(report["bounded_behavior_agrees"], true);
    let frozen = binary()
        .args([
            "experiment",
            "execute",
            bytecode_dir.join("backend-artifact.json").to_str().unwrap(),
            &corpus,
        ])
        .output()
        .expect("execute frozen backend artifact");
    assert!(frozen.status.success());
    let observations: Value =
        serde_json::from_slice(&frozen.stdout).expect("frozen observations JSON");
    assert_eq!(observations.as_array().unwrap().len(), 3);
    let _ = std::fs::remove_dir_all(temp);
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

#[test]
fn compiler_study_exposes_a_family_record_reference_without_claiming_success() {
    let manifest = example("execution/bounded-sum-baseline.mncs.json");
    let output = binary()
        .args([
            "compiler-study",
            &manifest,
            "--node-id",
            "test-node",
            "--family-reference",
        ])
        .output()
        .expect("run compiler family reference");
    assert!(output.status.success());
    let reference: Value = serde_json::from_slice(&output.stdout).expect("family reference JSON");
    assert_eq!(reference["producer"], "mncs-language");
    assert_eq!(reference["recordKind"], "CompilationStudyResult");
    assert_eq!(reference["contentDigest"].as_str().unwrap().len(), 64);
    assert!(reference["compiler"]["selectedSsaIdentity"].is_string());
    assert!(reference["authorityBoundary"]
        .as_str()
        .unwrap()
        .contains("does not certify experiment success"));
}

#[test]
fn cre1_profile_runs_exact_finite_properties_through_both_backends() {
    let source = example("source/cre1-evidence-combine.mncs");
    let corpus = example("execution/cre1-evidence-corpus.json");
    let mut results = Vec::new();
    for backend in ["mncs-portable-wasm-mvp", "mncs-research-bytecode"] {
        let output = binary()
            .args([
                "experiment",
                "run",
                &source,
                "--backend",
                backend,
                "--corpus",
                &corpus,
            ])
            .output()
            .expect("run CRE-1 experiment");
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
            .all(|case_| case_["expectation_met"] == true));
        assert!(result["properties"]
            .as_array()
            .unwrap()
            .iter()
            .all(|property| property["passed"] == true));
        results.push(result);
    }
    assert_eq!(
        results[0]["compiler_study"]["semantic_fingerprint"],
        results[1]["compiler_study"]["semantic_fingerprint"]
    );
    assert_eq!(
        results[0]["compiler_study"]["ssa_fingerprint"],
        results[1]["compiler_study"]["ssa_fingerprint"]
    );
}

#[test]
fn cre1_wrong_candidate_retains_exact_counterexamples() {
    let source = example("source/cre1-evidence-combine-wrong.mncs");
    let corpus = example("execution/cre1-evidence-corpus.json");
    let output = binary()
        .args([
            "experiment",
            "run",
            &source,
            "--backend",
            "mncs-research-bytecode",
            "--corpus",
            &corpus,
        ])
        .output()
        .expect("run incorrect CRE-1 candidate");
    assert_eq!(output.status.code(), Some(1));
    let result: Value = serde_json::from_slice(&output.stdout).expect("experiment JSON");
    assert_eq!(result["status"], "FAIL");
    assert!(result["cases"]
        .as_array()
        .unwrap()
        .iter()
        .any(|case_| case_["case_id"] == "unknown-unknown" && case_["expectation_met"] == false));
    assert!(result["properties"]
        .as_array()
        .unwrap()
        .iter()
        .any(|property| property["property_id"] == "idempotence"
            && property["passed"] == false
            && property["counterexample"].is_array()));
}

#[test]
fn profile_03_rejects_non_exhaustive_matches_and_authority_laundering() {
    for (fixture, code) in [
        ("source/cre1-non-exhaustive.mncs", "MNE140"),
        ("source/cre2-undeclared-capability.mncs", "MNE111"),
        ("source/cre2-authority-laundering.mncs", "MNE134"),
        ("source/profile03-recursive-call.mncs", "MNE130"),
    ] {
        let source = example(fixture);
        let output = binary()
            .args(["source-study", &source])
            .output()
            .expect("run negative Source Profile 0.3 fixture");
        assert_eq!(output.status.code(), Some(1));
        let result: Value = serde_json::from_slice(&output.stdout).expect("front-end JSON");
        assert!(result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == code));
    }
}

#[test]
fn cre3_profile_04_runs_typed_bounded_observations_through_both_backends() {
    let source = example("source/cre3-retry-authority.mncs");
    let corpus = example("execution/cre3-retry-authority-corpus.json");
    let mut results = Vec::new();
    for backend in ["mncs-portable-wasm-mvp", "mncs-research-bytecode"] {
        let output = binary()
            .args([
                "experiment",
                "run",
                &source,
                "--backend",
                backend,
                "--corpus",
                &corpus,
            ])
            .output()
            .expect("run CRE-3 candidate");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("CRE-3 result JSON");
        let typed: mncs_model::LanguageExperimentResult =
            serde_json::from_slice(&output.stdout).expect("typed CRE-3 result");
        let family = typed.family_reference();
        assert_eq!(family.producer, "mncs-language");
        assert_eq!(family.stable_id, typed.identity.0);
        assert_eq!(family.experiment.experiment_result_identity, typed.identity);
        assert!(family
            .authority_boundary
            .contains("does not claim universal equivalence"));
        assert_eq!(result["status"], "PASS");
        assert!(result["compiler_study"]["unresolved_obligations"]
            .as_array()
            .is_some_and(|items| items.len() == 1));
        assert!(result["cases"].as_array().unwrap().iter().all(|case_| {
            case_["expectation_met"] == true
                && case_["status_met"] == true
                && case_["step_bound_met"] == true
                && case_["effects_met"] == true
        }));
        results.push(result);
    }
    assert_eq!(
        results[0]["compiler_study"]["semantic_fingerprint"],
        results[1]["compiler_study"]["semantic_fingerprint"]
    );
    assert_eq!(
        results[0]["compiler_study"]["hir_fingerprint"],
        results[1]["compiler_study"]["hir_fingerprint"]
    );
    assert_eq!(
        results[0]["compiler_study"]["ssa_fingerprint"],
        results[1]["compiler_study"]["ssa_fingerprint"]
    );
    assert_ne!(results[0]["backend"], results[1]["backend"]);
    assert_ne!(
        results[0]["artifact"]["identity"],
        results[1]["artifact"]["identity"]
    );
}

#[test]
fn cre3_body_ssa_and_each_backend_agree_over_the_typed_corpus() {
    let source = std::fs::read_to_string(example("source/cre3-retry-authority.mncs"))
        .expect("read CRE-3 source");
    let envelope = mncs_syntax::SourceEnvelope::inline(
        mncs_syntax::SourceArtifactKind::Program,
        "examples.cre3.layered-execution",
        source,
    );
    let front_end = mncs_compiler::ReferenceCompiler::default().front_end(envelope);
    assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
    let program = front_end.program.expect("elaborated CRE-3 program");
    let ssa = program.lower_to_ssa().expect("selected CRE-3 SSA");
    let corpus: mncs_model::ExecutionCorpus = serde_json::from_slice(
        &std::fs::read(example("execution/cre3-retry-authority-corpus.json"))
            .expect("read CRE-3 corpus"),
    )
    .expect("typed CRE-3 corpus");
    let body_ssa = mncs_model::compare_body_and_ssa(&program, &corpus);
    assert_eq!(
        body_ssa.status,
        mncs_model::LoweringExecutionStatus::ConsistentOverCorpus
    );
    assert_eq!(body_ssa.matching_cases, corpus.cases.len());
    let selected = mncs_codegen::selected_ssa_ref(&ssa);

    for backend_name in ["mncs-portable-wasm-mvp", "mncs-research-bytecode"] {
        let adapter = mncs_codegen::backend_adapter(backend_name).expect("backend adapter");
        let plan = adapter.plan(selected.clone());
        let artifact = adapter
            .lower(&program, &ssa, selected.clone(), &plan)
            .artifact
            .expect("backend artifact");
        let comparison =
            mncs_codegen::compare_body_ssa_and_backend(&program, &ssa, &artifact, &corpus);
        assert_eq!(
            comparison.status,
            mncs_codegen::LayeredExecutionStatus::ConsistentOverCorpus,
            "{backend_name}: {comparison:#?}"
        );
        assert_eq!(comparison.matching_cases, corpus.cases.len());
        assert_eq!(comparison.mismatching_cases, 0);
    }
}

#[test]
fn cre3_mutants_retain_bounded_counterexamples() {
    let corpus = example("execution/cre3-retry-authority-corpus.json");
    for source in [
        "source/cre3-retry-authority-wrong-unknown.mncs",
        "source/cre3-retry-authority-wrong-bound.mncs",
    ] {
        let source = example(source);
        let output = binary()
            .args([
                "experiment",
                "run",
                &source,
                "--backend",
                "mncs-research-bytecode",
                "--corpus",
                &corpus,
            ])
            .output()
            .expect("run CRE-3 mutant");
        assert_eq!(output.status.code(), Some(1));
        let result: Value = serde_json::from_slice(&output.stdout).expect("mutant result JSON");
        assert_eq!(result["status"], "FAIL");
        assert!(result["cases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|case_| case_["expectation_met"] == false));
    }
}

#[test]
fn profile_04_rejects_unbounded_malformed_and_authority_expanding_iteration() {
    for (fixture, code) in [
        ("source/profile04-unbounded-while.mncs", "MNP106"),
        ("source/profile04-invalid-bound-zero.mncs", "MNE142"),
        ("source/profile04-invalid-bound-too-large.mncs", "MNE142"),
        ("source/profile04-invalid-nonliteral-bound.mncs", "MNP094"),
        ("source/profile04-invalid-carried-state.mncs", "MNE144"),
        ("source/profile04-authority-laundering.mncs", "MNE134"),
        ("source/profile04-recursive-retry.mncs", "MNE130"),
    ] {
        let source = example(fixture);
        let output = binary()
            .args(["source-study", &source])
            .output()
            .expect("run negative Source Profile 0.4 fixture");
        assert_eq!(output.status.code(), Some(1));
        let result: Value = serde_json::from_slice(&output.stdout).expect("front-end JSON");
        assert!(result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == code));
    }
}

fn frozen_replication_workspace(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mncs-frozen-replication-{}-{}",
        name,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create workspace");
    dir
}

#[test]
fn frozen_execute_with_baseline_emits_a_sealed_replicated_result() {
    let temp = frozen_replication_workspace("happy");
    let source = example("source/cre1-evidence-combine.mncs");
    let corpus = example("execution/cre1-evidence-corpus.json");
    let baseline_dir = temp.join("baseline");
    let run = binary()
        .args([
            "experiment",
            "run",
            &source,
            "--backend",
            "mncs-portable-wasm-mvp",
            "--corpus",
            &corpus,
            "--output-dir",
            baseline_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run CRE-1 baseline");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    let replica_dir = temp.join("replica");
    let replication = binary()
        .args([
            "experiment",
            "execute",
            baseline_dir.join("backend-artifact.json").to_str().unwrap(),
            &corpus,
            "--baseline",
            baseline_dir.join("result.json").to_str().unwrap(),
            "--output-dir",
            replica_dir.to_str().unwrap(),
        ])
        .output()
        .expect("frozen replication");
    assert!(
        replication.status.success(),
        "{}",
        String::from_utf8_lossy(&replication.stderr)
    );
    let summary: Value =
        serde_json::from_slice(&replication.stdout).expect("replication summary JSON");
    assert_eq!(summary["schemaVersion"], "0.1");
    assert_eq!(summary["identityValid"], true);
    assert_eq!(summary["status"], "PASS");
    assert_eq!(summary["boundedBehaviorAgrees"], true);
    assert_eq!(summary["casesMatchingBaseline"], 9);
    assert_eq!(summary["propertiesMatchingBaseline"], 6);
    let definition_identity = summary["definitionIdentity"].clone();
    assert_eq!(
        summary["replicatedResultIdentity"],
        summary["baselineResultIdentity"]
    );
    assert!(summary["comparison"]["same_backend_artifact"] == true);

    let replicated: Value = serde_json::from_slice(
        &std::fs::read(replica_dir.join("replicated-result.json")).expect("replicated result"),
    )
    .expect("replicated result JSON");
    assert_eq!(replicated["definition"]["identity"], definition_identity);
    assert_eq!(replicated["identity"], summary["replicatedResultIdentity"]);

    let inspection = binary()
        .args([
            "experiment",
            "inspect",
            replica_dir.join("replicated-result.json").to_str().unwrap(),
        ])
        .output()
        .expect("inspect replicated result");
    assert!(inspection.status.success());
    let report: Value = serde_json::from_slice(&inspection.stdout).expect("inspection JSON");
    assert_eq!(report["identity_valid"], true);

    let _ = std::fs::remove_dir_all(temp);
}

#[test]
fn frozen_execute_rejects_substituted_corpus_without_writing_outputs() {
    let temp = frozen_replication_workspace("corpus-mismatch");
    let source = example("source/cre1-evidence-combine.mncs");
    let corpus = example("execution/cre1-evidence-corpus.json");
    let baseline_dir = temp.join("baseline");
    let run = binary()
        .args([
            "experiment",
            "run",
            &source,
            "--backend",
            "mncs-portable-wasm-mvp",
            "--corpus",
            &corpus,
            "--output-dir",
            baseline_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run CRE-1 baseline");
    assert!(run.status.success());

    let mut mutated: Value =
        serde_json::from_str(&std::fs::read_to_string(&corpus).expect("read canonical corpus"))
            .expect("canonical corpus JSON");
    mutated["cases"][0]["request"]["step_budget"] = Value::from(
        mutated["cases"][0]["request"]["step_budget"]
            .as_u64()
            .unwrap_or(1000)
            + 7,
    );
    let mutated_path = temp.join("corpus-mutated.json");
    std::fs::write(&mutated_path, serde_json::to_vec_pretty(&mutated).unwrap())
        .expect("write mutated corpus");

    let replica_dir = temp.join("replica");
    let replication = binary()
        .args([
            "experiment",
            "execute",
            baseline_dir.join("backend-artifact.json").to_str().unwrap(),
            mutated_path.to_str().unwrap(),
            "--baseline",
            baseline_dir.join("result.json").to_str().unwrap(),
            "--output-dir",
            replica_dir.to_str().unwrap(),
        ])
        .output()
        .expect("frozen replication with substituted corpus");
    assert_eq!(replication.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&replication.stderr).contains("corpus does not match"));
    assert!(!replica_dir.exists());
    let _ = std::fs::remove_dir_all(temp);
}

#[test]
fn frozen_execute_rejects_a_mutated_frozen_backend_artifact() {
    let temp = frozen_replication_workspace("artifact-mutation");
    let source = example("source/cre1-evidence-combine.mncs");
    let corpus = example("execution/cre1-evidence-corpus.json");
    let baseline_dir = temp.join("baseline");
    let run = binary()
        .args([
            "experiment",
            "run",
            &source,
            "--backend",
            "mncs-portable-wasm-mvp",
            "--corpus",
            &corpus,
            "--output-dir",
            baseline_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run CRE-1 baseline");
    assert!(run.status.success());

    // Mutate the artifact body while keeping the recorded bytes digest
    // self-consistent, so only the comparison against the frozen realization
    // identity can catch the substitution.
    let mut artifact: Value = serde_json::from_str(
        &std::fs::read_to_string(baseline_dir.join("backend-artifact.json"))
            .expect("read frozen artifact"),
    )
    .expect("artifact JSON");
    let hex = artifact["bytes_hex"].as_str().unwrap().to_owned();
    let mut bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    artifact["bytes_hex"] = Value::String(
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    artifact["bytes_sha256"] = Value::String({
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(&bytes);
        format!("sha256:{digest:x}")
    });
    let mutated_artifact_path = temp.join("backend-artifact-mutated.json");
    std::fs::write(
        &mutated_artifact_path,
        serde_json::to_vec_pretty(&artifact).unwrap(),
    )
    .expect("write mutated artifact");

    let replica_dir = temp.join("replica");
    let replication = binary()
        .args([
            "experiment",
            "execute",
            mutated_artifact_path.to_str().unwrap(),
            &corpus,
            "--baseline",
            baseline_dir.join("result.json").to_str().unwrap(),
            "--output-dir",
            replica_dir.to_str().unwrap(),
        ])
        .output()
        .expect("frozen replication with mutated artifact");
    assert_eq!(replication.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&replication.stderr)
        .contains("does not match the frozen realization"));
    assert!(!replica_dir.exists());
    let _ = std::fs::remove_dir_all(temp);
}

#[test]
fn frozen_execute_rejects_a_tampered_baseline_result() {
    let temp = frozen_replication_workspace("tampered-baseline");
    let source = example("source/cre1-evidence-combine.mncs");
    let corpus = example("execution/cre1-evidence-corpus.json");
    let baseline_dir = temp.join("baseline");
    let run = binary()
        .args([
            "experiment",
            "run",
            &source,
            "--backend",
            "mncs-portable-wasm-mvp",
            "--corpus",
            &corpus,
            "--output-dir",
            baseline_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run CRE-1 baseline");
    assert!(run.status.success());

    let mut tampered: Value = serde_json::from_str(
        &std::fs::read_to_string(baseline_dir.join("result.json")).expect("read baseline result"),
    )
    .expect("baseline JSON");
    tampered["status"] = Value::String("FAIL".to_owned());
    let tampered_path = temp.join("baseline-tampered.json");
    std::fs::write(
        &tampered_path,
        serde_json::to_vec_pretty(&tampered).unwrap(),
    )
    .expect("write tampered baseline");

    let replication = binary()
        .args([
            "experiment",
            "execute",
            baseline_dir.join("backend-artifact.json").to_str().unwrap(),
            &corpus,
            "--baseline",
            tampered_path.to_str().unwrap(),
            "--output-dir",
            temp.join("replica").to_str().unwrap(),
        ])
        .output()
        .expect("frozen replication with tampered baseline");
    assert_eq!(replication.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&replication.stderr).contains("identity is invalid"));
    let _ = std::fs::remove_dir_all(temp);
}

#[test]
fn frozen_replication_preserves_distinct_realization_identities_across_backends() {
    let temp = frozen_replication_workspace("plurality");
    let source = example("source/cre1-evidence-combine.mncs");
    let corpus = example("execution/cre1-evidence-corpus.json");
    let mut summaries = Vec::new();
    for backend in ["mncs-portable-wasm-mvp", "mncs-research-bytecode"] {
        let baseline_dir = temp.join(format!("baseline-{backend}"));
        let run = binary()
            .args([
                "experiment",
                "run",
                &source,
                "--backend",
                backend,
                "--corpus",
                &corpus,
                "--output-dir",
                baseline_dir.to_str().unwrap(),
            ])
            .output()
            .expect("run CRE-1 baseline");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let replica_dir = temp.join(format!("replica-{backend}"));
        let replication = binary()
            .args([
                "experiment",
                "execute",
                baseline_dir.join("backend-artifact.json").to_str().unwrap(),
                &corpus,
                "--baseline",
                baseline_dir.join("result.json").to_str().unwrap(),
                "--output-dir",
                replica_dir.to_str().unwrap(),
            ])
            .output()
            .expect("frozen replication");
        assert!(
            replication.status.success(),
            "{}",
            String::from_utf8_lossy(&replication.stderr)
        );
        summaries.push((
            serde_json::from_slice::<Value>(&run.stdout).expect("baseline JSON"),
            serde_json::from_slice::<Value>(&replication.stdout).expect("summary JSON"),
        ));
    }
    let (wasm_baseline, wasm_summary) = &summaries[0];
    let (bc_baseline, bc_summary) = &summaries[1];
    // Each replication keeps its own frozen realization identity.
    assert_eq!(
        wasm_baseline["artifact"]["identity"],
        wasm_summary["backendArtifactIdentity"]
    );
    assert_eq!(
        bc_baseline["artifact"]["identity"],
        bc_summary["backendArtifactIdentity"]
    );
    assert_ne!(
        wasm_summary["backendArtifactIdentity"],
        bc_summary["backendArtifactIdentity"]
    );
    // Both replications agree on bounded behavior while remaining distinct records.
    assert_eq!(wasm_summary["boundedBehaviorAgrees"], true);
    assert_eq!(bc_summary["boundedBehaviorAgrees"], true);
    let _ = std::fs::remove_dir_all(temp);
}

#[test]
fn profile_05_record_values_execute_identically_across_backends() {
    let source = example("source/profile05-record-values.mncs");
    let corpus = example("execution/profile05-record-values-corpus.json");
    let mut results = Vec::new();
    for backend in ["mncs-portable-wasm-mvp", "mncs-research-bytecode"] {
        let output = binary()
            .args([
                "experiment",
                "run",
                &source,
                "--backend",
                backend,
                "--corpus",
                &corpus,
            ])
            .output()
            .expect("run Profile 0.5 record experiment");
        assert!(
            output.status.success(),
            "{backend}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("record result JSON");
        assert_eq!(result["definition"]["source_profile"], "0.5");
        // Checked arithmetic outside a bounded counter stays UNKNOWN; the
        // record operations themselves must not change that honesty.
        assert_eq!(result["status"], "UNKNOWN");
        results.push(result);
    }
    // Bounded behavior agrees across realizations even though artifacts differ.
    let observed: Vec<Vec<(String, String, Option<i64>)>> = results
        .iter()
        .map(|result| {
            result["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|case_| {
                    (
                        case_["case_id"].as_str().unwrap().to_owned(),
                        case_["status"].as_str().unwrap().to_owned(),
                        case_["returned"]
                            .as_array()
                            .and_then(|values| values.first())
                            .and_then(|value| value["integer"]["value"].as_i64()),
                    )
                })
                .collect()
        })
        .collect();
    assert_eq!(observed[0], observed[1]);
    let by_id = |observed: &[(String, String, Option<i64>)], id: &str| {
        observed
            .iter()
            .find(|(case_id, _, _)| case_id == id)
            .expect("corpus case present")
            .clone()
    };
    let zero = by_id(&observed[0], "zero");
    assert_eq!(zero.1, "returned");
    assert_eq!(zero.2, Some(42));
    // Checked i32 multiply overflow traps identically on both realizations.
    assert_eq!(by_id(&observed[0], "overflow-guard").1, "runtime_failure");
    assert_eq!(by_id(&observed[1], "overflow-guard").1, "runtime_failure");
}

#[test]
fn profile_05_record_negatives_are_rejected_before_execution() {
    for (fixture, code) in [
        ("source/profile05-invalid-duplicate-field.mncs", "MNE152"),
        (
            "source/profile05-invalid-literal-unknown-field.mncs",
            "MNE156",
        ),
        ("source/profile05-records-in-profile04.mncs", "MNP120"),
    ] {
        let source = example(fixture);
        let output = binary()
            .args(["source-study", &source])
            .output()
            .expect("run negative Profile 0.5 fixture");
        assert_eq!(output.status.code(), Some(1));
        let result: Value = serde_json::from_slice(&output.stdout).expect("front-end JSON");
        assert!(result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == code));
    }
}
