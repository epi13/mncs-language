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
    BoundsEvidence, Function, FunctionBody, IntegerType, Program, SequenceBound,
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
        /// Payload fields in canonical (sorted) name order (Profile 0.6).
        /// Absent/empty for payload-free variants; the serialized form of
        /// payload-free values is unchanged.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        payload: Vec<(String, ExecutionValue)>,
    },
    /// A logical record value.  Fields are kept in canonical (sorted) name
    /// order; this is the *logical* value model — no physical layout, offset,
    /// or packing is part of its meaning.
    Record {
        type_identity: SemanticId,
        name: String,
        fields: Vec<(String, ExecutionValue)>,
    },
    /// A byte-oriented logical value (Profile 0.7). The domain is 0..=255;
    /// no host pointer or storage shape participates in meaning.
    Byte {
        value: i128,
    },
    /// A bounded sequence's logical value: exactly its ordered element
    /// values. Length facts come from the type, not from the value.
    Sequence {
        values: Vec<ExecutionValue>,
    },
    /// Logical vector lanes. No storage/register representation participates
    /// in this reference value.
    Vector {
        values: Vec<ExecutionValue>,
    },
    /// Logical lane predicates. Backends may realize these as packed bits.
    Mask {
        lanes: Vec<bool>,
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
            function_identity: Some(function_id(
                function.identity_namespace(&program.module),
                &function.name,
            )),
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
    // The target may name the program's own module or the home module of
    // any linked declaration: multi-module programs keep per-module
    // identities, and every function is executable at its own address.
    if request.target.module != program.module
        && !program
            .functions
            .iter()
            .any(|function| function.home_module.as_deref() == Some(request.target.module.as_str()))
    {
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
    let namespace = function.identity_namespace(&program.module);
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
                    Some(operation.identity(namespace, &function.name, &block.id)),
                    "execution step budget exhausted".to_owned(),
                );
                return result;
            }
            let identity = operation.identity(namespace, &function.name, &block.id);
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
                    ty: match &operation.results[0].ty {
                        BodyType::Integer(ty) => *ty,
                        _ => *operand_type,
                    },
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
        BodyOperationKind::BooleanOp { operator } => {
            let Some((left, right)) = boolean_operands(operation, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "boolean operation operands were unavailable or not boolean".to_owned(),
                );
                return Some(result.clone());
            };
            let value = match operator.as_str() {
                "and" => left && right,
                "or" => left || right,
                _ => {
                    result.fail(
                        ExecutionStatus::Unsupported,
                        Some(identity.clone()),
                        format!("unsupported boolean operator {operator:?}"),
                    );
                    return Some(result.clone());
                }
            };
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Boolean { value },
            );
        }
        BodyOperationKind::ByteBitwise { operator } => {
            let Some((left, right)) = byte_operands(operation, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "byte bitwise operands were unavailable or not byte values".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(value) = evaluate_byte_bitwise(operator, left, right) else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    Some(identity.clone()),
                    format!("unsupported byte bitwise operator {operator:?}"),
                );
                return Some(result.clone());
            };
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Byte { value },
            );
        }
        BodyOperationKind::ByteShift { operator } => {
            let Some((byte, count)) = byte_shift_operands(operation, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "byte shift operands were unavailable or mistyped".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(value) = evaluate_byte_shift(operator, byte, count) else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    Some(identity.clone()),
                    format!("unsupported byte shift operator {operator:?}"),
                );
                return Some(result.clone());
            };
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Byte { value },
            );
        }
        BodyOperationKind::ByteCompare { predicate } => {
            let Some((left, right)) = byte_operands(operation, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "byte comparison operands were unavailable or not byte values".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(value) = compare_bytes(predicate, left, right) else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    Some(identity.clone()),
                    format!("unsupported byte comparison predicate {predicate:?}"),
                );
                return Some(result.clone());
            };
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Boolean { value },
            );
        }
        BodyOperationKind::Convert { from, to } => {
            let Some(operand_value) = operation
                .operands
                .first()
                .and_then(|operand| values.get(operand))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "conversion operand was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let converted = match (operand_value, from, to) {
                (
                    ExecutionValue::Integer { value, .. },
                    BodyType::Integer(_),
                    BodyType::Integer(target),
                ) => wrap_to_bits(*value, target.bits, target.signed)
                    .map(|value| ExecutionValue::Integer { value, ty: *target }),
                (ExecutionValue::Integer { value, .. }, BodyType::Integer(_), BodyType::Byte) => {
                    wrap_to_bits(*value, 8, false).map(|value| ExecutionValue::Byte { value })
                }
                (ExecutionValue::Byte { value }, BodyType::Byte, BodyType::Integer(target)) => {
                    wrap_to_bits(*value, target.bits, target.signed)
                        .map(|value| ExecutionValue::Integer { value, ty: *target })
                }
                (ExecutionValue::Byte { value }, BodyType::Byte, BodyType::Byte) => {
                    Some(ExecutionValue::Byte { value: *value })
                }
                // Booleans convert outward as 0/1; never inward.
                (
                    ExecutionValue::Boolean { value },
                    BodyType::Named(name),
                    BodyType::Integer(target),
                ) if name == "bool" => wrap_to_bits(i128::from(*value), target.bits, target.signed)
                    .map(|value| ExecutionValue::Integer { value, ty: *target }),
                (ExecutionValue::Boolean { value }, BodyType::Named(name), BodyType::Byte)
                    if name == "bool" =>
                {
                    Some(ExecutionValue::Byte {
                        value: i128::from(*value),
                    })
                }
                _ => None,
            };
            let Some(converted) = converted else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    Some(identity.clone()),
                    "conversion operands do not match the declared conversion".to_owned(),
                );
                return Some(result.clone());
            };
            values.insert(operation.results[0].id.clone(), converted);
        }
        BodyOperationKind::SequenceConstruct { length, .. } => {
            let mut element_values = Vec::with_capacity(operation.operands.len());
            for operand in &operation.operands {
                match values.get(operand) {
                    Some(value) => element_values.push(value.clone()),
                    None => {
                        result.fail(
                            ExecutionStatus::InvalidRequest,
                            Some(identity.clone()),
                            "sequence construction operand was unavailable".to_owned(),
                        );
                        return Some(result.clone());
                    }
                }
            }
            if element_values.len() != *length as usize {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "sequence construction element count does not match the declared length"
                        .to_owned(),
                );
                return Some(result.clone());
            }
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Sequence {
                    values: element_values,
                },
            );
        }
        BodyOperationKind::VectorConstruct { lanes, .. } => {
            let Some(vector) = collect_operands(operation, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "vector construction operand was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            if vector.len() != *lanes as usize {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "vector construction lane count mismatch".to_owned(),
                );
                return Some(result.clone());
            }
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Vector { values: vector },
            );
        }
        BodyOperationKind::VectorSplat { lanes, .. } => {
            let Some(value) = values
                .get(operation.operands.first().unwrap_or(&String::new()))
                .cloned()
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "vector splat operand was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Vector {
                    values: vec![value; *lanes as usize],
                },
            );
        }
        BodyOperationKind::VectorExtract {
            lanes, evidence, ..
        } => {
            let Some(ExecutionValue::Vector { values: vector }) =
                values.get(operation.operands.first().unwrap_or(&String::new()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "vector extraction operand was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(index) = integer_operand(operation, values, 1) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "vector lane index was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            if index < 0 || index as u128 >= u128::from(*lanes) {
                let status = if matches!(evidence, BoundsEvidence::RuntimeChecked { .. }) {
                    ExecutionStatus::RuntimeFailure
                } else {
                    ExecutionStatus::InvalidRequest
                };
                result.fail(
                    status,
                    Some(identity.clone()),
                    "vector lane index is out of bounds".to_owned(),
                );
                return Some(result.clone());
            }
            values.insert(
                operation.results[0].id.clone(),
                vector[index as usize].clone(),
            );
        }
        BodyOperationKind::VectorReplace {
            lanes, evidence, ..
        } => {
            let Some(ExecutionValue::Vector { values: source }) =
                values.get(operation.operands.first().unwrap_or(&String::new()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "vector replacement source was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(index) = integer_operand(operation, values, 1) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "vector lane index was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(element) = values
                .get(operation.operands.get(2).unwrap_or(&String::new()))
                .cloned()
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "vector replacement element was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            if index < 0 || index as u128 >= u128::from(*lanes) {
                let status = if matches!(evidence, BoundsEvidence::RuntimeChecked { .. }) {
                    ExecutionStatus::RuntimeFailure
                } else {
                    ExecutionStatus::InvalidRequest
                };
                result.fail(
                    status,
                    Some(identity.clone()),
                    "vector lane index is out of bounds".to_owned(),
                );
                return Some(result.clone());
            }
            let mut updated = source.clone();
            updated[index as usize] = element;
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Vector { values: updated },
            );
        }
        BodyOperationKind::VectorBinary {
            operator,
            element_type,
            intent,
            ..
        } => {
            let Some((left, right)) = vector_operands(operation, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "vector binary operands were unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(produced) =
                evaluate_vector_binary(operator, element_type, *intent, left, right)
            else {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    Some(identity.clone()),
                    "vector arithmetic failed in at least one lane".to_owned(),
                );
                return Some(result.clone());
            };
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Vector { values: produced },
            );
        }
        BodyOperationKind::VectorCompare {
            predicate,
            element_type,
            ..
        } => {
            let Some((left, right)) = vector_operands(operation, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "vector comparison operands were unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(lanes) = evaluate_vector_compare(predicate, element_type, left, right) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "vector comparison operands violated their element type".to_owned(),
                );
                return Some(result.clone());
            };
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Mask { lanes },
            );
        }
        BodyOperationKind::MaskBinary { operator, .. } => {
            let Some((left, right)) = mask_operands(operation, values) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "mask operands were unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let lanes = left
                .iter()
                .zip(right)
                .map(|(left, right)| match operator.as_str() {
                    "and" => *left && *right,
                    "or" => *left || *right,
                    _ => *left != *right,
                })
                .collect();
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Mask { lanes },
            );
        }
        BodyOperationKind::MaskNot { .. } => {
            let Some(ExecutionValue::Mask { lanes }) =
                values.get(operation.operands.first().unwrap_or(&String::new()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "mask not operand was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Mask {
                    lanes: lanes.iter().map(|lane| !lane).collect(),
                },
            );
        }
        BodyOperationKind::MaskReduce { operator, .. } => {
            let Some(ExecutionValue::Mask { lanes }) =
                values.get(operation.operands.first().unwrap_or(&String::new()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "mask reduction operand was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let value = match operator.as_str() {
                "any" => lanes.iter().any(|lane| *lane),
                "all" => lanes.iter().all(|lane| *lane),
                _ => !lanes.iter().any(|lane| *lane),
            };
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Boolean { value },
            );
        }
        BodyOperationKind::VectorReduce {
            operator,
            element_type,
            intent,
            ..
        } => {
            let Some(ExecutionValue::Vector { values: lanes }) =
                values.get(operation.operands.first().unwrap_or(&String::new()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "vector reduction operand was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(value) = evaluate_vector_reduce(operator, element_type, *intent, lanes) else {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    Some(identity.clone()),
                    "vector reduction failed".to_owned(),
                );
                return Some(result.clone());
            };
            values.insert(operation.results[0].id.clone(), value);
        }
        BodyOperationKind::Select { operand_type: _ } => {
            let Some(condition) = values.get(operation.operands.first().unwrap_or(&String::new()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "selection condition was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            if let ExecutionValue::Mask { lanes } = condition {
                let Some(ExecutionValue::Vector { values: true_lanes }) =
                    values.get(&operation.operands[1])
                else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        Some(identity.clone()),
                        "masked selection true candidate was not a vector".to_owned(),
                    );
                    return Some(result.clone());
                };
                let Some(ExecutionValue::Vector {
                    values: false_lanes,
                }) = values.get(&operation.operands[2])
                else {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        Some(identity.clone()),
                        "masked selection false candidate was not a vector".to_owned(),
                    );
                    return Some(result.clone());
                };
                if lanes.len() != true_lanes.len() || lanes.len() != false_lanes.len() {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        Some(identity.clone()),
                        "masked selection lane count mismatch".to_owned(),
                    );
                    return Some(result.clone());
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
                    .collect();
                values.insert(
                    operation.results[0].id.clone(),
                    ExecutionValue::Vector { values: selected },
                );
                return None;
            }
            let ExecutionValue::Boolean { value: chosen_true } = condition else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "selection condition was neither bool nor mask".to_owned(),
                );
                return Some(result.clone());
            };
            // Both candidates are read as values; neither branch "executes".
            // This is the semantic distinction a branchless realization
            // preserves at the machine level.
            let candidate_index = if *chosen_true { 1 } else { 2 };
            let Some(selected) = values.get(
                operation
                    .operands
                    .get(candidate_index)
                    .unwrap_or(&String::new()),
            ) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "selection candidate was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let selected = selected.clone();
            values.insert(operation.results[0].id.clone(), selected);
        }
        BodyOperationKind::SequenceReplace {
            element_type: _,
            bound,
            evidence,
        } => {
            let SequenceBound::Exact(length) = bound else {
                result.fail(
                    ExecutionStatus::Unsupported,
                    Some(identity.clone()),
                    "functional sequence update requires an exact bound".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(source) = sequence_operand(operation, values, 0) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "functional update source was unavailable or not a sequence".to_owned(),
                );
                return Some(result.clone());
            };
            let source = source.to_vec();
            let Some(index) = integer_operand(operation, values, 1) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "functional update index was unavailable or not a u64 value".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(element) = values.get(operation.operands.get(2).unwrap_or(&String::new()))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "functional update element was unavailable".to_owned(),
                );
                return Some(result.clone());
            };
            let element = element.clone();
            if index < 0 || index as u128 >= u128::from(*length) {
                match evidence {
                    BoundsEvidence::RuntimeChecked { .. } => {
                        result.fail(
                            ExecutionStatus::RuntimeFailure,
                            Some(identity.clone()),
                            format!("functional update index {index} is outside the exact bound {length}"),
                        );
                    }
                    _ => {
                        result.fail(
                            ExecutionStatus::RuntimeFailure,
                            Some(identity.clone()),
                            format!("statically established update index {index} violated the declared bound; elaboration invariant broken"),
                        );
                    }
                }
                return Some(result.clone());
            }
            let mut updated = source;
            updated[index as usize] = element;
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Sequence { values: updated },
            );
        }
        BodyOperationKind::SequenceProject { bound: _, evidence } => {
            let Some(sequence_value) = sequence_operand(operation, values, 0) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "sequence projection operand was unavailable or not a sequence".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(index) = integer_operand(operation, values, 1) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "sequence index operand was unavailable or not a u64".to_owned(),
                );
                return Some(result.clone());
            };
            if !(0..=u64::MAX as i128).contains(&index) {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "sequence index is outside the u64 domain".to_owned(),
                );
                return Some(result.clone());
            }
            let length = sequence_value.len() as u128;
            let index = index as u128;
            match evidence {
                BoundsEvidence::StaticExact | BoundsEvidence::TraversalDomain => {
                    // The evidence claims validity; a violation would be a
                    // compiler defect, so fail closed without pretending it
                    // was a runtime check.
                    if index >= length {
                        result.fail(
                            ExecutionStatus::InvalidRequest,
                            Some(identity.clone()),
                            "statically established sequence bounds were violated".to_owned(),
                        );
                        return Some(result.clone());
                    }
                }
                BoundsEvidence::RuntimeChecked { failure: _ } => {
                    if index >= length {
                        result.fail(
                            ExecutionStatus::RuntimeFailure,
                            Some(identity.clone()),
                            "sequence index out of bounds".to_owned(),
                        );
                        return Some(result.clone());
                    }
                }
            }
            if let Some(value) = sequence_value.get(index as usize) {
                values.insert(operation.results[0].id.clone(), value.clone());
            }
        }
        BodyOperationKind::SequenceLength { bound: _ } => {
            let Some(sequence_value) = sequence_operand(operation, values, 0) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "length observation operand was unavailable or not a sequence".to_owned(),
                );
                return Some(result.clone());
            };
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Integer {
                    value: sequence_value.len() as i128,
                    ty: IntegerType {
                        bits: 64,
                        signed: false,
                    },
                },
            );
        }
        BodyOperationKind::ViewConstruct {
            source_bound: _,
            view_bound,
        } => {
            let SequenceBound::UpTo(capacity) = view_bound else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "view construction must produce an UpTo-bounded view".to_owned(),
                );
                return Some(result.clone());
            };
            let Some(source) = sequence_operand(operation, values, 0) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "view construction source was unavailable or not a sequence".to_owned(),
                );
                return Some(result.clone());
            };
            let (Some(start), Some(end)) = (
                integer_operand(operation, values, 1),
                integer_operand(operation, values, 2),
            ) else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "view range endpoints were unavailable or not u64 values".to_owned(),
                );
                return Some(result.clone());
            };
            if start < 0 || end < 0 || end < start {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    Some(identity.clone()),
                    "view range is not a valid half-open range".to_owned(),
                );
                return Some(result.clone());
            }
            if end as u128 > source.len() as u128 {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    Some(identity.clone()),
                    "view range end exceeds the source length".to_owned(),
                );
                return Some(result.clone());
            }
            if (end - start) as u128 > u128::from(*capacity) {
                result.fail(
                    ExecutionStatus::RuntimeFailure,
                    Some(identity.clone()),
                    "view range exceeds the declared view capacity".to_owned(),
                );
                return Some(result.clone());
            }
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Sequence {
                    values: source[start as usize..end as usize].to_vec(),
                },
            );
        }
        BodyOperationKind::FiniteConstruct {
            type_identity,
            variant_identity,
            discriminant,
            payload_fields,
        } => {
            // Payload values arrive in canonical field order via operands.
            let mut payload = Vec::new();
            let mut valid = true;
            for (index, field) in payload_fields.iter().enumerate() {
                match operation
                    .operands
                    .get(index)
                    .and_then(|operand| values.get(operand))
                {
                    Some(value) => payload.push((field.clone(), value.clone())),
                    None => valid = false,
                }
            }
            if !valid || operation.operands.len() != payload_fields.len() {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "finite constructor payload operand was unavailable".to_owned(),
                );
                return Some(result.clone());
            }
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Finite {
                    type_identity: type_identity.clone(),
                    variant_identity: variant_identity.clone(),
                    discriminant: *discriminant,
                    payload,
                },
            );
        }
        BodyOperationKind::FinitePayloadProject {
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
            }) = operation
                .operands
                .first()
                .and_then(|operand| values.get(operand))
            else {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "payload projection operand was unavailable or not a finite value".to_owned(),
                );
                return Some(result.clone());
            };
            if actual_type != type_identity
                || actual_variant != variant_identity
                || actual_discriminant != discriminant
                || !valid_finite_value(program, actual_type, actual_variant, *actual_discriminant)
            {
                result.fail(
                    ExecutionStatus::InvalidRequest,
                    Some(identity.clone()),
                    "payload projection operand has an invalid type/variant/discriminant"
                        .to_owned(),
                );
                return Some(result.clone());
            }
            match payload.iter().find(|(name, _)| name == field) {
                Some((_, value)) => {
                    values.insert(operation.results[0].id.clone(), value.clone());
                }
                None => {
                    result.fail(
                        ExecutionStatus::InvalidRequest,
                        Some(identity.clone()),
                        format!("payload projection field {field:?} was not carried by the value"),
                    );
                    return Some(result.clone());
                }
            }
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
                ..
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
        BodyOperationKind::Call {
            function,
            function_name,
            ..
        } => {
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
                    module: program
                        .functions
                        .iter()
                        .find(|candidate| {
                            function_id(
                                candidate.identity_namespace(&program.module),
                                &candidate.name,
                            ) == *function
                        })
                        .map(|candidate| candidate.identity_namespace(&program.module).to_owned())
                        .unwrap_or_else(|| request.target.module.clone()),
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
        (ExecutionValue::Byte { value }, BodyType::Byte) => (0..=255).contains(value),
        (
            ExecutionValue::Sequence { values },
            BodyType::Sequence {
                element,
                bound: SequenceBound::Exact(length),
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
                bound: SequenceBound::UpTo(capacity),
            },
        ) => {
            values.len() <= *capacity as usize
                && values
                    .iter()
                    .all(|element_value| value_matches_type(program, element_value, element))
        }
        (
            ExecutionValue::Finite {
                type_identity,
                variant_identity,
                discriminant,
                payload,
            },
            BodyType::Finite { identity, .. },
        ) => {
            type_identity == identity
                && valid_finite_value(program, type_identity, variant_identity, *discriminant)
                && finite_payload_matches(program, type_identity, variant_identity, payload)
        }
        (
            ExecutionValue::Record {
                type_identity,
                fields,
                ..
            },
            BodyType::Record { identity, .. },
        ) => type_identity == identity && record_fields_match(program, type_identity, fields),
        _ => false,
    }
}

/// A logical record value matches its declaration when the field sets agree
/// exactly (both sides keep fields sorted by name) and every field value
/// matches its declared semantic type.
fn record_fields_match(
    program: &Program,
    record_identity: &SemanticId,
    fields: &[(String, ExecutionValue)],
) -> bool {
    let Some(declaration) = program
        .record_types
        .iter()
        .find(|decl| &decl.identity == record_identity)
    else {
        return false;
    };
    if declaration.fields.len() != fields.len() {
        return false;
    }
    declaration
        .fields
        .iter()
        .zip(fields.iter())
        .all(|(declared, (field_name, field_value))| {
            field_name == &declared.name
                && value_matches_named_type(program, field_value, &declared.field_type)
        })
}

/// A finite value's payload matches its declared variant when the field sets
/// agree exactly (both sides keep fields sorted by name) and every payload
/// value matches its declared semantic type.
fn finite_payload_matches(
    program: &Program,
    type_identity: &SemanticId,
    variant_identity: &SemanticId,
    payload: &[(String, ExecutionValue)],
) -> bool {
    let Some(declared_payload) = program
        .finite_types
        .iter()
        .find(|decl| &decl.identity == type_identity)
        .and_then(|finite_type| {
            finite_type
                .variants
                .iter()
                .find(|variant| &variant.identity == variant_identity)
        })
        .map(|variant| variant.payload.clone())
    else {
        return payload.is_empty();
    };
    if declared_payload.len() != payload.len() {
        return false;
    }
    declared_payload
        .iter()
        .zip(payload.iter())
        .all(|(declared, (field_name, field_value))| {
            field_name == &declared.name
                && value_matches_named_type(program, field_value, &declared.field_type)
        })
}

fn value_matches_named_type(program: &Program, value: &ExecutionValue, name: &str) -> bool {
    let derived = BodyType::from_semantic_name(name);
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
    false
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
        BodyType::Byte if (0..=255).contains(&value) => Some(ExecutionValue::Byte { value }),
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
                payload,
            },
            BodyType::Finite { identity, .. },
        ) if type_identity == identity
            && valid_finite_value(program, type_identity, variant_identity, *discriminant)
            && finite_payload_matches(program, type_identity, variant_identity, payload) =>
        {
            Some(value.clone())
        }
        (
            ExecutionValue::Record {
                type_identity,
                fields,
                ..
            },
            BodyType::Record { identity, .. },
        ) if type_identity == identity && record_fields_match(program, type_identity, fields) => {
            Some(value.clone())
        }
        (ExecutionValue::Byte { value }, BodyType::Byte) if (0..=255).contains(value) => {
            Some(ExecutionValue::Byte { value: *value })
        }
        (
            ExecutionValue::Sequence { values },
            BodyType::Sequence {
                element,
                bound: SequenceBound::Exact(length),
            },
        ) if values.len() == *length as usize => normalize_sequence(program, values, element),
        (
            ExecutionValue::Sequence { values },
            BodyType::Sequence {
                element,
                bound: SequenceBound::UpTo(capacity),
            },
        ) if values.len() <= *capacity as usize => normalize_sequence(program, values, element),
        _ => None,
    }
}

fn normalize_sequence(
    program: &Program,
    values: &[ExecutionValue],
    element_type: &BodyType,
) -> Option<ExecutionValue> {
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        normalized.push(normalize_value(program, value, element_type)?);
    }
    Some(ExecutionValue::Sequence { values: normalized })
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

fn boolean_operands(
    operation: &BodyOperation,
    values: &BTreeMap<String, ExecutionValue>,
) -> Option<(bool, bool)> {
    let [left, right] = operation.operands.as_slice() else {
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

fn byte_operands(
    operation: &BodyOperation,
    values: &BTreeMap<String, ExecutionValue>,
) -> Option<(i128, i128)> {
    let [left, right] = operation.operands.as_slice() else {
        return None;
    };
    let Some(ExecutionValue::Byte { value: left }) = values.get(left) else {
        return None;
    };
    let Some(ExecutionValue::Byte { value: right }) = values.get(right) else {
        return None;
    };
    Some((*left, *right))
}

fn byte_shift_operands(
    operation: &BodyOperation,
    values: &BTreeMap<String, ExecutionValue>,
) -> Option<(i128, i128)> {
    let [left, right] = operation.operands.as_slice() else {
        return None;
    };
    let Some(ExecutionValue::Byte { value: left }) = values.get(left) else {
        return None;
    };
    let Some(ExecutionValue::Integer { value: right, .. }) = values.get(right) else {
        return None;
    };
    if *right < 0 {
        return None;
    }
    Some((*left, *right))
}

fn sequence_operand<'a>(
    operation: &BodyOperation,
    values: &'a BTreeMap<String, ExecutionValue>,
    index: usize,
) -> Option<&'a [ExecutionValue]> {
    match values.get(operation.operands.get(index)?)? {
        ExecutionValue::Sequence { values } => Some(values),
        _ => None,
    }
}

fn integer_operand(
    operation: &BodyOperation,
    values: &BTreeMap<String, ExecutionValue>,
    index: usize,
) -> Option<i128> {
    match values.get(operation.operands.get(index)?)? {
        ExecutionValue::Integer { value, .. } => Some(*value),
        _ => None,
    }
}

pub(crate) fn compare_bytes(predicate: &str, left: i128, right: i128) -> Option<bool> {
    if !(0..=255).contains(&left) || !(0..=255).contains(&right) {
        return None;
    }
    Some(match predicate {
        "eq" => left == right,
        "ne" => left != right,
        // Byte ordering is unsigned by definition.
        "lt" => left < right,
        "le" => left <= right,
        "gt" => left > right,
        "ge" => left >= right,
        _ => return None,
    })
}

pub(crate) fn evaluate_byte_bitwise(operator: &str, left: i128, right: i128) -> Option<i128> {
    if !(0..=255).contains(&left) || !(0..=255).contains(&right) {
        return None;
    }
    match operator {
        "and" => Some(left & right),
        "or" => Some(left | right),
        "xor" => Some(left ^ right),
        _ => None,
    }
}

/// Byte shifts are total: the count is taken modulo 8 and both shifts are
/// logical (bytes are unsigned by definition).
pub(crate) fn evaluate_byte_shift(operator: &str, byte: i128, count: i128) -> Option<i128> {
    if !(0..=255).contains(&byte) || count < 0 {
        return None;
    }
    let count = (count % 8) as u32;
    match operator {
        "shl" => Some((byte << count) & 0xFF),
        "shr" => Some(byte >> count),
        _ => None,
    }
}

/// Total two's-complement truncation/extension into `bits` width.
/// Widening extends by the declared signedness of the target; narrowing
/// truncates high bits. Total for 1..=126 bits.
pub(crate) fn wrap_to_bits(value: i128, bits: u16, signed: bool) -> Option<i128> {
    if !(1..=126).contains(&bits) {
        return None;
    }
    let modulus = 1_i128.checked_shl(u32::from(bits))?;
    let truncated = value.rem_euclid(modulus);
    if signed {
        let sign = 1_i128.checked_shl(u32::from(bits - 1))?;
        Some(if truncated >= sign {
            truncated - modulus
        } else {
            truncated
        })
    } else {
        Some(truncated)
    }
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

fn collect_operands(
    operation: &BodyOperation,
    values: &BTreeMap<String, ExecutionValue>,
) -> Option<Vec<ExecutionValue>> {
    operation
        .operands
        .iter()
        .map(|operand| values.get(operand).cloned())
        .collect()
}

fn vector_operands<'a>(
    operation: &BodyOperation,
    values: &'a BTreeMap<String, ExecutionValue>,
) -> Option<(&'a [ExecutionValue], &'a [ExecutionValue])> {
    let ExecutionValue::Vector { values: left } = values.get(operation.operands.first()?)? else {
        return None;
    };
    let ExecutionValue::Vector { values: right } = values.get(operation.operands.get(1)?)? else {
        return None;
    };
    (left.len() == right.len()).then_some((left, right))
}

fn mask_operands<'a>(
    operation: &BodyOperation,
    values: &'a BTreeMap<String, ExecutionValue>,
) -> Option<(&'a [bool], &'a [bool])> {
    let ExecutionValue::Mask { lanes: left } = values.get(operation.operands.first()?)? else {
        return None;
    };
    let ExecutionValue::Mask { lanes: right } = values.get(operation.operands.get(1)?)? else {
        return None;
    };
    (left.len() == right.len()).then_some((left, right))
}

pub(crate) fn evaluate_vector_binary(
    operator: &str,
    element_type: &BodyType,
    intent: ArithmeticIntent,
    left: &[ExecutionValue],
    right: &[ExecutionValue],
) -> Option<Vec<ExecutionValue>> {
    let BodyType::Integer(ty) = element_type else {
        return None;
    };
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let (
                ExecutionValue::Integer {
                    value: left_value,
                    ty: left_ty,
                },
                ExecutionValue::Integer {
                    value: right_value,
                    ty: right_ty,
                },
            ) = (left, right)
            else {
                return None;
            };
            if left_ty != ty || right_ty != ty {
                return None;
            }
            let value = match operator {
                "min" => (*left_value).min(*right_value),
                "max" => (*left_value).max(*right_value),
                _ => evaluate_integer(operator, *ty, intent, *left_value, *right_value)?,
            };
            Some(ExecutionValue::Integer { value, ty: *ty })
        })
        .collect()
}

pub(crate) fn evaluate_vector_compare(
    predicate: &str,
    element_type: &BodyType,
    left: &[ExecutionValue],
    right: &[ExecutionValue],
) -> Option<Vec<bool>> {
    let BodyType::Integer(ty) = element_type else {
        return None;
    };
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let (
                ExecutionValue::Integer {
                    value: left_value,
                    ty: left_ty,
                },
                ExecutionValue::Integer {
                    value: right_value,
                    ty: right_ty,
                },
            ) = (left, right)
            else {
                return None;
            };
            if left_ty != ty || right_ty != ty {
                return None;
            }
            compare_integers(predicate, *ty, *left_value, *right_value)
        })
        .collect()
}

pub(crate) fn evaluate_vector_reduce(
    operator: &str,
    element_type: &BodyType,
    intent: ArithmeticIntent,
    lanes: &[ExecutionValue],
) -> Option<ExecutionValue> {
    let BodyType::Integer(ty) = element_type else {
        return None;
    };
    let mut values = lanes.iter().map(|lane| match lane {
        ExecutionValue::Integer { value, ty: lane_ty } if lane_ty == ty => Some(*value),
        _ => None,
    });
    let mut reduced = values.next()??;
    for value in values {
        let value = value?;
        reduced = match operator {
            "min" => reduced.min(value),
            "max" => reduced.max(value),
            "sum" => evaluate_integer("add", *ty, intent, reduced, value)?,
            _ => return None,
        };
    }
    Some(ExecutionValue::Integer {
        value: reduced,
        ty: *ty,
    })
}

pub(crate) fn integer_operator_supported(operator: &str, intent: ArithmeticIntent) -> bool {
    matches!(operator, "add" | "sub" | "mul" | "div" | "mod")
        || (matches!(operator, "and" | "or" | "xor" | "shl" | "shr")
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
    let result_bits = match intent {
        ArithmeticIntent::Widening { bits } => bits,
        _ => ty.bits,
    };
    if crate::arithmetic_result_type(operator, ty, intent)
        != Some(IntegerType {
            bits: result_bits,
            signed: ty.signed,
        })
    {
        return None;
    }
    if !(1..=126).contains(&result_bits) {
        return None;
    }
    crate::IntegerOperation {
        operator: operator.to_owned(),
        operand_type: ty,
        left,
        right,
        intent,
    }
    .evaluate()
    .value
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
        block: body.block_identity(
            function.identity_namespace(&program.module),
            &function.name,
            &block.id,
        ),
        operation: operation.map(|operation| {
            operation.identity(
                function.identity_namespace(&program.module),
                &function.name,
                &block.id,
            )
        }),
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
    let function_identity = (!function.is_empty()).then(|| {
        program
            .functions
            .iter()
            .find(|candidate| candidate.name == function)
            .map(|candidate| {
                function_id(
                    candidate.identity_namespace(&program.module),
                    &candidate.name,
                )
            })
            .unwrap_or_else(|| function_id(&program.module, &function))
    });
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
                dependencies: Vec::new(),
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
                home_module: None,
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
