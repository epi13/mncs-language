//! Versioned evidence for reusing successful declaration and function-body
//! validation.
//!
//! Facts are inputs to the Language validator, never authority on their own:
//! the validator re-derives the current subject, declaration interface,
//! semantic dependencies, language profile, and verifier policy before it
//! skips a body check. Source locations are carried separately and refreshed
//! for every source snapshot.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{
    assumption_id, function_id, specialization_id, BodyOperationKind, BodyType, Effect, FiniteType,
    Function, FunctionBody, GenericArg, GenericSpecializationRecord, Program, RecordType,
    SemanticId, SemanticIdentities,
};

pub const VERIFICATION_FACT_SCHEMA_VERSION: &str = "mncs.language-verification-fact/4";
pub const VERIFICATION_FACT_REUSE_OBSERVATION_SCHEMA_VERSION: &str =
    "mncs.language-verification-fact-reuse-observation/2";
pub const VERIFICATION_FACT_VERIFIER_ID: &str = "mncs-language-function-body-validator/4";
pub const VERIFICATION_FACT_POLICY_ID: &str = "mncs-language-function-body-policy/2";
pub const DECLARATION_FACT_SCHEMA_VERSION: &str = "mncs.language-declaration-verification-fact/1";
pub const DECLARATION_FACT_REUSE_SCHEMA_VERSION: &str = "mncs.language-declaration-fact-reuse/1";
pub const DECLARATION_FACT_VERIFIER_ID: &str = "mncs-language-function-declaration-validator/1";
pub const DECLARATION_FACT_POLICY_ID: &str = "mncs-language-function-declaration-policy/2";
pub const FINITE_TYPE_FACT_VERIFIER_ID: &str = "mncs-language-finite-type-validator/1";
pub const FINITE_TYPE_FACT_POLICY_ID: &str = "mncs-language-finite-type-policy/1";
pub const RECORD_TYPE_FACT_VERIFIER_ID: &str = "mncs-language-record-type-validator/1";
pub const RECORD_TYPE_FACT_POLICY_ID: &str = "mncs-language-record-type-policy/1";
pub const VERIFICATION_DEPENDENCY_SUMMARY_CACHE_SCHEMA_VERSION: &str =
    "mncs.language-verification-dependency-summary-cache/2";
pub const VERIFICATION_DEPENDENCY_SUMMARY_OBSERVATION_SCHEMA_VERSION: &str =
    "mncs.language-verification-dependency-summary-cache-observation/2";
pub const VERIFICATION_BODY_DEPENDENCY_SUMMARY_SCHEMA_VERSION: &str =
    "mncs.language-verification-body-dependency-summary/1";
/// Bump this when the resolved-body dependency extraction rules change.
pub const VERIFICATION_DEPENDENCY_SUMMARY_POLICY_IDENTITY: &str =
    "mncs-language-verification-dependency-extractor/1";

const MAX_DEPENDENCY_SUMMARY_CACHE_ENTRIES: usize = 4096;
const MAX_DEPENDENCY_SUMMARY_CACHE_REFERENCES: usize = 262_144;
const MAX_DEPENDENCY_SUMMARY_REFERENCES_PER_ENTRY: usize = 8192;
const MAX_DEPENDENCY_SUMMARY_CACHE_BYTES: usize = 16 * 1024 * 1024;
const MAX_DEPENDENCY_SUMMARY_CACHE_BYTES_PER_ENTRY: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationDependencyRole {
    CallableInterface,
    CallableImplementation,
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
    CallableImplementationChanged(SemanticId),
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
    /// Resolved direct references used to derive the dependency closure.
    /// Language revalidates the subject, interface, verifier, policy, and
    /// witness before using this summary to seed a fresh process cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_summary: Option<VerificationBodyDependencySummary>,
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
    /// Prior successful nominal record declaration checks. Record facts are
    /// kept separate from function and finite-type declarations so each
    /// verifier policy and dependency set remains independently addressable.
    #[serde(default)]
    pub prior_record_type_declaration_facts: Vec<DeclarationVerificationFact>,
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
    pub finite_type_declaration_facts: Vec<DeclarationVerificationFact>,
    pub record_type_declaration_facts: Vec<DeclarationVerificationFact>,
    pub reuse: VerificationFactReuseObservation,
    /// Structural dependency extraction reused during this validation pass.
    /// This is analysis work, separate from the verified body facts above.
    pub dependency_summary_cache: Option<VerificationDependencySummaryCacheObservation>,
    pub declaration_reuse: DeclarationFactReuseObservation,
    pub finite_type_declaration_reuse: DeclarationFactReuseObservation,
    pub record_type_declaration_reuse: DeclarationFactReuseObservation,
}

/// Observation of direct dependency extraction avoided by a verified fact or
/// the process-local cache. These hits do not skip body verification or
/// authorize reuse of a `VerificationFact`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationDependencySummaryCacheObservation {
    pub schema_version: String,
    /// Direct summaries restored from verified Language facts after checking
    /// the current body, interface, verifier, policy, and witness identities.
    pub fact_seeded_function_subjects: Vec<SemanticId>,
    /// Direct summaries reused from this process's bounded memory cache.
    pub reused_function_subjects: Vec<SemanticId>,
    pub recomputed_function_subjects: Vec<SemanticId>,
    pub evicted_entry_count: usize,
}

impl Default for VerificationDependencySummaryCacheObservation {
    fn default() -> Self {
        Self {
            schema_version: VERIFICATION_DEPENDENCY_SUMMARY_OBSERVATION_SCHEMA_VERSION.to_owned(),
            fact_seeded_function_subjects: Vec::new(),
            reused_function_subjects: Vec::new(),
            recomputed_function_subjects: Vec::new(),
            evicted_entry_count: 0,
        }
    }
}

/// Caller-owned, bounded in-memory cache for Language's direct semantic
/// dependency extraction. Its private entries cannot be deserialized from an
/// artifact. A fresh cache can be seeded only from a verified body fact whose
/// witness, body, interface, verifier, and extractor policy match. Dependency
/// fingerprints and verification facts are rebuilt against the current
/// program before semantic reuse is considered.
#[derive(Debug)]
pub struct VerificationDependencySummaryCache {
    entries: BTreeMap<SemanticId, DependencySummaryCacheEntry>,
    insertion_order: VecDeque<(u64, SemanticId)>,
    next_generation: u64,
    total_reference_count: usize,
    total_byte_count: usize,
}

/// Direct dependency information produced by Language's resolved-body
/// extractor and carried inside a successful body fact. It can avoid
/// repeating structural extraction in a fresh process; current dependency
/// fingerprints and fact validity are still rebuilt from the current program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationBodyDependencySummary {
    pub schema_version: String,
    pub extractor_policy_identity: String,
    pub calls: Vec<VerificationBodyCallReference>,
    pub assumptions: Vec<String>,
    pub nominal_types: Vec<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VerificationBodyCallReference {
    pub function: SemanticId,
    pub instantiation: Option<SemanticId>,
    pub specialization: Option<SemanticId>,
    pub canonical_arguments: Option<String>,
}

impl Default for VerificationDependencySummaryCache {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            insertion_order: VecDeque::new(),
            next_generation: 1,
            total_reference_count: 0,
            total_byte_count: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct DependencySummaryCacheEntry {
    generation: u64,
    body_fingerprint: String,
    interface_fingerprint: String,
    policy_identity: String,
    reference_count: usize,
    byte_count: usize,
    summary: CachedBodyDependencySummary,
}

#[derive(Debug, Clone)]
struct CachedBodyDependencySummary {
    calls: BTreeSet<VerificationBodyCallReference>,
    assumptions: BTreeSet<String>,
    nominal_types: BTreeSet<SemanticId>,
}

impl CachedBodyDependencySummary {
    fn reference_count(&self) -> usize {
        self.calls.len() + self.assumptions.len() + self.nominal_types.len()
    }

    fn retained_byte_count(&self) -> usize {
        let call_bytes = self
            .calls
            .iter()
            .map(|call| {
                std::mem::size_of::<VerificationBodyCallReference>()
                    + call.function.as_str().len()
                    + call
                        .instantiation
                        .as_ref()
                        .map_or(0, |identity| identity.as_str().len())
                    + call
                        .specialization
                        .as_ref()
                        .map_or(0, |identity| identity.as_str().len())
                    + call
                        .canonical_arguments
                        .as_ref()
                        .map_or(0, |arguments| arguments.len())
            })
            .sum::<usize>();
        let assumption_bytes = self
            .assumptions
            .iter()
            .map(|assumption| std::mem::size_of::<String>() + assumption.len())
            .sum::<usize>();
        let nominal_type_bytes = self
            .nominal_types
            .iter()
            .map(|identity| std::mem::size_of::<SemanticId>() + identity.as_str().len())
            .sum::<usize>();
        call_bytes + assumption_bytes + nominal_type_bytes
    }
}

impl VerificationDependencySummaryCache {
    fn body_summary(
        &mut self,
        nominal_type_index: &VerificationNominalTypeIndex<'_>,
        subject: &SemanticId,
        function: &Function,
        body: &FunctionBody,
        body_fingerprint: Option<&str>,
        interface_fingerprint: &str,
        fact_subject: &SemanticId,
        prior_fact: Option<&VerificationFact>,
        observation: &mut VerificationDependencySummaryCacheObservation,
    ) -> CachedBodyDependencySummary {
        if let Some(body_fingerprint) = body_fingerprint {
            if let Some(entry) = self.entries.get(subject).filter(|entry| {
                entry.body_fingerprint == body_fingerprint
                    && entry.interface_fingerprint == interface_fingerprint
                    && entry.policy_identity == VERIFICATION_DEPENDENCY_SUMMARY_POLICY_IDENTITY
            }) {
                observation.reused_function_subjects.push(subject.clone());
                return entry.summary.clone();
            }

            if let Some(summary) = prior_fact.and_then(|fact| {
                fact.dependency_summary_for_cache(
                    fact_subject,
                    subject,
                    body_fingerprint,
                    interface_fingerprint,
                )
            }) {
                if self.retain(
                    subject,
                    body_fingerprint,
                    interface_fingerprint,
                    &summary,
                    observation,
                ) {
                    observation
                        .fact_seeded_function_subjects
                        .push(subject.clone());
                    return summary;
                }
            }
        }

        observation
            .recomputed_function_subjects
            .push(subject.clone());
        let mut summary = CachedBodyDependencySummary {
            calls: body_call_references(body),
            assumptions: body_assumption_references(body),
            nominal_types: body_nominal_type_references(nominal_type_index, function, body),
        };
        summary
            .assumptions
            .extend(function.assumptions.iter().cloned());

        let Some(body_fingerprint) = body_fingerprint else {
            return summary;
        };
        self.retain(
            subject,
            body_fingerprint,
            interface_fingerprint,
            &summary,
            observation,
        );
        summary
    }

    fn retain(
        &mut self,
        subject: &SemanticId,
        body_fingerprint: &str,
        interface_fingerprint: &str,
        summary: &CachedBodyDependencySummary,
        observation: &mut VerificationDependencySummaryCacheObservation,
    ) -> bool {
        let reference_count = summary.reference_count();
        let byte_count = summary.retained_byte_count();
        if reference_count > MAX_DEPENDENCY_SUMMARY_REFERENCES_PER_ENTRY
            || byte_count > MAX_DEPENDENCY_SUMMARY_CACHE_BYTES_PER_ENTRY
        {
            if self.remove(subject) {
                observation.evicted_entry_count += 1;
            }
            return false;
        }

        if self.remove(subject) {
            observation.evicted_entry_count += 1;
        }
        if self.next_generation == 0 {
            observation.evicted_entry_count += self.entries.len();
            self.entries.clear();
            self.insertion_order.clear();
            self.total_reference_count = 0;
            self.total_byte_count = 0;
            self.next_generation = 1;
        }
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);
        self.total_reference_count += reference_count;
        self.total_byte_count += byte_count;
        self.entries.insert(
            subject.clone(),
            DependencySummaryCacheEntry {
                generation,
                body_fingerprint: body_fingerprint.to_owned(),
                interface_fingerprint: interface_fingerprint.to_owned(),
                policy_identity: VERIFICATION_DEPENDENCY_SUMMARY_POLICY_IDENTITY.to_owned(),
                reference_count,
                byte_count,
                summary: summary.clone(),
            },
        );
        self.insertion_order
            .push_back((generation, subject.clone()));

        while self.entries.len() > MAX_DEPENDENCY_SUMMARY_CACHE_ENTRIES
            || self.total_reference_count > MAX_DEPENDENCY_SUMMARY_CACHE_REFERENCES
            || self.total_byte_count > MAX_DEPENDENCY_SUMMARY_CACHE_BYTES
        {
            let Some((old_generation, old_subject)) = self.insertion_order.pop_front() else {
                break;
            };
            let matches_generation = self
                .entries
                .get(&old_subject)
                .is_some_and(|entry| entry.generation == old_generation);
            if matches_generation && self.remove(&old_subject) {
                observation.evicted_entry_count += 1;
            }
        }
        if self.insertion_order.len() > MAX_DEPENDENCY_SUMMARY_CACHE_ENTRIES * 2 {
            self.insertion_order.retain(|(generation, subject)| {
                self.entries
                    .get(subject)
                    .is_some_and(|entry| entry.generation == *generation)
            });
        }
        true
    }

    fn remove(&mut self, subject: &SemanticId) -> bool {
        if let Some(entry) = self.entries.remove(subject) {
            self.total_reference_count = self
                .total_reference_count
                .saturating_sub(entry.reference_count);
            self.total_byte_count = self.total_byte_count.saturating_sub(entry.byte_count);
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationVerificationWarning {
    pub code: String,
    /// Path suffix relative to the current `functions[index]` slot. The
    /// validator attaches the current slot when it replays this warning.
    pub relative_path: String,
    pub message: String,
}

/// A successful Language-issued function or finite-type declaration result.
/// The verifier identity distinguishes declaration kinds while this shared
/// fact contract keeps their cache and invalidation handling consistent.
/// Function declaration fingerprints include evidence annotations and guard
/// assumption dependencies. Warnings use relative paths so reordered
/// declarations retain fresh diagnostics, and source locations are refreshed
/// per snapshot.
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
    /// Declaration checks actually skipped after Language matched each
    /// current declaration, dependency, verifier, policy, and profile guard.
    pub reused: Vec<SemanticId>,
    /// Declaration checks actually run, with the reason prior evidence was
    /// unavailable or stale.
    pub recomputed: Vec<DeclarationFactRecomputation>,
}

#[derive(Debug, Clone)]
pub(crate) struct BodyFactBasis {
    pub subject: SemanticId,
    pub subject_fingerprint: String,
    pub declaration_interface: VerificationFactDependency,
    pub dependencies: Vec<VerificationFactDependency>,
    pub dependency_summary: VerificationBodyDependencySummary,
    pub declared_effects: Vec<Effect>,
    pub declared_capabilities: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct DeclarationFactBasis {
    pub subject: SemanticId,
    pub subject_fingerprint: String,
    pub dependencies: Vec<VerificationFactDependency>,
    pub verifier_identity: String,
    pub policy_identity: String,
}

#[derive(Clone, Copy)]
enum NominalTypeDefinition<'a> {
    Record(&'a RecordType),
    Finite(&'a FiniteType),
}

impl NominalTypeDefinition<'_> {
    fn identity(&self) -> &SemanticId {
        match self {
            Self::Record(record_type) => &record_type.identity,
            Self::Finite(finite_type) => &finite_type.identity,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Record(record_type) => &record_type.name,
            Self::Finite(finite_type) => &finite_type.name,
        }
    }

    fn body_dependency(&self) -> Option<VerificationFactDependency> {
        let (identity, fingerprint) = match self {
            Self::Record(record_type) => (
                &record_type.identity,
                crate::sha256_hex(serde_json::to_string(record_type).ok()?.as_bytes()),
            ),
            Self::Finite(finite_type) => (
                &finite_type.identity,
                crate::sha256_hex(serde_json::to_string(finite_type).ok()?.as_bytes()),
            ),
        };
        Some(VerificationFactDependency {
            role: VerificationDependencyRole::NominalType,
            identity: identity.clone(),
            fingerprint,
        })
    }
}

/// Per-snapshot nominal declarations indexed for record-fact dependency
/// resolution. The three indexes preserve all candidates so ambiguous names
/// remain unavailable for reuse instead of being resolved by first match.
pub(crate) struct VerificationNominalTypeIndex<'a> {
    by_identity: BTreeMap<SemanticId, Vec<NominalTypeDefinition<'a>>>,
    by_module_and_name: BTreeMap<(Option<String>, String), Vec<NominalTypeDefinition<'a>>>,
    by_name: BTreeMap<String, Vec<NominalTypeDefinition<'a>>>,
}

/// Per-snapshot indexes for semantic dependencies consulted by body facts.
/// Building these once avoids rescanning every declaration for each function.
pub(crate) struct VerificationFactDependencyIndex<'a> {
    pub(crate) nominal_types: VerificationNominalTypeIndex<'a>,
    assumptions_by_name: BTreeMap<String, Vec<VerificationFactDependency>>,
    functions: BTreeMap<SemanticId, VerificationFunctionDependencySummary<'a>>,
}

/// Semantic references extracted once from one function body. A program-wide
/// fact basis contains the full reachable implementation closure, so deriving
/// these references while walking every caller's closure repeatedly rescanned
/// the same bodies. Keeping the direct summary beside the other per-snapshot
/// indexes makes closure expansion proportional to graph traversal rather
/// than AST traversal.
struct VerificationFunctionDependencySummary<'a> {
    function: &'a Function,
    body_fingerprint: Option<String>,
    interface_fingerprint: String,
    declaration_fingerprint: String,
    semantic_fingerprint: Option<String>,
    calls: BTreeSet<VerificationBodyCallReference>,
    assumptions: BTreeSet<String>,
    nominal_types: BTreeSet<SemanticId>,
    specializations: BTreeMap<SemanticId, &'a GenericSpecializationRecord>,
    effect_and_capability_dependencies: Option<BTreeSet<VerificationFactDependency>>,
}

fn declaration_fingerprint(
    interface_fingerprint: &str,
    evidence: &[crate::EvidenceClaim],
) -> String {
    #[derive(Serialize)]
    struct FingerprintMaterial<'a> {
        schema_version: &'static str,
        interface_fingerprint: &'a str,
        evidence: &'a [crate::EvidenceClaim],
    }

    let material = FingerprintMaterial {
        schema_version: "mncs.language-function-declaration-fingerprint/1",
        interface_fingerprint,
        evidence,
    };
    let bytes = serde_json::to_vec(&material).expect("function declaration fingerprint serializes");
    crate::sha256_hex(&bytes)
}

impl<'a> VerificationNominalTypeIndex<'a> {
    fn insert(&mut self, definition: NominalTypeDefinition<'a>) {
        let identity = definition.identity().clone();
        let module = identity.declaring_module();
        let name = definition.name().to_owned();
        self.by_identity
            .entry(identity)
            .or_default()
            .push(definition);
        self.by_module_and_name
            .entry((module, name.clone()))
            .or_default()
            .push(definition);
        self.by_name.entry(name).or_default().push(definition);
    }
}

impl Program {
    pub(crate) fn verification_function_index(&self) -> BTreeMap<SemanticId, &Function> {
        let mut index = BTreeMap::new();
        for function in &self.functions {
            let identity = function_id(function.identity_namespace(&self.module), &function.name);
            index.entry(identity).or_insert(function);
        }
        index
    }

    pub(crate) fn verification_specialization_index(
        &self,
    ) -> BTreeMap<SemanticId, Vec<(usize, &GenericSpecializationRecord)>> {
        let mut index = BTreeMap::<SemanticId, Vec<(usize, &GenericSpecializationRecord)>>::new();
        for (ordinal, specialization) in self.generic_specializations.iter().enumerate() {
            index
                .entry(specialization.generic_function.clone())
                .or_default()
                .push((ordinal, specialization));
            if specialization.specialization_function != specialization.generic_function {
                index
                    .entry(specialization.specialization_function.clone())
                    .or_default()
                    .push((ordinal, specialization));
            }
        }
        index
    }

    pub(crate) fn verification_nominal_type_index(&self) -> VerificationNominalTypeIndex<'_> {
        let mut index = VerificationNominalTypeIndex {
            by_identity: BTreeMap::new(),
            by_module_and_name: BTreeMap::new(),
            by_name: BTreeMap::new(),
        };
        for record_type in &self.record_types {
            index.insert(NominalTypeDefinition::Record(record_type));
        }
        for finite_type in &self.finite_types {
            index.insert(NominalTypeDefinition::Finite(finite_type));
        }
        index
    }

    #[cfg(test)]
    fn verification_fact_dependency_index<'a>(
        &'a self,
        identities: &SemanticIdentities,
        function_index: &BTreeMap<SemanticId, &'a Function>,
        specialization_index: &BTreeMap<SemanticId, Vec<(usize, &'a GenericSpecializationRecord)>>,
    ) -> VerificationFactDependencyIndex<'a> {
        self.verification_fact_dependency_index_with_cache(
            identities,
            function_index,
            specialization_index,
            None,
            None,
        )
        .0
    }

    pub(crate) fn verification_fact_dependency_index_with_cache<'a>(
        &'a self,
        identities: &SemanticIdentities,
        function_index: &BTreeMap<SemanticId, &'a Function>,
        specialization_index: &BTreeMap<SemanticId, Vec<(usize, &'a GenericSpecializationRecord)>>,
        prior_facts_by_subject: Option<&BTreeMap<SemanticId, Vec<&VerificationFact>>>,
        mut dependency_summary_cache: Option<&mut VerificationDependencySummaryCache>,
    ) -> (
        VerificationFactDependencyIndex<'a>,
        Option<VerificationDependencySummaryCacheObservation>,
    ) {
        let cache_enabled = dependency_summary_cache.is_some();
        let mut dependency_summary_observation =
            VerificationDependencySummaryCacheObservation::default();
        let mut assumption_fingerprints = BTreeMap::<SemanticId, Vec<String>>::new();
        for identity in &identities.objects {
            if identity.kind == crate::IdentityKind::Assumption {
                assumption_fingerprints
                    .entry(identity.identity.clone())
                    .or_default()
                    .push(identity.fingerprint.clone());
            }
        }

        let mut assumptions_by_name = BTreeMap::new();
        for assumption in &self.assumptions {
            let identity = assumption_id(&self.module, &assumption.id);
            if let Some(fingerprints) = assumption_fingerprints.get(&identity) {
                let dependencies = assumptions_by_name
                    .entry(assumption.id.clone())
                    .or_insert_with(Vec::new);
                dependencies.extend(fingerprints.iter().map(|fingerprint| {
                    VerificationFactDependency {
                        role: VerificationDependencyRole::Assumption,
                        identity: identity.clone(),
                        fingerprint: fingerprint.clone(),
                    }
                }));
            }
        }

        let nominal_types = self.verification_nominal_type_index();
        let functions = function_index
            .iter()
            .map(|(identity, function)| {
                let interface_fingerprint = function.interface_fingerprint();
                let declaration_fingerprint =
                    declaration_fingerprint(&interface_fingerprint, &function.evidence);
                let body_fingerprint_result = match function.body.as_ref() {
                    Some(body) => body.fingerprint().ok().map(Some),
                    None => Some(None),
                };
                let body_fingerprint = body_fingerprint_result.clone().flatten();
                let semantic_fingerprint =
                    body_fingerprint_result.as_ref().and_then(|fingerprint| {
                        function_semantic_fingerprint(function, &interface_fingerprint, fingerprint)
                    });
                let (calls, assumptions, nominal_types, specializations) = if let Some(body) =
                    function.body.as_ref()
                {
                    let body_subject =
                        body.identity(function.identity_namespace(&self.module), &function.name);
                    let prior_fact = prior_facts_by_subject
                        .and_then(|facts| facts.get(&body_subject))
                        .and_then(|facts| match facts.as_slice() {
                            [fact] => Some(*fact),
                            _ => None,
                        });
                    let summary = match dependency_summary_cache.as_deref_mut() {
                        Some(cache) => cache.body_summary(
                            &nominal_types,
                            identity,
                            function,
                            body,
                            body_fingerprint.as_deref(),
                            &interface_fingerprint,
                            &body_subject,
                            prior_fact,
                            &mut dependency_summary_observation,
                        ),
                        None => {
                            let mut assumptions = body_assumption_references(body);
                            assumptions.extend(function.assumptions.iter().cloned());
                            CachedBodyDependencySummary {
                                calls: body_call_references(body),
                                assumptions,
                                nominal_types: body_nominal_type_references(
                                    &nominal_types,
                                    function,
                                    body,
                                ),
                            }
                        }
                    };
                    let specializations = select_relevant_specializations(
                        identity,
                        &summary.calls,
                        specialization_index,
                    );
                    (
                        summary.calls,
                        summary.assumptions,
                        summary.nominal_types,
                        specializations,
                    )
                } else {
                    (
                        BTreeSet::new(),
                        BTreeSet::new(),
                        BTreeSet::new(),
                        BTreeMap::new(),
                    )
                };
                let mut effect_and_capability_dependencies = BTreeSet::new();
                let effect_and_capability_dependencies = self
                    .add_effect_and_capability_dependencies(
                        function,
                        identities,
                        &mut effect_and_capability_dependencies,
                    )
                    .map(|()| effect_and_capability_dependencies);
                (
                    identity.clone(),
                    VerificationFunctionDependencySummary {
                        function,
                        body_fingerprint,
                        interface_fingerprint,
                        declaration_fingerprint,
                        semantic_fingerprint,
                        calls,
                        assumptions,
                        nominal_types,
                        specializations,
                        effect_and_capability_dependencies,
                    },
                )
            })
            .collect();

        let dependency_summary_cache = cache_enabled.then(|| {
            dependency_summary_observation
                .fact_seeded_function_subjects
                .sort();
            dependency_summary_observation
                .reused_function_subjects
                .sort();
            dependency_summary_observation
                .recomputed_function_subjects
                .sort();
            dependency_summary_observation
        });
        (
            VerificationFactDependencyIndex {
                nominal_types,
                assumptions_by_name,
                functions,
            },
            dependency_summary_cache,
        )
    }

    #[cfg(test)]
    pub(crate) fn declaration_fact_basis_with_identities(
        &self,
        function: &Function,
        identities: &SemanticIdentities,
    ) -> Option<DeclarationFactBasis> {
        let interface_fingerprint = function.interface_fingerprint();
        let subject_fingerprint =
            declaration_fingerprint(&interface_fingerprint, &function.evidence);
        self.declaration_fact_basis_with_fingerprint(function, identities, subject_fingerprint)
    }

    pub(crate) fn declaration_fact_basis_with_dependency_index(
        &self,
        function: &Function,
        identities: &SemanticIdentities,
        dependency_index: &VerificationFactDependencyIndex<'_>,
    ) -> Option<DeclarationFactBasis> {
        let identity = function_id(function.identity_namespace(&self.module), &function.name);
        let summary = dependency_index.functions.get(&identity)?;
        self.declaration_fact_basis_with_fingerprint(
            function,
            identities,
            summary.declaration_fingerprint.clone(),
        )
    }

    fn declaration_fact_basis_with_fingerprint(
        &self,
        function: &Function,
        identities: &SemanticIdentities,
        subject_fingerprint: String,
    ) -> Option<DeclarationFactBasis> {
        let home = function.identity_namespace(&self.module);
        let subject = function_id(home, &function.name);
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
            verifier_identity: DECLARATION_FACT_VERIFIER_ID.to_owned(),
            policy_identity: DECLARATION_FACT_POLICY_ID.to_owned(),
        })
    }

    pub(crate) fn finite_type_declaration_fact_basis(
        &self,
        finite_type: &FiniteType,
    ) -> Option<DeclarationFactBasis> {
        let bytes = serde_json::to_vec(finite_type).ok()?;
        Some(DeclarationFactBasis {
            subject: finite_type.identity.clone(),
            subject_fingerprint: crate::sha256_hex(&bytes),
            dependencies: Vec::new(),
            verifier_identity: FINITE_TYPE_FACT_VERIFIER_ID.to_owned(),
            policy_identity: FINITE_TYPE_FACT_POLICY_ID.to_owned(),
        })
    }

    pub(crate) fn record_type_declaration_fact_basis(
        &self,
        record_type: &RecordType,
        nominal_type_index: &VerificationNominalTypeIndex<'_>,
    ) -> Option<DeclarationFactBasis> {
        let mut dependencies = BTreeSet::new();
        let mut visited_types = BTreeSet::from([record_type.identity.clone()]);
        for field in &record_type.fields {
            self.add_record_field_type_dependencies(
                record_type,
                &field.field_type,
                nominal_type_index,
                &mut dependencies,
                &mut visited_types,
            )?;
        }
        Some(DeclarationFactBasis {
            subject: record_type.identity.clone(),
            subject_fingerprint: record_type_semantic_fingerprint(record_type)?,
            dependencies: dependencies.into_iter().collect(),
            verifier_identity: RECORD_TYPE_FACT_VERIFIER_ID.to_owned(),
            policy_identity: RECORD_TYPE_FACT_POLICY_ID.to_owned(),
        })
    }

    fn add_record_field_type_dependencies(
        &self,
        owner: &RecordType,
        field_type: &str,
        nominal_type_index: &VerificationNominalTypeIndex<'_>,
        dependencies: &mut BTreeSet<VerificationFactDependency>,
        visited_types: &mut BTreeSet<SemanticId>,
    ) -> Option<()> {
        let field_type = field_type.trim();
        if let Some(inner) = field_type
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
        {
            if let Some((element, _bound)) = inner.rsplit_once(';') {
                return self.add_record_field_type_dependencies(
                    owner,
                    element.trim(),
                    nominal_type_index,
                    dependencies,
                    visited_types,
                );
            }
        }

        if let Some(candidates) = nominal_type_index
            .by_identity
            .get(&SemanticId(field_type.to_owned()))
        {
            if candidates.len() > 1 {
                return None;
            }
            if let Some(candidate) = candidates.first().copied() {
                self.add_nominal_type_dependency(
                    candidate,
                    nominal_type_index,
                    dependencies,
                    visited_types,
                )?;
                return Some(());
            }
        }

        let local_key = (owner.identity.declaring_module(), field_type.to_owned());
        let candidates = nominal_type_index
            .by_module_and_name
            .get(&local_key)
            .filter(|candidates| !candidates.is_empty())
            .or_else(|| nominal_type_index.by_name.get(field_type));
        if let Some(candidates) = candidates {
            if candidates.len() > 1 {
                return None;
            }
            if let Some(candidate) = candidates.first().copied() {
                self.add_nominal_type_dependency(
                    candidate,
                    nominal_type_index,
                    dependencies,
                    visited_types,
                )?;
            }
        }
        Some(())
    }

    fn add_nominal_type_dependency(
        &self,
        candidate: NominalTypeDefinition<'_>,
        nominal_type_index: &VerificationNominalTypeIndex<'_>,
        dependencies: &mut BTreeSet<VerificationFactDependency>,
        visited_types: &mut BTreeSet<SemanticId>,
    ) -> Option<()> {
        match candidate {
            NominalTypeDefinition::Record(record_type) => self.add_record_type_dependency_closure(
                record_type,
                nominal_type_index,
                dependencies,
                visited_types,
            ),
            NominalTypeDefinition::Finite(finite_type) => {
                dependencies.insert(VerificationFactDependency {
                    role: VerificationDependencyRole::NominalType,
                    identity: finite_type.identity.clone(),
                    fingerprint: crate::sha256_hex(&serde_json::to_vec(finite_type).ok()?),
                });
                Some(())
            }
        }
    }

    fn add_record_type_dependency_closure(
        &self,
        record_type: &RecordType,
        nominal_type_index: &VerificationNominalTypeIndex<'_>,
        dependencies: &mut BTreeSet<VerificationFactDependency>,
        visited_types: &mut BTreeSet<SemanticId>,
    ) -> Option<()> {
        if !visited_types.insert(record_type.identity.clone()) {
            return Some(());
        }
        dependencies.insert(VerificationFactDependency {
            role: VerificationDependencyRole::NominalType,
            identity: record_type.identity.clone(),
            fingerprint: record_type_semantic_fingerprint(record_type)?,
        });
        for field in &record_type.fields {
            self.add_record_field_type_dependencies(
                record_type,
                &field.field_type,
                nominal_type_index,
                dependencies,
                visited_types,
            )?;
        }
        Some(())
    }

    #[cfg(test)]
    pub(crate) fn body_fact_basis_with_identities(
        &self,
        function: &Function,
        identities: &SemanticIdentities,
    ) -> Option<BodyFactBasis> {
        let function_index = self.verification_function_index();
        let specialization_index = self.verification_specialization_index();
        let dependency_index = self.verification_fact_dependency_index(
            identities,
            &function_index,
            &specialization_index,
        );
        self.body_fact_basis_with_dependency_index(function, identities, &dependency_index)
    }

    pub(crate) fn body_fact_basis_with_dependency_index(
        &self,
        function: &Function,
        identities: &SemanticIdentities,
        dependency_index: &VerificationFactDependencyIndex<'_>,
    ) -> Option<BodyFactBasis> {
        let body = function.body.as_ref()?;
        let home = function.identity_namespace(&self.module);
        let subject = body.identity(home, &function.name);
        let declaration = function_id(home, &function.name);
        let function_summary = dependency_index.functions.get(&declaration)?;
        let subject_fingerprint = function_summary.body_fingerprint.clone()?;
        let declaration_interface = VerificationFactDependency {
            role: VerificationDependencyRole::CallableInterface,
            identity: declaration.clone(),
            fingerprint: function_summary.interface_fingerprint.clone(),
        };
        let mut dependencies = BTreeSet::new();
        dependencies.extend(
            function_summary
                .effect_and_capability_dependencies
                .as_ref()?
                .iter()
                .cloned(),
        );

        let mut used_assumptions = function_summary.assumptions.clone();
        let mut relevant_specializations = function_summary.specializations.clone();
        let mut nominal_types = function_summary.nominal_types.clone();

        // A caller depends on the successful verification of the entire
        // reachable implementation closure. If a downstream dependency
        // changes, an intermediate function's body fingerprint can stay the
        // same while its verification fact becomes stale. Carry the
        // dependency closure into this fact so that change also invalidates
        // callers beyond the first edge. The visited set keeps recursive call
        // graphs finite and preserves one dependency per semantic identity.
        let mut pending_functions = function_summary
            .calls
            .iter()
            .map(|call| call.function.clone())
            .collect::<Vec<_>>();
        pending_functions.extend(
            relevant_specializations
                .values()
                .map(|specialization| specialization.specialization_function.clone()),
        );
        let mut visited_functions = BTreeSet::new();
        let mut reachable_function_identities = Vec::new();
        let mut next_pending = 0;
        while next_pending < pending_functions.len() {
            let identity = pending_functions[next_pending].clone();
            next_pending += 1;
            if identity == declaration || !visited_functions.insert(identity.clone()) {
                continue;
            }
            let Some(candidate_summary) = dependency_index.functions.get(&identity) else {
                continue;
            };
            reachable_function_identities.push(identity);
            let candidate = candidate_summary.function;
            let Some(_candidate_body) = candidate.body.as_ref() else {
                continue;
            };
            used_assumptions.extend(candidate_summary.assumptions.iter().cloned());
            nominal_types.extend(candidate_summary.nominal_types.iter().cloned());
            pending_functions.extend(
                candidate_summary
                    .calls
                    .iter()
                    .map(|call| call.function.clone()),
            );
            for (specialization_identity, specialization) in &candidate_summary.specializations {
                if relevant_specializations
                    .insert(specialization_identity.clone(), *specialization)
                    .is_none()
                {
                    pending_functions.push(specialization.specialization_function.clone());
                }
            }
        }

        for specialization in relevant_specializations.values() {
            for argument in &specialization.args {
                if let GenericArg::Type { ty } = argument {
                    collect_nominal_type_references(
                        &dependency_index.nominal_types,
                        ty,
                        &mut nominal_types,
                    );
                }
            }
        }
        for identity in reachable_function_identities {
            let candidate_summary = dependency_index.functions.get(&identity)?;
            let candidate = candidate_summary.function;
            let identity = function_id(candidate.identity_namespace(&self.module), &candidate.name);
            dependencies.insert(VerificationFactDependency {
                role: VerificationDependencyRole::CallableInterface,
                identity: identity.clone(),
                fingerprint: candidate_summary.interface_fingerprint.clone(),
            });
            let implementation_fingerprint = candidate_summary.semantic_fingerprint.clone()?;
            dependencies.insert(VerificationFactDependency {
                role: VerificationDependencyRole::CallableImplementation,
                identity,
                fingerprint: implementation_fingerprint,
            });
            dependencies.extend(
                candidate_summary
                    .effect_and_capability_dependencies
                    .as_ref()?
                    .iter()
                    .cloned(),
            );
        }

        for identity in nominal_types {
            if let Some(candidates) = dependency_index.nominal_types.by_identity.get(&identity) {
                for candidate in candidates {
                    dependencies.insert(candidate.body_dependency()?);
                }
            }
        }

        for assumption in used_assumptions {
            if let Some(candidates) = dependency_index.assumptions_by_name.get(&assumption) {
                dependencies.extend(candidates.iter().cloned());
            }
        }

        for (identity, specialization) in relevant_specializations {
            if let Some(fingerprint) = self.generic_specialization_fingerprint(
                specialization,
                &identity,
                identities,
                &dependency_index.functions,
            ) {
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
            dependency_summary: VerificationBodyDependencySummary {
                schema_version: VERIFICATION_BODY_DEPENDENCY_SUMMARY_SCHEMA_VERSION.to_owned(),
                extractor_policy_identity: VERIFICATION_DEPENDENCY_SUMMARY_POLICY_IDENTITY
                    .to_owned(),
                calls: function_summary.calls.iter().cloned().collect(),
                assumptions: function_summary.assumptions.iter().cloned().collect(),
                nominal_types: function_summary.nominal_types.iter().cloned().collect(),
            },
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
        function_summaries: &BTreeMap<SemanticId, VerificationFunctionDependencySummary<'_>>,
    ) -> Option<String> {
        let record_fingerprint = identities.fingerprint(identity)?;
        let generic_function_fingerprint = function_summaries
            .get(&specialization.generic_function)?
            .semantic_fingerprint
            .as_ref()?;
        let specialized_function_fingerprint = function_summaries
            .get(&specialization.specialization_function)?
            .semantic_fingerprint
            .as_ref()?;
        let bytes = serde_json::to_vec(&(
            record_fingerprint,
            generic_function_fingerprint,
            specialized_function_fingerprint,
        ))
        .ok()?;
        Some(crate::sha256_hex(&bytes))
    }
}

fn function_semantic_fingerprint(
    function: &Function,
    interface_fingerprint: &str,
    body_fingerprint: &Option<String>,
) -> Option<String> {
    #[derive(Serialize)]
    struct FunctionSemanticFingerprint<'a> {
        interface_fingerprint: &'a str,
        body_fingerprint: &'a Option<String>,
        effects: &'a [Effect],
        capabilities: &'a [String],
    }
    let material = FunctionSemanticFingerprint {
        interface_fingerprint,
        body_fingerprint,
        effects: &function.effects,
        capabilities: &function.capabilities,
    };
    let bytes = serde_json::to_vec(&material).ok()?;
    Some(crate::sha256_hex(&bytes))
}

fn record_type_semantic_fingerprint(record_type: &RecordType) -> Option<String> {
    let mut fields = record_type
        .fields
        .iter()
        .map(|field| (&field.name, &field.field_type))
        .collect::<Vec<_>>();
    fields.sort();
    let bytes = serde_json::to_vec(&(&record_type.identity, &record_type.name, fields)).ok()?;
    Some(crate::sha256_hex(&bytes))
}

fn body_call_references(body: &FunctionBody) -> BTreeSet<VerificationBodyCallReference> {
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
                calls.insert(VerificationBodyCallReference {
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
        calls.extend(iteration.callees.iter().cloned().map(|function| {
            VerificationBodyCallReference {
                function,
                instantiation: None,
                specialization: None,
                canonical_arguments: None,
            }
        }));
    }
    calls
}

fn body_nominal_type_references(
    nominal_type_index: &VerificationNominalTypeIndex<'_>,
    function: &Function,
    body: &FunctionBody,
) -> BTreeSet<SemanticId> {
    let mut identities = BTreeSet::new();
    for value in function.inputs.iter().chain(&function.outputs) {
        let ty = BodyType::from_semantic_name(&value.value_type);
        collect_nominal_type_references(nominal_type_index, &ty, &mut identities);
    }
    for parameter in &body.parameters {
        collect_nominal_type_references(nominal_type_index, &parameter.ty, &mut identities);
    }
    for iteration in &body.bounded_iterations {
        collect_nominal_type_references(nominal_type_index, &iteration.state_type, &mut identities);
        if let crate::IterationDomain::OverSequence { element_type } = &iteration.domain {
            collect_nominal_type_references(nominal_type_index, element_type, &mut identities);
        }
    }
    for block in &body.blocks {
        for value in &block.parameters {
            collect_nominal_type_references(nominal_type_index, &value.ty, &mut identities);
        }
        for operation in &block.operations {
            for result in &operation.results {
                collect_nominal_type_references(nominal_type_index, &result.ty, &mut identities);
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
                } => collect_nominal_type_references(
                    nominal_type_index,
                    operand_type,
                    &mut identities,
                ),
                BodyOperationKind::Convert { from, to } => {
                    collect_nominal_type_references(nominal_type_index, from, &mut identities);
                    collect_nominal_type_references(nominal_type_index, to, &mut identities);
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
                            collect_nominal_type_references(
                                nominal_type_index,
                                ty,
                                &mut identities,
                            );
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
    nominal_type_index: &VerificationNominalTypeIndex<'_>,
    ty: &BodyType,
    identities: &mut BTreeSet<SemanticId>,
) {
    match ty {
        BodyType::Finite { identity, .. } | BodyType::Record { identity, .. } => {
            identities.insert(identity.clone());
        }
        BodyType::Named(name) => {
            if let Some(candidates) = nominal_type_index
                .by_identity
                .get(&SemanticId(name.clone()))
            {
                identities.extend(
                    candidates
                        .iter()
                        .map(|candidate| candidate.identity().clone()),
                );
            }
            if let Some(candidates) = nominal_type_index.by_name.get(name) {
                identities.extend(
                    candidates
                        .iter()
                        .map(|candidate| candidate.identity().clone()),
                );
            }
        }
        BodyType::Sequence { element, .. } | BodyType::Vector { element, .. } => {
            collect_nominal_type_references(nominal_type_index, element, identities);
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
                VerificationDependencyRole::CallableImplementation => {
                    VerificationFactInvalidator::CallableImplementationChanged(
                        dependency.identity.clone(),
                    )
                }
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
            dependency_summary: Some(basis.dependency_summary),
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

    fn dependency_summary_for_cache(
        &self,
        subject: &SemanticId,
        declaration_identity: &SemanticId,
        body_fingerprint: &str,
        interface_fingerprint: &str,
    ) -> Option<CachedBodyDependencySummary> {
        if self.schema_version != VERIFICATION_FACT_SCHEMA_VERSION
            || self.verifier_identity != VERIFICATION_FACT_VERIFIER_ID
            || self.policy_identity != VERIFICATION_FACT_POLICY_ID
            || self.result != VerificationFactResult::Verified
            || self.verification_witness_identity != self.computed_witness_identity()
            || &self.subject != subject
            || self.subject_fingerprint != body_fingerprint
            || self.declaration_interface.role != VerificationDependencyRole::CallableInterface
            || &self.declaration_interface.identity != declaration_identity
            || self.declaration_interface.fingerprint != interface_fingerprint
        {
            return None;
        }
        let summary = self.dependency_summary.as_ref()?;
        if summary.schema_version != VERIFICATION_BODY_DEPENDENCY_SUMMARY_SCHEMA_VERSION
            || summary.extractor_policy_identity != VERIFICATION_DEPENDENCY_SUMMARY_POLICY_IDENTITY
            || summary.calls.len() + summary.assumptions.len() + summary.nominal_types.len()
                > MAX_DEPENDENCY_SUMMARY_REFERENCES_PER_ENTRY
            || !summary.calls.windows(2).all(|pair| pair[0] < pair[1])
            || !summary.assumptions.windows(2).all(|pair| pair[0] < pair[1])
            || !summary
                .nominal_types
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        {
            return None;
        }
        let summary = CachedBodyDependencySummary {
            calls: summary.calls.iter().cloned().collect(),
            assumptions: summary.assumptions.iter().cloned().collect(),
            nominal_types: summary.nominal_types.iter().cloned().collect(),
        };
        let byte_count = summary.retained_byte_count();
        let accepted = byte_count <= MAX_DEPENDENCY_SUMMARY_CACHE_BYTES_PER_ENTRY;
        accepted.then_some(summary)
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
            dependency_summary: &'a Option<VerificationBodyDependencySummary>,
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
            dependency_summary: &self.dependency_summary,
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
            verifier_identity: basis.verifier_identity,
            policy_identity: basis.policy_identity,
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
        if self.verifier_identity != current.verifier_identity {
            return Err(DeclarationFactReuseMiss::VerifierChanged);
        }
        if self.policy_identity != current.policy_identity {
            return Err(DeclarationFactReuseMiss::PolicyChanged);
        }
        if self.language_profile != current.language_profile {
            return Err(DeclarationFactReuseMiss::LanguageProfileChanged);
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

#[cfg(test)]
fn specialization_is_relevant(
    specialization: &GenericSpecializationRecord,
    declaration: &SemanticId,
    calls: &BTreeSet<VerificationBodyCallReference>,
) -> bool {
    specialization.specialization_function == *declaration
        || calls
            .iter()
            .any(|call| specialization_call_matches(specialization, call))
}

fn specialization_call_matches(
    specialization: &GenericSpecializationRecord,
    call: &VerificationBodyCallReference,
) -> bool {
    let addresses_specialization = call.instantiation.as_ref()
        == Some(&specialization.instantiation)
        || call.specialization.as_ref() == Some(&specialization.specialization_function)
        || call.canonical_arguments.as_deref() == Some(specialization.canonical_args.as_str());
    addresses_specialization
        && (call.function == specialization.generic_function
            || call.function == specialization.specialization_function)
}

fn select_relevant_specializations<'a>(
    declaration: &SemanticId,
    calls: &BTreeSet<VerificationBodyCallReference>,
    index: &BTreeMap<SemanticId, Vec<(usize, &'a GenericSpecializationRecord)>>,
) -> BTreeMap<SemanticId, &'a GenericSpecializationRecord> {
    let mut selected_by_ordinal = BTreeMap::<usize, &'a GenericSpecializationRecord>::new();
    if let Some(candidates) = index.get(declaration) {
        for (ordinal, specialization) in candidates {
            if specialization.specialization_function == *declaration {
                selected_by_ordinal.insert(*ordinal, *specialization);
            }
        }
    }
    for call in calls {
        if let Some(candidates) = index.get(&call.function) {
            for (ordinal, specialization) in candidates {
                if specialization_call_matches(specialization, call) {
                    selected_by_ordinal.insert(*ordinal, *specialization);
                }
            }
        }
    }
    selected_by_ordinal
        .into_values()
        .map(|specialization| {
            (
                specialization_id(
                    &specialization.generic_function,
                    &specialization.canonical_args,
                ),
                specialization,
            )
        })
        .collect()
}

fn body_assumption_references(body: &FunctionBody) -> BTreeSet<String> {
    body.blocks
        .iter()
        .flat_map(|block| block.operations.iter())
        .flat_map(|operation| operation.assumptions.iter().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        finite_type_id, finite_variant_id, function_id, record_type_id, BodyBlock, BodyOperation,
        BodyOperationKind, BodyTerminator, BodyType, BodyValue, FiniteVariant, RecordField,
        RecordType, Value,
    };

    fn request(profile: &str, prior_facts: Vec<VerificationFact>) -> VerificationFactRequest {
        VerificationFactRequest {
            language_profile: profile.to_owned(),
            subject_language_profiles: BTreeMap::new(),
            prior_facts,
            prior_declaration_facts: Vec::new(),
            prior_record_type_declaration_facts: Vec::new(),
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

    fn record_type_declaration_request(
        profile: &str,
        prior_facts: Vec<DeclarationVerificationFact>,
    ) -> VerificationFactRequest {
        let mut request = declaration_request(profile, Vec::new());
        request.prior_record_type_declaration_facts = prior_facts;
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
    fn invalid_declaration_does_not_issue_or_reuse_a_body_fact() {
        let mut program = crate::body::tests::executable_program();
        program.functions[0].effects.push(Effect {
            kind: "read".to_owned(),
            target: "storage".to_owned(),
            capability: "storage.read".to_owned(),
        });

        let first = program.validate_with_verified_facts(&request("0.19", Vec::new()));
        assert!(!first.report.valid, "{:?}", first.report.errors);
        assert!(first.facts.is_empty(), "{:?}", first.facts);

        // Even a well-formed fact with a matching current basis cannot skip
        // the body when its declaration failed in this validation request.
        let identities = program.semantic_identities();
        let basis = program
            .body_fact_basis_with_identities(&program.functions[0], &identities)
            .expect("the body has an identity even though its declaration is invalid");
        let unverified_candidate = VerificationFact::from_basis(basis, "0.19", None);
        let second =
            program.validate_with_verified_facts(&request("0.19", vec![unverified_candidate]));
        assert!(!second.report.valid, "{:?}", second.report.errors);
        assert!(second.reuse.reused.is_empty());
        assert_eq!(second.reuse.recomputed.len(), 1);
        assert_eq!(
            second.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::DeclarationInterfaceChanged
        );
        assert!(second.facts.is_empty());
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
    fn body_fact_dependency_index_skips_unrelated_declarations_across_many_functions() {
        let mut program = call_program();
        let prototype = program
            .functions
            .iter()
            .find(|function| function.name == "independent")
            .unwrap()
            .clone();
        for index in 0..96 {
            let mut function = prototype.clone();
            function.name = format!("independent_{index}");
            program.functions.push(function);
        }
        for index in 0..2048 {
            let name = format!("Unused_{index}");
            program.record_types.push(RecordType {
                identity: record_type_id("Unrelated.Module", &name, &[("field", "i64")]),
                name,
                fields: vec![RecordField {
                    name: "field".to_owned(),
                    field_type: "i64".to_owned(),
                }],
            });
        }

        let first = program.validate_with_verified_facts(&request("0.19", Vec::new()));
        assert!(first.report.valid, "{:?}", first.report.errors);
        assert_eq!(first.facts.len(), 98);
        assert!(first.facts.iter().all(|fact| fact
            .dependencies
            .iter()
            .all(|dependency| dependency.role != VerificationDependencyRole::NominalType)));

        let second = program.validate_with_verified_facts(&request("0.19", first.facts));
        assert!(second.report.valid, "{:?}", second.report.errors);
        assert_eq!(second.reuse.reused.len(), 98);
        assert!(second.reuse.recomputed.is_empty());
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
    fn unrelated_body_text_does_not_create_an_assumption_dependency() {
        let assumption_name = "one";
        let mut program = crate::body::tests::executable_program();
        program.assumptions.push(crate::Assumption {
            id: assumption_name.to_owned(),
            statement: "The unrelated body text has no assumption meaning.".to_owned(),
            supplied_by: "test fixture".to_owned(),
            confidence: crate::AssumptionConfidence::External,
        });

        let identities = program.semantic_identities();
        let basis = program
            .body_fact_basis_with_identities(&program.functions[0], &identities)
            .unwrap();
        let identity = assumption_id(&program.module, assumption_name);
        assert!(!basis.dependencies.iter().any(|dependency| {
            dependency.role == VerificationDependencyRole::Assumption
                && dependency.identity == identity
        }));

        let function = &mut program.functions[0];
        function.assumptions.push(assumption_name.to_owned());
        function.body.as_mut().unwrap().blocks[0].operations[0]
            .assumptions
            .push(assumption_name.to_owned());
        let identities = program.semantic_identities();
        let basis = program
            .body_fact_basis_with_identities(&program.functions[0], &identities)
            .unwrap();
        assert!(basis.dependencies.iter().any(|dependency| {
            dependency.role == VerificationDependencyRole::Assumption
                && dependency.identity == identity
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
        let other_specialization = GenericSpecializationRecord {
            generic_function: generic_function.clone(),
            specialization_function: function_id(&program.module, "generic_callee__spec_8"),
            instantiation: SemanticId("mncs:0.2:instantiation:generic_callee:value:8".to_owned()),
            args: vec![GenericArg::Value { value: 8 }],
            canonical_args: "value:8".to_owned(),
            host_spellings: Vec::new(),
        };
        let calls = body_call_references(body);
        assert!(specialization_is_relevant(
            &specialization,
            &function_id(&program.module, "caller"),
            &calls,
        ));
        assert!(!specialization_is_relevant(
            &other_specialization,
            &function_id(&program.module, "caller"),
            &calls,
        ));

        program.generic_specializations =
            vec![specialization.clone(), other_specialization.clone()];
        for index in 0..2048 {
            let unrelated_generic =
                SemanticId(format!("mncs:0.2:function:unrelated::generic_{index}"));
            program
                .generic_specializations
                .push(GenericSpecializationRecord {
                    generic_function: unrelated_generic.clone(),
                    specialization_function: SemanticId(format!(
                        "mncs:0.2:function:unrelated::generic_{index}__spec"
                    )),
                    instantiation: SemanticId(format!(
                        "mncs:0.2:instantiation:unrelated::generic_{index}"
                    )),
                    args: vec![GenericArg::Value { value: index }],
                    canonical_args: format!("value:{index}"),
                    host_spellings: Vec::new(),
                });
        }
        let specialization_index = program.verification_specialization_index();
        assert_eq!(program.generic_specializations.len(), 2050);
        assert_eq!(
            specialization_index
                .get(&generic_function)
                .expect("indexed generic call target")
                .len(),
            2,
            "selector lookup examines the called generic's bucket, not unrelated records"
        );
        let indexed = select_relevant_specializations(
            &function_id(&program.module, "caller"),
            &calls,
            &specialization_index,
        );
        assert_eq!(
            indexed.keys().cloned().collect::<BTreeSet<_>>(),
            BTreeSet::from([specialization_id(
                &specialization.generic_function,
                &specialization.canonical_args,
            )]),
            "indexed lookup must keep exact resolved specialization identity"
        );
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
        let generic_name = generic_function.name.clone();
        let specialized_identity = function_id(&program.module, &specialized_function.name);
        let downstream_identity = function_id(&program.module, "downstream");
        let downstream_name = "downstream".to_owned();
        let instantiation = SemanticId("mncs:0.2:instantiation:generic_callee:money".to_owned());
        let argument = GenericArg::Type {
            ty: BodyType::Record {
                identity: nominal_identity.clone(),
                name: "Money".to_owned(),
            },
        };
        let canonical_args = argument.canonical_string();
        let mut specialized_body = program.functions[1].body.clone().unwrap();
        let BodyOperationKind::Call {
            function,
            function_name,
            ..
        } = &mut specialized_body.blocks[0].operations[0].kind
        else {
            unreachable!("call_program caller contains a call")
        };
        *function = downstream_identity.clone();
        *function_name = downstream_name;
        specialized_function.body = Some(specialized_body);
        let mut downstream = program.functions[2].clone();
        downstream.name = "downstream".to_owned();
        program.generic_specializations = vec![GenericSpecializationRecord {
            generic_function: generic_identity.clone(),
            specialization_function: specialized_identity.clone(),
            instantiation: instantiation.clone(),
            args: vec![argument.clone()],
            canonical_args,
            host_spellings: Vec::new(),
        }];
        program
            .functions
            .extend([generic_function, specialized_function.clone(), downstream]);

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
        *function = generic_identity;
        *function_name = generic_name;
        *generic_args = vec![argument];
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
        assert!(basis.dependencies.iter().any(|dependency| {
            dependency.role == VerificationDependencyRole::CallableImplementation
                && dependency.identity == downstream_identity
        }));
    }

    #[test]
    fn changed_generic_specialization_rechecks_callers_and_reuses_independent_bodies() {
        let mut program = call_program();
        let generic_identity = function_id(&program.module, "callee");
        let specialized_identity = function_id(&program.module, "callee__spec_4");
        let instantiation = SemanticId("mncs:0.2:instantiation:callee:value:4".to_owned());
        let argument = GenericArg::Value { value: 4 };
        let canonical_args = argument.canonical_string();

        program.functions[0].generic_params = vec![crate::GenericParam {
            name: "N".to_owned(),
            kind: crate::GenericParamKind::Nat,
        }];
        let mut specialization = program.functions[2].clone();
        specialization.name = "callee__spec_4".to_owned();
        program.functions.push(specialization);
        program.generic_specializations = vec![GenericSpecializationRecord {
            generic_function: generic_identity.clone(),
            specialization_function: specialized_identity.clone(),
            instantiation: instantiation.clone(),
            args: vec![argument.clone()],
            canonical_args,
            host_spellings: Vec::new(),
        }];

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
        *function = generic_identity;
        *function_name = "callee".to_owned();
        *generic_args = vec![argument];
        *call_instantiation = Some(instantiation);
        *call_specialization = Some(specialized_identity.clone());

        let first = program.validate_with_verified_facts(&request("0.19", Vec::new()));
        assert!(first.report.valid, "{:?}", first.report.errors);
        let caller_subject = program.functions[1]
            .body
            .as_ref()
            .unwrap()
            .identity(&program.module, "caller");
        let specialized_subject = program.functions[3]
            .body
            .as_ref()
            .unwrap()
            .identity(&program.module, "callee__spec_4");
        let independent_subject = program.functions[2]
            .body
            .as_ref()
            .unwrap()
            .identity(&program.module, "independent");
        let caller_fact = first
            .facts
            .iter()
            .find(|fact| fact.subject == caller_subject)
            .expect("caller body fact");
        assert!(caller_fact.dependencies.iter().any(|dependency| {
            dependency.role == VerificationDependencyRole::GenericSpecialization
                && dependency.identity
                    == specialization_id(&function_id(&program.module, "callee"), "value:4")
        }));

        let mut changed = program;
        changed.functions[3].body.as_mut().unwrap().entry = "missing".to_owned();
        let result = changed.validate_with_verified_facts(&request("0.19", first.facts));
        assert!(!result.report.valid);
        assert!(result
            .report
            .errors
            .iter()
            .any(|error| error.code == "MNB006"));
        let recomputed = result
            .reuse
            .recomputed
            .iter()
            .map(|row| (row.subject.clone(), row.reason.clone()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            recomputed.get(&caller_subject),
            Some(&VerificationFactReuseMiss::DependenciesChanged)
        );
        assert_eq!(
            recomputed.get(&specialized_subject),
            Some(&VerificationFactReuseMiss::SubjectChanged)
        );
        assert_eq!(result.reuse.reused, vec![independent_subject]);
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
    fn changed_callee_implementation_rechecks_direct_callers_only() {
        let mut program = call_program();
        let independent_body = program.functions[2].body.clone().unwrap();
        program.functions[0].body = Some(independent_body);
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
        assert!(first.facts.iter().any(|fact| {
            fact.subject == caller_subject
                && fact.dependencies.iter().any(|dependency| {
                    dependency.role == VerificationDependencyRole::CallableImplementation
                        && dependency.identity == function_id(&program.module, "callee")
                })
        }));

        let mut changed = program;
        changed.functions[0].body.as_mut().unwrap().entry = "missing".to_owned();
        let result = changed.validate_with_verified_facts(&request("0.19", first.facts));
        assert!(!result.report.valid);
        assert!(result
            .report
            .errors
            .iter()
            .any(|error| error.code == "MNB006"));
        let caller_recomputation = result
            .reuse
            .recomputed
            .iter()
            .find(|row| row.subject == caller_subject)
            .expect("callee body edit invalidates the direct caller");
        assert_eq!(
            caller_recomputation.reason,
            VerificationFactReuseMiss::DependenciesChanged
        );
        assert_eq!(result.reuse.reused, vec![independent_subject]);
    }

    #[test]
    fn transitive_callee_dependency_rechecks_reachable_callers() {
        let mut program = call_program();
        let mut callee_body = program.functions[1].body.clone().unwrap();
        let BodyOperationKind::Call {
            function,
            function_name,
            ..
        } = &mut callee_body.blocks[0].operations[0].kind
        else {
            unreachable!("call_program caller contains a call")
        };
        *function = function_id(&program.module, "downstream");
        *function_name = "downstream".to_owned();
        program.functions[0].body = Some(callee_body);
        let mut downstream = program.functions[2].clone();
        downstream.name = "downstream".to_owned();
        program.functions.push(downstream);

        let first = program.validate_with_verified_facts(&request("0.19", Vec::new()));
        assert!(first.report.valid, "{:?}", first.report.errors);
        let caller_subject = program.functions[1]
            .body
            .as_ref()
            .unwrap()
            .identity(&program.module, "caller");
        let callee_subject = program.functions[0]
            .body
            .as_ref()
            .unwrap()
            .identity(&program.module, "callee");
        let downstream_subject = program.functions[3]
            .body
            .as_ref()
            .unwrap()
            .identity(&program.module, "downstream");
        let independent_subject = program.functions[2]
            .body
            .as_ref()
            .unwrap()
            .identity(&program.module, "independent");

        let mut changed = program;
        changed.functions[3].body.as_mut().unwrap().entry = "missing".to_owned();
        let result = changed.validate_with_verified_facts(&request("0.19", first.facts));
        assert!(!result.report.valid);
        assert!(result
            .report
            .errors
            .iter()
            .any(|error| error.code == "MNB006"));
        let recomputed = result
            .reuse
            .recomputed
            .iter()
            .map(|row| row.subject.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            recomputed,
            BTreeSet::from([caller_subject, callee_subject, downstream_subject,])
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

        let mut old_verifier = cached.clone();
        old_verifier[0].verifier_identity = "mncs-language-function-body-validator/3".to_owned();
        old_verifier[0].verification_witness_identity = old_verifier[0].computed_witness_identity();
        let old_verifier_result =
            program.validate_with_verified_facts(&request("0.19", old_verifier));
        assert_eq!(
            old_verifier_result.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::VerifierChanged
        );

        let mut new_verifier = cached.clone();
        new_verifier[0].verifier_identity = "mncs-language-function-body-validator/5".to_owned();
        new_verifier[0].verification_witness_identity = new_verifier[0].computed_witness_identity();
        let verifier_result = program.validate_with_verified_facts(&request("0.19", new_verifier));
        assert_eq!(
            verifier_result.reuse.recomputed[0].reason,
            VerificationFactReuseMiss::VerifierChanged
        );

        let mut new_policy = cached.clone();
        new_policy[0].policy_identity = "mncs-language-function-body-policy/3".to_owned();
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

        let mut prior_policy = first.declaration_facts.clone();
        prior_policy[0].policy_identity = "mncs-language-function-declaration-policy/1".to_owned();
        prior_policy[0].verification_witness_identity = prior_policy[0].computed_witness_identity();
        let policy_miss =
            program.validate_with_verified_facts(&declaration_request("0.19", prior_policy));
        assert_eq!(
            policy_miss.declaration_reuse.recomputed[0].reason,
            DeclarationFactReuseMiss::PolicyChanged
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
    fn declaration_fingerprints_share_interface_work_and_preserve_evidence() {
        let program = crate::validation::tests::valid_program();
        let function = &program.functions[0];
        let identities = program.semantic_identities();
        let function_index = program.verification_function_index();
        let specialization_index = program.verification_specialization_index();
        let dependency_index = program.verification_fact_dependency_index(
            &identities,
            &function_index,
            &specialization_index,
        );

        let direct = program
            .declaration_fact_basis_with_identities(function, &identities)
            .expect("function declaration fact basis");
        let indexed = program
            .declaration_fact_basis_with_dependency_index(function, &identities, &dependency_index)
            .expect("indexed function declaration fact basis");
        assert_eq!(indexed.subject_fingerprint, direct.subject_fingerprint);
        assert_eq!(
            direct.policy_identity,
            "mncs-language-function-declaration-policy/2"
        );

        let mut without_evidence = function.clone();
        without_evidence.evidence.clear();
        let without_evidence = program
            .declaration_fact_basis_with_identities(&without_evidence, &identities)
            .expect("declaration fact basis without evidence");
        assert_ne!(
            direct.subject_fingerprint,
            without_evidence.subject_fingerprint
        );
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
    fn finite_type_declaration_facts_reuse_refresh_locations_and_revalidate_changes() {
        let mut program = crate::validation::tests::valid_program();
        let module = program.module.clone();
        program.finite_types.push(FiniteType {
            identity: finite_type_id(&module, "TransferState"),
            name: "TransferState".to_owned(),
            variants: vec![
                FiniteVariant {
                    identity: finite_variant_id(&module, "TransferState", "Pending"),
                    name: "Pending".to_owned(),
                    discriminant: 0,
                    payload: Vec::new(),
                },
                FiniteVariant {
                    identity: finite_variant_id(&module, "TransferState", "Committed"),
                    name: "Committed".to_owned(),
                    discriminant: 1,
                    payload: Vec::new(),
                },
            ],
        });

        let first = program.validate_with_verified_facts(&declaration_request("0.19", Vec::new()));
        assert!(first.report.valid, "{:?}", first.report.errors);
        assert_eq!(first.finite_type_declaration_facts.len(), 1);
        assert_eq!(
            first.finite_type_declaration_reuse.recomputed[0].reason,
            DeclarationFactReuseMiss::Missing
        );
        assert_eq!(
            first.finite_type_declaration_facts[0].verifier_identity,
            FINITE_TYPE_FACT_VERIFIER_ID
        );

        let subject = program.finite_types[0].identity.clone();
        let mut reused_request =
            declaration_request("0.19", first.finite_type_declaration_facts.clone());
        reused_request
            .source_location_identities
            .insert(subject.clone(), "source-map:next-snapshot".to_owned());
        let reused = program.validate_with_verified_facts(&reused_request);
        assert!(reused.report.valid, "{:?}", reused.report.errors);
        assert_eq!(
            reused.finite_type_declaration_reuse.reused,
            vec![subject.clone()]
        );
        assert_eq!(
            reused.finite_type_declaration_facts[0]
                .source_location_identity
                .as_deref(),
            Some("source-map:next-snapshot")
        );

        let mut changed = program.clone();
        changed.finite_types[0].variants[0].name = "Queued".to_owned();
        changed.finite_types[0].variants[0].identity =
            finite_variant_id(&module, "TransferState", "Queued");
        let changed_result = changed.validate_with_verified_facts(&declaration_request(
            "0.19",
            first.finite_type_declaration_facts.clone(),
        ));
        assert!(
            changed_result.report.valid,
            "{:?}",
            changed_result.report.errors
        );
        assert_eq!(
            changed_result.finite_type_declaration_reuse.recomputed[0].reason,
            DeclarationFactReuseMiss::SubjectChanged
        );

        let changed_profile = program.validate_with_verified_facts(&declaration_request(
            "0.20",
            first.finite_type_declaration_facts.clone(),
        ));
        assert_eq!(
            changed_profile.finite_type_declaration_reuse.recomputed[0].reason,
            DeclarationFactReuseMiss::LanguageProfileChanged
        );

        let mut invalid = program;
        invalid.finite_types[0].variants[1].name = invalid.finite_types[0].variants[0].name.clone();
        invalid.finite_types[0].variants[1].identity =
            invalid.finite_types[0].variants[0].identity.clone();
        let invalid_result = invalid.validate_with_verified_facts(&declaration_request(
            "0.19",
            first.finite_type_declaration_facts,
        ));
        assert!(!invalid_result.report.valid);
        assert!(invalid_result.report.errors.iter().any(|error| {
            error.code == "MNCS017" && error.path == "finite_types[0].variants[1].name"
        }));
        assert!(invalid_result.finite_type_declaration_facts.is_empty());
    }

    #[test]
    fn record_type_declaration_facts_reuse_refresh_locations_and_invalidate_dependencies() {
        let mut program = crate::validation::tests::valid_program();
        let module = program.module.clone();
        let currency_identity = record_type_id(&module, "Currency", &[("code", "i64")]);
        let money_identity = record_type_id(&module, "Money", &[("currency", "Currency")]);
        let wallet_identity = record_type_id(&module, "Wallet", &[("money", "Money")]);
        program.record_types = vec![
            RecordType {
                identity: currency_identity,
                name: "Currency".to_owned(),
                fields: vec![RecordField {
                    name: "code".to_owned(),
                    field_type: "i64".to_owned(),
                }],
            },
            RecordType {
                identity: money_identity.clone(),
                name: "Money".to_owned(),
                fields: vec![RecordField {
                    name: "currency".to_owned(),
                    field_type: "Currency".to_owned(),
                }],
            },
            RecordType {
                identity: wallet_identity.clone(),
                name: "Wallet".to_owned(),
                fields: vec![RecordField {
                    name: "money".to_owned(),
                    field_type: "Money".to_owned(),
                }],
            },
        ];

        let first = program
            .validate_with_verified_facts(&record_type_declaration_request("0.19", Vec::new()));
        assert!(first.report.valid, "{:?}", first.report.errors);
        assert_eq!(first.record_type_declaration_facts.len(), 3);
        assert_eq!(first.record_type_declaration_reuse.recomputed.len(), 3);
        assert_eq!(
            first.record_type_declaration_facts[0].verifier_identity,
            RECORD_TYPE_FACT_VERIFIER_ID
        );
        let restored: Vec<DeclarationVerificationFact> = serde_json::from_slice(
            &serde_json::to_vec(&first.record_type_declaration_facts).unwrap(),
        )
        .unwrap();

        let mut reused_request = record_type_declaration_request("0.19", restored.clone());
        reused_request.source_location_identities.insert(
            money_identity.clone(),
            "source-map:next-snapshot".to_owned(),
        );
        let reused = program.validate_with_verified_facts(&reused_request);
        assert!(reused.report.valid, "{:?}", reused.report.errors);
        assert_eq!(reused.record_type_declaration_reuse.reused.len(), 3);
        let money_fact = reused
            .record_type_declaration_facts
            .iter()
            .find(|fact| fact.subject == money_identity)
            .unwrap();
        assert_eq!(
            money_fact.source_location_identity.as_deref(),
            Some("source-map:next-snapshot")
        );

        let mut changed_dependency = program.clone();
        changed_dependency.record_types[0].fields[0].field_type = "i32".to_owned();
        changed_dependency.record_types[0].identity =
            record_type_id(&module, "Currency", &[("code", "i32")]);
        let changed = changed_dependency.validate_with_verified_facts(
            &record_type_declaration_request("0.19", restored.clone()),
        );
        assert!(changed.report.valid, "{:?}", changed.report.errors);
        let money_recomputation = changed
            .record_type_declaration_reuse
            .recomputed
            .iter()
            .find(|entry| entry.subject == money_identity)
            .unwrap();
        assert_eq!(
            money_recomputation.reason,
            DeclarationFactReuseMiss::DependenciesChanged
        );
        let wallet_recomputation = changed
            .record_type_declaration_reuse
            .recomputed
            .iter()
            .find(|entry| entry.subject == wallet_identity)
            .unwrap();
        assert_eq!(
            wallet_recomputation.reason,
            DeclarationFactReuseMiss::DependenciesChanged
        );

        let changed_profile = program.validate_with_verified_facts(
            &record_type_declaration_request("0.20", restored.clone()),
        );
        assert!(changed_profile
            .record_type_declaration_reuse
            .recomputed
            .iter()
            .all(|entry| entry.reason == DeclarationFactReuseMiss::LanguageProfileChanged));

        let mut invalid = program;
        invalid.record_types[1].fields.push(RecordField {
            name: "currency".to_owned(),
            field_type: "i64".to_owned(),
        });
        let invalid_result = invalid
            .validate_with_verified_facts(&record_type_declaration_request("0.19", restored));
        assert!(!invalid_result.report.valid);
        assert!(invalid_result
            .report
            .errors
            .iter()
            .any(|error| error.code == "MNCS021"));
        assert!(invalid_result
            .record_type_declaration_facts
            .iter()
            .all(|fact| fact.subject != money_identity));
    }

    #[test]
    fn record_type_dependency_index_preserves_resolution_with_many_unrelated_types() {
        let mut program = crate::validation::tests::valid_program();
        let module = program.module.clone();
        let local_currency = record_type_id(&module, "Currency", &[("code", "i64")]);
        let foreign_currency = record_type_id("Finance", "Currency", &[("code", "u64")]);
        let wallet_identity = record_type_id(&module, "Wallet", &[("currency", "Currency")]);
        program.record_types.extend([
            RecordType {
                identity: local_currency.clone(),
                name: "Currency".to_owned(),
                fields: vec![RecordField {
                    name: "code".to_owned(),
                    field_type: "i64".to_owned(),
                }],
            },
            RecordType {
                identity: foreign_currency.clone(),
                name: "Currency".to_owned(),
                fields: vec![RecordField {
                    name: "code".to_owned(),
                    field_type: "u64".to_owned(),
                }],
            },
            RecordType {
                identity: wallet_identity.clone(),
                name: "Wallet".to_owned(),
                fields: vec![RecordField {
                    name: "currency".to_owned(),
                    field_type: "Currency".to_owned(),
                }],
            },
        ]);
        for ordinal in 0..2048 {
            let name = format!("Unrelated{ordinal}");
            program.record_types.push(RecordType {
                identity: record_type_id("Unrelated", &name, &[("value", "i64")]),
                name,
                fields: vec![RecordField {
                    name: "value".to_owned(),
                    field_type: "i64".to_owned(),
                }],
            });
        }

        let index = program.verification_nominal_type_index();
        let local_key = (Some(module.clone()), "Currency".to_owned());
        assert_eq!(
            index.by_module_and_name.get(&local_key).map(Vec::len),
            Some(1),
            "lookup considers the matching module/name bucket instead of scanning unrelated declarations"
        );
        assert_eq!(
            index.by_name.get("Currency").map(Vec::len),
            Some(2),
            "the global fallback retains all candidates so ambiguity remains visible"
        );
        let mut named_candidates = BTreeSet::new();
        collect_nominal_type_references(
            &index,
            &BodyType::Named("Currency".to_owned()),
            &mut named_candidates,
        );
        assert_eq!(
            named_candidates,
            BTreeSet::from([local_currency.clone(), foreign_currency.clone()]),
            "body dependency extraction uses the same indexed candidate set"
        );

        let wallet = program
            .record_types
            .iter()
            .find(|record_type| record_type.identity == wallet_identity)
            .unwrap();
        let basis = program
            .record_type_declaration_fact_basis(wallet, &index)
            .unwrap();
        assert!(basis.dependencies.iter().any(|dependency| {
            dependency.role == VerificationDependencyRole::NominalType
                && dependency.identity == local_currency
        }));
        assert!(!basis.dependencies.iter().any(|dependency| {
            dependency.role == VerificationDependencyRole::NominalType
                && dependency.identity == foreign_currency
        }));

        let mut ambiguous = program;
        ambiguous.finite_types.push(FiniteType {
            identity: finite_type_id(&module, "Currency"),
            name: "Currency".to_owned(),
            variants: Vec::new(),
        });
        let ambiguous_index = ambiguous.verification_nominal_type_index();
        let wallet = ambiguous
            .record_types
            .iter()
            .find(|record_type| record_type.identity == wallet_identity)
            .unwrap();
        assert!(
            ambiguous
                .record_type_declaration_fact_basis(wallet, &ambiguous_index)
                .is_none(),
            "a local record/finite collision must remain unresolved for fact reuse"
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
