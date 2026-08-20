# RFC 0036: Machine-Native Language, Specification, Feature, Compatibility, Migration, Deprecation, and Evolution Semantics

- **Status:** Draft
- **Authors:** MNCS project
- **Requires:** RFC 0004, RFC 0005, RFC 0007, RFC 0012, RFC 0013, RFC 0014, RFC 0016, RFC 0018, RFC 0020, RFC 0027, RFC 0030, RFC 0031, RFC 0034, RFC 0035
- **Related:** RFC 0021, RFC 0022, RFC 0024, RFC 0025, RFC 0032, RFC 0033

## 1. Summary

MNCS needs a language-evolution model that can change syntax, elaboration, features, core semantics, defaults, proof obligations, tooling, and support policy without silently rewriting the meaning of historical source.

This RFC establishes the semantic vocabulary and ownership boundaries for the evolution of MNCS itself.

Its central rule is:

> **A language version does not redefine old source retroactively. Every source artifact is interpreted under an explicit semantic profile, and every evolution step must state which meanings it preserves, extends, restricts, migrates, or intentionally breaks.**

A second rule follows immediately:

> **Compiler version, language profile, specification revision, edition, feature profile, core generation, support status, and migration identity are distinct.**

A third rule is equally important:

> **There is no universal `backward_compatible` Boolean. Parsing, binding, elaboration, typing, core meaning, behavior, linking, proof validity, evidence applicability, and migration compatibility are independently scoped claims.**

The fourth rule defines the trust model:

> **Migration and evolution tooling may propose transformations aggressively, but compatibility, preservation, and transport claims must terminate in explicit artifacts and witnesses that can be checked independently of the tool that generated them.**

RFC 0036 therefore treats language evolution as a proof- and evidence-bearing transformation graph rather than a release-note convention.

## 2. Motivation

A mature MNCS implementation will eventually need to change itself.

Potential changes include:

- new source constructs;
- new keywords;
- grammar changes;
- new elaboration rules;
- changed defaults;
- new interface-resolution rules;
- new proof obligations;
- new core constructs;
- corrected core semantics;
- stronger safety restrictions;
- deprecated or retired constructs;
- new feature combinations;
- experimental semantic branches;
- new proof-kernel profiles;
- new reference evaluators;
- changed tool protocols;
- changed conformance requirements.

In a conventional language, much of this is represented indirectly through a compiler version and prose documentation.

That is insufficient for MNCS.

The same source text can change meaning when any of the following change:

- grammar;
- binding rules;
- implicit prelude;
- interface-instance set;
- overload specificity;
- coercion rules;
- numeric defaults;
- elaboration policy;
- proof elaboration;
- core reduction rules;
- enabled features.

A compiler that merely accepts old source therefore has not proven compatibility.

The language must preserve the semantic environment under which source was interpreted and must model transitions between language environments as explicitly as it models transitions between deployed programs.

## 3. Scope

RFC 0036 owns:

- semantic language-profile identity;
- specification lineage;
- editions;
- core generations;
- language-feature identity and feature profiles;
- feature dependencies and conflicts;
- feature maturity/status metadata;
- language deltas;
- compatibility relations between language profiles;
- conservative-extension claims;
- migration plans and migration witnesses;
- migration path coherence;
- deprecation and removal states;
- mixed-profile source graphs;
- proof/evidence transport across language evolution;
- compiler-conformance claims scoped to a profile;
- evolution impact and claim invalidation;
- historical semantic preservation.

RFC 0036 does **not** own:

- user data/schema evolution, owned by RFC 0027;
- deployed-software lifecycle/version rollout, owned by RFC 0030;
- build derivation/provenance, owned by RFC 0031;
- preference among several valid migrations, owned by RFC 0033;
- empirical experiment semantics, owned by RFC 0034;
- elaboration rules within one profile, owned by RFC 0035;
- proof checking itself, owned by RFC 0007;
- general equivalence/refinement definitions, owned by RFC 0020.

## 4. Constitutional distinctions

The following distinctions are normative design constraints.

1. Compiler version is not language version.
2. Specification revision is not necessarily semantic revision.
3. Edition is not necessarily core generation.
4. Feature status is not feature semantics.
5. Feature support is not feature activation.
6. Feature activation is not feature use.
7. A version label is not compatibility evidence.
8. Parsing compatibility is not elaboration compatibility.
9. Elaboration compatibility is not behavioral compatibility.
10. Source compatibility is not binary/link compatibility.
11. Proof compatibility is not empirical-evidence applicability.
12. Adding syntax is not automatically a conservative extension.
13. Adding bindings, instances, coercions, defaults, or preludes may change old meaning.
14. Old source meaning is always profile-scoped.
15. A newer compiler does not gain authority to reinterpret an older profile.
16. Published semantic profiles are immutable historical semantic objects.
17. Editorial corrections do not automatically create new language semantics.
18. Implementation bug fixes do not automatically create new language semantics.
19. Genuine specification semantic corrections require new semantic identity.
20. A safety-motivated breaking change remains a breaking change.
21. Deprecation does not itself change semantics.
22. Soft deprecation and scheduled removal are different states.
23. Removal from a new profile does not erase old-profile semantics.
24. Tool-support retirement is not semantic erasure.
25. A migration is not valid merely because destination source compiles.
26. Migration must state which relation it preserves or intentionally changes.
27. Migration may be partial, lossy, irreversible, or incoherent.
28. Multiple migration paths require explicit path-coherence reasoning where relevant.
29. Migration generation remains outside the TCB when results can be independently checked.
30. Feature combinations require interaction analysis; individual conservativity need not compose.
31. A feature profile can be invalid before any source is parsed.
32. Experimental features still require exact semantic identities.
33. Feature requirements should name feature capabilities rather than only compiler version ranges.
34. Mixed language profiles are legitimate when their boundaries are compatible.
35. Importing a newer-profile component does not upgrade the importer’s profile.
36. Intentional realization freedom survives migration unless migration explicitly resolves it.
37. Proof transport and empirical-evidence transport are different claims.
38. Corpus compatibility is bounded empirical evidence, not universal compatibility proof.
39. Stabilization is an assurance decision, not elapsed time.
40. `latest` is not a reproducible semantic profile.

## 5. Language profile

The principal semantic object is `LanguageProfile`.

```text
LanguageProfile {
    id

    specification_snapshot
    edition
    core_generation

    concrete_syntax_profile
    abstract_syntax_profile
    elaboration_profile

    feature_profile
    default_profile
    prelude_profile

    proof_kernel_profile
    reference_semantics_profile

    predecessor_relations
}
```

A source artifact is interpreted under a specific language profile.

```text
SourceArtifact {
    source_identity
    language_profile_id
}
```

The meaning of a source artifact is therefore not determined by source bytes alone.

Conceptually:

```text
Meaning = Elaborate(Source, LanguageProfile)
```

where elaboration itself is governed by RFC 0035.

### 5.1 Exact profile identity

A profile identity should eventually be canonically representable and digestible through RFC 0027 and RFC 0031.

A profile identifier must cover every meaning-affecting input required to reproduce source interpretation.

A development alias such as `latest` may resolve to a profile, but reproducible artifacts must store the resolved exact identity.

### 5.2 Immutability

Once published, a semantic profile is immutable.

If an editorial document changes while semantic rules do not, the specification revision may change while the semantic profile remains the same.

If semantic meaning changes, a new semantic profile must be created.

## 6. Compiler version versus language profile

A compiler implementation can support many profiles.

```text
Compiler C7
    supports E2
    supports E3
    supports E4
```

The compiler version determines implementation identity.

It does not determine source meaning when the source already names a profile.

Two compiler versions can therefore satisfy:

```text
Conforms(C1, P)
Conforms(C2, P)
```

while remaining distinct compiler artifacts.

A newer compiler may improve:

- performance;
- diagnostics;
- proof search;
- optimization;
- incremental compilation;
- internal representation;

without changing the semantics of profile `P`.

## 7. Specification identity

MNCS separates:

```text
SpecificationRevision
SemanticLanguageProfile
```

A specification snapshot may have:

```text
SpecificationSnapshot {
    semantic_profile
    editorial_revision
    canonical_documents
    executable_reference
}
```

This permits corrections such as spelling fixes, examples, or clarifying prose without fabricating a semantic version change.

Conversely, a genuine semantic correction creates a new profile even if the surface version label changes only slightly.

## 8. Editions

An edition is primarily a source-facing interpretation profile.

It may alter:

- reserved keywords;
- grammar;
- surface constructs;
- defaulting policy;
- prelude membership;
- elaboration restrictions;
- diagnostic policy;
- feature baseline.

An edition need not imply a new core generation.

```text
Edition E1 ----\
Edition E2 -----+--> CoreGeneration C4
Edition E3 ----/
```

This architecture allows source-facing evolution while preserving a common semantic core.

### 8.1 Mixed editions

Different source artifacts may use different editions in one dependency graph.

```text
A@E2
B@E4
```

Each artifact is parsed and elaborated under its own profile.

Importing `B@E4` does not change the profile of `A@E2`.

Cross-component interoperability remains subject to RFC 0014.

## 9. Core generations

A core generation identifies a versioned semantic core vocabulary and rule set.

```text
CoreGeneration {
    syntax
    typing_rules
    reduction_or_execution_rules
    proof_obligations
}
```

Surface-edition migration and core-generation migration are different events.

A source migration can preserve the same core.

A core migration may require translation and proof of preservation.

### 9.1 Core translation

```text
CoreTranslation {
    from_generation
    to_generation
    transformation
    preserved_relations
    obligations
    witness
}
```

The translation algorithm can be untrusted if the resulting core artifact and preservation claims can be checked independently.

## 10. Language features

Language functionality is represented through explicit features rather than only a monolithic version number.

```text
LanguageFeature {
    id
    semantic_version
    semantic_definition
    status
}
```

A feature version determines semantics.

A feature status determines governance/maturity metadata.

They are different.

### 10.1 Feature status

Suggested initial states:

```text
PROPOSED
EXPERIMENTAL
PROVISIONAL
STABLE
DEPRECATED
RETIRED
```

A transition from `EXPERIMENTAL` to `STABLE` does not itself change feature meaning.

If meaning changes, the feature semantic version changes.

### 10.2 Feature support, activation, and use

These are distinct:

```text
ToolSupports(F)
ProfileEnables(F)
ArtifactUses(F)
```

A compiler may support a feature that a profile forbids.

A profile may enable a feature unused by a particular artifact.

## 11. Feature profiles

```text
FeatureProfile {
    enabled_features
    disabled_features
    feature_versions
    requirements
    conflicts
}
```

Feature profiles are constraint systems.

Possible relations include:

```text
Requires(F1, F2)
Excludes(F3, F4)
RequiresCore(F5, C7)
RequiresEdition(F6, E4)
```

Before source compilation, a feature profile can be checked as:

```text
SAT
UNSAT
UNKNOWN
```

`UNKNOWN` must not be converted to `SAT` or `UNSAT` by convention.

### 11.1 Feature closure

`FeatureClosure` captures transitive feature requirements.

The closure is distinct from the explicitly requested feature list.

### 11.2 Feature provenance

A feature may be enabled because of:

- edition baseline;
- explicit project opt-in;
- dependency requirement;
- migration requirement;
- experiment profile.

This origin should be retained.

## 12. Feature interaction

Individually conservative features need not compose conservatively.

```text
Conservative(F1)
Conservative(F2)
```

does not imply:

```text
Conservative(F1 + F2)
```

A `FeatureInteractionClaim` names:

```text
FeatureInteractionClaim {
    feature_set
    baseline_profile
    resulting_profile
    protected_relation
    evidence
}
```

Feature interaction can arise through:

- grammar overlap;
- name resolution;
- interface instances;
- coercions;
- defaulting;
- effects;
- proof rules;
- core behavior.

RFC 0034 can empirically explore combinations; formal claims remain scoped to their actual method and relation.

## 13. Language evolution graph

Language evolution is a directed graph, not necessarily one version line.

```text
LanguageEvolutionGraph {
    profiles
    evolution_edges
}
```

It may contain:

- stable branches;
- experimental profiles;
- long-term support branches;
- research dialects;
- converging editions.

Each edge is a `LanguageDelta`.

## 14. Language delta

```text
LanguageDelta {
    from_profile
    to_profile

    grammar_delta
    abstract_syntax_delta
    binding_delta
    elaboration_delta
    feature_delta
    core_delta
    proof_delta
    support_delta

    declared_compatibility
    migration_refs
}
```

A delta is descriptive.

Compatibility remains an independently evidenced relation.

### 14.1 Change classes

Initial classifications:

```text
EDITORIAL
IMPLEMENTATION_ONLY
CONSERVATIVE_ADDITION
SURFACE_BREAK_CORE_PRESERVING
SEMANTIC_REFINEMENT
SEMANTIC_RESTRICTION
SEMANTIC_EXTENSION
INCOMPATIBLE_SEMANTIC_CHANGE
DEFECT_CORRECTION
DEPRECATION
REMOVAL
```

These classifications do not replace relation evidence.

## 15. Grammar evolution

`GrammarDelta` records:

```text
GrammarDelta {
    added_tokens
    removed_tokens
    keyword_changes
    added_productions
    removed_productions
    precedence_changes
    lexical_changes
}
```

Compatibility dimensions include:

```text
OldSourceStillLexes
OldSourceStillParses
OldParseStructurePreserved
OldAbstractSyntaxPreserved
```

These are different claims.

A grammar addition can make an old parse ambiguous or can change macro matching even if no old syntax is removed.

## 16. Abstract syntax evolution

Concrete syntax and abstract syntax evolve separately.

A keyword rename may preserve the same semantic node.

An identical textual spelling may map to a changed abstract construct under a new profile.

Therefore:

```text
ConcreteSyntaxCompatibility
```

and:

```text
AbstractSyntaxCompatibility
```

remain distinct.

## 17. Elaboration evolution

RFC 0035 defines elaboration within one profile.

RFC 0036 defines relationships between elaboration profiles.

Changes to any of the following are language evolution:

- defaulting rules;
- preludes;
- interface-instance visibility;
- overload specificity;
- coercion insertion;
- type inference;
- effect inference;
- universe inference;
- ambiguity policy;
- coherence profile.

### 17.1 Elaboration compatibility

```text
ElaborationCompatibilityClaim {
    old_profile
    new_profile
    source_domain
    core_relation
    witness
}
```

Old source can remain accepted while failing elaboration compatibility if its elaborated core changes materially.

## 18. Conservative extension

A new profile is conservative relative to an old profile only under an explicit relation and source domain.

Conceptually:

```text
forall p in Programs(P_old):
    Meaning(p, P_old) R Meaning(p, P_new)
```

where `R` is an RFC 0020 relation.

```text
ConservativeExtensionClaim {
    old_profile
    new_profile
    source_domain
    relation
    assumptions
    evidence
}
```

### 18.1 Conservative syntax addition is insufficient

Adding a feature without removing syntax can still be non-conservative if it changes:

- parse choice;
- binding;
- overload resolution;
- interface resolution;
- coercion path;
- defaulted type;
- prelude lookup;
- effect inference;
- proof obligations.

### 18.2 Corpus compatibility is not proof

Testing all currently known source artifacts can produce substantial bounded empirical support.

It cannot by itself prove a universal conservative-extension claim.

RFC 0034 remains authoritative for empirical scope.

## 19. Compatibility relation family

RFC 0036 refuses a universal compatibility Boolean.

Initial compatibility families include:

```text
LexicalCompatibility
ParsingCompatibility
AbstractSyntaxCompatibility
BindingCompatibility
ElaborationCompatibility
TypingCompatibility
CoreCompatibility
BehaviorCompatibility<R>
EffectCompatibility
ProofCompatibility
EvidenceApplicabilityCompatibility
SourceCompatibility
BinaryCompatibility
LinkCompatibility
ToolingCompatibility
MigrationCompatibility
```

Each claim must identify:

- source/destination profiles;
- direction;
- domain;
- observation/relation;
- assumptions;
- evidence.

### 19.1 Directionality

Compatibility is not assumed symmetric.

Examples:

```text
OldSourceAcceptedByNewProfile
NewSourceAcceptedByOldProfile
OldCoreReadableByNewTool
NewCoreReadableByOldTool
```

are distinct.

### 19.2 Compatibility envelope

A `CompatibilityEnvelope` may aggregate several independently proven relation claims for a declared use case.

It must not erase which component relation each conclusion came from.

## 20. Version labels

Human-facing version labels can be useful.

They are metadata and policy outputs.

A SemVer-like label may classify changes according to a project policy, but version numbers never establish compatibility by themselves.

Conceptually:

```text
CompatibilityEvidence
    -> VersionClassificationPolicy
    -> ReleaseLabel
```

not the reverse.

## 21. Migration

Language migration is an explicit transformation between source/profile worlds.

```text
LanguageMigration {
    id

    source_profile
    destination_profile

    source_artifact
    destination_artifact

    intended_relation
    transformation
    obligations
    witness
}
```

Migration tools are proposal mechanisms.

Acceptance depends on the resulting artifact and witness.

### 21.1 Migration result states

Suggested states:

```text
PRESERVED
PRESERVED_WITH_LOSS
INTENTIONALLY_CHANGED
PARTIAL
AMBIGUOUS
FAILED
UNKNOWN
```

### 21.2 Migration intent

A migration should state whether it intends to:

```text
PRESERVE_OLD_MEANING
ADOPT_NEW_SEMANTICS
PRESERVE_EXCEPT<DeclaredDelta>
```

Without this, migration correctness is underspecified.

## 22. Source migration versus semantic migration

A source migration changes representation while preserving explicit meaning.

A semantic migration deliberately changes meaning under a declared relation/refinement.

These are different.

```text
SourceMigration
SemanticMigration
```

must never collapse into one `upgrade` operation.

## 23. Compatibility migration versus idiomatic migration

A migration should generally separate:

```text
CompatibilityMigration
```

from optional:

```text
IdiomaticRefinement
```

The first aims to preserve old meaning under the destination profile.

The second intentionally adopts newer constructs or patterns after compatibility has been secured.

This makes migration review and proof obligations substantially clearer.

## 24. Semantic migration patches

MNCS should support structured migration rules resembling semantic patches.

```text
SemanticMigrationPatch {
    match_semantics
    preconditions
    transformation
    postconditions
    preserved_relation
}
```

Textual rewriting may be a realization for trivial changes, but the semantic object is the transformation relation.

## 25. Migration graph and paths

A `MigrationGraph` describes available profile transitions.

```text
MigrationGraph {
    profiles
    migration_edges
}
```

There may be multiple paths from one profile to another.

```text
E1 -> E2 -> E4
E1 -> E3 -> E4
```

### 25.1 Migration path coherence

```text
MigrationPathCoherenceClaim {
    source_profile
    destination_profile
    paths
    relation
    witness
}
```

Two paths need not produce identical source text.

They may be coherent if their destination elaborations satisfy the protected relation.

### 25.2 No automatic diamond property

Multiple migration paths are not assumed coherent.

A counterexample is first-class.

## 26. Migration reversibility

Migration reversibility is explicit.

Possible classifications:

```text
REVERSIBLE
LEFT_INVERTIBLE
RIGHT_INVERTIBLE
PARTIAL_INVERSE
LOSSY
IRREVERSIBLE
UNKNOWN
```

A migration may be correct and intentionally irreversible.

## 27. Migration loss

```text
MigrationLoss {
    surface_information_loss
    semantic_freedom_loss
    default_resolution_loss
    annotation_loss
    observation_change
    proof_evidence_loss
}
```

A migration that loses information must not be reported simply as `SUCCESS` without preserving the loss record.

## 28. Language migration closure

Language migration can affect more than source.

```text
LanguageMigrationClosure {
    source_artifacts
    proof_artifacts
    tests
    benchmarks
    formatters
    linters
    build_profiles
    code_generators
    FFI_artifacts
    documentation_artifacts
}
```

The closure describes affected artifacts.

It does not mean every artifact must be rewritten.

Impact analysis determines which claims remain valid.

## 29. Deprecation

Deprecation is a lifecycle state over language features or constructs.

Initial states:

```text
ACTIVE
DISCOURAGED
SOFT_DEPRECATED
DEPRECATED
DENY_BY_DEFAULT
REMOVED_FROM_NEW_PROFILES
UNSUPPORTED
```

Deprecation state does not itself change semantics.

### 29.1 Soft deprecation

A soft-deprecated construct remains valid and supported but is discouraged for new source.

It need not have a removal date.

### 29.2 Scheduled removal

A construct scheduled for removal has an explicit future profile or policy boundary.

This is stronger than deprecation.

### 29.3 Historical validity

A construct removed from profile E4 can remain valid in profile E2 indefinitely as a historical semantic fact.

## 30. Support policy versus semantic validity

```text
SemanticValidity
SupportStatus
```

are distinct.

A tool may stop supporting profile E2 while E2 remains an immutable semantic profile.

Historical artifacts can therefore remain interpretable in principle even after official support ends.

## 31. Specification defects

Evolution must distinguish:

```text
ImplementationDefect
SpecificationAmbiguity
SpecificationSemanticDefect
```

### 31.1 Implementation defect

If the specification already defines correct behavior and an implementation diverges, fixing the implementation does not necessarily create a new language profile.

### 31.2 Editorial clarification

If a formal rule is unchanged and prose becomes clearer, the specification revision may change while semantic profile identity remains stable.

### 31.3 Ambiguous specification

If the authoritative specification genuinely admitted more than one interpretation, historical ambiguity should be recorded rather than retroactively pretending one interpretation was always uniquely established.

### 31.4 Semantic defect correction

If the actual semantic rule changes, a new semantic profile is required.

A safety or security repair may justify the incompatibility.

It does not erase that incompatibility.

## 32. Semantic restriction and refinement

A new profile can restrict behavior relative to an old profile.

Example:

```text
Old behaviors = {A, B, C}
New behaviors = {A, B}
```

This may satisfy an RFC 0020 refinement relation.

It may still break source or programs relying on behavior C.

Therefore:

```text
SemanticRefinement
```

does not imply:

```text
SourceCompatibility
```

## 33. Historical semantic preservation

Old profiles remain addressable.

This enables:

- reproduction of historical builds;
- interpretation of archived source;
- validation of old proofs;
- forensic comparison;
- migration testing.

A corrected profile may supersede an old one for new development.

It does not rewrite the old profile.

## 34. Reference semantics

A profile should bind to a versioned reference-semantics artifact where feasible.

```text
ReferenceSemantics {
    language_profile
    core_rules
    evaluator_or_relation_spec
}
```

The reference artifact should have RFC 0031 provenance.

A reference implementation is not automatically the specification unless the profile explicitly grants it that status.

## 35. Compiler conformance

```text
CompilerConformanceClaim {
    compiler
    language_profile
    conformance_domain
    method
    evidence
}
```

A compiler is never merely `MNCS compliant` without profile scope.

### 35.1 Conformance versus compatibility

A compiler may perfectly conform to P2 while P2 intentionally differs from P1.

Conformance and language-version compatibility are independent.

### 35.2 Conformance evidence

Conformance evidence may include:

- proofs;
- certificate checking;
- differential reference execution;
- conformance suites;
- bounded fuzzing.

RFC 0034 governs empirical scope.

## 36. Conformance suite versioning

A conformance suite is scoped to a language profile and feature set.

```text
ConformanceProfile {
    language_profile
    feature_profile
    fixture_set
    relation_set
}
```

Passing an old suite cannot establish support for a newly added feature.

## 37. Mixed-profile graphs

One project may contain many profiles.

```text
MixedProfileGraph {
    source_artifacts
    language_profiles
    dependency_edges
    core_generations
    bridge_requirements
}
```

Every source artifact keeps its local language identity.

The graph can be valid even when several editions coexist.

## 38. Profile boundaries

A profile boundary occurs when artifacts interpreted under distinct language profiles interact.

Compatibility is ideally checked after elaboration against explicit component contracts.

```text
A@E2 -> CoreA
B@E4 -> CoreB
```

RFC 0014 then determines whether CoreA/CoreB contracts are compatible.

## 39. Core bridges

When two source profiles target different core generations, a core bridge may be required.

```text
CoreBridge {
    old_core
    new_core
    translation
    preserved_relations
    witness
}
```

Core bridging is language-representation evolution.

It is not runtime deployment version coexistence from RFC 0030.

## 40. Proof migration

Language evolution may invalidate proof terms.

Proof migration can use explicit equivalences or refinements to construct new proof terms.

```text
ProofMigration {
    old_proof
    old_profile
    new_profile
    transport_relation
    generated_proof
}
```

The proof-migration algorithm remains untrusted.

RFC 0007 checks the generated proof.

## 41. Type and structure transport

Structured changes to datatypes, ASTs, or proof objects can expose explicit evolution relations.

```text
TypeEvolutionRelation {
    old_type
    new_type
    correspondence
    data_transport
    function_transport?
}
```

Ornament-like theories and bidirectional transformations are candidate realization/theory families.

They do not define the ontology.

## 42. Evidence transport

Evidence is not automatically reusable across language profiles.

```text
EvidenceTransportClaim {
    evidence
    old_subject
    new_subject
    profile_delta
    applicability_relation
}
```

Examples:

- a proof may transport;
- a benchmark may become stale;
- a side-channel experiment may need rerun;
- a placement witness may remain valid;
- a serializer compatibility claim may remain unaffected.

The result is claim-class specific.

## 43. Evolution impact

```text
EvolutionImpact {
    language_delta

    affected_source
    affected_elaborations
    affected_proofs
    affected_empirical_evidence
    affected_builds
    affected_interfaces
    affected_tools

    invalidated_claims
    preserved_claims
    unresolved_claims
}
```

Impact analysis should be dependency-driven rather than globally invalidating all evidence.

## 44. Localized invalidation

Examples:

### 44.1 Grammar-only change

Potentially invalidates:

- parser evidence;
- source migrations;
- formatter behavior.

Core semantic proofs may remain valid.

### 44.2 Defaulting change

Invalidates elaboration claims only for source that depended on the changed default.

Explicitly annotated source may remain stable.

### 44.3 Core semantic change

May invalidate:

- proofs;
- behavior relations;
- empirical equivalence evidence;
- lowering preservation evidence.

The exact impact remains dependency scoped.

## 45. Evolution-stable source

Some source constructs may be more stable under a declared language delta.

```text
EvolutionStableUnder<Source, Delta, Relation>
```

For example, explicit numeric annotation can be stable across a defaulting change that alters implicit literals.

This property may guide future source generation.

It must not force maximal explicitness as a universal design rule.

## 46. Elaboration witnesses as migration inputs

RFC 0035 elaboration witnesses can record decisions made under an old profile.

Migration tooling may use those witnesses to preserve old meaning explicitly.

Example:

```text
Old source:
    let x = 1

Old witness:
    inferred integer type = i64

New profile default:
    BigInt

Migration:
    let x: i64 = 1
```

The migration preserves the old core choice without pretending the new default is compatible.

## 47. Feature interaction experiments

RFC 0034 can test combinations of features using combinatorial, differential, and property-based methods.

Results remain bounded empirical evidence.

A feature-interaction corpus can expose counterexamples that motivate formal or specification changes.

## 48. Evolution proposal artifact

A language change should be representable as:

```text
LanguageEvolutionProposal {
    proposed_profile
    predecessor_profiles

    feature_changes
    grammar_changes
    elaboration_changes
    core_changes

    compatibility_claims
    migration_plan
    deprecation_plan

    proof_impact
    tooling_impact
    empirical_plan
}
```

This is a proposal, not an accepted profile.

## 49. Feature stabilization

Feature maturity is an RFC 0018 appraisal decision.

A stabilization profile may require some combination of:

- formal semantics;
- reference semantics;
- implementation;
- independent implementation;
- conformance fixtures;
- compatibility analysis;
- migration path;
- empirical feature-interaction studies;
- proof/evidence impact analysis.

Elapsed time alone is insufficient.

## 50. Specification lineage

Profiles and specifications form provenance-bearing lineage.

```text
SpecificationLineage {
    profile
    predecessors
    approved_deltas
    feature_proposals
    compatibility_evidence
    migration_rules
    proof_refs
}
```

RFC 0031 owns exact derivation/provenance of those artifacts.

## 51. Canonical profile identity

Long-term, the project should support a canonical digest of meaning-affecting profile data.

Potential components:

- core semantic rule set;
- feature definitions;
- elaboration profile;
- default profile;
- prelude identity;
- proof-kernel profile.

Human-readable names remain aliases over this exact identity.

## 52. Migration provenance

```text
MigrationProvenance {
    source_artifact
    source_profile
    migration_tool_or_rule
    language_delta
    destination_artifact
    destination_profile
    witness
}
```

Manual changes after automatic migration are separate deltas.

## 53. AI- and heuristic-assisted migration

LLMs, symbolic tools, rewrite engines, and Forge can propose migrations.

They do not receive semantic authority merely because they generated source accepted by the destination compiler.

The required architecture is:

```text
Migration proposal
    -> destination elaboration
    -> old/new semantic relation checking
    -> migration witness
    -> acceptance/rejection
```

## 54. Search and checking

Evolution planning can involve expensive or heuristic search.

Examples:

- finding a migration;
- finding a compatibility-preserving syntax;
- finding a bridge feature;
- finding a proof repair;
- finding a minimal deprecation path.

Search remains outside the TCB.

Narrow checkers validate:

- feature-profile satisfiability;
- declared migration relation;
- conservative-extension witnesses;
- profile-boundary compatibility;
- proof terms;
- migration-path coherence.

## 55. Migration alternatives and RFC 0033

Several migrations may preserve the required relation.

RFC 0036 establishes validity.

RFC 0032 can quantify:

- source churn;
- build cost;
- proof-repair cost;
- migration time;
- evidence invalidation.

RFC 0033 then decides preference.

No migration generator should smuggle preference into semantic validity.

## 56. Quantitative evolution impact

RFC 0032 may quantify:

```text
AffectedArtifactCount
MigrationWork
ReverificationCost
BuildCost
CompatibilityBridgeCost
ProofRepairCost
```

These quantities never replace semantic compatibility.

## 57. Runtime lifecycle boundary

Language-profile evolution and deployed-software lifecycle are distinct.

RFC 0036:

```text
source/specification/profile coexistence
```

RFC 0030:

```text
running software version coexistence
```

A language migration may eventually produce a release that enters RFC 0030.

The language migration itself is not a deployment.

## 58. Data-schema evolution boundary

RFC 0027 owns application/user data schema evolution.

RFC 0036 owns the MNCS language/specification itself.

A language feature may affect how schemas are expressed.

That does not move schema-evolution semantics into RFC 0036.

## 59. Tool capability negotiation

Tools may advertise supported profile/features.

```text
ToolLanguageCapabilities {
    supported_profiles
    supported_features
    supported_core_generations
}
```

A capability report does not activate features for source artifacts.

It only supplies compatibility facts.

## 60. Reserved language space

The language may reserve syntax or semantic namespaces for future evolution.

```text
ReservedLanguageSpace {
    lexical
    syntactic
    semantic_namespace
}
```

Reservation can reduce future migration cost.

It protects only the declared dimension and is not a universal compatibility guarantee.

## 61. Evolution rollback

A project may choose to move its source profile from E4 back to E3.

This is language-profile migration.

It is not RFC 0030 runtime rollback.

Reverse migration may require explicit reverse transforms and may be impossible after adoption of E4-only semantics.

## 62. Partial project migration

Projects can migrate incrementally.

```text
A@E2
B@E2
C@E3
D@E4
```

is valid when boundaries remain compatible.

A global project-wide edition flip is not required by the semantic model.

## 63. Language migration and intentional realization freedom

Migration must preserve realization freedom unless its contract intentionally resolves it.

For example, a location variable owned by RFC 0029 should not be concretized merely because the migration engine needs to print new source.

Likewise, algorithm, representation, or selection freedom should survive when preserved by the source semantics.

## 64. Compatibility counterexamples

```text
CompatibilityCounterexample {
    old_profile
    new_profile
    source_or_core
    claimed_relation
    observation
    explanation
}
```

A counterexample refutes only the scoped compatibility relation it actually violates.

A parsing failure does not automatically refute binary compatibility.

A timing difference does not automatically refute functional compatibility.

## 65. Migration counterexamples

```text
MigrationCounterexample {
    migration
    source_artifact
    destination_artifact
    intended_relation
    violating_observation
}
```

This becomes a Forge repair target under RFC 0004.

## 66. Evolution delta artifact

```text
EvolutionDelta {
    old_profile
    new_profile

    changed_semantic_components
    compatibility_results
    migration_results
    invalidated_claims
    preserved_claims
    unresolved_claims
}
```

This is a machine-readable summary, not a replacement for underlying evidence.

## 67. Diagnostic projection

Human diagnostics such as:

```text
"construct X is deprecated"
"edition migration changed default integer type"
"feature set is inconsistent"
```

should be projections from structured deprecation/evolution/constraint artifacts.

The semantic data remains primary.

## 68. Theory synthesis

RFC 0036 draws selectively from several theory and practice families.

### 68.1 Conservative extension

Structural operational semantics and related formal-language theory provide conservative-extension criteria: new rules/constructs should not alter old-term semantics unless intended.

MNCS generalizes this through RFC 0020 relation-scoped conservativity.

### 68.2 Definitional extension and desugaring

Surface additions that elaborate into existing core constructs can preserve a core theory while expanding source expressiveness.

This is useful but must still account for grammar and elaboration interactions.

### 68.3 Editions

Rust-style editions demonstrate opt-in source incompatibility, gradual migration, coexistence, and a common-core strategy.

MNCS adopts the conceptual separation rather than a particular edition cadence.

### 68.4 Compatibility taxonomies

Java-style binary compatibility demonstrates that source and binary compatibility are distinct.

MNCS extends that principle across the entire semantic stack.

### 68.5 Version-label policies

Semantic Versioning is useful as a release communication policy but does not provide semantic evidence by itself.

### 68.6 Deprecation lifecycle

Python-style ordinary versus soft deprecation demonstrates that discouragement and scheduled removal are different policy states.

MNCS models these states directly.

### 68.7 Feature models

Software-product-line feature models provide requires/excludes constraints and SAT-style configuration validation.

MNCS uses those concepts for language feature profiles.

### 68.8 Feature interaction theory

Feature interaction demonstrates that individually valid or conservative features can interact non-compositionally.

MNCS therefore requires set-level interaction analysis.

### 68.9 Grammar convergence

Grammar-convergence techniques are useful for comparing and reconciling parser, documentation, formal, and tooling grammars across versions.

### 68.10 Language/metamodel co-evolution

Model-driven engineering provides a large body of work on migrating models/artifacts when their defining metamodel/language changes.

MNCS generalizes this to source, proof, test, build, and tooling artifacts.

### 68.11 Semantic patches

Coccinelle-style collateral evolution demonstrates systematic structural source migration based on semantic patterns rather than raw text replacement.

### 68.12 Refactoring

Behavior-preserving refactoring provides a useful class of migrations whose defining property is an explicit preserved relation.

### 68.13 Bidirectional transformations

Lenses and related bidirectional-transformation theories provide useful foundations for maintaining consistency between old/new representations and preserving nonsemantic information where possible.

### 68.14 Ornaments and functional transport

Type ornaments and related theories can express structured datatype evolution and transport functions across related data descriptions.

### 68.15 Proof repair

Proof-repair systems show that explicit equivalences can support transport of proof artifacts after specification/type evolution.

The generated proof remains checked by RFC 0007.

### 68.16 Feature-proposal governance

WebAssembly-style staged feature proposals illustrate modular feature evolution and tool capability reporting.

MNCS treats maturity/status as assurance/governance metadata distinct from semantics.

### 68.17 Proof-producing migration

The MNCS synthesis is to keep powerful migration/evolution generation outside the TCB while requiring explicit preservation artifacts and small checkers inside it.

## 69. Semantic planes

The RFC is organized conceptually into ten planes.

### 69.1 Version identity plane

```text
LanguageProfile
SpecificationRevision
Edition
CoreGeneration
LanguageProfileId
```

### 69.2 Feature plane

```text
LanguageFeature
FeatureVersion
FeatureStatus
FeatureRequirement
FeatureDependency
FeatureConflict
FeatureProfile
FeatureClosure
```

### 69.3 Compatibility plane

```text
CompatibilityClaim
CompatibilityEnvelope
ConservativeExtensionClaim
CoexistenceClaim
```

### 69.4 Evolution plane

```text
LanguageEvolutionGraph
LanguageDelta
GrammarDelta
ElaborationDelta
CoreSemanticDelta
FeatureDelta
```

### 69.5 Migration plane

```text
LanguageMigration
MigrationGraph
MigrationPath
MigrationLoss
MigrationWitness
LanguageMigrationClosure
```

### 69.6 Deprecation plane

```text
DeprecationState
DeprecationPolicy
Replacement
RemovalPolicy
```

### 69.7 Coexistence plane

```text
MixedProfileGraph
ProfileBoundary
CoreBridge
ToolLanguageCapabilities
```

### 69.8 Proof/evidence plane

```text
ProofMigration
EvidenceTransportClaim
EvolutionImpact
ClaimInvalidation
```

### 69.9 Conformance plane

```text
ReferenceSemantics
CompilerConformanceClaim
ConformanceProfile
```

### 69.10 Assurance plane

```text
EvolutionWitness
CompatibilityCounterexample
MigrationCounterexample
EvolutionDelta
```

## 70. Initial artifact vocabulary

The first implementation should reserve versioned forms for at least:

```text
LanguageProfile
LanguageProfileId
Edition
CoreGeneration

LanguageFeature
FeatureProfile
FeatureRequirement

LanguageDelta
GrammarDelta
ElaborationDelta

CompatibilityClaim
ConservativeExtensionClaim

LanguageMigration
MigrationWitness
MigrationLoss

DeprecationState
MixedProfileGraph

ProofTransportClaim
CompilerConformanceClaim

EvolutionImpact
EvolutionCounterexample
EvolutionDelta
```

## 71. Micro-verifier direction

Candidate narrow verifiers include:

```text
FeatureProfileConstraintChecker
GrammarCompatibilityChecker
ProfileIdentityChecker
ConservativeExtensionWitnessChecker
ElaborationCompatibilityChecker
MigrationRelationChecker
MigrationPathCoherenceChecker
MigrationLossChecker
MixedProfileBoundaryChecker
CoreBridgeRelationChecker
ProofTransportChecker
EvidenceApplicabilityChecker
CompilerProfileConformanceChecker
EvolutionImpactDependencyChecker
```

The objective is not to put the entire language-evolution planner inside a trusted verifier.

The planner can remain complex and untrusted.

## 72. Fixture A: compiler version is not language meaning

Compile the same `Source@P1` with two compiler versions.

Both produce core terms equivalent under the required relation.

Required result:

```text
CompilerIdentity: different
LanguageProfileIdentity: same
SourceMeaning: preserved
```

## 73. Fixture B: conservative feature addition

Add feature F to produce P2.

Old-program domain retains semantics under relation R.

Required result:

```text
ConservativeExtension<P1,P2,R>: SUPPORTED
```

## 74. Fixture C: additive but non-conservative

Add a new implicit interface instance.

Old source that was uniquely resolved becomes ambiguous.

Required result:

```text
FeatureAdded: true
ParsingCompatibility: PASS
ConservativeExtension: REFUTED
```

## 75. Fixture D: source accepted but meaning changed

Old source parses and typechecks under both profiles.

A default or overload change produces a different core.

Required result:

```text
SourceAcceptanceCompatibility: PASS
ElaborationCompatibility: FAIL
```

This fixture is mandatory for the constitutional distinction.

## 76. Fixture E: syntax-breaking, core-preserving edition migration

Old syntax is rejected under E2.

Automatic migration rewrites source.

Old/new sources elaborate to equivalent core.

Required result:

```text
SourceCompatibility: FAIL
CoreCompatibility: PASS
MigrationRelation: PASS
```

## 77. Fixture F: default-profile evolution

E1:

```text
integer default = i64
```

E2:

```text
integer default = BigInt
```

Implicit source changes meaning.

Explicit `i64` source remains stable.

Required result:

```text
ImplicitSource EvolutionStable: FAIL
ExplicitSource EvolutionStable: PASS
```

## 78. Fixture G: editorial correction

Specification wording changes.

Canonical semantic rules do not.

Required result:

```text
SpecificationRevision: changed
LanguageProfileId: unchanged
```

## 79. Fixture H: genuine semantic defect correction

A faulty core rule changes.

Required result:

```text
LanguageProfileId: changed
CompatibilityDelta: explicit
HistoricalProfile: retained
```

## 80. Fixture I: soft deprecation

Feature remains valid and tested but is discouraged.

Required result:

```text
SemanticMeaning: unchanged
DeprecationState: SOFT_DEPRECATED
RemovalScheduled: false
```

## 81. Fixture J: removal from new edition

Feature F remains valid in E2 but is unavailable in E4.

Compiler supports both profiles.

Required result:

```text
F@E2: valid
F@E4: invalid
HistoricalMeaning: preserved
```

## 82. Fixture K: mixed editions

A@E2 imports B@E4.

Both target a compatible core contract.

Required result:

```text
MixedProfileGraph: valid
A profile after import: E2
B profile: E4
```

No importer profile mutation is permitted.

## 83. Fixture L: feature-profile conflict

```text
F1 requires F2
F3 excludes F2
```

Profile requests F1 + F3.

Required result:

```text
FeatureProfile: UNSAT
```

before source interpretation.

## 84. Fixture M: feature interaction

F1 and F2 each appear conservative independently.

Their combination changes RFC 0035 resolution for old source.

Required result:

```text
Conservative(F1): SUPPORTED
Conservative(F2): SUPPORTED
Conservative(F1+F2): REFUTED
```

## 85. Fixture N: migration-path coherence

Compare:

```text
E1 -> E2 -> E4
E1 -> E3 -> E4
```

Final source text may differ.

Required check:

```text
Elaborate(path1_result, E4)
    R
Elaborate(path2_result, E4)
```

## 86. Fixture O: lossy migration

Migration remains valid but resolves an old semantic freedom or loses source metadata.

Required result includes explicit `MigrationLoss`.

No generic lossless success may be claimed.

## 87. Fixture P: proof repair

A structured type change is related by explicit equivalence.

Migration tool generates a transported proof.

RFC 0007 accepts it.

Required result:

```text
ProofMigration: PASS
MigrationToolTrust: not required for proof truth
```

## 88. Fixture Q: empirical evidence does not automatically transport

Functional proof transports successfully.

Old performance benchmark has a profile-dependent applicability edge.

Required result:

```text
Proof: transported
BenchmarkEvidence: STALE or UNRESOLVED
```

## 89. Fixture R: AI migration

An AI proposes two migrations.

One preserves required core semantics.

One only compiles.

Independent checker must accept the first and reject the second regardless of model confidence.

## 90. Fixture S: support retirement is not semantic erasure

New compiler C4 stops implementing E1.

Required result:

```text
ToolSupports(C4,E1): false
SemanticProfile(E1): still valid historical object
```

## 91. Fixture T: compatibility relation separation

Construct a profile change with:

```text
ParsingCompatibility: PASS
ElaborationCompatibility: PASS
FunctionalBehaviorCompatibility: PASS
BitwiseNumericCompatibility: FAIL
```

Required result must preserve all four claims independently.

## 92. Fixture U: bounded corpus compatibility

Compile the entire known historical corpus under P1 and P2.

No counterexample found.

Required result:

```text
CorpusCompatibility: SUPPORTED within corpus
UniversalConservativeExtension: not proven by corpus alone
```

## 93. Fixture V: intentional semantic adoption

Migration policy explicitly requests:

```text
ADOPT_NEW_SEMANTICS
```

The transformed source differs from old behavior under relation R.

This is not reported as migration failure if the declared target semantics were intentionally changed and all stated obligations are met.

The semantic delta remains explicit.

## 94. Forge study 1: feature proposal compatibility laboratory

Input:

```text
CurrentProfile E1
CandidateFeature F
ProposedProfile E2
```

Forge generates:

- grammar delta;
- elaboration delta;
- feature constraints;
- migration candidates;
- conservativity obligations.

Formal checking covers an explicit semantic fragment.

RFC 0034 experiments run the historical corpus under both profiles.

Example outcome:

```text
Formal core fragment:
    conservative

Corpus:
    3 binding changes found

Overall proposal:
    not ready for stabilization
```

The experiment demonstrates that formal and empirical evidence can complement without replacing each other.

## 95. Forge study 2: automatic edition migration

E2 changes:

- a keyword;
- numeric default;
- interface-resolution rule;
- explicit safety requirement.

Forge uses RFC 0035 old elaboration witnesses to generate preserving annotations where needed.

For each transformation it records:

```text
old_source
old_core
reason_for_change
new_source
new_core
preserved_relation
```

The destination core is independently checked.

## 96. Forge study 3: core generation evolution

Propose C2 from C1.

A translation `T : C1 -> C2` is generated.

Check independently whether T preserves:

- functional behavior;
- effects;
- failure semantics;
- numerical relations;
- information flow;
- termination/progress obligations.

Results may differ by relation.

No universal `core compatible` bit is permitted.

## 97. Forge study 4: feature interaction search

Candidate features F1–F4 each pass individual checks.

RFC 0034 combinatorial exploration finds that F2+F4 changes RFC 0035 overload resolution.

Produce:

```text
FeatureInteractionCounterexample
```

before stabilization.

## 98. Forge study 5: proof/evidence co-evolution

Apply one language change and compute an impact graph.

Example:

```text
Source migration: succeeds
Core proof: transported
Serialization evidence: unaffected
Performance benchmark: stale
Side-channel evidence: stale
Placement evidence: preserved
```

The study demonstrates localized rather than global invalidation.

## 99. Forge study 6: machine-assisted language evolution

Prompt Forge with a design goal such as:

```text
Remove construct X while preserving old program behavior where possible.
```

Forge may propose:

- new syntax;
- migration rules;
- compatibility profiles;
- proof repairs;
- deprecation metadata.

The system independently checks grammar, elaboration, core relation, migration correctness, and feature interactions.

The AI/planner is not the authority on whether preservation succeeded.

## 100. Bounded implementation pilot

The first implementation pilot should remain bounded but structurally complete.

It should implement versioned forms for:

```text
LanguageProfile
LanguageProfileId
Edition
CoreGeneration
LanguageFeature
FeatureProfile
LanguageDelta
GrammarDelta
ElaborationDelta
CompatibilityClaim
ConservativeExtensionClaim
LanguageMigration
MigrationWitness
MigrationLoss
DeprecationState
MixedProfileGraph
EvolutionImpact
EvolutionCounterexample
EvolutionDelta
```

It should initially support:

- two editions;
- one shared core generation;
- one deliberately changed elaboration/default profile;
- one feature dependency/conflict graph;
- one conservative feature addition;
- one non-conservative additive feature;
- one automatic source migration;
- one mixed-edition dependency graph;
- one proof/evidence impact example.

## 101. Initial micro-verifiers

The first verifier slice should include:

```text
FeatureProfileConstraintChecker
LanguageProfileIdentityChecker
GrammarDeltaChecker
ElaborationCompatibilityChecker
MigrationRelationChecker
MigrationLossChecker
MixedProfileBoundaryChecker
EvolutionImpactDependencyChecker
```

Where practical, conservative-extension and proof-transport checking should be implemented as narrow certificates checked by existing RFC 0007/RFC 0020 mechanisms rather than one monolithic evolution verifier.

## 102. 1.0 research threshold

RFC 0036 is not complete enough for the MNCS research 1.0 threshold until the project demonstrates all of the following.

### 102.1 Identity

- compiler identity distinct from language-profile identity;
- specification editorial revision distinct from semantic-profile identity;
- edition distinct from core generation;
- feature semantic version distinct from maturity status;
- canonical/versioned language-profile identity.

### 102.2 Historical semantics

- a published profile remains immutable;
- a newer compiler can execute/check an older profile without reinterpreting it;
- support retirement does not erase old semantic identity;
- historical reference semantics can be provenance-bound.

### 102.3 Feature profiles

- explicit feature identities;
- feature dependencies/conflicts;
- `SAT`/`UNSAT`/`UNKNOWN` profile checking;
- support versus enablement versus use distinction;
- one feature-interaction counterexample.

### 102.4 Compatibility

- at least four independently represented compatibility relations;
- source accepted but semantic compatibility failing;
- parsing compatibility passing while another protected relation fails;
- relation-scoped compatibility counterexamples;
- bounded corpus compatibility refusing to claim universal conservativity.

### 102.5 Conservative extension

- one supported conservative-extension case;
- one additive-but-non-conservative case;
- explicit source-domain and RFC 0020 relation in the claim;
- no inference of conservativity from version labels or test pass count.

### 102.6 Migration

- source-profile migration with explicit source/destination profile;
- independently checked migration relation;
- compatibility migration distinct from idiomatic migration;
- `MigrationLoss` for one lossy case;
- direct/staged migration-path coherence study;
- an AI/heuristic migration accepted only through independent checking.

### 102.7 Deprecation

- soft deprecation distinct from scheduled removal;
- deprecated semantics unchanged;
- removed construct remaining valid under its historical profile;
- structured replacement/migration metadata.

### 102.8 Elaboration evolution

- an RFC 0035 default change that changes implicit source meaning;
- explicit source that remains stable across the same delta;
- migration using old elaboration evidence to preserve meaning;
- imported newer-profile source unable to mutate importer profile.

### 102.9 Proof and evidence co-evolution

- proof transport accepted by RFC 0007;
- empirical evidence independently evaluated for applicability;
- localized invalidation through an `EvolutionImpact` graph;
- at least one claim class preserved while another becomes stale.

### 102.10 Mixed-profile coexistence

- two source profiles in one dependency graph;
- profile-local interpretation;
- compatible linking through explicit core/component contracts;
- no implicit project-wide edition mutation.

### 102.11 Forge capstone

An end-to-end language-evolution study must:

1. propose a new profile;
2. produce grammar/elaboration/feature/core deltas;
3. check feature-profile validity;
4. generate compatibility claims;
5. run bounded RFC 0034 compatibility experiments;
6. find or prove at least one compatibility counterexample or conservativity witness;
7. generate a migration;
8. independently check the migration relation;
9. produce an evolution-impact graph;
10. preserve historical profile identity;
11. hand multiple valid migration alternatives to RFC 0033 when preference remains;
12. refuse to equate successful compilation with preserved meaning.

## 103. Integration with RFC 0031 through RFC 0035

RFC 0036 closes the semantic foundation needed to evolve the newer roadmap tracks coherently.

The integrated sequence is:

```text
Language/Profile definition and evolution — RFC 0036
        |
        v
Source meaning / explicit elaboration — RFC 0035
        |
        v
Core and semantic verification — earlier RFCs
        |
        v
Derivation / provenance — RFC 0031
        |
        v
Empirical evidence generation — RFC 0034
        |
        v
Quantitative facts / feasibility — RFC 0032
        |
        v
Preference / selection — RFC 0033
        |
        v
Deployment lifecycle — RFC 0030
```

The order is not purely linear; the evidence graph is recursive.

For example, RFC 0033 may request more RFC 0034 experiments, while RFC 0036 evolution may invalidate only a subset of RFC 0031–0034 evidence.

## 104. Cross-RFC ownership boundaries

### RFC 0004

Owns recursive repair/refinement. RFC 0036 exposes evolution deltas and counterexamples that become repair targets.

### RFC 0005

Owns source representations. RFC 0036 owns version relations among source representations.

### RFC 0007

Owns proof checking. RFC 0036 owns proof transport/migration across language profiles.

### RFC 0012

Owns executable semantic core. RFC 0036 versions and relates core generations.

### RFC 0013

Owns abstraction/polymorphism/interface semantics. RFC 0036 owns evolution of those language rules.

### RFC 0014

Owns components/linking. RFC 0036 supplies profile/coexistence facts at source-language boundaries.

### RFC 0016

Owns metaprogramming. RFC 0036 owns evolution of metaprogramming language rules.

### RFC 0018

Owns appraisal/authorization for feature stabilization, migration acceptance, and compatibility evidence.

### RFC 0020

Owns relation semantics. RFC 0036 instantiates relations for profiles, migrations, and conservative extensions.

### RFC 0027

Owns application/data schema evolution. RFC 0036 owns MNCS language/specification evolution.

### RFC 0030

Owns runtime deployment lifecycle and live version coexistence. RFC 0036 owns language/source/profile coexistence.

### RFC 0031

Owns provenance and reproducibility of specification, compiler, migrator, reference semantics, and migration artifacts.

### RFC 0032

Owns quantitative migration/evolution cost.

### RFC 0033

Owns preference among several valid migrations or evolution realizations.

### RFC 0034

Owns empirical compatibility experiments and feature-interaction testing.

### RFC 0035

Owns elaboration within a profile. RFC 0036 owns how elaboration profiles evolve and relate.

## 105. Rejected simplifications

This RFC explicitly rejects the following simplifications.

### 105.1 `language_version = compiler_version`

Rejected because compilers can implement several profiles and can change internally without changing language meaning.

### 105.2 One `backward_compatible` bit

Rejected because compatibility is multi-dimensional and relation-scoped.

### 105.3 SemVer as semantic truth

Rejected because version numbers classify already-established compatibility facts rather than establish them.

### 105.4 `old source compiles` as compatibility proof

Rejected because parsing/typechecking can succeed while elaboration or behavior changes.

### 105.5 Automatic migration as compiler acceptance

Rejected because destination acceptance does not establish preserved meaning.

### 105.6 Experimental feature semantics may float

Rejected because artifacts depending on an experimental feature still require reproducible historical meaning.

### 105.7 Deprecation means removal

Rejected because discouragement, soft deprecation, scheduled removal, and profile removal are distinct.

### 105.8 Updating the spec in place

Rejected because published semantic profiles must remain immutable.

### 105.9 Retest everything after every language change

Rejected because evolution impact should invalidate only dependency-related claims where possible.

### 105.10 AI migration authority

Rejected because generative migration systems remain proposal mechanisms, not semantic authorities.

## 106. Open research questions

RFC 0036 deliberately leaves several questions open for experiment.

1. What should the canonical encoding of a complete language profile include?
2. Which profile components can safely be independently versioned?
3. How much conservative-extension proof can be automated for the executable core?
4. Which elaboration changes can be proven extension-stable?
5. How should feature-interaction search scale beyond bounded combinatorial campaigns?
6. Which migration path-coherence classes are worth proving universally versus testing empirically?
7. How much source trivia should bidirectional migration preserve?
8. What proof-transport techniques best fit RFC 0007?
9. How should long-term historical profile interpreters be archived and reproduced?
10. Should one stable core generation span many editions or should core generations evolve more frequently?
11. Which compatibility claims should be mandatory before feature stabilization?
12. How should profile support retirement interact with long-lived package ecosystems?
13. How should experimental dialect branches merge back into a stable lineage?
14. Can Forge synthesize minimally breaking language deltas from a desired semantic change?
15. Can compatibility impact be predicted before implementation and then validated after implementation?

## 107. Final architectural rule

RFC 0036 establishes the following architectural rule for MNCS:

> **A programming language evolves safely only when the relationship between its old and new meanings is as explicit and checkable as the meanings themselves.**

For MNCS specifically:

> **Machines must never be asked to trust that a new language release is “backward compatible.” They must be able to identify the exact old and new semantic profiles, inspect the language delta, determine which compatibility relations are claimed, generate or inspect migration paths, preserve historical meaning, localize affected evidence, and independently check which semantic relationships actually survived.**

This allows MNCS to evolve aggressively without making the past ambiguous.
