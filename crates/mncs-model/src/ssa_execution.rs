//! Independent bounded execution of the validated SSA artifact.
//!
//! This is a reference evaluator for the experimental SSA subset, not a
//! backend.  It interprets SSA values, block parameters, instructions, and
//! terminators directly.  Agreement with body execution is bounded evidence
//! about the declared corpus and is not compiler correctness.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::execution::{
    compare_floats, compare_integers, evaluate_float, evaluate_integer, execution_value_summary,
    integer_operator_supported, ExecutionEffectEvent,
};
use crate::identity::{function_id, program_id};
use crate::{
    execute_with_policy, BodyType, EvidenceReceipt, EvidenceReceiptOutcome, ExecutionCorpus,
    ExecutionFailure, ExecutionResult, ExecutionStatus, ExecutionSubject, ExecutionValue,
    IntegerType, IrType, Program, SemanticId, SsaBlock, SsaFunction, SsaInstruction,
    SsaInstructionKind, SsaModule, SsaTerminator, MAX_EXECUTION_BUDGET,
};

pub const SSA_EXECUTION_RESULT_SCHEMA_VERSION: &str = "0.1";
pub const LOWERING_EXECUTION_COMPARISON_SCHEMA_VERSION: &str = "0.1";
const MAX_SSA_TRACE_ENTRIES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaExecutionTraceEntry {
    pub step: u64,
    pub block: SemanticId,
    pub semantic_identity: Option<SemanticId>,
    pub hir_identities: Vec<SemanticId>,
    pub ssa_identity: Option<SemanticId>,
    pub event: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaExecutionResult {
    pub schema_version: String,
    pub status: ExecutionStatus,
    pub target: crate::ExecutionTarget,
    pub semantic_program_identity: Option<SemanticId>,
    pub semantic_function_identity: Option<SemanticId>,
    pub ssa_module_identity: Option<SemanticId>,
    pub ssa_module_fingerprint: Option<String>,
    pub hir_fingerprint: Option<String>,
    pub returned: Vec<ExecutionValue>,
    pub steps: u64,
    pub failure: Option<ExecutionFailure>,
    pub trace: Vec<SsaExecutionTraceEntry>,
    pub trace_truncated: bool,
    pub effects: Vec<ExecutionEffectEvent>,
}

impl SsaExecutionResult {
    fn empty(request: &crate::ExecutionRequest) -> Self {
        Self {
            schema_version: SSA_EXECUTION_RESULT_SCHEMA_VERSION.to_owned(),
            status: ExecutionStatus::InvalidRequest,
            target: request.target.clone(),
            semantic_program_identity: None,
            semantic_function_identity: None,
            ssa_module_identity: None,
            ssa_module_fingerprint: None,
            hir_fingerprint: None,
            returned: Vec::new(),
            steps: 0,
            failure: None,
            trace: Vec::new(),
            trace_truncated: false,
            effects: Vec::new(),
        }
    }

    fn invalid(request: &crate::ExecutionRequest, reason: impl Into<String>) -> Self {
        let mut result = Self::empty(request);
        result.failure = Some(ExecutionFailure {
            identity: None,
            reason: reason.into(),
        });
        result
    }

    fn fail(
        &mut self,
        status: ExecutionStatus,
        identity: Option<SemanticId>,
        reason: impl Into<String>,
    ) {
        self.status = status;
        self.failure = Some(ExecutionFailure {
            identity,
            reason: reason.into(),
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoweringExecutionStatus {
    ConsistentOverCorpus,
    MismatchDetected,
    Unsupported,
    InvalidInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweringDivergenceContext {
    pub last_matching_semantic_identity: Option<SemanticId>,
    pub first_body_event: Option<String>,
    pub first_ssa_event: Option<String>,
    pub related_hir_identities: Vec<SemanticId>,
    pub related_ssa_identities: Vec<SemanticId>,
    pub uncertainty: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweringExecutionMismatch {
    pub case_id: String,
    pub body: ExecutionResult,
    pub ssa: SsaExecutionResult,
    pub divergence: LoweringDivergenceContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweringExecutionComparison {
    pub schema_version: String,
    pub status: LoweringExecutionStatus,
    pub epistemic_note: String,
    pub corpus_name: String,
    pub corpus_size: usize,
    pub matching_cases: usize,
    pub mismatching_cases: usize,
    pub mismatches_truncated: bool,
    pub program_identity: Option<SemanticId>,
    pub program_fingerprint: Option<String>,
    pub function_identity: Option<SemanticId>,
    pub ssa_module_identity: Option<SemanticId>,
    pub ssa_module_fingerprint: Option<String>,
    pub hir_fingerprint: Option<String>,
    pub body: ExecutionSubject,
    pub ssa: ExecutionSubject,
    pub mismatches: Vec<LoweringExecutionMismatch>,
}

/// Resolve the executable identity triple once per entry and hand it back
/// owned so each frame can re-share references with its nested calls.
///
/// Every fresh entry point arrives with `cached_identity == None`, and the
/// triple costs a full SSA-module serialization plus two hashes to build.
/// Threading the raw parameter through unchanged meant every nested MNCS
/// function call rebuilt it inside `module.fingerprint()` — quadratic
/// validation blowup (full canonical JSON + SHA-256 per call) that made
/// record-heavy observers with tens of thousands of calls unexecutable.
/// Callers keep a `None` result honest: identity stays unknown rather than
/// re-derived per call.
fn resolve_execution_cache(
    program: &Program,
    module: &SsaModule,
    cached_identity: Option<(&SemanticId, &String, &String)>,
) -> Option<(SemanticId, String, String)> {
    if let Some((identity, program_fingerprint, fingerprint)) = cached_identity {
        return Some((
            identity.clone(),
            program_fingerprint.clone(),
            fingerprint.clone(),
        ));
    }
    let fingerprint = module.fingerprint().ok()?;
    Some((
        program_id(&program.module),
        program.content_fingerprint().ok().unwrap_or_default(),
        fingerprint,
    ))
}

/// Lower the validated program once and independently interpret the resulting
/// SSA artifact for one request.
pub fn execute_ssa(program: &Program, request: &crate::ExecutionRequest) -> SsaExecutionResult {
    let module = match program.lower_to_ssa() {
        Ok(module) => module,
        Err(error) => return SsaExecutionResult::invalid(request, error.to_string()),
    };
    execute_ssa_module(program, &module, request)
}

/// Interpret an already materialized SSA artifact.  This entry point is kept
/// public so callers can test a mutated/loaded SSA artifact without silently
/// reconstructing the body representation.
pub fn execute_ssa_module(
    program: &Program,
    module: &SsaModule,
    request: &crate::ExecutionRequest,
) -> SsaExecutionResult {
    execute_ssa_module_with_validation(program, module, request, true, None, None, None, 0)
}

/// Interpret an SSA module after its immutable program/module validation has
/// already been performed by a trusted artifact boundary. Request-specific
/// checks still run for every call. This is used by bounded stateful backend
/// sessions to avoid validating the same deserialized artifact on every
/// transition without making the public one-shot API less strict.
pub fn execute_ssa_module_prevalidated(
    program: &Program,
    module: &SsaModule,
    request: &crate::ExecutionRequest,
) -> SsaExecutionResult {
    execute_ssa_module_with_validation(program, module, request, false, None, None, None, 0)
}

/// Reusable immutable execution preparation for a bounded stateful session.
/// The program and SSA validation, plus each function's block index, are
/// performed once; request schema, target, argument, and step-budget checks
/// continue to run at every transition.
pub struct SsaExecutionSession {
    /// The validated executable material is owned by the session. Keeping the
    /// pair together makes it impossible for a caller to supply a different
    /// program or SSA module after the receipt and block indexes were built.
    program: Arc<Program>,
    module: Arc<SsaModule>,
    block_indices: BTreeMap<SemanticId, BTreeMap<SemanticId, usize>>,
    program_identity: SemanticId,
    program_fingerprint: String,
    module_fingerprint: String,
    validation_receipt: EvidenceReceipt,
}

impl SsaExecutionSession {
    pub fn new(program: &Program, module: &SsaModule) -> Result<Self, String> {
        Self::from_shared(Arc::new(program.clone()), Arc::new(module.clone()))
    }

    /// Construct a session from already-owned immutable artifact material.
    /// This is used by decoded backend payloads so the payload and session
    /// share one allocation for the validated pair.
    pub fn from_shared(program: Arc<Program>, module: Arc<SsaModule>) -> Result<Self, String> {
        if module.schema_version != crate::SSA_SCHEMA_VERSION {
            return Err("unsupported SSA schema version".to_owned());
        }
        let program_identity = program_id(&program.module);
        if module.semantic_identity != program_identity {
            return Err("SSA semantic program identity does not match program".to_owned());
        }
        if !module.identity_is_valid() {
            return Err("SSA module identity does not match semantic/HIR identity".to_owned());
        }
        if !program.validate().valid {
            return Err("program validation failed".to_owned());
        }
        if !module.validate().valid {
            return Err("SSA validation failed".to_owned());
        }
        let program_fingerprint = program
            .content_fingerprint()
            .map_err(|error| format!("program fingerprint failed: {error}"))?;
        let module_fingerprint = module
            .fingerprint()
            .map_err(|error| format!("SSA fingerprint failed: {error}"))?;
        let block_indices = module
            .functions
            .iter()
            .map(|function| {
                (
                    function.semantic_identity.clone(),
                    function
                        .blocks
                        .iter()
                        .enumerate()
                        .map(|(index, block)| (block.identity.clone(), index))
                        .collect(),
                )
            })
            .collect();
        let validation_receipt = EvidenceReceipt::new(
            "SSA artifact validates against semantic program",
            module.identity.clone(),
            SemanticId("mncs:validator:ssa-execution-session:0.1".to_owned()),
            BTreeMap::from([
                ("ssa_schema".to_owned(), module.schema_version.clone()),
                (
                    "execution_contract".to_owned(),
                    SSA_EXECUTION_RESULT_SCHEMA_VERSION.to_owned(),
                ),
            ]),
            BTreeMap::from([
                (program_identity.clone(), program_fingerprint.clone()),
                (module.identity.clone(), module_fingerprint.clone()),
            ]),
            Vec::new(),
            EvidenceReceiptOutcome::Pass,
            vec![
                "semantic program identity changes".to_owned(),
                "SSA identity or content changes".to_owned(),
                "validator or execution contract changes".to_owned(),
            ],
        );
        Ok(Self {
            program,
            module,
            block_indices,
            program_identity,
            program_fingerprint,
            module_fingerprint,
            validation_receipt,
        })
    }

    pub fn validation_receipt(&self) -> &EvidenceReceipt {
        &self.validation_receipt
    }

    /// Execute only the exact immutable program/SSA pair validated by this
    /// session. Request-specific validation still runs for every transition.
    pub fn execute(&self, request: &crate::ExecutionRequest) -> SsaExecutionResult {
        execute_ssa_module_with_validation(
            &self.program,
            &self.module,
            request,
            false,
            Some(&self.block_indices),
            Some((
                &self.program_identity,
                &self.program_fingerprint,
                &self.module_fingerprint,
            )),
            None,
            0,
        )
    }

    /// Execute a stateful transition while transferring ownership of its
    /// logical arguments into the evaluator. This is equivalent to `execute`
    /// but avoids deep-cloning a large immutable checkpoint merely to validate
    /// and normalize it at the backend boundary.
    pub fn execute_owned(&self, request: crate::ExecutionRequest) -> SsaExecutionResult {
        let crate::ExecutionRequest {
            schema_version,
            target,
            arguments,
            type_arguments,
            step_budget,
            policy,
            host_grants,
            call_depth_budget,
        } = request;
        let request = crate::ExecutionRequest {
            schema_version,
            target,
            arguments: Vec::new(),
            type_arguments,
            step_budget,
            policy,
            host_grants,
            call_depth_budget,
        };
        execute_ssa_module_with_validation(
            &self.program,
            &self.module,
            &request,
            false,
            Some(&self.block_indices),
            Some((
                &self.program_identity,
                &self.program_fingerprint,
                &self.module_fingerprint,
            )),
            Some(arguments),
            0,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_ssa_module_with_validation(
    program: &Program,
    module: &SsaModule,
    request: &crate::ExecutionRequest,
    validate_artifact: bool,
    block_cache: Option<&BTreeMap<SemanticId, BTreeMap<SemanticId, usize>>>,
    cached_identity: Option<(&SemanticId, &String, &String)>,
    owned_arguments: Option<Vec<ExecutionValue>>,
    call_depth: u64,
) -> SsaExecutionResult {
    if request.schema_version != crate::EXECUTION_REQUEST_SCHEMA_VERSION {
        return SsaExecutionResult::invalid(
            request,
            format!(
                "unsupported execution request schema {:?}; expected {:?}",
                request.schema_version,
                crate::EXECUTION_REQUEST_SCHEMA_VERSION
            ),
        );
    }
    if validate_artifact && module.schema_version != crate::SSA_SCHEMA_VERSION {
        return SsaExecutionResult::invalid(request, "unsupported SSA schema version");
    }
    if validate_artifact && module.semantic_identity != program_id(&program.module) {
        return SsaExecutionResult::invalid(
            request,
            "SSA semantic program identity does not match program",
        );
    }
    if request.target.module != program.module
        && !program
            .functions
            .iter()
            .any(|function| function.home_module.as_deref() == Some(request.target.module.as_str()))
    {
        return SsaExecutionResult::invalid(
            request,
            "execution target module does not match program",
        );
    }
    if request.step_budget == 0 || request.step_budget > MAX_EXECUTION_BUDGET {
        return SsaExecutionResult::invalid(
            request,
            format!("step_budget must be between 1 and {MAX_EXECUTION_BUDGET}"),
        );
    }
    if request.call_depth_budget == Some(0) {
        return SsaExecutionResult::invalid(
            request,
            "call_depth_budget must be at least 1 when present",
        );
    }
    if request.call_depth_budget > Some(crate::MODEL_MAX_CALL_DEPTH) {
        return SsaExecutionResult::invalid(
            request,
            format!(
                "call_depth_budget must not exceed {}",
                crate::MODEL_MAX_CALL_DEPTH
            ),
        );
    }
    // RFC 0047 call-depth fuel, mirroring the body reference executor:
    // every nested module call consumes one unit of a budget defaulting
    // to the absolute model cap, so exhaustion is deterministic fuel
    // failure rather than unbounded host stacking.
    let depth_limit = request
        .call_depth_budget
        .unwrap_or(crate::MODEL_MAX_CALL_DEPTH);
    if call_depth > depth_limit {
        let mut exhausted = SsaExecutionResult::empty(request);
        exhausted.fail(
            ExecutionStatus::BudgetExhausted,
            None,
            format!("execution call depth {call_depth} exceeded budget {depth_limit}"),
        );
        return exhausted;
    }
    if validate_artifact {
        let validation = program.validate();
        if !validation.valid {
            return SsaExecutionResult::invalid(request, "program validation failed");
        }
        let module_validation = module.validate();
        if !module_validation.valid {
            return SsaExecutionResult::invalid(request, "SSA validation failed");
        }
    }
    // P1-013: resolve a generic target through its explicit,
    // previously compiled specialization exactly like the body executor.
    // `NotFound` falls through to the historical SSA lookup so the
    // missing-entry message below stays byte-identical.
    let entry_name = match crate::resolve_generic_entry(
        program,
        &request.target.module,
        &request.target.function,
        &request.type_arguments,
    ) {
        Ok(crate::GenericEntryTarget::Concrete { function_name }) => function_name,
        Ok(crate::GenericEntryTarget::Specialization { function_name, .. }) => function_name,
        Err(crate::GenericEntryFailure::NotFound) => request.target.function.clone(),
        Err(failure) => {
            return SsaExecutionResult::invalid(
                request,
                crate::generic_entry_failure_reason(&failure),
            );
        }
    };
    // Resolve the target against its home module namespace when the target
    // names a linked declaration rather than a root-module function.
    let target_namespace = program
        .functions
        .iter()
        .find(|candidate| {
            candidate.name == entry_name
                && candidate.identity_namespace(&program.module) == request.target.module
        })
        .map(|candidate| candidate.identity_namespace(&program.module).to_owned())
        .unwrap_or_else(|| program.module.clone());
    let semantic_function = function_id(&target_namespace, &entry_name);
    let Some(function) = module
        .functions
        .iter()
        .find(|function| function.semantic_identity == semantic_function)
    else {
        return SsaExecutionResult::invalid(
            request,
            "execution target SSA function does not exist",
        );
    };
    let (program_identity, module_fingerprint) = cached_identity
        .map(
            |(program_identity, _program_fingerprint, module_fingerprint)| {
                (program_identity.clone(), Some(module_fingerprint.clone()))
            },
        )
        .unwrap_or_else(|| (program_id(&program.module), module.fingerprint().ok()));
    // Re-share the resolved identity with nested calls (see
    // `resolve_execution_cache`): the raw parameter is `None` on every fresh
    // entry point, and passing it through unchanged would make every nested
    // MNCS call re-serialize and re-hash the whole SSA module.
    let owned_cache = resolve_execution_cache(program, module, cached_identity);
    let resolved_cache: Option<(&SemanticId, &String, &String)> =
        owned_cache
            .as_ref()
            .map(|(identity, program_fingerprint, fingerprint)| {
                (identity, program_fingerprint, fingerprint)
            });
    let mut result = SsaExecutionResult {
        schema_version: SSA_EXECUTION_RESULT_SCHEMA_VERSION.to_owned(),
        status: ExecutionStatus::InvalidRequest,
        target: request.target.clone(),
        semantic_program_identity: Some(program_identity),
        semantic_function_identity: Some(semantic_function),
        ssa_module_identity: Some(module.identity.clone()),
        ssa_module_fingerprint: module_fingerprint,
        hir_fingerprint: Some(module.hir_fingerprint.clone()),
        returned: Vec::new(),
        steps: 0,
        failure: None,
        trace: Vec::new(),
        trace_truncated: false,
        effects: Vec::new(),
    };
    let Some(mut values) =
        initialize_inputs(program, function, request, &mut result, owned_arguments)
    else {
        return result;
    };
    let local_block_indices = if block_cache.is_none() {
        function
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (block.identity.clone(), index))
            .collect::<BTreeMap<_, _>>()
    } else {
        BTreeMap::new()
    };
    let block_indices = block_cache
        .and_then(|cache| cache.get(&function.semantic_identity))
        .unwrap_or(&local_block_indices);
    let Some(entry) = function.blocks.first().map(|block| block.identity.clone()) else {
        result.fail(
            ExecutionStatus::InvalidRequest,
            None,
            "SSA function has no entry block",
        );
        return result;
    };
    let mut current = entry;
    loop {
        let Some(block_index) = block_indices.get(&current).copied() else {
            result.fail(
                ExecutionStatus::InvalidRequest,
                None,
                "SSA reached an unknown block",
            );
            return result;
        };
        let block = &function.blocks[block_index];
        trace_block(&mut result, block, "block_enter");
        for instruction in &block.instructions {
            if !consume_step(&mut result, request.step_budget) {
                result.fail(
                    ExecutionStatus::BudgetExhausted,
                    instruction
                        .semantic_identity
                        .clone()
                        .or_else(|| Some(instruction.identity.clone())),
                    "execution step budget exhausted",
                );
                return result;
            }
            trace_instruction(&mut result, block, instruction, "instruction");
            if execute_instruction(
                program,
                module,
                instruction,
                &mut values,
                &mut result,
                request,
                block_cache,
                resolved_cache,
                call_depth,
            ) {
                return result;
            }
        }
        if !consume_step(&mut result, request.step_budget) {
            result.fail(
                ExecutionStatus::BudgetExhausted,
                block.semantic_identity.clone(),
                "execution step budget exhausted",
            );
            return result;
        }
        trace_block(&mut result, block, "terminator");
        match &block.terminator {
            SsaTerminator::Return { values: returned } => {
                let mut returned_values = Vec::with_capacity(returned.len());
                for value in returned {
                    let returned_value = if returned
                        .iter()
                        .filter(|candidate| *candidate == value)
                        .count()
                        == 1
                    {
                        values.remove(value)
                    } else {
                        values.get(value).cloned()
                    };
                    let Some(returned_value) = returned_value else {
                        result.fail(
                            ExecutionStatus::InvalidRequest,
                            block.semantic_identity.clone(),
                            "return referenced an unavailable value",
                        );
                        return result;
                    };
                    returned_values.push(returned_value);
                }
                if returned_values.len() != returned.len() {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        block.semantic_identity.clone(),
                        "return referenced an unavailable value",
                    );
                    return result;
                }
                result.returned = returned_values;
                result.status = ExecutionStatus::Returned;
                return result;
            }
            SsaTerminator::Branch { target, arguments } => {
                if !assign_block_arguments(function, block_indices, target, arguments, &mut values)
                {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        block.semantic_identity.clone(),
                        "branch arguments did not match target parameters",
                    );
                    return result;
                }
                current = target.clone();
            }
            SsaTerminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } => {
                let Some(ExecutionValue::Boolean { value }) = values.get(condition) else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        block.semantic_identity.clone(),
                        "conditional branch did not receive a boolean value",
                    );
                    return result;
                };
                let (target, arguments) = if *value {
                    (then_target, then_arguments)
                } else {
                    (else_target, else_arguments)
                };
                if !assign_block_arguments(function, block_indices, target, arguments, &mut values)
                {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        block.semantic_identity.clone(),
                        "conditional branch arguments did not match target parameters",
                    );
                    return result;
                }
                current = target.clone();
            }
            SsaTerminator::Failure { mode } => {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    block.semantic_identity.clone(),
                    format!("failure terminator reached: {mode:?}"),
                );
                return result;
            }
        }
    }
}

/// Compare the body evaluator against a separately interpreted SSA artifact
/// over a declared finite corpus.
pub fn compare_body_and_ssa(
    program: &Program,
    corpus: &ExecutionCorpus,
) -> LoweringExecutionComparison {
    let module = match program.lower_to_ssa() {
        Ok(module) => module,
        Err(_) => return invalid_comparison(program, corpus),
    };
    let target = corpus
        .cases
        .first()
        .map(|case_| case_.request.target.clone());
    let function_identity = target.as_ref().map(|target| {
        program
            .functions
            .iter()
            .find(|candidate| {
                candidate.name == target.function
                    && candidate.identity_namespace(&program.module) == target.module
            })
            .map(|candidate| {
                function_id(
                    candidate.identity_namespace(&program.module),
                    &candidate.name,
                )
            })
            .unwrap_or_else(|| function_id(&target.module, &target.function))
    });
    let mut mismatches = Vec::new();
    let mut matching_cases = 0;
    let mut mismatching_cases = 0;
    let mut unsupported_case = false;
    let mut invalid_case = !crate::execution_corpus_schema_supported(&corpus.schema_version)
        || corpus.cases.is_empty();
    for case_ in &corpus.cases {
        invalid_case |= case_.id.trim().is_empty();
        let body = execute_with_policy(program, &case_.request);
        let ssa = execute_ssa_module(program, &module, &case_.request);
        let body_invalid = body.status == ExecutionStatus::InvalidRequest;
        let ssa_invalid = ssa.status == ExecutionStatus::InvalidRequest;
        unsupported_case |= body.status == ExecutionStatus::Unsupported
            || ssa.status == ExecutionStatus::Unsupported;
        if observable_body_ssa(&body, &ssa) {
            matching_cases += 1;
        } else {
            mismatching_cases += 1;
            if mismatches.is_empty() {
                mismatches.push(LoweringExecutionMismatch {
                    case_id: case_.id.clone(),
                    divergence: divergence_context(&body, &ssa),
                    body,
                    ssa,
                });
            }
        }
        invalid_case |= body_invalid || ssa_invalid;
    }
    let status = if invalid_case {
        LoweringExecutionStatus::InvalidInput
    } else if unsupported_case {
        LoweringExecutionStatus::Unsupported
    } else if mismatching_cases == 0 {
        LoweringExecutionStatus::ConsistentOverCorpus
    } else {
        LoweringExecutionStatus::MismatchDetected
    };
    let body_subject = execution_subject(program, target.as_ref());
    let ssa_subject = ExecutionSubject {
        module: body_subject.module.clone(),
        function: body_subject.function.clone(),
        program_identity: body_subject.program_identity.clone(),
        program_fingerprint: body_subject.program_fingerprint.clone(),
        function_identity: body_subject.function_identity.clone(),
    };
    LoweringExecutionComparison {
        schema_version: LOWERING_EXECUTION_COMPARISON_SCHEMA_VERSION.to_owned(),
        status,
        epistemic_note: "body/SSA agreement is bounded reference-execution evidence, not universal equivalence, backend validation, or formal compiler correctness; shared helpers can create common-mode defects".to_owned(),
        corpus_name: corpus.name.clone(),
        corpus_size: corpus.cases.len(),
        matching_cases,
        mismatching_cases,
        mismatches_truncated: mismatching_cases > mismatches.len(),
        program_identity: body_subject.program_identity.clone(),
        program_fingerprint: body_subject.program_fingerprint.clone(),
        function_identity,
        ssa_module_identity: Some(module.identity.clone()),
        ssa_module_fingerprint: module.fingerprint().ok(),
        hir_fingerprint: Some(module.hir_fingerprint.clone()),
        body: body_subject,
        ssa: ssa_subject,
        mismatches,
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_instruction(
    program: &Program,
    module: &SsaModule,
    instruction: &SsaInstruction,
    values: &mut BTreeMap<SemanticId, ExecutionValue>,
    result: &mut SsaExecutionResult,
    request: &crate::ExecutionRequest,
    block_cache: Option<&BTreeMap<SemanticId, BTreeMap<SemanticId, usize>>>,
    cached_identity: Option<(&SemanticId, &String, &String)>,
    call_depth: u64,
) -> bool {
    match &instruction.kind {
        SsaInstructionKind::Constant { value, ty } => {
            let Some(value) = constant_value(*value, ty) else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    instruction_identity(instruction),
                    "constant type or value is outside the reference subset",
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), value);
            }
        }
        SsaInstructionKind::RecordConstruct {
            type_identity,
            field_names,
        } => {
            let Some(output) = instruction.outputs.first() else {
                return true;
            };
            if field_names.len() != instruction.inputs.len() {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "record construction field count does not match its operands",
                );
                return true;
            }
            let mut fields = Vec::new();
            for (name, input) in field_names.iter().zip(&instruction.inputs) {
                let Some(value) = values.get(input) else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        "record construction operand was unavailable",
                    );
                    return true;
                };
                fields.push((name.clone(), value.clone()));
            }
            fields.sort_by(|left, right| left.0.cmp(&right.0));
            let name = match &output.ty {
                IrType::Record { name, .. } | IrType::Named(name) => name.clone(),
                IrType::Finite { name, .. } => name.clone(),
            };
            values.insert(
                output.identity.clone(),
                ExecutionValue::Record {
                    type_identity: type_identity.clone(),
                    name,
                    fields: fields.into(),
                },
            );
        }
        SsaInstructionKind::RecordProject { field, .. } => {
            let Some(input) = instruction.inputs.first() else {
                return true;
            };
            let Some(ExecutionValue::Record { fields, .. }) = values.get(input) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "record projection operand is not a record value",
                );
                return true;
            };
            match fields.iter().find(|(name, _)| name == field) {
                Some((_, value)) => {
                    if let Some(output) = instruction.outputs.first() {
                        values.insert(output.identity.clone(), value.clone());
                    }
                }
                None => {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        "record has no such field",
                    );
                    return true;
                }
            }
        }
        SsaInstructionKind::Integer {
            operator,
            operand_type,
            intent,
        } => {
            if !integer_operator_supported(operator, *intent) {
                result.fail(
                    ExecutionStatus::Unsupported,
                    instruction_identity(instruction),
                    format!("unsupported integer operator or intent {operator:?}/{intent:?}"),
                );
                return true;
            }
            let Some((left, right)) = integer_operands(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "integer operation operands were unavailable",
                );
                return true;
            };
            let Some(value) = evaluate_integer(operator, *operand_type, *intent, left, right)
            else {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    instruction_identity(instruction),
                    format!("integer {operator} overflow or did not produce a value"),
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Integer {
                        value,
                        ty: match &output.ty {
                            crate::IrType::Named(name) => {
                                match crate::BodyType::from_semantic_name(name) {
                                    crate::BodyType::Integer(ty) => ty,
                                    _ => *operand_type,
                                }
                            }
                            _ => *operand_type,
                        },
                    },
                );
            }
        }
        SsaInstructionKind::IntegerCompare {
            predicate,
            operand_type,
        } => {
            let Some((left, right)) = integer_operands(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "integer comparison operands were unavailable",
                );
                return true;
            };
            let Some(value) = compare_integers(predicate, *operand_type, left, right) else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    instruction_identity(instruction),
                    format!("unsupported integer comparison predicate {predicate:?}"),
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), ExecutionValue::Boolean { value });
            }
        }
        SsaInstructionKind::FloatConstant { bits, ty } => {
            if !ty.is_supported() {
                result.fail(
                    ExecutionStatus::Unsupported,
                    instruction_identity(instruction),
                    "only binary64 float constants are supported",
                );
                return true;
            }
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Float {
                        bits: *bits,
                        ty: *ty,
                    },
                );
            }
        }
        SsaInstructionKind::Float { operator } => {
            if !matches!(operator.as_str(), "add" | "sub" | "mul" | "div") {
                result.fail(
                    ExecutionStatus::Unsupported,
                    instruction_identity(instruction),
                    format!("unsupported float operator {operator:?}"),
                );
                return true;
            }
            let Some((left, right)) = float_operands(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "float operation operands were unavailable or not float values",
                );
                return true;
            };
            let Some(value) = evaluate_float(operator, left, right) else {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    instruction_identity(instruction),
                    format!("float {operator} trapped on a non-finite input or result"),
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Float {
                        bits: value.to_bits(),
                        ty: crate::FloatType::f64(),
                    },
                );
            }
        }
        SsaInstructionKind::FloatCompare { predicate } => {
            let Some((left, right)) = float_operands(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "float comparison operands were unavailable or not float values",
                );
                return true;
            };
            if !left.is_finite() || !right.is_finite() {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    instruction_identity(instruction),
                    "float comparison trapped on a non-finite input",
                );
                return true;
            }
            let Some(value) = compare_floats(predicate, left, right) else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    instruction_identity(instruction),
                    format!("unsupported float comparison predicate {predicate:?}"),
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), ExecutionValue::Boolean { value });
            }
        }
        SsaInstructionKind::FloatIntrinsic { function } => {
            let Some(input) = float_operand(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "float intrinsic operand was unavailable or not a float value",
                );
                return true;
            };
            if !input.is_finite() {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    instruction_identity(instruction),
                    format!("float {function} trapped on a non-finite input"),
                );
                return true;
            }
            let value = match function.as_str() {
                "sin" => input.sin(),
                "cos" => input.cos(),
                // Exact IEEE-754 negation (total on finite inputs;
                // non-finite inputs already trapped above, and the
                // result of negating a finite input is finite, so the
                // non-finite-result trap below cannot fire for neg).
                "neg" => -input,
                _ => {
                    result.fail(
                        ExecutionStatus::Unsupported,
                        instruction_identity(instruction),
                        format!("unsupported float intrinsic {function:?}"),
                    );
                    return true;
                }
            };
            if !value.is_finite() {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    instruction_identity(instruction),
                    format!("float {function} trapped on a non-finite result"),
                );
                return true;
            }
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Float {
                        bits: value.to_bits(),
                        ty: crate::FloatType::f64(),
                    },
                );
            }
        }
        SsaInstructionKind::BooleanOp { operator } => {
            let Some((left, right)) = boolean_operands(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "boolean operation operands were unavailable or not boolean",
                );
                return true;
            };
            let value = match operator.as_str() {
                "and" => left && right,
                "or" => left || right,
                _ => {
                    result.fail(
                        ExecutionStatus::Unsupported,
                        instruction_identity(instruction),
                        format!("unsupported boolean operator {operator:?}"),
                    );
                    return true;
                }
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), ExecutionValue::Boolean { value });
            }
        }
        SsaInstructionKind::BooleanCompare { predicate } => {
            let Some((left, right)) = boolean_operands(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "boolean comparison operands were unavailable or not boolean",
                );
                return true;
            };
            let value = match predicate.as_str() {
                "eq" => left == right,
                "ne" => left != right,
                _ => {
                    result.fail(
                        ExecutionStatus::Unsupported,
                        instruction_identity(instruction),
                        format!("unsupported boolean comparison predicate {predicate:?}"),
                    );
                    return true;
                }
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), ExecutionValue::Boolean { value });
            }
        }
        SsaInstructionKind::BooleanNot => {
            let [input] = instruction.inputs.as_slice() else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "boolean negation requires one operand",
                );
                return true;
            };
            let Some(ExecutionValue::Boolean { value }) = values.get(input) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "boolean negation operand was unavailable or not boolean",
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Boolean { value: !value },
                );
            }
        }
        SsaInstructionKind::ByteBitwise { operator } => {
            let Some((left, right)) = byte_operands(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "byte bitwise operands were unavailable or not byte values",
                );
                return true;
            };
            let Some(value) = crate::execution::evaluate_byte_bitwise(operator, left, right) else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    instruction_identity(instruction),
                    format!("unsupported byte bitwise operator {operator:?}"),
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), ExecutionValue::Byte { value });
            }
        }
        SsaInstructionKind::ByteShift { operator } => {
            let Some((byte, count)) = byte_shift_operands(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "byte shift operands were unavailable or mistyped",
                );
                return true;
            };
            let Some(value) = crate::execution::evaluate_byte_shift(operator, byte, count) else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    instruction_identity(instruction),
                    format!("unsupported byte shift operator {operator:?}"),
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), ExecutionValue::Byte { value });
            }
        }
        SsaInstructionKind::ByteCompare { predicate } => {
            let Some((left, right)) = byte_operands(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "byte comparison operands were unavailable or not byte values",
                );
                return true;
            };
            let Some(value) = crate::execution::compare_bytes(predicate, left, right) else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    instruction_identity(instruction),
                    format!("unsupported byte comparison predicate {predicate:?}"),
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), ExecutionValue::Boolean { value });
            }
        }
        SsaInstructionKind::Convert { from, to } => {
            let Some(operand_value) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "conversion operand was unavailable",
                );
                return true;
            };
            match crate::execution::convert_scalar_value(operand_value, from, to) {
                Ok(converted) => {
                    if let Some(output) = instruction.outputs.first() {
                        values.insert(output.identity.clone(), converted);
                    }
                }
                Err(ExecutionStatus::RuntimeFailure) => {
                    result.fail(
                        ExecutionStatus::RuntimeFailure,
                        instruction_identity(instruction),
                        "float conversion trapped on a non-finite or out-of-range input",
                    );
                    return true;
                }
                Err(_) => {
                    result.fail(
                        ExecutionStatus::Unsupported,
                        instruction_identity(instruction),
                        "conversion operands do not match the declared conversion",
                    );
                    return true;
                }
            }
        }
        SsaInstructionKind::SequenceConstruct { length, .. } => {
            let Some(output) = instruction.outputs.first() else {
                return true;
            };
            if *length as usize != instruction.inputs.len() {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "sequence construction element count does not match its operands",
                );
                return true;
            }
            let mut element_values = Vec::with_capacity(instruction.inputs.len());
            for input in &instruction.inputs {
                let Some(value) = values.get(input) else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        "sequence construction operand was unavailable",
                    );
                    return true;
                };
                element_values.push(value.clone());
            }
            values.insert(
                output.identity.clone(),
                ExecutionValue::Sequence {
                    values: element_values.into(),
                },
            );
        }
        SsaInstructionKind::VectorConstruct { lanes, .. } => {
            let Some(output) = instruction.outputs.first() else {
                return true;
            };
            let collected: Option<Vec<_>> = instruction
                .inputs
                .iter()
                .map(|input| values.get(input).cloned())
                .collect();
            let Some(collected) = collected else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "vector construction operand was unavailable",
                );
                return true;
            };
            if collected.len() != *lanes as usize {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "vector construction lane count mismatch",
                );
                return true;
            }
            values.insert(
                output.identity.clone(),
                ExecutionValue::Vector {
                    values: collected.into(),
                },
            );
        }
        SsaInstructionKind::VectorSplat { lanes, .. } => {
            let Some(output) = instruction.outputs.first() else {
                return true;
            };
            let Some(value) = values
                .get(instruction.inputs.first().unwrap_or(&output_sentinel()))
                .cloned()
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "vector splat operand was unavailable",
                );
                return true;
            };
            values.insert(
                output.identity.clone(),
                ExecutionValue::Vector {
                    values: vec![value; *lanes as usize].into(),
                },
            );
        }
        SsaInstructionKind::VectorExtract {
            lanes, evidence, ..
        } => {
            let Some(ExecutionValue::Vector { values: vector }) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "vector extraction operand was unavailable",
                );
                return true;
            };
            let Some(index) = ssa_integer_operand(instruction, values, 1) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "vector lane index was unavailable",
                );
                return true;
            };
            if index < 0 || index as u128 >= u128::from(*lanes) {
                let status = if matches!(evidence, crate::BoundsEvidence::RuntimeChecked { .. }) {
                    ExecutionStatus::RuntimeFailure
                } else {
                    ExecutionStatus::InvalidRequest
                };
                result.fail(
                    status,
                    instruction_identity(instruction),
                    "vector lane index is out of bounds",
                );
                return true;
            }
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), vector[index as usize].clone());
            }
        }
        SsaInstructionKind::VectorReplace {
            lanes, evidence, ..
        } => {
            let Some(ExecutionValue::Vector { values: source }) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "vector replacement source was unavailable",
                );
                return true;
            };
            let Some(index) = ssa_integer_operand(instruction, values, 1) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "vector lane index was unavailable",
                );
                return true;
            };
            let Some(element) = values
                .get(instruction.inputs.get(2).unwrap_or(&output_sentinel()))
                .cloned()
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "vector replacement element was unavailable",
                );
                return true;
            };
            if index < 0 || index as u128 >= u128::from(*lanes) {
                let status = if matches!(evidence, crate::BoundsEvidence::RuntimeChecked { .. }) {
                    ExecutionStatus::RuntimeFailure
                } else {
                    ExecutionStatus::InvalidRequest
                };
                result.fail(
                    status,
                    instruction_identity(instruction),
                    "vector lane index is out of bounds",
                );
                return true;
            }
            let mut updated = source.as_ref().clone();
            updated[index as usize] = element;
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Vector {
                        values: updated.into(),
                    },
                );
            }
        }
        SsaInstructionKind::VectorBinary {
            operator,
            element_type,
            intent,
            ..
        } => {
            let Some((left, right)) = ssa_vector_operands(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "vector binary operands were unavailable",
                );
                return true;
            };
            let Some(produced) = crate::execution::evaluate_vector_binary(
                operator,
                element_type,
                *intent,
                left,
                right,
            ) else {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    instruction_identity(instruction),
                    "vector arithmetic failed in at least one lane",
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Vector {
                        values: produced.into(),
                    },
                );
            }
        }
        SsaInstructionKind::VectorCompare {
            predicate,
            element_type,
            ..
        } => {
            let Some((left, right)) = ssa_vector_operands(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "vector comparison operands were unavailable",
                );
                return true;
            };
            let Some(lanes) =
                crate::execution::evaluate_vector_compare(predicate, element_type, left, right)
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "vector comparison operands violated their element type",
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Mask {
                        lanes: lanes.into(),
                    },
                );
            }
        }
        SsaInstructionKind::MaskBinary { operator, .. } => {
            let Some((left, right)) = ssa_mask_operands(instruction, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "mask operands were unavailable",
                );
                return true;
            };
            let lanes: Vec<bool> = left
                .iter()
                .zip(right)
                .map(|(left, right)| match operator.as_str() {
                    "and" => *left && *right,
                    "or" => *left || *right,
                    _ => *left != *right,
                })
                .collect();
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Mask {
                        lanes: lanes.into(),
                    },
                );
            }
        }
        SsaInstructionKind::MaskNot { .. } => {
            let Some(ExecutionValue::Mask { lanes }) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "mask not operand was unavailable",
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Mask {
                        lanes: lanes.iter().map(|lane| !lane).collect::<Vec<_>>().into(),
                    },
                );
            }
        }
        SsaInstructionKind::MaskReduce { operator, .. } => {
            let Some(ExecutionValue::Mask { lanes }) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "mask reduction operand was unavailable",
                );
                return true;
            };
            let value = match operator.as_str() {
                "any" => lanes.iter().any(|lane| *lane),
                "all" => lanes.iter().all(|lane| *lane),
                _ => !lanes.iter().any(|lane| *lane),
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), ExecutionValue::Boolean { value });
            }
        }
        SsaInstructionKind::VectorReduce {
            operator,
            element_type,
            intent,
            ..
        } => {
            let Some(ExecutionValue::Vector { values: lanes }) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "vector reduction operand was unavailable",
                );
                return true;
            };
            let Some(value) =
                crate::execution::evaluate_vector_reduce(operator, element_type, *intent, lanes)
            else {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    instruction_identity(instruction),
                    "vector reduction failed",
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), value);
            }
        }
        SsaInstructionKind::SequenceProject { bound: _, evidence } => {
            let Some(ExecutionValue::Sequence { values: elements }) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "sequence projection operand was unavailable or not a sequence",
                );
                return true;
            };
            let Some(index) = ssa_integer_operand(instruction, values, 1) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "sequence index operand was unavailable or not a u64",
                );
                return true;
            };
            if index < 0 {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "sequence index is outside the u64 domain",
                );
                return true;
            }
            let length = elements.len() as u128;
            let index = index as u128;
            match evidence {
                crate::BoundsEvidence::StaticExact
                | crate::BoundsEvidence::TraversalDomain
                | crate::BoundsEvidence::CheckedBound => {
                    if index >= length {
                        result.fail(
                            ExecutionStatus::InvalidRequest,
                            instruction_identity(instruction),
                            "statically established sequence bounds were violated",
                        );
                        return true;
                    }
                }
                crate::BoundsEvidence::RuntimeChecked { failure: _ } => {
                    if index >= length {
                        result.fail(
                            ExecutionStatus::RuntimeFailure,
                            instruction_identity(instruction),
                            "sequence index out of bounds",
                        );
                        return true;
                    }
                }
            }
            if let Some(output) = instruction.outputs.first() {
                if let Some(value) = elements.get(index as usize) {
                    values.insert(output.identity.clone(), value.clone());
                }
            }
        }
        SsaInstructionKind::Select { operand_type: _ } => {
            let Some(condition) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "selection condition was unavailable",
                );
                return true;
            };
            if let ExecutionValue::Mask { lanes } = condition {
                let Some(ExecutionValue::Vector { values: true_lanes }) =
                    values.get(&instruction.inputs[1])
                else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        "masked selection true candidate was not a vector",
                    );
                    return true;
                };
                let Some(ExecutionValue::Vector {
                    values: false_lanes,
                }) = values.get(&instruction.inputs[2])
                else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        "masked selection false candidate was not a vector",
                    );
                    return true;
                };
                if lanes.len() != true_lanes.len() || lanes.len() != false_lanes.len() {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        "masked selection lane count mismatch",
                    );
                    return true;
                }
                let selected = lanes
                    .iter()
                    .enumerate()
                    .map(|(index, lane)| {
                        if *lane {
                            true_lanes[index].clone()
                        } else {
                            false_lanes[index].clone()
                        }
                    })
                    .collect::<Vec<_>>();
                if let Some(output) = instruction.outputs.first() {
                    values.insert(
                        output.identity.clone(),
                        ExecutionValue::Vector {
                            values: selected.into(),
                        },
                    );
                }
                return false;
            }
            let ExecutionValue::Boolean { value: chosen_true } = condition else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "selection condition was neither bool nor mask",
                );
                return true;
            };
            let candidate_index = if *chosen_true { 1 } else { 2 };
            let Some(selected) = values
                .get(
                    instruction
                        .inputs
                        .get(candidate_index)
                        .unwrap_or(&output_sentinel()),
                )
                .cloned()
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "selection candidate was unavailable",
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), selected);
            }
        }
        SsaInstructionKind::SequenceReplace {
            bound, evidence, ..
        } => {
            let crate::SequenceBound::Exact(length) = bound else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    instruction_identity(instruction),
                    "functional sequence update requires an exact bound",
                );
                return true;
            };
            let Some(ExecutionValue::Sequence { values: source }) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "functional update source was unavailable or not a sequence",
                );
                return true;
            };
            let source = source.clone();
            let Some(index) = ssa_integer_operand(instruction, values, 1) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "functional update index was unavailable or not a u64",
                );
                return true;
            };
            let Some(element) = values
                .get(instruction.inputs.get(2).unwrap_or(&output_sentinel()))
                .cloned()
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "functional update element was unavailable",
                );
                return true;
            };
            if index < 0 || index as u128 >= u128::from(*length) {
                match evidence {
                    crate::BoundsEvidence::RuntimeChecked { .. } => {
                        result.fail(
                            ExecutionStatus::RuntimeFailure,
                            instruction_identity(instruction),
                            format!(
                                "functional update index {index} is outside the exact bound {length}"
                            ),
                        );
                    }
                    _ => {
                        result.fail(
                            ExecutionStatus::RuntimeFailure,
                            instruction_identity(instruction),
                            "statically established update index violated the declared bound",
                        );
                    }
                }
                return true;
            }
            let mut updated = source.as_ref().clone();
            updated[index as usize] = element;
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Sequence {
                        values: updated.into(),
                    },
                );
            }
        }
        SsaInstructionKind::BoundCheck { .. } => {
            let Some(ExecutionValue::Sequence { values: sequence }) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "checked-index sequence was unavailable or not a sequence",
                );
                return true;
            };
            let Some(index) = ssa_integer_operand(instruction, values, 1) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "checked-index candidate was unavailable or not u64",
                );
                return true;
            };
            if index < 0 || index as u128 >= sequence.len() as u128 {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    instruction_identity(instruction),
                    format!(
                        "checked index {index} escapes the sequence length {}",
                        sequence.len()
                    ),
                );
                return true;
            }
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Integer {
                        value: index,
                        ty: IntegerType {
                            bits: 64,
                            signed: false,
                        },
                    },
                );
            }
        }
        SsaInstructionKind::SequenceCopy {
            dst_bound,
            src_bound: _,
            evidence,
            ..
        } => {
            let crate::SequenceBound::Exact(dst_length) = dst_bound else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    instruction_identity(instruction),
                    "functional span copy requires an exact-bound destination",
                );
                return true;
            };
            let Some(ExecutionValue::Sequence {
                values: destination,
            }) = values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "span copy destination was unavailable or not a sequence",
                );
                return true;
            };
            let destination = destination.clone();
            let Some(ExecutionValue::Sequence { values: source }) =
                values.get(instruction.inputs.get(2).unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "span copy source was unavailable or not a sequence",
                );
                return true;
            };
            let source = source.clone();
            let (Some(dst_at), Some(src_at), Some(len)) = (
                ssa_integer_operand(instruction, values, 1),
                ssa_integer_operand(instruction, values, 3),
                ssa_integer_operand(instruction, values, 4),
            ) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "span copy offsets or length were unavailable or not u64",
                );
                return true;
            };
            let windows_valid = [dst_at, src_at, len].iter().all(|v| *v >= 0)
                && (dst_at as u128) + (len as u128) <= u128::from(*dst_length)
                && (src_at as u128) + (len as u128) <= source.len() as u128;
            if !windows_valid {
                match evidence {
                    crate::BoundsEvidence::RuntimeChecked { .. } => {
                        result.fail(
                            ExecutionStatus::RuntimeFailure,
                            instruction_identity(instruction),
                            format!(
                                "span copy range (dst_at {dst_at}, src_at {src_at}, len {len}) exceeds dst bound {dst_length} or src length {}",
                                source.len()
                            ),
                        );
                    }
                    _ => {
                        result.fail(
                            ExecutionStatus::RuntimeFailure,
                            instruction_identity(instruction),
                            "statically established span copy range violated the declared bounds",
                        );
                    }
                }
                return true;
            }
            let mut updated = destination.as_ref().clone();
            updated[(dst_at as usize)..((dst_at as usize) + (len as usize))]
                .clone_from_slice(&source[(src_at as usize)..((src_at as usize) + (len as usize))]);
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Sequence {
                        values: updated.into(),
                    },
                );
            }
        }
        SsaInstructionKind::SequenceLength { bound: _ } => {
            let Some(ExecutionValue::Sequence { values: elements }) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "length observation operand was unavailable or not a sequence",
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Integer {
                        value: elements.len() as i128,
                        ty: IntegerType {
                            bits: 64,
                            signed: false,
                        },
                    },
                );
            }
        }
        SsaInstructionKind::ViewConstruct {
            source_bound: _,
            view_bound,
        } => {
            let crate::SequenceBound::UpTo(capacity) = view_bound else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "view construction must produce an UpTo-bounded view",
                );
                return true;
            };
            let Some(ExecutionValue::Sequence { values: source }) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "view construction source was unavailable or not a sequence",
                );
                return true;
            };
            let (Some(start), Some(end)) = (
                ssa_integer_operand(instruction, values, 1),
                ssa_integer_operand(instruction, values, 2),
            ) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "view range endpoints were unavailable or not u64 values",
                );
                return true;
            };
            if start < 0 || end < 0 || end < start {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    instruction_identity(instruction),
                    "view range is not a valid half-open range",
                );
                return true;
            }
            if end as u128 > source.len() as u128 {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    instruction_identity(instruction),
                    "view range end exceeds the source length",
                );
                return true;
            }
            if (end - start) as u128 > u128::from(*capacity) {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    instruction_identity(instruction),
                    "view range exceeds the declared view capacity",
                );
                return true;
            }
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Sequence {
                        values: source[start as usize..end as usize].to_vec().into(),
                    },
                );
            }
        }
        SsaInstructionKind::ViewNarrow {
            source_cap: _,
            new_cap,
        } => {
            let Some(ExecutionValue::Sequence { values: source }) =
                values.get(instruction.inputs.first().unwrap_or(&output_sentinel()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "view narrowing source was unavailable or not a sequence",
                );
                return true;
            };
            if source.len() as u128 > u128::from(*new_cap) {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    instruction_identity(instruction),
                    format!(
                        "view span {} exceeds the narrowed capacity {new_cap}",
                        source.len()
                    ),
                );
                return true;
            }
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Sequence {
                        values: source.clone(),
                    },
                );
            }
        }
        SsaInstructionKind::FiniteConstruct {
            type_identity,
            variant_identity,
            discriminant,
            payload_fields,
        } => {
            if let Some(output) = instruction.outputs.first() {
                let mut payload = Vec::new();
                let mut available = true;
                for (index, field) in payload_fields.iter().enumerate() {
                    match values.get(instruction.inputs.get(index).unwrap_or(&output.identity)) {
                        Some(value) => payload.push((field.clone(), value.clone())),
                        None => available = false,
                    }
                }
                if !available || instruction.inputs.len() != payload_fields.len() {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        "finite constructor payload operand was unavailable",
                    );
                    return true;
                }
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Finite {
                        type_identity: type_identity.clone(),
                        variant_identity: variant_identity.clone(),
                        discriminant: *discriminant,
                        payload: payload.into(),
                    },
                );
            }
        }
        SsaInstructionKind::FinitePayloadProject {
            type_identity,
            variant_identity,
            discriminant,
            field,
        } => {
            let Some(ExecutionValue::Finite {
                type_identity: actual_type,
                variant_identity: actual_variant,
                discriminant: actual_discriminant,
                payload,
            }) = instruction
                .inputs
                .first()
                .and_then(|input| values.get(input))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "payload projection operand was unavailable or not a finite value",
                );
                return true;
            };
            if actual_type != type_identity
                || actual_variant != variant_identity
                || actual_discriminant != discriminant
            {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "payload projection operand has an invalid type/variant/discriminant",
                );
                return true;
            }
            match payload.iter().find(|(name, _)| name == field) {
                Some((_, value)) => {
                    if let Some(output) = instruction.outputs.first() {
                        let output_identity = output.identity.clone();
                        values.insert(output_identity, value.clone());
                    }
                }
                None => {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        format!("payload projection field {field:?} was not carried by the value"),
                    );
                    return true;
                }
            }
        }
        SsaInstructionKind::FiniteIsVariant {
            type_identity,
            variant_identity,
            discriminant,
        } => {
            let Some(ExecutionValue::Finite {
                type_identity: actual_type,
                variant_identity: actual_variant,
                discriminant: actual_discriminant,
                ..
            }) = instruction
                .inputs
                .first()
                .and_then(|input| values.get(input))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "finite variant test operand was unavailable",
                );
                return true;
            };
            if actual_type != type_identity
                || !crate::execution::valid_finite_value(
                    program,
                    actual_type,
                    actual_variant,
                    *actual_discriminant,
                )
            {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "finite value has an invalid type/variant/discriminant",
                );
                return true;
            }
            if let Some(output) = instruction.outputs.first() {
                values.insert(
                    output.identity.clone(),
                    ExecutionValue::Boolean {
                        value: actual_variant == variant_identity
                            && actual_discriminant == discriminant,
                    },
                );
            }
        }
        SsaInstructionKind::Call { function, .. } => {
            let Some(callee) = program.functions.iter().find(|candidate| {
                function_id(
                    candidate.identity_namespace(&program.module),
                    &candidate.name,
                ) == *function
            }) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "SSA call target does not exist",
                );
                return true;
            };
            let Some(arguments) = instruction
                .inputs
                .iter()
                .map(|input| values.get(input).cloned())
                .collect::<Option<Vec<_>>>()
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "SSA call operands were unavailable",
                );
                return true;
            };
            let remaining = request.step_budget.saturating_sub(result.steps);
            if remaining == 0 {
                result.fail(
                    ExecutionStatus::BudgetExhausted,
                    instruction_identity(instruction),
                    "execution step budget exhausted before SSA call",
                );
                return true;
            }
            let nested_request = crate::ExecutionRequest {
                schema_version: request.schema_version.clone(),
                target: crate::ExecutionTarget {
                    module: callee.identity_namespace(&program.module).to_owned(),
                    function: callee.name.clone(),
                },
                arguments,
                // Nested callees are specialization-rewritten concrete
                // targets; a nested call still naming a generic template
                // fails closed in entry resolution below.
                type_arguments: Vec::new(),
                step_budget: remaining,
                policy: request.policy.clone(),
                // Authority flows explicitly to callees within one
                // execution: nested calls inherit the request's grants,
                // still bounded and still matched by capability name.
                host_grants: request.host_grants.clone(),
                // Depth fuel flows the same way (RFC 0047).
                call_depth_budget: request.call_depth_budget,
            };
            // Re-share the caller-resolved identity (cheap clone) so the
            // nested call never re-fingerprints the module. Resolved here,
            // on the Call path only, to keep every other instruction free
            // of cache bookkeeping.
            let owned_nested_cache = resolve_execution_cache(program, module, cached_identity);
            let nested_cache: Option<(&SemanticId, &String, &String)> = owned_nested_cache
                .as_ref()
                .map(|(identity, program_fingerprint, fingerprint)| {
                    (identity, program_fingerprint, fingerprint)
                });
            let nested = execute_ssa_module_with_validation(
                program,
                module,
                &nested_request,
                false,
                block_cache,
                nested_cache,
                None,
                call_depth + 1,
            );
            let trace_offset = result.steps;
            result.steps = result.steps.saturating_add(nested.steps);
            result.effects.extend(nested.effects.clone());
            for mut entry in nested.trace.clone() {
                entry.step = entry.step.saturating_add(trace_offset);
                if result.trace.len() < 256 {
                    result.trace.push(entry);
                } else {
                    result.trace_truncated = true;
                }
            }
            if nested.status != ExecutionStatus::Returned {
                result.status = nested.status;
                result.failure = nested.failure;
                return true;
            }
            let Some(returned) = nested.returned.first().cloned() else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    "SSA callee returned no value",
                );
                return true;
            };
            if let Some(output) = instruction.outputs.first() {
                values.insert(output.identity.clone(), returned);
            }
        }
        SsaInstructionKind::Effect => {
            if !matches!(request.policy.effects, crate::EffectExecutionPolicy::Record) {
                result.fail(ExecutionStatus::Unsupported, instruction_identity(instruction), "effect execution requires explicit record policy; no external side effect was performed");
                return true;
            }
            for effect in &instruction.effects {
                let declared = declared_effect(program, &request.target.function, effect);
                result.effects.push(ExecutionEffectEvent {
                    operation: instruction_identity(instruction).unwrap_or_else(|| effect.clone()),
                    kind: declared
                        .as_ref()
                        .map(|effect| effect.kind.clone())
                        .unwrap_or_else(|| format!("effect:{}", effect.0)),
                    target: declared
                        .as_ref()
                        .map(|effect| effect.target.clone())
                        .unwrap_or_else(|| effect.0.clone()),
                    capability: declared
                        .as_ref()
                        .map(|effect| effect.capability.clone())
                        .or_else(|| {
                            instruction
                                .capability_uses
                                .first()
                                .map(|use_| use_.capability.0.clone())
                        })
                        .unwrap_or_default(),
                    // Record-only observations realize nothing.
                    provenance: None,
                });
            }
        }
        SsaInstructionKind::HostCall {
            capability,
            operation,
        } => {
            // Host-realized value-producing operation, mirroring the body
            // reference executor (HARNESS-PRESSURE-004/005/006). Same
            // fail-closed discipline: explicit realize policy, explicit
            // grant for the declared capability, bounded operand views
            // (or a host clock observation for `clock_read`), recorded
            // provenance.
            if crate::host_call_arity(operation).is_none() {
                result.fail(
                    ExecutionStatus::Unsupported,
                    instruction_identity(instruction),
                    format!("unknown host operation {operation:?}; fail closed"),
                );
                return true;
            }
            // Observation versus realization, mirroring the body
            // reference executor: `Record` observes mutating intents
            // without performing them; only `Realize` mutates.
            let observing = matches!(request.policy.effects, crate::EffectExecutionPolicy::Record);
            if !observing
                && !matches!(
                    request.policy.effects,
                    crate::EffectExecutionPolicy::Realize
                )
            {
                result.fail(ExecutionStatus::Unsupported, instruction_identity(instruction), "host call requires the explicit realize policy with a matching grant; no external access was performed");
                return true;
            }
            let Some(grant) = request
                .host_grants
                .iter()
                .find(|grant| grant.capability == *capability)
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    format!("no host grant for capability {capability:?}; declared authority was not fulfilled"),
                );
                return true;
            };
            if grant.bytes.len() > crate::execution::HOST_GRANT_MAX_BYTES {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    instruction_identity(instruction),
                    format!(
                        "host grant for capability {capability:?} exceeds the {}-byte bound",
                        crate::execution::HOST_GRANT_MAX_BYTES
                    ),
                );
                return true;
            }
            if matches!(
                operation.as_str(),
                "fs_list_count"
                    | "fs_entry_name_at"
                    | "fs_entry_kind_at"
                    | "fs_generation"
                    | "fs_read_bytes_at"
            ) {
                // Granted-filesystem intrinsics (index PRESS-003), mirroring
                // the body reference executor arm-for-arm: same grant
                // matching, same u64-operand rule, same realization, so the
                // layered `check-backend-execution` comparison agrees.
                let arity = crate::host_call_arity(operation).unwrap_or(0);
                let mut fs_args = Vec::with_capacity(arity);
                for position in 0..arity {
                    let Some(arg) =
                        crate::fs_resource::host_u64_by(&instruction.inputs, values, position)
                    else {
                        result.fail(
                            ExecutionStatus::InvalidRequest,
                            instruction_identity(instruction),
                            format!(
                                "{operation} requires u64 index/offset/length operands, not a non-u64 value"
                            ),
                        );
                        return true;
                    };
                    fs_args.push(arg);
                }
                match crate::fs_resource::fs_realize(operation, grant, &fs_args) {
                    Ok((value, effect)) => {
                        // Success stores the value and records the effect,
                        // then returns `false` (continue with the next
                        // instruction): falling through would re-enter the
                        // host-call chain below and let the `blob_read`
                        // default overwrite this value.
                        if let Some(output) = instruction.outputs.first() {
                            values.insert(output.identity.clone(), value);
                        }
                        result.effects.push(ExecutionEffectEvent {
                            operation: instruction_identity(instruction).unwrap_or_else(|| {
                                crate::identity::SemanticId(format!("host-call:{capability}"))
                            }),
                            kind: effect.kind,
                            target: effect.target,
                            capability: capability.clone(),
                            provenance: Some(effect.provenance),
                        });
                        return false;
                    }
                    Err(crate::fs_resource::FsFail::InvalidRequest(reason)) => {
                        result.fail(
                            ExecutionStatus::InvalidRequest,
                            instruction_identity(instruction),
                            reason,
                        );
                        return true;
                    }
                    Err(crate::fs_resource::FsFail::RuntimeFailure(reason)) => {
                        result.fail(
                            ExecutionStatus::RuntimeFailure,
                            instruction_identity(instruction),
                            reason,
                        );
                        return true;
                    }
                }
            }
            if matches!(
                operation.as_str(),
                "fs_create_file"
                    | "fs_write_bytes_at"
                    | "fs_append_bytes_at"
                    | "fs_mkdir"
                    | "fs_delete_at"
                    | "fs_rename_at"
                    | "fs_sync_at"
            ) {
                // Granted-filesystem mutations (Tranche A), mirroring the
                // body reference executor arm-for-arm: same grant
                // matching, same scalar/view split through
                // `fs_view_positions`, same realization, same intent-only
                // path under `Record`, so the layered
                // `check-backend-execution` comparison agrees.
                let arity = crate::host_call_arity(operation).unwrap_or(0);
                let view_positions = crate::fs_resource::fs_view_positions(operation);
                let mut ints: Vec<u64> = Vec::new();
                let mut views: Vec<Vec<u8>> = Vec::new();
                for position in 0..arity {
                    if view_positions.contains(&position) {
                        let Some(view) = instruction.inputs.get(position).and_then(|input| {
                            crate::execution::host_view_bytes(values.get(input)?)
                        }) else {
                            result.fail(
                                ExecutionStatus::InvalidRequest,
                                instruction_identity(instruction),
                                format!(
                                    "{operation} requires byte-view operands at view positions, not a non-view value"
                                ),
                            );
                            return true;
                        };
                        views.push(view);
                    } else {
                        let Some(arg) =
                            crate::fs_resource::host_u64_by(&instruction.inputs, values, position)
                        else {
                            result.fail(
                                ExecutionStatus::InvalidRequest,
                                instruction_identity(instruction),
                                format!(
                                    "{operation} requires u64 index/offset operands, not a non-u64 value"
                                ),
                            );
                            return true;
                        };
                        ints.push(arg);
                    }
                }
                let intent_only = observing;
                debug_assert!(crate::execution::host_operation_mutates(operation));
                match crate::fs_resource::fs_mutate(operation, grant, &ints, &views, !intent_only) {
                    Ok((value, effect)) => {
                        if let Some(output) = instruction.outputs.first() {
                            values.insert(output.identity.clone(), value);
                        }
                        result.effects.push(ExecutionEffectEvent {
                            operation: instruction_identity(instruction).unwrap_or_else(|| {
                                crate::identity::SemanticId(format!("host-call:{capability}"))
                            }),
                            kind: effect.kind,
                            target: effect.target,
                            capability: capability.clone(),
                            provenance: (!intent_only).then_some(effect.provenance),
                        });
                        return false;
                    }
                    Err(crate::fs_resource::FsFail::InvalidRequest(reason)) => {
                        result.fail(
                            ExecutionStatus::InvalidRequest,
                            instruction_identity(instruction),
                            reason,
                        );
                        return true;
                    }
                    Err(crate::fs_resource::FsFail::RuntimeFailure(reason)) => {
                        result.fail(
                            ExecutionStatus::RuntimeFailure,
                            instruction_identity(instruction),
                            reason,
                        );
                        return true;
                    }
                }
            }
            if operation == "clock_read" {
                let Some(millis) = crate::execution::host_epoch_millis() else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        "host clock is unavailable; no instant was observed",
                    );
                    return true;
                };
                if let Some(output) = instruction.outputs.first() {
                    values.insert(
                        output.identity.clone(),
                        ExecutionValue::Integer {
                            value: millis as i128,
                            ty: IntegerType {
                                bits: 64,
                                signed: false,
                            },
                        },
                    );
                }
                result.effects.push(ExecutionEffectEvent {
                    operation: instruction_identity(instruction).unwrap_or_else(|| {
                        crate::identity::SemanticId(format!("host-call:{capability}"))
                    }),
                    kind: "clock_read".to_owned(),
                    target: operation.clone(),
                    capability: capability.clone(),
                    provenance: Some("host-clock:wall".to_owned()),
                });
            } else if operation == "sha256_digest" {
                let view = instruction
                    .inputs
                    .first()
                    .and_then(|input| crate::execution::host_view_bytes(values.get(input)?));
                let Some(view) = view else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        "sha256_digest requires one byte-view operand",
                    );
                    return true;
                };
                let digest = crate::execution::sha256_digest_bytes(&view);
                if let Some(output) = instruction.outputs.first() {
                    values.insert(
                        output.identity.clone(),
                        ExecutionValue::Sequence {
                            values: digest
                                .iter()
                                .map(|byte| ExecutionValue::Byte {
                                    value: i128::from(*byte),
                                })
                                .collect::<Vec<_>>()
                                .into(),
                        },
                    );
                }
                result.effects.push(ExecutionEffectEvent {
                    operation: instruction_identity(instruction).unwrap_or_else(|| {
                        crate::identity::SemanticId(format!("host-call:{capability}"))
                    }),
                    kind: "sha256_digest".to_owned(),
                    target: operation.clone(),
                    capability: capability.clone(),
                    provenance: Some("crypto:sha256".to_owned()),
                });
            } else if operation == "ed25519_verify" {
                let view_at = |position: usize| {
                    instruction
                        .inputs
                        .get(position)
                        .and_then(|input| crate::execution::host_view_bytes(values.get(input)?))
                };
                let (Some(key_bytes), Some(message), Some(signature_bytes)) =
                    (view_at(0), view_at(1), view_at(2))
                else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        "ed25519_verify requires byte-view public key, message, and signature",
                    );
                    return true;
                };
                let Some(valid) =
                    crate::execution::ed25519_verify_bytes(&key_bytes, &message, &signature_bytes)
                else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        "ed25519_verify requires a 32-byte public key and a 64-byte signature",
                    );
                    return true;
                };
                if let Some(output) = instruction.outputs.first() {
                    values.insert(
                        output.identity.clone(),
                        ExecutionValue::Boolean { value: valid },
                    );
                }
                result.effects.push(ExecutionEffectEvent {
                    operation: instruction_identity(instruction).unwrap_or_else(|| {
                        crate::identity::SemanticId(format!("host-call:{capability}"))
                    }),
                    kind: "ed25519_verify".to_owned(),
                    target: operation.clone(),
                    capability: capability.clone(),
                    provenance: Some("crypto:ed25519".to_owned()),
                });
            } else if operation == "blob_append" {
                let view = instruction
                    .inputs
                    .first()
                    .and_then(|input| crate::execution::host_view_bytes(values.get(input)?));
                let Some(view) = view else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        instruction_identity(instruction),
                        "host_write requires one byte-view operand",
                    );
                    return true;
                };
                // Mutating intent observed without realizing under
                // `Record` (mirrors the body executor): authority and view
                // validated, would-be count returned, intent recorded with
                // absent provenance, file untouched.
                debug_assert!(crate::execution::host_operation_mutates(operation));
                let (appended, intent_only) = if observing {
                    (view.len() as u64, true)
                } else {
                    match crate::execution::append_grant_bytes(&grant.locator, &view) {
                        Ok(count) => (count, false),
                        Err(reason) => {
                            result.fail(
                                ExecutionStatus::RuntimeFailure,
                                instruction_identity(instruction),
                                reason,
                            );
                            return true;
                        }
                    }
                };
                if let Some(output) = instruction.outputs.first() {
                    values.insert(
                        output.identity.clone(),
                        ExecutionValue::Integer {
                            value: appended as i128,
                            ty: IntegerType {
                                bits: 64,
                                signed: false,
                            },
                        },
                    );
                }
                result.effects.push(ExecutionEffectEvent {
                    operation: instruction_identity(instruction).unwrap_or_else(|| {
                        crate::identity::SemanticId(format!("host-call:{capability}"))
                    }),
                    kind: "host_write".to_owned(),
                    target: operation.clone(),
                    capability: capability.clone(),
                    provenance: (!intent_only).then(|| {
                        format!(
                            "grant:{} sha256:{}",
                            grant.locator,
                            crate::canonical::sha256_hex(&view)
                        )
                    }),
                });
            } else {
                if let Some(output) = instruction.outputs.first() {
                    let delivered: Vec<ExecutionValue> = grant
                        .bytes
                        .iter()
                        .map(|byte| ExecutionValue::Byte {
                            value: *byte as i128,
                        })
                        .collect();
                    values.insert(
                        output.identity.clone(),
                        ExecutionValue::Sequence {
                            values: delivered.into(),
                        },
                    );
                }
                result.effects.push(ExecutionEffectEvent {
                    operation: instruction_identity(instruction).unwrap_or_else(|| {
                        crate::identity::SemanticId(format!("host-call:{capability}"))
                    }),
                    kind: "host_read".to_owned(),
                    target: operation.clone(),
                    capability: capability.clone(),
                    provenance: Some(format!(
                        "grant:{} sha256:{}",
                        grant.locator,
                        crate::canonical::sha256_hex(&grant.bytes)
                    )),
                });
            }
        }
        SsaInstructionKind::RuntimeCheck { .. } => {
            result.fail(
                ExecutionStatus::Unsupported,
                instruction_identity(instruction),
                "runtime checks have no executable condition in the current body representation",
            );
            return true;
        }
    }
    false
}

fn declared_effect(
    program: &Program,
    _function_name: &str,
    identity: &SemanticId,
) -> Option<crate::Effect> {
    program.functions.iter().find_map(|function| {
        function
            .effects
            .iter()
            .enumerate()
            .find_map(|(occurrence, effect)| {
                let canonical =
                    serde_json::to_string(&crate::canonical::canonical_effect(effect)).ok()?;
                let candidate = crate::identity::effect_id(
                    function.identity_namespace(&program.module),
                    &function.name,
                    &canonical,
                    occurrence,
                );
                (candidate == *identity).then(|| effect.clone())
            })
    })
}

/// Full ABI mismatch diagnostic: function/entrypoint identity, argument
/// position, expected SSA input type (name + canonical identity), and the
/// received value summary. Keeps the historical message prefix so existing
/// consumers keep matching.
fn abi_mismatch_reason(
    program: &Program,
    request: &crate::ExecutionRequest,
    function: &SsaFunction,
    index: usize,
    expected: &BodyType,
    received: &ExecutionValue,
) -> String {
    let mut reason = format!(
        "argument does not match SSA input type: function {}::{} (ssa function {}), argument index {index}, expected {} ({}) but received {}",
        request.target.module,
        request.target.function,
        function.identity.0,
        expected.semantic_name(),
        expected.canonical_identity(),
        execution_value_summary(received)
    );
    // Name-based field mismatches carry the actionable detail: which field
    // disagrees and the expected-vs-received lists (WEB-P-011).
    if let Some(note) =
        aggregate_shape_note(program, received, expected, &format!("argument {index}"))
    {
        reason.push_str("; ");
        reason.push_str(&note);
    }
    reason
}

/// Field-identity detail for a rejected aggregate SSA argument, shared with
/// the reference interpreter so both paths diagnose identically.
fn aggregate_shape_note(
    program: &Program,
    value: &ExecutionValue,
    ty: &BodyType,
    path: &str,
) -> Option<String> {
    crate::value_contract::first_aggregate_mismatch(program, value, ty, path)
}

fn initialize_inputs(
    program: &Program,
    function: &SsaFunction,
    request: &crate::ExecutionRequest,
    result: &mut SsaExecutionResult,
    owned_arguments: Option<Vec<ExecutionValue>>,
) -> Option<BTreeMap<SemanticId, ExecutionValue>> {
    let argument_count = owned_arguments
        .as_ref()
        .map_or(request.arguments.len(), Vec::len);
    if function.inputs.len() != argument_count {
        result.fail(
            ExecutionStatus::InvalidRequest,
            None,
            "argument count does not match SSA inputs",
        );
        return None;
    }
    let mut owned_arguments =
        owned_arguments.map(|arguments| arguments.into_iter().map(Some).collect::<Vec<_>>());
    let mut values = BTreeMap::new();
    for (index, input) in function.inputs.iter().enumerate() {
        let Some(ty) = ssa_input_type(program, &input.ty) else {
            result.fail(
                ExecutionStatus::Unsupported,
                Some(input.identity.clone()),
                "SSA input type is outside the scalar reference subset",
            );
            return None;
        };
        if owned_arguments.is_some() {
            let argument = owned_arguments
                .as_ref()
                .and_then(|arguments| arguments.get(index).and_then(Option::as_ref))?;
            if !value_matches_type(program, argument, &ty) {
                let reason = abi_mismatch_reason(program, request, function, index, &ty, argument);
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(input.identity.clone()),
                    reason,
                );
                return None;
            }
            if let ExecutionValue::Finite {
                type_identity,
                variant_identity,
                discriminant,
                ..
            } = argument
            {
                if !crate::execution::valid_finite_value(
                    program,
                    type_identity,
                    variant_identity,
                    *discriminant,
                ) {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        Some(input.identity.clone()),
                        "finite argument has an invalid variant/discriminant",
                    );
                    return None;
                }
            }
        } else {
            let argument = request.arguments.get(index)?;
            if !value_matches_type(program, argument, &ty) {
                let reason = abi_mismatch_reason(program, request, function, index, &ty, argument);
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(input.identity.clone()),
                    reason,
                );
                return None;
            }
            if let ExecutionValue::Finite {
                type_identity,
                variant_identity,
                discriminant,
                ..
            } = argument
            {
                if !crate::execution::valid_finite_value(
                    program,
                    type_identity,
                    variant_identity,
                    *discriminant,
                ) {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        Some(input.identity.clone()),
                        "finite argument has an invalid variant/discriminant",
                    );
                    return None;
                }
            }
        }
        let normalized = if owned_arguments.is_some() {
            normalize_value_owned(
                program,
                owned_arguments.as_mut()?.get_mut(index)?.take(),
                &ty,
            )?
        } else {
            let argument = request.arguments.get(index)?;
            normalize_value(program, argument, &ty)?
        };
        values.insert(input.identity.clone(), normalized);
    }
    Some(values)
}

fn normalize_value_owned(
    program: &Program,
    value: Option<ExecutionValue>,
    ty: &BodyType,
) -> Option<ExecutionValue> {
    let value = value?;
    match (&value, ty) {
        (ExecutionValue::Finite { type_identity, .. }, BodyType::Finite { identity, .. })
            if type_identity == identity =>
        {
            Some(value)
        }
        (ExecutionValue::Record { type_identity, .. }, BodyType::Record { identity, .. })
            if type_identity == identity =>
        {
            Some(value)
        }
        (ExecutionValue::Record { name, .. }, BodyType::Named(named)) if name == named => {
            Some(value)
        }
        _ => normalize_value(program, &value, ty),
    }
}

fn assign_block_arguments(
    function: &SsaFunction,
    block_indices: &BTreeMap<SemanticId, usize>,
    target: &SemanticId,
    arguments: &[SemanticId],
    values: &mut BTreeMap<SemanticId, ExecutionValue>,
) -> bool {
    let Some(block_index) = block_indices.get(target).copied() else {
        return false;
    };
    let Some(block) = function.blocks.get(block_index) else {
        return false;
    };
    if block.parameters.len() != arguments.len() {
        return false;
    }
    let Some(incoming) = arguments
        .iter()
        .map(|argument| values.get(argument).cloned())
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    for (parameter, value) in block.parameters.iter().zip(incoming) {
        values.insert(parameter.identity.clone(), value);
    }
    true
}

fn boolean_operands(
    instruction: &SsaInstruction,
    values: &BTreeMap<SemanticId, ExecutionValue>,
) -> Option<(bool, bool)> {
    let [left, right] = instruction.inputs.as_slice() else {
        return None;
    };
    let Some(ExecutionValue::Boolean { value: left }) = values.get(left) else {
        return None;
    };
    let Some(ExecutionValue::Boolean { value: right }) = values.get(right) else {
        return None;
    };
    Some((*left, *right))
}

fn float_operand(
    instruction: &SsaInstruction,
    values: &BTreeMap<SemanticId, ExecutionValue>,
) -> Option<f64> {
    let [input] = instruction.inputs.as_slice() else {
        return None;
    };
    let ExecutionValue::Float { bits, .. } = values.get(input)? else {
        return None;
    };
    Some(f64::from_bits(*bits))
}

fn float_operands(
    instruction: &SsaInstruction,
    values: &BTreeMap<SemanticId, ExecutionValue>,
) -> Option<(f64, f64)> {
    let [left, right] = instruction.inputs.as_slice() else {
        return None;
    };
    let ExecutionValue::Float { bits: left, .. } = values.get(left)? else {
        return None;
    };
    let ExecutionValue::Float { bits: right, .. } = values.get(right)? else {
        return None;
    };
    Some((f64::from_bits(*left), f64::from_bits(*right)))
}

fn integer_operands(
    instruction: &SsaInstruction,
    values: &BTreeMap<SemanticId, ExecutionValue>,
) -> Option<(i128, i128)> {
    let [left, right] = instruction.inputs.as_slice() else {
        return None;
    };
    let ExecutionValue::Integer { value: left, .. } = values.get(left)? else {
        return None;
    };
    let ExecutionValue::Integer { value: right, .. } = values.get(right)? else {
        return None;
    };
    Some((*left, *right))
}

fn byte_operands(
    instruction: &SsaInstruction,
    values: &BTreeMap<SemanticId, ExecutionValue>,
) -> Option<(i128, i128)> {
    let [left, right] = instruction.inputs.as_slice() else {
        return None;
    };
    let ExecutionValue::Byte { value: left } = values.get(left)? else {
        return None;
    };
    let ExecutionValue::Byte { value: right } = values.get(right)? else {
        return None;
    };
    Some((*left, *right))
}

fn byte_shift_operands(
    instruction: &SsaInstruction,
    values: &BTreeMap<SemanticId, ExecutionValue>,
) -> Option<(i128, i128)> {
    let [left, right] = instruction.inputs.as_slice() else {
        return None;
    };
    let ExecutionValue::Byte { value: left } = values.get(left)? else {
        return None;
    };
    let ExecutionValue::Integer { value: right, .. } = values.get(right)? else {
        return None;
    };
    if *right < 0 {
        return None;
    }
    Some((*left, *right))
}

fn ssa_integer_operand(
    instruction: &SsaInstruction,
    values: &BTreeMap<SemanticId, ExecutionValue>,
    index: usize,
) -> Option<i128> {
    match values.get(instruction.inputs.get(index)?)? {
        ExecutionValue::Integer { value, .. } => Some(*value),
        _ => None,
    }
}

fn ssa_vector_operands<'a>(
    instruction: &SsaInstruction,
    values: &'a BTreeMap<SemanticId, ExecutionValue>,
) -> Option<(&'a [ExecutionValue], &'a [ExecutionValue])> {
    let ExecutionValue::Vector { values: left } = values.get(instruction.inputs.first()?)? else {
        return None;
    };
    let ExecutionValue::Vector { values: right } = values.get(instruction.inputs.get(1)?)? else {
        return None;
    };
    (left.len() == right.len()).then_some((left, right))
}

fn ssa_mask_operands<'a>(
    instruction: &SsaInstruction,
    values: &'a BTreeMap<SemanticId, ExecutionValue>,
) -> Option<(&'a [bool], &'a [bool])> {
    let ExecutionValue::Mask { lanes: left } = values.get(instruction.inputs.first()?)? else {
        return None;
    };
    let ExecutionValue::Mask { lanes: right } = values.get(instruction.inputs.get(1)?)? else {
        return None;
    };
    (left.len() == right.len()).then_some((left, right))
}

/// A stand-in identity for operand lookups when an instruction is malformed;
/// the lookup misses and the caller reports InvalidRequest.
fn output_sentinel() -> SemanticId {
    SemanticId("mncs:ssa-execution:sentinel".to_owned())
}

fn constant_value(value: i128, ty: &IrType) -> Option<ExecutionValue> {
    let ty = body_type(ty)?;
    match ty {
        BodyType::Integer(integer)
            if integer.bits == 1 && !integer.signed && matches!(value, 0 | 1) =>
        {
            Some(ExecutionValue::Boolean { value: value == 1 })
        }
        BodyType::Integer(integer) if in_range(value, integer) => {
            Some(ExecutionValue::Integer { value, ty: integer })
        }
        BodyType::Named(name) if name == "bool" && matches!(value, 0 | 1) => {
            Some(ExecutionValue::Boolean { value: value == 1 })
        }
        // Byte literals materialize through their unsigned 8-bit domain.
        BodyType::Byte if (0..=255).contains(&value) => Some(ExecutionValue::Byte { value }),
        _ => None,
    }
}

/// Resolves an SSA input type against the module's declarations. Sequences
/// of records keep only their spelling (`[Point; 4]`) in SSA type positions,
/// so nested composite elements are resolved through the program before any
/// argument is admitted.
fn ssa_input_type(program: &Program, ty: &IrType) -> Option<BodyType> {
    Some(match ty {
        IrType::Named(name) => BodyType::from_program(program, name),
        IrType::Finite { identity, name } => BodyType::Finite {
            identity: identity.clone(),
            name: name.clone(),
        },
        IrType::Record { identity, name } => BodyType::Record {
            identity: identity.clone(),
            name: name.clone(),
        },
    })
}

fn body_type(ty: &IrType) -> Option<BodyType> {
    Some(match ty {
        IrType::Named(name) => BodyType::from_semantic_name(name),
        IrType::Finite { identity, name } => BodyType::Finite {
            identity: identity.clone(),
            name: name.clone(),
        },
        IrType::Record { identity, name } => BodyType::Record {
            identity: identity.clone(),
            name: name.clone(),
        },
    })
}

fn value_matches_type(program: &Program, value: &ExecutionValue, ty: &BodyType) -> bool {
    match (value, ty) {
        (ExecutionValue::Integer { value, ty: actual }, BodyType::Integer(expected)) => {
            actual == expected && in_range(*value, *expected)
        }
        (ExecutionValue::Boolean { .. }, BodyType::Named(name)) if name == "bool" => true,
        (ExecutionValue::Boolean { .. }, BodyType::Integer(integer)) => {
            integer.bits == 1 && !integer.signed
        }
        (
            ExecutionValue::Finite {
                type_identity,
                variant_identity,
                payload,
                ..
            },
            BodyType::Finite { identity, name },
        ) => {
            // Host-facing nominal resolution (P1-014): a name-spelled
            // request resolves to exactly the expected declaration. The
            // leniency profile below is unchanged.
            crate::execution::nominal_identity_matches(identity, name, type_identity)
                && finite_payload_matches(program, identity, variant_identity, payload)
        }
        (
            ExecutionValue::Record {
                type_identity,
                fields,
                ..
            },
            BodyType::Record { identity, name },
        ) => {
            crate::execution::nominal_identity_matches(identity, name, type_identity)
                && record_fields_match(program, identity, fields)
        }
        // Sequence spellings reconstructed without a program leave record
        // elements as Named; admit them when the value's record name matches.
        // Field order is still resolved by name downstream, so a known
        // declaration is shape-checked here as well.
        (ExecutionValue::Record { name, .. }, BodyType::Named(named)) if name == named => {
            match program.record_types.iter().find(|decl| &decl.name == named) {
                Some(declaration) => {
                    let ExecutionValue::Record { fields, .. } = value else {
                        return false;
                    };
                    record_fields_match(program, &declaration.identity, fields)
                }
                None => true,
            }
        }
        (ExecutionValue::Byte { value }, BodyType::Byte) => (0..=255).contains(value),
        (
            ExecutionValue::Sequence { values },
            BodyType::Sequence {
                element,
                bound: crate::SequenceBound::Exact(length),
            },
        ) => {
            values.len() == *length as usize
                && values
                    .iter()
                    .all(|element_value| value_matches_type(program, element_value, element))
        }
        (
            ExecutionValue::Sequence { values },
            BodyType::Sequence {
                element,
                bound: crate::SequenceBound::UpTo(capacity),
            },
        ) => {
            values.len() <= *capacity as usize
                && values
                    .iter()
                    .all(|element_value| value_matches_type(program, element_value, element))
        }
        (ExecutionValue::Vector { values }, BodyType::Vector { element, lanes }) => {
            values.len() == *lanes as usize
                && values
                    .iter()
                    .all(|lane| value_matches_type(program, lane, element))
        }
        (ExecutionValue::Mask { lanes: bits }, BodyType::Mask { lanes }) => {
            bits.len() == *lanes as usize
        }
        // Binary64 arguments match by type only: finiteness is a runtime
        // trap (the guards check inputs), not a request rejection.
        (ExecutionValue::Float { ty: actual, .. }, BodyType::Float(expected)) => actual == expected,
        _ => false,
    }
}

/// A logical record value matches its declaration when every received field
/// resolves by name against the canonical declaration. Unknown declarations
/// stay lenient (identity was already checked by the caller); every known
/// declaration rejects duplicate, unknown, or missing fields (WEB-P-011).
fn record_fields_match(
    program: &Program,
    record_identity: &SemanticId,
    fields: &[(String, ExecutionValue)],
) -> bool {
    let Some(declaration) = program.record_types.iter().find(|decl| {
        crate::execution::nominal_identity_matches(&decl.identity, &decl.name, record_identity)
    }) else {
        return true;
    };
    let declared_names: Vec<String> = declaration
        .fields
        .iter()
        .map(|field| field.name.clone())
        .collect();
    let context = format!("record {:?}", declaration.name);
    let Ok(order) = crate::value_contract::order_fields_by_name(&context, &declared_names, fields)
    else {
        return false;
    };
    order
        .iter()
        .enumerate()
        .all(|(declared_index, received_index)| {
            let declared = &declaration.fields[declared_index];
            value_matches_named_type(program, &fields[*received_index].1, &declared.field_type)
        })
}

/// A finite payload matches when every received field resolves by name
/// against the canonical variant declaration (WEB-P-011).
fn finite_payload_matches(
    program: &Program,
    type_identity: &SemanticId,
    variant_identity: &SemanticId,
    payload: &[(String, ExecutionValue)],
) -> bool {
    let Some(variant) = program
        .finite_types
        .iter()
        .find(|decl| {
            crate::execution::nominal_identity_matches(&decl.identity, &decl.name, type_identity)
        })
        .and_then(|finite_type| {
            finite_type.variants.iter().find(|variant| {
                crate::execution::nominal_identity_matches(
                    &variant.identity,
                    &variant.name,
                    variant_identity,
                )
            })
        })
    else {
        return true;
    };
    let declared_names: Vec<String> = variant
        .payload
        .iter()
        .map(|field| field.name.clone())
        .collect();
    let context = format!("finite payload {variant_identity:?}");
    let Ok(order) = crate::value_contract::order_fields_by_name(&context, &declared_names, payload)
    else {
        return false;
    };
    order
        .iter()
        .enumerate()
        .all(|(declared_index, received_index)| {
            let declared = &variant.payload[declared_index];
            value_matches_named_type(program, &payload[*received_index].1, &declared.field_type)
        })
}

/// Resolve one declared field semantic type against the linked program so
/// nested nominal values validate recursively. Unknown spellings stay
/// lenient; every known spelling checks exactly.
fn value_matches_named_type(program: &Program, value: &ExecutionValue, name: &str) -> bool {
    if let Some(finite) = program.finite_types.iter().find(|decl| {
        crate::execution::nominal_identity_matches(
            &decl.identity,
            &decl.name,
            &crate::SemanticId(name.to_owned()),
        )
    }) {
        return value_matches_type(
            program,
            value,
            &BodyType::Finite {
                identity: finite.identity.clone(),
                name: finite.name.clone(),
            },
        );
    }
    if let Some(record) = program.record_types.iter().find(|decl| {
        crate::execution::nominal_identity_matches(
            &decl.identity,
            &decl.name,
            &crate::SemanticId(name.to_owned()),
        )
    }) {
        return match value {
            ExecutionValue::Record {
                type_identity,
                fields,
                ..
            } => {
                crate::execution::nominal_identity_matches(
                    &record.identity,
                    &record.name,
                    type_identity,
                ) && record_fields_match(program, &record.identity, fields)
            }
            _ => false,
        };
    }
    let derived = BodyType::from_program(program, name);
    if !matches!(derived, BodyType::Named(_)) {
        return value_matches_type(program, value, &derived);
    }
    if name == "bool" {
        return matches!(value, ExecutionValue::Boolean { .. });
    }
    if let Some(finite) = program.finite_types.iter().find(|decl| decl.name == name) {
        return value_matches_type(
            program,
            value,
            &BodyType::Finite {
                identity: finite.identity.clone(),
                name: finite.name.clone(),
            },
        );
    }
    if let Some(record) = program.record_types.iter().find(|decl| decl.name == name) {
        return match value {
            ExecutionValue::Record {
                type_identity,
                fields,
                ..
            } => {
                type_identity == &record.identity
                    && record_fields_match(program, &record.identity, fields)
            }
            _ => false,
        };
    }
    // Anything else has no program declaration to resolve against; like the
    // reference interpreter, reject rather than admit an unresolvable shape.
    false
}

fn normalize_value(
    program: &Program,
    value: &ExecutionValue,
    ty: &BodyType,
) -> Option<ExecutionValue> {
    match (value, ty) {
        (ExecutionValue::Integer { value, ty: actual }, BodyType::Integer(expected))
            if actual == expected
                && in_range(*value, *expected)
                && expected.bits == 1
                && !expected.signed =>
        {
            Some(ExecutionValue::Boolean { value: *value == 1 })
        }
        (ExecutionValue::Integer { value, ty: actual }, BodyType::Integer(expected))
            if actual == expected && in_range(*value, *expected) =>
        {
            Some(ExecutionValue::Integer {
                value: *value,
                ty: *actual,
            })
        }
        (ExecutionValue::Boolean { value }, BodyType::Named(name)) if name == "bool" => {
            Some(ExecutionValue::Boolean { value: *value })
        }
        (ExecutionValue::Boolean { value }, BodyType::Integer(integer))
            if integer.bits == 1 && !integer.signed =>
        {
            Some(ExecutionValue::Boolean { value: *value })
        }
        (
            ExecutionValue::Finite {
                type_identity,
                variant_identity,
                discriminant,
                payload,
            },
            BodyType::Finite { identity, .. },
        ) if type_identity == identity => {
            normalize_finite_payload(program, type_identity, variant_identity, payload).map(
                |normalized_payload| ExecutionValue::Finite {
                    type_identity: type_identity.clone(),
                    variant_identity: variant_identity.clone(),
                    discriminant: *discriminant,
                    payload: normalized_payload.into(),
                },
            )
        }
        (
            ExecutionValue::Record {
                type_identity,
                name,
                fields,
            },
            BodyType::Record { identity, .. },
        ) if type_identity == identity => normalize_record_fields(program, type_identity, fields)
            .map(|normalized_fields| ExecutionValue::Record {
                type_identity: type_identity.clone(),
                name: name.clone(),
                fields: normalized_fields.into(),
            }),
        (ExecutionValue::Record { name, .. }, BodyType::Named(named)) if name == named => {
            Some(value.clone())
        }
        (ExecutionValue::Byte { value }, BodyType::Byte) if (0..=255).contains(value) => {
            Some(ExecutionValue::Byte { value: *value })
        }
        // Binary64 arguments normalize to themselves: bits are already the
        // canonical form, and finiteness is enforced by runtime guards.
        (ExecutionValue::Float { bits, ty: actual }, BodyType::Float(expected))
            if actual == expected =>
        {
            Some(ExecutionValue::Float {
                bits: *bits,
                ty: *actual,
            })
        }
        (
            ExecutionValue::Sequence { values },
            BodyType::Sequence {
                element,
                bound: crate::SequenceBound::Exact(length),
            },
        ) if values.len() == *length as usize => {
            let mut normalized = Vec::with_capacity(values.len());
            for value in values.iter() {
                normalized.push(normalize_value(program, value, element)?);
            }
            Some(ExecutionValue::Sequence {
                values: normalized.into(),
            })
        }
        (
            ExecutionValue::Sequence { values },
            BodyType::Sequence {
                element,
                bound: crate::SequenceBound::UpTo(capacity),
            },
        ) if values.len() <= *capacity as usize => {
            let mut normalized = Vec::with_capacity(values.len());
            for value in values.iter() {
                normalized.push(normalize_value(program, value, element)?);
            }
            Some(ExecutionValue::Sequence {
                values: normalized.into(),
            })
        }
        (ExecutionValue::Vector { values }, BodyType::Vector { element, lanes })
            if values.len() == *lanes as usize =>
        {
            let mut normalized = Vec::with_capacity(values.len());
            for value in values.iter() {
                normalized.push(normalize_value(program, value, element)?);
            }
            Some(ExecutionValue::Vector {
                values: normalized.into(),
            })
        }
        (ExecutionValue::Mask { lanes: bits }, BodyType::Mask { lanes })
            if bits.len() == *lanes as usize =>
        {
            Some(ExecutionValue::Mask {
                lanes: bits.clone(),
            })
        }
        _ => None,
    }
}

/// Reorder received record fields into canonical declaration order,
/// normalizing each field value against its declared semantic type. Unknown
/// declarations keep the legacy clone (identity was already checked); every
/// known declaration resolves strictly by name (WEB-P-011).
fn normalize_record_fields(
    program: &Program,
    record_identity: &SemanticId,
    fields: &[(String, ExecutionValue)],
) -> Option<Vec<(String, ExecutionValue)>> {
    let Some(declaration) = program
        .record_types
        .iter()
        .find(|decl| &decl.identity == record_identity)
    else {
        return Some(fields.to_vec());
    };
    let declared_names: Vec<String> = declaration
        .fields
        .iter()
        .map(|field| field.name.clone())
        .collect();
    let context = format!("record {:?}", declaration.name);
    let order =
        crate::value_contract::order_fields_by_name(&context, &declared_names, fields).ok()?;
    let mut normalized = Vec::with_capacity(declaration.fields.len());
    for (declared_index, received_index) in order.iter().enumerate() {
        let declared = &declaration.fields[declared_index];
        let field_ty = BodyType::from_program(program, &declared.field_type);
        // Unresolvable spellings cannot occur after successful validation;
        // the Named fallback keeps them verbatim instead of failing.
        let field_value = match &field_ty {
            BodyType::Named(_) => fields[*received_index].1.clone(),
            _ => normalize_value(program, &fields[*received_index].1, &field_ty)?,
        };
        normalized.push((declared.name.clone(), field_value));
    }
    Some(normalized)
}

/// Reorder finite payload fields into canonical variant order (WEB-P-011).
fn normalize_finite_payload(
    program: &Program,
    type_identity: &SemanticId,
    variant_identity: &SemanticId,
    payload: &[(String, ExecutionValue)],
) -> Option<Vec<(String, ExecutionValue)>> {
    let Some(variant) = program
        .finite_types
        .iter()
        .find(|decl| &decl.identity == type_identity)
        .and_then(|finite_type| {
            finite_type
                .variants
                .iter()
                .find(|variant| &variant.identity == variant_identity)
        })
    else {
        return Some(payload.to_vec());
    };
    let declared_names: Vec<String> = variant
        .payload
        .iter()
        .map(|field| field.name.clone())
        .collect();
    let context = format!("finite payload {variant_identity:?}");
    let order =
        crate::value_contract::order_fields_by_name(&context, &declared_names, payload).ok()?;
    let mut normalized = Vec::with_capacity(variant.payload.len());
    for (declared_index, received_index) in order.iter().enumerate() {
        let declared = &variant.payload[declared_index];
        let field_ty = BodyType::from_program(program, &declared.field_type);
        let field_value = match &field_ty {
            BodyType::Named(_) => payload[*received_index].1.clone(),
            _ => normalize_value(program, &payload[*received_index].1, &field_ty)?,
        };
        normalized.push((declared.name.clone(), field_value));
    }
    Some(normalized)
}

fn integer_limits(bits: u16, signed: bool) -> Option<(i128, i128)> {
    if !(1..=126).contains(&bits) {
        return None;
    }
    if signed {
        let top = 1_i128.checked_shl(u32::from(bits - 1))?;
        Some((-top, top - 1))
    } else {
        Some((0, 1_i128.checked_shl(u32::from(bits))?.checked_sub(1)?))
    }
}

fn in_range(value: i128, ty: crate::IntegerType) -> bool {
    integer_limits(ty.bits, ty.signed)
        .is_some_and(|(minimum, maximum)| (minimum..=maximum).contains(&value))
}

fn instruction_identity(instruction: &SsaInstruction) -> Option<SemanticId> {
    instruction
        .semantic_identity
        .clone()
        .or_else(|| Some(instruction.identity.clone()))
}

fn trace_block(result: &mut SsaExecutionResult, block: &SsaBlock, event: &str) {
    if result.trace.len() >= MAX_SSA_TRACE_ENTRIES {
        result.trace_truncated = true;
        return;
    }
    result.trace.push(SsaExecutionTraceEntry {
        step: result.steps,
        block: block
            .semantic_identity
            .clone()
            .unwrap_or_else(|| block.identity.clone()),
        semantic_identity: block.semantic_identity.clone(),
        hir_identities: Vec::new(),
        ssa_identity: Some(block.identity.clone()),
        event: event.to_owned(),
    });
}

fn trace_instruction(
    result: &mut SsaExecutionResult,
    block: &SsaBlock,
    instruction: &SsaInstruction,
    event: &str,
) {
    if result.trace.len() >= MAX_SSA_TRACE_ENTRIES {
        result.trace_truncated = true;
        return;
    }
    result.trace.push(SsaExecutionTraceEntry {
        step: result.steps,
        block: block
            .semantic_identity
            .clone()
            .unwrap_or_else(|| block.identity.clone()),
        semantic_identity: instruction.semantic_identity.clone(),
        hir_identities: instruction.hir_identity.clone().into_iter().collect(),
        ssa_identity: Some(instruction.identity.clone()),
        event: event.to_owned(),
    });
}

fn consume_step(result: &mut SsaExecutionResult, budget: u64) -> bool {
    if result.steps >= budget {
        false
    } else {
        result.steps += 1;
        true
    }
}

fn observable_body_ssa(body: &ExecutionResult, ssa: &SsaExecutionResult) -> bool {
    body.status == ssa.status
        && body.returned == ssa.returned
        && body
            .failure
            .as_ref()
            .map(|failure| (&failure.identity, &failure.reason))
            == ssa
                .failure
                .as_ref()
                .map(|failure| (&failure.identity, &failure.reason))
        && body
            .effects
            .iter()
            .map(|effect| (&effect.kind, &effect.target, &effect.capability))
            .collect::<Vec<_>>()
            == ssa
                .effects
                .iter()
                .map(|effect| (&effect.kind, &effect.target, &effect.capability))
                .collect::<Vec<_>>()
}

fn divergence_context(
    body: &ExecutionResult,
    ssa: &SsaExecutionResult,
) -> LoweringDivergenceContext {
    let mut last_matching = None;
    let mut body_event = None;
    let mut ssa_event = None;
    for (left, right) in body.trace.iter().zip(&ssa.trace) {
        if left.operation == right.semantic_identity
            || (left.operation.is_none()
                && right.semantic_identity.is_none()
                && left.block == right.block
                && left.event == right.event)
        {
            last_matching.clone_from(&left.operation);
        } else {
            body_event = Some(left.event.clone());
            ssa_event = Some(right.event.clone());
            break;
        }
    }
    let related_hir_identities = ssa
        .trace
        .iter()
        .flat_map(|entry| entry.hir_identities.clone())
        .take(8)
        .collect();
    let related_ssa_identities = ssa
        .trace
        .iter()
        .filter_map(|entry| entry.ssa_identity.clone())
        .take(8)
        .collect();
    LoweringDivergenceContext {
        last_matching_semantic_identity: last_matching,
        first_body_event: body_event,
        first_ssa_event: ssa_event,
        related_hir_identities,
        related_ssa_identities,
        uncertainty: "bounded trace alignment is divergence context, not a causal-root proof"
            .to_owned(),
    }
}

fn execution_subject(
    program: &Program,
    target: Option<&crate::ExecutionTarget>,
) -> ExecutionSubject {
    let (module, function) = target
        .map(|target| (target.module.clone(), target.function.clone()))
        .unwrap_or_else(|| (program.module.clone(), String::new()));
    ExecutionSubject {
        module: module.clone(),
        function: function.clone(),
        program_identity: Some(program_id(&program.module)),
        program_fingerprint: program.content_fingerprint().ok(),
        function_identity: (!function.is_empty()).then(|| {
            program
                .functions
                .iter()
                .find(|candidate| {
                    candidate.name == function
                        && candidate.identity_namespace(&program.module) == module
                })
                .map(|candidate| {
                    function_id(
                        candidate.identity_namespace(&program.module),
                        &candidate.name,
                    )
                })
                .unwrap_or_else(|| function_id(&program.module, &function))
        }),
    }
}

fn invalid_comparison(program: &Program, corpus: &ExecutionCorpus) -> LoweringExecutionComparison {
    let subject = execution_subject(
        program,
        corpus.cases.first().map(|case_| &case_.request.target),
    );
    LoweringExecutionComparison {
        schema_version: LOWERING_EXECUTION_COMPARISON_SCHEMA_VERSION.to_owned(),
        status: LoweringExecutionStatus::InvalidInput,
        epistemic_note: "SSA lowering or validation failed before bounded comparison".to_owned(),
        corpus_name: corpus.name.clone(),
        corpus_size: corpus.cases.len(),
        matching_cases: 0,
        mismatching_cases: 0,
        mismatches_truncated: false,
        program_identity: subject.program_identity.clone(),
        program_fingerprint: subject.program_fingerprint.clone(),
        function_identity: subject.function_identity.clone(),
        ssa_module_identity: None,
        ssa_module_fingerprint: None,
        hir_fingerprint: None,
        body: subject.clone(),
        ssa: subject,
        mismatches: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BodyOperationKind, ExecutionTraceEntry};

    fn request_for(program: &Program, amount: i128) -> crate::ExecutionRequest {
        crate::ExecutionRequest {
            schema_version: crate::EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
            target: crate::ExecutionTarget {
                module: program.module.clone(),
                function: program.functions[0].name.clone(),
            },
            arguments: vec![ExecutionValue::Integer {
                value: amount,
                ty: IntegerType {
                    bits: 32,
                    signed: true,
                },
            }],
            step_budget: 64,
            type_arguments: Vec::new(),
            policy: crate::ExecutionPolicy::default(),
            host_grants: Vec::new(),
            call_depth_budget: None,
        }
    }

    #[test]
    fn ssa_session_is_bound_to_one_owned_program_and_module_pair() {
        let program_a = crate::body::tests::executable_program();
        let module_a = program_a.lower_to_ssa().expect("SSA A");
        let mut caller_owned_program = program_a.clone();
        let session = SsaExecutionSession::new(&caller_owned_program, &module_a).expect("session");
        // NOTE: no global cost-counter assertion here. `cost_report()` is a
        // process-wide static shared by every test thread, so asserting it
        // is unchanged across these executes only proves the scheduler was
        // quiet, not that the session skipped validation (CI failed once
        // with 59 vs 58 from one concurrent validation). The receipt and
        // determinism assertions below carry the binding property.

        // Mutating the caller's copy cannot launder a different program into
        // the receipt-bound session: the session owns its immutable pair.
        if let Some(function) = caller_owned_program.functions.first_mut() {
            if let Some(body) = function.body.as_mut() {
                if let Some(operation) = body.blocks[0].operations.first_mut() {
                    if let BodyOperationKind::Constant { value, .. } = &mut operation.kind {
                        *value = 99;
                    }
                }
            }
        }
        let first = session.execute(&request_for(&program_a, 4));
        let second = session.execute(&request_for(&program_a, 4));
        assert_eq!(first.status, ExecutionStatus::Returned);
        assert_eq!(
            first.returned,
            vec![ExecutionValue::Integer {
                value: 5,
                ty: IntegerType {
                    bits: 32,
                    signed: true,
                },
            }]
        );
        assert_eq!(first, second);

        let mut program_b = program_a.clone();
        let operation = program_b.functions[0].body.as_mut().expect("body").blocks[0]
            .operations
            .first_mut()
            .expect("constant");
        if let BodyOperationKind::Constant { value, .. } = &mut operation.kind {
            *value = 2;
        } else {
            panic!("fixture constant changed shape");
        }
        let module_b = program_b.lower_to_ssa().expect("SSA B");
        assert_ne!(
            program_a
                .content_fingerprint()
                .expect("program A fingerprint"),
            program_b
                .content_fingerprint()
                .expect("program B fingerprint")
        );
        assert_ne!(
            module_a.fingerprint().expect("SSA A fingerprint"),
            module_b.fingerprint().expect("SSA B fingerprint")
        );
        let direct_b = execute_ssa_module(&program_b, &module_b, &request_for(&program_b, 4));
        assert_eq!(direct_b.status, ExecutionStatus::Returned);
        assert_eq!(
            direct_b.returned[0],
            ExecutionValue::Integer {
                value: 6,
                ty: IntegerType {
                    bits: 32,
                    signed: true,
                },
            }
        );
        assert_ne!(
            first.ssa_module_fingerprint,
            direct_b.ssa_module_fingerprint
        );

        let receipt = session.validation_receipt();
        assert!(receipt.identity_is_valid());
        assert!(receipt
            .dependencies
            .contains_key(&program_id(&program_a.module)));
        let mut tampered_receipt = receipt.clone();
        tampered_receipt
            .scope
            .insert("tampered".to_owned(), "true".to_owned());
        assert!(!tampered_receipt.identity_is_valid());
    }

    #[test]
    fn ssa_session_rejects_schema_identity_and_reordered_block_mutations() {
        let program = crate::body::tests::executable_program();
        let module = program.lower_to_ssa().expect("SSA");

        let mut wrong_schema = module.clone();
        wrong_schema.schema_version = "0.3".to_owned();
        assert!(SsaExecutionSession::new(&program, &wrong_schema).is_err());

        let mut wrong_identity = module.clone();
        wrong_identity.identity = SemanticId("mncs:tampered:ssa".to_owned());
        assert!(SsaExecutionSession::new(&program, &wrong_identity).is_err());

        let mut reordered_blocks = module;
        reordered_blocks.functions[0].blocks.reverse();
        assert!(!reordered_blocks.validate().valid);
        assert!(SsaExecutionSession::new(&program, &reordered_blocks).is_err());
    }

    #[test]
    fn divergence_context_is_bounded_and_explicit() {
        let body = ExecutionResult {
            schema_version: crate::EXECUTION_RESULT_SCHEMA_VERSION.to_owned(),
            status: ExecutionStatus::Returned,
            target: crate::ExecutionTarget {
                module: "m".to_owned(),
                function: "f".to_owned(),
            },
            program_identity: None,
            program_fingerprint: None,
            function_identity: None,
            returned: Vec::new(),
            steps: 1,
            failure: None,
            trace: vec![ExecutionTraceEntry {
                step: 1,
                block: SemanticId("b".to_owned()),
                operation: Some(SemanticId("op".to_owned())),
                event: "operation".to_owned(),
            }],
            trace_truncated: false,
            effects: Vec::new(),
        };
        let ssa = SsaExecutionResult {
            schema_version: SSA_EXECUTION_RESULT_SCHEMA_VERSION.to_owned(),
            status: ExecutionStatus::RuntimeFailure,
            target: body.target.clone(),
            semantic_program_identity: None,
            semantic_function_identity: None,
            ssa_module_identity: None,
            ssa_module_fingerprint: None,
            hir_fingerprint: None,
            returned: Vec::new(),
            steps: 1,
            failure: None,
            trace: Vec::new(),
            trace_truncated: false,
            effects: Vec::new(),
        };
        let context = divergence_context(&body, &ssa);
        assert_eq!(context.last_matching_semantic_identity, None);
        assert!(context.uncertainty.contains("not a causal-root proof"));
    }
}
