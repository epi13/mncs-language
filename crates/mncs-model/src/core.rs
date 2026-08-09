use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SUPPORTED_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Program {
    pub schema_version: String,
    pub module: String,
    #[serde(default)]
    pub assumptions: Vec<Assumption>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Function {
    pub name: String,
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
