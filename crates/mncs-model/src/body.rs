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
/// Semantic vectors and masks are deliberately bounded in the first Profile
/// 0.8 tranche. Lane count is logical identity and never a register width.
pub const MAX_VECTOR_LANES: u32 = 64;

/// The declared length facts of a bounded sequence type. `Exact` sequences
/// carry their length statically; `UpTo` views carry a maximum and a runtime
/// length observation. The bound is part of logical type identity.
/// Profile 0.10 adds value-parameterized bounds (`Param`) where the length
/// is a compile-time `Nat` instantiation argument.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequenceBound {
    Exact(u32),
    UpTo(u32),
    /// Generic value parameter as exact bound: `[E; N]`
    Param(String),
    /// Generic value parameter as view capacity: `[E; up_to N]`
    UpToParam(String),
}

impl SequenceBound {
    /// The static ceiling this bound contributes to traversal step counts.
    /// For generic params the ceiling is the profile maximum; instantiation
    /// will substitute a concrete bound that must respect it.
    pub fn ceiling(&self) -> u32 {
        match self {
            Self::Exact(length) | Self::UpTo(length) => *length,
            Self::Param(_) | Self::UpToParam(_) => MAX_SEQUENCE_BOUND,
        }
    }

    pub fn canonical_text(&self) -> String {
        match self {
            Self::Exact(length) => format!("{length}"),
            Self::UpTo(capacity) => format!("up_to {capacity}"),
            Self::Param(name) => name.clone(),
            Self::UpToParam(name) => format!("up_to {name}"),
        }
    }

    pub fn is_generic(&self) -> bool {
        matches!(self, Self::Param(_) | Self::UpToParam(_))
    }

    pub fn generic_param_name(&self) -> Option<&str> {
        match self {
            Self::Param(n) | Self::UpToParam(n) => Some(n.as_str()),
            _ => None,
        }
    }
}

/// The kind of a generic parameter (RFC 0013): type vs value/index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenericParamKind {
    Type,
    Nat,
}

/// Declaration of a generic parameter. Stored with its declaring function
/// body; kind distinguishes `T : Type` from `N : Nat`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenericParam {
    pub name: String,
    pub kind: GenericParamKind,
}

fn is_valid_ident(text: &str) -> bool {
    !text.is_empty() && {
        let mut chars = text.chars();
        chars
            .next()
            .map(|c| c == '_' || c.is_alphabetic())
            .unwrap_or(false)
            && chars.all(|c| c == '_' || c.is_alphanumeric())
    }
}

/// A value substituted for a generic `Nat` parameter or a type substituted
/// for a generic `Type` parameter (RFC 0013, Profile 0.10). The variant
/// distinguishes the semantic kind so value/type substitution identity stays
/// stable across encodings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum GenericArg {
    Type {
        ty: BodyType,
    },
    Value {
        value: u32,
    },
    /// A value-parameter reference used when a generic caller forwards its
    /// own `Nat` parameter as a generic argument to another generic.
    ValueParam {
        name: String,
    },
}

impl GenericArg {
    pub fn canonical_string(&self) -> String {
        match self {
            Self::Type { ty } => format!("type:{}", ty.canonical_identity()),
            Self::Value { value } => format!("value:{value}"),
            Self::ValueParam { name } => format!("value_param:{name}"),
        }
    }

    pub fn is_concrete(&self) -> bool {
        !matches!(self, Self::ValueParam { .. })
            && match self {
                Self::Type { ty } => !ty.contains_generic_parameter(),
                _ => true,
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
    /// Generic parameters for this function (Profile 0.10). Empty for
    /// non-generic functions; serialized absent when empty so pre-0.10
    /// artifacts canonicalize identically.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generic_params: Vec<GenericParam>,
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
    /// The source sequence bound, when this is a sequence traversal. Keeping
    /// it separate from `bound` lets generic specialization replace `N`
    /// precisely instead of guessing from the profile ceiling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence_bound: Option<SequenceBound>,
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
    /// A machine-independent vector of logical lanes (Profile 0.8). This is
    /// not a fixed sequence and does not prescribe a physical register count.
    Vector {
        element: Box<BodyType>,
        lanes: u32,
    },
    /// A logical collection of lane predicates (Profile 0.8). Physical
    /// realizations may be packed bits, vector booleans, or predicate state.
    Mask {
        lanes: u32,
    },
    Finite {
        identity: SemanticId,
        name: String,
    },
    Record {
        identity: SemanticId,
        name: String,
    },
    /// A generic type parameter reference `T` (Profile 0.10, RFC 0013).
    /// Carries the parameter name only; resolution knows its declaring
    /// generic function. Distinct from `Named` so substitution identity is
    /// precise and aliases cannot accidentally collapse.
    GenericParam {
        name: String,
    },
}

impl BodyType {
    pub fn from_semantic_name(name: &str) -> Self {
        if let Some(vector) = Self::parse_vector_name(name) {
            return vector;
        }
        if let Some(mask) = Self::parse_mask_name(name) {
            return mask;
        }
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

    /// Resolve a semantic type spelling against a program's declared
    /// records, finites, and nested bounded-sequence structure.
    ///
    /// `[Point; N]` becomes a sequence whose element is the declared
    /// `Point` record, not a leftover `Named("Point")` spelling. ABI
    /// classification without a program still uses [`Self::from_semantic_name`].
    pub fn from_program(program: &Program, name: &str) -> Self {
        if let Some(finite_type) = program
            .finite_types
            .iter()
            .find(|finite_type| finite_type.identity.0 == name)
        {
            return Self::Finite {
                identity: finite_type.identity.clone(),
                name: finite_type.name.clone(),
            };
        }
        if let Some(record_type) = program
            .record_types
            .iter()
            .find(|record_type| record_type.identity.0 == name)
        {
            return Self::Record {
                identity: record_type.identity.clone(),
                name: record_type.name.clone(),
            };
        }
        if let Some(sequence) = semantic_sequence_type(program, name) {
            return sequence;
        }
        if let Some(record_type) = program.record_types.iter().find(|item| item.name == name) {
            return Self::Record {
                identity: record_type.identity.clone(),
                name: record_type.name.clone(),
            };
        }
        program
            .finite_types
            .iter()
            .find(|finite_type| finite_type.name == name)
            .map(|finite_type| Self::Finite {
                identity: finite_type.identity.clone(),
                name: finite_type.name.clone(),
            })
            .unwrap_or_else(|| Self::from_semantic_name(name))
    }

    /// Parse a canonical bounded-sequence spelling `[E; N]` or
    /// `[E; up_to M]`. Only canonical spellings parse; anything else stays
    /// an ordinary named type so downstream diagnostics stay truthful.
    /// Generic value-parameter bounds `N` / `up_to N` are accepted and
    /// represented as parameterised bounds.
    fn parse_sequence_name(name: &str) -> Option<Self> {
        let inner = name.strip_prefix('[')?.strip_suffix(']')?;
        let separator = inner.rfind(';')?;
        let (element_text, bound_text) = (inner[..separator].trim(), inner[separator + 1..].trim());
        let bound = if let Some(capacity) = bound_text.strip_prefix("up_to") {
            let cap_text = capacity.trim();
            if let Ok(capacity) = cap_text.parse::<u32>() {
                if capacity > MAX_SEQUENCE_BOUND {
                    return None;
                }
                SequenceBound::UpTo(capacity)
            } else if is_valid_ident(cap_text) {
                SequenceBound::UpToParam(cap_text.to_owned())
            } else {
                return None;
            }
        } else if let Ok(length) = bound_text.parse::<u32>() {
            if length > MAX_SEQUENCE_BOUND {
                return None;
            }
            SequenceBound::Exact(length)
        } else if is_valid_ident(bound_text) {
            SequenceBound::Param(bound_text.to_owned())
        } else {
            return None;
        };
        if element_text.is_empty() {
            return None;
        }
        let element = Box::new(Self::from_semantic_name(element_text));
        // Nominal elements (`bool`, declared records/finites) stay Named
        // until a program resolves them. The spelling is still a sequence.
        Some(Self::Sequence { element, bound })
    }

    fn parse_vector_name(name: &str) -> Option<Self> {
        let inner = name.strip_prefix("vec<")?.strip_suffix('>')?;
        let separator = inner.rfind(',')?;
        let element = Self::from_semantic_name(inner[..separator].trim());
        if !matches!(element, Self::Integer(_)) {
            return None;
        }
        let lanes = inner[separator + 1..].trim().parse::<u32>().ok()?;
        if lanes == 0 || lanes > MAX_VECTOR_LANES {
            return None;
        }
        Some(Self::Vector {
            element: Box::new(element),
            lanes,
        })
    }

    fn parse_mask_name(name: &str) -> Option<Self> {
        let lanes = name
            .strip_prefix("mask<")?
            .strip_suffix('>')?
            .trim()
            .parse::<u32>()
            .ok()?;
        (lanes > 0 && lanes <= MAX_VECTOR_LANES).then_some(Self::Mask { lanes })
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
            Self::Vector { element, lanes } => {
                format!("vec<{}, {lanes}>", element.semantic_name())
            }
            Self::Mask { lanes } => format!("mask<{lanes}>"),
            Self::Finite { name, .. } => name.clone(),
            Self::Record { name, .. } => name.clone(),
            Self::GenericParam { name } => name.clone(),
        }
    }

    /// Stable type identity for generic instantiation keys. `semantic_name`
    /// is source-friendly and omits nominal identity; specialization keys
    /// must retain it so same-named records from two modules cannot collide.
    pub fn canonical_identity(&self) -> String {
        match self {
            Self::Named(name) => format!("named:{name}"),
            Self::Integer(integer) => format!(
                "integer:{}:{}",
                if integer.signed { "signed" } else { "unsigned" },
                integer.bits
            ),
            Self::Byte => "byte".to_owned(),
            Self::Sequence { element, bound } => format!(
                "sequence<{};{}>",
                element.canonical_identity(),
                bound.canonical_text()
            ),
            Self::Vector { element, lanes } => {
                format!("vector<{},{}>", element.canonical_identity(), lanes)
            }
            Self::Mask { lanes } => format!("mask<{lanes}>"),
            Self::Finite { identity, .. } => format!("finite:{}", identity.0),
            Self::Record { identity, .. } => format!("record:{}", identity.0),
            Self::GenericParam { name } => format!("generic:{name}"),
        }
    }

    pub fn contains_generic_parameter(&self) -> bool {
        match self {
            Self::GenericParam { .. } => true,
            Self::Sequence { element, bound } => {
                element.contains_generic_parameter() || bound.is_generic()
            }
            Self::Vector { element, .. } => element.contains_generic_parameter(),
            _ => false,
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
    VectorConstruct {
        element_type: Box<BodyType>,
        lanes: u32,
    },
    VectorSplat {
        element_type: Box<BodyType>,
        lanes: u32,
    },
    VectorExtract {
        element_type: Box<BodyType>,
        lanes: u32,
        evidence: BoundsEvidence,
    },
    VectorReplace {
        element_type: Box<BodyType>,
        lanes: u32,
        evidence: BoundsEvidence,
    },
    /// Lane-wise integer operation. Arithmetic intent applies independently
    /// to every lane; one checked/trapping failure fails the whole operation.
    VectorBinary {
        operator: String,
        element_type: Box<BodyType>,
        lanes: u32,
        intent: ArithmeticIntent,
    },
    VectorCompare {
        predicate: String,
        element_type: Box<BodyType>,
        lanes: u32,
    },
    MaskBinary {
        operator: String,
        lanes: u32,
    },
    MaskNot {
        lanes: u32,
    },
    MaskReduce {
        operator: String,
        lanes: u32,
    },
    VectorReduce {
        operator: String,
        element_type: Box<BodyType>,
        lanes: u32,
        intent: ArithmeticIntent,
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
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        generic_args: Vec<GenericArg>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instantiation: Option<SemanticId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        specialization: Option<SemanticId>,
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
                let expected_ty =
                    body_type_for_function_value(program, function, &input.value_type);
                if parameter.name != input.name || parameter.ty != expected_ty {
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
        let cfg = crate::Cfg::from_body(
            self,
            self.identity(function.identity_namespace(&program.module), &function.name),
        );
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
                let unresolved_named = matches!(
                    element_type.as_ref(),
                    BodyType::Named(name) if name != "bool"
                );
                if **element_type == expected_counter || unresolved_named {
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
                bound: bound.clone(),
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
            let condition_type = match operand_type.as_ref() {
                BodyType::Vector { lanes, .. } => BodyType::Mask { lanes: *lanes },
                _ => BodyType::Named("bool".to_owned()),
            };
            if available.get(operation.operands[0].as_str()) != Some(&condition_type) {
                errors.push(body_diagnostic(
                    "MNB103",
                    format!("{path}.operands[0]"),
                    "selection condition must be bool, or a lane-matched mask for vectors",
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
                bound: bound.clone(),
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
        BodyOperationKind::VectorConstruct {
            element_type,
            lanes,
        } => {
            let vector = BodyType::Vector {
                element: element_type.clone(),
                lanes: *lanes,
            };
            if *lanes == 0
                || *lanes > MAX_VECTOR_LANES
                || operation.operands.len() != *lanes as usize
                || operation.results.len() != 1
                || operation
                    .operands
                    .iter()
                    .any(|operand| available.get(operand) != Some(element_type.as_ref()))
                || operation
                    .results
                    .first()
                    .is_none_or(|result| result.ty != vector)
            {
                errors.push(body_diagnostic("MNB112", path.to_owned(), "vector construction must have one correctly typed operand per bounded lane and one matching vector result"));
            }
        }
        BodyOperationKind::VectorSplat {
            element_type,
            lanes,
        } => {
            let vector = BodyType::Vector {
                element: element_type.clone(),
                lanes: *lanes,
            };
            if operation.operands.len() != 1
                || operation.results.len() != 1
                || available.get(operation.operands.first().unwrap_or(&String::new()))
                    != Some(element_type.as_ref())
                || operation
                    .results
                    .first()
                    .is_none_or(|result| result.ty != vector)
            {
                errors.push(body_diagnostic(
                    "MNB113",
                    path.to_owned(),
                    "vector splat requires one element and one matching vector result",
                ));
            }
        }
        BodyOperationKind::VectorExtract {
            element_type,
            lanes,
            ..
        } => {
            let vector = BodyType::Vector {
                element: element_type.clone(),
                lanes: *lanes,
            };
            let counter = BodyType::Integer(IntegerType {
                bits: 64,
                signed: false,
            });
            if operation.operands.len() != 2
                || operation.results.len() != 1
                || available.get(operation.operands.first().unwrap_or(&String::new()))
                    != Some(&vector)
                || available.get(operation.operands.get(1).unwrap_or(&String::new()))
                    != Some(&counter)
                || operation
                    .results
                    .first()
                    .is_none_or(|result| &result.ty != element_type.as_ref())
            {
                errors.push(body_diagnostic("MNB114", path.to_owned(), "vector extraction requires a vector, u64 lane index, and matching element result"));
            }
        }
        BodyOperationKind::VectorReplace {
            element_type,
            lanes,
            ..
        } => {
            let vector = BodyType::Vector {
                element: element_type.clone(),
                lanes: *lanes,
            };
            let counter = BodyType::Integer(IntegerType {
                bits: 64,
                signed: false,
            });
            if operation.operands.len() != 3
                || operation.results.len() != 1
                || available.get(operation.operands.first().unwrap_or(&String::new()))
                    != Some(&vector)
                || available.get(operation.operands.get(1).unwrap_or(&String::new()))
                    != Some(&counter)
                || available.get(operation.operands.get(2).unwrap_or(&String::new()))
                    != Some(element_type.as_ref())
                || operation
                    .results
                    .first()
                    .is_none_or(|result| result.ty != vector)
            {
                errors.push(body_diagnostic(
                    "MNB115",
                    path.to_owned(),
                    "vector replacement must preserve vector type and use a u64 lane index",
                ));
            }
        }
        BodyOperationKind::VectorBinary {
            operator,
            element_type,
            lanes,
            ..
        } => {
            let vector = BodyType::Vector {
                element: element_type.clone(),
                lanes: *lanes,
            };
            if !matches!(element_type.as_ref(), BodyType::Integer(_))
                || !matches!(
                    operator.as_str(),
                    "add" | "sub" | "mul" | "and" | "or" | "xor" | "shl" | "shr" | "min" | "max"
                )
                || operation.operands.len() != 2
                || operation.results.len() != 1
                || operation
                    .operands
                    .iter()
                    .any(|operand| available.get(operand) != Some(&vector))
                || operation
                    .results
                    .first()
                    .is_none_or(|result| result.ty != vector)
            {
                errors.push(body_diagnostic("MNB116", path.to_owned(), "vector binary operation requires two identical integer vectors and one matching result"));
            }
        }
        BodyOperationKind::VectorCompare {
            predicate,
            element_type,
            lanes,
        } => {
            let vector = BodyType::Vector {
                element: element_type.clone(),
                lanes: *lanes,
            };
            let mask = BodyType::Mask { lanes: *lanes };
            if !matches!(element_type.as_ref(), BodyType::Integer(_))
                || !matches!(predicate.as_str(), "eq" | "ne" | "lt" | "le" | "gt" | "ge")
                || operation.operands.len() != 2
                || operation.results.len() != 1
                || operation
                    .operands
                    .iter()
                    .any(|operand| available.get(operand) != Some(&vector))
                || operation
                    .results
                    .first()
                    .is_none_or(|result| result.ty != mask)
            {
                errors.push(body_diagnostic("MNB117", path.to_owned(), "vector comparison requires matching integer vectors and produces a lane-matched mask"));
            }
        }
        BodyOperationKind::MaskBinary { operator, lanes } => {
            let mask = BodyType::Mask { lanes: *lanes };
            if !matches!(operator.as_str(), "and" | "or" | "xor")
                || operation.operands.len() != 2
                || operation.results.len() != 1
                || operation
                    .operands
                    .iter()
                    .any(|operand| available.get(operand) != Some(&mask))
                || operation
                    .results
                    .first()
                    .is_none_or(|result| result.ty != mask)
            {
                errors.push(body_diagnostic(
                    "MNB118",
                    path.to_owned(),
                    "mask binary operation requires matching masks and preserves lane count",
                ));
            }
        }
        BodyOperationKind::MaskNot { lanes } => {
            let mask = BodyType::Mask { lanes: *lanes };
            if operation.operands.len() != 1
                || operation.results.len() != 1
                || available.get(operation.operands.first().unwrap_or(&String::new()))
                    != Some(&mask)
                || operation
                    .results
                    .first()
                    .is_none_or(|result| result.ty != mask)
            {
                errors.push(body_diagnostic(
                    "MNB119",
                    path.to_owned(),
                    "mask not requires one mask and preserves lane count",
                ));
            }
        }
        BodyOperationKind::MaskReduce { operator, lanes } => {
            let mask = BodyType::Mask { lanes: *lanes };
            let boolean = BodyType::Named("bool".to_owned());
            if !matches!(operator.as_str(), "any" | "all" | "none")
                || operation.operands.len() != 1
                || operation.results.len() != 1
                || available.get(operation.operands.first().unwrap_or(&String::new()))
                    != Some(&mask)
                || operation
                    .results
                    .first()
                    .is_none_or(|result| result.ty != boolean)
            {
                errors.push(body_diagnostic(
                    "MNB120",
                    path.to_owned(),
                    "mask reduction requires one mask and produces bool",
                ));
            }
        }
        BodyOperationKind::VectorReduce {
            operator,
            element_type,
            lanes,
            ..
        } => {
            let vector = BodyType::Vector {
                element: element_type.clone(),
                lanes: *lanes,
            };
            if !matches!(operator.as_str(), "sum" | "min" | "max")
                || operation.operands.len() != 1
                || operation.results.len() != 1
                || available.get(operation.operands.first().unwrap_or(&String::new()))
                    != Some(&vector)
                || operation
                    .results
                    .first()
                    .is_none_or(|result| &result.ty != element_type.as_ref())
            {
                errors.push(body_diagnostic(
                    "MNB121",
                    path.to_owned(),
                    "vector reduction requires one integer vector and produces one element",
                ));
            }
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
                bound: view_bound.clone(),
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
            generic_args,
            ..
        } => {
            // Call targets may live in this program's own namespace or in any
            // bound dependency namespace; identities stay anchored to the
            // declaring module either way.
            let simple_name = function_name.rsplit('.').next().unwrap_or(function_name);
            let callee = program.functions.iter().find(|candidate| {
                candidate.name == simple_name
                    && crate::identity::function_id(
                        candidate.identity_namespace(&program.module),
                        &candidate.name,
                    ) == *callee_identity
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
            // Generic call validation: substitution for callee's generic params
            let is_generic_callee = !callee.generic_params.is_empty();
            if is_generic_callee && generic_args.is_empty() {
                errors.push(body_diagnostic(
                    "MNB075",
                    format!("{path}.kind.generic_args"),
                    "generic function requires explicit type/value arguments; unresolved generic call must not reach backend lowering",
                ));
            } else if !is_generic_callee && !generic_args.is_empty() {
                errors.push(body_diagnostic(
                    "MNB076",
                    format!("{path}.kind.generic_args"),
                    "non-generic function cannot be called with generic arguments",
                ));
            }
            if is_generic_callee {
                if generic_args.len() != callee.generic_params.len() {
                    errors.push(body_diagnostic(
                        "MNB077",
                        format!("{path}.kind.generic_args"),
                        "generic call argument count does not match the callee declaration",
                    ));
                }
                for (index, (param, arg)) in
                    callee.generic_params.iter().zip(generic_args).enumerate()
                {
                    let kind_matches = match (&param.kind, arg) {
                        (GenericParamKind::Type, GenericArg::Type { ty }) => {
                            !generic_type_references_are_foreign(ty, function)
                        }
                        (GenericParamKind::Nat, GenericArg::Value { .. }) => true,
                        (GenericParamKind::Nat, GenericArg::ValueParam { name }) => {
                            function.generic_params.iter().any(|candidate| {
                                candidate.name == *name && candidate.kind == GenericParamKind::Nat
                            })
                        }
                        _ => false,
                    };
                    if !kind_matches {
                        errors.push(body_diagnostic(
                            "MNB078",
                            format!("{path}.kind.generic_args[{index}]"),
                            "generic call argument kind or forwarding scope does not match the callee parameter",
                        ));
                    }
                }
            }
            // Helper to compute expected callee parameter/output types after generic substitution
            let expected_callee_input = |idx: usize| -> BodyType {
                let param = &callee.inputs[idx];
                let base = body_type_for_function_value(program, callee, &param.value_type);
                if generic_args.is_empty() {
                    return base;
                }
                // Build substitution maps from callee generic params and call args
                let mut type_map: std::collections::BTreeMap<String, BodyType> =
                    std::collections::BTreeMap::new();
                let mut value_concrete: std::collections::BTreeMap<String, u32> =
                    std::collections::BTreeMap::new();
                let mut value_param_map: std::collections::BTreeMap<String, String> =
                    std::collections::BTreeMap::new();
                for (gp, arg) in callee.generic_params.iter().zip(generic_args.iter()) {
                    match (&gp.kind, arg) {
                        (GenericParamKind::Type, GenericArg::Type { ty }) => {
                            type_map.insert(gp.name.clone(), ty.clone());
                        }
                        (GenericParamKind::Nat, GenericArg::Value { value }) => {
                            value_concrete.insert(gp.name.clone(), *value);
                        }
                        (GenericParamKind::Nat, GenericArg::ValueParam { name }) => {
                            value_param_map.insert(gp.name.clone(), name.clone());
                        }
                        _ => {}
                    }
                }
                substitute_body_type_for_call(base, &type_map, &value_concrete, &value_param_map)
            };
            let expected_callee_output = |idx: usize| -> BodyType {
                let out = &callee.outputs[idx];
                let base = body_type_for_function_value(program, callee, &out.value_type);
                if generic_args.is_empty() {
                    return base;
                }
                let mut type_map: std::collections::BTreeMap<String, BodyType> =
                    std::collections::BTreeMap::new();
                let mut value_concrete: std::collections::BTreeMap<String, u32> =
                    std::collections::BTreeMap::new();
                let mut value_param_map: std::collections::BTreeMap<String, String> =
                    std::collections::BTreeMap::new();
                for (gp, arg) in callee.generic_params.iter().zip(generic_args.iter()) {
                    match (&gp.kind, arg) {
                        (GenericParamKind::Type, GenericArg::Type { ty }) => {
                            type_map.insert(gp.name.clone(), ty.clone());
                        }
                        (GenericParamKind::Nat, GenericArg::Value { value }) => {
                            value_concrete.insert(gp.name.clone(), *value);
                        }
                        (GenericParamKind::Nat, GenericArg::ValueParam { name }) => {
                            value_param_map.insert(gp.name.clone(), name.clone());
                        }
                        _ => {}
                    }
                }
                substitute_body_type_for_call(base, &type_map, &value_concrete, &value_param_map)
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
            for (idx, (operand, _parameter)) in
                operation.operands.iter().zip(&callee.inputs).enumerate()
            {
                let expected = expected_callee_input(idx);
                if available.get(operand) != Some(&expected) {
                    errors.push(body_diagnostic(
                        "MNB051",
                        format!("{path}.operands"),
                        "call argument type does not match the callee parameter",
                    ));
                }
            }
            for (idx, (result, _output)) in
                operation.results.iter().zip(&callee.outputs).enumerate()
            {
                let expected = expected_callee_output(idx);
                if result.ty != expected {
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
        let expected =
            body_type_for_function_value(program, function, &function.outputs[index].value_type);
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
    BodyType::from_program(program, name)
}

fn generic_type_references_are_foreign(ty: &BodyType, function: &Function) -> bool {
    match ty {
        BodyType::GenericParam { name } => !function
            .generic_params
            .iter()
            .any(|param| param.name == *name && param.kind == GenericParamKind::Type),
        BodyType::Sequence { element, .. } | BodyType::Vector { element, .. } => {
            generic_type_references_are_foreign(element, function)
        }
        _ => false,
    }
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
        BodyType::Named(name) if name != "bool" => None,
        _ => Some(BodyType::Sequence { element, bound }),
    }
}

/// Resolve a declared record field's semantic body type at validation time.
fn semantic_record_field_type(program: &Program, field: &crate::core::RecordField) -> BodyType {
    semantic_body_type(program, &field.field_type)
}

fn body_type_for_function_value(
    program: &Program,
    function: &Function,
    value_type: &str,
) -> BodyType {
    if let Some(gp) = function
        .generic_params
        .iter()
        .find(|p| p.name == value_type && p.kind == GenericParamKind::Type)
    {
        return BodyType::GenericParam {
            name: gp.name.clone(),
        };
    }
    if value_type.trim_start().starts_with('[') {
        let base = BodyType::from_program(program, value_type);
        // from_program for "[Point; N]" where Point is a record and N is a value param
        // will return Sequence with Named("Point") and Param("N") because
        // semantic_sequence_type doesn't handle Param bounds. So we need to
        // handle generic case via from_semantic_name fallback.
        let base = if let BodyType::Sequence { .. } = base {
            base
        } else {
            // Try generic-aware parsing for "[T; N]" where T is a type param
            let alt = BodyType::from_semantic_name(value_type);
            if let BodyType::Sequence { element, bound } = alt {
                // Check if element is a generic param or a record that from_semantic_name left as Named
                BodyType::Sequence { element, bound }
            } else {
                alt
            }
        };
        if let BodyType::Sequence { element, bound } = base {
            let new_element = match *element {
                BodyType::Named(n)
                    if function
                        .generic_params
                        .iter()
                        .any(|p| p.name == n && p.kind == GenericParamKind::Type) =>
                {
                    Box::new(BodyType::GenericParam { name: n })
                }
                BodyType::Named(n) => {
                    if let Some(rec) = program
                        .record_types
                        .iter()
                        .find(|r| r.identity.0 == n || r.name == n)
                    {
                        Box::new(BodyType::Record {
                            identity: rec.identity.clone(),
                            name: rec.name.clone(),
                        })
                    } else if let Some(fin) = program
                        .finite_types
                        .iter()
                        .find(|f| f.identity.0 == n || f.name == n)
                    {
                        Box::new(BodyType::Finite {
                            identity: fin.identity.clone(),
                            name: fin.name.clone(),
                        })
                    } else {
                        Box::new(BodyType::Named(n))
                    }
                }
                other => Box::new(other),
            };
            // Validate bound param kind if generic
            if let Some(param_name) = bound.generic_param_name() {
                if !function
                    .generic_params
                    .iter()
                    .any(|p| p.name == param_name && p.kind == GenericParamKind::Nat)
                {
                    // Let validation error be emitted elsewhere; keep bound as is
                }
            }
            return BodyType::Sequence {
                element: new_element,
                bound,
            };
        }
        // If from_program didn't return a Sequence (e.g., for generic [T; N] where T is generic, from_program returns Named("T") not Record, but still Sequence with Named("T"))
        // Fall through to generic handling already done via from_program's Named("T") case, but we already handled generic element above.
        // For safety, try from_semantic_name as fallback for generic bounds.
        let parsed = BodyType::from_semantic_name(value_type);
        if let BodyType::Sequence { element, bound } = parsed {
            let new_element = match *element {
                BodyType::Named(n)
                    if function
                        .generic_params
                        .iter()
                        .any(|p| p.name == n && p.kind == GenericParamKind::Type) =>
                {
                    Box::new(BodyType::GenericParam { name: n })
                }
                BodyType::Named(n) => program
                    .record_types
                    .iter()
                    .find(|record| record.identity.0 == n || record.name == n)
                    .map(|record| {
                        Box::new(BodyType::Record {
                            identity: record.identity.clone(),
                            name: record.name.clone(),
                        })
                    })
                    .or_else(|| {
                        program
                            .finite_types
                            .iter()
                            .find(|finite| finite.identity.0 == n || finite.name == n)
                            .map(|finite| {
                                Box::new(BodyType::Finite {
                                    identity: finite.identity.clone(),
                                    name: finite.name.clone(),
                                })
                            })
                    })
                    .unwrap_or_else(|| Box::new(BodyType::Named(n))),
                other => Box::new(other),
            };
            return BodyType::Sequence {
                element: new_element,
                bound,
            };
        }
    }
    // For non-generic or generic param used as value type error will be caught elsewhere
    semantic_body_type(program, value_type)
}

fn substitute_body_type_for_call(
    ty: BodyType,
    type_map: &std::collections::BTreeMap<String, BodyType>,
    value_concrete: &std::collections::BTreeMap<String, u32>,
    value_param_map: &std::collections::BTreeMap<String, String>,
) -> BodyType {
    match ty {
        BodyType::GenericParam { name } => type_map
            .get(&name)
            .cloned()
            .unwrap_or(BodyType::GenericParam { name }),
        BodyType::Sequence { element, bound } => {
            let new_elem = Box::new(substitute_body_type_for_call(
                *element,
                type_map,
                value_concrete,
                value_param_map,
            ));
            let new_bound = match bound {
                SequenceBound::Exact(v) => SequenceBound::Exact(v),
                SequenceBound::UpTo(v) => SequenceBound::UpTo(v),
                SequenceBound::Param(n) => {
                    if let Some(v) = value_concrete.get(&n) {
                        SequenceBound::Exact(*v)
                    } else if let Some(caller) = value_param_map.get(&n) {
                        SequenceBound::Param(caller.clone())
                    } else {
                        SequenceBound::Param(n)
                    }
                }
                SequenceBound::UpToParam(n) => {
                    if let Some(v) = value_concrete.get(&n) {
                        SequenceBound::UpTo(*v)
                    } else if let Some(caller) = value_param_map.get(&n) {
                        SequenceBound::UpToParam(caller.clone())
                    } else {
                        SequenceBound::UpToParam(n)
                    }
                }
            };
            BodyType::Sequence {
                element: new_elem,
                bound: new_bound,
            }
        }
        BodyType::Vector { element, lanes } => BodyType::Vector {
            element: Box::new(substitute_body_type_for_call(
                *element,
                type_map,
                value_concrete,
                value_param_map,
            )),
            lanes,
        },
        other => other,
    }
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
    use crate::{FailureMode, RecordField, RecordType, Value, SUPPORTED_SCHEMA_VERSION};

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
            generic_params: Vec::new(),
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

    #[test]
    fn sequence_spellings_keep_bool_and_nominal_elements() {
        assert_eq!(
            BodyType::from_semantic_name("[bool; 4]"),
            BodyType::Sequence {
                element: Box::new(BodyType::Named("bool".to_owned())),
                bound: SequenceBound::Exact(4),
            }
        );
        assert_eq!(
            BodyType::from_semantic_name("[Point; up_to 4]"),
            BodyType::Sequence {
                element: Box::new(BodyType::Named("Point".to_owned())),
                bound: SequenceBound::UpTo(4),
            }
        );
        assert_eq!(
            BodyType::from_semantic_name("[[i64; 2]; 3]"),
            BodyType::Sequence {
                element: Box::new(BodyType::Sequence {
                    element: Box::new(BodyType::Integer(IntegerType {
                        bits: 64,
                        signed: true,
                    })),
                    bound: SequenceBound::Exact(2),
                }),
                bound: SequenceBound::Exact(3),
            }
        );
    }

    #[test]
    fn from_program_resolves_nested_record_sequence_elements() {
        let mut program = executable_program();
        let point_id =
            SemanticId("mncs:0.2:record-type:m::Point::column%3Ai64%3Brow%3Ai64%3B".to_owned());
        program.record_types.push(RecordType {
            identity: point_id.clone(),
            name: "Point".to_owned(),
            fields: vec![
                RecordField {
                    name: "column".to_owned(),
                    field_type: "i64".to_owned(),
                },
                RecordField {
                    name: "row".to_owned(),
                    field_type: "i64".to_owned(),
                },
            ],
        });
        let resolved = BodyType::from_program(&program, "[Point; 4]");
        assert_eq!(
            resolved,
            BodyType::Sequence {
                element: Box::new(BodyType::Record {
                    identity: point_id,
                    name: "Point".to_owned(),
                }),
                bound: SequenceBound::Exact(4),
            }
        );
        assert_eq!(
            BodyType::from_program(&program, "[bool; 2]"),
            BodyType::Sequence {
                element: Box::new(BodyType::Named("bool".to_owned())),
                bound: SequenceBound::Exact(2),
            }
        );
    }
}
