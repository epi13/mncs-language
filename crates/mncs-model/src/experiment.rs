//! Language-owned contracts for bounded MNCS source experiments.
//!
//! Forge may persist and compare these records and Fabric may transport their
//! frozen workloads, but neither layer owns their semantic interpretation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::canonical::{canonical_json_value, sha256_hex};
use crate::{
    BackendArtifact, BackendCapabilityManifest, BackendIdentity, CompilationStudyResult,
    ExecutionCorpus, ExecutionStatus, ExecutionValue, RealizationRequest, SemanticId,
    TargetLoweringPlan, TranslationJudgement, TranslationValidationResult,
};

pub const LANGUAGE_EXPERIMENT_SCHEMA_VERSION: &str = "0.1";
pub const LANGUAGE_EXPERIMENT_DEFINITION_CONTRACT_ID: &str =
    "mncs:language:experiment-definition:0.1";
pub const LANGUAGE_EXPERIMENT_RESULT_CONTRACT_ID: &str = "mncs:language:experiment-result:0.1";
pub const LANGUAGE_EXPERIMENT_INTERPRETATION: &str =
    "bounded_language_observation_not_universal_equivalence_or_conformance";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LanguageExperimentStatus {
    Pass,
    Fail,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRequirement {
    pub relation: String,
    pub validator_profile: String,
    pub required: bool,
    pub allow_unknown: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageExperimentDefinition {
    pub schema_version: String,
    pub contract_id: String,
    pub identity: SemanticId,
    pub name: String,
    pub source_artifact_identity: String,
    pub source_profile: String,
    pub compiler_identity: SemanticId,
    pub compiler_configuration: BTreeMap<String, String>,
    pub realization: RealizationRequest,
    pub corpus: ExecutionCorpus,
    pub validators: Vec<ValidatorRequirement>,
    pub requested_result_artifacts: Vec<String>,
    pub expected_invariants: Vec<String>,
    pub comparison_relation: String,
    pub environment_observations_are_non_semantic: bool,
    pub parent_experiment: Option<SemanticId>,
    pub provenance: Vec<SemanticId>,
}

impl LanguageExperimentDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        source_artifact_identity: impl Into<String>,
        source_profile: impl Into<String>,
        compiler_identity: SemanticId,
        compiler_configuration: BTreeMap<String, String>,
        realization: RealizationRequest,
        corpus: ExecutionCorpus,
        mut validators: Vec<ValidatorRequirement>,
        mut requested_result_artifacts: Vec<String>,
        mut expected_invariants: Vec<String>,
        comparison_relation: impl Into<String>,
        parent_experiment: Option<SemanticId>,
        mut provenance: Vec<SemanticId>,
    ) -> Self {
        validators.sort_by(|left, right| {
            (&left.relation, &left.validator_profile)
                .cmp(&(&right.relation, &right.validator_profile))
        });
        requested_result_artifacts.sort();
        requested_result_artifacts.dedup();
        expected_invariants.sort();
        expected_invariants.dedup();
        provenance.sort();
        provenance.dedup();
        let mut definition = Self {
            schema_version: LANGUAGE_EXPERIMENT_SCHEMA_VERSION.to_owned(),
            contract_id: LANGUAGE_EXPERIMENT_DEFINITION_CONTRACT_ID.to_owned(),
            identity: SemanticId(String::new()),
            name: name.into(),
            source_artifact_identity: source_artifact_identity.into(),
            source_profile: source_profile.into(),
            compiler_identity,
            compiler_configuration,
            realization,
            corpus,
            validators,
            requested_result_artifacts,
            expected_invariants,
            comparison_relation: comparison_relation.into(),
            environment_observations_are_non_semantic: true,
            parent_experiment,
            provenance,
        };
        definition.seal();
        definition
    }

    pub fn seal(&mut self) {
        self.identity = SemanticId(String::new());
        self.identity = experiment_identity("definition", self);
    }

    pub fn identity_is_valid(&self) -> bool {
        let mut material = self.clone();
        material.identity = SemanticId(String::new());
        self.schema_version == LANGUAGE_EXPERIMENT_SCHEMA_VERSION
            && self.contract_id == LANGUAGE_EXPERIMENT_DEFINITION_CONTRACT_ID
            && !self.name.trim().is_empty()
            && !self.source_artifact_identity.trim().is_empty()
            && !self.source_profile.trim().is_empty()
            && self.realization.identity_is_valid()
            && !self.corpus.cases.is_empty()
            && self.environment_observations_are_non_semantic
            && self.identity == experiment_identity("definition", &material)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageExperimentCaseObservation {
    pub case_id: String,
    pub status: ExecutionStatus,
    pub returned: Vec<ExecutionValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<Vec<ExecutionValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expectation_met: Option<bool>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageExperimentPropertyObservation {
    pub property_id: String,
    pub law: String,
    pub passed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counterexample: Option<Vec<ExecutionValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageExperimentResult {
    pub schema_version: String,
    pub contract_id: String,
    pub identity: SemanticId,
    pub definition_identity: SemanticId,
    pub definition: LanguageExperimentDefinition,
    pub compiler_study: CompilationStudyResult,
    pub backend: BackendIdentity,
    pub backend_capabilities: BackendCapabilityManifest,
    pub realization_plan: TargetLoweringPlan,
    pub artifact: BackendArtifact,
    pub translation_validations: Vec<TranslationValidationResult>,
    pub cases: Vec<LanguageExperimentCaseObservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<LanguageExperimentPropertyObservation>,
    pub status: LanguageExperimentStatus,
    pub unresolved_reasons: Vec<String>,
    pub interpretation: String,
}

impl LanguageExperimentResult {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        definition: LanguageExperimentDefinition,
        compiler_study: CompilationStudyResult,
        backend_capabilities: BackendCapabilityManifest,
        realization_plan: TargetLoweringPlan,
        artifact: BackendArtifact,
        mut translation_validations: Vec<TranslationValidationResult>,
        mut cases: Vec<LanguageExperimentCaseObservation>,
        mut properties: Vec<LanguageExperimentPropertyObservation>,
        mut unresolved_reasons: Vec<String>,
    ) -> Self {
        translation_validations.sort_by(|left, right| left.identity.cmp(&right.identity));
        cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
        properties.sort_by(|left, right| left.property_id.cmp(&right.property_id));
        unresolved_reasons.sort();
        unresolved_reasons.dedup();
        let has_failure = translation_validations
            .iter()
            .any(|validation| validation.judgement == TranslationJudgement::Fail)
            || cases.iter().any(|case_| {
                matches!(
                    case_.status,
                    ExecutionStatus::InvalidRequest | ExecutionStatus::Unsupported
                )
            })
            || cases
                .iter()
                .any(|case_| case_.expectation_met == Some(false))
            || properties.iter().any(|property| !property.passed);
        let has_unknown = !unresolved_reasons.is_empty()
            || translation_validations
                .iter()
                .any(|validation| validation.judgement == TranslationJudgement::Unknown);
        let status = if has_failure {
            LanguageExperimentStatus::Fail
        } else if has_unknown {
            LanguageExperimentStatus::Unknown
        } else {
            LanguageExperimentStatus::Pass
        };
        let backend = artifact.backend.clone();
        let mut result = Self {
            schema_version: LANGUAGE_EXPERIMENT_SCHEMA_VERSION.to_owned(),
            contract_id: LANGUAGE_EXPERIMENT_RESULT_CONTRACT_ID.to_owned(),
            identity: SemanticId(String::new()),
            definition_identity: definition.identity.clone(),
            definition,
            compiler_study,
            backend,
            backend_capabilities,
            realization_plan,
            artifact,
            translation_validations,
            cases,
            properties,
            status,
            unresolved_reasons,
            interpretation: LANGUAGE_EXPERIMENT_INTERPRETATION.to_owned(),
        };
        result.seal();
        result
    }

    pub fn seal(&mut self) {
        self.identity = SemanticId(String::new());
        self.identity = experiment_identity("result", self);
    }

    pub fn identity_is_valid(&self) -> bool {
        let mut material = self.clone();
        material.identity = SemanticId(String::new());
        self.schema_version == LANGUAGE_EXPERIMENT_SCHEMA_VERSION
            && self.contract_id == LANGUAGE_EXPERIMENT_RESULT_CONTRACT_ID
            && self.interpretation == LANGUAGE_EXPERIMENT_INTERPRETATION
            && self.definition_identity == self.definition.identity
            && self.definition.identity_is_valid()
            && self.compiler_study.identity_is_valid()
            && self.backend == self.artifact.backend
            && self.backend_capabilities.backend == self.backend
            && self.backend_capabilities.identity_is_valid()
            && self.realization_plan.identity_is_valid()
            && self.realization_plan.identity
                == self
                    .compiler_study
                    .realization_plan_identity
                    .clone()
                    .unwrap_or_else(|| self.realization_plan.identity.clone())
            && self.artifact.identity_is_valid()
            && self
                .translation_validations
                .iter()
                .all(TranslationValidationResult::identity_is_valid)
            && self.identity == experiment_identity("result", &material)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageExperimentComparison {
    pub schema_version: String,
    pub left: SemanticId,
    pub right: SemanticId,
    pub same_source: bool,
    pub same_semantics: bool,
    pub same_hir: bool,
    pub same_ssa: bool,
    pub same_realization_request: bool,
    pub same_backend: bool,
    pub same_backend_artifact: bool,
    pub bounded_behavior_agrees: bool,
    pub earliest_divergence: Option<String>,
    pub interpretation: String,
}

impl LanguageExperimentComparison {
    pub fn compare(left: &LanguageExperimentResult, right: &LanguageExperimentResult) -> Self {
        let same_source =
            left.definition.source_artifact_identity == right.definition.source_artifact_identity;
        let same_semantics =
            left.compiler_study.semantic_fingerprint == right.compiler_study.semantic_fingerprint;
        let same_hir = left.compiler_study.hir_fingerprint == right.compiler_study.hir_fingerprint;
        let same_ssa = left.compiler_study.ssa_fingerprint == right.compiler_study.ssa_fingerprint;
        let same_realization_request =
            left.definition.realization.identity == right.definition.realization.identity;
        let same_backend = left.backend == right.backend;
        let same_backend_artifact = left.artifact.identity == right.artifact.identity;
        let left_cases = left
            .cases
            .iter()
            .map(|case_| (&case_.case_id, (&case_.status, &case_.returned)))
            .collect::<BTreeMap<_, _>>();
        let right_cases = right
            .cases
            .iter()
            .map(|case_| (&case_.case_id, (&case_.status, &case_.returned)))
            .collect::<BTreeMap<_, _>>();
        let left_properties = left
            .properties
            .iter()
            .map(|property| (&property.property_id, property.passed))
            .collect::<BTreeMap<_, _>>();
        let right_properties = right
            .properties
            .iter()
            .map(|property| (&property.property_id, property.passed))
            .collect::<BTreeMap<_, _>>();
        let bounded_behavior_agrees =
            left_cases == right_cases && left_properties == right_properties;
        let earliest_divergence = [
            ("source", same_source),
            ("semantic", same_semantics),
            ("hir", same_hir),
            ("ssa", same_ssa),
            ("realization_request", same_realization_request),
            ("backend", same_backend),
            ("backend_artifact", same_backend_artifact),
            ("bounded_behavior", bounded_behavior_agrees),
        ]
        .into_iter()
        .find_map(|(stage, equal)| (!equal).then(|| stage.to_owned()));
        Self {
            schema_version: LANGUAGE_EXPERIMENT_SCHEMA_VERSION.to_owned(),
            left: left.identity.clone(),
            right: right.identity.clone(),
            same_source,
            same_semantics,
            same_hir,
            same_ssa,
            same_realization_request,
            same_backend,
            same_backend_artifact,
            bounded_behavior_agrees,
            earliest_divergence,
            interpretation: LANGUAGE_EXPERIMENT_INTERPRETATION.to_owned(),
        }
    }
}

fn experiment_identity<T: Serialize>(kind: &str, value: &T) -> SemanticId {
    let canonical = canonical_json_value(value).expect("experiment record is canonicalizable");
    SemanticId(format!(
        "mncs:language:experiment:{kind}:{}",
        sha256_hex(canonical.as_bytes())
    ))
}
