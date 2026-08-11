//! Independent bounded execution of the validated SSA artifact.
//!
//! This is a reference evaluator for the experimental SSA subset, not a
//! backend.  It interprets SSA values, block parameters, instructions, and
//! terminators directly.  Agreement with body execution is bounded evidence
//! about the declared corpus and is not compiler correctness.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::execution::{
    compare_integers, evaluate_integer, integer_operator_supported, ExecutionEffectEvent,
};
use crate::identity::{function_id, program_id};
use crate::{
    execute_with_policy, BodyType, ExecutionCorpus, ExecutionFailure, ExecutionResult,
    ExecutionStatus, ExecutionSubject, ExecutionValue, IrType, Program, SemanticId, SsaBlock,
    SsaFunction, SsaInstruction, SsaInstructionKind, SsaModule, SsaTerminator,
    MAX_EXECUTION_BUDGET,
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
    if module.schema_version != crate::SSA_SCHEMA_VERSION {
        return SsaExecutionResult::invalid(request, "unsupported SSA schema version");
    }
    if module.semantic_identity != program_id(&program.module) {
        return SsaExecutionResult::invalid(
            request,
            "SSA semantic program identity does not match program",
        );
    }
    if request.target.module != program.module {
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
    let validation = program.validate();
    if !validation.valid {
        return SsaExecutionResult::invalid(request, "program validation failed");
    }
    let module_validation = module.validate();
    if !module_validation.valid {
        return SsaExecutionResult::invalid(request, "SSA validation failed");
    }
    let semantic_function = function_id(&program.module, &request.target.function);
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
    let mut result = SsaExecutionResult {
        schema_version: SSA_EXECUTION_RESULT_SCHEMA_VERSION.to_owned(),
        status: ExecutionStatus::InvalidRequest,
        target: request.target.clone(),
        semantic_program_identity: Some(program_id(&program.module)),
        semantic_function_identity: Some(semantic_function),
        ssa_module_identity: Some(module.identity.clone()),
        ssa_module_fingerprint: module.fingerprint().ok(),
        hir_fingerprint: Some(module.hir_fingerprint.clone()),
        returned: Vec::new(),
        steps: 0,
        failure: None,
        trace: Vec::new(),
        trace_truncated: false,
        effects: Vec::new(),
    };
    let Some(mut values) = initialize_inputs(function, request, &mut result) else {
        return result;
    };
    let blocks = function
        .blocks
        .iter()
        .map(|block| (block.identity.clone(), block))
        .collect::<BTreeMap<_, _>>();
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
        let Some(block) = blocks.get(&current).copied() else {
            result.fail(
                ExecutionStatus::InvalidRequest,
                None,
                "SSA reached an unknown block",
            );
            return result;
        };
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
            if execute_instruction(program, instruction, &mut values, &mut result, request) {
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
                let Some(returned) = returned
                    .iter()
                    .map(|value| values.get(value).cloned())
                    .collect::<Option<Vec<_>>>()
                else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        block.semantic_identity.clone(),
                        "return referenced an unavailable value",
                    );
                    return result;
                };
                result.returned = returned;
                result.status = ExecutionStatus::Returned;
                return result;
            }
            SsaTerminator::Branch { target, arguments } => {
                let incoming = values.clone();
                if !assign_block_arguments(&blocks, target, arguments, &incoming, &mut values) {
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
                let incoming = values.clone();
                if !assign_block_arguments(&blocks, target, arguments, &incoming, &mut values) {
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
    let function_identity = target
        .as_ref()
        .map(|target| function_id(&target.module, &target.function));
    let mut mismatches = Vec::new();
    let mut matching_cases = 0;
    let mut mismatching_cases = 0;
    let mut unsupported_case = false;
    let mut invalid_case =
        corpus.schema_version != crate::EXECUTION_CORPUS_SCHEMA_VERSION || corpus.cases.is_empty();
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

fn execute_instruction(
    program: &Program,
    instruction: &SsaInstruction,
    values: &mut BTreeMap<SemanticId, ExecutionValue>,
    result: &mut SsaExecutionResult,
    request: &crate::ExecutionRequest,
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
                        ty: *operand_type,
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
    function_name: &str,
    identity: &SemanticId,
) -> Option<crate::Effect> {
    let function = program
        .functions
        .iter()
        .find(|function| function.name == function_name)?;
    function
        .effects
        .iter()
        .enumerate()
        .find_map(|(occurrence, effect)| {
            let canonical =
                serde_json::to_string(&crate::canonical::canonical_effect(effect)).ok()?;
            let candidate =
                crate::identity::effect_id(&program.module, function_name, &canonical, occurrence);
            (candidate == *identity).then(|| effect.clone())
        })
}

fn initialize_inputs(
    function: &SsaFunction,
    request: &crate::ExecutionRequest,
    result: &mut SsaExecutionResult,
) -> Option<BTreeMap<SemanticId, ExecutionValue>> {
    if function.inputs.len() != request.arguments.len() {
        result.fail(
            ExecutionStatus::InvalidRequest,
            None,
            "argument count does not match SSA inputs",
        );
        return None;
    }
    let mut values = BTreeMap::new();
    for (input, argument) in function.inputs.iter().zip(&request.arguments) {
        let Some(ty) = body_type(&input.ty) else {
            result.fail(
                ExecutionStatus::Unsupported,
                Some(input.identity.clone()),
                "SSA input type is outside the scalar reference subset",
            );
            return None;
        };
        if !value_matches_type(argument, &ty) {
            result.fail(
                ExecutionStatus::InvalidRequest,
                Some(input.identity.clone()),
                "argument does not match SSA input type",
            );
            return None;
        }
        values.insert(input.identity.clone(), normalize_value(argument, &ty)?);
    }
    Some(values)
}

fn assign_block_arguments(
    blocks: &BTreeMap<SemanticId, &SsaBlock>,
    target: &SemanticId,
    arguments: &[SemanticId],
    values: &BTreeMap<SemanticId, ExecutionValue>,
    destination: &mut BTreeMap<SemanticId, ExecutionValue>,
) -> bool {
    let Some(block) = blocks.get(target).copied() else {
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
        destination.insert(parameter.identity.clone(), value);
    }
    true
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
        _ => None,
    }
}

fn body_type(ty: &IrType) -> Option<BodyType> {
    let IrType::Named(name) = ty;
    Some(BodyType::from_semantic_name(name))
}

fn value_matches_type(value: &ExecutionValue, ty: &BodyType) -> bool {
    match (value, ty) {
        (ExecutionValue::Integer { value, ty: actual }, BodyType::Integer(expected)) => {
            actual == expected && in_range(*value, *expected)
        }
        (ExecutionValue::Boolean { .. }, BodyType::Named(name)) if name == "bool" => true,
        (ExecutionValue::Boolean { .. }, BodyType::Integer(integer)) => {
            integer.bits == 1 && !integer.signed
        }
        _ => false,
    }
}

fn normalize_value(value: &ExecutionValue, ty: &BodyType) -> Option<ExecutionValue> {
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
        _ => None,
    }
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
            last_matching = left.operation.clone();
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
        module,
        function: function.clone(),
        program_identity: Some(program_id(&program.module)),
        program_fingerprint: program.content_fingerprint().ok(),
        function_identity: (!function.is_empty()).then(|| function_id(&program.module, &function)),
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
    use crate::ExecutionTraceEntry;

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
