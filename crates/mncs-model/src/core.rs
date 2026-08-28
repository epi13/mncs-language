use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SUPPORTED_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Program {
    pub schema_version: String,
    pub module: String,
    /// Semantic identities of the modules this program binds against, in
    /// deterministic (sorted) order. Empty for self-contained programs.
    #[serde(default)]
    pub dependencies: Vec<crate::SemanticId>,
    #[serde(default)]
    pub finite_types: Vec<FiniteType>,
    #[serde(default)]
    pub record_types: Vec<RecordType>,
    #[serde(default)]
    pub assumptions: Vec<Assumption>,
    /// Authoritative scope/binding/reference evidence produced by source
    /// elaboration. Declaration-only JSON programs may omit this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_table: Option<crate::SemanticBindingTable>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FiniteType {
    pub identity: crate::SemanticId,
    pub name: String,
    pub variants: Vec<FiniteVariant>,
}

/// A logical record type: a labeled product with stable semantic field
/// identities.  Field order is canonicalized by field name so that two
/// source declarations naming the same fields with the same types denote one
/// logical type regardless of source order.  Logical identity never encodes
/// physical layout; representation is a separately evidenced backend choice.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordType {
    pub identity: crate::SemanticId,
    pub name: String,
    /// Fields sorted by name; each names a scalar, finite, or nested record
    /// semantic type.
    pub fields: Vec<RecordField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FiniteVariant {
    pub identity: crate::SemanticId,
    pub name: String,
    pub discriminant: u32,
    /// Payload fields (Profile 0.6 sums). Empty for payload-free variants;
    /// serialized form of payload-free variants is unchanged. Fields are kept
    /// in canonical (sorted) name order; logical field identity is separate
    /// from any backend layout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payload: Vec<RecordField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    /// Namespace of the module that declared this function. `None` means the
    /// program's own module. Linked imports carry their declaring module so
    /// their semantic identities stay anchored to their home namespace.
    #[serde(default)]
    pub home_module: Option<String>,
    #[serde(default)]
    pub inputs: Vec<Value>,
    #[serde(default)]
    pub outputs: Vec<Value>,
    #[serde(default)]
    pub contracts: Vec<ContractClause>,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<EvidenceClaim>,
    #[serde(default)]
    pub failure: FailureMode,
    /// Optional syntax-independent executable semantics. Omitted bodies use
    /// the declaration-only 0.1 compatibility path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<crate::FunctionBody>,
}

impl Function {
    /// The namespace this function's semantic identity is anchored to: its
    /// declaring module when it arrived through linking, otherwise the
    /// program's own module.
    pub fn identity_namespace<'a>(&'a self, program_module: &'a str) -> &'a str {
        self.home_module.as_deref().unwrap_or(program_module)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Value {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractClause {
    pub id: String,
    pub kind: ContractKind,
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractKind {
    Requires,
    Ensures,
    Invariant,
    Preserves,
    Budget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Effect {
    pub kind: String,
    pub target: String,
    /// Capability that authorizes this effect. Every effect must name one.
    pub capability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Assumption {
    pub id: String,
    pub statement: String,
    pub supplied_by: String,
    #[serde(default)]
    pub confidence: AssumptionConfidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AssumptionConfidence {
    #[default]
    External,
    Observed,
    Tested,
    Verified,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceClaim {
    pub property: String,
    pub verifier: String,
    pub status: EvidenceStatus,
    #[serde(default)]
    pub artifact: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Claimed,
    Tested,
    Analyzed,
    Verified,
    ExternallyVerified,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FailureMode {
    #[default]
    Isolated,
    Atomic,
    Partial,
    Compensating,
    Fatal,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid semantic manifest: {0}")]
    Json(#[from] serde_json::Error),
}

impl Program {
    pub fn from_json(input: &str) -> Result<Self, ParseError> {
        Ok(serde_json::from_str(input)?)
    }
}
