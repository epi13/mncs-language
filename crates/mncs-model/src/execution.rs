//! Deterministic, bounded reference execution for the validated executable body subset.
//!
//! This module is deliberately not a runtime or backend. It executes only
//! semantics already represented by a validated `FunctionBody`, records a
//! bounded identity trace, and returns explicit results for unsupported or
//! resource-bounded cases.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::canonical::{canonical_json_value, sha256_hex};
use crate::identity::{function_id, SemanticId};
use crate::{
    ArithmeticIntent, BodyBlock, BodyOperation, BodyOperationKind, BodyTerminator, BodyType,
    BoundsEvidence, Function, FunctionBody, IntegerType, Program, SequenceBound,
};

pub const EXECUTION_REQUEST_SCHEMA_VERSION: &str = "0.1";
pub const EXECUTION_RESULT_SCHEMA_VERSION: &str = "0.1";
pub const EXECUTION_CORPUS_SCHEMA_VERSION: &str = "0.1";
pub const EXECUTION_CORPUS_SCHEMA_VERSION_0_2: &str = "0.2";
pub const EXECUTION_CORPUS_SCHEMA_VERSION_0_3: &str = "0.3";
pub const EXECUTION_COMPARISON_SCHEMA_VERSION: &str = "0.1";
pub const STATEFUL_EXECUTION_SCHEMA_VERSION: &str = "0.1";
pub const STATEFUL_EXECUTION_COMPARISON_SCHEMA_VERSION: &str = "0.1";
/// Maximum bounded reference work for one request. Large aggregate model
/// finals may legitimately cross one million SSA instructions; the bound is
/// still finite and is surfaced as `BudgetExhausted` when exceeded.
pub const MAX_EXECUTION_BUDGET: u64 = 8_000_000;
pub const MAX_STATEFUL_CALLS: u32 = 4096;
const MAX_TRACE_ENTRIES: usize = 256;

pub fn execution_corpus_schema_supported(schema_version: &str) -> bool {
    matches!(
        schema_version,
        EXECUTION_CORPUS_SCHEMA_VERSION
            | EXECUTION_CORPUS_SCHEMA_VERSION_0_2
            | EXECUTION_CORPUS_SCHEMA_VERSION_0_3
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
        payload: Arc<Vec<(String, ExecutionValue)>>,
    },
    /// A logical record value.  Fields are kept in canonical (sorted) name
    /// order; this is the *logical* value model — no physical layout, offset,
    /// or packing is part of its meaning.
    Record {
        type_identity: SemanticId,
        name: String,
        fields: Arc<Vec<(String, ExecutionValue)>>,
    },
    /// A byte-oriented logical value (Profile 0.7). The domain is 0..=255;
    /// no host pointer or storage shape participates in meaning.
    Byte {
        value: i128,
    },
    /// A bounded sequence's logical value: exactly its ordered element
    /// values. Length facts come from the type, not from the value.
    Sequence {
        values: Arc<Vec<ExecutionValue>>,
    },
    /// Logical vector lanes. No storage/register representation participates
    /// in this reference value.
    Vector {
        values: Arc<Vec<ExecutionValue>>,
    },
    /// Logical lane predicates. Backends may realize these as packed bits.
    Mask {
        lanes: Arc<Vec<bool>>,
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

/// An argument in a bounded stateful trace.  A previous result is a logical
/// value reference, not a host pointer or a backend-specific handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatefulArgument {
    Literal(ExecutionValue),
    PreviousResult { step_id: String, result_index: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatefulExecutionStep {
    pub id: String,
    pub target: ExecutionTarget,
    pub arguments: Vec<StatefulArgument>,
    pub step_budget: u64,
    #[serde(default)]
    pub policy: ExecutionPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<Vec<ExecutionValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_status: Option<ExecutionStatus>,
    /// Keep the complete logical return only for selected observations.  A
    /// digest is retained for every successful transition, allowing large
    /// states to be compared without serializing the same state at every
    /// chunk boundary.
    #[serde(default)]
    pub observe_returned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatefulExecutionCase {
    pub id: String,
    pub steps: Vec<StatefulExecutionStep>,
    pub maximum_calls: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_total_steps: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_final: Option<Vec<ExecutionValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_final_status: Option<ExecutionStatus>,
}

impl StatefulExecutionCase {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.id.trim().is_empty() {
            errors.push("stateful case id must not be empty".to_owned());
        }
        if self.steps.is_empty() {
            errors.push("stateful case must contain at least one step".to_owned());
        }
        if self.maximum_calls == 0 || self.maximum_calls > MAX_STATEFUL_CALLS {
            errors.push(format!(
                "maximum_calls must be between 1 and {MAX_STATEFUL_CALLS}"
            ));
        }
        if self.steps.len() > self.maximum_calls as usize {
            errors.push("stateful step count exceeds maximum_calls".to_owned());
        }
        if self.maximum_total_steps == Some(0) {
            errors.push("maximum_total_steps must be greater than zero".to_owned());
        }
        let mut ids = std::collections::BTreeSet::new();
        for (index, step) in self.steps.iter().enumerate() {
            if step.id.trim().is_empty() || !ids.insert(step.id.clone()) {
                errors.push(format!(
                    "stateful step {index} has a missing or duplicate id"
                ));
            }
            if step.step_budget == 0 || step.step_budget > MAX_EXECUTION_BUDGET {
                errors.push(format!(
                    "stateful step {:?} has an invalid step budget",
                    step.id
                ));
            }
            for argument in &step.arguments {
                if let StatefulArgument::PreviousResult { step_id, .. } = argument {
                    if !ids.contains(step_id) {
                        errors.push(format!(
                            "stateful step {:?} references a later or unknown step {:?}",
                            step.id, step_id
                        ));
                    }
                }
            }
        }
        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatefulCallResult {
    pub status: ExecutionStatus,
    pub returned: Vec<ExecutionValue>,
    pub steps: u64,
    pub effects: Vec<ExecutionEffectEvent>,
    pub failure: Option<ExecutionFailure>,
    pub program_identity: Option<SemanticId>,
    pub program_fingerprint: Option<String>,
}

impl From<&ExecutionResult> for StatefulCallResult {
    fn from(result: &ExecutionResult) -> Self {
        Self {
            status: result.status,
            returned: result.returned.clone(),
            steps: result.steps,
            effects: result.effects.clone(),
            failure: result.failure.clone(),
            program_identity: result.program_identity.clone(),
            program_fingerprint: result.program_fingerprint.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatefulStepObservation {
    pub step_id: String,
    pub target: ExecutionTarget,
    pub status: ExecutionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returned: Option<Vec<ExecutionValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returned_digest: Option<String>,
    pub steps: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<ExecutionEffectEvent>,
    pub failure: Option<ExecutionFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatefulExecutionResult {
    pub schema_version: String,
    pub case_id: String,
    pub corpus_identity: String,
    pub trace_identity: SemanticId,
    pub status: ExecutionStatus,
    pub observations: Vec<StatefulStepObservation>,
    pub returned: Vec<ExecutionValue>,
    pub steps: u64,
    pub calls: u32,
    pub failure: Option<ExecutionFailure>,
    pub program_identity: Option<SemanticId>,
    pub program_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_identity: Option<SemanticId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_identity: Option<SemanticId>,
}

impl StatefulExecutionResult {
    pub fn seal(&mut self) {
        self.trace_identity = SemanticId(String::new());
        self.trace_identity = stateful_trace_identity(self);
    }

    pub fn identity_is_valid(&self) -> bool {
        self.schema_version == STATEFUL_EXECUTION_SCHEMA_VERSION
            && !self.case_id.trim().is_empty()
            && !self.corpus_identity.trim().is_empty()
            && self.trace_identity == stateful_trace_identity(self)
            && self.calls == self.observations.len() as u32
    }
}

/// An in-process checkpoint for a validated stateful prefix. The checkpoint
/// is deliberately opaque to backend adapters: its prefix identity covers the
/// corpus name, case bounds, and exact step definitions, while the backend
/// session that stores it must additionally bind it to an artifact identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatefulExecutionCheckpoint {
    schema_version: String,
    /// The artifact/backend session scope is present for backend-owned
    /// checkpoints. A checkpoint without a scope remains available to the
    /// model-only helper, which has no executable artifact to bind.
    session_scope: Option<SemanticId>,
    prefix_identity: SemanticId,
    next_step_index: usize,
    previous: BTreeMap<String, Vec<ExecutionValue>>,
    observations: Vec<StatefulStepObservation>,
    returned: Vec<ExecutionValue>,
    steps: u64,
    calls: u32,
    program_identity: Option<SemanticId>,
    program_fingerprint: Option<String>,
    checkpoint_identity: SemanticId,
}

impl StatefulExecutionCheckpoint {
    #[allow(clippy::too_many_arguments)]
    fn new(
        corpus_name: &str,
        case: &StatefulExecutionCase,
        session_scope: Option<SemanticId>,
        next_step_index: usize,
        previous: BTreeMap<String, Vec<ExecutionValue>>,
        observations: Vec<StatefulStepObservation>,
        returned: Vec<ExecutionValue>,
        steps: u64,
        calls: u32,
        program_identity: Option<SemanticId>,
        program_fingerprint: Option<String>,
    ) -> Self {
        let mut checkpoint = Self {
            schema_version: STATEFUL_EXECUTION_SCHEMA_VERSION.to_owned(),
            session_scope,
            prefix_identity: stateful_prefix_identity(corpus_name, case, next_step_index),
            next_step_index,
            previous,
            observations,
            returned,
            steps,
            calls,
            program_identity,
            program_fingerprint,
            checkpoint_identity: SemanticId(String::new()),
        };
        checkpoint.seal();
        checkpoint
    }

    fn seal(&mut self) {
        self.checkpoint_identity = SemanticId(String::new());
        let material = (
            &self.schema_version,
            &self.session_scope,
            &self.prefix_identity,
            self.next_step_index,
            &self.previous,
            &self.observations,
            &self.returned,
            self.steps,
            self.calls,
            &self.program_identity,
            &self.program_fingerprint,
        );
        let canonical = canonical_json_value(&material).expect("checkpoint is serializable");
        self.checkpoint_identity = SemanticId(format!(
            "mncs:language:stateful-checkpoint:{}",
            sha256_hex(canonical.as_bytes())
        ));
    }

    pub fn next_step_index(&self) -> usize {
        self.next_step_index
    }

    pub fn identity_is_valid_for(&self, corpus_name: &str, case: &StatefulExecutionCase) -> bool {
        self.identity_is_valid_for_scope(corpus_name, case, None)
    }

    pub fn identity_is_valid_for_scope(
        &self,
        corpus_name: &str,
        case: &StatefulExecutionCase,
        session_scope: Option<&SemanticId>,
    ) -> bool {
        if self.next_step_index > case.steps.len() || self.session_scope.as_ref() != session_scope {
            return false;
        }
        let prefix_step_ids = case.steps[..self.next_step_index]
            .iter()
            .map(|step| step.id.as_str())
            .collect::<BTreeSet<_>>();
        self.schema_version == STATEFUL_EXECUTION_SCHEMA_VERSION
            && self.next_step_index <= case.steps.len()
            && self.prefix_identity
                == stateful_prefix_identity(corpus_name, case, self.next_step_index)
            && self.observations.len() == self.next_step_index
            && self.calls == self.next_step_index as u32
            && self.checkpoint_identity == {
                let mut copy = self.clone();
                copy.seal();
                copy.checkpoint_identity
            }
            && case.steps[self.next_step_index..].iter().all(|step| {
                step.arguments.iter().all(|argument| match argument {
                    StatefulArgument::Literal(_) => true,
                    StatefulArgument::PreviousResult {
                        step_id,
                        result_index,
                    } => {
                        !prefix_step_ids.contains(step_id.as_str())
                            || self
                                .previous
                                .get(step_id)
                                .and_then(|values| values.get(*result_index as usize))
                                .is_some()
                    }
                })
            })
    }
}

/// Stable identity for an executable stateful prefix. Expected observations
/// are included in the step definitions, so a checkpoint cannot silently
/// cross an observation-policy or bound change.
pub fn stateful_prefix_identity(
    corpus_name: &str,
    case: &StatefulExecutionCase,
    next_step_index: usize,
) -> SemanticId {
    let end = next_step_index.min(case.steps.len());
    let material = (
        STATEFUL_EXECUTION_SCHEMA_VERSION,
        corpus_name,
        case.maximum_calls,
        case.maximum_total_steps,
        &case.steps[..end],
    );
    let canonical = canonical_json_value(&material).expect("stateful prefix is serializable");
    SemanticId(format!(
        "mncs:language:stateful-prefix:{}",
        sha256_hex(canonical.as_bytes())
    ))
}

/// Execute a bounded sequence while retaining only logical returned values.
/// The closure is the backend boundary; all sequencing, reference resolution,
/// bounds, and trace identity remain language-owned.
pub fn execute_stateful_case<F>(
    corpus_name: &str,
    case: &StatefulExecutionCase,
    mut execute: F,
) -> StatefulExecutionResult
where
    F: FnMut(&ExecutionRequest) -> StatefulCallResult,
{
    execute_stateful_case_owned(corpus_name, case, |request| execute(&request))
}

/// Owned-request variant for backend sessions. The request is constructed by
/// the language-owned runner and transferred into the session, so large
/// immutable checkpoint values are not deep-cloned at the call boundary.
pub fn execute_stateful_case_owned<F>(
    corpus_name: &str,
    case: &StatefulExecutionCase,
    mut execute: F,
) -> StatefulExecutionResult
where
    F: FnMut(ExecutionRequest) -> StatefulCallResult,
{
    let mut checkpoints = Vec::new();
    execute_stateful_case_inner(
        corpus_name,
        case,
        &mut execute,
        None,
        None,
        &[],
        &mut checkpoints,
    )
}

/// Execute a stateful trace from an identity-checked prefix checkpoint and
/// optionally capture selected successful prefixes for later sibling traces.
/// Checkpoints never bypass per-step request execution after the selected
/// prefix, and an invalid checkpoint fails closed as an invalid request.
pub fn execute_stateful_case_with_checkpoint<F>(
    corpus_name: &str,
    case: &StatefulExecutionCase,
    checkpoint: Option<&StatefulExecutionCheckpoint>,
    checkpoint_indices: &[usize],
    execute: F,
) -> (StatefulExecutionResult, Vec<StatefulExecutionCheckpoint>)
where
    F: FnMut(ExecutionRequest) -> StatefulCallResult,
{
    execute_stateful_case_with_checkpoint_scoped(
        None,
        corpus_name,
        case,
        checkpoint,
        checkpoint_indices,
        execute,
    )
}

/// Scoped checkpoint variant used by reusable backend sessions. The scope
/// must identify the exact immutable artifact/backend session that owns the
/// retained logical values.
pub fn execute_stateful_case_with_checkpoint_scoped<F>(
    session_scope: Option<&SemanticId>,
    corpus_name: &str,
    case: &StatefulExecutionCase,
    checkpoint: Option<&StatefulExecutionCheckpoint>,
    checkpoint_indices: &[usize],
    mut execute: F,
) -> (StatefulExecutionResult, Vec<StatefulExecutionCheckpoint>)
where
    F: FnMut(ExecutionRequest) -> StatefulCallResult,
{
    let mut checkpoints = Vec::new();
    let result = execute_stateful_case_inner(
        corpus_name,
        case,
        &mut execute,
        checkpoint,
        session_scope,
        checkpoint_indices,
        &mut checkpoints,
    );
    (result, checkpoints)
}

fn execute_stateful_case_inner<F>(
    corpus_name: &str,
    case: &StatefulExecutionCase,
    execute: &mut F,
    checkpoint: Option<&StatefulExecutionCheckpoint>,
    session_scope: Option<&SemanticId>,
    checkpoint_indices: &[usize],
    captured_checkpoints: &mut Vec<StatefulExecutionCheckpoint>,
) -> StatefulExecutionResult
where
    F: FnMut(ExecutionRequest) -> StatefulCallResult,
{
    crate::record_counter("stateful_trace");
    let corpus_identity = canonical_json_value(&(corpus_name, case))
        .map(|json| sha256_hex(json.as_bytes()))
        .unwrap_or_default();
    let mut result = StatefulExecutionResult {
        schema_version: STATEFUL_EXECUTION_SCHEMA_VERSION.to_owned(),
        case_id: case.id.clone(),
        corpus_identity,
        trace_identity: SemanticId(String::new()),
        status: ExecutionStatus::InvalidRequest,
        observations: Vec::new(),
        returned: Vec::new(),
        steps: 0,
        calls: 0,
        failure: None,
        program_identity: None,
        program_fingerprint: None,
        backend_identity: None,
        artifact_identity: None,
    };
    let validation = case.validate();
    if !validation.is_empty() {
        result.failure = Some(ExecutionFailure {
            identity: None,
            reason: validation.join("; "),
        });
        result.seal();
        return result;
    }

    let start_index = checkpoint.map_or(0, StatefulExecutionCheckpoint::next_step_index);
    if checkpoint.is_some_and(|checkpoint| {
        !checkpoint.identity_is_valid_for_scope(corpus_name, case, session_scope)
    }) {
        result.failure = Some(ExecutionFailure {
            identity: None,
            reason: "stateful prefix checkpoint identity or retained references are invalid"
                .to_owned(),
        });
        result.seal();
        return result;
    }
    if let Some(checkpoint) = checkpoint {
        result.status = ExecutionStatus::Returned;
        result.observations = checkpoint.observations.clone();
        result.returned = checkpoint.returned.clone();
        result.steps = checkpoint.steps;
        result.calls = checkpoint.calls;
        result.program_identity = checkpoint.program_identity.clone();
        result.program_fingerprint = checkpoint.program_fingerprint.clone();
    }
    let mut remaining_references = BTreeMap::<String, u32>::new();
    for step in &case.steps[start_index..] {
        for argument in &step.arguments {
            if let StatefulArgument::PreviousResult { step_id, .. } = argument {
                *remaining_references.entry(step_id.clone()).or_default() += 1;
            }
        }
    }
    let mut previous = checkpoint
        .map(|checkpoint| checkpoint.previous.clone())
        .unwrap_or_default();
    for (index, step) in case.steps.iter().enumerate().skip(start_index) {
        crate::record_counter("stateful_call");
        let arguments = (|| {
            let mut arguments = Vec::with_capacity(step.arguments.len());
            for argument in &step.arguments {
                let value = match argument {
                    StatefulArgument::Literal(value) => value.clone(),
                    StatefulArgument::PreviousResult {
                        step_id,
                        result_index,
                    } => {
                        let remaining = remaining_references
                            .get_mut(step_id)
                            .expect("validated stateful reference count");
                        *remaining = remaining.saturating_sub(1);
                        if *remaining == 0 {
                            let mut values = previous.remove(step_id).ok_or_else(|| {
                                format!(
                                    "stateful step {:?} references unavailable result {} from {:?}",
                                    step.id, result_index, step_id
                                )
                            })?;
                            values
                                .get(*result_index as usize)
                                .is_some()
                                .then(|| values.swap_remove(*result_index as usize))
                                .ok_or_else(|| {
                                    format!(
                                        "stateful step {:?} references unavailable result {} from {:?}",
                                        step.id, result_index, step_id
                                    )
                                })?
                        } else {
                            previous
                                .get(step_id)
                                .and_then(|values| values.get(*result_index as usize))
                                .cloned()
                                .ok_or_else(|| {
                                    format!(
                                        "stateful step {:?} references unavailable result {} from {:?}",
                                        step.id, result_index, step_id
                                    )
                                })?
                        }
                    }
                };
                arguments.push(value);
            }
            Ok::<Vec<ExecutionValue>, String>(arguments)
        })();
        let arguments = match arguments {
            Ok(arguments) => arguments,
            Err(reason) => {
                result.status = ExecutionStatus::InvalidRequest;
                result.failure = Some(ExecutionFailure {
                    identity: None,
                    reason,
                });
                break;
            }
        };
        let request = ExecutionRequest {
            schema_version: EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
            target: step.target.clone(),
            arguments,
            step_budget: step.step_budget,
            policy: step.policy.clone(),
        };
        let call = execute(request);
        result.calls += 1;
        result.steps = match result.steps.checked_add(call.steps) {
            Some(steps) => steps,
            None => {
                result.status = ExecutionStatus::BudgetExhausted;
                result.failure = Some(ExecutionFailure {
                    identity: None,
                    reason: "stateful aggregate step count overflowed".to_owned(),
                });
                break;
            }
        };
        result.program_identity = result.program_identity.or(call.program_identity.clone());
        result.program_fingerprint = result
            .program_fingerprint
            .clone()
            .or(call.program_fingerprint.clone());
        let returned_digest = (call.status == ExecutionStatus::Returned)
            .then(|| canonical_json_value(&call.returned).map(|json| sha256_hex(json.as_bytes())))
            .transpose()
            .unwrap_or(None);
        let terminal = call.status != ExecutionStatus::Returned || index + 1 == case.steps.len();
        let checkpoint_returned = checkpoint_indices
            .binary_search(&(index + 1))
            .is_ok()
            .then(|| call.returned.clone());
        let retain_returned = terminal || step.observe_returned || step.expected.is_some();
        result.observations.push(StatefulStepObservation {
            step_id: step.id.clone(),
            target: step.target.clone(),
            status: call.status,
            returned: retain_returned.then(|| call.returned.clone()),
            returned_digest,
            steps: call.steps,
            effects: call.effects,
            failure: call.failure.clone(),
        });
        if call.status != ExecutionStatus::Returned {
            result.status = call.status;
            result.failure = call.failure;
            break;
        }
        if let Some(maximum) = case.maximum_total_steps {
            if result.steps > maximum {
                result.status = ExecutionStatus::BudgetExhausted;
                result.failure = Some(ExecutionFailure {
                    identity: None,
                    reason: format!(
                        "stateful aggregate step bound {maximum} was exceeded after {:?}",
                        step.id
                    ),
                });
                break;
            }
        }
        if terminal {
            result.returned = call.returned;
        } else if remaining_references
            .get(&step.id)
            .copied()
            .unwrap_or_default()
            > 0
        {
            previous.insert(step.id.clone(), call.returned);
        }
        result.status = ExecutionStatus::Returned;
        if let Some(returned) = checkpoint_returned {
            let mut checkpoint_previous = previous.clone();
            checkpoint_previous
                .entry(step.id.clone())
                .or_insert_with(|| returned.clone());
            captured_checkpoints.push(StatefulExecutionCheckpoint::new(
                corpus_name,
                case,
                session_scope.cloned(),
                index + 1,
                checkpoint_previous,
                result.observations.clone(),
                returned,
                result.steps,
                result.calls,
                result.program_identity.clone(),
                result.program_fingerprint.clone(),
            ));
        }
    }
    if result.calls > case.maximum_calls {
        result.status = ExecutionStatus::BudgetExhausted;
        result.failure = Some(ExecutionFailure {
            identity: None,
            reason: "stateful call bound was exceeded".to_owned(),
        });
    }
    result.seal();
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatefulExecutionMismatch {
    pub case_id: String,
    pub baseline: StatefulExecutionResult,
    pub candidate: StatefulExecutionResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatefulExecutionComparison {
    pub schema_version: String,
    pub status: ComparisonStatus,
    pub corpus_name: String,
    pub corpus_size: usize,
    pub matching_cases: usize,
    pub mismatching_cases: usize,
    pub mismatches: Vec<StatefulExecutionMismatch>,
    pub interpretation: String,
}

pub fn compare_stateful_results(
    corpus_name: impl Into<String>,
    baseline: &[StatefulExecutionResult],
    candidate: &[StatefulExecutionResult],
) -> StatefulExecutionComparison {
    let corpus_name = corpus_name.into();
    if baseline.len() != candidate.len()
        || baseline.is_empty()
        || baseline.iter().any(|result| !result.identity_is_valid())
        || candidate.iter().any(|result| !result.identity_is_valid())
    {
        return StatefulExecutionComparison {
            schema_version: STATEFUL_EXECUTION_COMPARISON_SCHEMA_VERSION.to_owned(),
            status: ComparisonStatus::InvalidInput,
            corpus_name,
            corpus_size: baseline.len().max(candidate.len()),
            matching_cases: 0,
            mismatching_cases: 0,
            mismatches: Vec::new(),
            interpretation: "stateful comparison refused invalid or incomplete trace evidence"
                .to_owned(),
        };
    }
    let mut matching_cases = 0;
    let mut mismatches = Vec::new();
    for left in baseline {
        let Some(right) = candidate.iter().find(|right| right.case_id == left.case_id) else {
            mismatches.push(StatefulExecutionMismatch {
                case_id: left.case_id.clone(),
                baseline: left.clone(),
                candidate: StatefulExecutionResult {
                    schema_version: STATEFUL_EXECUTION_SCHEMA_VERSION.to_owned(),
                    case_id: left.case_id.clone(),
                    corpus_identity: String::new(),
                    trace_identity: SemanticId(String::new()),
                    status: ExecutionStatus::InvalidRequest,
                    observations: Vec::new(),
                    returned: Vec::new(),
                    steps: 0,
                    calls: 0,
                    failure: None,
                    program_identity: None,
                    program_fingerprint: None,
                    backend_identity: None,
                    artifact_identity: None,
                },
            });
            continue;
        };
        let equivalent = left.corpus_identity == right.corpus_identity
            && left.status == right.status
            && left.returned == right.returned
            && left
                .observations
                .iter()
                .map(|observation| {
                    (
                        &observation.step_id,
                        observation.status,
                        &observation.returned_digest,
                        &observation.effects,
                    )
                })
                .eq(right.observations.iter().map(|observation| {
                    (
                        &observation.step_id,
                        observation.status,
                        &observation.returned_digest,
                        &observation.effects,
                    )
                }));
        if equivalent {
            matching_cases += 1;
        } else if mismatches.is_empty() {
            mismatches.push(StatefulExecutionMismatch {
                case_id: left.case_id.clone(),
                baseline: left.clone(),
                candidate: right.clone(),
            });
        }
    }
    let mismatching_cases = baseline.len() - matching_cases;
    StatefulExecutionComparison {
        schema_version: STATEFUL_EXECUTION_COMPARISON_SCHEMA_VERSION.to_owned(),
        status: if mismatching_cases == 0 {
            ComparisonStatus::EquivalentOverCorpus
        } else {
            ComparisonStatus::MismatchDetected
        },
        corpus_name,
        corpus_size: baseline.len(),
        matching_cases,
        mismatching_cases,
        mismatches,
        interpretation: "stateful agreement is bounded logical trace evidence, not universal semantic equivalence or compiler correctness".to_owned(),
    }
}

fn stateful_trace_identity(result: &StatefulExecutionResult) -> SemanticId {
    let material = (
        &result.schema_version,
        &result.case_id,
        &result.corpus_identity,
        &result.status,
        &result.observations,
        &result.returned,
        result.steps,
        result.calls,
        &result.program_identity,
        &result.program_fingerprint,
        &result.backend_identity,
        &result.artifact_identity,
    );
    let canonical = canonical_json_value(&material).expect("stateful trace is serializable");
    SemanticId(format!(
        "mncs:language:execution-trace:{}",
        sha256_hex(canonical.as_bytes())
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionCorpus {
    pub schema_version: String,
    pub name: String,
    pub cases: Vec<ExecutionCase>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<ExecutionProperty>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stateful_cases: Vec<StatefulExecutionCase>,
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
    let Some(function) = program.functions.iter().find(|function| {
        function.name == request.target.function
            && function.identity_namespace(&program.module) == request.target.module
    }) else {
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
                    fields: fields.into(),
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
                    values: element_values.into(),
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
                ExecutionValue::Vector {
                    values: vector.into(),
                },
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
                    values: vec![value; *lanes as usize].into(),
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
            let mut updated = source.as_ref().clone();
            updated[index as usize] = element;
            values.insert(
                operation.results[0].id.clone(),
                ExecutionValue::Vector {
                    values: updated.into(),
                },
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
                ExecutionValue::Vector {
                    values: produced.into(),
                },
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
                ExecutionValue::Mask {
                    lanes: lanes.into(),
                },
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
                .collect::<Vec<_>>()
                .into();
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
                    lanes: lanes.iter().map(|lane| !lane).collect::<Vec<_>>().into(),
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
                    .collect::<Vec<_>>()
                    .into();
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
                ExecutionValue::Sequence {
                    values: updated.into(),
                },
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
                    values: source[start as usize..end as usize].to_vec().into(),
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
                    payload: payload.into(),
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
                target: {
                    let target = program.functions.iter().find(|candidate| {
                        function_id(
                            candidate.identity_namespace(&program.module),
                            &candidate.name,
                        ) == *function
                    });
                    ExecutionTarget {
                        module: target
                            .map(|candidate| {
                                candidate.identity_namespace(&program.module).to_owned()
                            })
                            .unwrap_or_else(|| request.target.module.clone()),
                        function: target
                            .map(|candidate| candidate.name.clone())
                            .unwrap_or_else(|| {
                                function_name
                                    .rsplit('.')
                                    .next()
                                    .unwrap_or(function_name)
                                    .to_owned()
                            }),
                    }
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
        (ExecutionValue::Vector { values }, BodyType::Vector { element, lanes }) => {
            values.len() == *lanes as usize
                && values
                    .iter()
                    .all(|lane| value_matches_type(program, lane, element))
        }
        (ExecutionValue::Mask { lanes: bits }, BodyType::Mask { lanes }) => {
            bits.len() == *lanes as usize
        }
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
    if let Some(finite) = program
        .finite_types
        .iter()
        .find(|decl| decl.identity.0 == name)
    {
        return value_matches_type(
            program,
            value,
            &BodyType::Finite {
                identity: finite.identity.clone(),
                name: finite.name.clone(),
            },
        );
    }
    if let Some(record) = program
        .record_types
        .iter()
        .find(|decl| decl.identity.0 == name)
    {
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

fn normalize_sequence(
    program: &Program,
    values: &[ExecutionValue],
    element_type: &BodyType,
) -> Option<ExecutionValue> {
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        normalized.push(normalize_value(program, value, element_type)?);
    }
    Some(ExecutionValue::Sequence {
        values: normalized.into(),
    })
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
    fn arc_backed_aggregate_values_preserve_json_round_trip_and_equality() {
        let integer = ExecutionValue::Integer {
            value: 7,
            ty: IntegerType {
                bits: 32,
                signed: true,
            },
        };
        let sequence = ExecutionValue::Sequence {
            values: Arc::new(vec![
                integer.clone(),
                ExecutionValue::Boolean { value: true },
            ]),
        };
        let record = ExecutionValue::Record {
            type_identity: SemanticId("mncs:test:record".to_owned()),
            name: "Pair".to_owned(),
            fields: Arc::new(vec![
                ("left".to_owned(), integer.clone()),
                ("items".to_owned(), sequence.clone()),
            ]),
        };
        let values = vec![
            ExecutionValue::Finite {
                type_identity: SemanticId("mncs:test:finite".to_owned()),
                variant_identity: SemanticId("mncs:test:finite:Some".to_owned()),
                discriminant: 1,
                payload: Arc::new(vec![("pair".to_owned(), record)]),
            },
            sequence,
            ExecutionValue::Vector {
                values: Arc::new(vec![integer]),
            },
            ExecutionValue::Mask {
                lanes: Arc::new(vec![true, false, true]),
            },
        ];
        let encoded = serde_json::to_string(&values).expect("aggregate JSON");
        let decoded: Vec<ExecutionValue> = serde_json::from_str(&encoded).expect("round trip");
        assert_eq!(decoded, values);
        assert_eq!(
            serde_json::to_string(&decoded).expect("canonical JSON"),
            encoded
        );
    }

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
                binding_table: None,
                generic_specializations: Vec::new(),
                functions: Vec::new(),
            },
            &FunctionBody {
                schema_version: "0.2".to_owned(),
                entry: "b".to_owned(),
                parameters: Vec::new(),
                generic_params: Vec::new(),
                cycle_policy: crate::BodyCyclePolicy::Legacy,
                bounded_iterations: Vec::new(),
                blocks: Vec::new(),
            },
            &Function {
                home_module: None,
                generic_params: Vec::new(),
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

    #[test]
    fn stateful_trace_threads_previous_logical_results_and_hashes_every_transition() {
        let integer = |value| ExecutionValue::Integer {
            value,
            ty: IntegerType {
                bits: 64,
                signed: true,
            },
        };
        let case = StatefulExecutionCase {
            id: "increment-sequence".to_owned(),
            steps: vec![
                StatefulExecutionStep {
                    id: "init".to_owned(),
                    target: ExecutionTarget {
                        module: "m".to_owned(),
                        function: "init".to_owned(),
                    },
                    arguments: Vec::new(),
                    step_budget: 4,
                    policy: ExecutionPolicy::default(),
                    expected: None,
                    expected_status: Some(ExecutionStatus::Returned),
                    observe_returned: false,
                },
                StatefulExecutionStep {
                    id: "next".to_owned(),
                    target: ExecutionTarget {
                        module: "m".to_owned(),
                        function: "next".to_owned(),
                    },
                    arguments: vec![StatefulArgument::PreviousResult {
                        step_id: "init".to_owned(),
                        result_index: 0,
                    }],
                    step_budget: 4,
                    policy: ExecutionPolicy::default(),
                    expected: None,
                    expected_status: Some(ExecutionStatus::Returned),
                    observe_returned: true,
                },
            ],
            maximum_calls: 2,
            maximum_total_steps: Some(8),
            expected_final: Some(vec![integer(2)]),
            expected_final_status: Some(ExecutionStatus::Returned),
        };
        let result = execute_stateful_case("test-corpus", &case, |request| {
            assert_eq!(
                request.target.function,
                if request.arguments.is_empty() {
                    "init"
                } else {
                    "next"
                }
            );
            let value = request
                .arguments
                .first()
                .and_then(|value| match value {
                    ExecutionValue::Integer { value, .. } => Some(*value + 1),
                    _ => None,
                })
                .unwrap_or(1);
            StatefulCallResult {
                status: ExecutionStatus::Returned,
                returned: vec![integer(value)],
                steps: 1,
                effects: Vec::new(),
                failure: None,
                program_identity: Some(SemanticId("program".to_owned())),
                program_fingerprint: Some("fingerprint".to_owned()),
            }
        });
        assert_eq!(result.status, ExecutionStatus::Returned);
        assert_eq!(result.returned, vec![integer(2)]);
        assert_eq!(result.observations.len(), 2);
        assert!(result.observations[0].returned_digest.is_some());
        assert!(result.observations[0].returned.is_none());
        assert!(result.observations[1].returned.is_some());
        assert!(result.identity_is_valid());
        let mut mutant = result.clone();
        mutant.observations[0].returned_digest = Some("mutated-transition".to_owned());
        mutant.seal();
        let comparison = compare_stateful_results("test-corpus", &[result], &[mutant]);
        assert_eq!(comparison.status, ComparisonStatus::MismatchDetected);
        assert_eq!(comparison.mismatching_cases, 1);
    }

    #[test]
    fn stateful_trace_rejects_forward_result_references_before_execution() {
        let case = StatefulExecutionCase {
            id: "forward-reference".to_owned(),
            steps: vec![StatefulExecutionStep {
                id: "first".to_owned(),
                target: ExecutionTarget {
                    module: "m".to_owned(),
                    function: "f".to_owned(),
                },
                arguments: vec![StatefulArgument::PreviousResult {
                    step_id: "later".to_owned(),
                    result_index: 0,
                }],
                step_budget: 1,
                policy: ExecutionPolicy::default(),
                expected: None,
                expected_status: None,
                observe_returned: false,
            }],
            maximum_calls: 1,
            maximum_total_steps: None,
            expected_final: None,
            expected_final_status: None,
        };
        let mut called = false;
        let result = execute_stateful_case("test-corpus", &case, |_| {
            called = true;
            panic!("invalid stateful case must not reach the backend");
        });
        assert_eq!(result.status, ExecutionStatus::InvalidRequest);
        assert!(!called);
        assert!(result.failure.is_some());
        assert!(result.identity_is_valid());
    }

    #[test]
    fn stateful_prefix_checkpoint_reuses_only_the_verified_prefix() {
        let integer = |value| ExecutionValue::Integer {
            value,
            ty: IntegerType {
                bits: 64,
                signed: true,
            },
        };
        let previous = |step_id: &str| StatefulArgument::PreviousResult {
            step_id: step_id.to_owned(),
            result_index: 0,
        };
        let step =
            |id: &str, function: &str, arguments: Vec<StatefulArgument>| StatefulExecutionStep {
                id: id.to_owned(),
                target: ExecutionTarget {
                    module: "m".to_owned(),
                    function: function.to_owned(),
                },
                arguments,
                step_budget: 4,
                policy: ExecutionPolicy::default(),
                expected: None,
                expected_status: Some(ExecutionStatus::Returned),
                observe_returned: false,
            };
        let case = StatefulExecutionCase {
            id: "checkpointed-sequence".to_owned(),
            steps: vec![
                step("init", "init", Vec::new()),
                step("next", "next", vec![previous("init")]),
                step("finish", "finish", vec![previous("next")]),
            ],
            maximum_calls: 3,
            maximum_total_steps: Some(12),
            expected_final: None,
            expected_final_status: Some(ExecutionStatus::Returned),
        };
        let mut first_requests = 0;
        let (_first, checkpoints) =
            execute_stateful_case_with_checkpoint("test-corpus", &case, None, &[1], |request| {
                first_requests += 1;
                let value = request
                    .arguments
                    .first()
                    .and_then(|value| match value {
                        ExecutionValue::Integer { value, .. } => Some(*value + 1),
                        _ => None,
                    })
                    .unwrap_or(1);
                StatefulCallResult {
                    status: ExecutionStatus::Returned,
                    returned: vec![integer(value)],
                    steps: 1,
                    effects: Vec::new(),
                    failure: None,
                    program_identity: Some(SemanticId("program".to_owned())),
                    program_fingerprint: Some("fingerprint".to_owned()),
                }
            });
        assert_eq!(first_requests, 3);
        assert_eq!(checkpoints.len(), 1);
        assert!(checkpoints[0].identity_is_valid_for("test-corpus", &case));

        let mut resumed_requests = 0;
        let (resumed, no_checkpoints) = execute_stateful_case_with_checkpoint(
            "test-corpus",
            &case,
            checkpoints.first(),
            &[],
            |request| {
                resumed_requests += 1;
                let value = request
                    .arguments
                    .first()
                    .and_then(|value| match value {
                        ExecutionValue::Integer { value, .. } => Some(*value + 1),
                        _ => None,
                    })
                    .unwrap_or(1);
                StatefulCallResult {
                    status: ExecutionStatus::Returned,
                    returned: vec![integer(value)],
                    steps: 1,
                    effects: Vec::new(),
                    failure: None,
                    program_identity: Some(SemanticId("program".to_owned())),
                    program_fingerprint: Some("fingerprint".to_owned()),
                }
            },
        );
        assert_eq!(resumed_requests, 2);
        assert!(no_checkpoints.is_empty());
        assert_eq!(resumed.returned, vec![integer(3)]);
        assert!(resumed.identity_is_valid());

        let scope_a = SemanticId("mncs:test:artifact:a".to_owned());
        let scope_b = SemanticId("mncs:test:artifact:b".to_owned());
        let (_scoped, scoped_checkpoints) = execute_stateful_case_with_checkpoint_scoped(
            Some(&scope_a),
            "test-corpus",
            &case,
            None,
            &[1],
            |_request| StatefulCallResult {
                status: ExecutionStatus::Returned,
                returned: vec![integer(1)],
                steps: 1,
                effects: Vec::new(),
                failure: None,
                program_identity: Some(SemanticId("program".to_owned())),
                program_fingerprint: Some("fingerprint".to_owned()),
            },
        );
        let scoped_checkpoint = scoped_checkpoints.first().expect("scoped checkpoint");
        assert!(scoped_checkpoint.identity_is_valid_for_scope(
            "test-corpus",
            &case,
            Some(&scope_a)
        ));
        assert!(!scoped_checkpoint.identity_is_valid_for_scope(
            "test-corpus",
            &case,
            Some(&scope_b)
        ));

        // The exact prefix may be shared with a sibling whose suffix
        // diverges, but state, policy, bounds, and the checkpoint seal may
        // not be laundered across mutations.
        let mut sibling = case.clone();
        sibling.steps[2].target.function = "different-finish".to_owned();
        assert!(scoped_checkpoint.identity_is_valid_for_scope(
            "test-corpus",
            &sibling,
            Some(&scope_a)
        ));
        let mut changed_policy = case.clone();
        changed_policy.steps[0].step_budget += 1;
        assert!(!scoped_checkpoint.identity_is_valid_for_scope(
            "test-corpus",
            &changed_policy,
            Some(&scope_a)
        ));
        let mut changed_result_index = case.clone();
        changed_result_index.steps[1].arguments = vec![StatefulArgument::PreviousResult {
            step_id: "init".to_owned(),
            result_index: 1,
        }];
        assert!(!scoped_checkpoint.identity_is_valid_for_scope(
            "test-corpus",
            &changed_result_index,
            Some(&scope_a)
        ));
        let mut out_of_range = scoped_checkpoint.clone();
        out_of_range.next_step_index = case.steps.len() + 1;
        assert!(!out_of_range.identity_is_valid_for_scope("test-corpus", &case, Some(&scope_a)));
        let mut changed_state = scoped_checkpoint.clone();
        changed_state.returned.push(integer(99));
        assert!(!changed_state.identity_is_valid_for_scope("test-corpus", &case, Some(&scope_a)));
    }
}
