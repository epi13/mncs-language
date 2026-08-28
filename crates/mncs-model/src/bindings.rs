//! Versioned semantic binding records shared by elaboration, tooling, and IR.
//!
//! Source coordinates are retained only as diagnostic/navigation metadata. The
//! identities in this module are derived from semantic owners and stable
//! structural slots, so a resolver does not turn a span, hash-map order, or
//! host path into meaning.

use serde::{Deserialize, Serialize};

use crate::SemanticId;

pub const SEMANTIC_BINDING_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticBindingKind {
    Function,
    Parameter,
    Local,
    IterationState,
    FiniteType,
    FiniteVariant,
    RecordType,
    RecordField,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionProvenance {
    Local,
    DirectImport,
    Qualified,
    Aliased,
    TransitiveImport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticNamespace {
    pub identity: SemanticId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticScope {
    pub identity: SemanticId,
    pub namespace: SemanticId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<SemanticId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<SemanticId>,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticBinding {
    pub identity: SemanticId,
    pub spelling: String,
    pub namespace: SemanticId,
    pub scope: SemanticId,
    pub declaration: SemanticId,
    pub kind: SemanticBindingKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticReference {
    pub identity: SemanticId,
    pub spelling: String,
    /// Navigation metadata; never used to derive `identity` or `binding`.
    pub occurrence_start: usize,
    pub occurrence_end: usize,
    pub namespace: SemanticId,
    pub scope: SemanticId,
    pub binding: SemanticId,
    /// Canonical semantic route, such as `alias -> module -> declaration`.
    pub path: Vec<String>,
    pub provenance: ResolutionProvenance,
}

/// The authoritative, compact binding substrate for one elaborated program.
/// Vectors are canonicalized by identity before publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticBindingTable {
    pub schema_version: String,
    pub namespaces: Vec<SemanticNamespace>,
    pub scopes: Vec<SemanticScope>,
    pub bindings: Vec<SemanticBinding>,
    pub references: Vec<SemanticReference>,
}

impl SemanticBindingTable {
    pub fn new(
        mut namespaces: Vec<SemanticNamespace>,
        mut scopes: Vec<SemanticScope>,
        mut bindings: Vec<SemanticBinding>,
        mut references: Vec<SemanticReference>,
    ) -> Self {
        namespaces.sort_by(|left, right| left.identity.cmp(&right.identity));
        namespaces.dedup_by(|left, right| left.identity == right.identity);
        scopes.sort_by(|left, right| left.identity.cmp(&right.identity));
        scopes.dedup_by(|left, right| left.identity == right.identity);
        bindings.sort_by(|left, right| left.identity.cmp(&right.identity));
        bindings.dedup_by(|left, right| left.identity == right.identity);
        references.sort_by(|left, right| left.identity.cmp(&right.identity));
        references.dedup_by(|left, right| left.identity == right.identity);
        Self {
            schema_version: SEMANTIC_BINDING_SCHEMA_VERSION.to_owned(),
            namespaces,
            scopes,
            bindings,
            references,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.namespaces.is_empty()
            && self.scopes.is_empty()
            && self.bindings.is_empty()
            && self.references.is_empty()
    }
}
