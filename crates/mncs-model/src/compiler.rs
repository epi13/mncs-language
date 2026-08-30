//! Versioned, deterministic contracts for evidence-bearing compilation.
//!
//! These artifacts describe compiler derivations. They do not redefine the
//! semantic program, HIR, or SSA representations owned elsewhere in this
//! crate, and they do not make a successful compiler invocation proof of a
//! transformation claim.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::canonical::{canonical_json_value, sha256_hex};
use crate::{CanonicalForm, HighLevelIr, ObligationRecord, SemanticId, SsaModule};

pub const PORTABLE_WASM_MVP_TARGET: &str = "mncs:target:portable-wasm-mvp-0.1";
pub const PORTABLE_WASM_MVP_BACKEND_NAME: &str = "mncs-portable-wasm-mvp";
pub const PORTABLE_WASM_MVP_BACKEND_VERSION: &str = "0.1";
pub const BACKEND_ARTIFACT_SCHEMA_VERSION: &str = "0.2";
pub const BACKEND_CAPABILITY_SCHEMA_VERSION: &str = "0.1";
pub const REALIZATION_REQUEST_SCHEMA_VERSION: &str = "0.1";
pub const LAYERED_EXECUTION_COMPARISON_INTERPRETATION: &str =
    "empirical_bounded_agreement_not_universal_equivalence";

pub const COMPILER_ARTIFACT_SCHEMA_VERSION: &str = "0.1";
pub const COMPILATION_STUDY_RESULT_CONTRACT_ID: &str = "mncs:language:compilation-study-result:0.1";
pub const FAMILY_COMPILER_REFERENCE_SCHEMA_VERSION: &str =
    "mncs-language.family-compiler-reference.v0.1";
pub const COMPILATION_STUDY_OBSERVATION_INTERPRETATION: &str =
    "observation_only_not_assurance_or_conformance";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRepresentation {
    Source,
    LexicalTokens,
    ConcreteSyntaxTree,
    AbstractSyntaxTree,
    Semantic,
    SemanticGraph,
    IdentityMap,
    Validation,
    Hir,
    Ssa,
    SelectedSsa,
    TargetLoweringPlan,
    BackendArtifact,
    EvidenceBundle,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CompilerArtifactRef {
    pub identity: SemanticId,
    pub representation: ArtifactRepresentation,
    pub schema_version: String,
    pub fingerprint: String,
}

impl CompilerArtifactRef {
    pub fn new(
        representation: ArtifactRepresentation,
        schema_version: impl Into<String>,
        fingerprint: impl Into<String>,
    ) -> Self {
        let schema_version = schema_version.into();
        let fingerprint = fingerprint.into();
        let identity = artifact_identity(representation, &schema_version, &fingerprint);
        Self {
            identity,
            representation,
            schema_version,
            fingerprint,
        }
    }

    pub fn identity_is_valid(&self) -> bool {
        !self.schema_version.trim().is_empty()
            && !self.fingerprint.trim().is_empty()
            && self.identity
                == artifact_identity(self.representation, &self.schema_version, &self.fingerprint)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerImplementationIdentity {
    pub identity: SemanticId,
    pub implementation: String,
    pub version: String,
    pub artifact_fingerprint: Option<String>,
}

impl CompilerImplementationIdentity {
    pub fn new(
        implementation: impl Into<String>,
        version: impl Into<String>,
        artifact_fingerprint: Option<String>,
    ) -> Self {
        let implementation = implementation.into();
        let version = version.into();
        let identity = identified(
            "compiler",
            &(&implementation, &version, &artifact_fingerprint),
        );
        Self {
            identity,
            implementation,
            version,
            artifact_fingerprint,
        }
    }

    pub fn identity_is_valid(&self) -> bool {
        !self.implementation.trim().is_empty()
            && !self.version.trim().is_empty()
            && self.identity
                == identified(
                    "compiler",
                    &(
                        &self.implementation,
                        &self.version,
                        &self.artifact_fingerprint,
                    ),
                )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerHostIdentity {
    pub identity: SemanticId,
    pub architecture: String,
    pub operating_system: String,
    pub environment: String,
    pub properties: BTreeMap<String, String>,
}

impl CompilerHostIdentity {
    pub fn new(
        architecture: impl Into<String>,
        operating_system: impl Into<String>,
        environment: impl Into<String>,
        properties: BTreeMap<String, String>,
    ) -> Self {
        let architecture = architecture.into();
        let operating_system = operating_system.into();
        let environment = environment.into();
        let identity = identified(
            "compiler-host",
            &(&architecture, &operating_system, &environment, &properties),
        );
        Self {
            identity,
            architecture,
            operating_system,
            environment,
            properties,
        }
    }

    pub fn identity_is_valid(&self) -> bool {
        self.identity
            == identified(
                "compiler-host",
                &(
                    &self.architecture,
                    &self.operating_system,
                    &self.environment,
                    &self.properties,
                ),
            )
    }

    pub fn is_complete(&self) -> bool {
        !self.architecture.trim().is_empty()
            && !self.operating_system.trim().is_empty()
            && !self.environment.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildHostIdentity {
    pub identity: SemanticId,
    pub architecture: String,
    pub operating_system: String,
    pub environment: String,
    pub toolchain: String,
    pub properties: BTreeMap<String, String>,
}

impl BuildHostIdentity {
    pub fn new(
        architecture: impl Into<String>,
        operating_system: impl Into<String>,
        environment: impl Into<String>,
        toolchain: impl Into<String>,
        properties: BTreeMap<String, String>,
    ) -> Self {
        let architecture = architecture.into();
        let operating_system = operating_system.into();
        let environment = environment.into();
        let toolchain = toolchain.into();
        let identity = identified(
            "build-host",
            &(
                &architecture,
                &operating_system,
                &environment,
                &toolchain,
                &properties,
            ),
        );
        Self {
            identity,
            architecture,
            operating_system,
            environment,
            toolchain,
            properties,
        }
    }

    pub fn identity_is_valid(&self) -> bool {
        self.identity
            == identified(
                "build-host",
                &(
                    &self.architecture,
                    &self.operating_system,
                    &self.environment,
                    &self.toolchain,
                    &self.properties,
                ),
            )
    }

    pub fn is_complete(&self) -> bool {
        !self.architecture.trim().is_empty()
            && !self.operating_system.trim().is_empty()
            && !self.environment.trim().is_empty()
            && !self.toolchain.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetContractRef {
    pub identity: SemanticId,
    pub candidate: String,
    pub facts: BTreeMap<String, String>,
    pub evidence: Vec<SemanticId>,
    pub assumptions: Vec<String>,
}

impl TargetContractRef {
    pub fn new(
        candidate: impl Into<String>,
        facts: BTreeMap<String, String>,
        mut evidence: Vec<SemanticId>,
        mut assumptions: Vec<String>,
    ) -> Self {
        let candidate = candidate.into();
        evidence.sort();
        evidence.dedup();
        assumptions.sort();
        assumptions.dedup();
        let identity = identified(
            "target-contract",
            &(&candidate, &facts, &evidence, &assumptions),
        );
        Self {
            identity,
            candidate,
            facts,
            evidence,
            assumptions,
        }
    }

    pub fn identity_is_valid(&self) -> bool {
        let mut evidence = self.evidence.clone();
        let mut assumptions = self.assumptions.clone();
        evidence.sort();
        evidence.dedup();
        assumptions.sort();
        assumptions.dedup();
        !self.candidate.trim().is_empty()
            && self.identity
                == identified(
                    "target-contract",
                    &(&self.candidate, &self.facts, &evidence, &assumptions),
                )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEnvironmentRef {
    pub identity: SemanticId,
    pub architecture: String,
    pub operating_system: String,
    pub environment: String,
    pub facts: BTreeMap<String, String>,
}

impl RunEnvironmentRef {
    pub fn new(
        architecture: impl Into<String>,
        operating_system: impl Into<String>,
        environment: impl Into<String>,
        facts: BTreeMap<String, String>,
    ) -> Self {
        let architecture = architecture.into();
        let operating_system = operating_system.into();
        let environment = environment.into();
        let identity = identified(
            "run-environment",
            &(&architecture, &operating_system, &environment, &facts),
        );
        Self {
            identity,
            architecture,
            operating_system,
            environment,
            facts,
        }
    }

    pub fn identity_is_valid(&self) -> bool {
        !self.architecture.trim().is_empty()
            && !self.operating_system.trim().is_empty()
            && !self.environment.trim().is_empty()
            && self.identity
                == identified(
                    "run-environment",
                    &(
                        &self.architecture,
                        &self.operating_system,
                        &self.environment,
                        &self.facts,
                    ),
                )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationModelRef {
    pub identity: SemanticId,
    pub name: String,
    pub version: String,
    pub observable_kinds: BTreeSet<String>,
}

impl ObservationModelRef {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        observable_kinds: BTreeSet<String>,
    ) -> Self {
        let name = name.into();
        let version = version.into();
        let identity = identified("observation-model", &(&name, &version, &observable_kinds));
        Self {
            identity,
            name,
            version,
            observable_kinds,
        }
    }

    pub fn identity_is_valid(&self) -> bool {
        !self.name.trim().is_empty()
            && !self.version.trim().is_empty()
            && self.identity
                == identified(
                    "observation-model",
                    &(&self.name, &self.version, &self.observable_kinds),
                )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationClaim {
    pub kind: String,
    pub observation_model: ObservationModelRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerPassIdentity {
    pub identity: SemanticId,
    pub id: String,
    pub version: String,
    pub input: ArtifactRepresentation,
    pub output: ArtifactRepresentation,
    pub claimed_relation: RelationClaim,
    pub required_invariants: Vec<String>,
    pub required_target_facts: Vec<String>,
    pub consumed_assumptions: Vec<String>,
    pub generated_obligation_classes: Vec<String>,
    pub validator_profile: String,
    pub fallback_policy: String,
}

impl CompilerPassIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        input: ArtifactRepresentation,
        output: ArtifactRepresentation,
        claimed_relation: RelationClaim,
        mut required_invariants: Vec<String>,
        mut required_target_facts: Vec<String>,
        mut consumed_assumptions: Vec<String>,
        mut generated_obligation_classes: Vec<String>,
        validator_profile: impl Into<String>,
        fallback_policy: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let version = version.into();
        let validator_profile = validator_profile.into();
        let fallback_policy = fallback_policy.into();
        sort_dedup(&mut required_invariants);
        sort_dedup(&mut required_target_facts);
        sort_dedup(&mut consumed_assumptions);
        sort_dedup(&mut generated_obligation_classes);
        let identity = identified(
            "compiler-pass",
            &(
                &id,
                &version,
                input,
                output,
                &claimed_relation,
                &required_invariants,
                &required_target_facts,
                &consumed_assumptions,
                &generated_obligation_classes,
                &validator_profile,
                &fallback_policy,
            ),
        );
        Self {
            identity,
            id,
            version,
            input,
            output,
            claimed_relation,
            required_invariants,
            required_target_facts,
            consumed_assumptions,
            generated_obligation_classes,
            validator_profile,
            fallback_policy,
        }
    }

    pub fn identity_is_valid(&self) -> bool {
        let rebuilt = Self::new(
            self.id.clone(),
            self.version.clone(),
            self.input,
            self.output,
            self.claimed_relation.clone(),
            self.required_invariants.clone(),
            self.required_target_facts.clone(),
            self.consumed_assumptions.clone(),
            self.generated_obligation_classes.clone(),
            self.validator_profile.clone(),
            self.fallback_policy.clone(),
        );
        self.claimed_relation.observation_model.identity_is_valid()
            && self.identity == rebuilt.identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassPipelineIdentity {
    pub identity: SemanticId,
    pub id: String,
    pub version: String,
    pub passes: Vec<CompilerPassIdentity>,
}

impl PassPipelineIdentity {
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        passes: Vec<CompilerPassIdentity>,
    ) -> Self {
        let id = id.into();
        let version = version.into();
        let identities = passes.iter().map(|pass| &pass.identity).collect::<Vec<_>>();
        let identity = identified("pass-pipeline", &(&id, &version, identities));
        Self {
            identity,
            id,
            version,
            passes,
        }
    }

    pub fn identity_is_valid(&self) -> bool {
        self.passes
            .iter()
            .all(CompilerPassIdentity::identity_is_valid)
            && self.identity
                == Self::new(self.id.clone(), self.version.clone(), self.passes.clone()).identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransformationStatus {
    Pass,
    Fail,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformationEdge {
    pub identity: SemanticId,
    pub input: CompilerArtifactRef,
    pub output: CompilerArtifactRef,
    pub pass: CompilerPassIdentity,
    pub claimed_relation: RelationClaim,
    pub assumptions_consumed: Vec<String>,
    pub obligations_generated: Vec<SemanticId>,
    pub evidence: Vec<SemanticId>,
    pub status: TransformationStatus,
    pub provenance: Vec<SemanticId>,
}

impl TransformationEdge {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        input: CompilerArtifactRef,
        output: CompilerArtifactRef,
        pass: CompilerPassIdentity,
        mut assumptions_consumed: Vec<String>,
        mut obligations_generated: Vec<SemanticId>,
        mut evidence: Vec<SemanticId>,
        status: TransformationStatus,
        mut provenance: Vec<SemanticId>,
    ) -> Self {
        sort_dedup(&mut assumptions_consumed);
        obligations_generated.sort();
        obligations_generated.dedup();
        evidence.sort();
        evidence.dedup();
        provenance.sort();
        provenance.dedup();
        let claimed_relation = pass.claimed_relation.clone();
        let identity = identified(
            "transformation-edge",
            &(
                &input.identity,
                &output.identity,
                &pass.identity,
                &claimed_relation,
                &assumptions_consumed,
                &obligations_generated,
                &evidence,
                status,
                &provenance,
            ),
        );
        Self {
            identity,
            input,
            output,
            pass,
            claimed_relation,
            assumptions_consumed,
            obligations_generated,
            evidence,
            status,
            provenance,
        }
    }

    pub fn validate_against(&self, artifacts: &[CompilerArtifactRef]) -> bool {
        let input_known = artifacts.iter().any(|artifact| artifact == &self.input);
        let output_known = artifacts.iter().any(|artifact| artifact == &self.output);
        input_known
            && output_known
            && self.input.identity_is_valid()
            && self.output.identity_is_valid()
            && self.pass.identity_is_valid()
            && self.pass.input == self.input.representation
            && self.pass.output == self.output.representation
            && self.claimed_relation == self.pass.claimed_relation
            && self.identity
                == Self::new(
                    self.input.clone(),
                    self.output.clone(),
                    self.pass.clone(),
                    self.assumptions_consumed.clone(),
                    self.obligations_generated.clone(),
                    self.evidence.clone(),
                    self.status,
                    self.provenance.clone(),
                )
                .identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BackendIdentity {
    pub identity: SemanticId,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendCapabilityManifest {
    pub schema_version: String,
    pub identity: SemanticId,
    pub backend: BackendIdentity,
    pub realization_profile: String,
    pub target_families: BTreeSet<String>,
    pub accepted_ir_versions: BTreeSet<String>,
    pub required_target_facts: BTreeSet<String>,
    pub supported_machine_intents: BTreeSet<String>,
    pub unsupported_operations: BTreeSet<String>,
    pub artifact_kinds: BTreeSet<String>,
    pub validator_capabilities: BTreeSet<String>,
    pub runtime_requirements: BTreeSet<String>,
    pub abi_layout_requirements: BTreeSet<String>,
    pub failure_behavior: String,
    pub deterministic: bool,
}

impl BackendCapabilityManifest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        backend: BackendIdentity,
        realization_profile: impl Into<String>,
        target_families: BTreeSet<String>,
        accepted_ir_versions: BTreeSet<String>,
        required_target_facts: BTreeSet<String>,
        supported_machine_intents: BTreeSet<String>,
        unsupported_operations: BTreeSet<String>,
        artifact_kinds: BTreeSet<String>,
        validator_capabilities: BTreeSet<String>,
        runtime_requirements: BTreeSet<String>,
        abi_layout_requirements: BTreeSet<String>,
        failure_behavior: impl Into<String>,
        deterministic: bool,
    ) -> Self {
        let mut manifest = Self {
            schema_version: BACKEND_CAPABILITY_SCHEMA_VERSION.to_owned(),
            identity: SemanticId(String::new()),
            backend,
            realization_profile: realization_profile.into(),
            target_families,
            accepted_ir_versions,
            required_target_facts,
            supported_machine_intents,
            unsupported_operations,
            artifact_kinds,
            validator_capabilities,
            runtime_requirements,
            abi_layout_requirements,
            failure_behavior: failure_behavior.into(),
            deterministic,
        };
        manifest.identity = identified("backend-capability", &manifest.without_identity());
        manifest
    }

    pub fn identity_is_valid(&self) -> bool {
        self.schema_version == BACKEND_CAPABILITY_SCHEMA_VERSION
            && self.backend.identity_is_valid()
            && !self.realization_profile.trim().is_empty()
            && !self.artifact_kinds.is_empty()
            && !self.failure_behavior.trim().is_empty()
            && self.identity == identified("backend-capability", &self.without_identity())
    }

    fn without_identity(&self) -> BackendCapabilityMaterial<'_> {
        BackendCapabilityMaterial {
            schema_version: &self.schema_version,
            backend: &self.backend,
            realization_profile: &self.realization_profile,
            target_families: &self.target_families,
            accepted_ir_versions: &self.accepted_ir_versions,
            required_target_facts: &self.required_target_facts,
            supported_machine_intents: &self.supported_machine_intents,
            unsupported_operations: &self.unsupported_operations,
            artifact_kinds: &self.artifact_kinds,
            validator_capabilities: &self.validator_capabilities,
            runtime_requirements: &self.runtime_requirements,
            abi_layout_requirements: &self.abi_layout_requirements,
            failure_behavior: &self.failure_behavior,
            deterministic: self.deterministic,
        }
    }
}

#[derive(Serialize)]
struct BackendCapabilityMaterial<'a> {
    schema_version: &'a str,
    backend: &'a BackendIdentity,
    realization_profile: &'a str,
    target_families: &'a BTreeSet<String>,
    accepted_ir_versions: &'a BTreeSet<String>,
    required_target_facts: &'a BTreeSet<String>,
    supported_machine_intents: &'a BTreeSet<String>,
    unsupported_operations: &'a BTreeSet<String>,
    artifact_kinds: &'a BTreeSet<String>,
    validator_capabilities: &'a BTreeSet<String>,
    runtime_requirements: &'a BTreeSet<String>,
    abi_layout_requirements: &'a BTreeSet<String>,
    failure_behavior: &'a str,
    deterministic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealizationRequest {
    pub schema_version: String,
    pub identity: SemanticId,
    pub selected_ssa: CompilerArtifactRef,
    pub target: TargetContractRef,
    pub acceptable_backends: BTreeSet<BackendIdentity>,
    pub required_machine_intents: BTreeSet<String>,
    pub required_artifact_kinds: BTreeSet<String>,
    pub required_validators: BTreeSet<String>,
    pub fallback_policy: String,
}

impl RealizationRequest {
    pub fn new(
        selected_ssa: CompilerArtifactRef,
        target: TargetContractRef,
        acceptable_backends: BTreeSet<BackendIdentity>,
        required_machine_intents: BTreeSet<String>,
        required_artifact_kinds: BTreeSet<String>,
        required_validators: BTreeSet<String>,
        fallback_policy: impl Into<String>,
    ) -> Self {
        let mut request = Self {
            schema_version: REALIZATION_REQUEST_SCHEMA_VERSION.to_owned(),
            identity: SemanticId(String::new()),
            selected_ssa,
            target,
            acceptable_backends,
            required_machine_intents,
            required_artifact_kinds,
            required_validators,
            fallback_policy: fallback_policy.into(),
        };
        request.identity = identified("realization-request", &request.without_identity());
        request
    }

    pub fn identity_is_valid(&self) -> bool {
        self.schema_version == REALIZATION_REQUEST_SCHEMA_VERSION
            && self.selected_ssa.representation == ArtifactRepresentation::SelectedSsa
            && self.selected_ssa.identity_is_valid()
            && self.target.identity_is_valid()
            && !self.acceptable_backends.is_empty()
            && self
                .acceptable_backends
                .iter()
                .all(BackendIdentity::identity_is_valid)
            && !self.fallback_policy.trim().is_empty()
            && self.identity == identified("realization-request", &self.without_identity())
    }

    fn without_identity(&self) -> RealizationRequestMaterial<'_> {
        RealizationRequestMaterial {
            schema_version: &self.schema_version,
            selected_ssa: &self.selected_ssa,
            target: &self.target,
            acceptable_backends: &self.acceptable_backends,
            required_machine_intents: &self.required_machine_intents,
            required_artifact_kinds: &self.required_artifact_kinds,
            required_validators: &self.required_validators,
            fallback_policy: &self.fallback_policy,
        }
    }
}

#[derive(Serialize)]
struct RealizationRequestMaterial<'a> {
    schema_version: &'a str,
    selected_ssa: &'a CompilerArtifactRef,
    target: &'a TargetContractRef,
    acceptable_backends: &'a BTreeSet<BackendIdentity>,
    required_machine_intents: &'a BTreeSet<String>,
    required_artifact_kinds: &'a BTreeSet<String>,
    required_validators: &'a BTreeSet<String>,
    fallback_policy: &'a str,
}

impl BackendIdentity {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        let name = name.into();
        let version = version.into();
        let identity = identified("backend", &(&name, &version));
        Self {
            identity,
            name,
            version,
        }
    }

    pub fn identity_is_valid(&self) -> bool {
        !self.name.trim().is_empty()
            && !self.version.trim().is_empty()
            && self.identity == identified("backend", &(&self.name, &self.version))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendConfiguration {
    pub backend: BackendIdentity,
    pub options: BTreeMap<String, String>,
    pub target_features: BTreeSet<String>,
    pub linker_toolchain: Option<SemanticId>,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetLoweringPlan {
    pub identity: SemanticId,
    pub selected_ssa: CompilerArtifactRef,
    pub target: TargetContractRef,
    pub backend: Option<BackendConfiguration>,
    pub target_layout_assumptions: Vec<String>,
    pub abi_assumptions: Vec<String>,
    pub integer_lowering: BTreeMap<String, String>,
    pub trap_failure_mapping: BTreeMap<String, String>,
    pub linker_requirements: Vec<String>,
    pub promises_consumed: Vec<String>,
    pub assumptions_introduced: Vec<String>,
    pub status: TransformationStatus,
}

impl TargetLoweringPlan {
    #[allow(clippy::too_many_arguments)]
    pub fn with_explicit_facts(
        selected_ssa: CompilerArtifactRef,
        target: TargetContractRef,
        backend: Option<BackendConfiguration>,
        target_layout_assumptions: Vec<String>,
        abi_assumptions: Vec<String>,
        integer_lowering: BTreeMap<String, String>,
        trap_failure_mapping: BTreeMap<String, String>,
        linker_requirements: Vec<String>,
        promises_consumed: Vec<String>,
        status: TransformationStatus,
    ) -> Self {
        let mut assumptions_introduced = target.assumptions.clone();
        if let Some(configuration) = &backend {
            assumptions_introduced.extend(configuration.assumptions.clone());
        }
        sort_dedup(&mut assumptions_introduced);
        let mut layout = target_layout_assumptions;
        let mut abi = abi_assumptions;
        let mut linker = linker_requirements;
        let mut promises = promises_consumed;
        sort_dedup(&mut layout);
        sort_dedup(&mut abi);
        sort_dedup(&mut linker);
        sort_dedup(&mut promises);
        let mut plan = Self {
            identity: SemanticId(String::new()),
            selected_ssa,
            target,
            backend,
            target_layout_assumptions: layout,
            abi_assumptions: abi,
            integer_lowering,
            trap_failure_mapping,
            linker_requirements: linker,
            promises_consumed: promises,
            assumptions_introduced,
            status,
        };
        plan.identity = identified("target-lowering-plan", &plan.without_identity());
        plan
    }

    pub fn with_unknown_facts(
        selected_ssa: CompilerArtifactRef,
        target: TargetContractRef,
        backend: Option<BackendConfiguration>,
        mut required_facts: Vec<String>,
    ) -> Self {
        sort_dedup(&mut required_facts);
        let mut assumptions_introduced = target.assumptions.clone();
        if let Some(configuration) = &backend {
            assumptions_introduced.extend(configuration.assumptions.clone());
        }
        sort_dedup(&mut assumptions_introduced);
        let mut plan = Self {
            identity: SemanticId(String::new()),
            selected_ssa,
            target,
            backend,
            target_layout_assumptions: required_facts,
            abi_assumptions: Vec::new(),
            integer_lowering: BTreeMap::new(),
            trap_failure_mapping: BTreeMap::new(),
            linker_requirements: Vec::new(),
            promises_consumed: Vec::new(),
            assumptions_introduced,
            status: TransformationStatus::Unknown,
        };
        plan.identity = identified("target-lowering-plan", &plan.without_identity());
        plan
    }

    pub fn identity_is_valid(&self) -> bool {
        self.selected_ssa.identity_is_valid()
            && self.target.identity_is_valid()
            && match &self.backend {
                Some(configuration) => configuration.backend.identity_is_valid(),
                None => true,
            }
            && self.identity == identified("target-lowering-plan", &self.without_identity())
    }

    fn without_identity(&self) -> TargetLoweringPlanMaterial<'_> {
        TargetLoweringPlanMaterial {
            selected_ssa: &self.selected_ssa,
            target: &self.target,
            backend: &self.backend,
            target_layout_assumptions: &self.target_layout_assumptions,
            abi_assumptions: &self.abi_assumptions,
            integer_lowering: &self.integer_lowering,
            trap_failure_mapping: &self.trap_failure_mapping,
            linker_requirements: &self.linker_requirements,
            promises_consumed: &self.promises_consumed,
            assumptions_introduced: &self.assumptions_introduced,
            status: self.status,
        }
    }
}

#[derive(Serialize)]
struct TargetLoweringPlanMaterial<'a> {
    selected_ssa: &'a CompilerArtifactRef,
    target: &'a TargetContractRef,
    backend: &'a Option<BackendConfiguration>,
    target_layout_assumptions: &'a [String],
    abi_assumptions: &'a [String],
    integer_lowering: &'a BTreeMap<String, String>,
    trap_failure_mapping: &'a BTreeMap<String, String>,
    linker_requirements: &'a [String],
    promises_consumed: &'a [String],
    assumptions_introduced: &'a [String],
    status: TransformationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendEvidence {
    pub identity: SemanticId,
    pub backend: BackendIdentity,
    pub input: CompilerArtifactRef,
    pub output: CompilerArtifactRef,
    pub assumptions: Vec<String>,
    pub evidence: Vec<SemanticId>,
    pub status: TransformationStatus,
}

impl BackendEvidence {
    pub fn new(
        backend: BackendIdentity,
        input: CompilerArtifactRef,
        output: CompilerArtifactRef,
        mut assumptions: Vec<String>,
        mut evidence: Vec<SemanticId>,
        status: TransformationStatus,
    ) -> Self {
        sort_dedup(&mut assumptions);
        evidence.sort();
        evidence.dedup();
        let identity = identified(
            "backend-evidence",
            &(
                &backend.identity,
                &input.identity,
                &output.identity,
                &assumptions,
                &evidence,
                status,
            ),
        );
        Self {
            identity,
            backend,
            input,
            output,
            assumptions,
            evidence,
            status,
        }
    }

    pub fn is_current(
        &self,
        backend: &BackendIdentity,
        input: &CompilerArtifactRef,
        output: &CompilerArtifactRef,
    ) -> bool {
        self.backend.identity_is_valid()
            && backend.identity_is_valid()
            && self.input.identity_is_valid()
            && self.output.identity_is_valid()
            && &self.backend == backend
            && &self.input == input
            && &self.output == output
            && self.identity
                == Self::new(
                    self.backend.clone(),
                    self.input.clone(),
                    self.output.clone(),
                    self.assumptions.clone(),
                    self.evidence.clone(),
                    self.status,
                )
                .identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendFunctionValueContract {
    pub inputs: Vec<BackendValueContract>,
    pub outputs: Vec<BackendValueContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendValueContract {
    Scalar {
        semantic_type: String,
    },
    Finite {
        type_identity: SemanticId,
        variants: BTreeMap<u32, SemanticId>,
        /// Payload field declarations per discriminant, canonical order
        /// (field name, declared semantic type). Empty for payload-free
        /// finite types; serialized as absent so existing artifacts parse.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        payloads: BTreeMap<u32, Vec<(String, String)>>,
    },
    Record {
        type_identity: SemanticId,
        name: String,
        /// Canonical (name-sorted) fields: field name -> semantic type name.
        fields: Vec<(String, String)>,
    },
    /// Exact-length bounded sequence (`[E; N]`). The ABI value is a
    /// canonical cell root; element count is a type fact, not a runtime
    /// observation. Additive over the Scalar/Finite/Record vocabulary:
    /// earlier artifacts without this variant remain valid.
    Sequence {
        semantic_type: String,
        element: String,
        length: u32,
    },
    /// Bounded view (`[E; up_to M]`). The ABI value is a packed 64-bit
    /// descriptor `(offset as u32) | (length << 32)`; element storage lives
    /// in the same canonical arena as records and exact sequences.
    View {
        semantic_type: String,
        element: String,
        capacity: u32,
    },
    /// Logical integer vector (`vec<T, N>`). The ABI value is a cell root;
    /// lane count is a type fact. Native scalar backends store one 8-byte
    /// slot per lane; portable WASM stores packed lane widths.
    Vector {
        semantic_type: String,
        element: String,
        lanes: u32,
    },
    /// Logical mask (`mask<N>`). The ABI value is packed predicate bits in
    /// a 64-bit word, lane 0 in the least-significant bit. No arena cell.
    Mask {
        semantic_type: String,
        lanes: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendArtifact {
    pub schema_version: String,
    pub identity: SemanticId,
    pub backend: BackendIdentity,
    pub input: CompilerArtifactRef,
    pub target: TargetContractRef,
    #[serde(default = "default_backend_artifact_kind")]
    pub artifact_kind: String,
    pub format: String,
    pub bytes_sha256: String,
    pub bytes_hex: String,
    pub exports: Vec<String>,
    #[serde(default)]
    pub function_value_contracts: BTreeMap<String, BackendFunctionValueContract>,
    /// Logical composite types referenced by this artifact's signatures,
    /// keyed by semantic type name. Makes artifacts self-describing so
    /// execution can marshal record/payload values without the program.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub composite_value_contracts: BTreeMap<String, BackendValueContract>,
    pub assumptions: Vec<String>,
    pub obligations_generated: Vec<SemanticId>,
    pub execution_applicability: Vec<String>,
    pub evidence_dependencies: Vec<SemanticId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promise_decisions: Vec<crate::BackendPromiseDecision>,
    pub unsupported: Vec<String>,
    pub status: TransformationStatus,
}

impl BackendArtifact {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        backend: BackendIdentity,
        input: CompilerArtifactRef,
        target: TargetContractRef,
        format: impl Into<String>,
        bytes: &[u8],
        exports: Vec<String>,
        assumptions: Vec<String>,
        obligations_generated: Vec<SemanticId>,
        execution_applicability: Vec<String>,
        evidence_dependencies: Vec<SemanticId>,
        unsupported: Vec<String>,
        status: TransformationStatus,
    ) -> Self {
        Self::new_with_kind(
            backend,
            input,
            target,
            "backend_defined",
            format,
            bytes,
            exports,
            assumptions,
            obligations_generated,
            execution_applicability,
            evidence_dependencies,
            unsupported,
            status,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_kind(
        backend: BackendIdentity,
        input: CompilerArtifactRef,
        target: TargetContractRef,
        artifact_kind: impl Into<String>,
        format: impl Into<String>,
        bytes: &[u8],
        mut exports: Vec<String>,
        mut assumptions: Vec<String>,
        mut obligations_generated: Vec<SemanticId>,
        mut execution_applicability: Vec<String>,
        mut evidence_dependencies: Vec<SemanticId>,
        mut unsupported: Vec<String>,
        status: TransformationStatus,
    ) -> Self {
        exports.sort();
        exports.dedup();
        sort_dedup(&mut assumptions);
        obligations_generated.sort();
        obligations_generated.dedup();
        sort_dedup(&mut execution_applicability);
        evidence_dependencies.sort();
        evidence_dependencies.dedup();
        sort_dedup(&mut unsupported);
        let bytes_sha256 = sha256_hex(bytes);
        let bytes_hex = hex_encode(bytes);
        let mut artifact = Self {
            schema_version: BACKEND_ARTIFACT_SCHEMA_VERSION.to_owned(),
            identity: SemanticId(String::new()),
            backend,
            input,
            target,
            artifact_kind: artifact_kind.into(),
            format: format.into(),
            bytes_sha256,
            bytes_hex,
            exports,
            function_value_contracts: BTreeMap::new(),
            composite_value_contracts: BTreeMap::new(),
            assumptions,
            obligations_generated,
            execution_applicability,
            evidence_dependencies,
            promise_decisions: Vec::new(),
            unsupported,
            status,
        };
        artifact.identity = identified("backend-artifact", &artifact.without_identity());
        artifact
    }

    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        hex_decode(&self.bytes_hex)
    }

    pub fn with_function_value_contracts(
        mut self,
        function_value_contracts: BTreeMap<String, BackendFunctionValueContract>,
    ) -> Self {
        self.function_value_contracts = function_value_contracts;
        self.identity = identified("backend-artifact", &self.without_identity());
        self
    }

    pub fn with_composite_value_contracts(
        mut self,
        composite_value_contracts: BTreeMap<String, BackendValueContract>,
    ) -> Self {
        self.composite_value_contracts = composite_value_contracts;
        self.identity = identified("backend-artifact", &self.without_identity());
        self
    }

    pub fn with_promise_decisions(
        mut self,
        mut promise_decisions: Vec<crate::BackendPromiseDecision>,
    ) -> Self {
        promise_decisions.sort_by(|left, right| {
            (&left.subject, format!("{:?}", left.promise))
                .cmp(&(&right.subject, format!("{:?}", right.promise)))
        });
        promise_decisions.dedup();
        self.evidence_dependencies.extend(
            promise_decisions
                .iter()
                .filter_map(|decision| decision.certificate.as_ref())
                .map(|certificate| certificate.identity.clone()),
        );
        self.evidence_dependencies.sort();
        self.evidence_dependencies.dedup();
        self.promise_decisions = promise_decisions;
        self.identity = identified("backend-artifact", &self.without_identity());
        self
    }

    pub fn wasm_bytes(&self) -> Result<Vec<u8>, String> {
        self.bytes()
    }

    pub fn identity_is_valid(&self) -> bool {
        crate::record_counter("artifact_hash");
        self.backend.identity_is_valid()
            && self.input.identity_is_valid()
            && self.target.identity_is_valid()
            && !self.artifact_kind.trim().is_empty()
            && self.bytes_sha256 == sha256_hex(&self.bytes().unwrap_or_default())
            && self.identity == identified("backend-artifact", &self.without_identity())
    }

    fn without_identity(&self) -> BackendArtifactMaterial<'_> {
        BackendArtifactMaterial {
            schema_version: &self.schema_version,
            backend: &self.backend,
            input: &self.input,
            target: &self.target,
            artifact_kind: &self.artifact_kind,
            format: &self.format,
            bytes_sha256: &self.bytes_sha256,
            bytes_hex: &self.bytes_hex,
            exports: &self.exports,
            function_value_contracts: &self.function_value_contracts,
            assumptions: &self.assumptions,
            obligations_generated: &self.obligations_generated,
            execution_applicability: &self.execution_applicability,
            evidence_dependencies: &self.evidence_dependencies,
            promise_decisions: &self.promise_decisions,
            unsupported: &self.unsupported,
            status: self.status,
        }
    }
}

#[derive(Serialize)]
struct BackendArtifactMaterial<'a> {
    schema_version: &'a str,
    backend: &'a BackendIdentity,
    input: &'a CompilerArtifactRef,
    target: &'a TargetContractRef,
    artifact_kind: &'a str,
    format: &'a str,
    bytes_sha256: &'a str,
    bytes_hex: &'a str,
    exports: &'a [String],
    function_value_contracts: &'a BTreeMap<String, BackendFunctionValueContract>,
    assumptions: &'a [String],
    obligations_generated: &'a [SemanticId],
    execution_applicability: &'a [String],
    evidence_dependencies: &'a [SemanticId],
    promise_decisions: &'a [crate::BackendPromiseDecision],
    unsupported: &'a [String],
    status: TransformationStatus,
}

fn default_backend_artifact_kind() -> String {
    "backend_defined".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendResult {
    pub status: TransformationStatus,
    pub artifact: Option<BackendArtifact>,
    pub artifact_ref: Option<CompilerArtifactRef>,
    pub evidence: Option<BackendEvidence>,
    pub diagnostics: Vec<CompilerDiagnostic>,
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("backend artifact hex encoding has an odd length".to_owned());
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&hex[index..index + 2], 16)
                .map_err(|_| "backend artifact hex encoding is not hexadecimal".to_owned())
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilerDiagnosticKind {
    InvalidRequest,
    SemanticInvalidity,
    UnresolvedObligation,
    FailedTransformationRelation,
    UnsupportedTargetRealization,
    MissingTargetEvidence,
    UnavailableBackendCapability,
    ExternalToolFailure,
    InternalCompilerDefect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerDiagnostic {
    pub code: String,
    pub kind: CompilerDiagnosticKind,
    pub path: Option<String>,
    pub message: String,
    pub related_artifacts: Vec<SemanticId>,
    pub obligations: Vec<SemanticId>,
}

impl CompilerDiagnostic {
    pub fn new(
        code: impl Into<String>,
        kind: CompilerDiagnosticKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            kind,
            path: None,
            message: message.into(),
            related_artifacts: Vec::new(),
            obligations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilationRequest {
    pub schema_version: String,
    pub identity: SemanticId,
    pub input: CompilerArtifactRef,
    pub language_profile: SemanticId,
    pub emit: BTreeSet<ArtifactRepresentation>,
    pub compiler: CompilerImplementationIdentity,
    pub compiler_host: CompilerHostIdentity,
    pub build_host: BuildHostIdentity,
    pub target: Option<TargetContractRef>,
    pub run_environment: Option<RunEnvironmentRef>,
    pub pipeline: PassPipelineIdentity,
    pub assumptions: Vec<String>,
    pub assurance_profile: Option<SemanticId>,
    pub backend: Option<BackendConfiguration>,
}

impl CompilationRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        input: CompilerArtifactRef,
        language_profile: SemanticId,
        emit: BTreeSet<ArtifactRepresentation>,
        compiler: CompilerImplementationIdentity,
        compiler_host: CompilerHostIdentity,
        build_host: BuildHostIdentity,
        target: Option<TargetContractRef>,
        run_environment: Option<RunEnvironmentRef>,
        pipeline: PassPipelineIdentity,
        mut assumptions: Vec<String>,
        assurance_profile: Option<SemanticId>,
        backend: Option<BackendConfiguration>,
    ) -> Self {
        sort_dedup(&mut assumptions);
        let mut request = Self {
            schema_version: COMPILER_ARTIFACT_SCHEMA_VERSION.to_owned(),
            identity: SemanticId(String::new()),
            input,
            language_profile,
            emit,
            compiler,
            compiler_host,
            build_host,
            target,
            run_environment,
            pipeline,
            assumptions,
            assurance_profile,
            backend,
        };
        request.identity = identified("compilation-request", &request.without_identity());
        request
    }

    pub fn identity_is_valid(&self) -> bool {
        self.identity == identified("compilation-request", &self.without_identity())
    }

    pub fn validate(&self) -> Vec<CompilerDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.schema_version != COMPILER_ARTIFACT_SCHEMA_VERSION {
            diagnostics.push(CompilerDiagnostic::new(
                "CMP001",
                CompilerDiagnosticKind::InvalidRequest,
                "unsupported compiler request schema",
            ));
        }
        if !self.identity_is_valid()
            || !self.input.identity_is_valid()
            || !self.compiler.identity_is_valid()
            || !self.compiler_host.identity_is_valid()
            || !self.build_host.identity_is_valid()
            || !self.pipeline.identity_is_valid()
            || self
                .target
                .as_ref()
                .is_some_and(|target| !target.identity_is_valid())
            || self
                .run_environment
                .as_ref()
                .is_some_and(|run| !run.identity_is_valid())
        {
            diagnostics.push(CompilerDiagnostic::new(
                "CMP002",
                CompilerDiagnosticKind::InvalidRequest,
                "compiler request contains a laundered or inconsistent identity",
            ));
        }
        if !self.compiler_host.is_complete() || !self.build_host.is_complete() {
            diagnostics.push(CompilerDiagnostic::new(
                "CMP003",
                CompilerDiagnosticKind::InvalidRequest,
                "compiler and build host identities require explicit architecture, OS, and environment fields",
            ));
        }
        if self.input.representation != ArtifactRepresentation::Semantic {
            diagnostics.push(CompilerDiagnostic::new(
                "CMP004",
                CompilerDiagnosticKind::InvalidRequest,
                "the reference compiler currently accepts an exact semantic artifact input",
            ));
        }
        if self.backend.is_some() && self.target.is_none() {
            diagnostics.push(CompilerDiagnostic::new(
                "CMP005",
                CompilerDiagnosticKind::InvalidRequest,
                "backend configuration requires an explicit target contract",
            ));
        }
        diagnostics
    }

    fn without_identity(&self) -> CompilationRequestMaterial<'_> {
        CompilationRequestMaterial {
            schema_version: &self.schema_version,
            input: &self.input,
            language_profile: &self.language_profile,
            emit: &self.emit,
            compiler: &self.compiler,
            compiler_host: &self.compiler_host,
            build_host: &self.build_host,
            target: &self.target,
            run_environment: &self.run_environment,
            pipeline: &self.pipeline,
            assumptions: &self.assumptions,
            assurance_profile: &self.assurance_profile,
            backend: &self.backend,
        }
    }
}

#[derive(Serialize)]
struct CompilationRequestMaterial<'a> {
    schema_version: &'a str,
    input: &'a CompilerArtifactRef,
    language_profile: &'a SemanticId,
    emit: &'a BTreeSet<ArtifactRepresentation>,
    compiler: &'a CompilerImplementationIdentity,
    compiler_host: &'a CompilerHostIdentity,
    build_host: &'a BuildHostIdentity,
    target: &'a Option<TargetContractRef>,
    run_environment: &'a Option<RunEnvironmentRef>,
    pipeline: &'a PassPipelineIdentity,
    assumptions: &'a [String],
    assurance_profile: &'a Option<SemanticId>,
    backend: &'a Option<BackendConfiguration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CompilationEmissions {
    pub semantic: Option<CanonicalForm>,
    pub hir: Option<HighLevelIr>,
    pub ssa: Option<SsaModule>,
    pub target_lowering_plan: Option<TargetLoweringPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<BackendArtifact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilationStatus {
    Completed,
    CompletedWithUnresolvedObligations,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilationEvidenceBundle {
    pub schema_version: String,
    pub identity: SemanticId,
    pub request_identity: SemanticId,
    pub input: CompilerArtifactRef,
    pub emitted_artifacts: Vec<CompilerArtifactRef>,
    pub transformation_edges: Vec<TransformationEdge>,
    pub compiler: CompilerImplementationIdentity,
    pub pipeline: PassPipelineIdentity,
    pub compiler_host: CompilerHostIdentity,
    pub build_host: BuildHostIdentity,
    pub target: Option<TargetContractRef>,
    pub run_environment: Option<RunEnvironmentRef>,
    pub target_lowering_plan: Option<TargetLoweringPlan>,
    pub backend: Option<BackendIdentity>,
    pub obligations: Vec<ObligationRecord>,
    pub unresolved_obligations: Vec<SemanticId>,
    pub evidence: Vec<SemanticId>,
    pub assumptions: Vec<String>,
    pub diagnostics: Vec<CompilerDiagnostic>,
}

impl CompilationEvidenceBundle {
    pub fn seal(&mut self) {
        self.identity = SemanticId(String::new());
        self.identity = identified("compilation-evidence", self);
    }

    pub fn identity_is_valid(&self) -> bool {
        let mut material = self.clone();
        material.identity = SemanticId(String::new());
        self.identity == identified("compilation-evidence", &material)
    }

    pub fn validate_integrity(&self) -> Vec<CompilerDiagnostic> {
        let mut diagnostics = Vec::new();
        if !self.identity_is_valid()
            || !self.input.identity_is_valid()
            || self
                .emitted_artifacts
                .iter()
                .any(|artifact| !artifact.identity_is_valid())
        {
            diagnostics.push(CompilerDiagnostic::new(
                "CMP101",
                CompilerDiagnosticKind::InvalidRequest,
                "compilation evidence contains a stale or laundered artifact identity",
            ));
        }
        let mut artifacts = vec![self.input.clone()];
        artifacts.extend(self.emitted_artifacts.clone());
        if self
            .transformation_edges
            .iter()
            .any(|edge| !edge.validate_against(&artifacts))
        {
            diagnostics.push(CompilerDiagnostic::new(
                "CMP102",
                CompilerDiagnosticKind::FailedTransformationRelation,
                "a transformation edge does not bind exact artifacts and a compatible pass",
            ));
        }
        let obligation_ids = self
            .obligations
            .iter()
            .map(|obligation| &obligation.identity)
            .collect::<BTreeSet<_>>();
        let expected_unresolved = self
            .obligations
            .iter()
            .filter(|obligation| obligation.status == crate::ObligationStatus::Unknown)
            .map(|obligation| &obligation.identity)
            .collect::<BTreeSet<_>>();
        let reported_unresolved = self.unresolved_obligations.iter().collect::<BTreeSet<_>>();
        if reported_unresolved != expected_unresolved
            || self
                .unresolved_obligations
                .iter()
                .any(|identity| !obligation_ids.contains(identity))
        {
            diagnostics.push(CompilerDiagnostic::new(
                "CMP103",
                CompilerDiagnosticKind::UnresolvedObligation,
                "the evidence bundle dropped, invented, or misclassified an unresolved obligation",
            ));
        }
        if let Some(plan) = &self.target_lowering_plan {
            if !plan.identity_is_valid()
                || self.target.as_ref() != Some(&plan.target)
                || plan
                    .backend
                    .as_ref()
                    .map(|configuration| &configuration.backend)
                    != self.backend.as_ref()
                || plan.backend.as_ref().is_some_and(|configuration| {
                    configuration
                        .assumptions
                        .iter()
                        .any(|assumption| !plan.assumptions_introduced.contains(assumption))
                })
            {
                diagnostics.push(CompilerDiagnostic::new(
                    "CMP104",
                    CompilerDiagnosticKind::MissingTargetEvidence,
                    "target/backend lowering evidence is incomplete or bound to different identities",
                ));
            }
        }
        diagnostics
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilationResult {
    pub schema_version: String,
    pub identity: SemanticId,
    pub request_identity: SemanticId,
    pub request: CompilationRequest,
    pub status: CompilationStatus,
    pub artifacts: Vec<CompilerArtifactRef>,
    pub emissions: CompilationEmissions,
    pub evidence: Option<CompilationEvidenceBundle>,
    pub diagnostics: Vec<CompilerDiagnostic>,
}

impl CompilationResult {
    pub fn seal(&mut self) {
        self.identity = SemanticId(String::new());
        self.identity = identified("compilation-result", self);
    }

    pub fn identity_is_valid(&self) -> bool {
        let mut material = self.clone();
        material.identity = SemanticId(String::new());
        self.schema_version == COMPILER_ARTIFACT_SCHEMA_VERSION
            && self.request_identity == self.request.identity
            && self.request.identity_is_valid()
            && self.identity == identified("compilation-result", &material)
    }

    pub fn validate_integrity(&self) -> Vec<CompilerDiagnostic> {
        let mut diagnostics = Vec::new();
        if !self.identity_is_valid() {
            diagnostics.push(CompilerDiagnostic::new(
                "CMP105",
                CompilerDiagnosticKind::InvalidRequest,
                "compilation result identity is stale or laundered",
            ));
        }
        if self.request_identity != self.request.identity {
            diagnostics.push(CompilerDiagnostic::new(
                "CMP106",
                CompilerDiagnosticKind::InvalidRequest,
                "compilation result request identity does not match its request",
            ));
        }
        if self
            .artifacts
            .iter()
            .any(|artifact| !artifact.identity_is_valid())
        {
            diagnostics.push(CompilerDiagnostic::new(
                "CMP107",
                CompilerDiagnosticKind::InvalidRequest,
                "compilation result contains a stale artifact reference",
            ));
        }
        if let Some(evidence) = &self.evidence {
            diagnostics.extend(evidence.validate_integrity());
            if evidence.request_identity != self.request.identity {
                diagnostics.push(CompilerDiagnostic::new(
                    "CMP108",
                    CompilerDiagnosticKind::InvalidRequest,
                    "compilation evidence is bound to a different compilation request",
                ));
            }
        }
        diagnostics
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerNodeProfile {
    pub node_identity: String,
    pub architecture: String,
    pub operating_system: String,
    pub environment: String,
    pub observations: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilationStudyRequest {
    pub schema_version: String,
    pub identity: SemanticId,
    pub node: CompilerNodeProfile,
    pub compilation_request: CompilationRequest,
    pub execution_result_references: Vec<SemanticId>,
}

impl CompilationStudyRequest {
    pub fn new(
        node: CompilerNodeProfile,
        compilation_request: CompilationRequest,
        mut execution_result_references: Vec<SemanticId>,
    ) -> Self {
        execution_result_references.sort();
        execution_result_references.dedup();
        let identity = identified(
            "compilation-study-request",
            &(
                &node,
                &compilation_request.identity,
                &execution_result_references,
            ),
        );
        Self {
            schema_version: COMPILER_ARTIFACT_SCHEMA_VERSION.to_owned(),
            identity,
            node,
            compilation_request,
            execution_result_references,
        }
    }

    pub fn identity_is_valid(&self) -> bool {
        self.schema_version == COMPILER_ARTIFACT_SCHEMA_VERSION
            && !self.node.node_identity.trim().is_empty()
            && self.compilation_request.identity_is_valid()
            && self.identity
                == identified(
                    "compilation-study-request",
                    &(
                        &self.node,
                        &self.compilation_request.identity,
                        &self.execution_result_references,
                    ),
                )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossHostInvariants {
    pub expected_equal: Vec<String>,
    pub expected_distinct: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerPassExecutionObservation {
    pub edge_identity: SemanticId,
    pub pass_identity: SemanticId,
    pub pass_id: String,
    pub input_artifact: SemanticId,
    pub output_artifact: SemanticId,
    pub status: TransformationStatus,
}

impl From<&TransformationEdge> for CompilerPassExecutionObservation {
    fn from(edge: &TransformationEdge) -> Self {
        Self {
            edge_identity: edge.identity.clone(),
            pass_identity: edge.pass.identity.clone(),
            pass_id: edge.pass.id.clone(),
            input_artifact: edge.input.identity.clone(),
            output_artifact: edge.output.identity.clone(),
            status: edge.status,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilationStudyResult {
    pub schema_version: String,
    pub contract_id: String,
    pub identity: SemanticId,
    pub study_request_identity: SemanticId,
    pub compilation_request_identity: SemanticId,
    pub run_identity: SemanticId,
    pub node: CompilerNodeProfile,
    pub compiler_identity: SemanticId,
    pub pipeline_identity: SemanticId,
    pub compiler_host_identity: SemanticId,
    pub build_host_identity: SemanticId,
    pub target_identity: Option<SemanticId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_identity: Option<SemanticId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realization_plan_identity: Option<SemanticId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_ssa_identity: Option<SemanticId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_artifact_identity: Option<SemanticId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_artifact_kind: Option<String>,
    pub compilation_status: CompilationStatus,
    pub stage_fingerprints: BTreeMap<ArtifactRepresentation, String>,
    pub pass_executions: Vec<CompilerPassExecutionObservation>,
    pub semantic_fingerprint: Option<String>,
    pub hir_fingerprint: Option<String>,
    pub ssa_fingerprint: Option<String>,
    pub compilation_result_identity: SemanticId,
    pub execution_result_references: Vec<SemanticId>,
    pub diagnostics: Vec<CompilerDiagnostic>,
    pub unresolved_obligations: Vec<SemanticId>,
    pub unresolved_assumptions: Vec<String>,
    pub invariants: CrossHostInvariants,
    pub interpretation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyArtifactReference {
    pub identity: String,
    pub kind: String,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyCompilerObservation {
    pub study_request_identity: SemanticId,
    pub compilation_request_identity: SemanticId,
    pub run_identity: SemanticId,
    pub compiler_identity: SemanticId,
    pub pipeline_identity: SemanticId,
    pub selected_ssa_identity: Option<SemanticId>,
    pub target_identity: Option<SemanticId>,
    pub backend_identity: Option<SemanticId>,
    pub realization_plan_identity: Option<SemanticId>,
    pub backend_artifact_identity: Option<SemanticId>,
    pub backend_artifact_kind: Option<String>,
    pub compilation_status: CompilationStatus,
    pub stage_fingerprints: BTreeMap<ArtifactRepresentation, String>,
    pub unresolved_obligations: Vec<SemanticId>,
    pub unresolved_assumptions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyCompilerReference {
    pub schema: String,
    pub producer: String,
    pub record_kind: String,
    pub schema_version: String,
    pub stable_id: String,
    pub content_digest: String,
    pub artifact: FamilyArtifactReference,
    pub scope: BTreeMap<String, String>,
    pub compiler: FamilyCompilerObservation,
    pub authority_boundary: String,
}

impl CompilationStudyResult {
    pub fn seal(&mut self) {
        self.identity = SemanticId(String::new());
        self.identity = identified("compilation-study-result", self);
    }

    pub fn identity_is_valid(&self) -> bool {
        let mut material = self.clone();
        material.identity = SemanticId(String::new());
        self.contract_id == COMPILATION_STUDY_RESULT_CONTRACT_ID
            && self.schema_version == COMPILER_ARTIFACT_SCHEMA_VERSION
            && self.interpretation == COMPILATION_STUDY_OBSERVATION_INTERPRETATION
            && self.identity == identified("compilation-study-result", &material)
    }

    pub fn family_reference(&self) -> FamilyCompilerReference {
        let canonical = canonical_json_value(self)
            .expect("compilation study result is canonical JSON serializable");
        let content_digest = sha256_hex(canonical.as_bytes());
        let mut scope = BTreeMap::new();
        scope.insert("compiler".to_owned(), self.compiler_identity.0.clone());
        if let Some(backend) = &self.backend_identity {
            scope.insert("backend".to_owned(), backend.0.clone());
        }
        if let Some(target) = &self.target_identity {
            scope.insert("target".to_owned(), target.0.clone());
        }
        FamilyCompilerReference {
            schema: FAMILY_COMPILER_REFERENCE_SCHEMA_VERSION.to_owned(),
            producer: "mncs-language".to_owned(),
            record_kind: "CompilationStudyResult".to_owned(),
            schema_version: self.schema_version.clone(),
            stable_id: self.identity.0.clone(),
            content_digest: content_digest.clone(),
            artifact: FamilyArtifactReference {
                identity: self.identity.0.clone(),
                kind: "compilation-study-result".to_owned(),
                digest: content_digest,
            },
            scope,
            compiler: FamilyCompilerObservation {
                study_request_identity: self.study_request_identity.clone(),
                compilation_request_identity: self.compilation_request_identity.clone(),
                run_identity: self.run_identity.clone(),
                compiler_identity: self.compiler_identity.clone(),
                pipeline_identity: self.pipeline_identity.clone(),
                selected_ssa_identity: self.selected_ssa_identity.clone(),
                target_identity: self.target_identity.clone(),
                backend_identity: self.backend_identity.clone(),
                realization_plan_identity: self.realization_plan_identity.clone(),
                backend_artifact_identity: self.backend_artifact_identity.clone(),
                backend_artifact_kind: self.backend_artifact_kind.clone(),
                compilation_status: self.compilation_status,
                stage_fingerprints: self.stage_fingerprints.clone(),
                unresolved_obligations: self.unresolved_obligations.clone(),
                unresolved_assumptions: self.unresolved_assumptions.clone(),
            },
            authority_boundary: "Compiler evidence only; this reference does not certify experiment success, scientific validity, runtime conformance, or target assurance.".to_owned(),
        }
    }
}

fn artifact_identity(
    representation: ArtifactRepresentation,
    schema_version: &str,
    fingerprint: &str,
) -> SemanticId {
    identified(
        "compiler-artifact",
        &(representation, schema_version, fingerprint),
    )
}

fn identified<T: Serialize>(kind: &str, value: &T) -> SemanticId {
    let material = canonical_json_value(value).expect("compiler identity material is serializable");
    SemanticId(format!(
        "mncs:compiler:{kind}:{}",
        sha256_hex(material.as_bytes())
    ))
}

fn sort_dedup<T: Ord>(values: &mut Vec<T>) {
    values.sort();
    values.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(os: &str) -> CompilerHostIdentity {
        CompilerHostIdentity::new("x86_64", os, "native", BTreeMap::new())
    }

    fn build(os: &str) -> BuildHostIdentity {
        BuildHostIdentity::new("x86_64", os, "native", "rustc-1.79", BTreeMap::new())
    }

    fn observation() -> ObservationModelRef {
        ObservationModelRef::new(
            "mncs-public-behavior",
            "0.1",
            ["return_value", "declared_failure", "external_effect"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        )
    }

    fn pass(input: ArtifactRepresentation, output: ArtifactRepresentation) -> CompilerPassIdentity {
        CompilerPassIdentity::new(
            "test-pass",
            "0.1",
            input,
            output,
            RelationClaim {
                kind: "directional_observational_refinement".to_owned(),
                observation_model: observation(),
            },
            vec!["validated-input".to_owned()],
            Vec::new(),
            Vec::new(),
            vec!["semantic-preservation".to_owned()],
            "structural-reference-check",
            "retain-input",
        )
    }

    fn request(
        compiler_host: CompilerHostIdentity,
        build_host: BuildHostIdentity,
    ) -> CompilationRequest {
        let input =
            CompilerArtifactRef::new(ArtifactRepresentation::Semantic, "0.2", "a".repeat(64));
        let pipeline = PassPipelineIdentity::new(
            "test-pipeline",
            "0.1",
            vec![pass(
                ArtifactRepresentation::Semantic,
                ArtifactRepresentation::Hir,
            )],
        );
        CompilationRequest::new(
            input,
            SemanticId("language-profile:research-0.1".to_owned()),
            [ArtifactRepresentation::Hir].into_iter().collect(),
            CompilerImplementationIdentity::new("mncs-reference", "0.1", None),
            compiler_host,
            build_host,
            None,
            None,
            pipeline,
            Vec::new(),
            None,
            None,
        )
    }

    #[test]
    fn canonical_construction_is_deterministic_and_map_order_independent() {
        let mut first = BTreeMap::new();
        first.insert("b".to_owned(), "2".to_owned());
        first.insert("a".to_owned(), "1".to_owned());
        let mut second = BTreeMap::new();
        second.insert("a".to_owned(), "1".to_owned());
        second.insert("b".to_owned(), "2".to_owned());
        assert_eq!(
            CompilerHostIdentity::new("x86_64", "linux", "gnu", first),
            CompilerHostIdentity::new("x86_64", "linux", "gnu", second)
        );
    }

    #[test]
    fn host_and_build_changes_affect_request_without_laundering_roles() {
        let linux = request(host("linux"), build("linux"));
        let windows_host = request(host("windows"), build("linux"));
        let windows_build = request(host("linux"), build("windows"));
        assert_ne!(linux.identity, windows_host.identity);
        assert_ne!(linux.identity, windows_build.identity);
        assert_ne!(linux.compiler_host.identity, linux.build_host.identity);
    }

    #[test]
    fn incomplete_or_laundered_host_identity_is_rejected() {
        let mut invalid = request(host("linux"), build("linux"));
        invalid.build_host.architecture.clear();
        invalid.build_host.identity = invalid.compiler_host.identity.clone();
        assert!(invalid.validate().iter().any(|item| item.code == "CMP002"));
        assert!(invalid.validate().iter().any(|item| item.code == "CMP003"));
    }

    #[test]
    fn target_and_backend_identity_changes_do_not_mutate_earlier_artifacts() {
        let semantic =
            CompilerArtifactRef::new(ArtifactRepresentation::Semantic, "0.2", "a".repeat(64));
        let hir = CompilerArtifactRef::new(ArtifactRepresentation::Hir, "0.3", "b".repeat(64));
        let ssa = CompilerArtifactRef::new(ArtifactRepresentation::Ssa, "0.4", "c".repeat(64));
        let target_a = TargetContractRef::new(
            "wasm32-wasi",
            BTreeMap::new(),
            Vec::new(),
            vec!["candidate name is not target evidence".to_owned()],
        );
        let target_b = TargetContractRef::new(
            "aarch64-linux",
            BTreeMap::new(),
            Vec::new(),
            vec!["candidate name is not target evidence".to_owned()],
        );
        let backend_a = BackendIdentity::new("candidate-a", "1");
        let backend_b = BackendIdentity::new("candidate-b", "1");
        assert_ne!(target_a.identity, target_b.identity);
        assert_eq!(semantic.fingerprint, "a".repeat(64));
        assert_eq!(hir.fingerprint, "b".repeat(64));
        assert_eq!(ssa.fingerprint, "c".repeat(64));
        let output = CompilerArtifactRef::new(
            ArtifactRepresentation::BackendArtifact,
            "0.1",
            "d".repeat(64),
        );
        let evidence = BackendEvidence::new(
            backend_a.clone(),
            ssa.clone(),
            output.clone(),
            vec!["layout explicit".to_owned()],
            Vec::new(),
            TransformationStatus::Pass,
        );
        assert!(evidence.is_current(&backend_a, &ssa, &output));
        assert!(!evidence.is_current(&backend_b, &ssa, &output));
    }

    #[test]
    fn transformation_edge_rejects_wrong_artifact_or_pass_shape() {
        let input =
            CompilerArtifactRef::new(ArtifactRepresentation::Semantic, "0.2", "a".repeat(64));
        let output = CompilerArtifactRef::new(ArtifactRepresentation::Hir, "0.3", "b".repeat(64));
        let edge = TransformationEdge::new(
            input.clone(),
            output.clone(),
            pass(
                ArtifactRepresentation::Semantic,
                ArtifactRepresentation::Hir,
            ),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            TransformationStatus::Pass,
            Vec::new(),
        );
        assert!(edge.validate_against(&[input.clone(), output.clone()]));
        let wrong = CompilerArtifactRef::new(ArtifactRepresentation::Hir, "0.3", "c".repeat(64));
        assert!(!edge.validate_against(&[input, wrong]));
    }

    #[test]
    fn backend_assumption_cannot_be_omitted_from_target_plan_identity() {
        let selected =
            CompilerArtifactRef::new(ArtifactRepresentation::SelectedSsa, "0.4", "c".repeat(64));
        let target = TargetContractRef::new(
            "portable-candidate",
            BTreeMap::new(),
            Vec::new(),
            Vec::new(),
        );
        let backend = BackendConfiguration {
            backend: BackendIdentity::new("candidate-backend", "1"),
            options: BTreeMap::new(),
            target_features: BTreeSet::new(),
            linker_toolchain: None,
            assumptions: vec!["backend trap mapping is unverified".to_owned()],
        };
        let plan = TargetLoweringPlan::with_unknown_facts(
            selected,
            target,
            Some(backend),
            vec!["ABI evidence".to_owned()],
        );
        assert!(plan
            .assumptions_introduced
            .contains(&"backend trap mapping is unverified".to_owned()));
        assert!(plan.identity_is_valid());
        let mut laundered = plan;
        laundered.assumptions_introduced.clear();
        assert!(!laundered.identity_is_valid());
    }

    #[test]
    fn identity_contracts_have_no_path_timestamp_or_process_fields() {
        let json = serde_json::to_string(&request(host("linux"), build("linux"))).unwrap();
        for forbidden in [
            "absolute_path",
            "timestamp",
            "process_id",
            "hostname",
            "username",
        ] {
            assert!(!json.contains(forbidden));
        }
    }
}
