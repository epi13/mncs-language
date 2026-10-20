//! Versioned evidence for reusing successful declaration and function-body
//! validation.
//!
//! Facts are inputs to the Language validator, never authority on their own:
//! the validator re-derives the current subject, declaration interface,
//! semantic dependencies, language profile, and verifier policy before it
//! skips a body check. Source locations are carried separately and refreshed
//! for every source snapshot.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    assumption_id, function_id, specialization_id, BodyOperationKind, BodyType, Effect, Function,
    FunctionBody, GenericArg, GenericSpecializationRecord, Program, SemanticId, SemanticIdentities,
};

pub const VERIFICATION_FACT_SCHEMA_VERSION: &str = "mncs.language-verification-fact/2";
pub const VERIFICATION_FACT_REUSE_OBSERVATION_SCHEMA_VERSION: &str =
    "mncs.language-verification-fact-reuse-observation/2";
pub const VERIFICATION_FACT_VERIFIER_ID: &str = "mncs-language-function-body-validator/2";
pub const VERIFICATION_FACT_POLICY_ID: &str = "mncs-language-function-body-policy/1";
pub const DECLARATION_FACT_SCHEMA_VERSION: &str = "mncs.language-declaration-verification-fact/1";
pub const DECLARATION_FACT_REUSE_SCHEMA_VERSION: &str = "mncs.language-declaration-fact-reuse/1";
pub const DECLARATION_FACT_VERIFIER_ID: &str = "mncs-language-function-declaration-validator/1";
pub const DECLARATION_FACT_POLICY_ID: &str = "mncs-language-function-declaration-policy/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationDependencyRole {
    CallableInterface,
    NominalType,
    Assumption,
    GenericSpecialization,
    Effect,
    Capability,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VerificationFactDependency {
    pub role: VerificationDependencyRole,
    pub identity: SemanticId,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "identity")]
pub enum VerificationFactInvalidator {
    SubjectBodyChanged(SemanticId),
    DeclarationInterfaceChanged(SemanticId),
    DependencyChanged(SemanticId),
    EffectAssumptionChanged(SemanticId),
    CapabilityAssumptionChanged(SemanticId),
    VerifierChanged,
    PolicyChanged,
    LanguageProfileChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationFactResult {
    Verified,
}

/// A successful, independently addressable Language verification result.
/// `source_location_identity` is deliberately excluded from the witness and
/// reuse decision; callers refresh it from the current source map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationFact {
    pub schema_version: String,
    pub subject: SemanticId,
    pub subject_fingerprint: String,
    pub verifier_identity: String,
    pub policy_identity: String,
    pub language_profile: String,
    pub declaration_interface: VerificationFactDependency,
    pub dependencies: Vec<VerificationFactDependency>,
    pub declared_effects: Vec<Effect>,
    pub declared_capabilities: Vec<String>,
    pub result: VerificationFactResult,
    pub verification_witness_identity: String,
    /// The exact dependency guards whose change invalidates this evidence.
    pub invalidating_changes: Vec<VerificationFactInvalidator>,
    /// Current source location map identity. Refreshed independently on every
    /// validation request and never used to decide semantic validity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationFactRequest {
    pub language_profile: String,
    /// Exact profile assumptions for each verified body. The fallback above
    /// supports declaration-only callers; source frontends populate this map
    /// from the source artifact that declared each function.
    #[serde(default)]
    pub subject_language_profiles: BTreeMap<SemanticId, String>,
    #[serde(default)]
    pub prior_facts: Vec<VerificationFact>,
    /// Prior successful declaration checks. Body facts remain a separate
    /// contract because declaration and body validation have different
    /// subjects, dependencies, and recomputation units.
    #[serde(default)]
    pub prior_declaration_facts: Vec<DeclarationVerificationFact>,
    #[serde(default)]
    pub source_location_identities: BTreeMap<SemanticId, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationFactReuseMiss {
    Missing,
    DuplicatePriorFacts,
    UnsupportedSchema,
    UnsupportedResult,
    InvalidWitness,
    SubjectChanged,
    DeclarationInterfaceChanged,
    DependenciesChanged,
    EffectAssumptionsChanged,
    CapabilityAssumptionsChanged,
    VerifierChanged,
    PolicyChanged,
    LanguageProfileChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationFactRecomputation {
    pub subject: SemanticId,
    pub reason: VerificationFactReuseMiss,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationFactReuseObservation {
    pub schema_version: String,
    /// Body validation calls actually skipped after Language checked every
    /// current fact guard.
    pub reused: Vec<SemanticId>,
    /// Body validation units actually run, with the reason prior evidence was
    /// unavailable or stale.
    pub recomputed: Vec<VerificationFactRecomputation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationFactValidation {
    pub report: crate::ValidationReport,
    pub facts: Vec<VerificationFact>,
    pub declaration_facts: Vec<DeclarationVerificationFact>,
    pub reuse: VerificationFactReuseObservation,
    pub declaration_reuse: DeclarationFactReuseObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationVerificationWarning {
    pub code: String,
    /// Path suffix relative to the current `functions[index]` slot. The
    /// validator attaches the current slot when it replays this warning.
    pub relative_path: String,
    pub message: String,
}

/// A successful Language-issued function declaration result. The declaration
/// fingerprint includes every field checked by declaration validation,
/// including evidence annotations. Assumption dependencies are separately
/// guarded. Warnings are stored with relative paths so reordered functions
/// retain fresh diagnostics, and source locations are refreshed per snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationVerificationFact {
    pub schema_version: String,
    pub subject: SemanticId,
    pub subject_fingerprint: String,
    pub verifier_identity: String,
    pub policy_identity: String,
    pub language_profile: String,
    pub dependencies: Vec<VerificationFactDependency>,
    pub result: VerificationFactResult,
    pub verification_witness_identity: String,
    pub invalidating_changes: Vec<DeclarationFactInvalidator>,
    pub warnings: Vec<DeclarationVerificationWarning>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "identity")]
pub enum DeclarationFactInvalidator {
    SubjectDeclarationChanged(SemanticId),
    DependencyChanged(SemanticId),
    VerifierChanged,
    PolicyChanged,
    LanguageProfileChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclarationFactReuseMiss {
    Missing,
    DuplicatePriorFacts,
    UnsupportedSchema,
    UnsupportedResult,
    InvalidWitness,
    SubjectChanged,
    DependenciesChanged,
    VerifierChanged,
    PolicyChanged,
    LanguageProfileChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationFactRecomputation {
    pub subject: SemanticId,
    pub reason: DeclarationFactReuseMiss,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationFactReuseObservation {
    pub schema_version: String,
    /// Function declaration checks actually skipped after Language matched
    /// every current declaration and assumption guard.
    pub reused: Vec<SemanticId>,
    /// Function declaration checks actually run, with the reason prior
    /// evidence was unavailable or stale.
    pub recomputed: Vec<DeclarationFactRecomputation>,
}

#[derive(Debug, Clone)]
pub(crate) struct BodyFactBasis {
    pub subject: SemanticId,
    pub subject_fingerprint: String,
    pub declaration_interface: VerificationFactDependency,
    pub dependencies: Vec<VerificationFactDependency>,
    pub declared_effects: Vec<Effect>,
    pub declared_capabilities: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct DeclarationFactBasis {
    pub subject: SemanticId,
    pub subject_fingerprint: String,
    pub dependencies: Vec<VerificationFactDependency>,
}

impl Program {
    pub(crate) fn declaration_fact_basis_with_identities(
        &self,
        function: &Function,
        identities: &SemanticIdentities,
    ) -> Option<DeclarationFactBasis> {
        let home = function.identity_namespace(&self.module);
        let subject = function_id(home, &function.name);
        let mut declaration = function.clone();
        declaration.body = None;
        let bytes = serde_json::to_vec(&declaration).ok()?;
        let subject_fingerprint = crate::sha256_hex(&bytes);
        let mut dependencies = BTreeSet::new();
        for assumption in &function.assumptions {
            let identity = assumption_id(&self.module, assumption);
            // Missing assumptions are included as explicit dependencies. The
            // current validator will reject the declaration, and a previously
            // successful fact will fail the exact dependency comparison.
            let fingerprint = identities
                .fingerprint(&identity)
                .unwrap_or("missing")
                .to_owned();
            dependencies.insert(VerificationFactDependency {
                role: VerificationDependencyRole::Assumption,
                identity,
                fingerprint,
            });
        }
        Some(DeclarationFactBasis {
            subject,
            subject_fingerprint,
            dependencies: dependencies.into_iter().collect(),
        })
    }

    pub(crate) fn body_fact_basis_with_identities(
        &self,
        function: &Function,
        identities: &SemanticIdentities,
    ) -> Option<BodyFactBasis> {
        let body = function.body.as_ref()?;
        let home = function.identity_namespace(&self.module);
        let subject = body.identity(home, &function.name);
        let subject_fingerprint = body.fingerprint().ok()?;
        let declaration = function_id(home, &function.name);
        let declaration_interface = VerificationFactDependency {
            role: VerificationDependencyRole::CallableInterface,
            identity: declaration.clone(),
            fingerprint: function.interface_fingerprint(),
        };
        let mut dependencies = BTreeSet::new();
        self.add_effect_and_capability_dependencies(function, identities, &mut dependencies)?;

        // Call and nominal dependencies come from typed semantic references,
        // not from matching arbitrary strings in the serialized body. This
        // keeps labels, diagnostics, and user data from creating false edges.
        // Assumption identities are still conservatively found in the body
        // representation because their current payload is string-addressed.
        let body_strings = strings_in_json(&serde_json::to_value(body).ok()?);
        let calls = body_call_references(body);
        let referenced_functions = calls
            .iter()
            .map(|call| call.function.clone())
            .collect::<BTreeSet<_>>();
        let relevant_specializations = self
            .generic_specializations
            .iter()
            .filter(|specialization| {
                specialization_is_relevant(specialization, &declaration, &calls)
            })
            .collect::<Vec<_>>();
        let mut nominal_types = body_nominal_type_references(self, function, body);
        for specialization in &relevant_specializations {
            for argument in &specialization.args {
                if let GenericArg::Type { ty } = argument {
                    collect_nominal_type_references(self, ty, &mut nominal_types);
                }
            }
        }
        for candidate in &self.functions {
            let identity = function_id(candidate.identity_namespace(&self.module), &candidate.name);
            if identity != declaration && referenced_functions.contains(&identity) {
                dependencies.insert(VerificationFactDependency {
                    role: VerificationDependencyRole::CallableInterface,
                    identity,
                    fingerprint: candidate.interface_fingerprint(),
                });
                self.add_effect_and_capability_dependencies(
                    candidate,
                    identities,
                    &mut dependencies,
                )?;
            }
        }

        // `SemanticIdentities` preserves the legacy rule that record objects
        // are enumerated only for source-bound programs. The verifier still
        // resolves record types from declaration-only model programs, so
        // capture those exact existing record identities and their canonical
        // identity fingerprint directly as dependencies too.
        for record_type in &self.record_types {
            if nominal_types.contains(&record_type.identity) {
                let serialized = serde_json::to_string(record_type).ok()?;
                dependencies.insert(VerificationFactDependency {
                    role: VerificationDependencyRole::NominalType,
                    identity: record_type.identity.clone(),
                    fingerprint: crate::sha256_hex(serialized.as_bytes()),
                });
            }
        }

        for record in &identities.objects {
            match record.kind {
                crate::IdentityKind::FiniteType if nominal_types.contains(&record.identity) => {
                    dependencies.insert(VerificationFactDependency {
                        role: VerificationDependencyRole::NominalType,
                        identity: record.identity.clone(),
                        fingerprint: record.fingerprint.clone(),
                    });
                }
                crate::IdentityKind::Assumption => {
                    let used = self.assumptions.iter().any(|assumption| {
                        let assumption_identity =
                            crate::identity::assumption_id(&self.module, &assumption.id);
                        assumption_identity == record.identity
                            && (function.assumptions.contains(&assumption.id)
                                || body_strings.contains(assumption.id.as_str()))
                    });
                    if used {
                        dependencies.insert(VerificationFactDependency {
                            role: VerificationDependencyRole::Assumption,
                            identity: record.identity.clone(),
                            fingerprint: record.fingerprint.clone(),
                        });
                    }
                }
                _ => {}
            }
        }

        for specialization in relevant_specializations {
            let identity = specialization_id(
                &specialization.generic_function,
                &specialization.canonical_args,
            );
            if let Some(fingerprint) =
                self.generic_specialization_fingerprint(specialization, &identity, identities)
            {
                dependencies.insert(VerificationFactDependency {
                    role: VerificationDependencyRole::GenericSpecialization,
                    identity,
                    fingerprint,
                });
            }
        }

        dependencies.insert(declaration_interface.clone());
        Some(BodyFactBasis {
            subject,
            subject_fingerprint,
            declaration_interface,
            dependencies: dependencies.into_iter().collect(),
            declared_effects: function.effects.clone(),
            declared_capabilities: function.capabilities.clone(),
        })
    }

    fn add_effect_and_capability_dependencies(
        &self,
        function: &Function,
        identities: &SemanticIdentities,
        dependencies: &mut BTreeSet<VerificationFactDependency>,
    ) -> Option<()> {
        let namespace = function.identity_namespace(&self.module);
        for capability in &function.capabilities {
            let identity = crate::identity::capability_id(namespace, &function.name, capability);
            dependencies.insert(VerificationFactDependency {
                role: VerificationDependencyRole::Capability,
                fingerprint: identities.fingerprint(&identity)?.to_owned(),
                identity,
            });
        }

        let mut effect_occurrences = BTreeMap::<String, usize>::new();
        for effect in &function.effects {
            let canonical =
                crate::canonical::canonical_json_value(&crate::canonical::canonical_effect(effect))
                    .ok()?;
            let occurrence = effect_occurrences.entry(canonical.clone()).or_default();
            let identity =
                crate::identity::effect_id(namespace, &function.name, &canonical, *occurrence);
            *occurrence += 1;
            dependencies.insert(VerificationFactDependency {
                role: VerificationDependencyRole::Effect,
                fingerprint: identities.fingerprint(&identity)?.to_owned(),
                identity,
            });
        }
        Some(())
    }

    fn generic_specialization_fingerprint(
        &self,
        specialization: &GenericSpecializationRecord,
        identity: &SemanticId,
        identities: &SemanticIdentities,
    ) -> Option<String> {
        let record_fingerprint = identities.fingerprint(identity)?;
        let generic_function_fingerprint =
            self.function_semantic_fingerprint(&specialization.generic_function)?;
        let specialized_function_fingerprint =
            self.function_semantic_fingerprint(&specialization.specialization_function)?;
        let bytes = serde_json::to_vec(&(
            record_fingerprint,
            generic_function_fingerprint,
            specialized_function_fingerprint,
        ))
        .ok()?;
        Some(crate::sha256_hex(&bytes))
    }

    fn function_semantic_fingerprint(&self, identity: &SemanticId) -> Option<String> {
        let function = self.functions.iter().find(|function| {
            function_id(function.identity_namespace(&self.module), &function.name) == *identity
        })?;
        let body_fingerprint = function
            .body
            .as_ref()
            .map(|body| body.fingerprint())
            .transpose()
            .ok()?;
        #[derive(Serialize)]
        struct FunctionSemanticFingerprint<'a> {
            interface_fingerprint: String,
            body_fingerprint: Option<String>,
            effects: &'a [Effect],
            capabilities: &'a [String],
        }
        let material = FunctionSemanticFingerprint {
            interface_fingerprint: function.interface_fingerprint(),
            body_fingerprint,
            effects: &function.effects,
            capabilities: &function.capabilities,
        };
        let bytes = serde_json::to_vec(&material).ok()?;
        Some(crate::sha256_hex(&bytes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BodyCallReference {
    function: SemanticId,
    instantiation: Option<SemanticId>,
    specialization: Option<SemanticId>,
    canonical_arguments: Option<String>,
}

fn body_call_references(body: &FunctionBody) -> BTreeSet<BodyCallReference> {
    let mut calls = BTreeSet::new();
    for block in &body.blocks {
        for operation in &block.operations {
            if let BodyOperationKind::Call {
                function,
                generic_args,
                instantiation,
                specialization,
                ..
            } = &operation.kind
            {
                calls.insert(BodyCallReference {
                    function: function.clone(),
                    instantiation: instantiation.clone(),
                    specialization: specialization.clone(),
                    canonical_arguments: (!generic_args.is_empty()).then(|| {
                        generic_args
                            .iter()
                            .map(GenericArg::canonical_string)
                            .collect::<Vec<_>>()
                            .join("|")
                    }),
                });
            }
        }
    }
    // The iteration closure is validated against its body blocks. Including
    // its explicit call set keeps the dependency basis aligned with the
    // verified iteration summary as well as the individual operations.
    for iteration in &body.bounded_iterations {
        calls.extend(
            iteration
                .callees
                .iter()
                .cloned()
                .map(|function| BodyCallReference {
                    function,
                    instantiation: None,
                    specialization: None,
                    canonical_arguments: None,
                }),
        );
    }
    calls
}

fn body_nominal_type_references(
    program: &Program,
    function: &Function,
    body: &FunctionBody,
) -> BTreeSet<SemanticId> {
    let mut identities = BTreeSet::new();
    for value in function.inputs.iter().chain(&function.outputs) {
        let ty = BodyType::from_semantic_name(&value.value_type);
        collect_nominal_type_references(program, &ty, &mut identities);
    }
    for parameter in &body.parameters {
        collect_nominal_type_references(program, &parameter.ty, &mut identities);
    }
    for iteration in &body.bounded_iterations {
        collect_nominal_type_references(program, &iteration.state_type, &mut identities);
        if let crate::IterationDomain::OverSequence { element_type } = &iteration.domain {
            collect_nominal_type_references(program, element_type, &mut identities);
        }
    }
    for block in &body.blocks {
        for value in &block.parameters {
            collect_nominal_type_references(program, &value.ty, &mut identities);
        }
        for operation in &block.operations {
            for result in &operation.results {
                collect_nominal_type_references(program, &result.ty, &mut identities);
            }
            match &operation.kind {
                BodyOperationKind::Select { operand_type }
                | BodyOperationKind::SequenceReplace {
                    element_type: operand_type,
                    ..
                }
                | BodyOperationKind::SequenceCopy {
                    element_type: operand_type,
                    ..
                }
                | BodyOperationKind::VectorConstruct {
                    element_type: operand_type,
                    ..
                }
                | BodyOperationKind::VectorSplat {
                    element_type: operand_type,
                    ..
                }
                | BodyOperationKind::VectorExtract {
                    element_type: operand_type,
                    ..
                }
                | BodyOperationKind::VectorReplace {
                    element_type: operand_type,
                    ..
                }
                | BodyOperationKind::VectorBinary {
                    element_type: operand_type,
                    ..
                }
                | BodyOperationKind::VectorCompare {
                    element_type: operand_type,
                    ..
                }
                | BodyOperationKind::VectorReduce {
                    element_type: operand_type,
                    ..
                }
                | BodyOperationKind::SequenceConstruct {
                    element_type: operand_type,
                    ..
                } => collect_nominal_type_references(program, operand_type, &mut identities),
                BodyOperationKind::Convert { from, to } => {
                    collect_nominal_type_references(program, from, &mut identities);
                    collect_nominal_type_references(program, to, &mut identities);
                }
                BodyOperationKind::FiniteConstruct { type_identity, .. }
                | BodyOperationKind::FiniteIsVariant { type_identity, .. }
                | BodyOperationKind::FinitePayloadProject { type_identity, .. }
                | BodyOperationKind::RecordConstruct { type_identity, .. }
                | BodyOperationKind::RecordProject { type_identity, .. } => {
                    identities.insert(type_identity.clone());
                }
                BodyOperationKind::Call { generic_args, .. } => {
                    for argument in generic_args {
                        if let GenericArg::Type { ty } = argument {
                            collect_nominal_type_references(program, ty, &mut identities);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    identities
}

fn collect_nominal_type_references(
    program: &Program,
    ty: &BodyType,
    identities: &mut BTreeSet<SemanticId>,
) {
    match ty {
        BodyType::Finite { identity, .. } | BodyType::Record { identity, .. } => {
            identities.insert(identity.clone());
        }
        BodyType::Named(name) => {
            identities.extend(
                program
                    .finite_types
                    .iter()
                    .filter(|item| item.identity.as_str() == name || item.name == *name)
                    .map(|item| item.identity.clone()),
            );
            identities.extend(
                program
                    .record_types
                    .iter()
                    .filter(|item| item.identity.as_str() == name || item.name == *name)
                    .map(|item| item.identity.clone()),
            );
        }
        BodyType::Sequence { element, .. } | BodyType::Vector { element, .. } => {
            collect_nominal_type_references(program, element, identities);
        }
        _ => {}
    }
}

impl VerificationFact {
    pub(crate) fn from_basis(
        basis: BodyFactBasis,
        language_profile: &str,
        source_location_identity: Option<String>,
    ) -> Self {
        let mut invalidating_changes = vec![
            VerificationFactInvalidator::SubjectBodyChanged(basis.subject.clone()),
            VerificationFactInvalidator::DeclarationInterfaceChanged(
                basis.declaration_interface.identity.clone(),
            ),
            VerificationFactInvalidator::VerifierChanged,
            VerificationFactInvalidator::PolicyChanged,
            VerificationFactInvalidator::LanguageProfileChanged,
        ];
        invalidating_changes.extend(basis.dependencies.iter().map(
            |dependency| match dependency.role {
                VerificationDependencyRole::Effect => {
                    VerificationFactInvalidator::EffectAssumptionChanged(
                        dependency.identity.clone(),
                    )
                }
                VerificationDependencyRole::Capability => {
                    VerificationFactInvalidator::CapabilityAssumptionChanged(
                        dependency.identity.clone(),
                    )
                }
                _ => VerificationFactInvalidator::DependencyChanged(dependency.identity.clone()),
            },
        ));
        let mut fact = Self {
            schema_version: VERIFICATION_FACT_SCHEMA_VERSION.to_owned(),
            subject: basis.subject,
            subject_fingerprint: basis.subject_fingerprint,
            verifier_identity: VERIFICATION_FACT_VERIFIER_ID.to_owned(),
            policy_identity: VERIFICATION_FACT_POLICY_ID.to_owned(),
            language_profile: language_profile.to_owned(),
            declaration_interface: basis.declaration_interface,
            dependencies: basis.dependencies,
            declared_effects: basis.declared_effects,
            declared_capabilities: basis.declared_capabilities,
            result: VerificationFactResult::Verified,
            verification_witness_identity: String::new(),
            invalidating_changes,
            source_location_identity,
        };
        fact.verification_witness_identity = fact.computed_witness_identity();
        fact
    }

    pub(crate) fn current_basis_matches(
        &self,
        current: &Self,
    ) -> Result<(), VerificationFactReuseMiss> {
        if self.schema_version != VERIFICATION_FACT_SCHEMA_VERSION {
            return Err(VerificationFactReuseMiss::UnsupportedSchema);
        }
        if self.result != VerificationFactResult::Verified {
            return Err(VerificationFactReuseMiss::UnsupportedResult);
        }
        if self.verification_witness_identity != self.computed_witness_identity() {
            return Err(VerificationFactReuseMiss::InvalidWitness);
        }
        if self.subject != current.subject
            || self.subject_fingerprint != current.subject_fingerprint
        {
            return Err(VerificationFactReuseMiss::SubjectChanged);
        }
        if self.declared_effects != current.declared_effects
            || dependencies_for_role(&self.dependencies, VerificationDependencyRole::Effect)
                != dependencies_for_role(&current.dependencies, VerificationDependencyRole::Effect)
        {
            return Err(VerificationFactReuseMiss::EffectAssumptionsChanged);
        }
        if self.declared_capabilities != current.declared_capabilities
            || dependencies_for_role(&self.dependencies, VerificationDependencyRole::Capability)
                != dependencies_for_role(
                    &current.dependencies,
                    VerificationDependencyRole::Capability,
                )
        {
            return Err(VerificationFactReuseMiss::CapabilityAssumptionsChanged);
        }
        if self.declaration_interface != current.declaration_interface {
            return Err(VerificationFactReuseMiss::DeclarationInterfaceChanged);
        }
        if self.dependencies != current.dependencies {
            return Err(VerificationFactReuseMiss::DependenciesChanged);
        }
        if self.invalidating_changes != current.invalidating_changes {
            return Err(VerificationFactReuseMiss::DependenciesChanged);
        }
        if self.verifier_identity != current.verifier_identity {
            return Err(VerificationFactReuseMiss::VerifierChanged);
        }
        if self.policy_identity != current.policy_identity {
            return Err(VerificationFactReuseMiss::PolicyChanged);
        }
        if self.language_profile != current.language_profile {
            return Err(VerificationFactReuseMiss::LanguageProfileChanged);
        }
        Ok(())
    }

    fn computed_witness_identity(&self) -> String {
        #[derive(Serialize)]
        struct Witness<'a> {
            schema_version: &'a str,
            subject: &'a SemanticId,
            subject_fingerprint: &'a str,
            verifier_identity: &'a str,
            policy_identity: &'a str,
            language_profile: &'a str,
            declaration_interface: &'a VerificationFactDependency,
            dependencies: &'a [VerificationFactDependency],
            declared_effects: &'a [Effect],
            declared_capabilities: &'a [String],
            result: VerificationFactResult,
            invalidating_changes: &'a [VerificationFactInvalidator],
        }
        let witness = Witness {
            schema_version: &self.schema_version,
            subject: &self.subject,
            subject_fingerprint: &self.subject_fingerprint,
            verifier_identity: &self.verifier_identity,
            policy_identity: &self.policy_identity,
            language_profile: &self.language_profile,
            declaration_interface: &self.declaration_interface,
            dependencies: &self.dependencies,
            declared_effects: &self.declared_effects,
            declared_capabilities: &self.declared_capabilities,
            result: self.result,
            invalidating_changes: &self.invalidating_changes,
        };
        let bytes = serde_json::to_vec(&witness).expect("verification witness is serializable");
        crate::sha256_hex(&bytes)
    }
}

fn dependencies_for_role(
    dependencies: &[VerificationFactDependency],
    role: VerificationDependencyRole,
) -> Vec<&VerificationFactDependency> {
    dependencies
        .iter()
        .filter(|dependency| dependency.role == role)
        .collect()
}

impl DeclarationVerificationFact {
    pub(crate) fn from_basis(
        basis: DeclarationFactBasis,
        language_profile: &str,
        warnings: Vec<DeclarationVerificationWarning>,
        source_location_identity: Option<String>,
    ) -> Self {
        let mut invalidating_changes = vec![
            DeclarationFactInvalidator::SubjectDeclarationChanged(basis.subject.clone()),
            DeclarationFactInvalidator::VerifierChanged,
            DeclarationFactInvalidator::PolicyChanged,
            DeclarationFactInvalidator::LanguageProfileChanged,
        ];
        invalidating_changes.extend(basis.dependencies.iter().map(|dependency| {
            DeclarationFactInvalidator::DependencyChanged(dependency.identity.clone())
        }));
        let mut fact = Self {
            schema_version: DECLARATION_FACT_SCHEMA_VERSION.to_owned(),
            subject: basis.subject,
            subject_fingerprint: basis.subject_fingerprint,
            verifier_identity: DECLARATION_FACT_VERIFIER_ID.to_owned(),
            policy_identity: DECLARATION_FACT_POLICY_ID.to_owned(),
            language_profile: language_profile.to_owned(),
            dependencies: basis.dependencies,
            result: VerificationFactResult::Verified,
            verification_witness_identity: String::new(),
            invalidating_changes,
            warnings,
            source_location_identity,
        };
        fact.verification_witness_identity = fact.computed_witness_identity();
        fact
    }

    pub(crate) fn current_basis_matches(
        &self,
        current: &Self,
    ) -> Result<(), DeclarationFactReuseMiss> {
        if self.schema_version != DECLARATION_FACT_SCHEMA_VERSION {
            return Err(DeclarationFactReuseMiss::UnsupportedSchema);
        }
        if self.result != VerificationFactResult::Verified {
            return Err(DeclarationFactReuseMiss::UnsupportedResult);
        }
        if self.verification_witness_identity != self.computed_witness_identity() {
            return Err(DeclarationFactReuseMiss::InvalidWitness);
        }
        if self.subject != current.subject
            || self.subject_fingerprint != current.subject_fingerprint
        {
            return Err(DeclarationFactReuseMiss::SubjectChanged);
        }
        if self.dependencies != current.dependencies
            || self.invalidating_changes != current.invalidating_changes
        {
            return Err(DeclarationFactReuseMiss::DependenciesChanged);
        }
        if self.verifier_identity != current.verifier_identity {
            return Err(DeclarationFactReuseMiss::VerifierChanged);
        }
        if self.policy_identity != current.policy_identity {
            return Err(DeclarationFactReuseMiss::PolicyChanged);
        }
        if self.language_profile != current.language_profile {
            return Err(DeclarationFactReuseMiss::LanguageProfileChanged);
        }
        Ok(())
    }

    fn computed_witness_identity(&self) -> String {
        #[derive(Serialize)]
        struct Witness<'a> {
            schema_version: &'a str,
            subject: &'a SemanticId,
            subject_fingerprint: &'a str,
            verifier_identity: &'a str,
            policy_identity: &'a str,
            language_profile: &'a str,
            dependencies: &'a [VerificationFactDependency],
            result: VerificationFactResult,
            invalidating_changes: &'a [DeclarationFactInvalidator],
            warnings: &'a [DeclarationVerificationWarning],
        }
        let witness = Witness {
            schema_version: &self.schema_version,
            subject: &self.subject,
            subject_fingerprint: &self.subject_fingerprint,
            verifier_identity: &self.verifier_identity,
            policy_identity: &self.policy_identity,
            language_profile: &self.language_profile,
            dependencies: &self.dependencies,
            result: self.result,
            invalidating_changes: &self.invalidating_changes,
            warnings: &self.warnings,
        };
        let bytes = serde_json::to_vec(&witness).expect("declaration fact witness is serializable");
        crate::sha256_hex(&bytes)
    }
}

fn specialization_is_relevant(
    specialization: &GenericSpecializationRecord,
    declaration: &SemanticId,
    calls: &BTreeSet<BodyCallReference>,
) -> bool {
    specialization.specialization_function == *declaration
        || calls.iter().any(|call| {
            let addresses_specialization = call.instantiation.as_ref()
                == Some(&specialization.instantiation)
                || call.specialization.as_ref() == Some(&specialization.specialization_function)
                || call.canonical_arguments.as_deref()
                    == Some(specialization.canonical_args.as_str());
            addresses_specialization
                && (call.function == specialization.generic_function
                    || call.function == specialization.specialization_function)
        })
}

fn strings_in_json(value: &serde_json::Value) -> BTreeSet<String> {
    fn collect(value: &serde_json::Value, strings: &mut BTreeSet<String>) {
        match value {
            serde_json::Value::String(value) => {
                strings.insert(value.clone());
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    collect(value, strings);
                }
            }
            serde_json::Value::Object(values) => {
                for value in values.values() {
                    collect(value, strings);
                }
            }
            _ => {}
        }
    }
    let mut strings = BTreeSet::new();
    collect(value, &mut strings);
    strings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        function_id, record_type_id, BodyBlock, BodyOperation, BodyOperationKind, BodyTerminator,
        BodyType, BodyValue, RecordField, RecordType, Value,
    };

    fn request(profile: &str, prior_facts: Vec<VerificationFact>) -> VerificationFactRequest {
        VerificationFactRequest {
            language_profile: profile.to_owned(),
            subject_language_profiles: BTreeMap::new(),
            prior_facts,
            prior_declaration_facts: Vec::new(),
            source_location_identities: BTreeMap::new(),
        }
    }

    fn declaration_request(
        profile: &str,
        prior_facts: Vec<DeclarationVerificationFact>,
    ) -> VerificationFactRequest {
        let mut request = request(profile, Vec::new());
        request.prior_declaration_facts = prior_facts;
        request
    }

    fn record_program() -> Program {
        let mut program = crate::body::tests::executable_program();
        let identity = record_type_id("Banking.Transfer", "Money", &[("amount", "i64")]);
        program.record_types.push(RecordType {
            identity: identity.clone(),
            name: "Money".to_owned(),
            fields: vec![RecordField {
                name: "amount".to_owned(),
                field_type: "i64".to_owned(),
            }],
        });
        let function = &mut program.functions[0];
        function.inputs = vec![Value {
            name: "money".to_owned(),
            value_type: "Money".to_owned(),
        }];
        function.outputs = vec![Value {
            name: "result".to_owned(),
            value_type: "Money".to_owned(),
        }];
        let body = function.body.as_mut().unwrap();
        body.parameters[0].name = "money".to_owned();
        body.parameters[0].id = "money".to_owned();
        body.parameters[0].ty = BodyType::Record {
            identity,
            name: "Money".to_owned(),
        };
        body.blocks[0].operations.clear();
        body.blocks[0].terminator = BodyTerminator::Return {
            values: vec!["money".to_owned()],
        };
        program
    }

    fn call_program() -> Program {
        let mut program = crate::body::tests::executable_program();
        let mut callee = program.functions[0].clone();
        callee.name = "callee".to_owned();
        callee.body = None;
        let mut caller = program.functions[0].clone();
        caller.name = "caller".to_owned();
        let body = caller.body.as_mut().unwrap();
        body.blocks = vec![BodyBlock {
            id: "entry".to_owned(),
            parameters: Vec::new(),
            operations: vec![BodyOperation {
                id: "call".to_owned(),
                kind: BodyOperationKind::Call {
                    function: function_id(&program.module, "callee"),
                    function_name: "callee".to_owned(),
                    required_capabilities: Vec::new(),
                    effects: Vec::new(),
                    generic_args: Vec::new(),
                    instantiation: None,
                    specialization: None,
                },
                operands: vec!["a".to_owned()],
                results: vec![BodyValue {
                    id: "result".to_owned(),
                    ty: BodyType::Integer(crate::IntegerType {
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
            terminator: BodyTerminator::Return {
                values: vec!["result".to_owned()],
            },
        }];
        let mut independent = program.functions[0].clone();
        independent.name = "independent".to_owned();
        program.functions = vec![callee, caller, independent];
        program
    }

    #[test]
    fn verified_body_fact_round_trips_and_refreshes_locations() {
        let program = crate::body::tests::executable_program();
        let subject = program.functions[0]
            .body
            .as_ref()
            .unwrap()
            .identity(&program.module, &program.functions[0].name);
        let first = program.validate_with_verified_facts(&request("0.19", Vec::new()));
        assert!(first.report.valid, "{:?}", first.report.errors);
        assert!(first.reuse.reused.is_empty());
        assert_eq!(first.reuse.recomputed.len(), 1);
        assert_eq!(
            first.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::Missing
        );
        assert_eq!(first.facts.len(), 1);
        assert_eq!(
            first.facts[0].schema_version,
            VERIFICATION_FACT_SCHEMA_VERSION
        );
        assert_eq!(
            first.reuse.schema_version,
            VERIFICATION_FACT_REUSE_OBSERVATION_SCHEMA_VERSION
        );
        assert_eq!(first.facts[0].subject, subject);
        assert_eq!(first.facts[0].result, VerificationFactResult::Verified);
        assert_eq!(
            first.facts[0].verification_witness_identity.len(),
            64,
            "witness identity must be a SHA-256 digest"
        );

        // Simulate a fresh process loading the versioned facts from a cache.
        let bytes = serde_json::to_vec(&first.facts).unwrap();
        let restored: Vec<VerificationFact> = serde_json::from_slice(&bytes).unwrap();
        let mut second_request = request("0.19", restored);
        second_request
            .source_location_identities
            .insert(subject.clone(), "source-map:current-snapshot".to_owned());
        let second = program.validate_with_verified_facts(&second_request);
        assert!(second.report.valid, "{:?}", second.report.errors);
        assert_eq!(second.reuse.reused, vec![subject]);
        assert!(second.reuse.recomputed.is_empty());
        assert_eq!(
            second.facts[0].source_location_identity.as_deref(),
            Some("source-map:current-snapshot")
        );
    }

    #[test]
    fn changed_declaration_interface_rechecks_body_and_reports_fresh_error() {
        let original = crate::body::tests::executable_program();
        let cached = original
            .validate_with_verified_facts(&request("0.19", Vec::new()))
            .facts;
        let mut changed = original;
        changed.functions[0].inputs[0].value_type = "i64".to_owned();

        let result = changed.validate_with_verified_facts(&request("0.19", cached));
        assert!(!result.report.valid);
        assert!(result
            .report
            .errors
            .iter()
            .any(|error| error.code == "MNB005"));
        assert_eq!(result.reuse.recomputed.len(), 1);
        assert_eq!(
            result.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::DeclarationInterfaceChanged
        );
        assert!(result.reuse.reused.is_empty());
        assert!(result.facts.is_empty());
    }

    #[test]
    fn changed_body_and_profile_cannot_reuse_a_successful_fact() {
        let original = crate::body::tests::executable_program();
        let cached = original
            .validate_with_verified_facts(&request("0.19", Vec::new()))
            .facts;

        let mut body_changed = original.clone();
        body_changed.functions[0].body.as_mut().unwrap().entry = "missing".to_owned();
        let invalid = body_changed.validate_with_verified_facts(&request("0.19", cached.clone()));
        assert!(!invalid.report.valid);
        assert!(invalid
            .report
            .errors
            .iter()
            .any(|error| error.code == "MNB006"));
        assert_eq!(
            invalid.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::SubjectChanged
        );
        assert!(invalid.facts.is_empty());

        let changed_profile = original.validate_with_verified_facts(&request("0.20", cached));
        assert!(changed_profile.report.valid);
        assert_eq!(
            changed_profile.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::LanguageProfileChanged
        );
        assert!(changed_profile.reuse.reused.is_empty());
    }

    #[test]
    fn whole_program_declaration_errors_are_not_hidden_by_body_reuse() {
        let original = crate::body::tests::executable_program();
        let cached = original
            .validate_with_verified_facts(&request("0.19", Vec::new()))
            .facts;
        let mut invalid = original.clone();
        let mut duplicate = invalid.functions[0].clone();
        duplicate.body = None;
        invalid.functions.push(duplicate);

        let result = invalid.validate_with_verified_facts(&request("0.19", cached));
        assert!(!result.report.valid);
        assert!(result
            .report
            .errors
            .iter()
            .any(|error| error.code == "MNCS005"));
        assert_eq!(result.reuse.reused.len(), 1);
    }

    #[test]
    fn nominal_dependencies_invalidate_only_functions_that_reference_them() {
        let program = record_program();
        let first = program.validate_with_verified_facts(&request("0.19", Vec::new()));
        assert!(first.report.valid, "{:?}", first.report.errors);
        assert_eq!(first.facts[0].dependencies.len(), 2);
        assert!(first.facts[0]
            .dependencies
            .iter()
            .any(|dependency| dependency.role == VerificationDependencyRole::NominalType));

        let mut changed_record = program.clone();
        let record = &mut changed_record.record_types[0];
        record.fields[0].field_type = "u64".to_owned();
        record.identity = record_type_id("Banking.Transfer", "Money", &[("amount", "u64")]);
        let changed =
            changed_record.validate_with_verified_facts(&request("0.19", first.facts.clone()));
        assert!(!changed.report.valid);
        assert!(changed
            .report
            .errors
            .iter()
            .any(|error| error.code == "MNB005"));
        assert_eq!(
            changed.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::DependenciesChanged
        );
        assert!(changed.reuse.reused.is_empty());

        let mut unrelated_change = program.clone();
        unrelated_change.record_types.push(RecordType {
            identity: record_type_id("Banking.Transfer", "Unused", &[("field", "i32")]),
            name: "Unused".to_owned(),
            fields: vec![RecordField {
                name: "field".to_owned(),
                field_type: "i32".to_owned(),
            }],
        });
        let reused = unrelated_change.validate_with_verified_facts(&request("0.19", first.facts));
        assert!(reused.report.valid, "{:?}", reused.report.errors);
        assert_eq!(reused.reuse.reused.len(), 1);
        assert!(reused.reuse.recomputed.is_empty());
    }

    #[test]
    fn call_name_text_does_not_create_a_nominal_type_dependency() {
        let mut program = call_program();
        let decoy_identity = record_type_id("Banking.Transfer", "callee", &[("amount", "i32")]);
        program.record_types.push(RecordType {
            identity: decoy_identity.clone(),
            name: "callee".to_owned(),
            fields: vec![RecordField {
                name: "amount".to_owned(),
                field_type: "i32".to_owned(),
            }],
        });
        let identities = program.semantic_identities();
        let caller = program
            .functions
            .iter()
            .find(|function| function.name == "caller")
            .unwrap();
        let basis = program
            .body_fact_basis_with_identities(caller, &identities)
            .unwrap();

        assert!(!basis.dependencies.iter().any(|dependency| {
            dependency.role == VerificationDependencyRole::NominalType
                && dependency.identity == decoy_identity
        }));
        assert!(basis.dependencies.iter().any(|dependency| {
            dependency.role == VerificationDependencyRole::CallableInterface
                && dependency.identity == function_id(&program.module, "callee")
        }));
    }

    #[test]
    fn generic_specialization_dependency_uses_the_resolved_call_selector() {
        let mut program = call_program();
        let generic_function = function_id(&program.module, "generic_callee");
        let specialized_function = function_id(&program.module, "generic_callee__spec_4");
        let instantiation = SemanticId("mncs:0.2:instantiation:generic_callee:value:4".to_owned());
        let specialization = GenericSpecializationRecord {
            generic_function: generic_function.clone(),
            specialization_function: specialized_function.clone(),
            instantiation: instantiation.clone(),
            args: vec![GenericArg::Value { value: 4 }],
            canonical_args: "value:4".to_owned(),
            host_spellings: Vec::new(),
        };
        let body = program
            .functions
            .iter_mut()
            .find(|function| function.name == "caller")
            .unwrap()
            .body
            .as_mut()
            .unwrap();
        let BodyOperationKind::Call {
            function,
            generic_args,
            instantiation: call_instantiation,
            specialization: call_specialization,
            ..
        } = &mut body.blocks[0].operations[0].kind
        else {
            unreachable!("call_program contains a call")
        };
        *function = generic_function.clone();
        *generic_args = vec![GenericArg::Value { value: 4 }];
        *call_instantiation = Some(instantiation.clone());
        *call_specialization = Some(specialized_function.clone());

        let calls = body_call_references(body);
        assert!(specialization_is_relevant(
            &specialization,
            &function_id(&program.module, "caller"),
            &calls,
        ));

        let other_specialization = GenericSpecializationRecord {
            generic_function: generic_function.clone(),
            specialization_function: function_id(&program.module, "generic_callee__spec_8"),
            instantiation: SemanticId("mncs:0.2:instantiation:generic_callee:value:8".to_owned()),
            args: vec![GenericArg::Value { value: 8 }],
            canonical_args: "value:8".to_owned(),
            host_spellings: Vec::new(),
        };
        assert!(!specialization_is_relevant(
            &other_specialization,
            &function_id(&program.module, "caller"),
            &calls,
        ));
    }

    #[test]
    fn specialized_generic_type_arguments_retain_nominal_dependencies() {
        let mut program = call_program();
        let nominal_identity = record_type_id("Banking.Transfer", "Money", &[("amount", "i32")]);
        program.record_types.push(RecordType {
            identity: nominal_identity.clone(),
            name: "Money".to_owned(),
            fields: vec![RecordField {
                name: "amount".to_owned(),
                field_type: "i32".to_owned(),
            }],
        });

        let mut generic_function = program
            .functions
            .iter()
            .find(|function| function.name == "callee")
            .unwrap()
            .clone();
        generic_function.name = "generic_callee".to_owned();
        let mut specialized_function = generic_function.clone();
        specialized_function.name = "generic_callee__spec_money".to_owned();
        let generic_identity = function_id(&program.module, &generic_function.name);
        let specialized_identity = function_id(&program.module, &specialized_function.name);
        let instantiation = SemanticId("mncs:0.2:instantiation:generic_callee:money".to_owned());
        let argument = GenericArg::Type {
            ty: BodyType::Record {
                identity: nominal_identity.clone(),
                name: "Money".to_owned(),
            },
        };
        let canonical_args = argument.canonical_string();
        program.generic_specializations = vec![GenericSpecializationRecord {
            generic_function: generic_identity,
            specialization_function: specialized_identity.clone(),
            instantiation: instantiation.clone(),
            args: vec![argument],
            canonical_args,
            host_spellings: Vec::new(),
        }];
        program
            .functions
            .extend([generic_function, specialized_function.clone()]);

        let caller = program
            .functions
            .iter_mut()
            .find(|function| function.name == "caller")
            .unwrap();
        let BodyOperationKind::Call {
            function,
            function_name,
            generic_args,
            instantiation: call_instantiation,
            specialization: call_specialization,
            ..
        } = &mut caller.body.as_mut().unwrap().blocks[0].operations[0].kind
        else {
            unreachable!("call_program contains a call")
        };
        *function = specialized_identity.clone();
        *function_name = specialized_function.name;
        generic_args.clear();
        *call_instantiation = Some(instantiation);
        *call_specialization = Some(specialized_identity);

        let identities = program.semantic_identities();
        let caller = program
            .functions
            .iter()
            .find(|function| function.name == "caller")
            .unwrap();
        let basis = program
            .body_fact_basis_with_identities(caller, &identities)
            .unwrap();
        assert!(basis.dependencies.iter().any(|dependency| {
            dependency.role == VerificationDependencyRole::NominalType
                && dependency.identity == nominal_identity
        }));
        assert!(
            basis
                .dependencies
                .iter()
                .any(|dependency| dependency.role
                    == VerificationDependencyRole::GenericSpecialization)
        );
    }

    #[test]
    fn changed_callee_interface_rechecks_callers_and_returns_current_diagnostics() {
        let program = call_program();
        let first = program.validate_with_verified_facts(&request("0.19", Vec::new()));
        assert!(first.report.valid, "{:?}", first.report.errors);
        let caller_fact = first
            .facts
            .iter()
            .find(|fact| fact.subject.as_str().ends_with("::caller"))
            .expect("caller body fact");
        let caller_subject = caller_fact.subject.clone();
        let independent_subject = program.functions[2]
            .body
            .as_ref()
            .unwrap()
            .identity(&program.module, "independent");
        assert!(caller_fact.dependencies.iter().any(|dependency| {
            dependency.role == VerificationDependencyRole::CallableInterface
                && dependency.identity == function_id(&program.module, "callee")
        }));

        let mut changed = program;
        changed.functions[0].outputs[0].value_type = "i64".to_owned();
        let result = changed.validate_with_verified_facts(&request("0.19", first.facts));
        assert!(!result.report.valid);
        assert!(result
            .report
            .errors
            .iter()
            .any(|error| error.code == "MNB052"));
        let caller_recomputation = result
            .reuse
            .recomputed
            .iter()
            .find(|row| row.subject == caller_subject)
            .expect("caller invalidated by callee interface");
        assert_eq!(
            caller_recomputation.reason,
            VerificationFactReuseMiss::DependenciesChanged
        );
        assert_eq!(result.reuse.reused, vec![independent_subject]);
    }

    #[test]
    fn effect_and_capability_dependencies_recheck_only_their_callers() {
        let program = call_program();
        let first = program.validate_with_verified_facts(&request("0.19", Vec::new()));
        assert!(first.report.valid, "{:?}", first.report.errors);
        let caller_subject = first
            .facts
            .iter()
            .find(|fact| fact.subject.as_str().ends_with("::caller"))
            .expect("caller body fact")
            .subject
            .clone();
        let independent_subject = program.functions[2]
            .body
            .as_ref()
            .unwrap()
            .identity(&program.module, "independent");

        let mut effect_changed = program.clone();
        effect_changed.functions[0].effects.push(Effect {
            kind: "read".to_owned(),
            target: "storage".to_owned(),
            capability: "storage.read".to_owned(),
        });
        let effect_identities = effect_changed.semantic_identities();
        let effect_caller = effect_changed
            .functions
            .iter()
            .find(|function| function.name == "caller")
            .unwrap();
        let effect_basis = effect_changed
            .body_fact_basis_with_identities(effect_caller, &effect_identities)
            .unwrap();
        assert!(effect_basis.dependencies.iter().any(|dependency| {
            dependency.role == VerificationDependencyRole::Effect
                && dependency.identity.as_str().contains("callee")
        }));
        let effect_result =
            effect_changed.validate_with_verified_facts(&request("0.19", first.facts.clone()));
        let effect_recomputation = effect_result
            .reuse
            .recomputed
            .iter()
            .find(|row| row.subject == caller_subject)
            .expect("effect edit invalidates the caller");
        assert_eq!(
            effect_recomputation.reason,
            VerificationFactReuseMiss::EffectAssumptionsChanged
        );
        assert!(effect_result.reuse.reused.contains(&independent_subject));

        let mut capability_changed = program;
        capability_changed.functions[0]
            .capabilities
            .push("storage.read".to_owned());
        let capability_result =
            capability_changed.validate_with_verified_facts(&request("0.19", first.facts));
        let capability_recomputation = capability_result
            .reuse
            .recomputed
            .iter()
            .find(|row| row.subject == caller_subject)
            .expect("capability edit invalidates the caller");
        assert_eq!(
            capability_recomputation.reason,
            VerificationFactReuseMiss::CapabilityAssumptionsChanged
        );
        assert!(capability_result
            .reuse
            .reused
            .contains(&independent_subject));
    }

    #[test]
    fn facts_record_effect_and_capability_assumptions_as_invalidators() {
        let mut program = crate::body::tests::executable_program();
        program.functions[0].effects.push(Effect {
            kind: "read".to_owned(),
            target: "storage".to_owned(),
            capability: "storage.read".to_owned(),
        });
        program.functions[0]
            .capabilities
            .push("storage.read".to_owned());
        let first = program.validate_with_verified_facts(&request("0.19", Vec::new()));
        assert!(first.report.valid, "{:?}", first.report.errors);
        let fact = &first.facts[0];
        let effect = fact
            .dependencies
            .iter()
            .find(|dependency| dependency.role == VerificationDependencyRole::Effect)
            .expect("effect assumption dependency");
        let capability = fact
            .dependencies
            .iter()
            .find(|dependency| dependency.role == VerificationDependencyRole::Capability)
            .expect("capability assumption dependency");
        assert!(fact.invalidating_changes.contains(
            &VerificationFactInvalidator::EffectAssumptionChanged(effect.identity.clone())
        ));
        assert!(fact.invalidating_changes.contains(
            &VerificationFactInvalidator::CapabilityAssumptionChanged(capability.identity.clone())
        ));

        let mut changed_effect = program.clone();
        changed_effect.functions[0].effects[0].target = "cache".to_owned();
        let effect_result =
            changed_effect.validate_with_verified_facts(&request("0.19", first.facts.clone()));
        assert_eq!(
            effect_result.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::EffectAssumptionsChanged
        );

        let mut changed_capability = program;
        changed_capability.functions[0]
            .capabilities
            .push("storage.write".to_owned());
        let capability_result =
            changed_capability.validate_with_verified_facts(&request("0.19", first.facts));
        assert_eq!(
            capability_result.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::CapabilityAssumptionsChanged
        );
    }

    #[test]
    fn changed_verifier_policy_and_assumptions_isolate_prior_receipts() {
        let program = crate::body::tests::executable_program();
        let cached = program
            .validate_with_verified_facts(&request("0.19", Vec::new()))
            .facts;

        let mut new_verifier = cached.clone();
        new_verifier[0].verifier_identity = "mncs-language-function-body-validator/3".to_owned();
        new_verifier[0].verification_witness_identity = new_verifier[0].computed_witness_identity();
        let verifier_result = program.validate_with_verified_facts(&request("0.19", new_verifier));
        assert_eq!(
            verifier_result.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::VerifierChanged
        );

        let mut new_policy = cached.clone();
        new_policy[0].policy_identity = "mncs-language-function-body-policy/2".to_owned();
        new_policy[0].verification_witness_identity = new_policy[0].computed_witness_identity();
        let policy_result = program.validate_with_verified_facts(&request("0.19", new_policy));
        assert_eq!(
            policy_result.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::PolicyChanged
        );

        let mut changed_assumption = program;
        changed_assumption.functions[0]
            .capabilities
            .push("storage.read".to_owned());
        let assumption_result =
            changed_assumption.validate_with_verified_facts(&request("0.19", cached));
        assert_eq!(
            assumption_result.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::CapabilityAssumptionsChanged
        );
        assert!(assumption_result.reuse.reused.is_empty());
    }

    #[test]
    fn declaration_facts_round_trip_reuse_and_refresh_location_paths() {
        let mut program = crate::validation::tests::valid_program();
        program.functions[0].evidence.clear();
        let first = program.validate_with_verified_facts(&declaration_request("0.19", Vec::new()));
        assert!(first.report.valid, "{:?}", first.report.errors);
        assert_eq!(first.declaration_facts.len(), 1);
        assert_eq!(
            first.declaration_reuse.recomputed[0].reason,
            DeclarationFactReuseMiss::Missing
        );
        assert_eq!(
            first.declaration_facts[0].result,
            VerificationFactResult::Verified
        );
        assert_eq!(first.declaration_facts[0].warnings.len(), 1);
        assert_eq!(
            first.declaration_facts[0].warnings[0].relative_path,
            ".contracts.positive_amount"
        );

        let restored: Vec<DeclarationVerificationFact> =
            serde_json::from_slice(&serde_json::to_vec(&first.declaration_facts).unwrap()).unwrap();
        let subject = function_id(&program.module, "transfer");
        let mut request = declaration_request("0.19", restored);
        request
            .source_location_identities
            .insert(subject.clone(), "source-map:new-snapshot".to_owned());
        let mut reordered = program.clone();
        let mut earlier = reordered.functions[0].clone();
        earlier.name = "earlier".to_owned();
        reordered.functions.insert(0, earlier);
        let second = reordered.validate_with_verified_facts(&request);
        assert!(second.report.valid, "{:?}", second.report.errors);
        assert_eq!(second.declaration_reuse.reused, vec![subject.clone()]);
        assert_eq!(
            second.declaration_facts[1]
                .source_location_identity
                .as_deref(),
            Some("source-map:new-snapshot")
        );
        assert!(second.report.warnings.iter().any(|warning| {
            warning.path == "functions[1].contracts.positive_amount" && warning.code == "MNCS101"
        }));
    }

    #[test]
    fn declaration_facts_invalidate_changed_assumptions_and_refresh_diagnostics() {
        let program = crate::validation::tests::valid_program();
        let initial =
            program.validate_with_verified_facts(&declaration_request("0.19", Vec::new()));
        let cached = initial.declaration_facts;

        let mut changed_assumption = program.clone();
        changed_assumption.assumptions[0].statement =
            "committed writes are durably replicated".to_owned();
        let changed = changed_assumption
            .validate_with_verified_facts(&declaration_request("0.19", cached.clone()));
        assert!(changed.report.valid, "{:?}", changed.report.errors);
        assert_eq!(
            changed.declaration_reuse.recomputed[0].reason,
            DeclarationFactReuseMiss::DependenciesChanged
        );
        assert_eq!(changed.declaration_facts.len(), 1);

        let mut invalid = program;
        invalid.functions[0].evidence[0].verifier = " ".to_owned();
        let failed = invalid.validate_with_verified_facts(&declaration_request("0.19", cached));
        assert!(!failed.report.valid);
        assert!(failed.report.errors.iter().any(|error| {
            error.code == "MNCS013" && error.path == "functions[0].evidence[0].verifier"
        }));
        assert_eq!(
            failed.declaration_reuse.recomputed[0].reason,
            DeclarationFactReuseMiss::SubjectChanged
        );
        assert!(failed.declaration_facts.is_empty());
    }

    #[test]
    fn declaration_fact_witness_and_profile_are_required_for_reuse() {
        let program = crate::validation::tests::valid_program();
        let first = program.validate_with_verified_facts(&declaration_request("0.19", Vec::new()));
        let mut tampered = first.declaration_facts.clone();
        tampered[0].verifier_identity.push_str("/tampered");
        let bad_witness =
            program.validate_with_verified_facts(&declaration_request("0.19", tampered));
        assert_eq!(
            bad_witness.declaration_reuse.recomputed[0].reason,
            DeclarationFactReuseMiss::InvalidWitness
        );

        let changed_profile = program
            .validate_with_verified_facts(&declaration_request("0.20", first.declaration_facts));
        assert_eq!(
            changed_profile.declaration_reuse.recomputed[0].reason,
            DeclarationFactReuseMiss::LanguageProfileChanged
        );
    }

    #[test]
    fn duplicate_function_identity_remains_a_fresh_global_error() {
        let program = crate::validation::tests::valid_program();
        let first = program.validate_with_verified_facts(&declaration_request("0.19", Vec::new()));
        let mut duplicate = program.clone();
        duplicate.functions.push(duplicate.functions[0].clone());
        let result = duplicate
            .validate_with_verified_facts(&declaration_request("0.19", first.declaration_facts));
        assert!(!result.report.valid);
        assert!(result
            .report
            .errors
            .iter()
            .any(|error| error.code == "MNCS005"));
        assert_eq!(result.declaration_reuse.reused.len(), 1);
    }
}
