use std::{env, fs, path::Path, process::ExitCode};

use mncs_model::{EvidenceManifest, Program, SemanticDiff};
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
        "diff" => two_manifest_command(args, diff),
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

#[derive(Debug, Serialize)]
struct DiffReport {
    semantic: SemanticDiff,
    invalidation: mncs_model::InvalidationReport,
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
    eprintln!("  mncs diff <before.json> <after.json>");
    eprintln!("  mncs evidence-check <evidence.json> <current.json>");
    eprintln!("  mncs syntax-metrics <source> [source ...]");
    eprintln!("  mncs syntax-tournament <tournament.json>");
}
