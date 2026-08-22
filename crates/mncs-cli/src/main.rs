use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use mncs_codegen::{
    backend_adapter, backend_capabilities, backend_names, compare_body_ssa_and_backend,
    execute_backend, lower_selected_ssa, portable_wasm_plan, selected_ssa_ref, target_for_backend,
    target_is_portable_wasm,
};
use mncs_compiler::{native_node_profile, reference_compiler_architecture, ReferenceCompiler};
use mncs_model::{
    compare_body_and_ssa, compare_execution, execute_ssa, execute_with_policy,
    ArtifactRepresentation, CandidateEvaluation, CausalSlice, ComparisonStatus, CompilationResult,
    CompilationStatus, CompilationStudyRequest, Confidence, DeterministicVerifier,
    DiagnosticCategory, DiagnosticObligation, EvidenceFreshness, EvidenceManifest, EvidenceState,
    ExecutionComparison, ExecutionCorpus, ExecutionRequest, ExecutionStatus, FunctionBody,
    LanguageExperimentCaseObservation, LanguageExperimentComparison, LanguageExperimentDefinition,
    LanguageExperimentResult, LoweringExecutionComparison, LoweringExecutionStatus,
    ObligationStatus, Program, RealizationRequest, SemanticDiff, SemanticId, TargetContractRef,
    ValidatorRequirement,
};
use mncs_syntax::{
    analyze, SourceArtifactKind, SourceEnvelope, SourceMetrics, SourceOrigin, SourceOriginKind,
};
use mncs_translation_check::{
    generate, validate_backend_lowering, validate_branch_simplification, validate_checked_elision,
    validate_constant_folding, validate_unreachable_removal,
};
use serde::{Deserialize, Serialize};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return ExitCode::from(2);
    };

    match command.as_str() {
        "validate" => {
            let Some(path) = args.next() else {
                eprintln!("error: validate requires a manifest path");
                print_usage();
                return ExitCode::from(2);
            };
            if args.next().is_some() {
                eprintln!("error: unexpected additional arguments");
                return ExitCode::from(2);
            }
            validate(&path)
        }
        "canonicalize" => one_manifest_command(args, canonicalize),
        "identity" => one_manifest_command(args, identity),
        "graph" => one_manifest_command(args, graph),
        "evidence-manifest" => one_manifest_command(args, evidence_manifest),
        "ir" => one_manifest_command(args, ir),
        "ssa" => one_manifest_command(args, ssa),
        "obligations" => one_manifest_command(args, obligations),
        "verify" => one_manifest_command(args, verify),
        "diagnose" => one_manifest_command(args, diagnose),
        "body" => one_manifest_command(args, body),
        "trace" => one_manifest_command(args, trace),
        "verifier-request" => one_manifest_command(args, verifier_request),
        "execute" => execution_command(args),
        "execute-ssa" => ssa_execution_command(args),
        "compare-execution" => execution_compare_command(args),
        "check-lowering-execution" => lowering_execution_command(args),
        "compile" => compile_command(args),
        "execute-backend" => backend_execution_command(args),
        "check-backend-execution" => backend_compare_command(args),
        "validate-translation" => validate_translation_command(args),
        "compiler-architecture" => {
            if args.next().is_some() {
                eprintln!("error: compiler-architecture does not accept arguments");
                return ExitCode::from(2);
            }
            if print_json(&reference_compiler_architecture()) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        "compiler-study" => compiler_study_command(args),
        "source-study" => source_study_command(args),
        "experiment" => experiment_command(args),
        "diff" => two_manifest_command(args, diff),
        "compare" => two_manifest_command(args, compare),
        "slice" => slice_command(args),
        "evaluate-candidate" => evaluate_candidate_command(args),
        "verify-result" => verify_result_command(args),
        "evidence-check" => two_path_command(args, evidence_check),
        "syntax-metrics" => {
            let paths: Vec<String> = args.collect();
            syntax_metrics(paths)
        }
        "syntax-tournament" => {
            let Some(path) = args.next() else {
                eprintln!("error: syntax-tournament requires a tournament manifest");
                print_usage();
                return ExitCode::from(2);
            };
            if args.next().is_some() {
                eprintln!("error: unexpected additional arguments");
                return ExitCode::from(2);
            }
            syntax_tournament(&path)
        }
        "--help" | "-h" | "help" => {
            print_usage();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("error: unknown command {other:?}");
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn validate(path: &str) -> ExitCode {
    let input = match read_source(path) {
        Ok(input) => input,
        Err(code) => return code,
    };

    let program = match Program::from_json(&input) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };

    let report = program.validate();
    if !print_json(&report) {
        return ExitCode::from(2);
    }

    if report.valid {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn one_manifest_command<I>(args: I, command: fn(&str) -> ExitCode) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(path) = args.next() else {
        eprintln!("error: command requires a manifest path");
        print_usage();
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("error: unexpected additional arguments");
        return ExitCode::from(2);
    }
    command(&path)
}

fn two_manifest_command<I>(args: I, command: fn(&str, &str) -> ExitCode) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let (Some(before), Some(after)) = (args.next(), args.next()) else {
        eprintln!("error: command requires before and after manifest paths");
        print_usage();
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("error: unexpected additional arguments");
        return ExitCode::from(2);
    }
    command(&before, &after)
}

fn two_path_command<I>(args: I, command: fn(&str, &str) -> ExitCode) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    two_manifest_command(args, command)
}

fn read_valid_program(path: &str) -> Result<Program, ExitCode> {
    let input = read_source(path)?;
    let program = Program::from_json(&input).map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::from(2)
    })?;
    let report = program.validate();
    if !report.valid {
        if !print_json(&report) {
            return Err(ExitCode::from(2));
        }
        return Err(ExitCode::FAILURE);
    }
    Ok(program)
}

fn canonicalize(path: &str) -> ExitCode {
    let program = match read_valid_program(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    match program.canonical_form() {
        Ok(form) if print_json(&form) => ExitCode::SUCCESS,
        Ok(_) | Err(_) => {
            eprintln!("error: unable to create canonical representation");
            ExitCode::from(2)
        }
    }
}

fn identity(path: &str) -> ExitCode {
    let program = match read_valid_program(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    if print_json(&program.semantic_identities()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn graph(path: &str) -> ExitCode {
    let program = match read_valid_program(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    match program.semantic_graph() {
        Ok(graph) if print_json(&graph) => ExitCode::SUCCESS,
        Ok(_) | Err(_) => {
            eprintln!("error: unable to construct semantic graph");
            ExitCode::from(2)
        }
    }
}

fn evidence_manifest(path: &str) -> ExitCode {
    let program = match read_valid_program(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    match program.evidence_manifest() {
        Ok(manifest) if print_json(&manifest) => ExitCode::SUCCESS,
        Ok(_) | Err(_) => {
            eprintln!("error: unable to construct evidence manifest");
            ExitCode::from(2)
        }
    }
}

fn ir(path: &str) -> ExitCode {
    let program = match read_valid_program(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    match program.lower_to_ir() {
        Ok(ir) if print_json(&ir) => ExitCode::SUCCESS,
        Ok(_) | Err(_) => {
            eprintln!("error: unable to lower manifest to high-level IR");
            ExitCode::from(2)
        }
    }
}

fn ssa(path: &str) -> ExitCode {
    let program = match read_valid_program(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    match program.lower_to_ssa() {
        Ok(ssa) if print_json(&ssa) => ExitCode::SUCCESS,
        Ok(_) | Err(_) => {
            eprintln!("error: unable to lower manifest to validated SSA");
            ExitCode::FAILURE
        }
    }
}

fn obligations(path: &str) -> ExitCode {
    let program = match read_valid_program(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    if print_json(&program.generate_obligations()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn verify(path: &str) -> ExitCode {
    let program = match read_valid_program(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let results = program.verify_obligations(&DeterministicVerifier::default());
    let failed = results
        .iter()
        .any(|result| result.status == ObligationStatus::Fail);
    let unresolved = results
        .iter()
        .any(|result| result.status == ObligationStatus::Unknown);
    if !print_json(&results) {
        ExitCode::from(2)
    } else if failed || unresolved {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[derive(Debug, Serialize)]
struct BodyReport {
    schema_version: String,
    functions: Vec<BodyFunctionReport>,
}

#[derive(Debug, Serialize)]
struct BodyFunctionReport {
    name: String,
    identity: SemanticId,
    fingerprint: String,
    cfg: mncs_model::Cfg,
    body: FunctionBody,
}

fn body(path: &str) -> ExitCode {
    let program = match read_valid_program(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let mut functions = Vec::new();
    for function in &program.functions {
        if let Some(body) = &function.body {
            functions.push(BodyFunctionReport {
                name: function.name.clone(),
                identity: body.identity(&program.module, &function.name),
                fingerprint: body.fingerprint().expect("canonical body"),
                cfg: mncs_model::Cfg::from_body(
                    body,
                    body.identity(&program.module, &function.name),
                ),
                body: body.clone(),
            });
        }
    }
    if print_json(&BodyReport {
        schema_version: "0.2".to_owned(),
        functions,
    }) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn trace(path: &str) -> ExitCode {
    let program = match read_valid_program(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    match program.lower_to_ir() {
        Ok(ir) if print_json(&ir.trace) => ExitCode::SUCCESS,
        Ok(_) | Err(_) => {
            eprintln!("error: unable to lower manifest for trace emission");
            ExitCode::from(2)
        }
    }
}

fn verifier_request(path: &str) -> ExitCode {
    let program = match read_valid_program(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    if print_json(&program.verifier_requests()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn execution_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let (Some(program_path), Some(request_path)) = (args.next(), args.next()) else {
        eprintln!("error: execute requires a program and execution request path");
        print_usage();
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("error: unexpected additional arguments");
        return ExitCode::from(2);
    }
    let request_input = match read_source(&request_path) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let request: ExecutionRequest = match serde_json::from_str(&request_input) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("error: invalid execution request: {error}");
            return ExitCode::from(2);
        }
    };
    let program_input = match read_source(&program_path) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let program = match Program::from_json(&program_input) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("error: invalid semantic manifest: {error}");
            return ExitCode::from(2);
        }
    };
    let result = execute_with_policy(&program, &request);
    let status = result.status;
    if !print_json(&result) {
        ExitCode::from(2)
    } else {
        execution_status_code(status)
    }
}

fn execution_compare_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let (Some(baseline_path), Some(candidate_path), Some(corpus_path)) =
        (args.next(), args.next(), args.next())
    else {
        eprintln!("error: compare-execution requires baseline, candidate, and corpus paths");
        print_usage();
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("error: unexpected additional arguments");
        return ExitCode::from(2);
    }
    let baseline = match read_program_for_execution(&baseline_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let candidate = match read_program_for_execution(&candidate_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let corpus_input = match read_source(&corpus_path) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let corpus: ExecutionCorpus = match serde_json::from_str(&corpus_input) {
        Ok(corpus) => corpus,
        Err(error) => {
            eprintln!("error: invalid execution corpus: {error}");
            return ExitCode::from(2);
        }
    };
    let report: ExecutionComparison = compare_execution(&baseline, &candidate, &corpus);
    let status = report.status;
    if !print_json(&report) {
        ExitCode::from(2)
    } else {
        match status {
            ComparisonStatus::EquivalentOverCorpus => ExitCode::SUCCESS,
            ComparisonStatus::MismatchDetected => ExitCode::FAILURE,
            ComparisonStatus::InvalidInput => ExitCode::from(2),
        }
    }
}

fn ssa_execution_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let (Some(program_path), Some(request_path)) = (args.next(), args.next()) else {
        eprintln!("error: execute-ssa requires a program and execution request path");
        print_usage();
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("error: unexpected additional arguments");
        return ExitCode::from(2);
    }
    let request_input = match read_source(&request_path) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let request: ExecutionRequest = match serde_json::from_str(&request_input) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("error: invalid execution request: {error}");
            return ExitCode::from(2);
        }
    };
    let program = match read_program_for_execution(&program_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let result = execute_ssa(&program, &request);
    let status = result.status;
    if !print_json(&result) {
        ExitCode::from(2)
    } else {
        execution_status_code(status)
    }
}

fn lowering_execution_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let (Some(program_path), Some(corpus_path)) = (args.next(), args.next()) else {
        eprintln!("error: check-lowering-execution requires a program and corpus path");
        print_usage();
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("error: unexpected additional arguments");
        return ExitCode::from(2);
    }
    let program = match read_program_for_execution(&program_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let corpus_input = match read_source(&corpus_path) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let corpus: ExecutionCorpus = match serde_json::from_str(&corpus_input) {
        Ok(corpus) => corpus,
        Err(error) => {
            eprintln!("error: invalid execution corpus: {error}");
            return ExitCode::from(2);
        }
    };
    let report: LoweringExecutionComparison = compare_body_and_ssa(&program, &corpus);
    let status = report.status;
    if !print_json(&report) {
        ExitCode::from(2)
    } else {
        match status {
            LoweringExecutionStatus::ConsistentOverCorpus => ExitCode::SUCCESS,
            LoweringExecutionStatus::MismatchDetected | LoweringExecutionStatus::Unsupported => {
                ExitCode::FAILURE
            }
            LoweringExecutionStatus::InvalidInput => ExitCode::from(2),
        }
    }
}

#[derive(Debug)]
struct CompileOptions {
    program_path: String,
    emit: BTreeSet<ArtifactRepresentation>,
    output_dir: Option<PathBuf>,
    target: Option<TargetContractRef>,
}

fn compile_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let options = match parse_compile_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}");
            print_usage();
            return ExitCode::from(2);
        }
    };
    let program = match read_program_unvalidated(&options.program_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let compiler = ReferenceCompiler::default();
    let request = compiler.request_for_program(&program, options.emit, options.target);
    let result = compiler.compile(request, &program);
    if let Some(output_dir) = &options.output_dir {
        if let Err(error) = write_compilation_outputs(output_dir, &result) {
            eprintln!(
                "error: unable to write compiler artifacts to {:?}: {error}",
                output_dir
            );
            return ExitCode::from(2);
        }
    }
    let status = result.status;
    if !print_json(&result) {
        ExitCode::from(2)
    } else if status == CompilationStatus::Failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn compiler_study_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(program_path) = args.next() else {
        eprintln!("error: compiler-study requires a semantic program path");
        print_usage();
        return ExitCode::from(2);
    };
    let mut node_identity = "local-node".to_owned();
    let mut target_name = None;
    let mut family_reference = false;
    while let Some(option) = args.next() {
        match option.as_str() {
            "--node-id" => {
                let Some(value) = args.next() else {
                    eprintln!("error: --node-id requires a value");
                    return ExitCode::from(2);
                };
                node_identity = value;
            }
            "--target" => {
                let Some(value) = args.next() else {
                    eprintln!("error: --target requires a target-contract candidate");
                    return ExitCode::from(2);
                };
                target_name = Some(value);
            }
            "--family-reference" => family_reference = true,
            other => {
                eprintln!("error: unknown compiler-study option {other:?}");
                return ExitCode::from(2);
            }
        }
    }
    let program = match read_program_unvalidated(&program_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let compiler = ReferenceCompiler::default();
    let target = target_name.map(unevidenced_target);
    let mut emit = [
        ArtifactRepresentation::Semantic,
        ArtifactRepresentation::Hir,
        ArtifactRepresentation::Ssa,
        ArtifactRepresentation::EvidenceBundle,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if target.is_some() {
        emit.insert(ArtifactRepresentation::TargetLoweringPlan);
    }
    if target.as_ref().is_some_and(target_has_backend_adapter) {
        emit.insert(ArtifactRepresentation::BackendArtifact);
    }
    let request = compiler.request_for_program(&program, emit, target);
    let study =
        CompilationStudyRequest::new(native_node_profile(node_identity), request, Vec::new());
    let result = compiler.run_study(study, &program);
    let failed = result.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == mncs_model::CompilerDiagnosticKind::SemanticInvalidity
    });
    let printed = if family_reference {
        print_json(&result.family_reference())
    } else {
        print_json(&result)
    };
    if !printed {
        ExitCode::from(2)
    } else if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn source_study_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(source_path) = args.next() else {
        eprintln!("error: source-study requires an MNCS source path");
        return ExitCode::from(2);
    };
    let mut node_identity = "local-source-node".to_owned();
    while let Some(option) = args.next() {
        match option.as_str() {
            "--node-id" => {
                let Some(value) = args.next() else {
                    eprintln!("error: --node-id requires a value");
                    return ExitCode::from(2);
                };
                node_identity = value;
            }
            other => {
                eprintln!("error: unknown source-study option {other:?}");
                return ExitCode::from(2);
            }
        }
    }
    let source = match read_source(&source_path) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let envelope = SourceEnvelope::new(
        SourceArtifactKind::Program,
        source_path.clone(),
        SourceOrigin {
            kind: SourceOriginKind::Path,
            locator: Some(source_path),
        },
        source,
    );
    let output =
        ReferenceCompiler::default().run_source_study(envelope, native_node_profile(node_identity));
    match output.study {
        Some(study) => {
            if print_json(&study) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        None => {
            if print_json(&output.front_end) {
                ExitCode::FAILURE
            } else {
                ExitCode::from(2)
            }
        }
    }
}

#[derive(Debug)]
struct ExperimentOptions {
    source_path: String,
    backend: String,
    corpus_path: String,
    output_dir: Option<PathBuf>,
    node_identity: String,
}

struct PreparedExperiment {
    envelope: SourceEnvelope,
    program: Program,
    definition: LanguageExperimentDefinition,
}

fn experiment_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(action) = args.next() else {
        eprintln!("error: experiment requires plan, run, execute, inspect, or compare");
        return ExitCode::from(2);
    };
    match action.as_str() {
        "plan" | "run" => {
            let options = match parse_experiment_options(args) {
                Ok(options) => options,
                Err(error) => {
                    eprintln!("error: {error}");
                    return ExitCode::from(2);
                }
            };
            let prepared = match prepare_experiment(&options) {
                Ok(prepared) => prepared,
                Err(code) => return code,
            };
            if action == "plan" {
                if let Some(output_dir) = &options.output_dir {
                    if let Err(error) = fs::create_dir_all(output_dir).and_then(|_| {
                        write_pretty_json(output_dir.join("definition.json"), &prepared.definition)
                    }) {
                        eprintln!("error: unable to write experiment plan: {error}");
                        return ExitCode::from(2);
                    }
                }
                return if print_json(&prepared.definition) {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(2)
                };
            }
            run_experiment(options, prepared)
        }
        "execute" => {
            let (Some(artifact_path), Some(corpus_path)) = (args.next(), args.next()) else {
                eprintln!("error: experiment execute requires an artifact and corpus path");
                return ExitCode::from(2);
            };
            if args.next().is_some() {
                eprintln!("error: unexpected experiment execute arguments");
                return ExitCode::from(2);
            }
            let artifact = match read_json::<mncs_model::BackendArtifact>(&artifact_path) {
                Ok(artifact) => artifact,
                Err(code) => return code,
            };
            let corpus = match read_json::<ExecutionCorpus>(&corpus_path) {
                Ok(corpus) => corpus,
                Err(code) => return code,
            };
            let observations = corpus
                .cases
                .iter()
                .map(|case_| {
                    let observation = execute_backend(&artifact, &case_.request);
                    LanguageExperimentCaseObservation {
                        case_id: case_.id.clone(),
                        status: observation.status,
                        returned: observation.returned,
                        failure_reason: observation.failure.map(|failure| failure.reason),
                    }
                })
                .collect::<Vec<_>>();
            let invalid = observations.iter().any(|observation| {
                matches!(
                    observation.status,
                    ExecutionStatus::InvalidRequest | ExecutionStatus::Unsupported
                )
            });
            if !print_json(&observations) {
                ExitCode::from(2)
            } else if invalid {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        "inspect" => {
            let Some(path) = args.next() else {
                eprintln!("error: experiment inspect requires a result path");
                return ExitCode::from(2);
            };
            if args.next().is_some() {
                eprintln!("error: unexpected experiment inspect arguments");
                return ExitCode::from(2);
            }
            let result = match read_json::<LanguageExperimentResult>(&path) {
                Ok(result) => result,
                Err(code) => return code,
            };
            let report = ExperimentInspection::from(&result);
            if print_json(&report) && report.identity_valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        "compare" => {
            let (Some(left_path), Some(right_path)) = (args.next(), args.next()) else {
                eprintln!("error: experiment compare requires two result paths");
                return ExitCode::from(2);
            };
            if args.next().is_some() {
                eprintln!("error: unexpected experiment compare arguments");
                return ExitCode::from(2);
            }
            let left = match read_json::<LanguageExperimentResult>(&left_path) {
                Ok(result) => result,
                Err(code) => return code,
            };
            let right = match read_json::<LanguageExperimentResult>(&right_path) {
                Ok(result) => result,
                Err(code) => return code,
            };
            let comparison = LanguageExperimentComparison::compare(&left, &right);
            if print_json(&comparison) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        other => {
            eprintln!("error: unknown experiment action {other:?}");
            ExitCode::from(2)
        }
    }
}

fn parse_experiment_options<I>(args: I) -> Result<ExperimentOptions, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let source_path = args
        .next()
        .ok_or_else(|| "experiment plan/run requires an MNCS source path".to_owned())?;
    let mut backend = None;
    let mut corpus_path = None;
    let mut output_dir = None;
    let mut node_identity = "local-experiment-node".to_owned();
    while let Some(option) = args.next() {
        match option.as_str() {
            "--backend" => {
                backend = Some(
                    args.next()
                        .ok_or_else(|| "--backend requires an adapter name".to_owned())?,
                );
            }
            "--corpus" => {
                corpus_path = Some(
                    args.next()
                        .ok_or_else(|| "--corpus requires a JSON path".to_owned())?,
                );
            }
            "--output-dir" => {
                output_dir = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--output-dir requires a path".to_owned())?,
                ));
            }
            "--node-id" => {
                node_identity = args
                    .next()
                    .ok_or_else(|| "--node-id requires a value".to_owned())?;
            }
            other => return Err(format!("unknown experiment option {other:?}")),
        }
    }
    let backend = backend.ok_or_else(|| {
        format!(
            "--backend is required; available adapters: {}",
            backend_names().join(", ")
        )
    })?;
    if backend_adapter(&backend).is_none() {
        return Err(format!(
            "unknown backend {backend:?}; available adapters: {}",
            backend_names().join(", ")
        ));
    }
    Ok(ExperimentOptions {
        source_path,
        backend,
        corpus_path: corpus_path.ok_or_else(|| "--corpus is required".to_owned())?,
        output_dir,
        node_identity,
    })
}

fn prepare_experiment(options: &ExperimentOptions) -> Result<PreparedExperiment, ExitCode> {
    let source = read_source(&options.source_path)?;
    let envelope = SourceEnvelope::new(
        SourceArtifactKind::Program,
        options.source_path.clone(),
        SourceOrigin {
            kind: SourceOriginKind::Path,
            locator: Some(options.source_path.clone()),
        },
        source,
    );
    let compiler = ReferenceCompiler::default();
    let front_end = compiler.front_end(envelope.clone());
    let Some(program) = front_end.program.clone().filter(|_| front_end.is_valid()) else {
        let _ = print_json(&front_end);
        return Err(ExitCode::FAILURE);
    };
    let ssa = program.lower_to_ssa().map_err(|error| {
        eprintln!("error: unable to produce selected SSA for experiment: {error}");
        ExitCode::FAILURE
    })?;
    let selected = selected_ssa_ref(&ssa);
    let adapter = backend_adapter(&options.backend).expect("validated by option parser");
    let capabilities = adapter.capabilities();
    let target = adapter.target();
    let realization = RealizationRequest::new(
        selected,
        target,
        [capabilities.backend.clone()].into_iter().collect(),
        ["checked_integer", "explicit_failure"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        capabilities.artifact_kinds.clone(),
        ["bounded_execution_agreement".to_owned()]
            .into_iter()
            .collect(),
        "fail_closed_no_backend_fallback",
    );
    let corpus = read_json::<ExecutionCorpus>(&options.corpus_path)?;
    let definition = LanguageExperimentDefinition::new(
        Path::new(&options.source_path)
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("mncs-experiment"),
        envelope.identity.clone(),
        envelope.language_version.clone(),
        compiler.identity.identity.clone(),
        BTreeMap::from([("backend".to_owned(), capabilities.backend.name.clone())]),
        realization,
        corpus,
        vec![ValidatorRequirement {
            relation: "bounded_observational_agreement".to_owned(),
            validator_profile: "backend-lowering-bounded-agreement:0.1".to_owned(),
            required: true,
            allow_unknown: false,
        }],
        vec![
            "compiler_study".to_owned(),
            "realization_plan".to_owned(),
            "backend_artifact".to_owned(),
            "translation_validation".to_owned(),
            "case_observations".to_owned(),
        ],
        vec![
            "source_to_ssa_identity_chain_is_exact".to_owned(),
            "FAIL_dominates_UNKNOWN_dominates_PASS".to_owned(),
        ],
        "bounded_public_behavior_agreement",
        None,
        Vec::new(),
    );
    Ok(PreparedExperiment {
        envelope,
        program,
        definition,
    })
}

fn run_experiment(options: ExperimentOptions, prepared: PreparedExperiment) -> ExitCode {
    let compiler = ReferenceCompiler::default();
    let emit = [
        ArtifactRepresentation::Semantic,
        ArtifactRepresentation::Hir,
        ArtifactRepresentation::Ssa,
        ArtifactRepresentation::TargetLoweringPlan,
        ArtifactRepresentation::BackendArtifact,
        ArtifactRepresentation::EvidenceBundle,
    ]
    .into_iter()
    .collect();
    let request = match compiler.request_for_program_with_backend(
        &prepared.program,
        emit,
        &options.backend,
    ) {
        Ok(request) => request,
        Err(diagnostic) => {
            let _ = print_json(&diagnostic);
            return ExitCode::FAILURE;
        }
    };
    let compilation = compiler.compile(request, &prepared.program);
    if compilation.status == CompilationStatus::Failed {
        let _ = print_json(&compilation);
        return ExitCode::FAILURE;
    }
    let study_output = compiler.run_source_study_with_backend(
        prepared.envelope.clone(),
        native_node_profile(options.node_identity),
        &options.backend,
    );
    let Some(study) = study_output.study else {
        let _ = print_json(&study_output.front_end);
        return ExitCode::FAILURE;
    };
    let (Some(ssa), Some(plan), Some(artifact)) = (
        compilation.emissions.ssa.as_ref(),
        compilation.emissions.target_lowering_plan.clone(),
        compilation.emissions.backend.clone(),
    ) else {
        let _ = print_json(&compilation);
        return ExitCode::FAILURE;
    };
    let validation = validate_backend_lowering(
        &prepared.program,
        ssa,
        &artifact,
        &prepared.definition.corpus,
    );
    let cases = prepared
        .definition
        .corpus
        .cases
        .iter()
        .map(|case_| {
            let observation = execute_backend(&artifact, &case_.request);
            LanguageExperimentCaseObservation {
                case_id: case_.id.clone(),
                status: observation.status,
                returned: observation.returned,
                failure_reason: observation.failure.map(|failure| failure.reason),
            }
        })
        .collect();
    let mut unresolved = Vec::new();
    if compilation.status == CompilationStatus::CompletedWithUnresolvedObligations {
        unresolved.push("compilation retained unresolved obligations".to_owned());
    }
    let capabilities = backend_capabilities(&options.backend).expect("validated backend");
    let result = LanguageExperimentResult::new(
        prepared.definition,
        study,
        capabilities,
        plan,
        artifact,
        vec![validation],
        cases,
        unresolved,
    );
    if let Some(output_dir) = &options.output_dir {
        if let Err(error) = write_experiment_outputs(output_dir, &result) {
            eprintln!("error: unable to write experiment outputs: {error}");
            return ExitCode::from(2);
        }
    }
    let status = result.status;
    if !print_json(&result) {
        ExitCode::from(2)
    } else if status == mncs_model::LanguageExperimentStatus::Fail {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn write_experiment_outputs(
    output_dir: &Path,
    result: &LanguageExperimentResult,
) -> Result<(), std::io::Error> {
    fs::create_dir_all(output_dir)?;
    write_pretty_json(output_dir.join("definition.json"), &result.definition)?;
    write_pretty_json(
        output_dir.join("compiler-study.json"),
        &result.compiler_study,
    )?;
    write_pretty_json(
        output_dir.join("realization-plan.json"),
        &result.realization_plan,
    )?;
    write_pretty_json(output_dir.join("backend-artifact.json"), &result.artifact)?;
    write_pretty_json(output_dir.join("result.json"), result)?;
    let bytes = result.artifact.bytes().map_err(std::io::Error::other)?;
    fs::write(
        output_dir.join(format!("artifact.{}", result.artifact.artifact_kind)),
        bytes,
    )
}

#[derive(Debug, Serialize)]
struct ExperimentInspection {
    identity: SemanticId,
    identity_valid: bool,
    definition_identity: SemanticId,
    source_artifact_identity: String,
    semantic_fingerprint: Option<String>,
    hir_fingerprint: Option<String>,
    ssa_fingerprint: Option<String>,
    realization_request_identity: SemanticId,
    realization_plan_identity: SemanticId,
    backend_identity: SemanticId,
    backend_artifact_identity: SemanticId,
    backend_artifact_kind: String,
    status: mncs_model::LanguageExperimentStatus,
    case_count: usize,
    validation_judgements: Vec<mncs_model::TranslationJudgement>,
    interpretation: String,
}

impl From<&LanguageExperimentResult> for ExperimentInspection {
    fn from(result: &LanguageExperimentResult) -> Self {
        Self {
            identity: result.identity.clone(),
            identity_valid: result.identity_is_valid(),
            definition_identity: result.definition_identity.clone(),
            source_artifact_identity: result.definition.source_artifact_identity.clone(),
            semantic_fingerprint: result.compiler_study.semantic_fingerprint.clone(),
            hir_fingerprint: result.compiler_study.hir_fingerprint.clone(),
            ssa_fingerprint: result.compiler_study.ssa_fingerprint.clone(),
            realization_request_identity: result.definition.realization.identity.clone(),
            realization_plan_identity: result.realization_plan.identity.clone(),
            backend_identity: result.backend.identity.clone(),
            backend_artifact_identity: result.artifact.identity.clone(),
            backend_artifact_kind: result.artifact.artifact_kind.clone(),
            status: result.status,
            case_count: result.cases.len(),
            validation_judgements: result
                .translation_validations
                .iter()
                .map(|validation| validation.judgement)
                .collect(),
            interpretation: result.interpretation.clone(),
        }
    }
}

fn parse_compile_options<I>(args: I) -> Result<CompileOptions, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let program_path = args
        .next()
        .ok_or_else(|| "compile requires a semantic program path".to_owned())?;
    let mut emit = [
        ArtifactRepresentation::Semantic,
        ArtifactRepresentation::Hir,
        ArtifactRepresentation::Ssa,
        ArtifactRepresentation::EvidenceBundle,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let mut output_dir = None;
    let mut target = None;
    while let Some(option) = args.next() {
        match option.as_str() {
            "--emit" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--emit requires a comma-separated value".to_owned())?;
                emit = parse_emit(&value)?;
            }
            "--output-dir" => {
                output_dir = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--output-dir requires a path".to_owned())?,
                ));
            }
            "--target" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--target requires a target-contract candidate".to_owned())?;
                target = Some(unevidenced_target(value));
            }
            other => return Err(format!("unknown compile option {other:?}")),
        }
    }
    if emit.contains(&ArtifactRepresentation::TargetLoweringPlan) && target.is_none() {
        return Err("target-plan emission requires --target".to_owned());
    }
    if emit.contains(&ArtifactRepresentation::BackendArtifact) && target.is_none() {
        return Err("backend emission requires --target".to_owned());
    }
    if target.as_ref().is_some_and(target_is_portable_wasm) {
        emit.insert(ArtifactRepresentation::TargetLoweringPlan);
        emit.insert(ArtifactRepresentation::BackendArtifact);
    }
    Ok(CompileOptions {
        program_path,
        emit,
        output_dir,
        target,
    })
}

fn parse_emit(value: &str) -> Result<BTreeSet<ArtifactRepresentation>, String> {
    let mut emit = BTreeSet::new();
    for name in value.split(',') {
        let representation = match name.trim() {
            "semantic" => ArtifactRepresentation::Semantic,
            "hir" => ArtifactRepresentation::Hir,
            "ssa" => ArtifactRepresentation::Ssa,
            "evidence" => ArtifactRepresentation::EvidenceBundle,
            "target-plan" => ArtifactRepresentation::TargetLoweringPlan,
            "backend" => ArtifactRepresentation::BackendArtifact,
            "" => return Err("--emit contains an empty representation".to_owned()),
            other => return Err(format!("unsupported emitted representation {other:?}")),
        };
        emit.insert(representation);
    }
    if emit.is_empty() {
        return Err("--emit must request at least one representation".to_owned());
    }
    Ok(emit)
}

fn unevidenced_target(candidate: impl Into<String>) -> TargetContractRef {
    let candidate = candidate.into();
    for backend_name in backend_names() {
        if candidate == backend_name
            || (backend_name == mncs_model::PORTABLE_WASM_MVP_BACKEND_NAME
                && candidate == "portable-wasm")
            || (backend_name == mncs_codegen::RESEARCH_BYTECODE_BACKEND_NAME
                && candidate == "research-bytecode")
        {
            if let Some(target) = target_for_backend(backend_name) {
                return target;
            }
        }
    }
    TargetContractRef::new(
        candidate,
        BTreeMap::new(),
        Vec::new(),
        vec!["target name is a candidate identifier, not target evidence".to_owned()],
    )
}

fn target_has_backend_adapter(target: &TargetContractRef) -> bool {
    backend_names()
        .into_iter()
        .filter_map(target_for_backend)
        .any(|known| known == *target)
}

fn read_program_unvalidated(path: &str) -> Result<Program, ExitCode> {
    let input = read_source(path)?;
    Program::from_json(&input).map_err(|error| {
        eprintln!("error: invalid semantic manifest: {error}");
        ExitCode::from(2)
    })
}

fn write_compilation_outputs(
    output_dir: &Path,
    result: &CompilationResult,
) -> Result<(), std::io::Error> {
    fs::create_dir_all(output_dir)?;
    write_pretty_json(output_dir.join("result.json"), result)?;
    if let Some(semantic) = &result.emissions.semantic {
        write_pretty_json(output_dir.join("semantic.json"), semantic)?;
    }
    if let Some(hir) = &result.emissions.hir {
        write_pretty_json(output_dir.join("hir.json"), hir)?;
    }
    if let Some(ssa) = &result.emissions.ssa {
        write_pretty_json(output_dir.join("ssa.json"), ssa)?;
    }
    if let Some(plan) = &result.emissions.target_lowering_plan {
        write_pretty_json(output_dir.join("target-plan.json"), plan)?;
    }
    if let Some(backend) = &result.emissions.backend {
        write_pretty_json(output_dir.join("backend.json"), backend)?;
    }
    if let Some(evidence) = &result.evidence {
        write_pretty_json(output_dir.join("evidence.json"), evidence)?;
    }
    Ok(())
}

fn backend_execution_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(program_path) = args.next() else {
        eprintln!("error: execute-backend requires a program and execution request");
        return ExitCode::from(2);
    };
    let Some(request_path) = args.next() else {
        eprintln!("error: execute-backend requires an execution request");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("error: unexpected additional arguments");
        return ExitCode::from(2);
    }
    let program = match read_program_unvalidated(&program_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let request = match read_json::<ExecutionRequest>(&request_path) {
        Ok(request) => request,
        Err(code) => return code,
    };
    let ssa = match program.lower_to_ssa() {
        Ok(ssa) => ssa,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    let selected = selected_ssa_ref(&ssa);
    let lowered = lower_selected_ssa(
        &program,
        &ssa,
        selected.clone(),
        &portable_wasm_plan(selected),
    );
    let Some(artifact) = lowered.artifact else {
        eprintln!("error: portable backend lowering did not produce an artifact");
        let _ = print_json(&lowered);
        return ExitCode::FAILURE;
    };
    let result = execute_backend(&artifact, &request);
    let status = result.status;
    if !print_json(&result) {
        ExitCode::from(2)
    } else {
        execution_status_code(status)
    }
}

fn backend_compare_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(program_path) = args.next() else {
        eprintln!("error: check-backend-execution requires a program and corpus");
        return ExitCode::from(2);
    };
    let Some(corpus_path) = args.next() else {
        eprintln!("error: check-backend-execution requires a corpus");
        return ExitCode::from(2);
    };
    let program = match read_program_unvalidated(&program_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let corpus = match read_json::<ExecutionCorpus>(&corpus_path) {
        Ok(corpus) => corpus,
        Err(code) => return code,
    };
    let ssa = match program.lower_to_ssa() {
        Ok(ssa) => ssa,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    let selected = selected_ssa_ref(&ssa);
    let lowered = lower_selected_ssa(
        &program,
        &ssa,
        selected.clone(),
        &portable_wasm_plan(selected),
    );
    let Some(artifact) = lowered.artifact else {
        eprintln!("error: portable backend lowering did not produce an artifact");
        return ExitCode::FAILURE;
    };
    let comparison = compare_body_ssa_and_backend(&program, &ssa, &artifact, &corpus);
    let ok = comparison.status == mncs_codegen::LayeredExecutionStatus::ConsistentOverCorpus;
    if !print_json(&comparison) {
        ExitCode::from(2)
    } else if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn validate_translation_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(kind) = args.next() else {
        eprintln!("error: validate-translation requires a transformation kind");
        return ExitCode::from(2);
    };
    let Some(program_path) = args.next() else {
        eprintln!("error: validate-translation requires a program");
        return ExitCode::from(2);
    };
    let corpus_path = args.next();
    let program = match read_program_unvalidated(&program_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let before = match program.lower_to_ssa() {
        Ok(ssa) => ssa,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    let result = match kind.as_str() {
        "constant-folding" => {
            validate_constant_folding(&before, &generate::fold_constants(&before))
        }
        "unreachable-removal" => {
            validate_unreachable_removal(&before, &generate::remove_unreachable(&before))
        }
        "branch-simplification" => {
            validate_branch_simplification(&before, &generate::simplify_constant_branches(&before))
        }
        "checked-elision" => {
            let Some(corpus_path) = corpus_path else {
                eprintln!("error: checked-elision requires a corpus");
                return ExitCode::from(2);
            };
            let corpus = match read_json::<ExecutionCorpus>(&corpus_path) {
                Ok(corpus) => corpus,
                Err(code) => return code,
            };
            validate_checked_elision(
                &program,
                &before,
                &generate::rewrite_checked_to_wrapping(&before),
                &corpus,
            )
        }
        "backend-lowering" => {
            let Some(corpus_path) = corpus_path else {
                eprintln!("error: backend-lowering requires a corpus");
                return ExitCode::from(2);
            };
            let corpus = match read_json::<ExecutionCorpus>(&corpus_path) {
                Ok(corpus) => corpus,
                Err(code) => return code,
            };
            let selected = selected_ssa_ref(&before);
            let artifact = match lower_selected_ssa(
                &program,
                &before,
                selected.clone(),
                &portable_wasm_plan(selected),
            )
            .artifact
            {
                Some(artifact) => artifact,
                None => {
                    eprintln!("error: portable backend lowering did not produce an artifact");
                    return ExitCode::FAILURE;
                }
            };
            validate_backend_lowering(&program, &before, &artifact, &corpus)
        }
        other => {
            eprintln!("error: unknown translation kind {other:?}");
            return ExitCode::from(2);
        }
    };
    let judgement = result.judgement;
    if !print_json(&result) {
        ExitCode::from(2)
    } else if judgement == mncs_model::TranslationJudgement::Fail {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, ExitCode> {
    let input = read_source(path)?;
    serde_json::from_str(&input).map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::from(2)
    })
}

fn write_pretty_json(path: PathBuf, value: &impl Serialize) -> Result<(), std::io::Error> {
    let bytes = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    fs::write(path, bytes)
}

fn read_program_for_execution(path: &str) -> Result<Program, ExitCode> {
    let input = read_source(path)?;
    Program::from_json(&input).map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::from(2)
    })
}

fn execution_status_code(status: ExecutionStatus) -> ExitCode {
    match status {
        ExecutionStatus::Returned => ExitCode::SUCCESS,
        ExecutionStatus::InvalidRequest => ExitCode::from(2),
        ExecutionStatus::RuntimeFailure
        | ExecutionStatus::Unsupported
        | ExecutionStatus::BudgetExhausted => ExitCode::FAILURE,
    }
}

#[derive(Debug, Serialize)]
struct DiagnosticReport {
    obligations: Vec<DiagnosticObligation>,
    causal_slices: Vec<CausalSlice>,
}

fn diagnose(path: &str) -> ExitCode {
    let input = match read_source(path) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let program = match Program::from_json(&input) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    let generated = program.generate_obligations();
    let graph = program.semantic_graph().ok();
    let mut diagnostics = Vec::new();
    let mut slices = Vec::new();
    for obligation in generated.obligations {
        if obligation.status == ObligationStatus::Pass {
            continue;
        }
        let mut diagnostic = DiagnosticObligation::new(
            SemanticId(format!("mncs:0.2:diagnostic:{}", obligation.identity.0)),
            obligation.subject.clone(),
            if obligation.method == "language-effect-closure" {
                DiagnosticCategory::Authority
            } else {
                DiagnosticCategory::Verification
            },
        );
        diagnostic.obligation = Some(obligation.identity.clone());
        diagnostic.evidence_state = match obligation.freshness {
            EvidenceFreshness::Stale => EvidenceState::Stale,
            EvidenceFreshness::Current | EvidenceFreshness::Unknown => EvidenceState::Unknown,
        };
        diagnostic.assumptions = obligation.assumptions;
        diagnostic.dependencies = obligation.dependencies;
        diagnostic.severity = if obligation.status == ObligationStatus::Fail {
            3
        } else {
            2
        };
        diagnostic.confidence = if obligation.status == ObligationStatus::Fail {
            Confidence::High
        } else {
            Confidence::Medium
        };
        diagnostic.fallback = obligation.fallback;
        diagnostic.message = format!(
            "{} obligation is {:?}",
            obligation.method, obligation.status
        );
        if let Some(graph) = &graph {
            slices.push(graph.causal_slice(&obligation.subject));
        } else {
            let mut nodes = vec![obligation.subject.clone()];
            nodes.extend(diagnostic.dependencies.iter().cloned());
            nodes.sort();
            nodes.dedup();
            slices.push(CausalSlice {
                schema_version: "0.2".to_owned(),
                root: obligation.subject,
                nodes,
                edges: Vec::new(),
                complete: false,
                limitations: vec![
                    "semantic graph unavailable because the program is invalid".to_owned(),
                    "slice contains dependency identities only".to_owned(),
                ],
            });
        }
        diagnostics.push(diagnostic);
    }
    let failed = !diagnostics.is_empty();
    if !print_json(&DiagnosticReport {
        obligations: diagnostics,
        causal_slices: slices,
    }) {
        ExitCode::from(2)
    } else if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[derive(Debug, Serialize)]
struct DiffReport {
    semantic: SemanticDiff,
    invalidation: mncs_model::InvalidationReport,
}

fn slice_command(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("error: slice requires a manifest path and semantic identity");
        return ExitCode::from(2);
    };
    let Some(identity) = args.next() else {
        eprintln!("error: slice requires a manifest path and semantic identity");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("error: unexpected additional arguments");
        return ExitCode::from(2);
    }
    let program = match read_valid_program(&path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let graph = match program.semantic_graph() {
        Ok(graph) => graph,
        Err(error) => {
            eprintln!("error: unable to construct graph: {error}");
            return ExitCode::from(2);
        }
    };
    if print_json(&graph.causal_slice(&SemanticId(identity))) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

#[derive(Debug, Serialize)]
struct CandidateEvaluationReport {
    candidate: mncs_model::CandidateState,
    evaluation: CandidateEvaluation,
    protected_properties: mncs_model::ProtectedPropertyEvaluation,
}

fn evaluate_candidate_command(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(baseline_path) = args.next() else {
        eprintln!("error: evaluate-candidate requires a baseline and proposal");
        return ExitCode::from(2);
    };
    let Some(proposal_path) = args.next() else {
        eprintln!("error: evaluate-candidate requires a baseline and proposal");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("error: unexpected additional arguments");
        return ExitCode::from(2);
    }
    let baseline = match read_valid_program(&baseline_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let proposal_input = match read_source(&proposal_path) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let proposal: mncs_model::RepairProposal = match serde_json::from_str(&proposal_input) {
        Ok(proposal) => proposal,
        Err(error) => {
            eprintln!("error: invalid repair proposal: {error}");
            return ExitCode::from(2);
        }
    };
    let candidate = match proposal.apply_isolated(&baseline) {
        Ok(candidate) => candidate,
        Err(error) => {
            eprintln!("error: candidate evaluation rejected: {error}");
            return ExitCode::FAILURE;
        }
    };
    let evaluation = candidate.evaluate(&DeterministicVerifier::default());
    let protected = candidate.evaluate_protected_properties(&proposal.protected_properties);
    let success = evaluation
        .verifier_results
        .iter()
        .all(|result| result.status == ObligationStatus::Pass)
        && protected
            .results
            .iter()
            .all(|result| result.status == mncs_model::ProtectedPropertyStatus::Preserved);
    if !print_json(&CandidateEvaluationReport {
        candidate,
        evaluation,
        protected_properties: protected,
    }) {
        ExitCode::from(2)
    } else if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn verify_result_command(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(manifest_path) = args.next() else {
        eprintln!("error: verify-result requires manifest, request, and result paths");
        return ExitCode::from(2);
    };
    let Some(request_path) = args.next() else {
        eprintln!("error: verify-result requires manifest, request, and result paths");
        return ExitCode::from(2);
    };
    let Some(result_path) = args.next() else {
        eprintln!("error: verify-result requires manifest, request, and result paths");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("error: unexpected additional arguments");
        return ExitCode::from(2);
    }
    let program = match read_valid_program(&manifest_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let request_input = match read_source(&request_path) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let result_input = match read_source(&result_path) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let request: mncs_model::VerifierRequest = match serde_json::from_str(&request_input) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("error: invalid verifier request: {error}");
            return ExitCode::from(2);
        }
    };
    let result = match mncs_model::VerifierResult::import_json(
        &result_input,
        &request,
        &program.semantic_identities(),
    ) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("error: verifier result rejected: {error}");
            return ExitCode::FAILURE;
        }
    };
    let success =
        result.status == ObligationStatus::Pass && result.freshness == EvidenceFreshness::Current;
    if !print_json(&result) {
        ExitCode::from(2)
    } else if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn diff(before_path: &str, after_path: &str) -> ExitCode {
    let before = match read_valid_program(before_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let after = match read_valid_program(after_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let semantic = before.semantic_diff(&after);
    let invalidation = match before.invalidation_from(&after) {
        Ok(report) => report,
        Err(_) => {
            eprintln!("error: unable to compute invalidation report");
            return ExitCode::from(2);
        }
    };
    if print_json(&DiffReport {
        semantic,
        invalidation,
    }) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn compare(before_path: &str, after_path: &str) -> ExitCode {
    let before = match read_valid_program(before_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let after = match read_valid_program(after_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    match before.semantic_delta(&after) {
        Ok(delta) if print_json(&delta) => ExitCode::SUCCESS,
        Ok(_) | Err(_) => {
            eprintln!("error: unable to compute semantic delta");
            ExitCode::from(2)
        }
    }
}

fn evidence_check(manifest_path: &str, program_path: &str) -> ExitCode {
    let manifest_input = match read_source(manifest_path) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let manifest: EvidenceManifest = match serde_json::from_str(&manifest_input) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("error: unable to parse evidence manifest: {error}");
            return ExitCode::from(2);
        }
    };
    let program = match read_valid_program(program_path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    match manifest.assess_against(&program) {
        Ok(report) if print_json(&report) => ExitCode::SUCCESS,
        Ok(_) | Err(_) => {
            eprintln!("error: unable to assess evidence freshness");
            ExitCode::from(2)
        }
    }
}

#[derive(Debug, Serialize)]
struct SourceReport {
    path: String,
    metrics: SourceMetrics,
}

fn syntax_metrics(paths: Vec<String>) -> ExitCode {
    if paths.is_empty() {
        eprintln!("error: syntax-metrics requires at least one source path");
        print_usage();
        return ExitCode::from(2);
    }

    let mut reports = Vec::with_capacity(paths.len());
    for path in paths {
        let input = match read_source(&path) {
            Ok(input) => input,
            Err(code) => return code,
        };
        reports.push(SourceReport {
            path,
            metrics: analyze(&input),
        });
    }

    if print_json(&reports) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

#[derive(Debug, Deserialize)]
struct TournamentManifest {
    name: String,
    semantic_claims: Vec<String>,
    candidates: Vec<TournamentCandidate>,
}

#[derive(Debug, Deserialize)]
struct TournamentCandidate {
    name: String,
    role: String,
    path: String,
}

#[derive(Debug, Serialize)]
struct TournamentReport {
    name: String,
    claim_count: usize,
    candidates: Vec<CandidateReport>,
}

#[derive(Debug, Serialize)]
struct CandidateReport {
    name: String,
    role: String,
    path: String,
    lexical_units_per_claim_milli: usize,
    non_whitespace_characters_per_claim_milli: usize,
    metrics: SourceMetrics,
}

fn syntax_tournament(path: &str) -> ExitCode {
    let input = match read_source(path) {
        Ok(input) => input,
        Err(code) => return code,
    };

    let manifest: TournamentManifest = match serde_json::from_str(&input) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("error: unable to parse tournament manifest {path:?}: {error}");
            return ExitCode::from(2);
        }
    };

    if manifest.semantic_claims.is_empty() {
        eprintln!("error: tournament must declare at least one semantic claim");
        return ExitCode::from(2);
    }
    if manifest.candidates.is_empty() {
        eprintln!("error: tournament must declare at least one candidate");
        return ExitCode::from(2);
    }

    let base = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
    let claim_count = manifest.semantic_claims.len();
    let mut candidates = Vec::with_capacity(manifest.candidates.len());

    for candidate in manifest.candidates {
        let resolved_path = base.join(&candidate.path);
        let display_path = resolved_path.to_string_lossy().into_owned();
        let input = match read_source(&display_path) {
            Ok(input) => input,
            Err(code) => return code,
        };
        let metrics = analyze(&input);

        candidates.push(CandidateReport {
            name: candidate.name,
            role: candidate.role,
            path: candidate.path,
            lexical_units_per_claim_milli: per_claim_milli(metrics.lexical_units, claim_count),
            non_whitespace_characters_per_claim_milli: per_claim_milli(
                metrics.non_whitespace_characters,
                claim_count,
            ),
            metrics,
        });
    }

    let report = TournamentReport {
        name: manifest.name,
        claim_count,
        candidates,
    };

    if print_json(&report) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn per_claim_milli(value: usize, claim_count: usize) -> usize {
    value.saturating_mul(1000) / claim_count
}

fn read_source(path: &str) -> Result<String, ExitCode> {
    fs::read_to_string(path).map_err(|error| {
        eprintln!("error: unable to read {path:?}: {error}");
        ExitCode::from(2)
    })
}

fn print_json<T: Serialize>(value: &T) -> bool {
    match serde_json::to_string_pretty(value) {
        Ok(json) => {
            println!("{json}");
            true
        }
        Err(error) => {
            eprintln!("error: unable to serialize report: {error}");
            false
        }
    }
}

fn print_usage() {
    eprintln!("MNCS language research tools");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  mncs validate <manifest.json>");
    eprintln!("  mncs canonicalize <manifest.json>");
    eprintln!("  mncs identity <manifest.json>");
    eprintln!("  mncs graph <manifest.json>");
    eprintln!("  mncs evidence-manifest <manifest.json>");
    eprintln!("  mncs ir <manifest.json>");
    eprintln!("  mncs ssa <manifest.json>");
    eprintln!("  mncs obligations <manifest.json>");
    eprintln!("  mncs verify <manifest.json>");
    eprintln!("  mncs diagnose <manifest.json>");
    eprintln!("  mncs body <manifest.json>");
    eprintln!("  mncs trace <manifest.json>");
    eprintln!("  mncs verifier-request <manifest.json>");
    eprintln!("  mncs execute <program.json> <execution-request.json>");
    eprintln!("  mncs execute-ssa <program.json> <execution-request.json>");
    eprintln!("  mncs compare-execution <baseline.json> <candidate.json> <corpus.json>");
    eprintln!("  mncs check-lowering-execution <program.json> <corpus.json>");
    eprintln!("  mncs compile <program.json> [--emit semantic,hir,ssa,evidence,target-plan,backend] [--output-dir DIR] [--target TARGET]");
    eprintln!("  mncs execute-backend <program.json> <execution-request.json>");
    eprintln!("  mncs check-backend-execution <program.json> <corpus.json>");
    eprintln!("  mncs validate-translation <kind> <program.json> [corpus.json]");
    eprintln!("  mncs compiler-architecture");
    eprintln!("  mncs compiler-study <program.json> [--node-id NODE] [--target TARGET] [--family-reference]");
    eprintln!("  mncs source-study <program.mncs> [--node-id NODE]");
    eprintln!("  mncs experiment plan <program.mncs> --backend BACKEND --corpus CORPUS [--output-dir DIR]");
    eprintln!("  mncs experiment run <program.mncs> --backend BACKEND --corpus CORPUS [--output-dir DIR] [--node-id NODE]");
    eprintln!("  mncs experiment execute <backend-artifact.json> <corpus.json>");
    eprintln!("  mncs experiment inspect <result.json>");
    eprintln!("  mncs experiment compare <left-result.json> <right-result.json>");
    eprintln!("  mncs diff <before.json> <after.json>");
    eprintln!("  mncs compare <before.json> <after.json>");
    eprintln!("  mncs slice <manifest.json> <semantic-identity>");
    eprintln!("  mncs evaluate-candidate <baseline.json> <proposal.json>");
    eprintln!("  mncs verify-result <manifest.json> <request.json> <result.json>");
    eprintln!("  mncs evidence-check <evidence.json> <current.json>");
    eprintln!("  mncs syntax-metrics <source> [source ...]");
    eprintln!("  mncs syntax-tournament <tournament.json>");
}
