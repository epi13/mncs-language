use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::Instant,
};

use mncs_codegen::{
    backend_adapter, backend_capabilities, backend_family_matrix, backend_names,
    compare_body_ssa_and_backend, execute_backend, lower_selected_ssa, portable_wasm_plan,
    selected_ssa_ref, target_for_backend, target_is_portable_wasm, BackendStatefulSession,
};
use mncs_compiler::{
    native_node_profile, reference_compiler_architecture, ModuleResolution, ModuleResolver,
    ReferenceCompiler, SourceFrontEndResult,
};
use mncs_model::{
    compare_body_and_ssa, compare_execution, execute_ssa, execute_with_policy,
    ArtifactRepresentation, CandidateEvaluation, CausalSlice, ComparisonStatus, CompilationResult,
    CompilationStatus, CompilationStudyRequest, CompilationStudyResult, Confidence,
    DeterministicVerifier, DiagnosticCategory, DiagnosticObligation, EvidenceFreshness,
    EvidenceManifest, EvidenceState, ExecutionComparison, ExecutionCorpus, ExecutionProperty,
    ExecutionRequest, ExecutionStatus, ExecutionValue, FunctionBody,
    LanguageExperimentCaseObservation, LanguageExperimentComparison, LanguageExperimentDefinition,
    LanguageExperimentPropertyObservation, LanguageExperimentResult,
    LanguageExperimentStatefulCaseObservation, LoweringExecutionComparison,
    LoweringExecutionStatus, ObligationStatus, Program, RealizationRequest, SemanticDiff,
    SemanticId, SsaModule, StatefulExecutionCase, StatefulExecutionResult, TargetContractRef,
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

/// Bounded summary emitted by `experiment execute --baseline` after executing a
/// frozen backend artifact against a recorded baseline experiment result.
const FROZEN_REPLICATION_SUMMARY_SCHEMA_VERSION: &str = "0.1";
const LANGUAGE_EXPERIMENT_INTERPRETATION: &str = mncs_model::LANGUAGE_EXPERIMENT_INTERPRETATION;

fn trace_timing(stage: &str, cumulative_started: &Instant, stage_started: &mut Instant) {
    let cumulative_elapsed = cumulative_started.elapsed();
    let exclusive_elapsed = stage_started.elapsed();
    mncs_model::record_stage(stage, cumulative_elapsed);
    mncs_model::record_exclusive_stage(stage, exclusive_elapsed);
    if env::var_os("MNCS_TIMINGS").is_some() {
        eprintln!(
            "mncs-timing stage={} cumulative_elapsed_ms={} exclusive_elapsed_ms={}",
            stage,
            cumulative_elapsed.as_millis(),
            exclusive_elapsed.as_millis(),
        );
    }
    *stage_started = Instant::now();
}

const CLI_WORKER_STACK_BYTES: usize = 8 * 1024 * 1024;

fn main() -> ExitCode {
    std::thread::Builder::new()
        .name("mncs-cli".to_owned())
        .stack_size(CLI_WORKER_STACK_BYTES)
        .spawn(run_cli)
        .expect("unable to start bounded CLI worker")
        .join()
        .unwrap_or_else(|_| ExitCode::from(101))
}

fn run_cli() -> ExitCode {
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
        "abi" => abi_command(args),
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
    let program = match load_program(path) {
        Ok(program) => program,
        Err(code) => return code,
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
    let program = load_program(path)?;
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

#[derive(Debug, Serialize)]
struct SourceStudyCommandReport {
    #[serde(flatten)]
    study: CompilationStudyResult,
    module_resolutions: Vec<ModuleResolution>,
    binding_table: Option<mncs_model::SemanticBindingTable>,
    name_resolutions: mncs_compiler::NameResolutionIndex,
}

fn body(path: &str) -> ExitCode {
    let program = match read_valid_program(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let mut functions = Vec::new();
    for function in &program.functions {
        if let Some(body) = &function.body {
            let namespace = function.identity_namespace(&program.module);
            functions.push(BodyFunctionReport {
                name: function.name.clone(),
                identity: body.identity(namespace, &function.name),
                fingerprint: body.fingerprint().expect("canonical body"),
                cfg: mncs_model::Cfg::from_body(body, body.identity(namespace, &function.name)),
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
    let program = match read_program_for_execution(&program_path) {
        Ok(program) => program,
        Err(code) => return code,
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

#[derive(Debug, Serialize)]
struct LanguageOwnedAbi {
    schema_version: String,
    source_artifact_identity: String,
    module: String,
    semantic_fingerprint: Option<String>,
    functions: BTreeMap<String, mncs_codegen::LanguageOwnedFunctionAbi>,
    composites: BTreeMap<String, mncs_model::BackendValueContract>,
}

fn abi_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(source_path) = args.next() else {
        eprintln!("error: abi requires an MNCS source or semantic program path");
        print_usage();
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("error: unexpected abi arguments");
        return ExitCode::from(2);
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
            locator: Some(source_path.clone()),
        },
        source,
    );
    let compiler = ReferenceCompiler::default();
    let program = if Path::new(&source_path)
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        match Program::from_json(&envelope.text) {
            Ok(program) => program,
            Err(error) => {
                eprintln!("error: invalid canonical semantic JSON ABI source: {error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        let resolver = FileModuleResolver::with_libraries(&source_path);
        let front_end = compiler.front_end_with_resolver(envelope.clone(), &resolver);
        let Some(program) = front_end.program.clone().filter(|_| front_end.is_valid()) else {
            let _ = print_json(&front_end);
            return ExitCode::FAILURE;
        };
        program
    };
    let report = program.validate();
    if !report.valid {
        let _ = print_json(&report);
        return ExitCode::FAILURE;
    }
    let (functions, composites) = mncs_codegen::language_owned_abi_contracts(&program);
    let abi = LanguageOwnedAbi {
        schema_version: "0.1".to_owned(),
        source_artifact_identity: envelope.identity,
        module: program.module.clone(),
        semantic_fingerprint: program.content_fingerprint().ok(),
        functions,
        composites,
    };
    if print_json(&abi) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
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
            locator: Some(source_path.clone()),
        },
        source,
    );
    let resolver = FileModuleResolver::with_libraries(source_path.as_str());
    let output = ReferenceCompiler::default().run_source_study_with_resolver(
        envelope,
        native_node_profile(node_identity.as_str()),
        &resolver,
    );
    let front_end = output.front_end;
    match output.study {
        Some(study) => {
            if print_json(&SourceStudyCommandReport {
                study,
                module_resolutions: front_end.module_resolutions,
                binding_table: front_end.binding_table,
                name_resolutions: front_end.name_resolutions,
            }) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        None => {
            if print_json(&front_end) {
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
    validation_profile: Option<String>,
}

struct PreparedExperiment {
    program: Program,
    source_artifact_identity: String,
    source_profile: String,
    corpus: ExecutionCorpus,
    definition: Option<LanguageExperimentDefinition>,
    front_end: Option<SourceFrontEndResult>,
    validation_profile: Option<String>,
}

fn experiment_command<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(action) = args.next() else {
        eprintln!("error: experiment requires plan, run, execute, inspect, compare, refine, rust-control, or matrix (execute accepts --baseline/--output-dir for frozen replication)");
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
            if action == "run" {
                mncs_model::reset_cost_report();
            }
            let prepared = match prepare_experiment(&options, action == "plan") {
                Ok(prepared) => prepared,
                Err(code) => return code,
            };
            if action == "plan" {
                if let Some(output_dir) = &options.output_dir {
                    let definition = prepared.definition.as_ref().expect("plan definition");
                    if let Err(error) = fs::create_dir_all(output_dir).and_then(|_| {
                        write_pretty_json(output_dir.join("definition.json"), definition)
                    }) {
                        eprintln!("error: unable to write experiment plan: {error}");
                        return ExitCode::from(2);
                    }
                }
                return if print_json(prepared.definition.as_ref().expect("plan definition")) {
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
            let mut baseline_path: Option<String> = None;
            let mut output_dir: Option<PathBuf> = None;
            let mut parse_error = false;
            while let Some(option) = args.next() {
                let mut take_value = |what: &str| -> Option<String> {
                    match args.next() {
                        Some(value) => Some(value),
                        None => {
                            eprintln!("error: {what} requires a value");
                            parse_error = true;
                            None
                        }
                    }
                };
                match option.as_str() {
                    "--baseline" => baseline_path = take_value("--baseline"),
                    "--output-dir" => output_dir = take_value("--output-dir").map(PathBuf::from),
                    other => {
                        eprintln!("error: unknown experiment execute option {other:?}");
                        parse_error = true;
                    }
                }
            }
            if parse_error {
                return ExitCode::from(2);
            }
            if baseline_path.is_some() && output_dir.is_none() {
                eprintln!("error: experiment execute --baseline requires --output-dir");
                return ExitCode::from(2);
            }
            if output_dir.is_some() && baseline_path.is_none() {
                eprintln!("error: experiment execute --output-dir requires --baseline");
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
                    experiment_case_observation(case_, observation)
                })
                .collect::<Vec<_>>();
            let mut stateful_session = BackendStatefulSession::new(&artifact);
            let stateful_executions =
                stateful_session.execute_cases(&corpus.name, &corpus.stateful_cases);
            let stateful_observations = corpus
                .stateful_cases
                .iter()
                .zip(stateful_executions)
                .map(|(case_, execution)| stateful_case_observation(case_, execution))
                .collect::<Vec<_>>();
            if baseline_path.is_some() && output_dir.is_none() {
                eprintln!("error: experiment execute --baseline requires --output-dir");
                return ExitCode::from(2);
            }
            if let (Some(baseline_file), Some(dir)) = (baseline_path, output_dir) {
                return execute_frozen_replication(
                    &artifact,
                    &corpus,
                    observations,
                    stateful_observations,
                    &baseline_file,
                    &dir,
                );
            }
            let invalid = observations.iter().any(|observation| {
                matches!(
                    observation.status,
                    ExecutionStatus::InvalidRequest | ExecutionStatus::Unsupported
                ) || observation.expectation_met == Some(false)
                    || observation.status_met == Some(false)
                    || observation.step_bound_met == Some(false)
                    || observation.effects_met == Some(false)
            }) || stateful_observations.iter().any(|observation| {
                matches!(
                    observation.execution.status,
                    ExecutionStatus::InvalidRequest | ExecutionStatus::Unsupported
                ) || observation.final_expectation_met == Some(false)
                    || observation.final_status_met == Some(false)
                    || !observation.step_expectations_met
                    || !observation.call_bound_met
                    || observation.step_bound_met == Some(false)
            });
            let output = if stateful_observations.is_empty() {
                serde_json::json!(observations)
            } else {
                serde_json::json!({
                    "cases": observations,
                    "stateful_cases": stateful_observations,
                })
            };
            if !print_json(&output) {
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
        "matrix" => {
            if args.next().is_some() {
                eprintln!("error: experiment matrix does not accept arguments");
                return ExitCode::from(2);
            }
            if print_json(&backend_family_matrix()) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
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
        "refine" => {
            let Some(baseline_path) = args.next() else {
                eprintln!("error: experiment refine requires a baseline and at least one candidate result");
                return ExitCode::from(2);
            };
            let mut candidate_paths = Vec::new();
            let mut budget = 4_u32;
            while let Some(argument) = args.next() {
                if argument == "--budget" {
                    let Some(value) = args.next() else {
                        eprintln!("error: --budget requires an integer");
                        return ExitCode::from(2);
                    };
                    budget = match value.parse() {
                        Ok(value) => value,
                        Err(_) => {
                            eprintln!("error: --budget requires an integer");
                            return ExitCode::from(2);
                        }
                    };
                } else {
                    candidate_paths.push(argument);
                }
            }
            if candidate_paths.is_empty() {
                eprintln!("error: experiment refine requires at least one candidate result");
                return ExitCode::from(2);
            }
            let baseline = match read_json::<LanguageExperimentResult>(&baseline_path) {
                Ok(result) => result,
                Err(code) => return code,
            };
            let mut candidates = Vec::new();
            for path in candidate_paths {
                match read_json::<LanguageExperimentResult>(&path) {
                    Ok(result) => candidates.push(result),
                    Err(code) => return code,
                }
            }
            let cycle =
                match mncs_model::BoundedRefinementCycle::compare(&baseline, &candidates, budget) {
                    Ok(cycle) => cycle,
                    Err(error) => {
                        eprintln!("error: bounded refinement refused: {error}");
                        return ExitCode::FAILURE;
                    }
                };
            if print_json(&cycle) && cycle.identity_is_valid() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        "rust-control" => {
            let (Some(result_path), Some(control_path)) = (args.next(), args.next()) else {
                eprintln!(
                    "error: experiment rust-control requires a result and Rust control source"
                );
                return ExitCode::from(2);
            };
            if args.next().is_some() {
                eprintln!("error: unexpected experiment rust-control arguments");
                return ExitCode::from(2);
            }
            rust_control_comparison(&result_path, &control_path)
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
    let mut validation_profile = None;
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
            "--validation-profile" => {
                let profile = args
                    .next()
                    .ok_or_else(|| "--validation-profile requires a value".to_owned())?;
                if profile != "artifact-build" {
                    return Err(format!("unknown validation profile {profile:?}"));
                }
                validation_profile = Some(profile);
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
        validation_profile,
    })
}

#[derive(Debug, Deserialize)]
struct RustControlOutput {
    status: String,
    value: Option<i128>,
}

#[derive(Debug, Serialize)]
struct RustControlCaseComparison {
    case_id: String,
    backend_status: ExecutionStatus,
    backend_returned: Vec<ExecutionValue>,
    control_status: String,
    control_value: Option<i128>,
    agrees: bool,
}

#[derive(Debug, Serialize)]
struct RustControlComparison {
    schema_version: String,
    identity: SemanticId,
    experiment_result: SemanticId,
    backend_artifact: SemanticId,
    control_source_identity: String,
    rustc_version: String,
    cases: Vec<RustControlCaseComparison>,
    bounded_agreement: bool,
    interpretation: String,
}

fn rust_control_comparison(result_path: &str, control_path: &str) -> ExitCode {
    let result = match read_json::<LanguageExperimentResult>(result_path) {
        Ok(result) if result.identity_is_valid() => result,
        Ok(_) => {
            eprintln!("error: experiment result identity is invalid or stale");
            return ExitCode::FAILURE;
        }
        Err(code) => return code,
    };
    let control_source = match read_source(control_path) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let control_envelope = SourceEnvelope::new(
        SourceArtifactKind::Program,
        control_path.to_owned(),
        SourceOrigin {
            kind: SourceOriginKind::Path,
            locator: Some(control_path.to_owned()),
        },
        control_source,
    );
    let rustc_version = match Command::new("rustc").arg("--version").output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }
        Ok(output) => {
            eprintln!(
                "error: rustc --version failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return ExitCode::FAILURE;
        }
        Err(error) => {
            eprintln!("error: rustc is unavailable for the control comparison: {error}");
            return ExitCode::FAILURE;
        }
    };
    let binary_path = env::temp_dir().join(format!("mncs-rust-control-{}", std::process::id()));
    let compiled = Command::new("rustc")
        .args([control_path, "-O", "-o"])
        .arg(&binary_path)
        .output();
    let compiled = match compiled {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            eprintln!(
                "error: Rust control compilation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return ExitCode::FAILURE;
        }
        Err(error) => {
            eprintln!("error: unable to execute rustc: {error}");
            return ExitCode::FAILURE;
        }
    };
    let _ = compiled;
    let mut cases = Vec::new();
    for corpus_case in &result.definition.corpus.cases {
        let Some(backend_case) = result
            .cases
            .iter()
            .find(|case_| case_.case_id == corpus_case.id)
        else {
            let _ = fs::remove_file(&binary_path);
            eprintln!("error: result omitted corpus case {:?}", corpus_case.id);
            return ExitCode::FAILURE;
        };
        let mut command = Command::new(&binary_path);
        command.arg(&corpus_case.request.target.function);
        for argument in &corpus_case.request.arguments {
            let ExecutionValue::Integer { value, .. } = argument else {
                let _ = fs::remove_file(&binary_path);
                eprintln!("error: Rust control pilot supports integer arguments only");
                return ExitCode::FAILURE;
            };
            command.arg(value.to_string());
        }
        let output = match command.output() {
            Ok(output) if output.status.success() => output,
            Ok(output) => {
                let _ = fs::remove_file(&binary_path);
                eprintln!(
                    "error: Rust control failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return ExitCode::FAILURE;
            }
            Err(error) => {
                let _ = fs::remove_file(&binary_path);
                eprintln!("error: unable to execute Rust control: {error}");
                return ExitCode::FAILURE;
            }
        };
        let control: RustControlOutput = match serde_json::from_slice(&output.stdout) {
            Ok(control) => control,
            Err(error) => {
                let _ = fs::remove_file(&binary_path);
                eprintln!("error: Rust control emitted invalid JSON: {error}");
                return ExitCode::FAILURE;
            }
        };
        let backend_value = match backend_case.returned.as_slice() {
            [ExecutionValue::Integer { value, .. }] => Some(*value),
            _ => None,
        };
        let expected_control_status = match backend_case.status {
            ExecutionStatus::Returned => "returned",
            ExecutionStatus::RuntimeFailure => "runtime_failure",
            ExecutionStatus::Unsupported => "unsupported",
            ExecutionStatus::BudgetExhausted => "budget_exhausted",
            ExecutionStatus::InvalidRequest => "invalid_request",
        };
        let agrees = control.status == expected_control_status && control.value == backend_value;
        cases.push(RustControlCaseComparison {
            case_id: corpus_case.id.clone(),
            backend_status: backend_case.status,
            backend_returned: backend_case.returned.clone(),
            control_status: control.status,
            control_value: control.value,
            agrees,
        });
    }
    let _ = fs::remove_file(&binary_path);
    let bounded_agreement = cases.iter().all(|case_| case_.agrees);
    let identity_material = serde_json::to_string(&(
        &result.identity,
        &result.artifact.identity,
        &control_envelope.identity,
        &rustc_version,
        &cases,
    ))
    .expect("Rust control comparison is serializable");
    let identity = SourceEnvelope::new(
        SourceArtifactKind::VerificationSystem,
        "rust-control-comparison",
        SourceOrigin {
            kind: SourceOriginKind::Generated,
            locator: None,
        },
        identity_material,
    )
    .identity;
    let report = RustControlComparison {
        schema_version: "0.1".to_owned(),
        identity: SemanticId(identity),
        experiment_result: result.identity,
        backend_artifact: result.artifact.identity,
        control_source_identity: control_envelope.identity,
        rustc_version,
        cases,
        bounded_agreement,
        interpretation: "finite generated-backend/Rust-control agreement is bounded empirical evidence, not proof or universal equivalence".to_owned(),
    };
    if !print_json(&report) {
        ExitCode::from(2)
    } else if bounded_agreement {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn prepare_experiment(
    options: &ExperimentOptions,
    include_definition: bool,
) -> Result<PreparedExperiment, ExitCode> {
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
    let semantic_json_source = Path::new(&options.source_path)
        .extension()
        .is_some_and(|extension| extension == "json");
    let (program, front_end) = if semantic_json_source {
        let program = Program::from_json(&envelope.text).map_err(|error| {
            eprintln!("error: invalid canonical semantic JSON experiment source: {error}");
            ExitCode::FAILURE
        })?;
        let report = program.validate();
        if !report.valid {
            let _ = print_json(&report);
            return Err(ExitCode::FAILURE);
        }
        (program, None)
    } else {
        let resolver = FileModuleResolver::with_libraries(&options.source_path);
        let front_end = compiler.front_end_with_resolver(envelope.clone(), &resolver);
        let Some(program) = front_end.program.clone().filter(|_| front_end.is_valid()) else {
            let _ = print_json(&front_end);
            return Err(ExitCode::FAILURE);
        };
        (program, Some(front_end))
    };
    let corpus = read_json::<ExecutionCorpus>(&options.corpus_path)?;
    let source_artifact_identity = envelope.identity;
    let source_profile = envelope.language_version;
    let definition = if include_definition {
        Some(build_experiment_definition(
            &options.backend,
            &options.source_path,
            source_artifact_identity.clone(),
            source_profile.clone(),
            compiler.identity.identity.clone(),
            corpus.clone(),
            &program.lower_to_ssa().map_err(|error| {
                eprintln!("error: unable to produce selected SSA for experiment: {error}");
                ExitCode::FAILURE
            })?,
        )?)
    } else {
        None
    };
    Ok(PreparedExperiment {
        program,
        source_artifact_identity,
        source_profile,
        corpus,
        definition,
        front_end,
        validation_profile: options.validation_profile.clone(),
    })
}

fn build_experiment_definition(
    backend_name: &str,
    source_path: &str,
    source_artifact_identity: String,
    source_profile: String,
    compiler_identity: SemanticId,
    corpus: ExecutionCorpus,
    ssa: &SsaModule,
) -> Result<LanguageExperimentDefinition, ExitCode> {
    let selected = selected_ssa_ref(ssa);
    let adapter = backend_adapter(backend_name).expect("validated by option parser");
    let capabilities = adapter.capabilities();
    let realization = RealizationRequest::new(
        selected,
        adapter.target(),
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
    Ok(LanguageExperimentDefinition::new(
        Path::new(source_path)
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("mncs-experiment"),
        source_artifact_identity,
        source_profile,
        compiler_identity,
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
            "stateful_case_observations".to_owned(),
        ],
        vec![
            "source_to_ssa_identity_chain_is_exact".to_owned(),
            "FAIL_dominates_UNKNOWN_dominates_PASS".to_owned(),
        ],
        "bounded_public_behavior_agreement",
        None,
        Vec::new(),
    ))
}

fn run_experiment(options: ExperimentOptions, prepared: PreparedExperiment) -> ExitCode {
    let started = Instant::now();
    let mut stage_started = started;
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
    let compilation = compiler.compile(request.clone(), &prepared.program);
    if compilation.status == CompilationStatus::Failed {
        let _ = print_json(&compilation);
        return ExitCode::FAILURE;
    }
    trace_timing("cli-compile", &started, &mut stage_started);
    let study_request = CompilationStudyRequest::new(
        native_node_profile(options.node_identity),
        request,
        Vec::new(),
    );
    let (Some(ssa), Some(plan), Some(artifact)) = (
        compilation.emissions.ssa.as_ref(),
        compilation.emissions.target_lowering_plan.clone(),
        compilation.emissions.backend.clone(),
    ) else {
        let _ = print_json(&compilation);
        return ExitCode::FAILURE;
    };
    let definition = match build_experiment_definition(
        &options.backend,
        &options.source_path,
        prepared.source_artifact_identity.clone(),
        prepared.source_profile.clone(),
        compiler.identity.identity.clone(),
        prepared.corpus.clone(),
        ssa,
    ) {
        Ok(definition) => definition,
        Err(code) => return code,
    };
    trace_timing("cli-definition", &started, &mut stage_started);
    let study = prepared
        .front_end
        .as_ref()
        .map(|front_end| {
            compiler.with_front_end_observations(
                compiler.study_from_compilation(study_request.clone(), &compilation),
                front_end,
            )
        })
        .unwrap_or_else(|| compiler.study_from_compilation(study_request, &compilation));
    let validation = prepared
        .validation_profile
        .is_none()
        .then(|| validate_backend_lowering(&prepared.program, ssa, &artifact, &definition.corpus));
    trace_timing("cli-translation-validation", &started, &mut stage_started);
    let cases = prepared
        .corpus
        .cases
        .iter()
        .map(|case_| {
            let observation = execute_backend(&artifact, &case_.request);
            experiment_case_observation(case_, observation)
        })
        .collect();
    trace_timing("cli-case-observations", &started, &mut stage_started);
    let mut stateful_session = BackendStatefulSession::new(&artifact);
    let stateful_executions =
        stateful_session.execute_cases(&prepared.corpus.name, &prepared.corpus.stateful_cases);
    let stateful_cases = prepared
        .corpus
        .stateful_cases
        .iter()
        .zip(stateful_executions)
        .map(|(case_, execution)| stateful_case_observation(case_, execution))
        .collect::<Vec<_>>();
    trace_timing(
        "cli-stateful-case-observations",
        &started,
        &mut stage_started,
    );
    let mut unresolved = Vec::new();
    if prepared.validation_profile.as_deref() == Some("artifact-build") {
        unresolved.push(
            "layered body/SSA/backend validation is deferred to the differential corpus job"
                .to_owned(),
        );
    }
    if compilation.status == CompilationStatus::CompletedWithUnresolvedObligations {
        let exact_cost_only = study.unresolved_obligations.iter().all(|identity| {
            ssa.obligations.iter().any(|obligation| {
                obligation.identity == *identity
                    && obligation.method == "no-exact-body-cost-evidence"
            })
        });
        if !exact_cost_only {
            unresolved.push("compilation retained required unresolved obligations".to_owned());
        }
    }
    let capabilities = backend_capabilities(&options.backend).expect("validated backend");
    let properties = evaluate_execution_properties(&artifact, &definition.corpus);
    trace_timing("cli-properties", &started, &mut stage_started);
    let result = LanguageExperimentResult::new(
        definition,
        study,
        capabilities,
        plan,
        artifact,
        validation.into_iter().collect(),
        cases,
        stateful_cases,
        properties,
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

/// Execute an already-frozen backend artifact as a replication of a recorded
/// baseline experiment result.  The frozen realization is never recompiled and
/// every identity mismatch fails closed.
fn execute_frozen_replication(
    artifact: &mncs_model::BackendArtifact,
    corpus: &ExecutionCorpus,
    case_observations: Vec<LanguageExperimentCaseObservation>,
    stateful_observations: Vec<LanguageExperimentStatefulCaseObservation>,
    baseline_path: &str,
    output_dir: &Path,
) -> ExitCode {
    let baseline = match read_json::<LanguageExperimentResult>(baseline_path) {
        Ok(baseline) => baseline,
        Err(code) => return code,
    };
    if !baseline.identity_is_valid() {
        eprintln!("error: baseline experiment result identity is invalid; refusing to replicate");
        return ExitCode::from(2);
    }
    if *corpus != baseline.definition.corpus {
        eprintln!(
            "error: corpus does not match the frozen experiment definition corpus; \
             refusing to replicate with substituted inputs"
        );
        return ExitCode::from(2);
    }
    if *artifact != baseline.artifact {
        eprintln!(
            "error: backend artifact does not match the frozen realization recorded by \
             the baseline result; refusing to substitute another artifact"
        );
        return ExitCode::from(2);
    }
    let properties = evaluate_execution_properties(artifact, &baseline.definition.corpus);
    let replicated = LanguageExperimentResult::new(
        baseline.definition.clone(),
        baseline.compiler_study.clone(),
        baseline.backend_capabilities.clone(),
        baseline.realization_plan.clone(),
        artifact.clone(),
        baseline.translation_validations.clone(),
        case_observations,
        stateful_observations,
        properties,
        baseline.unresolved_reasons.clone(),
    );
    if !replicated.identity_is_valid() {
        eprintln!("error: replicated experiment result identity is invalid");
        return ExitCode::from(2);
    }
    if let Err(error) = fs::create_dir_all(output_dir) {
        eprintln!("error: unable to create replication output directory: {error}");
        return ExitCode::from(2);
    }
    if let Err(error) = write_pretty_json(output_dir.join("replicated-result.json"), &replicated) {
        eprintln!("error: unable to write replicated result: {error}");
        return ExitCode::from(2);
    }
    if let Err(error) = write_pretty_json(
        output_dir.join("replicated-family-reference.json"),
        &replicated.family_reference(),
    ) {
        eprintln!("error: unable to write replicated family reference: {error}");
        return ExitCode::from(2);
    }
    let cases_matching_baseline = baseline
        .cases
        .iter()
        .filter(|baseline_case| replicated.cases.iter().any(|case_| case_ == *baseline_case))
        .count();
    let properties_matching_baseline = baseline
        .properties
        .iter()
        .filter(|baseline_property| {
            replicated
                .properties
                .iter()
                .any(|property| property == *baseline_property)
        })
        .count();
    let bounded_behavior_agrees = cases_matching_baseline == baseline.cases.len()
        && properties_matching_baseline == baseline.properties.len()
        && replicated.cases.len() == baseline.cases.len()
        && replicated.properties.len() == baseline.properties.len();
    let stateful_cases_matching_baseline = baseline
        .stateful_cases
        .iter()
        .filter(|baseline_case| {
            replicated
                .stateful_cases
                .iter()
                .any(|case_| case_ == *baseline_case)
        })
        .count();
    let stateful_behavior_agrees = stateful_cases_matching_baseline
        == baseline.stateful_cases.len()
        && replicated.stateful_cases.len() == baseline.stateful_cases.len();
    let earliest_case_divergence = replicated
        .cases
        .iter()
        .zip(baseline.cases.iter())
        .find(|(observed, expected)| observed != expected)
        .map(|(observed, _)| observed.case_id.clone());
    let bounded_behavior_agrees = bounded_behavior_agrees && stateful_behavior_agrees;
    let summary = FrozenReplicationSummary {
        schema_version: FROZEN_REPLICATION_SUMMARY_SCHEMA_VERSION.to_owned(),
        baseline_result_identity: baseline.identity.clone(),
        replicated_result_identity: replicated.identity.clone(),
        definition_identity: replicated.definition_identity.clone(),
        backend_identity: replicated.backend.clone(),
        backend_artifact_identity: replicated.artifact.identity.clone(),
        identity_valid: true,
        status: replicated.status,
        case_count: replicated.cases.len(),
        property_count: replicated.properties.len(),
        stateful_case_count: replicated.stateful_cases.len(),
        cases_matching_baseline,
        properties_matching_baseline,
        stateful_cases_matching_baseline,
        bounded_behavior_agrees,
        earliest_case_divergence,
        comparison: LanguageExperimentComparison::compare(&baseline, &replicated),
        interpretation: LANGUAGE_EXPERIMENT_INTERPRETATION.to_owned(),
    };
    let printed = print_json(&summary);
    if !printed {
        ExitCode::from(2)
    } else if !bounded_behavior_agrees
        || replicated.status == mncs_model::LanguageExperimentStatus::Fail
    {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrozenReplicationSummary {
    schema_version: String,
    baseline_result_identity: SemanticId,
    replicated_result_identity: SemanticId,
    definition_identity: SemanticId,
    backend_identity: mncs_model::BackendIdentity,
    backend_artifact_identity: SemanticId,
    identity_valid: bool,
    status: mncs_model::LanguageExperimentStatus,
    case_count: usize,
    property_count: usize,
    stateful_case_count: usize,
    cases_matching_baseline: usize,
    properties_matching_baseline: usize,
    stateful_cases_matching_baseline: usize,
    bounded_behavior_agrees: bool,
    earliest_case_divergence: Option<String>,
    comparison: LanguageExperimentComparison,
    interpretation: String,
}

fn experiment_case_observation(
    case_: &mncs_model::ExecutionCase,
    observation: mncs_codegen::BackendExecutionResult,
) -> LanguageExperimentCaseObservation {
    let expectation_met = case_
        .expected
        .as_ref()
        .map(|expected| expected == &observation.returned);
    let status_met = case_
        .expected_status
        .map(|expected| expected == observation.status);
    let step_bound_met = case_
        .maximum_steps
        .map(|maximum| observation.steps <= maximum);
    let expected_effects = case_
        .expected_effects
        .iter()
        .map(|effect| {
            (
                effect.kind.clone(),
                effect.target.clone(),
                effect.capability.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    let actual_effects = observation
        .effects
        .iter()
        .map(|effect| {
            (
                effect.kind.clone(),
                effect.target.clone(),
                effect.capability.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    let effects_met = if case_.expected_effects.is_empty() && !case_.prohibit_unexpected_effects {
        None
    } else if case_.prohibit_unexpected_effects {
        Some(actual_effects == expected_effects)
    } else {
        Some(expected_effects.is_subset(&actual_effects))
    };
    LanguageExperimentCaseObservation {
        case_id: case_.id.clone(),
        status: observation.status,
        returned: observation.returned,
        expected: case_.expected.clone(),
        expectation_met,
        expected_status: case_.expected_status,
        status_met,
        steps: observation.steps,
        maximum_steps: case_.maximum_steps,
        step_bound_met,
        effects: observation.effects,
        expected_effects: case_.expected_effects.clone(),
        effects_met,
        failure_reason: observation.failure.map(|failure| failure.reason),
    }
}

fn stateful_case_observation(
    case_: &StatefulExecutionCase,
    execution: StatefulExecutionResult,
) -> LanguageExperimentStatefulCaseObservation {
    let final_expectation_met = case_
        .expected_final
        .as_ref()
        .map(|expected| expected == &execution.returned);
    let final_status_met = case_
        .expected_final_status
        .map(|expected| expected == execution.status);
    let step_expectations_met = case_.steps.len() == execution.observations.len()
        && case_
            .steps
            .iter()
            .zip(&execution.observations)
            .all(|(step, observation)| {
                step.expected_status
                    .map(|expected| expected == observation.status)
                    .unwrap_or(true)
                    && step
                        .expected
                        .as_ref()
                        .map(|expected| observation.returned.as_ref() == Some(expected))
                        .unwrap_or(true)
            });
    let call_bound_met = execution.calls <= case_.maximum_calls;
    let step_bound_met = case_
        .maximum_total_steps
        .map(|maximum| execution.steps <= maximum);
    LanguageExperimentStatefulCaseObservation {
        case_id: case_.id.clone(),
        execution,
        expected_final: case_.expected_final.clone(),
        final_expectation_met,
        expected_final_status: case_.expected_final_status,
        final_status_met,
        step_expectations_met,
        maximum_calls: case_.maximum_calls,
        call_bound_met,
        maximum_total_steps: case_.maximum_total_steps,
        step_bound_met,
    }
}

fn evaluate_execution_properties(
    artifact: &mncs_model::BackendArtifact,
    corpus: &ExecutionCorpus,
) -> Vec<LanguageExperimentPropertyObservation> {
    let Some(template) = corpus.cases.first().map(|case_| case_.request.clone()) else {
        return Vec::new();
    };
    corpus
        .properties
        .iter()
        .map(|property| evaluate_execution_property(artifact, &template, property))
        .collect()
}

fn evaluate_execution_property(
    artifact: &mncs_model::BackendArtifact,
    template: &ExecutionRequest,
    property: &ExecutionProperty,
) -> LanguageExperimentPropertyObservation {
    let run = |arguments: Vec<ExecutionValue>| -> Result<Vec<ExecutionValue>, String> {
        let mut request = template.clone();
        request.arguments = arguments;
        let result = execute_backend(artifact, &request);
        if result.status == ExecutionStatus::Returned {
            Ok(result.returned)
        } else {
            Err(result
                .failure
                .map(|failure| failure.reason)
                .unwrap_or_else(|| format!("execution ended with {:?}", result.status)))
        }
    };
    let binary =
        |left: &ExecutionValue, right: &ExecutionValue| -> Result<ExecutionValue, String> {
            let returned = run(vec![left.clone(), right.clone()])?;
            if returned.len() == 1 {
                Ok(returned[0].clone())
            } else {
                Err("property target must return exactly one value".to_owned())
            }
        };
    let mut counterexample = None;
    let mut failure_reason = None;
    let mut check = |condition: bool, values: Vec<ExecutionValue>, reason: &str| {
        if !condition && counterexample.is_none() {
            counterexample = Some(values);
            failure_reason = Some(reason.to_owned());
        }
    };
    match property.law.as_str() {
        "commutative" => {
            for left in &property.values {
                for right in &property.values {
                    match (binary(left, right), binary(right, left)) {
                        (Ok(forward), Ok(reverse)) => check(
                            forward == reverse,
                            vec![left.clone(), right.clone()],
                            "f(a,b) != f(b,a)",
                        ),
                        (Err(reason), _) | (_, Err(reason)) => {
                            check(false, vec![left.clone(), right.clone()], &reason)
                        }
                    }
                }
            }
        }
        "associative" => {
            for left in &property.values {
                for middle in &property.values {
                    for right in &property.values {
                        let outcome = binary(left, middle)
                            .and_then(|first| binary(&first, right))
                            .and_then(|lhs| {
                                binary(middle, right)
                                    .and_then(|second| binary(left, &second))
                                    .map(|rhs| (lhs, rhs))
                            });
                        match outcome {
                            Ok((lhs, rhs)) => check(
                                lhs == rhs,
                                vec![left.clone(), middle.clone(), right.clone()],
                                "f(f(a,b),c) != f(a,f(b,c))",
                            ),
                            Err(reason) => check(
                                false,
                                vec![left.clone(), middle.clone(), right.clone()],
                                &reason,
                            ),
                        }
                    }
                }
            }
        }
        "idempotent" => {
            for value in &property.values {
                match binary(value, value) {
                    Ok(returned) => check(returned == *value, vec![value.clone()], "f(a,a) != a"),
                    Err(reason) => check(false, vec![value.clone()], &reason),
                }
            }
        }
        "neutral" | "absorbing" | "preserved" => {
            let Some(distinguished) = property.distinguished.as_ref() else {
                check(false, Vec::new(), "this law requires a distinguished value");
                return LanguageExperimentPropertyObservation {
                    property_id: property.id.clone(),
                    law: property.law.clone(),
                    passed: false,
                    counterexample,
                    failure_reason,
                };
            };
            for value in &property.values {
                if property.exclusions.contains(value) {
                    continue;
                }
                let expected = if property.law == "neutral" {
                    value
                } else {
                    distinguished
                };
                match (binary(distinguished, value), binary(value, distinguished)) {
                    (Ok(left), Ok(right)) => check(
                        left == *expected && right == *expected,
                        vec![value.clone()],
                        "distinguished-value law failed on the left or right",
                    ),
                    (Err(reason), _) | (_, Err(reason)) => {
                        check(false, vec![value.clone()], &reason)
                    }
                }
            }
        }
        _ => check(false, Vec::new(), "unknown bounded execution property law"),
    }
    LanguageExperimentPropertyObservation {
        property_id: property.id.clone(),
        law: property.law.clone(),
        passed: counterexample.is_none(),
        counterexample,
        failure_reason,
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
    write_pretty_json(
        output_dir.join("family-reference.json"),
        &result.family_reference(),
    )?;
    write_pretty_json(output_dir.join("result.json"), result)?;
    write_pretty_json(
        output_dir.join("cost-report.json"),
        &mncs_model::cost_report(),
    )?;
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
    property_count: usize,
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
            property_count: result.properties.len(),
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

/// Load a semantic program from either canonical manifest JSON or, when the
/// path ends in `.mncs`, directly from MNCS source text via the reference
/// front end.
fn load_program(path: &str) -> Result<Program, ExitCode> {
    if Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mncs"))
    {
        let source = read_source(path)?;
        let envelope = mncs_syntax::SourceEnvelope::inline(
            mncs_syntax::SourceArtifactKind::Program,
            path,
            source,
        );
        let resolver = FileModuleResolver::with_libraries(path);
        let front_end = ReferenceCompiler::default().front_end_with_resolver(envelope, &resolver);
        let diagnostics = front_end.diagnostics.clone();
        let valid = front_end.is_valid();
        let program = front_end.program;
        match program.filter(|_| valid) {
            Some(program) => return Ok(program),
            None => {
                let _ = print_json(&diagnostics);
                return Err(ExitCode::from(2));
            }
        }
    }
    let input = read_source(path)?;
    Program::from_json(&input).map_err(|error| {
        eprintln!("error: invalid semantic manifest: {error}");
        ExitCode::from(2)
    })
}

fn read_program_unvalidated(path: &str) -> Result<Program, ExitCode> {
    load_program(path)
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
    // Source programs take the same elaboration bridge as every other
    // manifest-consuming command.
    if Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mncs"))
    {
        return load_program(path);
    }
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

/// Resolves imported module names against the directory of the root source
/// file using the dotted-path convention: `use a.b;` loads `<root>/a/b.mncs`.
/// Additional standard-library roots come from `MNCS_LIBRARY_PATH`
/// (`:`-separated directories searched after the source-local roots), so
/// external consumers can bind to `mncs.core.*` without vendoring the library
/// tree. The name identifies the candidate only; compatibility is established
/// by elaborating the resolved module.
struct FileModuleResolver {
    root: PathBuf,
    libraries: Vec<PathBuf>,
}

impl FileModuleResolver {
    fn for_source(source_path: &str) -> Self {
        let mut root = PathBuf::from(source_path);
        root.pop();
        Self {
            root,
            libraries: Vec::new(),
        }
    }

    /// Source-local resolution plus every configured standard-library root.
    fn with_libraries(source_path: &str) -> Self {
        let mut resolver = Self::for_source(source_path);
        resolver.libraries = library_roots();
        resolver
    }
}

/// Directories that may satisfy `use` targets beyond the importing file's own
/// directory tree. Deterministic order; missing entries are skipped so a stale
/// path degrades into an honest resolution miss instead of an error.
fn library_roots() -> Vec<PathBuf> {
    std::env::var("MNCS_LIBRARY_PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|entry| !entry.is_empty())
        .map(PathBuf::from)
        .collect()
}

impl FileModuleResolver {
    /// Candidate paths for one module name under one root directory:
    /// the full dotted path, then a sibling named after the final segment.
    fn candidates(root: &std::path::Path, module: &str) -> Vec<PathBuf> {
        let dotted = module.replace('.', "/");
        let tail = module.rsplit('.').next().unwrap_or(module);
        let mut paths = vec![root.join(format!("{dotted}.mncs"))];
        let version_tailed = Self::split_version_tail(module);
        if let Some((head, _)) = version_tailed {
            paths.push(root.join(format!("{}.mncs", head.replace('.', "/"))));
        }
        if let Some(rest) = module.strip_prefix("mncs.") {
            paths.push(root.join(format!("{}.mncs", rest.replace('.', "/"))));
            if let Some((head, _)) = version_tailed {
                if let Some(stripped) = head.strip_prefix("mncs.") {
                    paths.push(root.join(format!("{}.mncs", stripped.replace('.', "/"))));
                }
            }
        }
        paths.push(root.join(format!("{tail}.mncs")));
        paths
    }

    fn split_version_tail(module: &str) -> Option<(&str, &str)> {
        let (head, last) = module.rsplit_once('.')?;
        let digits = last.strip_prefix('v')?;
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        if head.is_empty() || head.contains("..") {
            return None;
        }
        Some((head, last))
    }
}

impl ModuleResolver for FileModuleResolver {
    fn resolve(&self, module: &str) -> Option<SourceEnvelope> {
        if module.is_empty()
            || !module
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
            || module.starts_with('.')
            || module.ends_with('.')
            || module.contains("..")
        {
            return None;
        }
        // Deterministic layout search: the importing file's directory first,
        // then its parent (so files grouped in a subdirectory can import a
        // sibling tree rooted one level up), then each configured
        // standard-library root in order.
        let mut roots = vec![self.root.clone()];
        if let Some(parent) = self.root.parent() {
            roots.push(parent.to_path_buf());
        }
        roots.extend(self.libraries.iter().cloned());
        for root in roots {
            for path in Self::candidates(&root, module) {
                if let Ok(source) = fs::read_to_string(&path) {
                    let Some(declared) = mncs_syntax::declared_module_name(&source) else {
                        continue;
                    };
                    if !mncs_syntax::module_names_compatible(module, &declared) {
                        continue;
                    }
                    return Some(SourceEnvelope::new(
                        SourceArtifactKind::Program,
                        path.to_string_lossy().to_string(),
                        SourceOrigin {
                            kind: SourceOriginKind::Path,
                            locator: Some(path.to_string_lossy().to_string()),
                        },
                        source,
                    ));
                }
            }
        }
        None
    }
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
    eprintln!("  mncs abi <program.mncs|program.json>");
    eprintln!("  mncs execute-backend <program.json> <execution-request.json>");
    eprintln!("  mncs check-backend-execution <program.json> <corpus.json>");
    eprintln!("  mncs validate-translation <kind> <program.json> [corpus.json]");
    eprintln!("  mncs compiler-architecture");
    eprintln!("  mncs compiler-study <program.json> [--node-id NODE] [--target TARGET] [--family-reference]");
    eprintln!("  mncs source-study <program.mncs> [--node-id NODE]");
    eprintln!("  mncs experiment plan <program.mncs> --backend BACKEND --corpus CORPUS [--output-dir DIR]");
    eprintln!("  mncs experiment run <program.mncs> --backend BACKEND --corpus CORPUS [--output-dir DIR] [--node-id NODE]");
    eprintln!("  mncs experiment execute <backend-artifact.json> <corpus.json> [--baseline RESULT.json --output-dir DIR]");
    eprintln!("  mncs experiment inspect <result.json>");
    eprintln!("  mncs experiment compare <left-result.json> <right-result.json>");
    eprintln!(
        "  mncs experiment refine <baseline-result.json> <candidate-result.json>... [--budget N]"
    );
    eprintln!("  mncs experiment rust-control <result.json> <equivalent-control.rs>");
    eprintln!("  mncs experiment matrix");
    eprintln!("  mncs diff <before.json> <after.json>");
    eprintln!("  mncs compare <before.json> <after.json>");
    eprintln!("  mncs slice <manifest.json> <semantic-identity>");
    eprintln!("  mncs evaluate-candidate <baseline.json> <proposal.json>");
    eprintln!("  mncs verify-result <manifest.json> <request.json> <result.json>");
    eprintln!("  mncs evidence-check <evidence.json> <current.json>");
    eprintln!("  mncs syntax-metrics <source> [source ...]");
    eprintln!("  mncs syntax-tournament <tournament.json>");
}
