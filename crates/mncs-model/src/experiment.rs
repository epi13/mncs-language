//! Language-owned contracts for bounded MNCS source experiments.
//!
//! Forge may persist and compare these records and Fabric may transport their
//! frozen workloads, but neither layer owns their semantic interpretation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::canonical::{canonical_json_value, sha256_hex};
use crate::{
    BackendArtifact, BackendCapabilityManifest, BackendIdentity, CompilationStudyResult,
    ExecutionCorpus, ExecutionEffectEvent, ExecutionStatus, ExecutionValue,
    ExpectedEffectObservation, RealizationRequest, SemanticId, TargetLoweringPlan,
    TranslationJudgement, TranslationValidationResult,
};

pub const LANGUAGE_EXPERIMENT_SCHEMA_VERSION: &str = "0.1";
pub const LANGUAGE_EXPERIMENT_DEFINITION_CONTRACT_ID: &str =
    "mncs:language:experiment-definition:0.1";
pub const LANGUAGE_EXPERIMENT_RESULT_CONTRACT_ID: &str = "mncs:language:experiment-result:0.1";
pub const LANGUAGE_EXPERIMENT_INTERPRETATION: &str =
    "bounded_language_observation_not_universal_equivalence_or_conformance";
pub const FAMILY_EXPERIMENT_REFERENCE_SCHEMA_VERSION: &str =
    "mncs.family-record.producer-reference.v0.1";

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_status: Option<ExecutionStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_met: Option<bool>,
    pub steps: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_steps: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_bound_met: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<ExecutionEffectEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_effects: Vec<ExpectedEffectObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects_met: Option<bool>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyExperimentObservation {
    pub source_artifact_identity: String,
    pub source_profile: String,
    pub compiler_identity: SemanticId,
    pub pipeline_identity: SemanticId,
    pub semantic_fingerprint: Option<String>,
    pub hir_fingerprint: Option<String>,
    pub ssa_fingerprint: Option<String>,
    pub selected_ssa_identity: Option<SemanticId>,
    pub realization_request_identity: SemanticId,
    pub realization_plan_identity: SemanticId,
    pub backend_identity: SemanticId,
    pub backend_artifact_identity: SemanticId,
    pub backend_artifact_kind: String,
    pub execution_corpus_digest: String,
    pub validator_identities: Vec<SemanticId>,
    pub experiment_definition_identity: SemanticId,
    pub experiment_result_identity: SemanticId,
    pub status: LanguageExperimentStatus,
    pub unresolved_obligations: Vec<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyExperimentReference {
    pub schema: String,
    pub producer: String,
    pub record_kind: String,
    pub schema_version: String,
    pub stable_id: String,
    pub content_digest: String,
    pub artifact: crate::FamilyArtifactReference,
    pub scope: BTreeMap<String, String>,
    pub experiment: FamilyExperimentObservation,
    pub authority_boundary: String,
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
            || cases.iter().any(|case_| case_.status_met == Some(false))
            || cases
                .iter()
                .any(|case_| case_.step_bound_met == Some(false))
            || cases.iter().any(|case_| case_.effects_met == Some(false))
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

    pub fn family_reference(&self) -> FamilyExperimentReference {
        let canonical = canonical_json_value(self)
            .expect("language experiment result is canonical JSON serializable");
        let content_digest = sha256_hex(canonical.as_bytes());
        let corpus = canonical_json_value(&self.definition.corpus)
            .expect("execution corpus is canonical JSON serializable");
        let mut validator_identities = self
            .definition
            .validators
            .iter()
            .map(|validator| {
                let material = format!(
                    "{}:{}:{}:{}",
                    validator.relation,
                    validator.validator_profile,
                    validator.required,
                    validator.allow_unknown
                );
                SemanticId(format!(
                    "mncs:language:validator:{}",
                    sha256_hex(material.as_bytes())
                ))
            })
            .collect::<Vec<_>>();
        validator_identities.sort();
        let mut scope = BTreeMap::from([
            (
                "source_profile".to_owned(),
                self.definition.source_profile.clone(),
            ),
            ("backend".to_owned(), self.backend.identity.0.clone()),
        ]);
        scope.insert("experiment".to_owned(), self.definition_identity.0.clone());
        FamilyExperimentReference {
            schema: FAMILY_EXPERIMENT_REFERENCE_SCHEMA_VERSION.to_owned(),
            producer: "mncs-language".to_owned(),
            record_kind: "LanguageExperimentResult".to_owned(),
            schema_version: self.schema_version.clone(),
            stable_id: self.identity.0.clone(),
            content_digest: content_digest.clone(),
            artifact: crate::FamilyArtifactReference {
                identity: self.identity.0.clone(),
                kind: "language-experiment-result".to_owned(),
                digest: content_digest,
            },
            scope,
            experiment: FamilyExperimentObservation {
                source_artifact_identity: self.definition.source_artifact_identity.clone(),
                source_profile: self.definition.source_profile.clone(),
                compiler_identity: self.compiler_study.compiler_identity.clone(),
                pipeline_identity: self.compiler_study.pipeline_identity.clone(),
                semantic_fingerprint: self.compiler_study.semantic_fingerprint.clone(),
                hir_fingerprint: self.compiler_study.hir_fingerprint.clone(),
                ssa_fingerprint: self.compiler_study.ssa_fingerprint.clone(),
                selected_ssa_identity: self.compiler_study.selected_ssa_identity.clone(),
                realization_request_identity: self.definition.realization.identity.clone(),
                realization_plan_identity: self.realization_plan.identity.clone(),
                backend_identity: self.backend.identity.clone(),
                backend_artifact_identity: self.artifact.identity.clone(),
                backend_artifact_kind: self.artifact.artifact_kind.clone(),
                execution_corpus_digest: sha256_hex(corpus.as_bytes()),
                validator_identities,
                experiment_definition_identity: self.definition_identity.clone(),
                experiment_result_identity: self.identity.clone(),
                status: self.status,
                unresolved_obligations: self.compiler_study.unresolved_obligations.clone(),
            },
            authority_boundary: "Observational language-owned bounded result only; this reference does not claim universal equivalence, conformance, independent evaluation, target assurance, or production fitness.".to_owned(),
        }
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
            .map(|case_| {
                (
                    &case_.case_id,
                    (
                        &case_.status,
                        &case_.returned,
                        case_.expectation_met,
                        case_.status_met,
                        case_.step_bound_met,
                        case_.effects_met,
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let right_cases = right
            .cases
            .iter()
            .map(|case_| {
                (
                    &case_.case_id,
                    (
                        &case_.status,
                        &case_.returned,
                        case_.expectation_met,
                        case_.status_met,
                        case_.step_bound_met,
                        case_.effects_met,
                    ),
                )
            })
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
