//! Deterministic, bounded reference execution for the validated executable body subset.
//!
//! This module is deliberately not a runtime or backend. It executes only
//! semantics already represented by a validated `FunctionBody`, records a
//! bounded identity trace, and returns explicit results for unsupported or
//! resource-bounded cases.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::identity::{function_id, SemanticId};
use crate::{
    ArithmeticIntent, BodyBlock, BodyOperation, BodyOperationKind, BodyTerminator, BodyType,
    Function, FunctionBody, IntegerType, Program,
};

pub const EXECUTION_REQUEST_SCHEMA_VERSION: &str = "0.1";
pub const EXECUTION_RESULT_SCHEMA_VERSION: &str = "0.1";
pub const EXECUTION_CORPUS_SCHEMA_VERSION: &str = "0.1";
pub const EXECUTION_CORPUS_SCHEMA_VERSION_0_2: &str = "0.2";
pub const EXECUTION_COMPARISON_SCHEMA_VERSION: &str = "0.1";
pub const MAX_EXECUTION_BUDGET: u64 = 1_000_000;
const MAX_TRACE_ENTRIES: usize = 256;

pub fn execution_corpus_schema_supported(schema_version: &str) -> bool {
    matches!(
        schema_version,
        EXECUTION_CORPUS_SCHEMA_VERSION | EXECUTION_CORPUS_SCHEMA_VERSION_0_2
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionTarget {
    pub module: String,
    pub function: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionValue {
    Integer {
        value: i128,
        #[serde(rename = "type")]
        ty: IntegerType,
    },
    Boolean {
        value: bool,
    },
    Finite {
        type_identity: SemanticId,
        variant_identity: SemanticId,
        discriminant: u32,
    },
    /// A logical record value.  Fields are kept in canonical (sorted) name
    /// order; this is the *logical* value model — no physical layout, offset,
    /// or packing is part of its meaning.
    Record {
        type_identity: SemanticId,
        name: String,
        fields: Vec<(String, ExecutionValue)>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EffectExecutionPolicy {
    #[default]
    Unsupported,
    Record,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    #[serde(default)]
    pub effects: EffectExecutionPolicy,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            effects: EffectExecutionPolicy::Unsupported,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub schema_version: String,
    pub target: ExecutionTarget,
    pub arguments: Vec<ExecutionValue>,
    pub step_budget: u64,
    #[serde(default)]
    pub policy: ExecutionPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Returned,
    RuntimeFailure,
    Unsupported,
    BudgetExhausted,
    InvalidRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionFailure {
    pub identity: Option<SemanticId>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionTraceEntry {
    pub step: u64,
    pub block: SemanticId,
    pub operation: Option<SemanticId>,
    pub event: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionEffectEvent {
    pub operation: SemanticId,
    pub kind: String,
    pub target: String,
    pub capability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub schema_version: String,
    pub status: ExecutionStatus,
    pub target: ExecutionTarget,
    pub program_identity: Option<SemanticId>,
    pub program_fingerprint: Option<String>,
    pub function_identity: Option<SemanticId>,
    pub returned: Vec<ExecutionValue>,
    pub steps: u64,
    pub failure: Option<ExecutionFailure>,
    pub trace: Vec<ExecutionTraceEntry>,
    pub trace_truncated: bool,
    pub effects: Vec<ExecutionEffectEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionCase {
    pub id: String,
    pub request: ExecutionRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<Vec<ExecutionValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_status: Option<ExecutionStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_steps: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_effects: Vec<ExpectedEffectObservation>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub prohibit_unexpected_effects: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExpectedEffectObservation {
    pub kind: String,
    pub target: String,
    pub capability: String,
}

/// A bounded, exhaustive property check over the supplied finite value set.
/// The language runner interprets the named law; Forge only stores observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionProperty {
    pub id: String,
    pub law: String,
    pub values: Vec<ExecutionValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distinguished: Option<ExecutionValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclusions: Vec<ExecutionValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionCorpus {
    pub schema_version: String,
    pub name: String,
    pub cases: Vec<ExecutionCase>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<ExecutionProperty>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSubject {
    pub module: String,
    pub function: String,
    pub program_identity: Option<SemanticId>,
    pub program_fingerprint: Option<String>,
    pub function_identity: Option<SemanticId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonStatus {
    EquivalentOverCorpus,
    MismatchDetected,
    InvalidInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionMismatch {
    pub case_id: String,
    pub baseline: ExecutionResult,
    pub candidate: ExecutionResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionComparison {
    pub schema_version: String,
    pub status: ComparisonStatus,
    pub epistemic_note: String,
    pub corpus_name: String,
    pub corpus_size: usize,
    pub matching_cases: usize,
    pub mismatching_cases: usize,
    pub mismatches_truncated: bool,
    pub baseline: ExecutionSubject,
    pub candidate: ExecutionSubject,
    pub mismatches: Vec<ExecutionMismatch>,
}

impl ExecutionResult {
    fn empty(request: &ExecutionRequest) -> Self {
        Self {
            schema_version: EXECUTION_RESULT_SCHEMA_VERSION.to_owned(),
            status: ExecutionStatus::InvalidRequest,
            target: request.target.clone(),
            program_identity: None,
            program_fingerprint: None,
            function_identity: None,
            returned: Vec::new(),
            steps: 0,
            failure: None,
            trace: Vec::new(),
            trace_truncated: false,
            effects: Vec::new(),
        }
    }

    fn invalid(request: &ExecutionRequest, reason: impl Into<String>) -> Self {
        let mut result = Self::empty(request);
        result.failure = Some(ExecutionFailure {
            identity: None,
            reason: reason.into(),
        });
        result
    }

    fn with_program(request: &ExecutionRequest, program: &Program, function: &Function) -> Self {
        let program_identity = program
            .semantic_identities()
            .objects
            .into_iter()
            .find(|record| record.kind == crate::IdentityKind::Program)
            .map(|record| record.identity);
        Self {
            schema_version: EXECUTION_RESULT_SCHEMA_VERSION.to_owned(),
            status: ExecutionStatus::InvalidRequest,
            target: request.target.clone(),
            program_identity,
            program_fingerprint: program.content_fingerprint().ok(),
            function_identity: Some(function_id(&program.module, &function.name)),
            returned: Vec::new(),
            steps: 0,
            failure: None,
            trace: Vec::new(),
            trace_truncated: false,
            effects: Vec::new(),
        }
    }

    fn fail(&mut self, status: ExecutionStatus, identity: Option<SemanticId>, reason: String) {
        self.status = status;
        self.failure = Some(ExecutionFailure { identity, reason });
    }
}

pub fn execute(program: &Program, request: &ExecutionRequest) -> ExecutionResult {
    execute_inner(
        program,
        request,
        matches!(request.policy.effects, EffectExecutionPolicy::Record),
    )
}

fn execute_inner(
    program: &Program,
    request: &ExecutionRequest,
    record_effects: bool,
) -> ExecutionResult {
    if request.schema_version != EXECUTION_REQUEST_SCHEMA_VERSION {
        return ExecutionResult::invalid(
            request,
            format!(
                "unsupported execution request schema {:?}; expected {:?}",
                request.schema_version, EXECUTION_REQUEST_SCHEMA_VERSION
            ),
        );
    }
    let validation = program.validate();
    if !validation.valid {
        return ExecutionResult::invalid(request, "program validation failed");
    }
    if request.target.module != program.module {
        return ExecutionResult::invalid(request, "execution target module does not match program");
    }
    if request.step_budget == 0 || request.step_budget > MAX_EXECUTION_BUDGET {
        return ExecutionResult::invalid(
            request,
            format!("step_budget must be between 1 and {MAX_EXECUTION_BUDGET}"),
        );
    }
    let Some(function) = program
        .functions
        .iter()
        .find(|function| function.name == request.target.function)
    else {
        return ExecutionResult::invalid(request, "execution target function does not exist");
    };
    let Some(body) = &function.body else {
        return ExecutionResult::invalid(
            request,
            "execution target function has no executable body",
        );
    };
    if let Err(reason) = validate_arguments(program, body, request) {
        return ExecutionResult::invalid(request, reason);
    }

    let mut result = ExecutionResult::with_program(request, program, function);
    let mut values = BTreeMap::new();
    for (parameter, argument) in body.parameters.iter().zip(&request.arguments) {
        let Some(argument) = normalize_value(program, argument, &parameter.ty) else {
            return ExecutionResult::invalid(request, "argument could not be normalized");
        };
        values.insert(parameter.id.clone(), argument);
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.id.as_str(), block))
        .collect::<BTreeMap<_, _>>();
    let mut current = body.entry.as_str();

    loop {
        let Some(block) = blocks.get(current).copied() else {
            result.fail(
                ExecutionStatus::InvalidRequest,
                None,
                format!("runtime reached unknown block {current:?}"),
            );
            return result;
        };
        record_trace(
            &mut result,
            program,
            body,
            function,
            block,
            None,
            "block_enter",
        );
        for operation in &block.operations {
            if !consume_step(&mut result, request.step_budget) {
                result.fail(
                    ExecutionStatus::BudgetExhausted,
                    Some(operation.identity(&program.module, &function.name, &block.id)),
                    "execution step budget exhausted".to_owned(),
                );
                return result;
            }
            let identity = operation.identity(&program.module, &function.name, &block.id);
            record_trace(
                &mut result,
                program,
                body,
                function,
                block,
                Some(operation),
                "operation",
            );
            if let Some(stop) = execute_operation(
                program,
                operation,
                &identity,
                &mut values,
                &mut result,
                request,
                record_effects,
            ) {
                return stop;
            }
        }
        if !consume_step(&mut result, request.step_budget) {
            result.fail(
                ExecutionStatus::BudgetExhausted,
                None,
                "execution step budget exhausted".to_owned(),
            );
            return result;
        }
        record_trace(
            &mut result,
            program,
            body,
            function,
            block,
            None,
            "terminator",
        );
        match &block.terminator {
            BodyTerminator::Failure { mode } => {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    None,
                    format!("failure terminator reached: {mode:?}"),
                );
                return result;
            }
            BodyTerminator::Return { values: returned } => {
                result.returned = match returned
                    .iter()
                    .map(|value| values.get(value).cloned())
                    .collect::<Option<Vec<_>>>()
                {
                    Some(values) => values,
                    None => {
                        result.fail(
                            ExecutionStatus::InvalidRequest,
                            None,
                            "return referenced an unavailable value".to_owned(),
                        );
                        return result;
                    }
                };
                result.status = ExecutionStatus::Returned;
                return result;
            }
            BodyTerminator::Branch { target, arguments } => {
                let incoming = values.clone();
                if !assign_block_arguments(&blocks, target, arguments, &incoming, &mut values) {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        None,
                        "branch arguments did not match target parameters".to_owned(),
                    );
                    return result;
                }
                current = target;
            }
            BodyTerminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } => {
                let Some(ExecutionValue::Boolean { value }) = values.get(condition) else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        None,
                        "conditional branch did not receive a boolean value".to_owned(),
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
                        None,
                        "conditional branch arguments did not match target parameters".to_owned(),
                    );
                    return result;
                }
                current = target;
            }
        }
    }
}

fn execute_operation(
    program: &Program,
    operation: &BodyOperation,
    identity: &SemanticId,
    values: &mut BTreeMap<String, ExecutionValue>,
    result: &mut ExecutionResult,
    request: &ExecutionRequest,
    record_effects: bool,
) -> Option<ExecutionResult> {
    match &operation.kind {
        BodyOperationKind::Constant { value, ty } => {
            let Some(value) = constant_value(*value, ty) else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    Some(identity.clone()),
                    "constant type or value is outside the reference subset".to_owned(),
                );
                return Some(result.clone());
            };
            values.insert(operation.results[0].id.clone(), value);
        }
        BodyOperationKind::RecordConstruct {
            type_identity,
            field_names,
        } => {
            let mut fields = Vec::new();
            for (name, operand) in field_names.iter().zip(&operation.operands) {
                match values.get(operand) {
                    Some(value) => fields.push((name.clone(), value.clone())),
                    None => {
                        result.fail(
                            ExecutionStatus::InvalidRequest,
                            Some(identity.clone()),
                            "record construction operand was unavailable".to_owned(),
                        );
                        return Some(result.clone());
                    }
                }
            }
            let output = operation.results.first()?;
            values.insert(
                output.id.clone(),
                ExecutionValue::Record {
                    type_identity: type_identity.clone(),
                    name: output.ty.semantic_name(),
                    fields,
                },
            );
        }
        BodyOperationKind::RecordProject { field, .. } => {
            let operand = operation.operands.first()?;
            match values.get(operand) {
                Some(ExecutionValue::Record { fields, .. }) => {
                    match fields.iter().find(|(name, _)| name == field) {
                        Some((_, value)) => {
                            if let Some(output) = operation.results.first() {
                                values.insert(output.id.clone(), value.clone());
                            }
                        }
                        None => {
                            result.fail(
                                ExecutionStatus::InvalidRequest,
                                Some(identity.clone()),
                                format!("record has no field {field:?}"),
                            );
                            return Some(result.clone());
                        }
                    }
                }
                _ => {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        Some(identity.clone()),
                        "record projection operand is not a record value".to_owned(),
                    );
                    return Some(result.clone());
                }
            }
        }
        BodyOperationKind::Integer {
            operator,
            operand_type,
            intent,
        } => {
            let Some((left, right)) = integer_operands(operation, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "integer operation operands were unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            if !integer_operator_supported(operator, *intent) {
                result.fail(
                    ExecutionStatus::Unsupported,
                    Some(identity.clone()),
                    format!("unsupported integer operator or intent {operator:?}/{intent:?}"),
                );
                return Some(result.clone());
            }
            let Some(value) = evaluate_integer(operator, *operand_type, *intent, left, right)
            else {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    Some(identity.clone()),
                    format!("integer {operator} overflow or did not produce a value"),
                );
                return Some(result.clone());
            };
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Integer {
                    value,
                    ty: *operand_type,
                },
            );
        }
        BodyOperationKind::IntegerCompare {
            predicate,
            operand_type,
        } => {
            let Some((left, right)) = integer_operands(operation, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "integer comparison operands were unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(value) = compare_integers(predicate, *operand_type, left, right) else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    Some(identity.clone()),
                    format!("unsupported integer comparison predicate {predicate:?}"),
                );
                return Some(result.clone());
            };
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Boolean { value },
            );
        }
        BodyOperationKind::FiniteConstruct {
            type_identity,
            variant_identity,
            discriminant,
        } => {
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Finite {
                    type_identity: type_identity.clone(),
                    variant_identity: variant_identity.clone(),
                    discriminant: *discriminant,
                },
            );
        }
        BodyOperationKind::FiniteIsVariant {
            type_identity,
            variant_identity,
            discriminant,
        } => {
            let Some(ExecutionValue::Finite {
                type_identity: actual_type,
                variant_identity: actual_variant,
                discriminant: actual_discriminant,
            }) = operation
                .operands
                .first()
                .and_then(|operand| values.get(operand))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "finite variant test operand was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            if actual_type != type_identity
                || !valid_finite_value(program, actual_type, actual_variant, *actual_discriminant)
            {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "finite value has an invalid type/variant/discriminant".to_owned(),
                );
                return Some(result.clone());
            }
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Boolean {
                    value: actual_variant == variant_identity
                        && actual_discriminant == discriminant,
                },
            );
        }
        BodyOperationKind::Call { function_name, .. } => {
            let Some(arguments) = operation
                .operands
                .iter()
                .map(|operand| values.get(operand).cloned())
                .collect::<Option<Vec<_>>>()
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "call operands were unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let remaining = request.step_budget.saturating_sub(result.steps);
            if remaining == 0 {
                result.fail(
                    ExecutionStatus::BudgetExhausted,
                    Some(identity.clone()),
                    "execution step budget exhausted before call".to_owned(),
                );
                return Some(result.clone());
            }
            let nested_request = ExecutionRequest {
                schema_version: request.schema_version.clone(),
                target: ExecutionTarget {
                    module: request.target.module.clone(),
                    function: function_name.clone(),
                },
                arguments,
                step_budget: remaining,
                policy: request.policy.clone(),
            };
            let nested = execute_inner(program, &nested_request, record_effects);
            let trace_offset = result.steps;
            result.steps = result.steps.saturating_add(nested.steps);
            result.effects.extend(nested.effects.clone());
            for mut entry in nested.trace.clone() {
                entry.step = entry.step.saturating_add(trace_offset);
                if result.trace.len() < MAX_TRACE_ENTRIES {
                    result.trace.push(entry);
                } else {
                    result.trace_truncated = true;
                }
            }
            if nested.status != ExecutionStatus::Returned {
                result.status = nested.status;
                result.failure = nested.failure;
                return Some(result.clone());
            }
            let Some(returned) = nested.returned.first().cloned() else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "callee returned no value".to_owned(),
                );
                return Some(result.clone());
            };
            values.insert(operation.results[0].id.clone(), returned);
        }
        BodyOperationKind::Effect { effect, capability } => {
            if !record_effects {
                result.fail(
                    ExecutionStatus::Unsupported,
                    Some(identity.clone()),
                    "effect execution requires explicit record policy; no external side effect was performed".to_owned(),
                );
                return Some(result.clone());
            }
            result.effects.push(ExecutionEffectEvent {
                operation: identity.clone(),
                kind: effect.kind.clone(),
                target: effect.target.clone(),
                capability: capability.clone(),
            });
        }
        BodyOperationKind::RuntimeCheck { .. } => {
            result.fail(
                ExecutionStatus::Unsupported,
                Some(identity.clone()),
                "runtime checks have no executable condition in the current body representation"
                    .to_owned(),
            );
            return Some(result.clone());
        }
    }
    None
}

pub fn execute_with_policy(program: &Program, request: &ExecutionRequest) -> ExecutionResult {
    execute(program, request)
}

fn validate_arguments(
    program: &Program,
    body: &FunctionBody,
    request: &ExecutionRequest,
) -> Result<(), String> {
    if body.parameters.len() != request.arguments.len() {
        return Err("argument count does not match executable body parameters".to_owned());
    }
    for (parameter, argument) in body.parameters.iter().zip(&request.arguments) {
        if !value_matches_type(program, argument, &parameter.ty) {
            return Err(format!(
                "argument does not match parameter {:?}",
                parameter.name
            ));
        }
    }
    Ok(())
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
                discriminant,
            },
            BodyType::Finite { identity, .. },
        ) => {
            type_identity == identity
                && valid_finite_value(program, type_identity, variant_identity, *discriminant)
        }
        _ => false,
    }
}

fn constant_value(value: i128, ty: &BodyType) -> Option<ExecutionValue> {
    match ty {
        BodyType::Integer(integer)
            if integer.bits == 1 && !integer.signed && matches!(value, 0 | 1) =>
        {
            Some(ExecutionValue::Boolean { value: value == 1 })
        }
        BodyType::Integer(integer) if in_range(value, *integer) => Some(ExecutionValue::Integer {
            value,
            ty: *integer,
        }),
        BodyType::Named(name) if name == "bool" && matches!(value, 0 | 1) => {
            Some(ExecutionValue::Boolean { value: value == 1 })
        }
        _ => None,
    }
}

fn normalize_value(
    program: &Program,
    value: &ExecutionValue,
    ty: &BodyType,
) -> Option<ExecutionValue> {
    match (value, ty) {
        (ExecutionValue::Integer { value, ty: actual }, BodyType::Integer(expected))
            if actual == expected && in_range(*value, *expected) =>
        {
            if expected.bits == 1 && !expected.signed {
                Some(ExecutionValue::Boolean { value: *value == 1 })
            } else {
                Some(ExecutionValue::Integer {
                    value: *value,
                    ty: *actual,
                })
            }
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
            },
            BodyType::Finite { identity, .. },
        ) if type_identity == identity
            && valid_finite_value(program, type_identity, variant_identity, *discriminant) =>
        {
            Some(value.clone())
        }
        _ => None,
    }
}

pub(crate) fn valid_finite_value(
    program: &Program,
    type_identity: &SemanticId,
    variant_identity: &SemanticId,
    discriminant: u32,
) -> bool {
    program
        .finite_types
        .iter()
        .find(|finite_type| &finite_type.identity == type_identity)
        .is_some_and(|finite_type| {
            finite_type.variants.iter().any(|variant| {
                &variant.identity == variant_identity && variant.discriminant == discriminant
            })
        })
}

fn integer_operands(
    operation: &BodyOperation,
    values: &BTreeMap<String, ExecutionValue>,
) -> Option<(i128, i128)> {
    let [left, right] = operation.operands.as_slice() else {
        return None;
    };
    let Some(ExecutionValue::Integer { value: left, .. }) = values.get(left) else {
        return None;
    };
    let Some(ExecutionValue::Integer { value: right, .. }) = values.get(right) else {
        return None;
    };
    Some((*left, *right))
}

pub(crate) fn compare_integers(
    predicate: &str,
    ty: IntegerType,
    left: i128,
    right: i128,
) -> Option<bool> {
    if !in_range(left, ty) || !in_range(right, ty) {
        return None;
    }
    Some(match predicate {
        "eq" => left == right,
        "ne" => left != right,
        "lt" => left < right,
        "le" => left <= right,
        "gt" => left > right,
        "ge" => left >= right,
        _ => return None,
    })
}

pub(crate) fn integer_operator_supported(operator: &str, intent: ArithmeticIntent) -> bool {
    matches!(operator, "add" | "sub" | "mul")
        || (matches!(operator, "and" | "or" | "xor")
            && matches!(intent, ArithmeticIntent::Wrapping))
}

pub(crate) fn evaluate_integer(
    operator: &str,
    ty: IntegerType,
    intent: ArithmeticIntent,
    left: i128,
    right: i128,
) -> Option<i128> {
    if !in_range(left, ty) || !in_range(right, ty) || !(1..=126).contains(&ty.bits) {
        return None;
    }
    if !integer_operator_supported(operator, intent) {
        return None;
    }
    if matches!(operator, "and" | "or" | "xor") {
        return evaluate_bitwise(operator, ty, left, right);
    }
    let raw = match operator {
        "add" => left.checked_add(right)?,
        "sub" => left.checked_sub(right)?,
        "mul" => left.checked_mul(right)?,
        _ => return None,
    };
    let result_bits = match intent {
        ArithmeticIntent::Widening { bits } => bits,
        _ => ty.bits,
    };
    if matches!(intent, ArithmeticIntent::Widening { bits } if bits != ty.bits) {
        return None;
    }
    if !(1..=126).contains(&result_bits) {
        return None;
    }
    let (minimum, maximum) = integer_limits(result_bits, ty.signed)?;
    let overflow = raw < minimum || raw > maximum;
    match intent {
        ArithmeticIntent::Wrapping => Some(wrap_value(raw, ty.bits, ty.signed)?),
        ArithmeticIntent::Checked
        | ArithmeticIntent::Trapping
        | ArithmeticIntent::Widening { .. }
            if overflow =>
        {
            None
        }
        ArithmeticIntent::Checked
        | ArithmeticIntent::Trapping
        | ArithmeticIntent::Widening { .. } => Some(raw),
        ArithmeticIntent::Saturating => Some(raw.clamp(minimum, maximum)),
    }
}

fn evaluate_bitwise(operator: &str, ty: IntegerType, left: i128, right: i128) -> Option<i128> {
    let (_, maximum) = integer_limits(ty.bits, false)?;
    let left = left.rem_euclid(maximum.checked_add(1)?);
    let right = right.rem_euclid(maximum.checked_add(1)?);
    let value = match operator {
        "and" => left & right,
        "or" => left | right,
        "xor" => left ^ right,
        _ => return None,
    };
    if ty.signed {
        let sign = 1_i128.checked_shl(u32::from(ty.bits - 1))?;
        let modulus = maximum.checked_add(1)?;
        Some(if value >= sign {
            value - modulus
        } else {
            value
        })
    } else {
        Some(value)
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
        let maximum = 1_i128.checked_shl(u32::from(bits))?.checked_sub(1)?;
        Some((0, maximum))
    }
}

fn in_range(value: i128, ty: IntegerType) -> bool {
    integer_limits(ty.bits, ty.signed)
        .is_some_and(|(minimum, maximum)| (minimum..=maximum).contains(&value))
}

fn wrap_value(value: i128, bits: u16, signed: bool) -> Option<i128> {
    let (_, maximum) = integer_limits(bits, false)?;
    let modulus = maximum.checked_add(1)?;
    let wrapped = value.rem_euclid(modulus);
    if signed {
        let sign = 1_i128.checked_shl(u32::from(bits - 1))?;
        Some(if wrapped >= sign {
            wrapped - modulus
        } else {
            wrapped
        })
    } else {
        Some(wrapped)
    }
}

fn assign_block_arguments(
    blocks: &BTreeMap<&str, &BodyBlock>,
    target: &str,
    arguments: &[String],
    values: &BTreeMap<String, ExecutionValue>,
    destination: &mut BTreeMap<String, ExecutionValue>,
) -> bool {
    let Some(block) = blocks.get(target).copied() else {
        return false;
    };
    if block.parameters.len() != arguments.len() {
        return false;
    }
    let resolved = arguments
        .iter()
        .map(|argument| values.get(argument).cloned())
        .collect::<Option<Vec<_>>>();
    let Some(resolved) = resolved else {
        return false;
    };
    for (parameter, value) in block.parameters.iter().zip(resolved) {
        destination.insert(parameter.id.clone(), value);
    }
    true
}

fn consume_step(result: &mut ExecutionResult, budget: u64) -> bool {
    if result.steps >= budget {
        false
    } else {
        result.steps += 1;
        true
    }
}

fn record_trace(
    result: &mut ExecutionResult,
    program: &Program,
    body: &FunctionBody,
    function: &Function,
    block: &BodyBlock,
    operation: Option<&BodyOperation>,
    event: &str,
) {
    if result.trace.len() >= MAX_TRACE_ENTRIES {
        result.trace_truncated = true;
        return;
    }
    result.trace.push(ExecutionTraceEntry {
        step: result.steps,
        block: body.block_identity(&program.module, &function.name, &block.id),
        operation: operation
            .map(|operation| operation.identity(&program.module, &function.name, &block.id)),
        event: event.to_owned(),
    });
}

fn observable_result(result: &ExecutionResult) -> String {
    let effects = result
        .effects
        .iter()
        .map(|effect| (&effect.kind, &effect.target, &effect.capability))
        .collect::<Vec<_>>();
    serde_json::to_string(&(
        result.status,
        &result.returned,
        result.failure.as_ref().map(|failure| &failure.reason),
        effects,
    ))
    .expect("execution result is serializable")
}

pub fn compare(
    baseline: &Program,
    candidate: &Program,
    corpus: &ExecutionCorpus,
) -> ExecutionComparison {
    let baseline_subject = subject(
        baseline,
        &corpus
            .cases
            .first()
            .map(|case_| case_.request.target.clone()),
    );
    let candidate_subject = subject(
        candidate,
        &corpus
            .cases
            .first()
            .map(|case_| case_.request.target.clone()),
    );
    let mut mismatches = Vec::new();
    let mut matching_cases = 0;
    let mut mismatching_cases = 0;
    let mut invalid_case = false;
    for case_ in &corpus.cases {
        let left = execute_with_policy(baseline, &case_.request);
        let right = execute_with_policy(candidate, &case_.request);
        invalid_case |= left.status == ExecutionStatus::InvalidRequest
            || right.status == ExecutionStatus::InvalidRequest;
        if observable_result(&left) == observable_result(&right) {
            matching_cases += 1;
        } else {
            mismatching_cases += 1;
            if mismatches.is_empty() {
                mismatches.push(ExecutionMismatch {
                    case_id: case_.id.clone(),
                    baseline: left,
                    candidate: right,
                });
            }
        }
    }
    let invalid = invalid_case
        || !execution_corpus_schema_supported(&corpus.schema_version)
        || corpus.cases.is_empty()
        || corpus.cases.iter().any(|case_| case_.id.trim().is_empty());
    let status = if invalid {
        ComparisonStatus::InvalidInput
    } else if mismatching_cases == 0 {
        ComparisonStatus::EquivalentOverCorpus
    } else {
        ComparisonStatus::MismatchDetected
    };
    ExecutionComparison {
        schema_version: EXECUTION_COMPARISON_SCHEMA_VERSION.to_owned(),
        status,
        epistemic_note: "agreement is bounded corpus behavioral evidence, not universal semantic equivalence or compiler correctness".to_owned(),
        corpus_name: corpus.name.clone(),
        corpus_size: corpus.cases.len(),
        matching_cases,
        mismatching_cases,
        mismatches_truncated: mismatching_cases > mismatches.len(),
        baseline: baseline_subject,
        candidate: candidate_subject,
        mismatches,
    }
}

fn subject(program: &Program, target: &Option<ExecutionTarget>) -> ExecutionSubject {
    let (module, function) = target
        .as_ref()
        .map(|target| (target.module.clone(), target.function.clone()))
        .unwrap_or_else(|| (program.module.clone(), String::new()));
    let function_identity = (!function.is_empty()).then(|| function_id(&program.module, &function));
    let program_identity = program
        .semantic_identities()
        .objects
        .into_iter()
        .find(|record| record.kind == crate::IdentityKind::Program)
        .map(|record| record.identity);
    ExecutionSubject {
        module,
        function,
        program_identity,
        program_fingerprint: program.content_fingerprint().ok(),
        function_identity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_predicates_use_signed_and_unsigned_domains() {
        let signed = IntegerType {
            bits: 8,
            signed: true,
        };
        let unsigned = IntegerType {
            bits: 8,
            signed: false,
        };
        assert!(compare_integers("lt", signed, -1, 1).unwrap());
        assert!(!compare_integers("lt", unsigned, 255, 1).unwrap());
    }

    #[test]
    fn arithmetic_edges_are_explicit_and_host_safe() {
        let ty = IntegerType {
            bits: 8,
            signed: true,
        };
        assert_eq!(
            evaluate_integer("add", ty, ArithmeticIntent::Wrapping, 127, 1),
            Some(-128)
        );
        assert_eq!(
            evaluate_integer("add", ty, ArithmeticIntent::Checked, 127, 1),
            None
        );
        assert_eq!(
            evaluate_integer("add", ty, ArithmeticIntent::Saturating, 127, 1),
            Some(127)
        );
        assert_eq!(
            evaluate_integer("sub", ty, ArithmeticIntent::Wrapping, -128, 1),
            Some(127)
        );
        assert_eq!(
            evaluate_integer("mul", ty, ArithmeticIntent::Saturating, 64, 2),
            Some(127)
        );
        assert_eq!(
            evaluate_integer("xor", ty, ArithmeticIntent::Wrapping, -1, 1),
            Some(-2)
        );
        let unsigned = IntegerType {
            bits: 8,
            signed: false,
        };
        assert_eq!(
            evaluate_integer("mul", unsigned, ArithmeticIntent::Wrapping, 255, 2),
            Some(254)
        );
        assert_eq!(
            evaluate_integer("mul", unsigned, ArithmeticIntent::Checked, 255, 2),
            None
        );
        assert_eq!(
            evaluate_integer("mul", unsigned, ArithmeticIntent::Saturating, 255, 2),
            Some(255)
        );
    }

    #[test]
    fn trace_is_bounded() {
        let mut result = ExecutionResult::empty(&ExecutionRequest {
            schema_version: EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
            target: ExecutionTarget {
                module: "m".to_owned(),
                function: "f".to_owned(),
            },
            arguments: Vec::new(),
            step_budget: 1,
            policy: ExecutionPolicy::default(),
        });
        result.trace = (0..MAX_TRACE_ENTRIES)
            .map(|step| ExecutionTraceEntry {
                step: step as u64,
                block: SemanticId("b".to_owned()),
                operation: None,
                event: "x".to_owned(),
            })
            .collect();
        record_trace(
            &mut result,
            &Program {
                schema_version: "0.1".to_owned(),
                module: "m".to_owned(),
                finite_types: Vec::new(),
                record_types: Vec::new(),
                assumptions: Vec::new(),
                functions: Vec::new(),
            },
            &FunctionBody {
                schema_version: "0.2".to_owned(),
                entry: "b".to_owned(),
                parameters: Vec::new(),
                cycle_policy: crate::BodyCyclePolicy::Legacy,
                bounded_iterations: Vec::new(),
                blocks: Vec::new(),
            },
            &Function {
                name: "f".to_owned(),
                inputs: Vec::new(),
                outputs: Vec::new(),
                contracts: Vec::new(),
                effects: Vec::new(),
                capabilities: Vec::new(),
                assumptions: Vec::new(),
                evidence: Vec::new(),
                failure: crate::FailureMode::Isolated,
                body: None,
            },
            &BodyBlock {
                id: "b".to_owned(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: BodyTerminator::Return { values: Vec::new() },
            },
            None,
            "block_enter",
        );
        assert!(result.trace_truncated);
    }
}
