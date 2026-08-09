use std::{env, fs, path::Path, process::ExitCode};

use mncs_model::{
    CandidateEvaluation, CausalSlice, Confidence, DeterministicVerifier, DiagnosticCategory,
    DiagnosticObligation, EvidenceFreshness, EvidenceManifest, EvidenceState, FunctionBody,
    ObligationStatus, Program, SemanticDiff, SemanticId,
};
use mncs_syntax::{analyze, SourceMetrics};
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
    eprintln!("  mncs diff <before.json> <after.json>");
    eprintln!("  mncs compare <before.json> <after.json>");
    eprintln!("  mncs slice <manifest.json> <semantic-identity>");
    eprintln!("  mncs evaluate-candidate <baseline.json> <proposal.json>");
    eprintln!("  mncs verify-result <manifest.json> <request.json> <result.json>");
    eprintln!("  mncs evidence-check <evidence.json> <current.json>");
    eprintln!("  mncs syntax-metrics <source> [source ...]");
    eprintln!("  mncs syntax-tournament <tournament.json>");
}
