//! Contract-derived conformance testing for MNCS semantic contracts.
//!
//! Pipeline:
//!
//! ```text
//! MNCS semantic contract (property/invariant/metamorphic clause + predicate)
//!         v
//! case generation (deterministic seeded domains per parameter type)
//!         v
//! execution (reference executor, twice for determinism)
//!         v
//! backend differential execution (each requested backend, same cases)
//!         v
//! observation (returned values vs expected `true`)
//!         v
//! semantic assertion (PASS / FAIL / UNKNOWN / UNSUPPORTED per case/backend)
//!         v
//! evidence (mncs.conformance-report/1, content-addressed, no wall-clock)
//! ```
//!
//! The semantics under test live entirely in MNCS: a contract predicate is an
//! ordinary boolean MNCS function, and structured inputs (images, scenes) are
//! built by MNCS case-constructor calls inside the predicate. This crate only
//! generates scalar seeds, executes, compares, and records. It never
//! re-implements checked semantics, and it never invents PASS: unavailable
//! backends are UNSUPPORTED, toolchain failures are UNKNOWN, and only an
//! observed `true` on every executed backend is PASS.

use std::collections::BTreeMap;

use mncs_model::{
    function_id, BackendArtifact, ContractKind, ExecutionCase, ExecutionCorpus, ExecutionRequest,
    ExecutionStatus, ExecutionTarget, ExecutionValue, Function, IntegerType, Program, SemanticId,
    Value, EXECUTION_CORPUS_SCHEMA_VERSION, EXECUTION_REQUEST_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};

pub const CONFORMANCE_REPORT_SCHEMA_VERSION: &str = "mncs.conformance-report/1";
pub const CONFORMANCE_GENERATOR: &str = "mncs-conformance/0.1";

/// MMIX LCG constants shared with `mncs.core.random.v1` (`lcg_next`), so the
/// host-side case stream and any MNCS-side stream derived from the same seed
/// agree by construction rather than by re-implementation.
const LCG_A: u64 = 6364136223846793005;
const LCG_C: u64 = 1442695040888963407;

fn lcg_next(state: u64) -> u64 {
    state.wrapping_mul(LCG_A).wrapping_add(LCG_C)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseVerdict {
    Pass,
    Fail,
    Unknown,
    Unsupported,
}

impl CaseVerdict {
    fn as_str(&self) -> &'static str {
        match self {
            CaseVerdict::Pass => "pass",
            CaseVerdict::Fail => "fail",
            CaseVerdict::Unknown => "unknown",
            CaseVerdict::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceOptions {
    pub seed: u64,
    pub cases_per_predicate: usize,
    pub backends: Vec<String>,
    pub step_budget: u64,
    /// When non-empty, only predicates with exactly these names execute.
    /// Unmatched discovered contracts are reported as `unsupported` with a
    /// filter reason, never silently dropped from the evidence.
    pub only_predicates: Vec<String>,
}

impl Default for ConformanceOptions {
    fn default() -> Self {
        Self {
            seed: 0xC04F_4F52_4D41_4E43,
            cases_per_predicate: 24,
            backends: vec!["mncs-portable-wasm-mvp".to_owned()],
            step_budget: 16384,
            only_predicates: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredContract {
    pub operation: String,
    pub clause: String,
    pub kind: String,
    pub predicate: String,
    pub predicate_identity: SemanticId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceObservation {
    pub status: String,
    pub observed: Vec<ExecutionValue>,
    pub verdict: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendObservation {
    pub backend: String,
    pub status: String,
    pub observed: Vec<ExecutionValue>,
    pub verdict: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseReport {
    pub id: String,
    pub arguments: Vec<ExecutionValue>,
    pub reference: ReferenceObservation,
    pub determinism_stable: bool,
    pub backends: Vec<BackendObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredicateReport {
    pub operation: String,
    pub clause: String,
    pub kind: String,
    pub predicate: String,
    pub predicate_identity: SemanticId,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default)]
    pub cases: Vec<CaseReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub pass: usize,
    pub fail: usize,
    pub unknown: usize,
    pub unsupported: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerIdentity {
    pub host: String,
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceReport {
    pub schema_version: String,
    pub generator: String,
    pub subject_module: String,
    pub subject_fingerprint: Option<String>,
    pub seed: u64,
    pub cases_per_predicate: usize,
    pub worker: WorkerIdentity,
    pub predicates: Vec<PredicateReport>,
    pub summary: ReportSummary,
}

impl ConformanceReport {
    pub fn failed(&self) -> bool {
        self.summary.fail > 0
    }

    /// Content identity of this report: sha256 over its canonical JSON
    /// bytes. Reports are deterministic (no wall-clock), so identical runs
    /// share one identity and any divergence is observable.
    pub fn report_sha256(&self) -> String {
        mncs_model::sha256_hex(
            serde_json::to_string(self)
                .expect("conformance report serializes")
                .as_bytes(),
        )
    }
}

/// Attach conformance evidence to the operations that earned it.
///
/// For every discovered contract whose predicate report is fully clean
/// (tested, zero failures, every reference observation a pass), an
/// `EvidenceClaim` is recorded on the *operation* function: property =
/// clause id, verifier = generator, status = Tested, artifact = report
/// sha256. The claim discharges the compilation `contract-evidence-bound`
/// obligation for that clause; the report itself (content-addressed by the
/// artifact hash, re-executable via `--emit-corpus`) is what a verifier
/// re-checks. Contracts with any failure, or never tested, earn nothing.
/// Returns the number of claims attached.
pub fn attach_evidence(program: &mut Program, report: &ConformanceReport) -> usize {
    let digest = report.report_sha256();
    let mut attached = 0;
    for predicate in &report.predicates {
        if predicate.status != "tested" {
            continue;
        }
        // Attachment needs a positive anchor, not merely absent
        // counter-evidence: every case must pass on the reference executor,
        // and no backend may report a failure. Unknown/unsupported backend
        // outcomes without failures still attach — the report records their
        // exact scope, and absence of capability is pressure evidence, not
        // counter-evidence.
        let anchored = !predicate.cases.is_empty()
            && predicate
                .cases
                .iter()
                .all(|case| case.reference.verdict == CaseVerdict::Pass.as_str())
            && !predicate.cases.iter().any(|case| {
                case.backends
                    .iter()
                    .any(|backend| backend.verdict == CaseVerdict::Fail.as_str())
            });
        if !anchored {
            continue;
        }
        let Some(operation) = program
            .functions
            .iter_mut()
            .find(|function| function.name == predicate.operation)
        else {
            continue;
        };
        if operation.evidence.iter().any(|claim| {
            claim.property == predicate.clause
                && claim.verifier == report.generator
                && claim.artifact.as_deref() == Some(digest.as_str())
        }) {
            continue;
        }
        operation.evidence.push(mncs_model::EvidenceClaim {
            property: predicate.clause.clone(),
            verifier: report.generator.clone(),
            status: mncs_model::EvidenceStatus::Tested,
            artifact: Some(digest.clone()),
        });
        attached += 1;
    }
    attached
}

impl ConformanceReport {
    /// Emit the generated predicate cases as a standard execution corpus
    /// (expected `true`) so the existing experiment/backend machinery
    /// (`mncs experiment run/compare`, `mncs check-backend-execution`) can
    /// re-execute exactly what this report observed.
    pub fn to_corpus(&self, program: &Program) -> ExecutionCorpus {
        let mut cases = Vec::new();
        for predicate in &self.predicates {
            for case in &predicate.cases {
                cases.push(ExecutionCase {
                    id: case.id.clone(),
                    request: ExecutionRequest {
                        schema_version: EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
                        target: ExecutionTarget {
                            module: program.module.clone(),
                            function: predicate.predicate.clone(),
                        },
                        arguments: case.arguments.clone(),
                        step_budget: 16384,
                        policy: Default::default(),
                        host_grants: Vec::new(),
                    },
                    expected: Some(vec![ExecutionValue::Boolean { value: true }]),
                    expected_status: None,
                    maximum_steps: None,
                    expected_effects: Vec::new(),
                    prohibit_unexpected_effects: false,
                });
            }
        }
        ExecutionCorpus {
            schema_version: EXECUTION_CORPUS_SCHEMA_VERSION.to_owned(),
            name: format!("{}-conformance-seed{}", program.module, self.seed),
            cases,
            properties: Vec::new(),
            stateful_cases: Vec::new(),
        }
    }
}

pub fn worker_identity() -> WorkerIdentity {
    WorkerIdentity {
        host: std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "unknown-host".to_owned()),
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
    }
}

pub fn executable_contract_kinds(kind: &ContractKind) -> bool {
    matches!(
        kind,
        ContractKind::Property | ContractKind::Invariant | ContractKind::Metamorphic
    )
}

fn contract_kind_name(kind: &ContractKind) -> &'static str {
    match kind {
        ContractKind::Property => "property",
        ContractKind::Invariant => "invariant",
        ContractKind::Metamorphic => "metamorphic",
        ContractKind::Requires => "requires",
        ContractKind::Ensures => "ensures",
        ContractKind::Preserves => "preserves",
        ContractKind::Budget => "budget",
    }
}

/// Discover (operation, clause, predicate) bindings from elaborated contract
/// clauses. Discovery is purely structural over validated compiler metadata.
pub fn discover_contracts(program: &Program) -> Vec<DiscoveredContract> {
    let mut discovered = Vec::new();
    for function in &program.functions {
        for clause in &function.contracts {
            if !executable_contract_kinds(&clause.kind) {
                continue;
            }
            discovered.push(DiscoveredContract {
                operation: function.name.clone(),
                clause: clause.id.clone(),
                kind: contract_kind_name(&clause.kind).to_owned(),
                predicate: clause.expression.clone(),
                predicate_identity: function_id(&program.module, &clause.expression),
            });
        }
    }
    discovered.sort_by(|left, right| {
        (left.operation.clone(), left.clause.clone())
            .cmp(&(right.operation.clone(), right.clause.clone()))
    });
    discovered.dedup_by(|a, b| a.operation == b.operation && a.clause == b.clause);
    discovered
}

#[derive(Debug, Clone)]
enum ScalarDomain {
    Bool,
    Byte,
    Integer { bits: u16, signed: bool },
}

fn scalar_domain(value_type: &str) -> Option<ScalarDomain> {
    match value_type {
        "bool" => Some(ScalarDomain::Bool),
        "byte" => Some(ScalarDomain::Byte),
        "i8" => Some(ScalarDomain::Integer {
            bits: 8,
            signed: true,
        }),
        "i16" => Some(ScalarDomain::Integer {
            bits: 16,
            signed: true,
        }),
        "i32" => Some(ScalarDomain::Integer {
            bits: 32,
            signed: true,
        }),
        "i64" => Some(ScalarDomain::Integer {
            bits: 64,
            signed: true,
        }),
        "u8" => Some(ScalarDomain::Integer {
            bits: 8,
            signed: false,
        }),
        "u16" => Some(ScalarDomain::Integer {
            bits: 16,
            signed: false,
        }),
        "u32" => Some(ScalarDomain::Integer {
            bits: 32,
            signed: false,
        }),
        "u64" => Some(ScalarDomain::Integer {
            bits: 64,
            signed: false,
        }),
        _ => None,
    }
}

fn integer_type(bits: u16, signed: bool) -> IntegerType {
    IntegerType { bits, signed }
}

fn int_range(bits: u16, signed: bool) -> (i128, i128) {
    if signed {
        let half = 1i128 << (bits - 1);
        (-half, half - 1)
    } else if bits == 128 {
        (0, i128::MAX)
    } else {
        (0, (1i128 << bits) - 1)
    }
}

fn encode_integer(raw: u64, bits: u16, signed: bool) -> ExecutionValue {
    let (lo, hi) = int_range(bits, signed);
    let span = (hi as u128).wrapping_sub(lo as u128).wrapping_add(1);
    let value = lo + (raw as u128 % span) as i128;
    ExecutionValue::Integer {
        value,
        ty: integer_type(bits, signed),
    }
}

fn boundary_values(domain: &ScalarDomain) -> Vec<ExecutionValue> {
    match domain {
        ScalarDomain::Bool => vec![
            ExecutionValue::Boolean { value: false },
            ExecutionValue::Boolean { value: true },
        ],
        ScalarDomain::Byte => [0i128, 1, 127, 128, 254, 255]
            .into_iter()
            .map(|value| ExecutionValue::Byte { value })
            .collect(),
        ScalarDomain::Integer { bits, signed } => {
            let (lo, hi) = int_range(*bits, *signed);
            let mut points = vec![0i128, 1, lo, hi];
            if *signed {
                points.push(-1);
            }
            if lo < hi {
                points.push(lo + 1);
            }
            if hi > lo {
                points.push(hi - 1);
            }
            let ty = integer_type(*bits, *signed);
            let mut values: Vec<ExecutionValue> = points
                .into_iter()
                .filter(|value| (lo..=hi).contains(value))
                .map(|value| ExecutionValue::Integer { value, ty })
                .collect();
            values.sort_by_key(|value| match value {
                ExecutionValue::Integer { value, .. } => *value,
                _ => 0,
            });
            values.dedup();
            values
        }
    }
}

/// Deterministic case stream for one predicate signature: boundary values
/// first (equivalence classes), then MMIX-LCG draws from the seed. Bounded
/// by `count`; deduplicated. The stream is a pure function of
/// (seed, signature, count).
pub fn generate_cases(
    inputs: &[Value],
    seed: u64,
    count: usize,
) -> Result<Vec<Vec<ExecutionValue>>, String> {
    let mut domains = Vec::new();
    for input in inputs {
        match scalar_domain(&input.value_type) {
            Some(domain) => domains.push(domain),
            None => {
                return Err(format!(
                    "parameter '{}' has non-scalar type '{}': contract predicates with structured parameters are unsupported by scalar case generation (build structured inputs with MNCS case constructors inside the predicate)",
                    input.name, input.value_type
                ));
            }
        }
    }
    if domains.is_empty() {
        // Nullary predicates are constant observations: exactly one case
        // with no arguments. They still execute on every backend.
        return Ok(vec![Vec::new()]);
    }
    let boundary_sets: Vec<Vec<ExecutionValue>> = domains.iter().map(boundary_values).collect();
    let mut cases: Vec<Vec<ExecutionValue>> = Vec::new();
    // Cartesian boundaries, capped: take the diagonal plus corners so the
    // count stays bounded for multi-parameter predicates.
    let mut state = seed;
    fn try_push(cases: &mut Vec<Vec<ExecutionValue>>, case: Vec<ExecutionValue>, count: usize) {
        if cases.len() < count && !cases.contains(&case) {
            cases.push(case);
        }
    }
    // First: all-false/zero corner, then per-parameter boundaries with other
    // parameters at their first boundary value.
    let firsts: Vec<ExecutionValue> = boundary_sets
        .iter()
        .map(|set| {
            set.first()
                .cloned()
                .unwrap_or(ExecutionValue::Boolean { value: false })
        })
        .collect();
    try_push(&mut cases, firsts.clone(), count);
    for (index, set) in boundary_sets.iter().enumerate() {
        for value in set.iter().skip(1) {
            let mut case = firsts.clone();
            case[index] = value.clone();
            try_push(&mut cases, case, count);
            if cases.len() >= count {
                break;
            }
        }
    }
    // Then: deterministic LCG draws.
    let mut guard = 0usize;
    while cases.len() < count && guard < count * 16 + 64 {
        guard += 1;
        state = lcg_next(state);
        let mut draw = state;
        let case: Vec<ExecutionValue> = domains
            .iter()
            .map(|domain| {
                draw = lcg_next(draw.wrapping_add(0x9E37_79B9_7F4A_7C15));
                match domain {
                    ScalarDomain::Bool => ExecutionValue::Boolean {
                        value: draw & 1 == 1,
                    },
                    ScalarDomain::Byte => ExecutionValue::Byte {
                        value: (draw % 256) as i128,
                    },
                    ScalarDomain::Integer { bits, signed } => encode_integer(draw, *bits, *signed),
                }
            })
            .collect();
        try_push(&mut cases, case, count);
    }
    Ok(cases)
}

fn status_name(status: &ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Returned => "returned",
        ExecutionStatus::RuntimeFailure => "runtime_failure",
        ExecutionStatus::Unsupported => "unsupported",
        ExecutionStatus::BudgetExhausted => "budget_exhausted",
        ExecutionStatus::InvalidRequest => "invalid_request",
    }
}

fn is_true_singleton(values: &[ExecutionValue]) -> bool {
    matches!(values, [ExecutionValue::Boolean { value: true }])
}

fn backend_case_verdict(
    backend_result: &mncs_codegen::BackendExecutionResult,
    reference: &mncs_model::ExecutionResult,
    reference_verdict: CaseVerdict,
) -> (CaseVerdict, Vec<ExecutionValue>, Option<String>) {
    let observed = backend_result.returned.clone();
    match backend_result.status {
        ExecutionStatus::Unsupported => (
            CaseVerdict::Unsupported,
            observed,
            backend_result
                .failure
                .as_ref()
                .map(|failure| failure.reason.clone()),
        ),
        ExecutionStatus::InvalidRequest => (
            CaseVerdict::Unknown,
            observed,
            Some(format!(
                "harness request invalid: {}",
                backend_result
                    .failure
                    .as_ref()
                    .map(|failure| failure.reason.as_str())
                    .unwrap_or("unknown")
            )),
        ),
        ExecutionStatus::Returned => {
            if reference.status != ExecutionStatus::Returned {
                return (
                    CaseVerdict::Unknown,
                    observed,
                    Some(
                        "reference did not return; backend agreement cannot be adjudicated"
                            .to_owned(),
                    ),
                );
            }
            let backend_true = is_true_singleton(&observed);
            let reference_true = is_true_singleton(&reference.returned);
            if backend_true && reference_true {
                (CaseVerdict::Pass, observed, None)
            } else if observed == reference.returned {
                (
                    CaseVerdict::Fail,
                    observed,
                    Some("backend agrees with reference, but the predicate is false: contract violated".to_owned()),
                )
            } else {
                let reason = format!(
                    "backend/reference differential: backend {:?} vs reference {:?}",
                    observed, reference.returned
                );
                (CaseVerdict::Fail, observed, Some(reason))
            }
        }
        _ => {
            if reference.status == ExecutionStatus::Returned {
                (
                    CaseVerdict::Fail,
                    observed,
                    backend_result.failure.as_ref().map(|failure| {
                        format!(
                            "{}: {}",
                            status_name(&backend_result.status),
                            failure.reason
                        )
                    }),
                )
            } else if reference_verdict == CaseVerdict::Fail {
                (
                    CaseVerdict::Fail,
                    observed,
                    backend_result.failure.as_ref().map(|failure| {
                        format!(
                            "{}: {}",
                            status_name(&backend_result.status),
                            failure.reason
                        )
                    }),
                )
            } else {
                (
                    CaseVerdict::Unknown,
                    observed,
                    backend_result.failure.as_ref().map(|failure| {
                        format!(
                            "{}: {}",
                            status_name(&backend_result.status),
                            failure.reason
                        )
                    }),
                )
            }
        }
    }
}

struct LoweredBackend {
    artifact: Option<BackendArtifact>,
    lowering_error: Option<String>,
}

fn find_predicate<'a>(program: &'a Program, name: &str) -> Option<&'a Function> {
    program
        .functions
        .iter()
        .find(|function| function.name == name)
}

fn predicate_output_bool(predicate: &Function) -> bool {
    matches!(predicate.outputs.as_slice(), [output] if output.value_type == "bool")
}

fn tally(summary: &mut ReportSummary, verdict: CaseVerdict) {
    match verdict {
        CaseVerdict::Pass => summary.pass += 1,
        CaseVerdict::Fail => summary.fail += 1,
        CaseVerdict::Unknown => summary.unknown += 1,
        CaseVerdict::Unsupported => summary.unsupported += 1,
    }
}

/// Run contract-derived conformance: discover executable clauses, generate
/// deterministic cases, execute on the reference executor (twice), execute
/// on each requested backend, and record evidence.
pub fn run_conformance(program: &Program, options: &ConformanceOptions) -> ConformanceReport {
    let mut summary = ReportSummary {
        pass: 0,
        fail: 0,
        unknown: 0,
        unsupported: 0,
    };
    // Lower once per backend; lowering failures become per-case UNKNOWN for
    // that backend (honest: the backend proved nothing either way).
    let mut lowered: BTreeMap<String, LoweredBackend> = BTreeMap::new();
    let ssa = program.lower_to_ssa().ok();
    for backend in &options.backends {
        let entry = ssa.as_ref().and_then(|ssa_module| {
            let selected = mncs_codegen::selected_ssa_ref(ssa_module);
            let plan = mncs_codegen::plan_for_backend(backend, selected.clone())?;
            Some((selected, plan))
        });
        match entry {
            None => {
                lowered.insert(
                    backend.clone(),
                    LoweredBackend {
                        artifact: None,
                        lowering_error: Some(if ssa.is_none() {
                            "program SSA lowering failed; backend observations unavailable"
                                .to_owned()
                        } else {
                            format!("unknown backend '{backend}': no registered lowering plan")
                        }),
                    },
                );
            }
            Some((selected, plan)) => {
                match mncs_codegen::lower_with_backend(
                    backend,
                    program,
                    ssa.as_ref().expect("ssa present"),
                    selected,
                    &plan,
                )
                .artifact
                {
                    Some(artifact) => {
                        lowered.insert(
                            backend.clone(),
                            LoweredBackend {
                                artifact: Some(artifact),
                                lowering_error: None,
                            },
                        );
                    }
                    None => {
                        lowered.insert(
                            backend.clone(),
                            LoweredBackend {
                                artifact: None,
                                lowering_error: Some(format!(
                                    "backend '{backend}' lowering produced no artifact"
                                )),
                            },
                        );
                    }
                }
            }
        }
    }

    let mut predicates = Vec::new();
    for contract in discover_contracts(program) {
        if !options.only_predicates.is_empty()
            && !options
                .only_predicates
                .iter()
                .any(|name| name == &contract.predicate)
        {
            summary.unsupported += 1;
            predicates.push(PredicateReport {
                operation: contract.operation,
                clause: contract.clause,
                kind: contract.kind,
                predicate: contract.predicate,
                predicate_identity: contract.predicate_identity,
                status: "unsupported".to_owned(),
                reason: Some("excluded by --predicate filter".to_owned()),
                cases: Vec::new(),
            });
            continue;
        }
        let Some(predicate) = find_predicate(program, &contract.predicate) else {
            summary.unknown += 1;
            predicates.push(PredicateReport {
                operation: contract.operation,
                clause: contract.clause,
                kind: contract.kind,
                predicate: contract.predicate,
                predicate_identity: contract.predicate_identity,
                status: "unknown".to_owned(),
                reason: Some("predicate function missing from program".to_owned()),
                cases: Vec::new(),
            });
            continue;
        };
        if !predicate.generic_params.is_empty() {
            summary.unsupported += 1;
            predicates.push(PredicateReport {
                operation: contract.operation,
                clause: contract.clause,
                kind: contract.kind,
                predicate: contract.predicate,
                predicate_identity: contract.predicate_identity,
                status: "unsupported".to_owned(),
                reason: Some(
                    "generic predicates need instantiation before case generation".to_owned(),
                ),
                cases: Vec::new(),
            });
            continue;
        }
        if !predicate_output_bool(predicate) {
            summary.unknown += 1;
            predicates.push(PredicateReport {
                operation: contract.operation,
                clause: contract.clause,
                kind: contract.kind,
                predicate: contract.predicate,
                predicate_identity: contract.predicate_identity,
                status: "unknown".to_owned(),
                reason: Some("predicate does not return exactly one bool".to_owned()),
                cases: Vec::new(),
            });
            continue;
        }
        let target_module = predicate
            .home_module
            .clone()
            .unwrap_or_else(|| program.module.clone());
        let argument_sets =
            match generate_cases(&predicate.inputs, options.seed, options.cases_per_predicate) {
                Ok(sets) => sets,
                Err(reason) => {
                    summary.unsupported += 1;
                    predicates.push(PredicateReport {
                        operation: contract.operation,
                        clause: contract.clause,
                        kind: contract.kind,
                        predicate: contract.predicate,
                        predicate_identity: contract.predicate_identity,
                        status: "unsupported".to_owned(),
                        reason: Some(reason),
                        cases: Vec::new(),
                    });
                    continue;
                }
            };
        let mut cases = Vec::new();
        for (index, arguments) in argument_sets.into_iter().enumerate() {
            let request = ExecutionRequest {
                schema_version: EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
                target: ExecutionTarget {
                    module: target_module.clone(),
                    function: predicate.name.clone(),
                },
                arguments: arguments.clone(),
                step_budget: options.step_budget,
                policy: Default::default(),
                host_grants: Vec::new(),
            };
            let first = mncs_model::execute(program, &request);
            let second = mncs_model::execute(program, &request);
            let stable = first.status == second.status && first.returned == second.returned;
            let (reference_verdict, reason) = reference_verdict(&first);
            let reference = ReferenceObservation {
                status: status_name(&first.status).to_owned(),
                observed: first.returned.clone(),
                verdict: reference_verdict.as_str().to_owned(),
                reason,
            };
            tally(&mut summary, reference_verdict);
            let case_id = format!("{}-c{index:03}", predicate.name);
            let mut backend_reports = Vec::new();
            for backend in &options.backends {
                let lowered_backend = lowered.get(backend).expect("lowered entry");
                match &lowered_backend.artifact {
                    None => {
                        tally(&mut summary, CaseVerdict::Unknown);
                        backend_reports.push(BackendObservation {
                            backend: backend.clone(),
                            status: "not_lowered".to_owned(),
                            observed: Vec::new(),
                            verdict: CaseVerdict::Unknown.as_str().to_owned(),
                            reason: lowered_backend.lowering_error.clone(),
                        });
                    }
                    Some(artifact) => {
                        let backend_result = mncs_codegen::execute_backend(artifact, &request);
                        let (verdict, observed, reason) =
                            backend_case_verdict(&backend_result, &first, reference_verdict);
                        tally(&mut summary, verdict);
                        backend_reports.push(BackendObservation {
                            backend: backend.clone(),
                            status: status_name(&backend_result.status).to_owned(),
                            observed,
                            verdict: verdict.as_str().to_owned(),
                            reason,
                        });
                    }
                }
            }
            cases.push(CaseReport {
                id: case_id,
                arguments,
                reference,
                determinism_stable: stable,
                backends: backend_reports,
            });
            if !stable {
                summary.fail += 1;
            }
        }
        predicates.push(PredicateReport {
            operation: contract.operation,
            clause: contract.clause,
            kind: contract.kind,
            predicate: contract.predicate,
            predicate_identity: contract.predicate_identity,
            status: "tested".to_owned(),
            reason: None,
            cases,
        });
    }

    ConformanceReport {
        schema_version: CONFORMANCE_REPORT_SCHEMA_VERSION.to_owned(),
        generator: CONFORMANCE_GENERATOR.to_owned(),
        subject_module: program.module.clone(),
        subject_fingerprint: program.content_fingerprint().ok(),
        seed: options.seed,
        cases_per_predicate: options.cases_per_predicate,
        worker: worker_identity(),
        predicates,
        summary,
    }
}

fn reference_verdict(result: &mncs_model::ExecutionResult) -> (CaseVerdict, Option<String>) {
    match result.status {
        ExecutionStatus::Returned if is_true_singleton(&result.returned) => {
            (CaseVerdict::Pass, None)
        }
        ExecutionStatus::Returned => (
            CaseVerdict::Fail,
            Some(format!("predicate observed {:?}", result.returned)),
        ),
        ExecutionStatus::InvalidRequest => (
            CaseVerdict::Unknown,
            result
                .failure
                .as_ref()
                .map(|failure| format!("harness request invalid: {}", failure.reason)),
        ),
        _ => (
            CaseVerdict::Fail,
            result
                .failure
                .as_ref()
                .map(|failure| format!("{}: {}", status_name(&result.status), failure.reason)),
        ),
    }
}
