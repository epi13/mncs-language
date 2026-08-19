# RFC 0016: Machine-Native Staging, Metaprogramming, Introspection, Specialization, and Self-Transformation Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.4–1.0
- **Depends on:** RFC 0001, RFC 0002, RFC 0003, RFC 0004, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0013, RFC 0014, RFC 0015

## Summary

MNCS Language should treat staging, metaprogramming, specialization, reflection, generation, and recursive self-transformation as **typed semantic operations over first-class semantic artifacts**, not as unrestricted text manipulation, compiler magic, or privileged macro execution.

The central rule is:

> **MNCS metaprogramming manipulates typed semantic artifacts, not unstructured source text. Every stage transition preserves explicit context, identity, authority, effects, assumptions, obligations, evidence, and transformation provenance.**

A second rule is equally important:

> **Self-inspection does not imply self-mutation, self-mutation does not imply self-promotion, and self-hosting does not erase bootstrap trust.**

RFC 0016 therefore establishes a machine-native staging model in which:

- stage is a first-class semantic dimension;
- artifacts can be open over explicit semantic contexts;
- facts, values, evidence, authority, representation commitments, and effects have stage-validity rules;
- compile-time, generation-time, proof-time, link-time, deployment-time, load-time, and runtime execution obey ordinary authority and effect semantics;
- reflection, candidate generation, mutation, verification, and promotion remain distinct capabilities;
- specialization is a refinement-bearing transformation rather than an implicit compiler privilege;
- binding-time analysis may move legal work between stages conservatively;
- unresolved stage placement may remain a bounded realization hole for Forge;
- generated artifacts carry provenance and narrow invalidation dependencies;
- recursive self-transformation always produces candidates before promotion;
- self-description and self-hosting are distinct properties;
- bootstrap trust remains explicit; and
- the RFC 0012 executable semantic machine can support bounded Futamura-style specialization experiments without making self-application the production compiler architecture.

The selected theoretical synthesis draws from:

- modal type theories for staged computation;
- contextual modal type theory for typed open code and explicit contexts;
- multi-level contextual type theory for arbitrary meta-level depth;
- MetaML / MetaOCaml style typed code generation and cross-stage persistence;
- phase-distinction calculi;
- binding-time analysis and partial evaluation;
- Futamura projections;
- semantics-preserving staging and specialization;
- hygienic macro theory generalized to semantic identity and authority;
- verified metaprogramming and self-description as exemplified by systems such as MetaCoq;
- equality-saturation/e-graph ideas as a candidate-search realization rather than semantic ontology; and
- the existing MNCS refinement, evidence, authority, trust-boundary, and promotion architecture.

The result should let machines generate, inspect, specialize, transform, and recursively improve software while preserving the exact semantic facts required to determine **what was known when, what was permitted where, what changed, what remains valid, and who or what is authorized to make that change current**.

---

## Motivation

Traditional metaprogramming systems often combine several very different mechanisms under one term:

- textual macros;
- AST macros;
- templates;
- compile-time evaluation;
- reflection;
- code generation;
- partial evaluation;
- JIT specialization;
- compiler plugins;
- link-time optimization;
- procedural derivation;
- generated bindings;
- proof automation; and
- self-hosting.

These mechanisms frequently inherit accidental properties from the host implementation:

- macro execution may have ambient filesystem or network authority;
- generated syntax may capture names or contexts accidentally;
- compile-time execution may be more privileged than runtime execution;
- values, proofs, or assumptions may cross phases without explicit justification;
- generated code may lose provenance connecting it to generators and source facts;
- specialization may silently change effect ordering;
- reflection may expose mutation capabilities;
- compiler plugins may modify trusted state directly;
- self-hosting may be described as if it proves its own bootstrap chain; and
- a code generator may be trusted merely because it executes during compilation.

MNCS cannot use that model.

The previous RFCs establish explicit semantics for:

- stable semantic identity and evidence dependencies;
- recursive refinement and isolated candidate transformation;
- machine intent and realization envelopes;
- a small proof-carrying dependent core;
- effects, resources, authority, and events;
- memory, provenance, lifetime, and storage;
- concurrency and causality;
- failure, uncertainty, and recovery;
- one executable semantic machine;
- abstraction and polymorphism;
- component composition and binding; and
- unsafe/foreign trust boundaries.

Staging must preserve those semantics rather than bypass them.

MNCS therefore needs formal answers to questions such as:

- What is a stage?
- Which stages are ordered, related, or incomparable?
- At which stage does a computation execute?
- At which stage is a fact known?
- At which stage is evidence valid?
- Can evidence produced at one stage be used in another?
- Can an authority capability cross a stage boundary?
- Can generated code possess authority the generator does not possess?
- Can a generator possess authority the generated program does not possess?
- What is an open generated artifact?
- Which semantic context does it require?
- How is an open artifact inserted into another context without capture?
- How are type, region, protocol, evidence, authority, and identity capture prevented?
- What is the semantic difference between quotation, generation, specialization, reflection, and lowering?
- When may computation be moved earlier?
- When must computation remain later?
- How are effects indexed by stage?
- What does it mean to run a foreign compiler during a build stage?
- How is that foreign tool represented in the evidence chain?
- What makes a specialization correct?
- When is equivalence required and when is refinement sufficient?
- Can stage selection be left unresolved for Forge?
- How are stage choices bounded?
- How does recursive metaprogramming avoid unbounded self-application?
- How can the language describe its own type system, semantic rules, or verifier interfaces?
- How can a compiler inspect itself without obtaining automatic authority to replace itself?
- What does self-hosting prove, and what does it not prove?
- How are bootstrap identities and trust assumptions preserved?
- Can the executable semantic machine be partially evaluated into an executable realization?
- Can the specializer itself be specialized against the semantic machine?
- How are generated artifacts invalidated when a generator or one of its facts changes?

RFC 0016 establishes the semantic vocabulary needed to answer those questions.

---

## Design goals

RFC 0016 should make the following possible.

1. **Stage as semantics.** The availability and execution stage of facts, values, evidence, effects, authority, and artifacts must be explicit where relevant.
2. **Typed semantic artifacts.** Generated program fragments must be typed semantic artifacts rather than opaque text.
3. **Explicit open contexts.** Generated artifacts may be open, but all external dependencies must be represented explicitly.
4. **Semantic hygiene.** Composition must prevent accidental capture not only of names but also of identities, authority, resources, evidence, regions, and protocol/session state.
5. **Stage-scoped effects.** Effects produced by a generator must remain distinct from effects that generated code may perform later.
6. **Stage-scoped authority.** The generator's authority and the generated artifact's future authority must remain distinct.
7. **Cross-stage persistence discipline.** Facts cross stages only under explicit validity rules.
8. **Conservative specialization.** Stage motion and specialization require equivalence/refinement evidence.
9. **Automatic binding-time reasoning.** The machine may infer when facts become available and move legal computation accordingly.
10. **Bounded stage search.** Stage placement may be an optimization/search problem under finite budgets.
11. **Reflection without ambient mutation.** Read, propose, apply, and promote capabilities remain separated.
12. **Candidate-first self-transformation.** A current trusted artifact is never silently mutated in place by recursive refinement.
13. **Self-description without forced self-hosting.** Semantic rules can become inspectable artifacts before the toolchain is written in MNCS.
14. **Explicit bootstrap trust.** Self-hosted artifacts retain ancestry to initial trusted tools and assumptions.
15. **Incremental invalidation.** Changes invalidate only generated artifacts and evidence that depend on changed inputs.
16. **Foreign build-step transparency.** Foreign compilers, generators, or tools executed at earlier stages remain RFC 0015 foreign operations.
17. **Proof integration.** Stage transformations generate ordinary RFC 0007 proof/checking obligations without inflating the kernel.
18. **Machine-readable transformation lineage.** Every generated or promoted artifact can explain how it came into existence.
19. **Multiple realization strategies.** Quotation syntax, AST builders, graph transforms, partial evaluators, JITs, and equality-saturation engines are realizations of semantic staging operations rather than the ontology.
20. **Independent implementation.** The model must be precise enough that two implementations can agree on stage legality and artifact validity.

---

## Non-goals

This RFC does not:

- select final surface syntax for quotation, splicing, macros, or staging;
- require text macros;
- require AST macros;
- require MetaML notation;
- require one fixed list of named compiler stages;
- require the production compiler to be generated by Futamura projection;
- require equality saturation;
- permit arbitrary self-modifying production code;
- make reflection ambient;
- grant generators unrestricted authority;
- make compile-time execution privileged;
- make all runtime specialization legal;
- claim that self-hosting proves compiler correctness;
- erase bootstrap trust;
- require every transformation to be kernel-proved directly;
- require all artifacts to be persisted at runtime;
- define the final deployment or live-upgrade model;
- define full distributed staging semantics;
- define the final package build system; or
- require arbitrary unbounded meta-level recursion.

Unsupported or unresolved stage transformations remain rejected, conservatively delayed, represented as unresolved obligations, or `UNKNOWN` under the active analysis. They do not inherit permissive host-language semantics.

---

# Constitutional principles

## 1. Stage is semantic

> **Every computation, observation, authority use, evidence claim, and generated artifact has a stage at which it occurs, becomes available, or remains valid.**

A stage is not merely a compiler implementation phase.

It determines which facts may be used and which effects actually happen.

For example:

```text
Stage<Build>
Stage<Link>
Stage<Deploy>
Stage<Run>
```

may be useful names in one toolchain, but the semantic property is the explicit stage relation, not those particular labels.

---

## 2. Code is not text

> **Metaprogramming operates on typed semantic artifacts whose context, dependencies, identities, and obligations remain explicit.**

Text may be an input or output representation.

It is not the canonical meaning of generated code.

---

## 3. Earlier is not more privileged

> **Compile-time, generation-time, proof-time, deployment-time, and load-time execution obey the same authority and effect discipline as runtime execution.**

A generator that reads a file must possess file-read authority at the generator's stage.

A compile-time plugin that opens the network is performing an ordinary network effect.

---

## 4. Generation authority and generated authority are independent

> **A generator does not gain the authority its generated artifact may later receive, and the generated artifact does not inherit the generator's authority.**

For example:

```text
Generator authority:
    Read<Repository>
    Write<CandidateWorkspace>

Generated application authority:
    Network<ServiceAPI>
```

Neither authority set implies the other.

---

## 5. Reflection is not mutation

> **Authority to inspect an artifact does not imply authority to create, apply, or promote a replacement.**

At minimum the semantic model should be able to distinguish:

```text
Reflect<A>
ProposeTransform<A>
ApplyTransform<Candidate<A>>
Promote<A>
```

These capabilities may be held by different components.

---

## 6. Generated artifacts inherit no hidden facts

> **Facts, evidence, authority, representation commitments, and semantic identities cross stages only through explicit persistence or re-establishment rules.**

Merely embedding a value in generated code does not automatically preserve all facts known about that value at generation time.

---

## 7. Specialization requires refinement

> **Moving, erasing, generating, or specializing computation is valid only when the resulting artifact preserves required observable semantics under the applicable equivalence/refinement envelope.**

Optimization is not permission to alter semantics.

---

## 8. Self-transformation is candidate generation

> **A system may generate its own successor, but replacement requires ordinary verification, comparison, and promotion.**

The current trusted artifact is not directly overwritten by the transformation that proposes its replacement.

---

## 9. Self-hosting does not erase bootstrap trust

> **Recursive self-description and self-compilation remain connected to an explicit bootstrap trust boundary.**

A compiler compiling itself is not a proof that the original bootstrap compiler was correct.

---

## 10. UNKNOWN removes stage-motion authority

> **If the system cannot establish that moving a computation or fact across a stage boundary preserves required semantics, it must not perform that motion automatically.**

`UNKNOWN` never means "probably safe to specialize earlier."

---

## 11. Transformation lifecycle and stage are distinct

> **When an artifact exists and whether an artifact is baseline, candidate, verified, promoted, or rejected are separate semantic dimensions.**

A build-stage artifact is not trusted merely because it was produced early.

---

## 12. Self-description and self-hosting are distinct

> **A language may describe its own semantic rules, verifier contracts, and IR schemas without requiring its compiler or checker to be implemented in that language.**

This permits incremental self-description before self-hosting.

---

# Selected theoretical synthesis

## Modal type theory for staged computation

Modal staged-computation calculi provide a useful foundation for representing values and computations available at different stages.

MNCS should investigate a stage-indexed judgment such as:

```text
Γ ; S ⊢ e : T
```

where `S` identifies the stage whose facts and authority are available to the expression.

A future-stage artifact may be represented conceptually as:

```text
Artifact<T, Stage<S1>>
```

created by a computation at:

```text
Stage<S0>
```

The exact modal operators remain open.

The required semantic property is that stage validity be type/checker visible.

---

## Contextual modal type theory

A generated artifact is often open over some environment.

MNCS should therefore prefer a contextual representation:

```text
Artifact<T, Context<C>, Stage<S>>
```

rather than treating all generated code as closed `Code<T>`.

For example:

```text
Artifact<
    Computation<Int>,
    Context<{
        x : Int,
        Read<RegionR>,
        Evidence<Bounded<x, 0, 10>>
    }>,
    Stage<Run>
>
```

This artifact is meaningful before its context is filled.

Its dependencies are explicit.

Insertion into another context requires an explicit legal substitution.

---

## Multi-level contextual type theory

MNCS should not assume exactly two levels such as `compile-time` and `runtime`.

Recursive tooling may create:

```text
Program
Generator<Program>
Generator<Generator<Program>>
Verifier<Generator<Program>>
Transformation<Transformation<T>>
```

The semantic model should permit multiple stage levels subject to finite policy bounds.

Possible structure:

```text
StageGraph {
    stages
    precedence_relations
    availability_relations
}
```

The implementation may impose:

```text
max_stage_depth <= N
```

for bounded research subsets.

---

## Typed multi-stage programming

MetaML / MetaOCaml style staging provides useful inspiration for:

- typed generation;
- static scope preservation;
- quotation-like artifact construction;
- splice/instantiation operations; and
- cross-stage persistence.

MNCS should not make quotation syntax fundamental.

Instead, the semantic operation is construction of a typed staged artifact:

```text
quote_semantics(A)
    -> StagedArtifact<A>
```

and insertion is a typed/contextual operation:

```text
instantiate(
    artifact,
    context_binding
)
```

or:

```text
compose_artifact(
    target,
    artifact,
    substitution
)
```

Textual or AST implementations are realizations of these operations.

---

## Phase distinction

Staged execution must distinguish **effects performed now** from **effects represented for later execution**.

Example:

```text
Generator at S0:
    effects {}

Generated artifact at S1:
    effects { Network<Service> }
```

is different from:

```text
Generator at S0:
    effects { Network<Service> }

Generated artifact at S1:
    effects {}
```

Therefore stage-indexed effects should be expressible conceptually as:

```text
Effect<E, Stage<S>>
```

or equivalent metadata in the semantic graph.

---

## Binding-time analysis

The system should be able to classify information according to when it becomes known.

Conceptually:

```text
BindingTime {
    KnownAt<Stage>
    DynamicUntil<Stage>
    Unknown
}
```

The exact lattice remains open.

A binding-time analysis may determine that:

```text
matrix_size     KnownAt<Build>
numa_topology   KnownAt<Deploy>
request_body    KnownAt<Run>
```

This information can support legal specialization without requiring explicit annotations for every operation.

---

## Partial evaluation

Partial evaluation should be a first-class transformation class.

Conceptually:

```text
PartialEvaluate {
    artifact
    known_inputs
    known_facts
    source_stage
    target_stage

    residual_artifact

    obligations
    evidence
}
```

The residual artifact must preserve required behavior under the source artifact's semantic contract.

Partial evaluation is one possible realization of staging.

It is not itself the language ontology.

---

## Futamura projections

RFC 0012 defines an executable semantic machine.

This creates an unusually valuable research opportunity.

First bounded projection:

```text
specialize(
    SemanticMachine,
    Program<P>
)
    -> ExecutableArtifact<P'>
```

with required evidence:

```text
Behavior(P') ⊑ Behavior(P)
```

for the supported subset.

A second experiment may investigate:

```text
specialize(
    Specializer,
    SemanticMachine
)
    -> CompilerSubset
```

The purpose is not to require the production compiler to be derived this way.

The purpose is to test whether the normative executable semantics is sufficiently precise that compilation behavior can be mechanically specialized from it without introducing another independent meaning of the language.

A third projection is explicitly research-only:

```text
specialize(
    Specializer,
    Specializer
)
```

and must remain bounded by explicit meta-level and resource limits.

---

## Semantics-preserving staging

Moving computation between stages can change behavior when:

- effects occur;
- evaluation order matters;
- memory state changes;
- observations differ;
- nondeterministic sources differ;
- authority differs;
- failure timing differs; or
- external interaction occurs.

Therefore a legal stage transformation must establish a relation such as:

```text
SpecializedArtifact
    ⊑
OriginalArtifact
```

or, where exact equivalence is required:

```text
SpecializedArtifact
    ≈
OriginalArtifact
```

under the relevant observation contract.

---

## Hygienic metaprogramming generalized to semantic hygiene

Traditional hygiene protects lexical identity.

MNCS must protect more.

Potential capture classes include:

```text
NameCapture
TypeIdentityCapture
ValueIdentityCapture
AuthorityCapture
ResourceCapture
EvidenceCapture
RegionCapture
ProvenanceCapture
ProtocolSessionCapture
StageCapture
AssumptionCapture
```

A legal artifact substitution must prove that only intended context bindings occur.

This is **semantic hygiene**.

---

## Verified metaprogramming and self-description

MNCS should eventually expose semantic artifacts describing parts of itself.

Potential subjects include:

```text
TypeSystemDescription
SemanticRuleDescription
IRSchema
VerifierContract
LoweringRule
EffectDefinition
CapabilityDefinition
ObligationTemplate
```

Machines should be able to inspect questions such as:

```text
what operations exist?
what obligations does this operation generate?
what semantic state does this transition touch?
which evidence classes may discharge this obligation?
which lowering rules can realize this semantic operation?
```

through typed reflection interfaces rather than undocumented compiler internals.

---

## Equality saturation and e-graph inspired search

Equality saturation is useful as a candidate-search realization because it can retain many equivalent forms simultaneously.

MNCS should not make equality saturation normative.

Instead, a generalized transformation graph may contain:

```text
TransformationGraph {
    artifacts

    equivalent_edges
    refinement_edges
    approximation_edges

    obligations
    evidence
}
```

A rewrite rule should itself be evidence-bearing:

```text
RewriteRule {
    identity

    pattern
    replacement

    preconditions

    claim:
        Equivalent
      | Refines
      | ApproximatesWithin<Envelope>

    assumptions
    obligations
    evidence
}
```

A candidate search engine may use e-graphs internally while preserving these semantic distinctions.

---

# Stage model

## Stage identity

A stage is a semantic object.

Conceptually:

```text
Stage {
    identity

    predecessors
    successors

    available_facts
    available_types
    available_evidence

    authority
    executable_effects

    environment_identity
    target_knowledge

    persistence_rules

    assumptions
}
```

This is not necessarily a runtime struct.

It is the semantic information required to determine what may legally happen at a stage.

---

## Stage graph

Stages need not always be a simple numeric sequence.

A toolchain may have:

```text
SourceElaboration
        ↓
ProofElaboration
        ↓
Compilation
        ↓
Link
        ↓
Deployment
        ↓
Load
        ↓
Execution
```

but different realizations may use other graphs.

For example:

```text
Compilation
    ↘
     TargetA-Link
    ↘
     TargetB-Link
```

The normative model should therefore permit a partial order where useful.

---

## Example stage facts

```text
Fact {
    target_arch = x86_64
    valid_at = Build .. Run
}
```

```text
Fact {
    numa_nodes = 4
    valid_at = Deploy .. Run
}
```

```text
Observation {
    current_time = ...
    valid_at = Run
}
```

Facts do not become available earlier merely because a later stage will know them.

---

## Stage-local identities

Some identities may be valid only within a particular stage or realization.

For example:

```text
MachineAddress
ThreadId
RuntimeHandle
ProcessId
DeviceQueueId
```

A stage transition must not silently preserve such identities as if they were stable semantic identities.

This follows RFC 0009's distinction between semantic identity and physical realization identity.

---

# Staged artifacts

## Canonical staged artifact

A staged artifact should conceptually record:

```text
StagedArtifact {
    identity

    semantic_type
    semantic_identity?

    context

    produced_at
    executable_at

    dependencies
    free_semantic_variables

    required_authority
    effects_by_stage

    assumptions
    obligations
    evidence

    representation_constraints
    target_constraints

    provenance
}
```

The exact schema remains experimental.

---

## Closed artifact

```text
ClosedArtifact<T>
```

has no unresolved semantic context requirements.

It may still have target/runtime requirements.

---

## Open artifact

```text
OpenArtifact<T, Context<C>>
```

requires explicit context binding before execution or incorporation into a closed artifact.

Example:

```text
OpenArtifact<
    Computation<Int>,
    Context<{
        x : Int,
        read_r : Read<RegionR>
    }>
>
```

---

## Context substitution

Composition requires a substitution object:

```text
ContextSubstitution {
    source_context
    target_context

    bindings

    identity_mapping
    authority_mapping
    evidence_mapping
    region_mapping

    obligations
    evidence
}
```

This provides a structural hygiene boundary.

---

# Cross-stage persistence

## General rule

Cross-stage persistence is not merely "can this value be embedded?"

MNCS asks:

> Which semantic claims attached to the value remain valid after the stage transition?

A persistence record may be:

```text
CrossStagePersistence {
    artifact_or_fact

    from_stage
    to_stage

    preserved {
        semantic_identity
        type
        value?
        evidence?
        authority?
        representation?
        provenance?
    }

    invalidated
    regenerated

    obligations
    evidence
}
```

---

## Value persistence

A pure immutable integer may persist trivially:

```text
42 @ Build
    -> 42 @ Run
```

subject to representation semantics.

A runtime object identity may not.

---

## Address persistence

A machine address observed during build or load is not automatically meaningful at runtime.

```text
MachineAddress<X, Stage<Load>>
```

must not become:

```text
MachineAddress<X, Stage<Run>>
```

without explicit realization evidence.

---

## Evidence persistence

Evidence may persist only when its dependency identities and validity scope persist.

Example:

```text
Proof<Sorted(Table)>
```

may persist if `Table` is embedded without semantic change.

But:

```text
Evidence<Aligned(Buffer, 64)>
```

may not persist when the later stage chooses a different allocation.

---

## Authority persistence

Authority does not persist merely because an artifact does.

```text
Read<Repository, Stage<Build>>
```

must not become:

```text
Read<Repository, Stage<Run>>
```

unless the runtime receives that authority through an explicit ordinary capability transfer.

---

## Representation persistence

A logical value may persist while its physical representation changes.

```text
LogicalValueIdentity = same
RepresentationIdentity = changed
```

RFC 0013 representation polymorphism and RFC 0009 storage semantics remain authoritative.

---

# Stage-indexed effects

## Effect distinction

A generator that emits a future effect is not currently performing that effect.

Conceptually:

```text
Generate<Effect<Network, Stage<Run>>>
```

is not:

```text
Effect<Network, Stage<Build>>
```

The distinction must survive through HIR/SSA and evidence artifacts.

---

## Stage effect rows

An artifact may expose:

```text
StageEffects {
    Build: { Read<SchemaFile> }
    Link: {}
    Deploy: { Observe<Topology> }
    Run: { Network<Service> }
}
```

This is a conceptual representation.

The final type syntax may differ.

---

## Effect motion

Moving an operation from one stage to another is legal only if:

```text
semantic_effect_equivalence_or_refinement
&& authority_available_at_target_stage
&& required_inputs_available
&& observation_contract_preserved
&& failure_contract_preserved
```

---

# Stage-indexed authority

## Authority dimensions

Authority can be stage scoped:

```text
Authority<A, Stage<S>>
```

or represented through stage-specific capability contexts.

The key property is non-ambient transfer.

---

## Generator confinement

A generator may be restricted to:

```text
Reflect<InputArtifacts>
Read<Schema>
Write<CandidateWorkspace>
```

while being denied:

```text
Network
Write<RepositoryMain>
Promote<Compiler>
```

This confinement applies regardless of whether the generator executes during compilation.

---

## Generated authority declaration

A generated artifact may require future authority:

```text
requires_at Run {
    Network<ServiceAPI>
}
```

The generator need not possess that capability.

It merely constructs an artifact whose contract records it.

---

# Reflection and reification

## Reflection

Reflection is an explicit semantic operation:

```text
Reflect<Artifact>
    -> ReflectionView
```

A reflection view should contain only information authorized to the reflector.

Conceptually:

```text
ReflectionView {
    subject

    visible_semantics
    visible_structure
    visible_evidence
    visible_history

    hidden_fields

    authority
}
```

---

## Reification

Reification constructs semantic artifacts from values/descriptions available at the current stage.

It must not invent identities or evidence.

Example:

```text
reify(schema)
    -> SerializerDescription
```

with explicit provenance from `schema`.

---

## Reflection is not proof

Inspecting an artifact does not prove its claims.

```text
Reflect<ProofClaim<P>>
```

returns a representation of the claim/evidence relationship.

It does not establish `P` unless the relevant checker verifies the proof evidence.

---

# Mutation and promotion authority

## Capability separation

The following should be distinguishable:

```text
Reflect<A>
ProposeTransform<A>
CreateCandidate<A>
ApplyTransform<Candidate<A>>
Verify<Candidate<A>>
Compare<Candidate<A>, Baseline<A>>
Promote<Candidate<A>>
```

No capability implies the next automatically.

---

## Candidate isolation

A transformation should produce:

```text
CandidateArtifact {
    parent_identity
    candidate_identity

    proposed_changes
    transformation_identity

    obligations
    evidence
}
```

The trusted baseline remains unchanged.

---

## Promotion

Promotion is an explicit governance transition:

```text
PromotionDecision {
    baseline
    candidate

    protected_properties
    comparison

    required_evidence
    supplied_evidence

    authority

    decision:
        Accept
      | Reject
      | Defer
}
```

This reuses RFC 0004 rather than creating an unrestricted self-modification path.

---

# Binding-time semantics

## Binding-time classes

A research representation may include:

```text
KnownNow
KnownAt<Stage>
DynamicUntil<Stage>
ExternalUntil<Observation>
Unknown
```

The exact taxonomy remains open.

---

## Knowledge does not imply authority

Knowing a value at an earlier stage does not imply authority to use it in all ways.

For example:

```text
KnownAt<Build>(SecretKey)
```

may be forbidden from persistence into the runtime binary.

A binding-time analysis must therefore incorporate information-flow and authority/persistence rules.

---

## Observation-bound values

Some values are known only after an external observation:

```text
KnownAfter<Observe<Topology>>
```

Moving computation earlier would require replacing the observation with another valid information source.

---

# Stage holes

## Motivation

Not every stage choice should be fixed by source.

An operation may be legal at several stages.

Example:

```text
BuildLookupTable()
```

could occur at:

```text
Build
Link
Deploy
Startup
Run
```

with different performance, binary-size, update, and proof consequences.

---

## Canonical stage hole

Conceptually:

```text
StageHole {
    identity
    operation

    earliest_stage
    latest_stage

    dependencies

    requirements
    forbidden_stages

    objectives

    authority_constraints
    effect_constraints

    evidence_policy

    budget
}
```

Forge may resolve this hole subject to semantics.

---

## Example objective

```text
stage intent {
    require {
        semantic_equivalence
        no_secret_persistence
    }

    prefer {
        minimize_runtime_latency
        minimize_binary_size
        minimize_build_cost
    }
}
```

The result is a realization choice, not a change in logical program meaning.

---

# Specialization

## Specialization artifact

```text
Specialization {
    identity

    source_artifact

    source_stage
    target_stage

    known_facts
    known_values
    selected_evidence

    transformation

    residual_artifact

    assumptions
    obligations

    refinement_claim
    refinement_evidence
}
```

---

## Representation specialization

RFC 0013 may leave representation as a realization hole.

RFC 0016 allows later stages to resolve it when enough facts are known.

Example:

```text
Representation<T> = ? @ Build
```

becomes:

```text
Representation<T> = SoA @ Deploy
```

under target/workload evidence.

---

## Component specialization

RFC 0014 component/provider holes may also be resolved at later stages.

For example:

```text
StorageProvider = ? @ Build
StorageProvider = NVMeProvider @ Deploy
```

provided the binding/refinement contract is satisfied.

---

## Runtime specialization

Runtime/JIT specialization is permitted only under the same rules.

It is not semantically privileged.

The runtime specializer must possess:

- reflection authority over its input artifact;
- candidate-generation authority;
- any execution/memory authority required for code generation;
- evidence-generation/checking capability; and
- promotion/installation authority if it will activate the new artifact.

---

# Semantic hygiene

## Lexical hygiene is insufficient

A generated artifact may accidentally capture much more than a variable name.

Examples:

```text
AuthorityCapture
EvidenceCapture
RegionCapture
SessionCapture
TypeIdentityCapture
GenerativeDomainCapture
AssumptionCapture
StageFactCapture
```

These are semantic errors even if all identifiers are uniquely renamed.

---

## Hygienic insertion rule

Conceptually:

```text
Γ_target
⊢
substitution : Context<C_source> -> Γ_target
```

must prove:

- every free semantic variable is intentionally bound;
- no undeclared authority is captured;
- no stale evidence is captured;
- no region/provenance identity is forged;
- no protocol/session identity is aliased illegally;
- no generative identity is collapsed accidentally; and
- stage validity is preserved.

---

## Explicit capture

Intentional capture should be possible only through explicit context binding.

For example:

```text
bind artifact.context.read_r
    <- Read<RegionR>#cap42
```

The resulting artifact records that capability identity/dependency where appropriate.

---

# Artifact provenance

## Generated artifact lineage

Every generated artifact should be attributable to its generator and semantic inputs.

```text
GeneratedArtifactProvenance {
    artifact_identity

    generator_identity
    generator_artifact_identity

    source_artifacts
    input_facts
    input_evidence

    generation_stage
    target_stage

    transformation_identity

    inherited_evidence
    generated_obligations

    assumptions
}
```

---

## Generator change invalidation

If a generator changes, artifacts that depend on its semantics become stale.

If only unrelated generator code changes, narrow dependency tracking may preserve unaffected generated evidence.

---

## Fact change invalidation

If a generated serializer depends on:

```text
SchemaIdentity#17
```

then changing unrelated application code should not invalidate it.

Changing `SchemaIdentity#17` should.

This makes staged dependency graphs a basis for semantic incremental compilation.

---

# Transformation graph

## Generalized transformation relationships

The system should distinguish:

```text
Equivalent<A,B>
Refines<A,B>
ApproximatesWithin<A,B,Envelope>
Unrelated<A,B>
UnknownRelation<A,B>
```

These should not collapse into a single "optimized version" label.

---

## Candidate families

A transformation search may retain multiple candidates:

```text
CandidateFamily {
    baseline
    candidates

    relation_evidence

    objectives
    protected_properties
}
```

Forge may compare candidates before promotion.

---

## Equality-saturation realization

An e-graph engine may implement a subset of `Equivalent` candidate search.

The e-graph is not the normative semantic model.

A rewrite that only establishes refinement must not be inserted into an equivalence class as if equality had been proven.

---

# Self-description

## Self-describing semantic artifacts

MNCS should eventually expose versioned descriptions such as:

```text
SemanticRuleArtifact
TypeRuleArtifact
EffectRuleArtifact
VerifierContractArtifact
ObligationTemplateArtifact
IRSchemaArtifact
LoweringRuleArtifact
```

These descriptions may be incomplete during early research.

Their existence does not imply full self-hosting.

---

## Introspection queries

A machine may ask:

```text
describe(Operation<Load>)
describe(Verifier<CapabilityConsistency>)
describe(SemanticRule<FunctionApply>)
describe(HIRInstruction<Branch>)
```

The returned artifact should have versioned identity and evidence provenance.

---

## Self-description trust

A self-description is itself an artifact.

If the implementation and description disagree, the disagreement must be detectable through conformance experiments or proof obligations.

The description is not authoritative merely because the compiler emits it.

---

# Self-hosting

## Self-hosted property

A tool is self-hosted when a version of the toolchain sufficient to construct it is implemented in MNCS and can build a successor under the defined bootstrap protocol.

This is separate from self-description.

---

## Bootstrap boundary

Conceptually:

```text
BootstrapBoundary {
    bootstrap_toolchain_identity
    bootstrap_checker_identity
    bootstrap_semantics_identity

    trusted_artifacts
    assumptions

    first_self_hosted_artifact

    cross_check_artifacts
    evidence
}
```

---

## Bootstrap lineage

A self-hosted compiler lineage should retain:

```text
BootstrapCompiler
    -> Compiler1
    -> Compiler2
    -> Compiler3
```

with transformation/build identities and evidence for each edge.

---

## Diverse double compilation / cross-check direction

Future experiments may compare artifacts built through independent bootstraps or implementations.

Such evidence can reduce trust in one implementation but does not remove all trust assumptions automatically.

---

# Recursive self-transformation

## Candidate-first cycle

The canonical cycle is:

```text
CurrentArtifact
       ↓ Reflect
ReflectionView
       ↓ Propose
TransformationProposal
       ↓ Apply in isolated candidate domain
CandidateArtifact
       ↓ Verify
Evidence
       ↓ Compare
Comparison
       ↓ Promotion authority
PromotionDecision
       ↓
NewCurrentArtifact
```

At no point does reflection imply direct mutation of the trusted baseline.

---

## Transformation lineage

```text
TransformationLineage {
    parent
    candidate
    accepted_as?

    transformation
    input_context

    evidence
    comparison
    promotion_decision
}
```

This becomes useful for compiler evolution, Forge optimization, and later MNEL/Lineage systems.

---

## Recursive authority

A compiler may be permitted to:

```text
Reflect<Self>
ProposeTransform<Self>
```

without:

```text
Promote<Self>
```

An external promotion authority may remain required.

---

## Recursive proof

A verifier may generate or improve its own untrusted proof-search component while retaining a separately versioned small proof checker.

RFC 0007 remains the proof trust boundary.

---

# Compile-time and generation-time security

## Same authority model

Compile-time code is ordinary execution.

A generator that attempts:

```text
Network<Egress>
```

must possess that authority at its actual execution stage.

---

## Sandboxed generators

A safe default may execute generators in a restricted boundary such as:

```text
GeneratorSandbox {
    Read<DeclaredInputs>
    Write<CandidateOutputs>
    no Network
    no RepositoryMutation
    bounded CPU
    bounded Memory
}
```

This is a realization/policy choice.

---

## Generator failure

Generator failure uses RFC 0011 semantics.

A timeout during generation is not proof that an external build effect did not occur.

---

## Generator nondeterminism

A generator using:

```text
Clock
Entropy
Network
Environment
```

introduces explicit nondeterministic observations.

Those observations affect reproducibility and artifact identity/evidence.

---

# Foreign staged execution

## Foreign tools remain foreign

Invoking Clang, a linker, a schema generator, a Python script, or another external tool during a build stage is an RFC 0015 foreign operation.

Example:

```text
ForeignOperation<Clang, Stage<Build>>
```

The tool does not become trusted simply because it participates in compilation.

---

## Foreign artifact provenance

Outputs of foreign tools should record:

```text
foreign_tool_identity
input_artifact_identities
ABI/toolchain identity
arguments/configuration
environment assumptions
output identity
boundary evidence
```

where relevant.

---

## Re-entry into staged semantic artifacts

Foreign-generated output becomes an MNCS semantic artifact only through a boundary adapter that establishes the relevant syntax/IR/schema validity claims.

A generated binary cannot claim semantic correspondence solely because the foreign compiler produced it.

---

# Interaction with RFC 0012 executable semantics

## Stage operations are semantic operations

The executable core should eventually be able to represent semantic requests such as:

```text
ReflectArtifact<A>
ConstructArtifact<A>
InstantiateArtifact<A>
SpecializeArtifact<A>
VerifyArtifact<A>
PromoteArtifact<A>
```

subject to authority.

---

## Interpretation vs specialization

RFC 0012's reference evaluator executes semantic artifacts.

RFC 0016 permits specialization of that evaluator against known programs/facts as a research transformation.

These are distinct activities.

---

## Behavior projection

Specialized artifacts must preserve the relevant RFC 0012 observable trace/refinement contract.

Internal reduction shape may differ arbitrarily when legal.

---

# Interaction with RFC 0013 abstraction

## Polymorphic staged artifacts

An artifact may remain polymorphic across stages:

```text
forall T.
    StagedArtifact<Algorithm<T>>
```

or a stage may resolve some but not all abstraction variables.

---

## Evidence dictionaries across stages

RFC 0013 evidence identities must obey cross-stage persistence.

An implicit dictionary available at generation time does not automatically exist at runtime.

The system may:

- embed it;
- reconstruct it;
- re-resolve it;
- re-prove it; or
- leave a runtime requirement.

Each choice has different semantics and identity.

---

## Realization holes

RFC 0013 `RealizationHole` objects may be solved at a later stage.

Stage choice and realization choice are separate dimensions.

---

# Interaction with RFC 0014 composition

## Component holes at stages

An unresolved provider requirement may remain open through build and be bound at deployment:

```text
Component<Open<StorageProvider>> @ Build
```

becomes:

```text
Unit<StorageProvider = NVMeStore> @ Deploy
```

with ordinary compatibility evidence.

---

## Dynamic binding

Runtime component selection is a stage-late binding operation.

It must verify the provider contract before authority is granted.

---

## Package/build closure

A package can record staged build/generation dependencies separately from runtime dependencies.

This prevents generator-only privileges from becoming runtime dependency privileges.

---

# Interaction with RFC 0015 trust boundaries

## Earlier stages do not bypass trust boundaries

Foreign build tools, plugins, generators, JIT engines, and code emitters remain trust-boundary operations.

---

## Unsafe staging

An unsafe transformation is an evidence-regime transition exactly as in RFC 0015.

Staging does not create a second unsafe model.

---

## Generated foreign boundaries

A generator may construct a future `TrustBoundary` artifact.

The generator does not thereby gain the authority represented by that future boundary.

---

# Canonical artifacts

RFC 0016 proposes research schemas for at least the following semantic objects.

## `StageArtifact`

```text
StageArtifact {
    identity
    graph_identity

    predecessors
    successors

    environment

    facts
    evidence

    authority
    effects

    target_knowledge

    persistence_policy
}
```

---

## `StagedArtifact`

```text
StagedArtifact {
    identity

    semantic_subject
    semantic_type

    context

    produced_at
    executable_at

    dependencies

    authority_requirements
    effects_by_stage

    assumptions
    obligations
    evidence

    provenance
}
```

---

## `CrossStagePersistence`

```text
CrossStagePersistence {
    subject

    from_stage
    to_stage

    preserved_claims
    invalidated_claims
    reconstructed_claims

    obligations
    evidence
}
```

---

## `BindingTimeFact`

```text
BindingTimeFact {
    subject

    availability
    validity_scope

    source
    observation_dependencies

    evidence
}
```

---

## `StageHole`

```text
StageHole {
    identity
    subject

    allowed_stages

    requirements
    objectives
    forbidden_transitions

    effect_constraints
    authority_constraints

    budget
    evidence_policy
}
```

---

## `ReflectionView`

```text
ReflectionView {
    identity
    subject

    visible_semantics
    visible_structure
    visible_evidence
    visible_history

    omitted_information

    authority
}
```

---

## `TransformationArtifact`

```text
TransformationArtifact {
    identity

    input_artifacts
    output_artifacts

    source_stage
    target_stage

    transformation_kind

    context_substitutions

    assumptions
    obligations

    preservation_claim
    evidence

    provenance
}
```

---

## `GeneratedArtifactProvenance`

```text
GeneratedArtifactProvenance {
    artifact

    generator
    source_artifacts

    input_facts
    input_evidence

    generation_stage
    target_stage

    transformation

    obligations
    assumptions
}
```

---

## `BootstrapBoundary`

```text
BootstrapBoundary {
    bootstrap_toolchain
    bootstrap_checker
    bootstrap_semantics

    trust_assumptions

    self_hosted_successors

    cross_checks
    evidence
}
```

---

## `TransformationLineage`

```text
TransformationLineage {
    parent
    candidate
    successor?

    proposal
    transformation

    verification
    comparison
    promotion
}
```

---

# Formal judgment direction

The final calculus remains open, but RFC 0016 should support judgments conceptually equivalent to the following.

## Stage typing

```text
Γ ; S ⊢ e : T
```

Expression `e` is valid under context `Γ` at stage `S`.

---

## Artifact construction

```text
Γ ; S0 ⊢ construct(e, S1) : Artifact<T, C, S1>
```

where the construction records context `C` rather than hiding free dependencies.

---

## Context insertion

```text
Γ_target ⊢ σ : C_source -> Γ_target
```

and:

```text
Artifact<T,C_source,S> + σ
    -> Artifact<T,C_target,S>
```

subject to semantic hygiene obligations.

---

## Cross-stage persistence

```text
Γ ⊢ Persist(x, S0 -> S1) : PASS | FAIL | UNKNOWN
```

with evidence explaining which claims persist.

---

## Specialization

```text
Γ ; Facts ⊢ specialize(A) => A'
```

and:

```text
Γ ⊢ A' ⊑ A
```

or stronger equivalence as required.

---

## Reflection

```text
Reflect<A> ; Γ ⊢ reflect(A) : ReflectionView<A>
```

---

## Candidate creation

```text
ProposeTransform<A> ; Γ
⊢
proposal(A) : CandidateProposal<A>
```

---

## Promotion

```text
Promote<A>
;
Verified<Candidate<A>>
;
ComparisonEvidence
⊢
promote(Candidate<A>)
```

No weaker authority should suffice.

---

# Proof obligations

Potential generated obligations include:

```text
StageAvailabilityValid
CrossStagePersistenceValid
NoSemanticCapture
AuthorityNotAmplified
GeneratorAuthoritySeparated
EffectStagePreserved
ObservationSemanticsPreserved
SpecializationRefinesSource
EvidenceDependenciesFresh
RepresentationPersistenceValid
NoSecretPersistence
CandidateIsolationPreserved
PromotionAuthorityValid
BootstrapBoundaryRecorded
ForeignGeneratorBoundaryValid
```

---

## Stage motion proof

Given operation `O` moved from `S1` to `S0`, prove:

```text
inputs_available_at(S0)
&& authority_available_at(S0)
&& effect_behavior_preserved
&& observation_behavior_preserved
&& failure_behavior_preserved
&& required_output_facts_persist_to(S1)
```

---

## Persistence proof

For fact `F`:

```text
Valid(F, S0)
+
PersistenceRule(F, S0, S1)
+
FreshDependencies(F, S1)
⊢
Valid(F, S1)
```

---

## Semantic hygiene proof

```text
ContextSubstitution σ
+
AllowedCaptureSet
⊢
NoUnauthorizedSemanticCapture
```

---

## Self-transformation proof

```text
CandidateRefinesBaseline
+
ProtectedPropertiesPreserved
+
PromotionPolicySatisfied
+
PromotionAuthority
⊢
CandidateMayReplaceBaseline
```

---

# Trusted checking boundary

The same RFC 0007 philosophy applies.

```text
staging / specialization problem
        ↓
partial evaluator / solver / Forge / LLM / e-graph
        ↓
proof terms / certificates / analysis results
        ↓
small independent checker(s)
```

The specializer is not trusted merely because it produced an optimized artifact.

The transformation engine may be enormous and untrusted.

Promotion depends on checkable evidence wherever the active assurance profile requires it.

---

# HIR / SSA requirements

HIR and SSA should preserve enough information to reconstruct relevant staging semantics.

At minimum, supported subsets should preserve:

- staged artifact identity;
- source/generator identity;
- stage identity;
- future-stage effect identity;
- generator-stage effect identity;
- authority requirements by stage;
- context substitutions;
- specialization identity;
- binding-time fact identities;
- assumptions;
- generated obligations;
- refinement/equivalence evidence references; and
- transformation provenance.

A lowering must not collapse:

```text
Generate<NetworkEffectAtRun>
```

into:

```text
NetworkEffectAtBuild
```

or vice versa.

---

# Diagnostics

Diagnostics should explain stage reasoning semantically.

Example:

```text
specialization rejected

operation:
    FetchConfiguration

requested move:
    Run -> Build

FAIL:
    required observation not available at Build

observation:
    RuntimeEnvironment.Config

available repairs:
    - retain operation at Run
    - provide a Build-stage configuration input with explicit equivalence contract
    - move only pure downstream computation
```

---

Another:

```text
cross-stage evidence invalid

evidence:
    Aligned<Buffer,64>#E22

produced at:
    Load

requested use at:
    Run

reason:
    Buffer allocation identity changed

status:
    FAIL
```

---

Semantic capture diagnostic:

```text
generated artifact insertion rejected

artifact:
    KernelCandidate#91

captured authority:
    Write<CompilerRepository>

artifact context declares:
    no repository authority

repair:
    pass an explicit bounded capability if semantically intended
```

---

Promotion diagnostic:

```text
self-transformation candidate cannot be promoted

candidate:
    Compiler#C17

verification:
    PASS

comparison:
    PASS

missing:
    Promote<Compiler> authority

current agent capabilities:
    Reflect<Compiler>
    ProposeTransform<Compiler>
    ApplyTransform<Candidate<Compiler>>
```

---

# Forge integration

Forge is a natural orchestration environment for RFC 0016.

Potential Forge search dimensions include:

- stage placement;
- partial-evaluation scope;
- specialization depth;
- representation resolution stage;
- component-binding stage;
- inlining stage;
- code generation strategy;
- JIT vs AOT choice;
- artifact caching;
- equality-saturation search;
- candidate extraction;
- proof strategy;
- verifier selection;
- bootstrap cross-check strategy; and
- generator containment strategy.

Forge may optimize within:

```text
LegalStageTransformations
=
    transformations satisfying semantic obligations
```

not by treating stage motion as unrestricted optimization.

---

## Stage tournament

Forge may compare:

```text
Build-stage specialization
Deploy-stage specialization
Startup specialization
Runtime specialization
```

under objectives such as:

```text
startup_latency
steady_state_latency
binary_size
memory
energy
build_time
proof_cost
update_freshness
secret_exposure
```

---

## Proof cost as an objective

A transformation that is slightly faster at runtime but dramatically increases proof/checking cost may be rejected under an intent profile.

This is a first-class machine-native tradeoff.

---

# Fabric / MNEL integration direction

RFC 0016 can support later machine-generated workflows in Fabric/MNEL.

For example, an agent may receive:

```text
Reflect<ExperimentHarness>
ProposeTransform<ExperimentHarness>
```

and generate:

```text
Candidate<ExperimentHarness>
```

without promotion authority.

Another verifier/decision process can evaluate and promote.

This mirrors the same semantic rules used for compiler self-transformation.

The language need not special-case AI-generated artifacts.

---

# Security consequences

## Compile-time supply-chain attacks

A build generator can be malicious.

RFC 0016 makes its authority explicit.

A generator dependency should not gain ambient:

```text
HomeDirectory
SSHKeys
Network
RepositoryWrite
```

merely because it executes during compilation.

---

## Reflection exfiltration

A component with reflection access might expose:

- source semantics;
- proof artifacts;
- secrets embedded in compile-time state;
- compiler configuration; or
- proprietary transformation history.

Reflection therefore requires explicit visibility scope.

---

## Candidate poisoning

A malicious transformation engine may generate an unsafe candidate.

Candidate isolation and independent checking limit the blast radius.

---

## Promotion attacks

The ability to generate a valid candidate is intentionally insufficient to install it.

Promotion authority is a separate capability.

---

## Stage confusion

Treating a future effect as a current effect, or vice versa, can produce major security errors.

Stage identity therefore participates in effect and authority checking.

---

## Secret persistence

A secret available at build stage must not accidentally be embedded in a generated artifact.

Potential obligation:

```text
NoPersistence<Secret, RunArtifact>
```

should be checkable through information-flow or explicit policy mechanisms later.

---

# Resource bounds

Recursive metaprogramming must be bounded.

Potential budget object:

```text
TransformationBudget {
    max_stage_depth
    max_candidates
    max_rewrites
    max_specialization_steps
    max_solver_calls
    max_verifier_calls
    max_generated_artifacts
    max_execution_steps
    max_memory
    max_proof_cost
}
```

Exhaustion produces an explicit bounded outcome, not proof of impossibility.

---

## Budget exhaustion

```text
BudgetExhausted
!=
NoValidTransformationExists
```

This continues RFC 0011's knowledge discipline.

---

# Portability

A semantic staging contract should not depend on one compiler architecture.

For example:

```text
require {
    generated_artifact_semantics_preserved
    no_secret_persistence
}

prefer {
    specialize_before_run
}

allow_realization {
    AOT
    deployment_specialization
    JIT
}
```

Different targets may realize stage boundaries differently.

---

# First implementation direction

The first executable subset should remain deliberately small.

Recommended minimum:

1. two or three explicit stages;
2. typed closed and open arithmetic artifacts;
3. explicit context substitution;
4. stage-indexed pure vs effectful operations;
5. one cross-stage persistence rule;
6. one rejected persistence rule;
7. one stage-scoped capability distinction;
8. one bounded specialization transformation;
9. one independent refinement check against RFC 0012 execution;
10. candidate isolation and explicit promotion records; and
11. deterministic artifact serialization/fingerprints.

Do not begin with unrestricted macro syntax.

---

# Experimental tracks

## Experiment 1 — Typed open-artifact experiment

Construct:

```text
Artifact<
    x + y,
    Context<{x:Int, y:Int}>,
    Stage<Run>
>
```

Then instantiate it in two legal contexts.

Attempt illegal context capture.

Verify:

- correct substitutions pass;
- missing variables fail;
- wrong types fail;
- identity capture is detected; and
- unauthorized authority capture fails.

---

## Experiment 2 — Cross-stage evidence experiment

Produce several facts at `Build`:

```text
PureValue<42>
Proof<TableSorted>
MachineAddress<Buffer>
Alignment<RuntimeBuffer>
Read<Repository>
```

Attempt to persist each to `Run`.

Expected:

- pure value may persist;
- proof persists only if its dependencies persist;
- machine address does not persist by default;
- runtime allocation alignment does not persist before allocation;
- repository authority does not persist automatically.

---

## Experiment 3 — Stage-authority separation

Generator authority:

```text
Read<Schema>
Write<CandidateOutput>
```

Generated program authority:

```text
Network<ServiceAPI>
```

Verify:

- generator cannot use `Network<ServiceAPI>`;
- generated program cannot use `Read<Schema>` merely because the generator had it;
- generated artifact may declare the future network requirement without possessing it during generation.

---

## Experiment 4 — Semantic hygiene attack suite

Attempt deliberate capture of:

```text
lexical variable
type identity
generative domain identity
region
provenance
protocol session
evidence
assumption
authority capability
stage-local fact
```

Verify only explicitly bound captures succeed.

---

## Experiment 5 — Binding-time / partial-evaluation experiment

Program:

```text
f(static_config, dynamic_request)
```

Specialize `static_config` at Build.

Leave `dynamic_request` at Run.

Compare residual artifact with reference interpreter over a bounded corpus.

Require structural/refinement evidence beyond differential testing before stronger claims.

---

## Experiment 6 — Effectful staging experiment

Start with:

```text
read_clock(); compute()
```

Attempt to move `read_clock()` to Build.

Reject because observation semantics change.

Then move only pure downstream computation that depends on a build-known constant.

Accept with evidence.

---

## Experiment 7 — Stage-hole tournament

Create one operation legal at:

```text
Build
Deploy
Startup
Run
```

Forge evaluates candidates under:

```text
runtime_latency
binary_size
build_cost
freshness
proof_cost
```

Select only among semantically legal placements.

---

## Experiment 8 — Verified reflection experiment

Expose typed reflection over one compiler semantic-rule artifact.

Agent has:

```text
Reflect<SemanticRules>
```

but no mutation authority.

Verify it can answer structural/semantic questions but cannot alter the trusted rule set.

---

## Experiment 9 — Self-transformation experiment

Compiler or verifier component has:

```text
Reflect<Self>
ProposeTransform<Self>
ApplyTransform<Candidate<Self>>
```

but not:

```text
Promote<Self>
```

Generate an improved candidate.

Verify independently.

Confirm the candidate cannot become current without external promotion authority.

---

## Experiment 10 — Equality-search experiment

Create several algebraically equivalent arithmetic kernels.

Represent equivalence edges with evidence.

Use an e-graph or another candidate-retention engine.

Select the lowest-cost candidate under machine intent.

Verify candidates connected only by refinement are not incorrectly merged as equality.

---

## Experiment 11 — Bootstrap-lineage experiment

Build:

```text
BootstrapCompiler -> Compiler1 -> Compiler2
```

Persist:

- compiler identities;
- source semantic identities;
- build stages;
- proof/checker identities;
- assumptions;
- resulting artifact hashes; and
- comparison evidence.

Show that Compiler2 being self-hosted does not remove the BootstrapCompiler assumption from history.

---

## Experiment 12 — Incremental staged invalidation

Generate:

```text
Schema#17
   -> Generator#9
   -> Serializer#42
   -> Evidence#91
```

Change unrelated source.

Confirm serializer evidence remains fresh.

Change `Schema#17`.

Confirm only dependent generated artifacts/evidence become stale.

---

## Experiment 13 — Bounded Futamura / semantic-machine specialization

This is the flagship experiment.

Take:

```text
RFC0012SemanticMachine
```

and a tiny supported program:

```text
Program<P>
```

Perform:

```text
specialize(
    RFC0012SemanticMachine,
    Program<P>
)
    -> Artifact<P'>
```

Require:

```text
Behavior(P') ⊑ Behavior(P)
```

for the supported subset.

Compare:

- direct semantic-machine interpretation;
- specialized artifact;
- HIR execution; and
- SSA execution.

Then, only after the first experiment is sound, investigate a bounded second projection:

```text
specialize(
    Specializer,
    RFC0012SemanticMachine
)
    -> CompilerSubset
```

The success criterion is not performance.

It is semantic alignment: can executable realization be mechanically derived from the language's own semantic machine without inventing another meaning?

---

## Experiment 14 — Foreign build-tool boundary experiment

Run a foreign generator at Build through RFC 0015.

Record:

```text
tool identity
inputs
stage authority
foreign assumptions
output identity
boundary evidence
```

Attempt undeclared network access and reject/contain it.

Import the output only through a semantic validation adapter.

---

# Central experiment recommendation

The **bounded Futamura / semantic-machine specialization experiment** should be RFC 0016's defining experiment.

RFC 0009's relocating-reference interpreter demonstrated that semantic memory identity must survive movement.

RFC 0011's ambiguous-commit experiment demonstrated that local control failure cannot be collapsed into external semantic failure.

RFC 0016 should demonstrate:

```text
semantic definition
+
program
+
verified specialization
!=
new independent language meaning
```

Instead:

```text
semantic definition
+
program
+
specialization
    -> verified realization
```

If even a tiny subset succeeds, it materially validates the architecture chosen in RFC 0012.

---

# Negative tests

RFC 0016 should reject or conservatively flag at least the following.

1. Generated artifact captures undeclared authority.
2. Generated artifact captures stale proof evidence.
3. Compile-time generator performs undeclared network I/O.
4. Generator receives generated program's future runtime capability automatically.
5. Runtime artifact inherits generator repository-write authority.
6. Machine address persists across stages without evidence.
7. Runtime allocation fact is assumed at build stage.
8. Stage motion moves an external observation earlier without equivalent source.
9. Stage motion reorders visible effects illegally.
10. Timeout in a generator is treated as proof no external effect occurred.
11. Generated artifact silently captures a protocol session.
12. Generated artifact collapses distinct generative type identities.
13. Reflection authority is used to mutate baseline directly.
14. Candidate-generation authority is treated as promotion authority.
15. Self-hosting is reported as proof of bootstrap compiler correctness.
16. Equality saturation merges a refinement-only transformation as equality.
17. `UNKNOWN` stage legality is treated as permission to move earlier.
18. Binding-time analysis embeds a secret into a later artifact contrary to policy.
19. Stage budget exhaustion is reported as proof no legal specialization exists.
20. Foreign build tool output is treated as verified source correspondence without boundary evidence.
21. Implicit evidence dictionary is assumed to exist at runtime because it existed at build.
22. Generated code performs a future effect at generation time due to phase confusion.
23. Reified artifact invents provenance identity.
24. Cross-stage persistence retains invalid target-specific representation evidence.
25. Stage-hole solver chooses a forbidden stage because it is faster.
26. Candidate modifies trusted baseline before promotion.
27. Recursive metaprogram exceeds stage-depth budget and continues.
28. Stale generator identity does not invalidate dependent generated evidence.
29. Open artifact executes without all required context bindings.
30. Context substitution binds a weaker capability than the artifact contract requires and proceeds.
31. Runtime JIT installs code without install/promotion authority.
32. Self-description artifact is treated as authoritative without checking correspondence to implementation.
33. Build-time environment variable becomes hidden semantic input without observation provenance.
34. Nondeterministic generator produces artifacts labeled reproducible without observation ledger/evidence.
35. Foreign compiler version changes but prior boundary evidence remains fresh.

---

# Success criteria

A prototype meaningfully validates RFC 0016 if it can demonstrate:

1. explicit stage identities and relationships;
2. typed open and closed semantic artifacts;
3. explicit context substitution;
4. semantic hygiene beyond identifier hygiene;
5. stage-indexed effect distinction;
6. generator/generated authority separation;
7. explicit cross-stage persistence decisions;
8. proof/evidence persistence tied to dependency freshness;
9. conservative binding-time analysis;
10. specialization with checkable refinement evidence;
11. stage holes solved only inside legal envelopes;
12. reflection without mutation authority;
13. candidate-first self-transformation;
14. explicit promotion authority;
15. bootstrap lineage that remains visible through self-hosting;
16. incremental invalidation of generated artifacts;
17. foreign build tools represented through RFC 0015 boundaries;
18. bounded recursive meta-level execution;
19. at least one machine-readable self-description artifact; and
20. a bounded RFC 0012 semantic-machine specialization experiment whose residual artifact matches/refines the reference semantics for the supported subset.

---

# Boundary to later RFCs

RFC 0016 intentionally defers several adjacent areas.

## Runtime/environment contract

A later RFC should define which runtime services a program or staged artifact requires and how targets satisfy those requirements.

## Full distributed staging

Remote generation, distributed proof search, deployment-wide staged consistency, and distributed live upgrades deserve separate treatment.

## Live replacement / upgrade semantics

Self-transformation here ends at candidate promotion semantics.

Replacing a live executing component while preserving state/protocols requires a later deployment/upgrade RFC.

## Information-flow / secrecy calculus

RFC 0016 identifies secret persistence as an obligation, but a complete confidentiality/noninterference system may deserve dedicated treatment.

## Full build/package realization

RFC 0014 defines package/build identities conceptually. RFC 0016 adds staged provenance. A complete package/build executor can be specified later.

---

# Open questions

1. Should stages form a total order, partial order, or a small category-like structure in the normative core?
2. Should stage identity be a type index, an effect/coeffect dimension, or explicit semantic metadata?
3. Which cross-stage persistence rules belong in the kernel versus external verifiers?
4. Should `Artifact<T,C,S>` be primitive or elaborated into a smaller modal core?
5. How should dependent types interact with open staged contexts?
6. How should proof terms themselves be quoted/reified across stages?
7. Which evidence can safely be erased after specialization?
8. How should stage-indexed authority appear in source-facing representations?
9. Can stage-hole search be unified directly with RFC 0013 realization-hole search?
10. Should component binding stage be represented as a specialization or a separate binding transformation?
11. How should JIT-generated machine code obtain executable-memory authority safely?
12. How should W^X or capability-machine constraints appear in the runtime realization?
13. How much of semantic hygiene should be checked structurally versus by proof obligations?
14. Can information-flow evidence prevent accidental secret persistence without a full security-label system?
15. Should generator observation ledgers be mandatory for reproducible artifacts?
16. How should deterministic and nondeterministic generators affect artifact identity?
17. What exact equivalence notion should the first partial evaluator prove?
18. Can the Futamura experiment reuse RFC 0012 simulation/refinement certificates directly?
19. Should the specializer be written in MNCS before self-hosting begins?
20. How should bootstrap trust be reduced through independent implementations?
21. Which compiler semantic rules should become self-describing first?
22. Can semantic-rule artifacts themselves be transformed safely?
23. How are verifier contracts versioned when they verify transformations of themselves?
24. Should promotion authority ever be delegable to an autonomous agent under bounded policy?
25. How should recursive transformation budgets compose across nested agents?
26. Should equality-saturation evidence be per rewrite edge, extracted proof, or both?
27. How should approximate transformations interact with stage placement?
28. When does changing stage alter artifact semantic identity versus realization identity only?
29. Should deployment-stage observations be replayable for reproducible deployment specialization?
30. How should stage-local failures interact with later recovery semantics?
31. Can a build-stage crash leave externally committed effects requiring reconciliation before retry?
32. How should staged foreign callbacks be represented?
33. How much staging metadata should survive into production artifacts?
34. Should runtime reflection be supported by default or require explicit retained metadata?
35. Can self-description be complete enough to generate independent checkers without circular trust?

---

# References and influences

This RFC is informed by, but does not normatively adopt wholesale:

- Davies and Pfenning's modal staged-computation type systems;
- contextual modal type theory;
- multi-level contextual type theory;
- MetaML and MetaOCaml typed multi-stage programming;
- phase-distinction calculi and staged compile-time evaluation systems;
- binding-time analysis and partial evaluation;
- Futamura projections and compiler-generation research;
- semantics-preserving staging and let-insertion work;
- hygienic macro systems;
- verified metaprogramming and reflective proof-assistant work including MetaCoq;
- equality saturation and e-graph systems such as `egg`;
- proof-carrying transformation research;
- self-hosting/bootstrap verification techniques; and
- the preceding MNCS RFCs.

These systems are theoretical and engineering influences, not dependencies or required surface designs.

---

# Final direction

MNCS should not define metaprogramming as a privileged compiler subsystem.

It should define a semantic world in which **programs, generated programs, compiler rules, proof artifacts, component bindings, specialization choices, and candidate successors are all inspectable versioned artifacts with explicit stage, context, authority, evidence, and provenance**.

The language should therefore be able to represent:

```text
artifact
    ↓ reflect
semantic view
    ↓ transform
candidate
    ↓ verify
refinement evidence
    ↓ compare
promotion decision
    ↓
successor artifact
```

without ever implying:

```text
reflection == mutation
mutation == trust
candidate == current
self-hosted == self-proven
```

The intended machine-native rule is:

> **Model when information exists, what context an artifact requires, what authority each stage possesses, what transformations preserve semantics, and what evidence authorizes promotion. Let machines search the legal transformation space without hiding the lineage.**

And the strongest research test is:

> **If the executable semantic machine can be partially evaluated against an MNCS program into a verified residual realization, then compilation begins to look like a mechanically justified transformation of the language's own semantics rather than a second independent definition of the language.**

That is the direction RFC 0016 establishes.
