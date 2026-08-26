use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::identity::{block_id, body_id, operation_id, value_id};
use crate::{
    ArithmeticIntent, Diagnostic, Effect, Fact, FailureMode, Function, IntegerType, Intent,
    MachinePreference, Obligation, Program, Requirement, SemanticId,
};

pub const EXECUTABLE_BODY_SCHEMA_VERSION: &str = "0.2";
pub const SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND: u32 = 32;
/// Inclusive upper bound for sequence lengths and view capacities
/// (Source Profile 0.7). Bounds are semantic facts carried by types, so they
/// must stay small enough to remain machine-checkable everywhere.
pub const MAX_SEQUENCE_BOUND: u32 = 64;

/// The declared length facts of a bounded sequence type. `Exact` sequences
/// carry their length statically; `UpTo` views carry a maximum and a runtime
/// length observation. The bound is part of logical type identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequenceBound {
    Exact(u32),
    UpTo(u32),
}

impl SequenceBound {
    /// The static ceiling this bound contributes to traversal step counts.
    pub fn ceiling(self) -> u32 {
        match self {
            Self::Exact(length) | Self::UpTo(length) => length,
        }
    }

    pub fn canonical_text(self) -> String {
        match self {
            Self::Exact(length) => format!("{length}"),
            Self::UpTo(capacity) => format!("up_to {capacity}"),
        }
    }
}

/// How an index operation establishes that it is within bounds. This is
/// first-class machine knowledge: realizations and downstream tooling must
/// not have to rediscover it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundsEvidence {
    /// A literal index inside an exact declared bound; validity is static.
    StaticExact,
    /// The index is the binding of an enclosing bounded traversal over this
    /// exact sequence; traversal semantics discharge the check.
    TraversalDomain,
    /// Validity is not established statically; realization performs a
    /// deterministic check whose failure is an explicit runtime failure and
    /// whose obligation stays UNKNOWN until discharged.
    RuntimeChecked { failure: FailureMode },
}

/// What domain a bounded iteration counts over. `Attempts` is the Profile 0.4
/// counted form; `OverSequence` traverses a bounded sequence's index domain,
/// so its step count derives from the sequence's declared bound.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IterationDomain {
    #[default]
    Attempts,
    OverSequence {
        element_type: Box<BodyType>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionBody {
    pub schema_version: String,
    pub entry: String,
    pub parameters: Vec<BodyParameter>,
    #[serde(default, skip_serializing_if = "BodyCyclePolicy::is_legacy")]
    pub cycle_policy: BodyCyclePolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bounded_iterations: Vec<BodyBoundedIteration>,
    pub blocks: Vec<BodyBlock>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyCyclePolicy {
    #[default]
    Legacy,
    BoundedIterationOnly,
}

impl BodyCyclePolicy {
    pub(crate) fn is_legacy(&self) -> bool {
        *self == Self::Legacy
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyBoundedIteration {
    pub id: String,
    pub bound: u32,
    pub state_name: String,
    pub state_type: BodyType,
    /// What the iteration counts over. Absent (legacy `Attempts`) forms
    /// serialize unchanged; Profile 0.7 sequence traversal records its domain.
    #[serde(default)]
    pub domain: IterationDomain,
    pub initial_value: String,
    pub header_state: String,
    pub header_counter: String,
    pub preheader: String,
    pub header: String,
    pub body_entry: String,
    pub backedge: String,
    pub exit: String,
    pub exit_state: String,
    pub body_blocks: Vec<String>,
    pub callees: Vec<SemanticId>,
    pub required_capabilities: Vec<String>,
    pub completion_modes: Vec<BoundedIterationCompletion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundedIterationCompletion {
    EarlyReturn,
    Exhausted,
    Failure,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyParameter {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: BodyType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyValue {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: BodyType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyType {
    Named(String),
    Integer(IntegerType),
    /// A byte-oriented 8-bit unsigned logical value (Profile 0.7). Distinct
    /// from `u8`: bytes support byte-appropriate operations only.
    Byte,
    /// A bounded sequence with explicit length facts (Profile 0.7). The
    /// logical value is the ordered element values plus the declared bound;
    /// no storage shape participates in meaning.
    Sequence {
        element: Box<BodyType>,
        bound: SequenceBound,
    },
    Finite {
        identity: SemanticId,
        name: String,
    },
    Record {
        identity: SemanticId,
        name: String,
    },
}

impl BodyType {
    pub fn from_semantic_name(name: &str) -> Self {
        if let Some(sequence) = Self::parse_sequence_name(name) {
            return sequence;
        }
        if name == "byte" {
            return Self::Byte;
        }
        let (signed, digits) = if let Some(digits) = name.strip_prefix('i') {
            (true, digits)
        } else if let Some(digits) = name.strip_prefix('u') {
            (false, digits)
        } else {
            return Self::Named(name.to_owned());
        };
        match digits.parse::<u16>() {
            Ok(bits) if bits > 0 => Self::Integer(IntegerType { bits, signed }),
            _ => Self::Named(name.to_owned()),
        }
    }

    /// Parse a canonical bounded-sequence spelling `[E; N]` or
    /// `[E; up_to M]`. Only canonical spellings parse; anything else stays
    /// an ordinary named type so downstream diagnostics stay truthful.
    fn parse_sequence_name(name: &str) -> Option<Self> {
        let inner = name.strip_prefix('[')?.strip_suffix(']')?;
        let separator = inner.rfind(';')?;
        let (element_text, bound_text) = (inner[..separator].trim(), inner[separator + 1..].trim());
        let bound = if let Some(capacity) = bound_text.strip_prefix("up_to") {
            let capacity = capacity.trim().parse::<u32>().ok()?;
            if capacity > MAX_SEQUENCE_BOUND {
                return None;
            }
            SequenceBound::UpTo(capacity)
        } else {
            let length = bound_text.parse::<u32>().ok()?;
            if length > MAX_SEQUENCE_BOUND {
                return None;
            }
            SequenceBound::Exact(length)
        };
        let element = Box::new(Self::from_semantic_name(element_text));
        match &*element {
            // Element types must be fully resolved; unresolved named types
            // cannot appear inside a canonical sequence spelling.
            Self::Named(_) => None,
            _ => Some(Self::Sequence { element, bound }),
        }
    }

    pub fn semantic_name(&self) -> String {
        match self {
            Self::Named(name) => name.clone(),
            Self::Integer(integer) => {
                format!("{}{}", if integer.signed { 'i' } else { 'u' }, integer.bits)
            }
            Self::Byte => "byte".to_owned(),
            Self::Sequence { element, bound } => {
                format!("[{}; {}]", element.semantic_name(), bound.canonical_text())
            }
            Self::Finite { name, .. } => name.clone(),
            Self::Record { name, .. } => name.clone(),
        }
    }

    fn is_boolean(&self) -> bool {
        matches!(self, Self::Named(name) if name == "bool")
            || matches!(self, Self::Integer(integer) if integer.bits == 1 && !integer.signed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyBlock {
    pub id: String,
    #[serde(default)]
    pub parameters: Vec<BodyValue>,
    pub operations: Vec<BodyOperation>,
    pub terminator: BodyTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyOperation {
    pub id: String,
    pub kind: BodyOperationKind,
    #[serde(default)]
    pub operands: Vec<String>,
    #[serde(default)]
    pub results: Vec<BodyValue>,
    #[serde(default)]
    pub contracts: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub machine_intent: Option<MachineIntentSpec>,
    #[serde(default)]
    pub lowering: Option<LoweringEnvelope>,
    #[serde(default)]
    pub portability: Option<PortabilityEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyOperationKind {
    Constant {
        value: i128,
        #[serde(rename = "type")]
        ty: BodyType,
    },
    Integer {
        operator: String,
        operand_type: IntegerType,
        intent: ArithmeticIntent,
    },
    IntegerCompare {
        predicate: String,
        operand_type: IntegerType,
    },
    /// Strict boolean conjunction/disjunction (Profile 0.6). Both operands
    /// are total values; evaluation is not short-circuited.
    BooleanOp {
        operator: String,
    },
    /// Byte-oriented bitwise operation (Profile 0.7): `and` | `or` | `xor`
    /// over two byte operands, producing a byte. Total.
    ByteBitwise {
        operator: String,
    },
    /// Byte shift (Profile 0.7): `shl` | `shr`. The left operand is a byte,
    /// the right operand a u64 count taken modulo 8 (logical shifts). Total.
    ByteShift {
        operator: String,
    },
    /// Byte comparison (Profile 0.7): unsigned 8-bit ordering/equality.
    ByteCompare {
        predicate: String,
    },
    /// Semantic conditional selection (Profile 0.8). Operand 0 is a bool
    /// condition; operands 1 and 2 are the candidate values of the declared
    /// operand type. Both candidates are *values* of one pure selection
    /// operation: neither is "executed after" the other, and no control-flow
    /// divergence between them is part of the meaning. Total by definition.
    /// The operation carries machine intent requesting that realization
    /// preserve selection without introducing data-dependent branch
    /// divergence; whether a backend met that request is decided by
    /// structural evidence, never assumed.
    Select {
        operand_type: Box<BodyType>,
    },
    /// Total functional sequence update (Profile 0.8): produce a new
    /// sequence equal to operand 0 except the element at operand 1's index,
    /// which becomes operand 2. Value semantics: the source sequence value
    /// is unchanged and no aliasing is created. Restricted to exact bounds
    /// this tranche; views refuse with a coded diagnostic. Index validity
    /// follows the same three-state evidence model as projection.
    SequenceReplace {
        element_type: Box<BodyType>,
        bound: SequenceBound,
        evidence: BoundsEvidence,
    },
    /// Explicit total scalar conversion (Profile 0.7). Truncation drops high
    /// bits; widening extends by the *source* signedness (`byte` zero-extends).
    /// Total by definition; no obligation survives.
    Convert {
        from: BodyType,
        to: BodyType,
    },
    /// Construct an exact-length sequence from exactly `length` element
    /// values in index order (Profile 0.7).
    SequenceConstruct {
        element_type: Box<BodyType>,
        length: u32,
    },
    /// Project the element at the index operand (Profile 0.7). Operand 0 is
    /// the sequence, operand 1 the u64 index. The evidence records how bounds
    /// safety is established for this occurrence.
    SequenceProject {
        bound: SequenceBound,
        evidence: BoundsEvidence,
    },
    /// Observe a sequence's length as u64 (Profile 0.7). Constant for exact
    /// bounds; the runtime length observation for views.
    SequenceLength {
        bound: SequenceBound,
    },
    /// Derive a bounded view from a sequence over a checked half-open range
    /// (Profile 0.7). Operands: source sequence, u64 start, u64 end. Checked
    /// semantics require `start <= end`, `end <= len(source)`, and
    /// `end - start <= view_capacity`; violations are explicit runtime
    /// failures with a retained range-validity obligation.
    ViewConstruct {
        source_bound: SequenceBound,
        view_bound: SequenceBound,
    },
    FiniteConstruct {
        type_identity: SemanticId,
        variant_identity: SemanticId,
        discriminant: u32,
        /// Canonical (sorted) payload field names for payload-bearing
        /// variants (Profile 0.6). `operands[i]` carries the value of
        /// `payload_fields[i]`. Empty (and serialized as absent) for
        /// payload-free variants.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        payload_fields: Vec<String>,
    },
    FiniteIsVariant {
        type_identity: SemanticId,
        variant_identity: SemanticId,
        discriminant: u32,
    },
    /// Project one payload field out of a finite value whose variant carries
    /// a payload (Profile 0.6). The result type is the declared field type;
    /// the operand must be a finite value of the declared type and variant.
    FinitePayloadProject {
        type_identity: SemanticId,
        variant_identity: SemanticId,
        discriminant: u32,
        field: String,
    },
    RecordConstruct {
        type_identity: SemanticId,
        /// Canonical (sorted) field names for the constructed value.
        field_names: Vec<String>,
    },
    RecordProject {
        type_identity: SemanticId,
        field: String,
    },
    Call {
        function: SemanticId,
        function_name: String,
        required_capabilities: Vec<String>,
        effects: Vec<Effect>,
    },
    Effect {
        effect: Effect,
        capability: String,
    },
    RuntimeCheck {
        obligation: SemanticId,
        fact: Fact,
        failure: FailureMode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyTerminator {
    Return {
        values: Vec<String>,
    },
    Branch {
        target: String,
        #[serde(default)]
        arguments: Vec<String>,
    },
    ConditionalBranch {
        condition: String,
        then_target: String,
        #[serde(default)]
        then_arguments: Vec<String>,
        else_target: String,
        #[serde(default)]
        else_arguments: Vec<String>,
    },
    Failure {
        mode: FailureMode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineIntentSpec {
    pub intent: Intent,
    #[serde(default)]
    pub preferences: Vec<MachinePreference>,
    #[serde(default)]
    pub facts: Vec<Fact>,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
    #[serde(default)]
    pub obligations: Vec<Obligation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealizationClass {
    PortableReference,
    RuntimeChecked,
    Specialized(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweringEnvelope {
    pub schema_version: String,
    #[serde(default)]
    pub permitted_realizations: Vec<RealizationClass>,
    #[serde(default)]
    pub prohibited_transformations: Vec<String>,
    #[serde(default)]
    pub required_obligations: Vec<SemanticId>,
    pub fallback: Option<RealizationClass>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortabilityEnvelope {
    pub schema_version: String,
    pub target: PortabilityTarget,
    #[serde(default)]
    pub required_features: Vec<String>,
    #[serde(default)]
    pub integer_widths: Vec<u16>,
    pub abi: Option<String>,
    pub fallback: Option<RealizationClass>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortabilityTarget {
    Portable,
    ArchitectureFamily(String),
}

impl FunctionBody {
    pub fn canonical_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.canonicalized())
    }

    pub fn fingerprint(&self) -> Result<String, serde_json::Error> {
        Ok(crate::canonical::sha256_hex(
            self.canonical_json()?.as_bytes(),
        ))
    }

    pub fn identity(&self, module: &str, function: &str) -> SemanticId {
        body_id(module, function)
    }

    pub fn block_identity(&self, module: &str, function: &str, block: &str) -> SemanticId {
        block_id(module, function, block)
    }

    pub fn validate(
        &self,
        program: &Program,
        function: &Function,
        function_path: &str,
        errors: &mut Vec<Diagnostic>,
    ) {
        if self.schema_version != EXECUTABLE_BODY_SCHEMA_VERSION {
            errors.push(body_diagnostic(
                "MNB001",
                format!("{function_path}.body.schema_version"),
                format!(
                    "unsupported executable body schema {:?}; expected {:?}",
                    self.schema_version, EXECUTABLE_BODY_SCHEMA_VERSION
                ),
            ));
        }
        let mut values = BTreeMap::<String, BodyType>::new();
        let mut parameter_ids = BTreeSet::new();
        if self.parameters.len() != function.inputs.len() {
            errors.push(body_diagnostic(
                "MNB002",
                format!("{function_path}.body.parameters"),
                "body parameters must match the enclosing function inputs",
            ));
        }
        for (index, parameter) in self.parameters.iter().enumerate() {
            let path = format!("{function_path}.body.parameters[{index}]");
            if parameter.id.trim().is_empty() || parameter.name.trim().is_empty() {
                errors.push(body_diagnostic(
                    "MNB003",
                    path.clone(),
                    "body parameter id and name must not be empty",
                ));
            }
            if !parameter_ids.insert(parameter.id.clone()) {
                errors.push(body_diagnostic(
                    "MNB004",
                    format!("{path}.id"),
                    format!("duplicate body value identity {:?}", parameter.id),
                ));
            }
            if let Some(input) = function.inputs.get(index) {
                if parameter.name != input.name
                    || parameter.ty != semantic_body_type(program, &input.value_type)
                {
                    errors.push(body_diagnostic(
                        "MNB005",
                        path,
                        "body parameter name or type does not match the function input",
                    ));
                }
            }
            values.insert(parameter.id.clone(), parameter.ty.clone());
        }

        let block_ids: BTreeSet<_> = self.blocks.iter().map(|block| block.id.clone()).collect();
        if self.entry.trim().is_empty() || !block_ids.contains(&self.entry) {
            errors.push(body_diagnostic(
                "MNB006",
                format!("{function_path}.body.entry"),
                format!("body entry block {:?} does not exist", self.entry),
            ));
        }
        if block_ids.len() != self.blocks.len() {
            errors.push(body_diagnostic(
                "MNB007",
                format!("{function_path}.body.blocks"),
                "body block identities must be unique",
            ));
        }
        let cfg = crate::Cfg::from_body(self, self.identity(&program.module, &function.name));
        for block in &cfg.unreachable {
            errors.push(body_diagnostic(
                "MNB038",
                format!("{function_path}.body.blocks.{block}"),
                "unreachable body blocks are not supported by this executable subset",
            ));
        }
        validate_bounded_iterations(self, function, function_path, &cfg, errors);

        let mut operation_ids = BTreeSet::new();
        let mut definitions = BTreeMap::<String, (Option<String>, BodyType)>::new();
        for parameter in &self.parameters {
            definitions.insert(parameter.id.clone(), (None, parameter.ty.clone()));
        }
        for block in &self.blocks {
            for parameter in &block.parameters {
                if definitions
                    .insert(
                        parameter.id.clone(),
                        (Some(block.id.clone()), parameter.ty.clone()),
                    )
                    .is_some()
                {
                    errors.push(body_diagnostic(
                        "MNB008",
                        format!("{function_path}.body.blocks.{}.parameters", block.id),
                        format!("duplicate body value identity {:?}", parameter.id),
                    ));
                }
            }
            for operation in &block.operations {
                for result in &operation.results {
                    if definitions
                        .insert(
                            result.id.clone(),
                            (Some(block.id.clone()), result.ty.clone()),
                        )
                        .is_some()
                    {
                        errors.push(body_diagnostic(
                            "MNB011",
                            format!("{function_path}.body.blocks.{}.operations", block.id),
                            format!("duplicate body value identity {:?}", result.id),
                        ));
                    }
                }
            }
        }
        let block_parameters = self
            .blocks
            .iter()
            .map(|block| (block.id.clone(), block.parameters.clone()))
            .collect::<BTreeMap<_, _>>();

        for (block_index, block) in self.blocks.iter().enumerate() {
            let block_path = format!("{function_path}.body.blocks[{block_index}]");
            let mut available = values.clone();
            for parameter in &block.parameters {
                available.insert(parameter.id.clone(), parameter.ty.clone());
            }
            for (value, (definition_block, ty)) in &definitions {
                if definition_block.as_deref().is_some_and(|definition| {
                    definition != block.id && cfg.dominates(definition, &block.id)
                }) {
                    available.insert(value.clone(), ty.clone());
                }
            }
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let operation_path = format!("{block_path}.operations[{operation_index}]");
                if operation.id.trim().is_empty() || !operation_ids.insert(operation.id.clone()) {
                    errors.push(body_diagnostic(
                        "MNB009",
                        format!("{operation_path}.id"),
                        format!(
                            "operation identity {:?} is empty or duplicated",
                            operation.id
                        ),
                    ));
                }
                for operand in &operation.operands {
                    if !available.contains_key(operand) {
                        errors.push(body_diagnostic(
                            "MNB010",
                            format!("{operation_path}.operands"),
                            format!("value {:?} is not defined before use", operand),
                        ));
                    }
                }
                validate_operation(
                    program,
                    function,
                    operation,
                    &operation_path,
                    &available,
                    errors,
                );
                for result in &operation.results {
                    available.insert(result.id.clone(), result.ty.clone());
                }
            }
            validate_terminator(
                &block.terminator,
                &block_path,
                &available,
                &block_ids,
                &block_parameters,
                errors,
            );
            if matches!(block.terminator, BodyTerminator::Return { .. }) {
                validate_return_types(
                    &block.terminator,
                    program,
                    function,
                    &available,
                    &block_path,
                    errors,
                );
            }
        }
    }

    fn canonicalized(&self) -> Self {
        let mut canonical = self.clone();
        canonical
            .bounded_iterations
            .sort_by(|left, right| left.id.cmp(&right.id));
        for iteration in &mut canonical.bounded_iterations {
            iteration.body_blocks.sort();
            iteration.body_blocks.dedup();
            iteration.callees.sort();
            iteration.callees.dedup();
            iteration.required_capabilities.sort();
            iteration.required_capabilities.dedup();
            iteration.completion_modes.sort();
            iteration.completion_modes.dedup();
        }
        for block in &mut canonical.blocks {
            for operation in &mut block.operations {
                operation.contracts.sort();
                operation.contracts.dedup();
                operation.assumptions.sort();
                operation.assumptions.dedup();
                if let Some(machine_intent) = &mut operation.machine_intent {
                    machine_intent
                        .preferences
                        .sort_by(|left, right| left.identity.cmp(&right.identity));
                    machine_intent
                        .facts
                        .sort_by(|left, right| left.identity.cmp(&right.identity));
                    machine_intent
                        .requirements
                        .sort_by(|left, right| left.identity.cmp(&right.identity));
                    machine_intent
                        .obligations
                        .sort_by(|left, right| left.identity.cmp(&right.identity));
                }
                if let Some(lowering) = &mut operation.lowering {
                    lowering.required_obligations.sort();
                    lowering.prohibited_transformations.sort();
                }
                if let Some(portability) = &mut operation.portability {
                    portability.required_features.sort();
                    portability.integer_widths.sort();
                }
            }
        }
        canonical
    }
}

fn validate_bounded_iterations(
    body: &FunctionBody,
    function: &Function,
    function_path: &str,
    cfg: &crate::Cfg,
    errors: &mut Vec<Diagnostic>,
) {
    let mut ids = BTreeSet::new();
    let declared_backedges = body
        .bounded_iterations
        .iter()
        .map(|iteration| (iteration.backedge.clone(), iteration.header.clone()))
        .collect::<BTreeSet<_>>();
    if body.cycle_policy == BodyCyclePolicy::BoundedIterationOnly {
        for block in &cfg.blocks {
            for target in &block.successors {
                if cfg.dominates(target, &block.id)
                    && !declared_backedges.contains(&(block.id.clone(), target.clone()))
                {
                    errors.push(body_diagnostic(
                        "MNB060",
                        format!("{function_path}.body.blocks.{}.terminator", block.id),
                        "cyclic control flow requires an exact bounded-iteration backedge",
                    ));
                }
            }
        }
    }
    for (index, iteration) in body.bounded_iterations.iter().enumerate() {
        let path = format!("{function_path}.body.bounded_iterations[{index}]");
        if iteration.id.trim().is_empty() || !ids.insert(iteration.id.clone()) {
            errors.push(body_diagnostic(
                "MNB061",
                format!("{path}.id"),
                "bounded iteration identities must be non-empty and unique per function",
            ));
        }
        match &iteration.domain {
            IterationDomain::Attempts => {
                if !(1..=SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND).contains(&iteration.bound) {
                    errors.push(body_diagnostic(
                        "MNB062",
                        format!("{path}.bound"),
                        format!(
                            "bounded iteration count must be between 1 and {}",
                            SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND
                        ),
                    ));
                }
            }
            IterationDomain::OverSequence { element_type } => {
                // Sequence traversal derives its step count from the
                // traversed bound: an empty exact sequence legitimately has
                // bound zero. The ceiling is the profile's declared maximum.
                if iteration.bound > MAX_SEQUENCE_BOUND {
                    errors.push(body_diagnostic(
                        "MNB100",
                        format!("{path}.bound"),
                        format!(
                            "sequence traversal bound must not exceed the profile ceiling {MAX_SEQUENCE_BOUND}"
                        ),
                    ));
                }
                let expected_counter = BodyType::Integer(IntegerType {
                    bits: 64,
                    signed: false,
                });
                if **element_type == expected_counter
                    || matches!(**element_type, BodyType::Named(_))
                {
                    errors.push(body_diagnostic(
                        "MNB101",
                        format!("{path}.domain"),
                        "sequence traversal domain must name a resolvable element type",
                    ));
                }
            }
        }
        let blocks = [
            &iteration.preheader,
            &iteration.header,
            &iteration.body_entry,
            &iteration.backedge,
            &iteration.exit,
        ];
        if blocks.iter().any(|id| cfg.block(id).is_none()) {
            errors.push(body_diagnostic(
                "MNB063",
                path.clone(),
                "bounded iteration references a missing control-flow region block",
            ));
            continue;
        }
        let header = body
            .blocks
            .iter()
            .find(|block| block.id == iteration.header);
        let exit = body.blocks.iter().find(|block| block.id == iteration.exit);
        let Some(header) = header else { continue };
        let Some(exit) = exit else { continue };
        if header.parameters.len() != 2
            || header.parameters[0].id != iteration.header_state
            || header.parameters[0].ty != iteration.state_type
            || header.parameters[1].id != iteration.header_counter
            || header.parameters[1].ty
                != BodyType::Integer(IntegerType {
                    bits: 64,
                    signed: false,
                })
            || exit.parameters.len() != 1
            || exit.parameters[0].id != iteration.exit_state
            || exit.parameters[0].ty != iteration.state_type
        {
            errors.push(body_diagnostic(
                "MNB064",
                path.clone(),
                "bounded iteration carried state/counter parameters are malformed",
            ));
        }
        if !cfg
            .block(&iteration.backedge)
            .is_some_and(|block| block.successors == vec![iteration.header.clone()])
        {
            errors.push(body_diagnostic(
                "MNB065",
                path.clone(),
                "bounded iteration lost its declared backedge",
            ));
        }
        let required = iteration
            .required_capabilities
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let declared = function
            .capabilities
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if !required.is_subset(&declared) {
            errors.push(body_diagnostic(
                "MNB066",
                path.clone(),
                "bounded iteration cannot expand authority across attempts",
            ));
        }
        let modes = iteration
            .completion_modes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if !modes.contains(&BoundedIterationCompletion::Exhausted)
            || !modes.contains(&BoundedIterationCompletion::EarlyReturn)
        {
            errors.push(body_diagnostic(
                "MNB067",
                path,
                "bounded iteration must retain early-return and exhaustion completion modes",
            ));
        }
    }
}

impl BodyOperation {
    pub fn identity(&self, module: &str, function: &str, block: &str) -> SemanticId {
        operation_id(module, function, block, &self.id)
    }

    pub fn result_identity(
        &self,
        module: &str,
        function: &str,
        block: &str,
        result: &str,
    ) -> SemanticId {
        value_id(module, function, block, &self.id, result)
    }

    pub fn machine_intent_identity(
        &self,
        module: &str,
        function: &str,
        block: &str,
    ) -> Option<SemanticId> {
        self.machine_intent
            .as_ref()
            .map(|_| crate::identity::machine_intent_id(module, function, block, &self.id))
    }
}

fn validate_operation(
    program: &Program,
    function: &Function,
    operation: &BodyOperation,
    path: &str,
    available: &BTreeMap<String, BodyType>,
    errors: &mut Vec<Diagnostic>,
) {
    match &operation.kind {
        BodyOperationKind::Constant { ty, .. } => {
            if operation.operands.is_empty() && operation.results.len() == 1 {
                if operation.results[0].ty != *ty {
                    errors.push(body_diagnostic(
                        "MNB013",
                        format!("{path}.results"),
                        "constant result type does not match the constant type",
                    ));
                }
            } else {
                errors.push(body_diagnostic(
                    "MNB014",
                    path.to_owned(),
                    "constant operations require one result and no operands",
                ));
            }
        }
        BodyOperationKind::Integer {
            operator,
            operand_type,
            intent,
        } => {
            if !matches!(
                operator.as_str(),
                "add" | "sub" | "mul" | "div" | "mod" | "and" | "or" | "xor" | "shl" | "shr"
            ) {
                errors.push(body_diagnostic(
                    "MNB015",
                    format!("{path}.kind"),
                    format!("unsupported symbolic integer operator {:?}", operator),
                ));
            }
            // Bitwise and shift operations are total by definition, so they
            // require the wrapping intent: no overflow obligation survives.
            if matches!(operator.as_str(), "and" | "or" | "xor" | "shl" | "shr")
                && !matches!(
                    &operation.kind,
                    BodyOperationKind::Integer {
                        intent: ArithmeticIntent::Wrapping,
                        ..
                    }
                )
            {
                errors.push(body_diagnostic(
                    "MNB043",
                    format!("{path}.kind.intent"),
                    "bitwise and shift integer operations currently require wrapping intent",
                ));
            }
            if operation.operands.len() != 2 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB016",
                    path.to_owned(),
                    "integer operations require two operands and one result",
                ));
            }
            for operand in &operation.operands {
                if available.get(operand) != Some(&BodyType::Integer(*operand_type)) {
                    errors.push(body_diagnostic(
                        "MNB017",
                        format!("{path}.operands"),
                        "integer operand type does not match the operation type",
                    ));
                }
            }
            let result_type = crate::arithmetic_result_type(operator, *operand_type, *intent);
            if result_type.is_none() {
                errors.push(body_diagnostic(
                    "MNB058",
                    format!("{path}.kind.intent"),
                    "widening arithmetic requires an exact wider result; unsigned widening subtraction is unsupported and widths must be <= 126",
                ));
            }
            if operation.results.first().is_some_and(|result| {
                result_type.is_some_and(|ty| result.ty != BodyType::Integer(ty))
            }) {
                errors.push(body_diagnostic(
                    "MNB018",
                    format!("{path}.results"),
                    "integer result type does not match the arithmetic intent result type",
                ));
            }
        }
        BodyOperationKind::IntegerCompare {
            predicate,
            operand_type,
        } => {
            if !matches!(predicate.as_str(), "eq" | "ne" | "lt" | "le" | "gt" | "ge") {
                errors.push(body_diagnostic(
                    "MNB039",
                    format!("{path}.kind.predicate"),
                    format!("unsupported integer comparison predicate {predicate:?}"),
                ));
            }
            if operation.operands.len() != 2 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB040",
                    path.to_owned(),
                    "integer comparison requires two operands and one result",
                ));
            }
            for operand in &operation.operands {
                if available.get(operand) != Some(&BodyType::Integer(*operand_type)) {
                    errors.push(body_diagnostic(
                        "MNB041",
                        format!("{path}.operands"),
                        "integer comparison operand type does not match the operation type",
                    ));
                }
            }
            if operation
                .results
                .first()
                .is_some_and(|result| !result.ty.is_boolean())
            {
                errors.push(body_diagnostic(
                    "MNB042",
                    format!("{path}.results"),
                    "integer comparison result must have boolean type",
                ));
            }
        }
        BodyOperationKind::BooleanOp { operator } => {
            if !matches!(operator.as_str(), "and" | "or") {
                errors.push(body_diagnostic(
                    "MNB067",
                    format!("{path}.kind"),
                    format!("unsupported boolean operator {:?}", operator),
                ));
            }
            if operation.operands.len() != 2 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB068",
                    path.to_owned(),
                    "boolean operation requires two operands and one result",
                ));
            }
            let bool_type = BodyType::Named("bool".to_owned());
            for (index, operand) in operation.operands.iter().enumerate() {
                if available.get(operand) != Some(&bool_type) {
                    errors.push(body_diagnostic(
                        "MNB069",
                        format!("{path}.inputs[{index}]"),
                        "boolean operands must have bool type",
                    ));
                }
            }
            if operation
                .results
                .first()
                .is_some_and(|result| result.ty != bool_type)
            {
                errors.push(body_diagnostic(
                    "MNB070",
                    format!("{path}.results"),
                    "boolean operation result must have bool type",
                ));
            }
        }
        BodyOperationKind::ByteBitwise { operator } => {
            if !matches!(operator.as_str(), "and" | "or" | "xor") {
                errors.push(body_diagnostic(
                    "MNB071",
                    format!("{path}.kind"),
                    format!("unsupported byte bitwise operator {:?}", operator),
                ));
            }
            validate_byte_operands(operation, available, path, errors);
        }
        BodyOperationKind::ByteShift { operator } => {
            if !matches!(operator.as_str(), "shl" | "shr") {
                errors.push(body_diagnostic(
                    "MNB072",
                    format!("{path}.kind"),
                    format!("unsupported byte shift operator {:?}", operator),
                ));
            }
            if operation.operands.len() != 2 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB073",
                    path.to_owned(),
                    "byte shifts require two operands and one result",
                ));
            }
            let byte_type = BodyType::Byte;
            let counter_type = BodyType::Integer(IntegerType {
                bits: 64,
                signed: false,
            });
            if available.get(operation.operands.first().unwrap_or(&String::new()))
                != Some(&byte_type)
                || available.get(operation.operands.get(1).unwrap_or(&String::new()))
                    != Some(&counter_type)
            {
                errors.push(body_diagnostic(
                    "MNB074",
                    format!("{path}.operands"),
                    "byte shift operands must be a byte and a u64 count",
                ));
            }
            if operation
                .results
                .first()
                .is_some_and(|result| result.ty != BodyType::Byte)
            {
                errors.push(body_diagnostic(
                    "MNB075",
                    format!("{path}.results"),
                    "byte shift result must have byte type",
                ));
            }
        }
        BodyOperationKind::ByteCompare { predicate } => {
            if !matches!(predicate.as_str(), "eq" | "ne" | "lt" | "le" | "gt" | "ge") {
                errors.push(body_diagnostic(
                    "MNB076",
                    format!("{path}.kind.predicate"),
                    format!("unsupported byte comparison predicate {predicate:?}"),
                ));
            }
            validate_byte_operands(operation, available, path, errors);
        }
        BodyOperationKind::Convert { from, to } => {
            if !is_convertible_scalar(from) || !is_convertible_scalar(to) {
                errors.push(body_diagnostic(
                    "MNB077",
                    format!("{path}.kind"),
                    "explicit conversions require scalar integer or byte types",
                ));
            }
            if operation.operands.len() != 1 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB078",
                    path.to_owned(),
                    "explicit conversion requires one operand and one result",
                ));
            }
            if available.get(operation.operands.first().unwrap_or(&String::new())) != Some(from) {
                errors.push(body_diagnostic(
                    "MNB079",
                    format!("{path}.operands"),
                    "conversion operand does not have the declared source type",
                ));
            }
            if operation
                .results
                .first()
                .is_some_and(|result| &result.ty != to)
            {
                errors.push(body_diagnostic(
                    "MNB080",
                    format!("{path}.results"),
                    "conversion result does not have the declared target type",
                ));
            }
        }
        BodyOperationKind::SequenceConstruct {
            element_type,
            length,
        } => {
            if *length > MAX_SEQUENCE_BOUND {
                errors.push(body_diagnostic(
                    "MNB081",
                    format!("{path}.kind.length"),
                    format!(
                        "sequence length exceeds the declared profile ceiling {MAX_SEQUENCE_BOUND}"
                    ),
                ));
            }
            if operation.operands.len() != *length as usize || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB082",
                    path.to_owned(),
                    "sequence construction requires exactly one operand per element and one result",
                ));
            }
            for (index, operand) in operation.operands.iter().enumerate() {
                if available.get(operand) != Some(element_type.as_ref()) {
                    errors.push(body_diagnostic(
                        "MNB083",
                        format!("{path}.operands[{index}]"),
                        "sequence element value does not have the declared element type",
                    ));
                }
            }
            if let Some(result) = operation.results.first() {
                let expected = BodyType::Sequence {
                    element: element_type.clone(),
                    bound: SequenceBound::Exact(*length),
                };
                if result.ty != expected {
                    errors.push(body_diagnostic(
                        "MNB084",
                        format!("{path}.results"),
                        "sequence construction result does not have the exact constructed type",
                    ));
                }
            }
        }
        BodyOperationKind::SequenceProject { bound, evidence } => {
            if operation.operands.len() != 2 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB085",
                    path.to_owned(),
                    "sequence projection requires a sequence operand, an index operand, and one result",
                ));
            }
            let counter_type = BodyType::Integer(IntegerType {
                bits: 64,
                signed: false,
            });
            if available.get(operation.operands.get(1).unwrap_or(&String::new()))
                != Some(&counter_type)
            {
                errors.push(body_diagnostic(
                    "MNB086",
                    format!("{path}.operands[1]"),
                    "sequence index must have u64 type",
                ));
            }
            let expected_sequence = BodyType::Sequence {
                // The element type is established by the result; validation
                // compares it below against this projection's operand.
                element: operation
                    .results
                    .first()
                    .map(|result| Box::new(result.ty.clone()))
                    .unwrap_or_else(|| Box::new(BodyType::Named("invalid".to_owned()))),
                bound: *bound,
            };
            if available.get(operation.operands.first().unwrap_or(&String::new()))
                != Some(&expected_sequence)
            {
                errors.push(body_diagnostic(
                    "MNB087",
                    format!("{path}.operands[0]"),
                    "sequence projection operand does not have the declared bounded-sequence type",
                ));
            }
            if matches!(evidence, BoundsEvidence::RuntimeChecked { failure: _ })
                && matches!(bound, SequenceBound::UpTo(_))
            {
                // Views always check against their runtime length; evidence is
                // recorded but never claims static validity for views.
            }
        }
        BodyOperationKind::Select { operand_type } => {
            if operation.operands.len() != 3 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB102",
                    path.to_owned(),
                    "selection requires a condition, two candidate values, and one result",
                ));
                return;
            }
            let boolean_type = BodyType::Named("bool".to_owned());
            if available.get(operation.operands[0].as_str()) != Some(&boolean_type) {
                errors.push(body_diagnostic(
                    "MNB103",
                    format!("{path}.operands[0]"),
                    "selection condition must have boolean type",
                ));
            }
            for (index, operand) in operation.operands[1..3].iter().enumerate() {
                if available.get(operand) != Some(operand_type.as_ref()) {
                    errors.push(body_diagnostic(
                        "MNB104",
                        format!("{path}.operands[{}]", index + 1),
                        "selection candidates must have the declared operand type",
                    ));
                }
            }
            if operation
                .results
                .first()
                .is_some_and(|result| &result.ty != operand_type.as_ref())
            {
                errors.push(body_diagnostic(
                    "MNB105",
                    format!("{path}.results"),
                    "selection result must have the declared operand type",
                ));
            }
        }
        BodyOperationKind::SequenceReplace {
            element_type,
            bound,
            evidence,
        } => {
            if matches!(bound, SequenceBound::UpTo(_)) {
                errors.push(body_diagnostic(
                    "MNB106",
                    format!("{path}.kind.bound"),
                    "functional sequence update requires an exact bound this tranche; views refuse",
                ));
            }
            if operation.operands.len() != 3 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB107",
                    path.to_owned(),
                    "functional sequence update requires a sequence, an index, an element, and one result",
                ));
            }
            let counter_type = BodyType::Integer(IntegerType {
                bits: 64,
                signed: false,
            });
            if available.get(operation.operands.get(1).unwrap_or(&String::new()))
                != Some(&counter_type)
            {
                errors.push(body_diagnostic(
                    "MNB108",
                    format!("{path}.operands[1]"),
                    "functional update index must have u64 type",
                ));
            }
            if available.get(operation.operands.get(2).unwrap_or(&String::new()))
                != Some(element_type.as_ref())
            {
                errors.push(body_diagnostic(
                    "MNB109",
                    format!("{path}.operands[2]"),
                    "functional update element does not have the declared element type",
                ));
            }
            let expected_sequence = BodyType::Sequence {
                element: element_type.clone(),
                bound: *bound,
            };
            if available.get(operation.operands.first().unwrap_or(&String::new()))
                != Some(&expected_sequence)
            {
                errors.push(body_diagnostic(
                    "MNB110",
                    format!("{path}.operands[0]"),
                    "functional update source does not have the declared bounded-sequence type",
                ));
            }
            if operation
                .results
                .first()
                .is_some_and(|result| result.ty != expected_sequence)
            {
                errors.push(body_diagnostic(
                    "MNB111",
                    format!("{path}.results"),
                    "functional update result does not preserve the source bounded-sequence type",
                ));
            }
            let _ = evidence;
        }
        BodyOperationKind::SequenceLength { .. } => {
            if operation.operands.len() != 1 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB088",
                    path.to_owned(),
                    "length observation requires one operand and one u64 result",
                ));
                return;
            }
            let counter_type = BodyType::Integer(IntegerType {
                bits: 64,
                signed: false,
            });
            if operation
                .results
                .first()
                .is_some_and(|result| result.ty != counter_type)
            {
                errors.push(body_diagnostic(
                    "MNB089",
                    format!("{path}.results"),
                    "length observation result must have u64 type",
                ));
            }
        }
        BodyOperationKind::ViewConstruct {
            source_bound,
            view_bound,
        } => {
            if operation.operands.len() != 3 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB090",
                    path.to_owned(),
                    "view construction requires source, start, end operands and one result",
                ));
                return;
            }
            let counter_type = BodyType::Integer(IntegerType {
                bits: 64,
                signed: false,
            });
            if available.get(operation.operands.get(1).unwrap_or(&String::new()))
                != Some(&counter_type)
                || available.get(operation.operands.get(2).unwrap_or(&String::new()))
                    != Some(&counter_type)
            {
                errors.push(body_diagnostic(
                    "MNB091",
                    format!("{path}.operands"),
                    "view range endpoints must have u64 type",
                ));
            }
            let SequenceBound::UpTo(capacity) = view_bound else {
                errors.push(body_diagnostic(
                    "MNB092",
                    format!("{path}.kind.view_bound"),
                    "view construction results must carry an UpTo bound",
                ));
                return;
            };
            let expected_result = BodyType::Sequence {
                // Element type agreement with the source is checked below via
                // the operand's declared type.
                element: match available.get(operation.operands.first().unwrap_or(&String::new())) {
                    Some(BodyType::Sequence { element, .. }) => element.clone(),
                    _ => Box::new(BodyType::Named("invalid".to_owned())),
                },
                bound: *view_bound,
            };
            let source_matches = matches!(
                available.get(operation.operands.first().unwrap_or(&String::new())),
                Some(BodyType::Sequence { bound, .. }) if bound == source_bound
            );
            if !source_matches {
                errors.push(body_diagnostic(
                    "MNB093",
                    format!("{path}.operands[0]"),
                    "view construction source does not have the declared source bound",
                ));
            }
            if let Some(result) = operation.results.first() {
                if result.ty != expected_result && !matches!(&result.ty, BodyType::Named(_)) {
                    errors.push(body_diagnostic(
                        "MNB094",
                        format!("{path}.results"),
                        "view construction result does not match its declared view type",
                    ));
                }
                if !matches!(result.ty, BodyType::Named(_)) && *capacity > MAX_SEQUENCE_BOUND {
                    errors.push(body_diagnostic(
                        "MNB095",
                        format!("{path}.kind.view_bound"),
                        format!(
                            "view capacity exceeds the declared profile ceiling {MAX_SEQUENCE_BOUND}"
                        ),
                    ));
                }
            }
        }
        BodyOperationKind::FiniteConstruct {
            type_identity,
            variant_identity,
            discriminant,
            payload_fields,
        } => {
            if operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB044",
                    path.to_owned(),
                    "finite construction requires exactly one result",
                ));
            }
            let declared = program
                .finite_types
                .iter()
                .find(|item| &item.identity == type_identity);
            let valid_variant = declared.and_then(|item| {
                item.variants.iter().find(|variant| {
                    &variant.identity == variant_identity && variant.discriminant == *discriminant
                })
            });
            if declared.is_none() || valid_variant.is_none() {
                errors.push(body_diagnostic(
                    "MNB045",
                    format!("{path}.kind"),
                    "finite constructor does not identify a declared type/variant/discriminant",
                ));
            }
            // Payload integrity: names must be canonical (sorted, unique) and
            // every declared payload field must be supplied, and nothing more.
            if payload_fields
                .windows(2)
                .any(|window| window[0] >= window[1])
            {
                errors.push(body_diagnostic(
                    "MNB060",
                    format!("{path}.kind.payload_fields"),
                    "payload field names must be canonically sorted and unique",
                ));
            }
            let declared_payload = valid_variant.map(|variant| variant.payload.clone());
            match &declared_payload {
                Some(fields) => {
                    let declared_names: Vec<_> =
                        fields.iter().map(|field| field.name.clone()).collect();
                    if declared_names != *payload_fields
                        || operation.operands.len() != payload_fields.len()
                    {
                        errors.push(body_diagnostic(
                            "MNB061",
                            format!("{path}.kind.payload_fields"),
                            "finite constructor payload does not match the declared variant payload fields",
                        ));
                    }
                }
                None if !payload_fields.is_empty() || !operation.operands.is_empty() => {
                    errors.push(body_diagnostic(
                        "MNB062",
                        format!("{path}.kind"),
                        "variant carries no declared payload but the construction supplies one",
                    ));
                }
                None => {}
            }
            for (index, _operand) in operation.operands.iter().enumerate() {
                let expected_ty = payload_fields
                    .get(index)
                    .and_then(|name| {
                        valid_variant.and_then(|variant| {
                            variant.payload.iter().find(|field| &field.name == name)
                        })
                    })
                    .map(|field| semantic_record_field_type(program, field));
                let actual_ty = operation.results.first().map(|_| ()).and_then(|()| {
                    operation
                        .operands
                        .get(index)
                        .and_then(|operand_id| available.get(operand_id))
                        .cloned()
                });
                if expected_ty.is_some() && actual_ty.is_some() && expected_ty != actual_ty {
                    errors.push(body_diagnostic(
                        "MNB063",
                        format!("{path}.inputs[{index}]"),
                        "payload operand type does not match the declared payload field type",
                    ));
                }
            }
            if let Some(result) = operation.results.first() {
                let expected = declared.map(|item| BodyType::Finite {
                    identity: item.identity.clone(),
                    name: item.name.clone(),
                });
                if expected.as_ref() != Some(&result.ty) {
                    errors.push(body_diagnostic(
                        "MNB046",
                        format!("{path}.results"),
                        "finite constructor result type does not match its nominal type",
                    ));
                }
            }
        }
        BodyOperationKind::FiniteIsVariant {
            type_identity,
            variant_identity,
            discriminant,
        } => {
            if operation.operands.len() != 1 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB047",
                    path.to_owned(),
                    "finite variant test requires one operand and one boolean result",
                ));
            }
            let declared = program
                .finite_types
                .iter()
                .find(|item| &item.identity == type_identity);
            let valid_variant = declared.and_then(|item| {
                item.variants.iter().find(|variant| {
                    &variant.identity == variant_identity && variant.discriminant == *discriminant
                })
            });
            let expected_operand = declared.map(|item| BodyType::Finite {
                identity: item.identity.clone(),
                name: item.name.clone(),
            });
            if declared.is_none()
                || valid_variant.is_none()
                || operation
                    .operands
                    .first()
                    .and_then(|operand| available.get(operand))
                    != expected_operand.as_ref()
                || operation
                    .results
                    .first()
                    .is_some_and(|result| !result.ty.is_boolean())
            {
                errors.push(body_diagnostic(
                    "MNB048",
                    format!("{path}.kind"),
                    "finite variant test is not type/variant/discriminant consistent",
                ));
            }
        }
        BodyOperationKind::FinitePayloadProject {
            type_identity,
            variant_identity,
            discriminant,
            field,
        } => {
            if operation.operands.len() != 1 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB064",
                    path.to_owned(),
                    "payload projection requires one operand and one result",
                ));
            }
            let declared = program
                .finite_types
                .iter()
                .find(|item| &item.identity == type_identity);
            let valid_variant = declared.and_then(|item| {
                item.variants.iter().find(|variant| {
                    &variant.identity == variant_identity && variant.discriminant == *discriminant
                })
            });
            let payload_field = valid_variant.and_then(|variant| {
                variant
                    .payload
                    .iter()
                    .find(|item| item.name == *field)
                    .cloned()
            });
            let expected_operand = declared.map(|item| BodyType::Finite {
                identity: item.identity.clone(),
                name: item.name.clone(),
            });
            if declared.is_none()
                || valid_variant.is_none()
                || payload_field.is_none()
                || operation
                    .operands
                    .first()
                    .and_then(|operand| available.get(operand))
                    != expected_operand.as_ref()
            {
                errors.push(body_diagnostic(
                    "MNB065",
                    format!("{path}.kind"),
                    "payload projection is not consistent with a declared variant payload field",
                ));
            }
            if let (Some(field), Some(result)) = (payload_field, operation.results.first()) {
                if semantic_record_field_type(program, &field) != result.ty {
                    errors.push(body_diagnostic(
                        "MNB066",
                        format!("{path}.results"),
                        "payload projection result type does not match the declared field type",
                    ));
                }
            }
        }
        BodyOperationKind::RecordConstruct {
            type_identity,
            field_names,
        } => {
            if operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB048",
                    path.to_owned(),
                    "record construction requires exactly one result",
                ));
            }
            let declared = program
                .record_types
                .iter()
                .find(|item| &item.identity == type_identity);
            let Some(declared) = declared else {
                errors.push(body_diagnostic(
                    "MNB049",
                    format!("{path}.kind"),
                    "record construction does not identify a declared record type",
                ));
                return;
            };
            // Canonical field order is sorted by field name; the operand list
            // must follow it so one logical construction has one encoding.
            let mut canonical: Vec<String> = declared
                .fields
                .iter()
                .map(|field| field.name.clone())
                .collect();
            canonical.sort();
            let mut supplied = field_names.clone();
            supplied.sort();
            supplied.dedup();
            if &supplied != field_names {
                errors.push(body_diagnostic(
                    "MNB050",
                    format!("{path}.kind.field_names"),
                    "record construction field names are not in canonical order",
                ));
            }
            if supplied != canonical || field_names.len() != operation.operands.len() {
                errors.push(body_diagnostic(
                    "MNB051",
                    format!("{path}.kind"),
                    "record construction fields do not exactly match the declared fields",
                ));
                return;
            }
            for (index, field) in declared.fields.iter().enumerate() {
                let expected_field_type = semantic_record_field_type(program, field);
                let Some(operand) = operation.operands.get(index) else {
                    break;
                };
                let actual = available.get(operand);
                if actual != Some(&expected_field_type) {
                    errors.push(body_diagnostic(
                        "MNB052",
                        format!("{path}.operands[{index}]"),
                        "record field value type does not match the declared field type",
                    ));
                }
            }
            if let Some(result) = operation.results.first() {
                let expected = BodyType::Record {
                    identity: declared.identity.clone(),
                    name: declared.name.clone(),
                };
                if result.ty != expected {
                    errors.push(body_diagnostic(
                        "MNB053",
                        format!("{path}.results"),
                        "record construction result type does not match its nominal record",
                    ));
                }
            }
        }
        BodyOperationKind::RecordProject {
            type_identity,
            field,
        } => {
            if operation.operands.len() != 1 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB054",
                    path.to_owned(),
                    "record projection requires one operand and one result",
                ));
                return;
            }
            let declared = program
                .record_types
                .iter()
                .find(|item| &item.identity == type_identity);
            let Some(declared) = declared else {
                errors.push(body_diagnostic(
                    "MNB049",
                    format!("{path}.kind"),
                    "record projection does not identify a declared record type",
                ));
                return;
            };
            let expected_operand = BodyType::Record {
                identity: declared.identity.clone(),
                name: declared.name.clone(),
            };
            if operation
                .operands
                .first()
                .and_then(|operand| available.get(operand))
                != Some(&expected_operand)
            {
                errors.push(body_diagnostic(
                    "MNB055",
                    format!("{path}.operands[0]"),
                    "record projection operand does not have the projected record type",
                ));
            }
            match declared.fields.iter().find(|item| &item.name == field) {
                None => errors.push(body_diagnostic(
                    "MNB056",
                    format!("{path}.kind.field"),
                    "record projection names a field the record does not declare",
                )),
                Some(declared_field) => {
                    let expected_field_type = semantic_record_field_type(program, declared_field);
                    if operation.results.first().map(|result| &result.ty)
                        != Some(&expected_field_type)
                    {
                        errors.push(body_diagnostic(
                            "MNB057",
                            format!("{path}.results"),
                            "record projection result type does not match the field type",
                        ));
                    }
                }
            }
        }
        BodyOperationKind::Call {
            function: callee_identity,
            function_name,
            required_capabilities,
            effects,
        } => {
            // Call targets may live in this program's own namespace or in any
            // bound dependency namespace; identities stay anchored to the
            // declaring module either way.
            let mut target_namespaces = vec![program.module.clone()];
            for dependency in &program.dependencies {
                if let Some(name) = dependency.module_name() {
                    target_namespaces.push(name);
                }
            }
            let callee = program.functions.iter().find(|candidate| {
                candidate.name == *function_name
                    && target_namespaces.iter().any(|namespace| {
                        crate::identity::function_id(namespace, &candidate.name) == *callee_identity
                    })
            });
            let Some(callee) = callee else {
                errors.push(body_diagnostic(
                    "MNB049",
                    format!("{path}.kind.function"),
                    "call target does not resolve to an exact function identity",
                ));
                validate_metadata(program, function, operation, path, errors);
                return;
            };
            if operation.operands.len() != callee.inputs.len()
                || operation.results.len() != callee.outputs.len()
            {
                errors.push(body_diagnostic(
                    "MNB050",
                    path.to_owned(),
                    "call arity does not match the callee signature",
                ));
            }
            for (operand, parameter) in operation.operands.iter().zip(&callee.inputs) {
                if available
                    .get(operand)
                    .map(BodyType::semantic_name)
                    .as_deref()
                    != Some(parameter.value_type.as_str())
                {
                    errors.push(body_diagnostic(
                        "MNB051",
                        format!("{path}.operands"),
                        "call argument type does not match the callee parameter",
                    ));
                }
            }
            for (result, output) in operation.results.iter().zip(&callee.outputs) {
                if result.ty.semantic_name() != output.value_type {
                    errors.push(body_diagnostic(
                        "MNB052",
                        format!("{path}.results"),
                        "call result type does not match the callee output",
                    ));
                }
            }
            let mut declared_required = callee.capabilities.clone();
            declared_required.sort();
            let mut recorded_required = required_capabilities.clone();
            recorded_required.sort();
            let mut declared_effects = callee.effects.clone();
            declared_effects.sort_by(|left, right| {
                (&left.kind, &left.target, &left.capability).cmp(&(
                    &right.kind,
                    &right.target,
                    &right.capability,
                ))
            });
            let mut recorded_effects = effects.clone();
            recorded_effects.sort_by(|left, right| {
                (&left.kind, &left.target, &left.capability).cmp(&(
                    &right.kind,
                    &right.target,
                    &right.capability,
                ))
            });
            let caller_authorizes = required_capabilities
                .iter()
                .all(|capability| function.capabilities.contains(capability))
                && effects.iter().all(|effect| {
                    function.effects.iter().any(|caller_effect| {
                        caller_effect.kind == effect.kind
                            && caller_effect.capability == effect.capability
                    })
                });
            if recorded_required != declared_required
                || recorded_effects != declared_effects
                || !caller_authorizes
            {
                errors.push(body_diagnostic(
                    "MNB053",
                    format!("{path}.kind"),
                    "call authority/effect closure is incomplete or was laundered",
                ));
            }
        }
        BodyOperationKind::Effect { effect, capability } => {
            if !operation.results.is_empty() || !operation.operands.is_empty() {
                errors.push(body_diagnostic(
                    "MNB019",
                    path.to_owned(),
                    "effect operations do not currently produce values or consume operands",
                ));
            }
            if capability != &effect.capability || !function.capabilities.contains(capability) {
                errors.push(body_diagnostic(
                    "MNB020",
                    format!("{path}.kind"),
                    "body effect capability does not match a declared function capability",
                ));
            }
            if !function.effects.iter().any(|declared| declared == effect) {
                errors.push(body_diagnostic(
                    "MNB021",
                    format!("{path}.kind"),
                    "body effect is not declared by the enclosing function",
                ));
            }
        }
        BodyOperationKind::RuntimeCheck {
            obligation,
            fact,
            failure: _,
        } => {
            if obligation.0.trim().is_empty() || fact.identity.0.trim().is_empty() {
                errors.push(body_diagnostic(
                    "MNB022",
                    path.to_owned(),
                    "runtime checks require an obligation and fact identity",
                ));
            }
        }
    }
    validate_metadata(program, function, operation, path, errors);
}

fn validate_metadata(
    program: &Program,
    function: &Function,
    operation: &BodyOperation,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    for contract in &operation.contracts {
        if !function.contracts.iter().any(|item| item.id == *contract) {
            errors.push(body_diagnostic(
                "MNB023",
                format!("{path}.contracts"),
                format!("unknown contract reference {:?}", contract),
            ));
        }
    }
    for assumption in &operation.assumptions {
        if !program
            .assumptions
            .iter()
            .any(|item| item.id == *assumption)
            || !function.assumptions.contains(assumption)
        {
            errors.push(body_diagnostic(
                "MNB024",
                format!("{path}.assumptions"),
                format!(
                    "assumption {:?} is not consumed by the function",
                    assumption
                ),
            ));
        }
    }
    if let Some(machine_intent) = &operation.machine_intent {
        if machine_intent.intent.identity.0.trim().is_empty() {
            errors.push(body_diagnostic(
                "MNB025",
                format!("{path}.machine_intent"),
                "machine intent requires a stable identity",
            ));
        }
        let mut requirement_ids = BTreeSet::new();
        for requirement in &machine_intent.requirements {
            if requirement.identity.0.trim().is_empty()
                || !requirement_ids.insert(requirement.identity.clone())
                || requirement.subject.0.trim().is_empty()
            {
                errors.push(body_diagnostic(
                    "MNB026",
                    format!("{path}.machine_intent.requirements"),
                    "machine-intent requirements must have unique identities and subjects",
                ));
            }
        }
        let mut obligation_ids = BTreeSet::new();
        for obligation in &machine_intent.obligations {
            if obligation.identity.0.trim().is_empty()
                || !obligation_ids.insert(obligation.identity.clone())
                || obligation.subject.0.trim().is_empty()
            {
                errors.push(body_diagnostic(
                    "MNB027",
                    format!("{path}.machine_intent.obligations"),
                    "machine-intent obligations must have unique identities and subjects",
                ));
            }
        }
    }
    if let Some(lowering) = &operation.lowering {
        if lowering.schema_version != EXECUTABLE_BODY_SCHEMA_VERSION {
            errors.push(body_diagnostic(
                "MNB028",
                format!("{path}.lowering.schema_version"),
                "unsupported lowering-envelope schema",
            ));
        }
        if lowering.permitted_realizations.is_empty() && lowering.fallback.is_none() {
            errors.push(body_diagnostic(
                "MNB029",
                format!("{path}.lowering"),
                "lowering envelope needs a realization class or conservative fallback",
            ));
        }
    }
    if let Some(portability) = &operation.portability {
        if portability.schema_version != EXECUTABLE_BODY_SCHEMA_VERSION {
            errors.push(body_diagnostic(
                "MNB030",
                format!("{path}.portability.schema_version"),
                "unsupported portability-envelope schema",
            ));
        }
        if portability.integer_widths.contains(&0) {
            errors.push(body_diagnostic(
                "MNB031",
                format!("{path}.portability.integer_widths"),
                "portability integer widths must be nonzero",
            ));
        }
    }
}

fn validate_terminator(
    terminator: &BodyTerminator,
    path: &str,
    available: &BTreeMap<String, BodyType>,
    block_ids: &BTreeSet<String>,
    block_parameters: &BTreeMap<String, Vec<BodyValue>>,
    errors: &mut Vec<Diagnostic>,
) {
    match terminator {
        BodyTerminator::Failure { .. } => {}
        BodyTerminator::Return { values } => {
            check_terminator_values(values, path, available, errors);
        }
        BodyTerminator::Branch { target, arguments } => {
            check_target(target, path, block_ids, errors);
            check_terminator_values(arguments, path, available, errors);
            validate_block_arguments(target, arguments, available, block_parameters, path, errors);
        }
        BodyTerminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => {
            if !available.get(condition).is_some_and(BodyType::is_boolean) {
                errors.push(body_diagnostic(
                    "MNB033",
                    format!("{path}.terminator.condition"),
                    "conditional branch requires a boolean condition",
                ));
            }
            check_target(then_target, path, block_ids, errors);
            check_target(else_target, path, block_ids, errors);
            check_terminator_values(then_arguments, path, available, errors);
            check_terminator_values(else_arguments, path, available, errors);
            validate_block_arguments(
                then_target,
                then_arguments,
                available,
                block_parameters,
                path,
                errors,
            );
            validate_block_arguments(
                else_target,
                else_arguments,
                available,
                block_parameters,
                path,
                errors,
            );
        }
    }
}

fn validate_block_arguments(
    target: &str,
    arguments: &[String],
    available: &BTreeMap<String, BodyType>,
    block_parameters: &BTreeMap<String, Vec<BodyValue>>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let Some(parameters) = block_parameters.get(target) else {
        return;
    };
    if parameters.len() != arguments.len() {
        errors.push(body_diagnostic(
            "MNB012",
            format!("{path}.terminator"),
            "control-flow arguments must match target block parameters",
        ));
    }
    for (index, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
        if available.get(argument) != Some(&parameter.ty) {
            errors.push(body_diagnostic(
                "MNB037",
                format!("{path}.terminator.arguments[{index}]"),
                "control-flow argument type does not match the target parameter",
            ));
        }
    }
}

fn check_terminator_values(
    values: &[String],
    path: &str,
    available: &BTreeMap<String, BodyType>,
    errors: &mut Vec<Diagnostic>,
) {
    for value in values {
        if !available.contains_key(value) {
            errors.push(body_diagnostic(
                "MNB032",
                format!("{path}.terminator"),
                format!("terminator value {:?} is not defined before use", value),
            ));
        }
    }
}

fn check_target(
    target: &str,
    path: &str,
    block_ids: &BTreeSet<String>,
    errors: &mut Vec<Diagnostic>,
) {
    if !block_ids.contains(target) {
        errors.push(body_diagnostic(
            "MNB034",
            format!("{path}.terminator"),
            format!("unknown block target {:?}", target),
        ));
    }
}

fn validate_return_types(
    terminator: &BodyTerminator,
    program: &Program,
    function: &Function,
    available: &BTreeMap<String, BodyType>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let BodyTerminator::Return { values } = terminator else {
        return;
    };
    if values.len() != function.outputs.len() {
        errors.push(body_diagnostic(
            "MNB035",
            format!("{path}.body"),
            "body return arity does not match function outputs",
        ));
        return;
    }
    for (index, value) in values.iter().enumerate() {
        let expected = semantic_body_type(program, &function.outputs[index].value_type);
        if available.get(value) != Some(&expected) {
            errors.push(body_diagnostic(
                "MNB036",
                format!("{path}.body"),
                format!(
                    "return value {:?} does not match declared output type",
                    value
                ),
            ));
        }
    }
}

fn semantic_body_type(program: &Program, name: &str) -> BodyType {
    if let Some(sequence) = semantic_sequence_type(program, name) {
        return sequence;
    }
    if let Some(record_type) = program.record_types.iter().find(|item| item.name == name) {
        return BodyType::Record {
            identity: record_type.identity.clone(),
            name: record_type.name.clone(),
        };
    }
    program
        .finite_types
        .iter()
        .find(|finite_type| finite_type.name == name)
        .map(|finite_type| BodyType::Finite {
            identity: finite_type.identity.clone(),
            name: finite_type.name.clone(),
        })
        .unwrap_or_else(|| BodyType::from_semantic_name(name))
}

/// Resolve a canonical bounded-sequence spelling `[E; N]` / `[E; up_to M]`
/// whose element type may be a declared nominal type of this program.
fn semantic_sequence_type(program: &Program, name: &str) -> Option<BodyType> {
    let inner = name.strip_prefix('[')?.strip_suffix(']')?;
    let separator = inner.rfind(';')?;
    let (element_text, bound_text) = (inner[..separator].trim(), inner[separator + 1..].trim());
    let bound = if let Some(capacity) = bound_text.strip_prefix("up_to") {
        SequenceBound::UpTo(capacity.trim().parse().ok()?)
    } else {
        SequenceBound::Exact(bound_text.parse().ok()?)
    };
    if bound.ceiling() > MAX_SEQUENCE_BOUND {
        return None;
    }
    let element = Box::new(semantic_body_type(program, element_text));
    match &*element {
        // Unresolvable element spellings cannot form a canonical sequence.
        BodyType::Named(_) => None,
        _ => Some(BodyType::Sequence { element, bound }),
    }
}

/// Resolve a declared record field's semantic body type at validation time.
fn semantic_record_field_type(program: &Program, field: &crate::core::RecordField) -> BodyType {
    semantic_body_type(program, &field.field_type)
}

fn validate_byte_operands(
    operation: &BodyOperation,
    available: &BTreeMap<String, BodyType>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if operation.operands.len() != 2 || operation.results.len() != 1 {
        errors.push(body_diagnostic(
            "MNB096",
            path.to_owned(),
            "byte operations require two operands and one result",
        ));
        return;
    }
    let byte_type = BodyType::Byte;
    for (index, operand) in operation.operands.iter().enumerate() {
        if available.get(operand) != Some(&byte_type) {
            errors.push(body_diagnostic(
                "MNB097",
                format!("{path}.operands[{index}]"),
                "byte operands must have byte type",
            ));
        }
    }
    let produces_bool = matches!(operation.kind, BodyOperationKind::ByteCompare { .. });
    let expected_result = if produces_bool {
        BodyType::Named("bool".to_owned())
    } else {
        BodyType::Byte
    };
    if operation.results.first().map(|result| &result.ty) != Some(&expected_result) {
        errors.push(body_diagnostic(
            if produces_bool { "MNB098" } else { "MNB099" },
            format!("{path}.results"),
            if produces_bool {
                "byte comparison result must have boolean type"
            } else {
                "bitwise byte operation result must have byte type"
            },
        ));
    }
}

/// Scalars that explicit total conversions may cross between. Booleans
/// convert outward as 0/1; they are never conversion targets.
fn is_convertible_scalar(ty: &BodyType) -> bool {
    matches!(ty, BodyType::Byte)
        || matches!(ty, BodyType::Integer(integer) if matches!(integer.bits, 1..=64))
        || matches!(ty, BodyType::Named(name) if name == "bool")
}

fn body_diagnostic(code: &str, path: String, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        path,
        message: message.into(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{FailureMode, Value, SUPPORTED_SCHEMA_VERSION};

    pub(crate) fn executable_program() -> Program {
        let mut program = crate::validation::tests::valid_program();
        let function = &mut program.functions[0];
        function.inputs = vec![Value {
            name: "a".to_owned(),
            value_type: "i32".to_owned(),
        }];
        function.outputs = vec![Value {
            name: "result".to_owned(),
            value_type: "i32".to_owned(),
        }];
        function.contracts.clear();
        function.effects.clear();
        function.capabilities.clear();
        function.assumptions.clear();
        function.evidence.clear();
        function.failure = FailureMode::Isolated;
        function.body = Some(FunctionBody {
            schema_version: EXECUTABLE_BODY_SCHEMA_VERSION.to_owned(),
            entry: "entry".to_owned(),
            parameters: vec![BodyParameter {
                id: "a".to_owned(),
                name: "a".to_owned(),
                ty: BodyType::Integer(IntegerType {
                    bits: 32,
                    signed: true,
                }),
            }],
            cycle_policy: BodyCyclePolicy::Legacy,
            bounded_iterations: Vec::new(),
            blocks: vec![BodyBlock {
                id: "entry".to_owned(),
                parameters: Vec::new(),
                operations: vec![
                    BodyOperation {
                        id: "one".to_owned(),
                        kind: BodyOperationKind::Constant {
                            value: 1,
                            ty: BodyType::Integer(IntegerType {
                                bits: 32,
                                signed: true,
                            }),
                        },
                        operands: Vec::new(),
                        results: vec![BodyValue {
                            id: "one".to_owned(),
                            ty: BodyType::Integer(IntegerType {
                                bits: 32,
                                signed: true,
                            }),
                        }],
                        contracts: Vec::new(),
                        assumptions: Vec::new(),
                        machine_intent: None,
                        lowering: None,
                        portability: None,
                    },
                    BodyOperation {
                        id: "sum".to_owned(),
                        kind: BodyOperationKind::Integer {
                            operator: "add".to_owned(),
                            operand_type: IntegerType {
                                bits: 32,
                                signed: true,
                            },
                            intent: ArithmeticIntent::Checked,
                        },
                        operands: vec!["a".to_owned(), "one".to_owned()],
                        results: vec![BodyValue {
                            id: "sum".to_owned(),
                            ty: BodyType::Integer(IntegerType {
                                bits: 32,
                                signed: true,
                            }),
                        }],
                        contracts: Vec::new(),
                        assumptions: Vec::new(),
                        machine_intent: None,
                        lowering: None,
                        portability: None,
                    },
                ],
                terminator: BodyTerminator::Return {
                    values: vec!["sum".to_owned()],
                },
            }],
        });
        program
    }

    #[test]
    fn symbolic_body_validates_and_has_deterministic_identity() {
        let program = executable_program();
        let report = program.validate();
        assert!(report.valid, "{:#?}", report.errors);
        let body = program.functions[0].body.as_ref().expect("body");
        assert_eq!(
            body.canonical_json().unwrap(),
            body.canonical_json().unwrap()
        );
        assert_eq!(body.fingerprint().unwrap().len(), 64);
        assert_eq!(
            body.identity(&program.module, &program.functions[0].name),
            body.identity(&program.module, &program.functions[0].name)
        );
        let graph = program.semantic_graph().expect("body graph");
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == crate::EdgeKind::ContainsOperation));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == crate::EdgeKind::ConsumesValue));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == crate::EdgeKind::RequiresObligation));
        let ir = program.lower_to_ir().expect("body IR");
        let operations = &ir.functions[0].blocks[0].operations;
        assert!(operations.iter().any(|operation| {
            matches!(operation.kind, crate::IrOperationKind::Integer { .. })
                && operation.machine_intent.is_some()
        }));
        assert!(ir
            .obligations
            .iter()
            .any(|obligation| obligation.status == crate::ObligationStatus::Unknown));
        assert!(!ir.transformations.is_empty());
        let mut changed = program.clone();
        let changed_body = changed.functions[0].body.as_mut().expect("body");
        let integer = changed_body.blocks[0]
            .operations
            .iter_mut()
            .find(|operation| matches!(operation.kind, BodyOperationKind::Integer { .. }))
            .expect("integer operation");
        if let BodyOperationKind::Integer { intent, .. } = &mut integer.kind {
            *intent = ArithmeticIntent::Wrapping;
        }
        assert_ne!(
            program.functions[0]
                .body
                .as_ref()
                .expect("body")
                .fingerprint()
                .unwrap(),
            changed_body.fingerprint().unwrap()
        );
    }

    #[test]
    fn body_rejects_undefined_values_and_type_mismatches() {
        let mut program = executable_program();
        let body = program.functions[0].body.as_mut().expect("body");
        "missing".clone_into(&mut body.blocks[0].operations[1].operands[0]);
        body.blocks[0].operations[1].results[0].ty = BodyType::Integer(IntegerType {
            bits: 16,
            signed: true,
        });
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB010"));
        assert!(report.errors.iter().any(|error| error.code == "MNB017"));
        assert!(report.errors.iter().any(|error| error.code == "MNB018"));
    }

    #[test]
    fn body_rejects_undeclared_effect_and_bad_return() {
        let mut program = executable_program();
        let function = &mut program.functions[0];
        "u32".clone_into(&mut function.outputs[0].value_type);
        let body = function.body.as_mut().expect("body");
        body.blocks[0].operations[0].kind = BodyOperationKind::Effect {
            effect: Effect {
                kind: "write".to_owned(),
                target: "Store.value".to_owned(),
                capability: "StoreMutation".to_owned(),
            },
            capability: "StoreMutation".to_owned(),
        };
        body.blocks[0].operations[0].operands.clear();
        body.blocks[0].operations[0].results.clear();
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB020"));
        assert!(report.errors.iter().any(|error| error.code == "MNB021"));
        assert!(report.errors.iter().any(|error| error.code == "MNB036"));
        assert_eq!(program.schema_version, SUPPORTED_SCHEMA_VERSION);
    }

    #[test]
    fn body_rejects_unknown_integer_comparison_predicate() {
        let mut program = executable_program();
        let body = program.functions[0].body.as_mut().expect("body");
        let operation = &mut body.blocks[0].operations[1];
        operation.kind = BodyOperationKind::IntegerCompare {
            predicate: "unordered".to_owned(),
            operand_type: IntegerType {
                bits: 32,
                signed: true,
            },
        };
        operation.operands = vec!["a".to_owned(), "one".to_owned()];
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB039"));
    }

    #[test]
    fn body_rejects_non_wrapping_bitwise_intent() {
        let mut program = executable_program();
        let body = program.functions[0].body.as_mut().expect("body");
        let operation = &mut body.blocks[0].operations[1];
        operation.kind = BodyOperationKind::Integer {
            operator: "xor".to_owned(),
            operand_type: IntegerType {
                bits: 32,
                signed: true,
            },
            intent: ArithmeticIntent::Checked,
        };
        operation.operands = vec!["a".to_owned(), "one".to_owned()];
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB043"));
    }

    #[test]
    fn body_requires_the_exact_widening_result_type() {
        let mut program = Program::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/executable/arithmetic-envelope.mncs.json"
        )))
        .expect("arithmetic envelope");
        assert!(program.validate().valid);
        let wide = program.functions[1].body.as_mut().expect("body").blocks[0]
            .operations
            .first_mut()
            .expect("widening operation");
        wide.results[0].ty = BodyType::Integer(IntegerType {
            bits: 15,
            signed: true,
        });
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB018"));
    }

    #[test]
    fn body_rejects_mismatched_block_arguments() {
        let mut program = executable_program();
        let body = program.functions[0].body.as_mut().expect("body");
        body.blocks[0].terminator = BodyTerminator::Branch {
            target: "join".to_owned(),
            arguments: vec!["a".to_owned()],
        };
        body.blocks.push(BodyBlock {
            id: "join".to_owned(),
            parameters: vec![BodyValue {
                id: "joined".to_owned(),
                ty: BodyType::Named("bool".to_owned()),
            }],
            operations: Vec::new(),
            terminator: BodyTerminator::Return {
                values: vec!["joined".to_owned()],
            },
        });
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB037"));
    }

    #[test]
    fn body_rejects_value_from_non_dominating_branch() {
        let mut program = executable_program();
        let function = &mut program.functions[0];
        function.inputs = vec![Value {
            name: "condition".to_owned(),
            value_type: "bool".to_owned(),
        }];
        let body = function.body.as_mut().expect("body");
        body.parameters = vec![BodyParameter {
            id: "condition".to_owned(),
            name: "condition".to_owned(),
            ty: BodyType::Named("bool".to_owned()),
        }];
        body.blocks = vec![
            BodyBlock {
                id: "entry".to_owned(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: BodyTerminator::ConditionalBranch {
                    condition: "condition".to_owned(),
                    then_target: "left".to_owned(),
                    then_arguments: Vec::new(),
                    else_target: "right".to_owned(),
                    else_arguments: Vec::new(),
                },
            },
            BodyBlock {
                id: "left".to_owned(),
                parameters: Vec::new(),
                operations: vec![BodyOperation {
                    id: "left-value".to_owned(),
                    kind: BodyOperationKind::Constant {
                        value: 1,
                        ty: BodyType::Integer(IntegerType {
                            bits: 32,
                            signed: true,
                        }),
                    },
                    operands: Vec::new(),
                    results: vec![BodyValue {
                        id: "left-value".to_owned(),
                        ty: BodyType::Integer(IntegerType {
                            bits: 32,
                            signed: true,
                        }),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                }],
                terminator: BodyTerminator::Branch {
                    target: "join".to_owned(),
                    arguments: Vec::new(),
                },
            },
            BodyBlock {
                id: "right".to_owned(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: BodyTerminator::Branch {
                    target: "join".to_owned(),
                    arguments: Vec::new(),
                },
            },
            BodyBlock {
                id: "join".to_owned(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: BodyTerminator::Return {
                    values: vec!["left-value".to_owned()],
                },
            },
        ];
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB032"));
    }

    #[test]
    fn body_rejects_unreachable_blocks_explicitly() {
        let mut program = executable_program();
        let body = program.functions[0].body.as_mut().expect("body");
        body.blocks.push(BodyBlock {
            id: "dead".to_owned(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: BodyTerminator::Return {
                values: vec!["a".to_owned()],
            },
        });
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB038"));
    }
}
