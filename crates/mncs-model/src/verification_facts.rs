//! Versioned evidence for reusing successful function-body validation.
//!
//! Facts are inputs to the Language validator, never authority on their own:
//! the validator re-derives the current subject, declaration interface,
//! semantic dependencies, language profile, and verifier policy before it
//! skips a body check. Source locations are carried separately and refreshed
//! for every source snapshot.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    function_id, specialization_id, Effect, Function, GenericSpecializationRecord, Program,
    SemanticId, SemanticIdentities,
};

pub const VERIFICATION_FACT_SCHEMA_VERSION: &str = "mncs.language-verification-fact/1";
pub const VERIFICATION_FACT_VERIFIER_ID: &str = "mncs-language-function-body-validator/1";
pub const VERIFICATION_FACT_POLICY_ID: &str = "mncs-language-function-body-policy/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationDependencyRole {
    CallableInterface,
    NominalType,
    Assumption,
    GenericSpecialization,
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
    pub reuse: VerificationFactReuseObservation,
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

impl Program {
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

        let body_value = serde_json::to_value(body).ok()?;
        let body_strings = strings_in_json(&body_value);
        let mut semantic_strings = body_strings.clone();
        semantic_strings.extend(
            function
                .inputs
                .iter()
                .chain(&function.outputs)
                .map(|value| value.value_type.clone()),
        );

        let mut referenced_functions = BTreeSet::new();
        for candidate in &self.functions {
            let identity = function_id(candidate.identity_namespace(&self.module), &candidate.name);
            if identity != declaration && body_strings.contains(identity.as_str()) {
                referenced_functions.insert(identity.clone());
                dependencies.insert(VerificationFactDependency {
                    role: VerificationDependencyRole::CallableInterface,
                    identity,
                    fingerprint: candidate.interface_fingerprint(),
                });
            }
        }

        // `SemanticIdentities` preserves the legacy rule that record objects
        // are enumerated only for source-bound programs. The verifier still
        // resolves record types from declaration-only model programs, so
        // capture those exact existing record identities and their canonical
        // identity fingerprint directly as dependencies too.
        for record_type in &self.record_types {
            if body_strings.contains(record_type.identity.as_str())
                || semantic_strings.contains(&record_type.name)
            {
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
                crate::IdentityKind::FiniteType => {
                    let name = self
                        .finite_types
                        .iter()
                        .find(|item| item.identity == record.identity)
                        .map(|item| item.name.as_str());
                    if body_strings.contains(record.identity.as_str())
                        || name.is_some_and(|name| semantic_strings.contains(name))
                    {
                        dependencies.insert(VerificationFactDependency {
                            role: VerificationDependencyRole::NominalType,
                            identity: record.identity.clone(),
                            fingerprint: record.fingerprint.clone(),
                        });
                    }
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

        for specialization in &self.generic_specializations {
            if specialization_is_relevant(
                specialization,
                &declaration,
                &referenced_functions,
                &body_strings,
            ) {
                let identity = specialization_id(
                    &specialization.generic_function,
                    &specialization.canonical_args,
                );
                if let Some(fingerprint) = identities.fingerprint(&identity) {
                    dependencies.insert(VerificationFactDependency {
                        role: VerificationDependencyRole::GenericSpecialization,
                        identity,
                        fingerprint: fingerprint.to_owned(),
                    });
                }
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
        invalidating_changes.extend(basis.dependencies.iter().map(|dependency| {
            VerificationFactInvalidator::DependencyChanged(dependency.identity.clone())
        }));
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
        if self.declared_effects != current.declared_effects
            || self.declared_capabilities != current.declared_capabilities
        {
            return Err(VerificationFactReuseMiss::DeclarationInterfaceChanged);
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

fn specialization_is_relevant(
    specialization: &GenericSpecializationRecord,
    declaration: &SemanticId,
    referenced_functions: &BTreeSet<SemanticId>,
    body_strings: &BTreeSet<String>,
) -> bool {
    specialization.generic_function == *declaration
        || specialization.specialization_function == *declaration
        || referenced_functions.contains(&specialization.generic_function)
        || body_strings.contains(specialization.instantiation.as_str())
        || body_strings.contains(specialization.specialization_function.as_str())
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
            source_location_identities: BTreeMap::new(),
        }
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
        program.functions = vec![callee, caller];
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
        assert!(result.reuse.reused.is_empty());
    }

    #[test]
    fn changed_verifier_policy_and_assumptions_isolate_prior_receipts() {
        let program = crate::body::tests::executable_program();
        let cached = program
            .validate_with_verified_facts(&request("0.19", Vec::new()))
            .facts;

        let mut new_verifier = cached.clone();
        new_verifier[0].verifier_identity = "mncs-language-function-body-validator/2".to_owned();
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
            VerificationFactReuseMiss::DeclarationInterfaceChanged
        );
        assert!(assumption_result.reuse.reused.is_empty());
    }
}
