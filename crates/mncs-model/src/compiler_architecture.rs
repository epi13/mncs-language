//! Machine-readable contracts for the logical compiler stage architecture.
//!
//! A reference implementation may fuse stages internally, but it must not
//! erase them from the architecture or imply that a planned stage exists.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::canonical::{canonical_json_value, sha256_hex};
use crate::SemanticId;

pub const COMPILER_STAGE_ARCHITECTURE_SCHEMA_VERSION: &str = "0.1";
pub const COMPILER_STAGE_ARCHITECTURE_CONTRACT_ID: &str =
    "mncs:language:compiler-stage-architecture:0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilerStage {
    SourceIntentRepresentation,
    LexicalAnalysis,
    Parsing,
    ConcreteSyntaxTree,
    AbstractSyntaxTree,
    SemanticGraphConstruction,
    IdentityResolution,
    TypeAndContractAnalysis,
    HighLevelIr,
    VerificationPasses,
    LoweringBackendGeneration,
    ExecutableArtifact,
}

impl CompilerStage {
    pub const ORDERED: [Self; 12] = [
        Self::SourceIntentRepresentation,
        Self::LexicalAnalysis,
        Self::Parsing,
        Self::ConcreteSyntaxTree,
        Self::AbstractSyntaxTree,
        Self::SemanticGraphConstruction,
        Self::IdentityResolution,
        Self::TypeAndContractAnalysis,
        Self::HighLevelIr,
        Self::VerificationPasses,
        Self::LoweringBackendGeneration,
        Self::ExecutableArtifact,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageAvailability {
    Available,
    Experimental,
    Partial,
    BootstrapOnly,
    Planned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageIntegration {
    CompilerDriver,
    LanguageLibrary,
    ExternalBootstrap,
    NotIntegrated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerStageContract {
    pub stage: CompilerStage,
    pub availability: StageAvailability,
    pub integration: StageIntegration,
    pub input_contracts: Vec<String>,
    pub output_contracts: Vec<String>,
    pub deterministic_output_required: bool,
    pub owner: String,
    pub decisions: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerArchitectureContract {
    pub schema_version: String,
    pub contract_id: String,
    pub identity: SemanticId,
    pub stages: Vec<CompilerStageContract>,
}

impl CompilerArchitectureContract {
    pub fn new(stages: Vec<CompilerStageContract>) -> Self {
        let mut contract = Self {
            schema_version: COMPILER_STAGE_ARCHITECTURE_SCHEMA_VERSION.to_owned(),
            contract_id: COMPILER_STAGE_ARCHITECTURE_CONTRACT_ID.to_owned(),
            identity: SemanticId(String::new()),
            stages,
        };
        contract.seal();
        contract
    }

    pub fn seal(&mut self) {
        self.identity = SemanticId(String::new());
        let material =
            canonical_json_value(self).expect("compiler architecture contract is canonicalizable");
        self.identity = SemanticId(format!(
            "mncs:compiler:stage-architecture:{}",
            sha256_hex(material.as_bytes())
        ));
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let actual = self
            .stages
            .iter()
            .map(|stage| stage.stage)
            .collect::<Vec<_>>();
        if actual != CompilerStage::ORDERED {
            errors.push("compiler stages must appear exactly once in canonical order".to_owned());
        }
        if self
            .stages
            .iter()
            .any(|stage| stage.owner.trim().is_empty())
        {
            errors.push("every compiler stage must declare an owning contract boundary".to_owned());
        }
        let unique = actual.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != actual.len() {
            errors.push("compiler stage contract contains duplicate stages".to_owned());
        }
        if self.schema_version != COMPILER_STAGE_ARCHITECTURE_SCHEMA_VERSION
            || self.contract_id != COMPILER_STAGE_ARCHITECTURE_CONTRACT_ID
        {
            errors.push("compiler stage architecture contract identity is unsupported".to_owned());
        }
        let mut material = self.clone();
        material.identity = SemanticId(String::new());
        let canonical = canonical_json_value(&material)
            .expect("compiler architecture contract is canonicalizable");
        let expected = SemanticId(format!(
            "mncs:compiler:stage-architecture:{}",
            sha256_hex(canonical.as_bytes())
        ));
        if self.identity != expected {
            errors.push("compiler stage architecture content identity is stale".to_owned());
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_stage_contract_that_hides_a_logical_stage() {
        let contract = CompilerArchitectureContract::new(Vec::new());
        assert!(contract
            .validate()
            .iter()
            .any(|error| error.contains("canonical order")));
    }
}
