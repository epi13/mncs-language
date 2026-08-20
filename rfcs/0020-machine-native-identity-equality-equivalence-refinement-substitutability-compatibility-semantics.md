# RFC 0020: Machine-Native Identity, Equality, Equivalence, Refinement, Substitutability, and Compatibility Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.2–1.0
- **Depends on:** RFC 0001, RFC 0002, RFC 0003, RFC 0004, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0013, RFC 0014, RFC 0015, RFC 0016, RFC 0017, RFC 0018, RFC 0019

## Summary

MNCS Language should not have one universal notion of “the same.” Identity, definitional equality, proof-bearing equality, structural equivalence, observational equivalence, contextual equivalence, bisimulation, trace equivalence, refinement, contextual refinement, representation equivalence, interface substitutability, compatibility, and sound abstraction are different semantic relations with different laws, scopes, directions, observations, witnesses, and consequences.

The central rule is:

> **Every claim that two machine objects are identical, equal, equivalent, substitutable, compatible, or related by refinement must name the relation being claimed and must preserve the relation's observation, context, direction, environment, assumption, and evidence boundaries.**

A second rule is equally important:

> **MNCS has no universal `same_as` relation. A relation that is valid for one purpose must not silently authorize substitution, rewriting, optimization, representation change, lowering, linking, or promotion under another purpose.**

This RFC establishes a machine-native relation architecture for expressing what a transformation, implementation, representation, abstraction, provider, or lowering step actually preserves.

The intended result is that MNCS can answer questions such as:

- Are these two artifacts byte-identical, structurally identical, alpha-equivalent, logically equal, observationally equivalent, or merely compatible?
- Is the relation symmetric or directional?
- Under which observations are two programs indistinguishable?
- Which contexts are permitted to distinguish them?
- Which internal transitions may be hidden?
- Does a candidate merely agree on a finite corpus, or does evidence support a universal relation?
- Does an implementation refine a specification by removing nondeterminism or failure behaviors?
- Can one provider safely replace another under a required interface?
- Does a representation transformation preserve every logical observation while changing physical layout?
- Does a lowering establish a simulation relation between source and target states?
- Which state relation, trace relation, encode/decode law, contract implication, or abstraction relation witnesses the claim?
- Which assumptions and target facts does that witness depend upon?
- If one relation is refuted, which other relation claims remain unaffected?
- Which relation claims are sufficient under RFC 0018 to authorize a particular action?

The RFC deliberately does not select one grand equivalence calculus. Instead, it establishes a typed family of relations and requires machine-readable relation claims to carry their exact semantics.

---

## Motivation

MNCS already uses language such as:

- preserve semantics;
- equivalent realization;
- compatible component;
- provider satisfies requirement;
- transformation preserves behavior;
- implementation refines a reference;
- representation preserves logical value;
- source and target agree;
- proof establishes equality;
- candidate is substitutable;
- lowering is correct.

These phrases are useful shorthand but are not precise enough to serve as the constitutional basis of a machine-native language.

Consider several examples.

Two binaries may be behaviorally indistinguishable while having different artifact identities and bytes.

Two values may have identical bytes while inhabiting different nominal types and therefore not be substitutable.

Two representations may encode the same logical matrix while exposing different size, alignment, address, or timing properties.

A deterministic implementation may refine a nondeterministic specification without being behaviorally identical to it.

A provider may satisfy an interface contract while not being type-equivalent to the provider it replaces.

Two components may be ABI-compatible while being behaviorally incompatible.

Two functions may return the same values but differ in externally observable I/O, failure, timing, allocation, energy, side-channel, or trace behavior.

A bounded execution corpus may provide empirical evidence that two implementations agreed on tested inputs without establishing universal equivalence.

A static analyzer may soundly approximate a concrete value without producing an equal value.

A compiler pass may preserve public observations while radically changing internal state, control flow, data layout, or transition structure.

If MNCS collapses these relations into one `equivalent = true` bit, later subsystems will inevitably overclaim what a verifier established.

RFC 0018 already requires evidence to remain scoped to the claim actually established. RFC 0019 already separates logical value, type, observation, representation, storage, and serialization. RFC 0020 supplies the relation vocabulary required to connect those layers without collapsing them.

---

## Constitutional principles

### 1. No universal sameness relation

> **MNCS MUST NOT define a universal relation whose satisfaction implies every other notion of sameness, substitutability, or compatibility.**

A relation claim must identify its kind.

Examples include:

```text
ArtifactIdentity(a, b)
DefinitionalEquality(x, y)
PropositionalEquality(x, y)
AlphaEquivalence(s1, s2)
ObservationalEquivalence(p, q, O)
ContextualRefinement(p, q, O, C)
RepresentationEquivalence(r1, r2, O)
Substitutable(provider, requirement, C)
AbiCompatible(a, b, abi_contract)
AbstractionSoundness(concrete, abstract, abstraction)
```

### 2. Identity is not equality

> **Two objects may satisfy a semantic equality or equivalence relation while retaining distinct identities and histories.**

Identity is attached to an identity scheme and an entity class.

Examples include:

- artifact identity;
- content identity;
- semantic node identity;
- nominal or generative type identity;
- logical value identity;
- representation identity;
- storage-object identity;
- runtime-object identity;
- location identity;
- verifier identity;
- evidence identity;
- transformation identity.

Proving that two artifacts are equivalent MUST NOT erase their identities, provenance, or evidence histories.

### 3. Equality is not equivalence

> **A structure-preserving or behavior-preserving relation does not automatically create identity or proof-theoretic equality.**

An invertible mapping between `A` and `B` may establish an isomorphism while `A` and `B` retain distinct type identities.

### 4. Equivalence is not refinement

> **Equivalence is generally symmetric; refinement is intentionally directional.**

An implementation may remove behaviors permitted by a specification and therefore refine it without being equivalent to it.

### 5. Refinement is not compatibility

> **A refinement relation does not automatically establish ABI, protocol, wire, module, deployment, or environment compatibility.**

Compatibility is always compatibility with respect to a declared contract class.

### 6. Compatibility is not substitutability

> **Two components may interact successfully without one being a valid replacement for the other in every required context.**

Substitutability is a contextual replacement claim.

### 7. Behavioral relations require observation scope

> **A behavioral equivalence or refinement claim without a declared observation model is incomplete.**

For example:

```text
O_public = {
    returned_values,
    externally_visible_effects,
    declared_failures
}
```

may establish a relation that does not hold under:

```text
O_security = O_public + {
    timing,
    cache_access_pattern,
    memory_access_pattern
}
```

### 8. Context matters

> **A component relation that quantifies over contexts MUST declare the class of contexts allowed to interact with the subjects.**

A relation that holds for well-typed callers may fail for unsafe foreign callers that can inspect layout or violate invariants.

### 9. Environment matters

> **Environment- or target-sensitive relation claims MUST retain the environmental assumptions under which they hold.**

The same pair of implementations may be equivalent on one target and distinguishable on another.

### 10. Relation claims are proof-relevant artifacts

> **A relation claim SHOULD retain the witness structure that explains how the relation is established, rather than only a boolean result.**

Examples include:

- equality proofs;
- state correspondence relations;
- forward/backward simulation witnesses;
- bisimulation relations;
- trace mappings;
- encode/decode relations;
- contract implication witnesses;
- abstraction/concretization relations;
- finite empirical comparison reports.

### 11. Evidence is relation-scoped

> **Evidence supporting or refuting one relation MUST NOT silently propagate to a stronger or unrelated relation.**

A timing mismatch may refute timing equivalence while leaving functional equivalence unaffected.

A corpus agreement may support bounded observational agreement without proving contextual equivalence.

### 12. Substitution requires congruence

> **Replacing one object with a related object is valid only in operators and contexts shown to respect the claimed relation.**

This principle applies to rewriting, optimization, module replacement, representation substitution, and lowering.

### 13. Hidden observations must be explicit

> **Weak equivalence or weak simulation may hide only transitions or observations explicitly classified as internal for that relation.**

A verifier MUST NOT choose ad hoc which events “do not matter.”

### 14. Refinement may reduce nondeterminism

> **A more deterministic implementation may refine a more permissive specification when every remaining implementation behavior is allowed by the specification.**

This must not be misreported as equality.

### 15. Mutual refinement may induce an equivalence

> **Where a refinement preorder is defined, mutual refinement may derive a relation-specific equivalence without collapsing subject identity.**

### 16. Abstraction is not equality

> **An abstract interpretation result soundly over- or under-approximating concrete behavior is an abstraction claim, not an equality claim.**

### 17. Finite agreement is not universal equivalence

> **Empirical comparison over a finite domain or corpus MUST retain that bound in the relation claim and evidence.**

### 18. Proof-theoretic equality remains kernel-scoped

> **Definitional and propositional equality used by the RFC 0007 proof core MUST remain distinguishable from broader program-equivalence relations.**

### 19. Representation equivalence preserves declared observations, not necessarily physical properties

> **RFC 0019 representation realizations may be equivalent under logical observations while remaining distinct under layout, address, timing, serialization, or resource observations.**

### 20. Relation preservation must be compositional where claimed

> **If MNCS composes relation witnesses across transformations, the composition rule and its preconditions MUST be explicit.**

---

## Terminology

### Subject

A **subject** is an object participating in a relation claim.

Subjects may include:

- logical values;
- types;
- semantic graph nodes;
- functions;
- computations;
- transition systems;
- modules;
- interfaces;
- implementations;
- representations;
- storage realizations;
- executable artifacts;
- traces;
- protocol states;
- environments;
- target realizations;
- proof terms;
- abstract-domain elements.

### Relation kind

A **relation kind** defines:

- admissible subject classes;
- arity;
- directionality;
- algebraic properties;
- observation requirements;
- context requirements;
- environment requirements;
- witness schema;
- composition law;
- substitution/congruence rules;
- invalidation dependencies.

### Observation model

An **observation model** defines which distinctions count as externally relevant for a relation.

### Context model

A **context model** defines which enclosing programs, callers, environments, or interacting systems are quantified over by a contextual relation.

### Relation witness

A **relation witness** is the structured justification used by a checker or verifier to establish a relation claim.

### Relation evidence

**Relation evidence** is evidence under RFC 0002/RFC 0018 supporting or refuting a relation claim.

A witness may itself be evidence, may reduce to a kernel-checkable proof, or may be an empirical artifact depending on the relation method.

### Preservation claim

A **preservation claim** states that a transformation maps subjects while preserving a declared relation or property set.

### Refinement direction

A **refinement direction** identifies which subject is the specification/reference and which is the implementation/candidate.

The canonical orientation MUST be standardized before implementation to prevent ambiguous use of symbols such as `⊑`.

---

## Relation family

RFC 0020 proposes a relation family rather than a total hierarchy.

The initial conceptual vocabulary is:

```text
Identity
DefinitionalEquality
PropositionalEquality
StructuralEquality
AlphaEquivalence
Isomorphism
LogicalEquivalence
ObservationalEquivalence
ContextualEquivalence
Simulation
Bisimulation
TraceEquivalence
Refinement
ContextualRefinement
RepresentationEquivalence
Substitutability
Compatibility
AbstractionSoundness
BoundedAgreement
```

This list is not intended to freeze the final schema. It establishes semantic ownership and prevents incompatible uses of one word from being silently merged.

---

# Part I — Identity

## Identity schemes

MNCS already uses multiple identity schemes. RFC 0020 makes their separation explicit.

A generic identity object may resemble:

```text
Identity {
    scheme_id
    subject_class
    identity_value
    generation_context?
    namespace?
    canonicalization_version?
}
```

Possible schemes include:

```text
ContentHashIdentity
CanonicalSemanticIdentity
NominalIdentity
GenerativeIdentity
ArtifactIdentity
RepresentationIdentity
StorageObjectIdentity
RuntimeObjectIdentity
LocationIdentity
TransformationIdentity
EvidenceIdentity
```

### Content identity

Two artifacts may share content identity if canonical bytes are identical under the declared content-identity scheme.

This does not imply:

- same provenance;
- same creation event;
- same authority;
- same trust state;
- same external location.

### Semantic identity

Semantic identity may be derived from canonical semantics under a versioned canonicalization scheme.

Two source texts may therefore have different source identities while sharing semantic identity.

### Nominal and generative identity

RFC 0013 and RFC 0019 require support for identities that are intentionally not collapsed by structural equivalence.

Two opaque types can have the same structure and remain distinct nominal types.

### Runtime object identity

A runtime object's identity may persist across changes to location or representation if the relevant runtime model defines such persistence.

This must remain distinct from address identity.

---

# Part II — Proof-theoretic equality

## Definitional equality

Definitional equality is the equality recognized directly by the RFC 0007 proof kernel through its selected normalization/computation rules.

Conceptually:

```text
DefEq<T>(a, b)
```

Properties may include:

- reflexivity;
- symmetry;
- transitivity;
- congruence under kernel constructors;
- decidability or semi-decidability depending on the selected core;
- normalization-based checking.

RFC 0020 does not choose the final reduction system, universe hierarchy, proof irrelevance policy, or equality theory for RFC 0007.

### Constitutional boundary

General compiler optimization, contextual equivalence, representation equivalence, and empirical agreement MUST NOT be silently added to definitional equality.

Otherwise the proof kernel would inherit the complexity and trust assumptions of the compiler/runtime world.

## Propositional equality

Propositional equality is an explicit proposition that two terms are equal within a logical system.

Conceptually:

```text
Eq<T>(a, b)
```

with a proof term:

```text
p : Eq<T>(a, b)
```

This equality may support transport/substitution within the proof system according to RFC 0007's eventual rules.

### Equality proof identity

The identity of the proof object is distinct from the equality proposition it establishes.

Different proof terms may establish the same equality claim.

### Univalence direction

Homotopy type theory and univalence provide a useful research direction for transporting structure across equivalences, but RFC 0020 does not declare unrestricted type equivalence to be global type identity.

Machine systems retain distinctions such as:

- ABI identity;
- representation identity;
- nominal identity;
- authority;
- target applicability;
- artifact provenance.

If a future proof stratum admits univalent reasoning, that interpretation must remain explicitly scoped.

---

# Part III — Structural and naming relations

## Structural equality

Structural equality compares logical structure under a declared structural schema.

Examples may compare:

- constructor identity;
- field names;
- field values;
- recursive shape;
- ordered or unordered members depending on schema.

Structural equality MUST state which metadata is ignored.

## Alpha-equivalence

Bound names often have no semantic significance.

RFC 0020 should support a relation such as:

```text
AlphaEquivalent(a, b, binding_model)
```

where differences caused only by consistently renamed bound identifiers are ignored.

Examples include:

```text
fn(x) = x + 1
```

and:

```text
fn(y) = y + 1
```

under an ordinary lexical binding model.

### Freshness and binding

The canonical semantic representation SHOULD avoid unnecessary dependence on human spellings of bound names.

Possible realizations include:

- de Bruijn indices;
- locally nameless representations;
- nominal techniques;
- canonical binder identities.

RFC 0020 does not select one representation.

### Alpha-equivalence is not artifact identity

The source artifacts remain distinct even if their binding structures are alpha-equivalent.

---

# Part IV — Isomorphism and logical equivalence

## Isomorphism

An isomorphism represents an invertible structure-preserving relationship.

Conceptually:

```text
Isomorphism<A, B> {
    forward: A -> B
    backward: B -> A
    left_inverse
    right_inverse
    preserved_structure
}
```

Different relation classes may require stronger structure-preservation laws.

Examples include:

- product reassociation;
- equivalent finite encodings;
- logical data views;
- type-level structural transformations.

An isomorphism MUST NOT automatically imply:

- nominal identity;
- ABI compatibility;
- representation identity;
- performance equivalence;
- side-channel equivalence.

## Logical equivalence

Logical equivalence relates propositions or semantic predicates that imply one another under a declared logic and assumption set.

Conceptually:

```text
LogicalEquivalent(P, Q)
```

may be witnessed by:

```text
P -> Q
Q -> P
```

inside an appropriate proof system.

This remains distinct from identity of propositions unless the proof theory explicitly identifies them.

---

# Part V — Observations

## Observation models are first-class

RFC 0019 establishes observation boundaries around values and representations. RFC 0020 generalizes observation scope to behavioral relations.

An observation model may resemble:

```text
ObservationModel {
    id
    observable_kinds
    visibility_rules
    hidden_kinds
    normalization_rules
    failure_observation_policy
    termination_observation_policy
    nondeterminism_policy
    time_model?
    resource_model?
    security_model?
}
```

Possible observable kinds include:

```text
ReturnValue
LogicalValue
DeclaredFailure
FatalFailure
ExternalEffect
Event
Trace
Termination
Divergence
Timing
Allocation
MemoryFootprint
Address
Layout
CachePattern
BranchPattern
Power
Energy
NetworkTraffic
StorageWrites
AuthorityUse
TargetPromise
```

The list is deliberately extensible.

## Observation-profile identity

A behavioral relation claim MUST reference a versioned observation model or an explicit canonical observation set.

Changing the observation model invalidates or creates a distinct relation claim.

## Observation projection

Where useful, an observation model may define a projection:

```text
observe_O : Behavior -> ObservableBehavior_O
```

Behavioral equivalence can then compare projected behaviors rather than internal implementation histories.

## Observation refinement

If one observation model exposes strictly more distinctions than another, an equivalence under the stronger model may imply equivalence under the weaker model when the relationship itself is proven.

MNCS MUST NOT assume this implication from names alone.

---

# Part VI — Contexts

## Context models

A context model describes which enclosing or interacting systems are allowed to distinguish subjects.

Conceptually:

```text
ContextModel {
    id
    subject_interface
    allowed_operations
    capabilities
    unsafe_access
    foreign_access
    observation_model
    environment_constraints
}
```

Examples include:

```text
WellTypedMNCSCaller
SafePublicInterfaceCaller
UnsafeFFIContext
SameModuleContext
CrossModuleContext
AdversarialNetworkPeer
TrustedRuntimeContext
```

## Contextual claims

A contextual relation quantifies over a declared context class.

Conceptually:

```text
for every C in ContextModel:
    observe(C[A]) relation observe(C[B])
```

This is stronger than direct unit-level comparison because the subjects may interact with arbitrary admissible clients.

## Context closure

Relation composition and substitution depend on context closure.

A relation established only for pure contexts cannot automatically be used in effectful, concurrent, unsafe, or foreign contexts.

---

# Part VII — Observational equivalence

## Observational equivalence

Two subjects are observationally equivalent when they cannot be distinguished by the observations defined by a particular model.

Conceptually:

```text
ObservationalEquivalent(A, B, O)
```

The exact semantic formulation depends on subject class.

For deterministic functions it may compare observable results for all admissible inputs.

For nondeterministic systems it may compare behavior sets, distributions, traces, or another declared model.

For interactive systems it may require stronger contextual or bisimulation-based semantics.

## Functional equivalence

A common specialization is functional equivalence:

```text
FunctionalEquivalent(f, g, input_domain, output_observation)
```

This relation SHOULD NOT silently include:

- timing;
- allocation;
- logging;
- network behavior;
- nondeterministic trace shape;
- side channels;
- power use.

Those require explicit observation inclusion.

## Termination-sensitive versus termination-insensitive equivalence

The observation model must indicate whether divergence is observable.

For example:

```text
TerminationSensitive
TerminationInsensitive
```

may induce different relation results.

RFC 0022 will later own the broader termination/productivity semantics.

## Failure-sensitive relations

RFC 0011 distinguishes failure classes and knowledge states. RFC 0020 requires behavioral relations to state which failure distinctions are observable.

A relation may preserve:

- exact failure class;
- only success versus failure;
- only externally visible effects;
- recovery outcome;
- compensation behavior.

This choice must not remain implicit.

---

# Part VIII — Contextual equivalence

## Contextual equivalence

Two subjects are contextually equivalent when every admissible context produces indistinguishable observations.

Conceptually:

```text
ContextualEquivalent(A, B, O, C)
```

This is one of the strongest general-purpose notions of behavioral sameness MNCS should support.

It is particularly useful for:

- component replacement;
- optimization validation;
- abstraction preservation;
- representation independence;
- module transformations;
- compiler correctness statements.

## Contextual equivalence is parameterized

There is no single universal contextual equivalence because the set of legal contexts matters.

Unsafe pointer inspection may distinguish implementations that safe typed code cannot.

A timing observer may distinguish implementations that a functional observer cannot.

A target-specific context may expose instructions or faults absent from a portable context.

---

# Part IX — Logical relations and representation independence

## Logical relations as relation witnesses

Logical relations provide a general proof technique for showing that two implementations, values, or representations are related at corresponding types.

RFC 0020 treats logical relations as a family of possible witnesses rather than the only semantics of equivalence.

A relation may be indexed by type:

```text
R_T(v1, v2)
```

and function-relatedness may require:

```text
R_A(a1, a2)
=>
R_B(f1(a1), f2(a2))
```

subject to effects and computation semantics.

## Representation independence

RFC 0019 representation realizations can use logical relations to establish that distinct physical encodings implement the same logical abstraction.

Example:

```text
R_matrix(row_major_rep, blocked_simd_rep)
```

may require all corresponding logical elements to agree while allowing layout, alignment, and traversal order to differ.

The relation witness may include:

- logical decode functions;
- correspondence predicates;
- constructor/view preservation;
- operation preservation;
- invariant preservation.

## Parametricity

RFC 0013 polymorphism may provide relational parametricity guarantees for code that cannot inspect representation-sensitive details.

A representation-sensitive observation can intentionally narrow or invalidate those guarantees.

---

# Part X — Transition systems and simulation

## Simulation

A simulation relates state-transition systems directionally.

Conceptually:

```text
Simulation {
    source_system
    target_system
    state_relation
    initial_relation
    step_relation
    final_relation
    observation_relation
    divergence_conditions
    assumptions
}
```

A source step may correspond to:

- exactly one target step;
- zero or more target steps;
- one or more target steps;
- a finite stuttering sequence;

according to the simulation kind.

## Forward simulation

A forward simulation generally shows that target execution can match source execution from related states.

Exact orientation and behavior-inclusion consequence MUST be standardized with care.

## Backward simulation

Backward simulation reasons in the opposite direction and may be required for stronger preservation claims in the presence of nondeterminism.

## Lock-step simulation

A lock-step simulation can be used where source and target have closely corresponding transitions.

MNCS SHOULD NOT require lock-step simulation for all correct transformations because lowering and optimization often radically alter transition structure.

## Stuttering simulation

A stuttering simulation allows multiple internal target steps to correspond to one source step or vice versa under declared rules.

This is useful for:

- lowering;
- instruction selection;
- administrative reductions;
- runtime bookkeeping.

## Simulation witness identity

The state relation is part of the witness identity.

Two verifiers may establish the same high-level preservation claim using different state correspondences.

---

# Part XI — Bisimulation and coinduction

## Bisimulation

Bisimulation is a symmetric state-correspondence technique in which related states can mutually match relevant transitions.

Conceptually:

```text
Bisimulation(A, B, R, O)
```

where `R` relates states and `O` determines observable labels.

## Strong bisimulation

Strong bisimulation requires transition matching without hiding declared internal actions.

## Weak bisimulation

Weak bisimulation may abstract over explicitly classified internal transitions.

Conceptually:

```text
internal = {tau, runtime_bookkeeping, ...}
```

The hidden-transition set MUST be part of the relation definition.

## Coinductive evidence

For potentially infinite reactive behavior, a bisimulation relation may provide coinductive evidence of behavioral equivalence without enumerating all traces.

This connects to RFC 0019 coinductive observations and RFC 0012 transition semantics.

## Concurrency

RFC 0010 concurrency semantics may require relation variants sensitive to:

- causality;
- visible event order;
- memory consistency;
- atomicity;
- interference;
- progress assumptions.

RFC 0020 defines the relation architecture but does not choose one concurrency equivalence.

---

# Part XII — Trace relations

## Traces

RFC 0012 and RFC 0011 already motivate observable execution histories.

A trace relation compares histories rather than only final states.

Possible events include:

- I/O operations;
- messages;
- failures;
- compensation events;
- resource acquisition/release;
- authority changes;
- externally visible state changes.

## Trace equivalence

```text
TraceEquivalent(A, B, O)
```

requires equivalent observable trace sets under the declared model.

## Trace refinement

```text
TraceRefines(Implementation, Specification, O)
```

requires implementation traces to fall within the behavior allowed by the specification, subject to the chosen orientation.

## Trace projection

Internal trace events may be projected away only if the observation model says they are hidden.

## Prefix-sensitive properties

Some safety properties depend on prefixes rather than final traces.

Relation witnesses SHOULD preserve enough trace structure to evaluate such properties.

## Infinite traces

Potentially infinite traces must be handled according to the coinductive/temporal semantics eventually established by RFC 0022 and RFC 0023.

---

# Part XIII — Refinement

## Refinement as directional semantic inclusion

Refinement is the primary relation for saying that a candidate is an admissible, more concrete, or more constrained realization of a specification.

Conceptually:

```text
Refines(candidate, specification, O)
```

means every behavior exposed by `candidate` under `O` remains permitted by `specification`, together with any additional required obligations.

The final symbol orientation MUST be standardized before implementation.

## Refinement as preorder

A useful refinement relation should normally support:

```text
reflexive
transitive
```

so that refinement chains compose.

## Mutual refinement

If:

```text
Refines(A, B, O)
Refines(B, A, O)
```

then MNCS may derive a relation-specific mutual-refinement equivalence.

This does not imply identity.

## Nondeterminism reduction

A specification may allow:

```text
A or B
```

while a candidate always produces:

```text
A
```

The candidate may refine the specification if `A` remains allowed.

## Failure refinement

A candidate may remove a permitted failure mode and still refine a specification.

A candidate that introduces a new externally visible failure generally does not refine the specification unless that failure was already permitted by the relevant observation/contract model.

## Effect refinement

RFC 0008 effects and authority can participate in refinement.

A candidate using fewer effects or less authority may refine a requirement in some policy models.

A candidate requiring broader authority may violate refinement even if returned values agree.

## Resource refinement

RFC 0032 will later define quantitative resource relations.

RFC 0020 reserves the ability for refinement contracts to include protected quantitative bounds without defining their final arithmetic semantics.

---

# Part XIV — Contextual refinement

## Contextual refinement

Contextual refinement asks whether one subject can replace another across every admissible context without introducing forbidden observations.

Conceptually:

```text
ContextualRefines(candidate, baseline, O, C)
```

This is likely to become MNCS's strongest general-purpose replacement relation for transformations and implementations.

## Why contextual refinement matters

Direct testing of a function may miss interactions with:

- caller state;
- aliasing;
- effects;
- authority;
- concurrency;
- callbacks;
- failures;
- hidden representation assumptions.

Contextual refinement makes the replacement boundary explicit.

## Deriving contextual equivalence

Mutual contextual refinement may derive:

```text
ContextualEquivalent(A, B, O, C)
```

without collapsing identity.

---

# Part XV — Substitutability

## Substitutability as interface-relative replacement

Substitutability is a relation between a provider and a requirement under a context/interface contract.

Conceptually:

```text
Substitutable {
    provider
    requirement
    interface
    context
    assumptions
    obligations
}
```

## Behavioral subtyping direction

The design should preserve the behavioral-subtyping principle that a replacement may not invalidate guarantees made to clients.

Relevant dimensions include:

- accepted input domain;
- preconditions;
- postconditions;
- invariants;
- effects;
- capabilities;
- authority requirements;
- failure modes;
- temporal behavior;
- resource requirements;
- ownership/lifetime constraints;
- concurrency guarantees.

## Preconditions

A substitute generally must not require stronger preconditions from existing clients unless the context contract explicitly changes.

## Postconditions

A substitute generally must satisfy at least the required postconditions.

## Effects and authority

A provider that produces additional undeclared effects or requires broader authority may fail substitutability even if return values match.

## Failure behavior

Failure modes are part of substitutability where clients can observe or must handle them.

## Relation to RFC 0014

RFC 0014 owns module/component/provider/requirement structures and compatibility negotiation.

RFC 0020 supplies the generic relation machinery for expressing when a provider is a valid substitute.

---

# Part XVI — Compatibility

## Compatibility is contract-class specific

Compatibility MUST always identify the contract class.

Possible classes include:

```text
TypeCompatibility
InterfaceCompatibility
ModuleCompatibility
AbiCompatibility
WireCompatibility
SchemaCompatibility
ProtocolCompatibility
RuntimeCompatibility
TargetCompatibility
VersionCompatibility
DeploymentCompatibility
```

## Compatibility is not one algebraic relation

Different compatibility classes may be:

- symmetric;
- directional;
- version-ordered;
- negotiated;
- environment-dependent.

The generic relation framework must not impose symmetry or transitivity where they do not hold.

## Compatibility witness

A compatibility claim may carry:

- requirement/provider matching;
- negotiated feature set;
- schema mapping;
- ABI layout agreement;
- protocol-state mapping;
- version migration;
- environment constraints.

## Compatibility versus equivalence

Two components can be compatible while producing different behavior.

Compatibility states that interaction satisfies a contract, not that behavior is identical.

---

# Part XVII — Representation equivalence

## Relationship to RFC 0019

RFC 0019 separates logical value/type semantics from physical representation realizations.

RFC 0020 defines how two representations can be related.

Conceptually:

```text
RepresentationEquivalent {
    logical_type
    left_representation
    right_representation
    observation_model
    correspondence
    obligations
}
```

## Logical observation preservation

Two representations may be equivalent under logical observation even when they differ in:

- size;
- alignment;
- field order;
- tag layout;
- boxing;
- pointer topology;
- cache locality;
- address identity;
- SIMD blocking;
- device location.

## Encode/decode laws

A representation witness may include:

```text
decode_R1 : R1 -> T
decode_R2 : R2 -> T
```

and establish:

```text
decode_R1(r1) == decode_R2(r2)
```

for related valid representations.

Where bidirectional encoding is required, stronger laws may establish round-trip behavior.

## Representation observations narrow equivalence

If a program explicitly observes layout or address, the observation model changes and a previously valid representation-equivalence claim may no longer apply.

## Transformation classes

The relation architecture should be able to describe legal:

- boxing/unboxing;
- niche encoding;
- packing;
- field reordering;
- AoS/SoA transformation;
- scalar replacement;
- vector blocking;
- closure conversion;
- defunctionalization;
- compression;
- target-specific layout.

---

# Part XVIII — Full abstraction

## Full abstraction as a translation property

A translation is fully abstract relative to source/target context and observation models when it preserves and reflects the relevant contextual equivalence.

Conceptually:

```text
A ≈source B
iff
L(A) ≈target L(B)
```

under declared context mappings.

## Why MNCS should reserve this concept

A lowering may preserve output behavior while exposing new distinguishing power at the target level.

Examples include exposing:

- layout;
- pointer identity;
- timing distinctions;
- side-channel behavior;
- previously inaccessible state.

Full abstraction provides a vocabulary for stronger abstraction-preservation claims.

## Not mandatory for every pass

Full abstraction is a strong property and RFC 0020 does not require every transformation to satisfy it.

Instead, it defines the claim so stronger compilers/security profiles can request it explicitly.

---

# Part XIX — Quotients and intentional erasure of distinctions

## Quotient relations

Sometimes MNCS may intentionally decide that distinctions under an equivalence relation no longer matter.

Conceptually:

```text
Quotient(A, ~)
```

treats equivalent members as one abstract equivalence class.

Potential uses include:

- syntax modulo alpha-equivalence;
- graph representations modulo canonical renaming;
- normalized algebraic structures.

## Quotient safety

Operations on a quotient must respect the quotient relation.

If:

```text
a ~ b
```

then any well-defined quotient operation `f` must preserve the relation required by its codomain.

## Quotient identity

Creating a quotient does not retroactively erase identity or provenance in underlying artifacts.

The quotient is a new abstraction.

---

# Part XX — Congruence and generalized rewriting

## Relation-respecting operators

An operator may declare that it preserves a relation.

Conceptually:

```text
CongruenceRule {
    operator
    input_relation
    output_relation
    positions
    assumptions
    witness
}
```

Example:

```text
R(a, b)
Congruent_f(R, S)
-----------------
S(f(a), f(b))
```

## Generalized rewriting

MNCS SHOULD support relation-aware rewriting rather than restricting all semantic rewriting to definitional equality.

However, a rewrite is legal only where the enclosing operator/context is proven to respect the relation.

## Rewrite authority

A rewrite that changes only a proof-level expression may have different authority requirements from a transformation that changes executable semantics.

RFC 0015/RFC 0016/RFC 0018 remain responsible for authorization and promotion boundaries.

---

# Part XXI — Abstract interpretation and sound approximation

## Abstraction relations

Static analyses often reason in abstract domains.

RFC 0020 introduces a generic relation class such as:

```text
Abstraction {
    concrete_domain
    abstract_domain
    abstraction_map?
    concretization_map?
    relation
    soundness_direction
    assumptions
}
```

## Over-approximation

For safety analysis, an abstract element may conservatively describe a superset of possible concrete behaviors.

Conceptually:

```text
concrete_behavior ∈ concretize(abstract_behavior)
```

## Under-approximation

Some analyses intentionally under-approximate reachable behavior, for example bug-finding or witness generation.

The direction must be explicit.

## Abstract equality is not concrete equality

Two concrete states mapping to one abstract state do not become semantically equal.

The abstraction intentionally discards distinctions.

## Relation to verifier evidence

An analyzer result can support a property because the abstraction is sound for that property class.

The evidence chain should include:

- abstraction definition;
- analyzer/version identity;
- soundness assumptions;
- concrete subject identity;
- abstract result identity.

---

# Part XXII — Bounded agreement

## BoundedAgreement relation

MNCS needs an explicit relation for empirical comparison over finite domains.

Conceptually:

```text
BoundedAgreement {
    left
    right
    observation_model
    corpus_or_domain
    environment
    result
}
```

This relation prevents existing tools such as bounded execution comparison from accidentally implying universal equivalence.

## Corpus identity

The exact corpus/domain is part of the relation scope.

Changing the corpus yields a new claim/evidence artifact.

## Counterexample

A single in-scope disagreement can refute bounded agreement for the tested corpus.

It may also provide evidence refuting a stronger universal relation when the counterexample lies inside that stronger relation's quantified domain.

## Passing a bounded comparison

A PASS establishes only:

```text
agreement over tested domain under declared observations/environment
```

unless additional proof machinery establishes more.

---

# Part XXIII — RelationClaim object model

## Core artifact

The canonical relation artifact should eventually resemble:

```text
RelationClaim {
    schema_version
    claim_id

    relation_kind

    left_subject
    right_subject

    direction?

    observation_scope?
    context_scope?
    environment_scope?
    target_scope?

    quantified_domain?

    assumptions
    protected_properties

    witness_refs
    obligation_refs
    evidence_refs

    status
    provenance
}
```

The exact serialization is experimental.

## Relation definition

A separate object may define reusable relation semantics:

```text
RelationDefinition {
    relation_id
    version

    subject_kinds
    arity

    directionality
    algebraic_properties

    observation_requirements
    context_requirements
    environment_requirements

    witness_schema
    verifier_contracts

    composition_rules
    congruence_rules
}
```

## Subject references

Subject references MUST use stable identities appropriate to their layers.

Examples:

- semantic identity;
- HIR identity;
- SSA identity;
- representation identity;
- artifact hash;
- component identity.

## Direction

Directional relations MUST never rely on argument ordering alone in human-facing tools.

The machine artifact should record semantic roles such as:

```text
specification
implementation
baseline
candidate
source
target
provider
requirement
concrete
abstract
```

## Status

Relation evaluation may produce method-scoped statuses such as RFC 0002/RFC 0018 `PASS`, `FAIL`, or `UNKNOWN`.

The claim's broader assurance disposition remains governed by RFC 0018.

---

# Part XXIV — Relation witnesses

## Equality witness

```text
EqualityWitness {
    proof_term
    proof_core_version
}
```

## Isomorphism witness

```text
IsomorphismWitness {
    forward
    backward
    left_inverse_evidence
    right_inverse_evidence
}
```

## Logical-relation witness

```text
LogicalRelationWitness {
    relation_definition
    base_cases
    constructor_cases
    function_case
    recursive_case
}
```

## Simulation witness

```text
SimulationWitness {
    state_relation
    initial_state_mapping
    step_correspondence
    terminal_correspondence
    observation_mapping
    divergence_conditions
}
```

## Bisimulation witness

```text
BisimulationWitness {
    state_relation
    left_step_rule
    right_step_rule
    hidden_transition_policy
}
```

## Trace witness

```text
TraceWitness {
    trace_projection
    trace_relation
    prefix_rules
    infinite_trace_policy
}
```

## Refinement witness

```text
RefinementWitness {
    behavior_inclusion_method
    contract_implications
    state_or_trace_relation
    assumptions
}
```

## Representation witness

```text
RepresentationWitness {
    logical_relation
    validity_relation
    decode_relation
    observation_preservation
    target_applicability
}
```

## Substitutability witness

```text
SubstitutabilityWitness {
    precondition_relation
    postcondition_relation
    invariant_relation
    effect_relation
    capability_relation
    failure_relation
}
```

## Abstraction witness

```text
AbstractionWitness {
    abstraction_relation
    concretization_relation
    soundness_evidence
    precision_scope
}
```

These are conceptual shapes, not accepted schemas.

---

# Part XXV — Relation algebra

## Reflexivity

Some relation kinds are reflexive.

Examples likely include:

- equality;
- equivalence;
- refinement;

subject to exact scope.

Compatibility need not always be modeled as reflexive under every external contract.

## Symmetry

Equality and equivalence relations are usually symmetric.

Refinement and substitutability are directional.

Compatibility may be symmetric or directional depending on the contract.

## Transitivity

Many relation kinds are intended to compose transitively.

Composition may fail when:

- observation models differ;
- context models differ;
- assumptions are incompatible;
- environment scopes do not overlap;
- target scopes differ;
- witness methods are not compositional.

Transitivity MUST NOT be inferred from relation names alone.

## Antisymmetry

Refinement need not be antisymmetric at the subject-identity level because distinct subjects may mutually refine one another.

A quotient by mutual refinement could yield an antisymmetric order, but that is a separate construction.

## Relation composition

A composed relation artifact should preserve provenance:

```text
A R B
B R C
---------
A R C
```

with references to both supporting claims and the composition rule.

The system MUST retain the intermediate identities and evidence dependencies.

## Cross-relation derivation

Some relation combinations may derive another relation.

Examples:

```text
ContextualRefines(A, B)
ContextualRefines(B, A)
=> ContextualEquivalent(A, B)
```

or:

```text
StrongObservationEquivalent(A, B)
projection_preserves(Strong, Weak)
=> WeakObservationEquivalent(A, B)
```

Every such rule must be explicit and versioned.

---

# Part XXVI — Relation strength and partial ordering

## No global ladder

RFC 0020 rejects a universal ordering such as:

```text
identity > equality > equivalence > refinement > compatibility
```

because these relations answer different questions.

## Local implication graphs

A relation family may define local implication rules.

For example, under appropriate definitions:

```text
ArtifactIdentity
=> ByteEquality
```

or:

```text
ContextualEquivalence(O,C)
=> ObservationalEquivalence(O)
```

may hold.

But such rules are relation-definition artifacts, not global folklore.

## Incomparability

Examples of incomparable claims include:

- ABI compatibility versus functional equivalence;
- timing equivalence versus type isomorphism;
- representation equivalence versus storage identity;
- trace refinement versus artifact identity.

---

# Part XXVII — Transformation contracts

## Transformations should name relations

A transformation artifact should not claim merely:

```text
preserves_semantics = true
```

Instead it should declare required relation claims.

Conceptually:

```text
TransformationContract {
    transformation
    source
    target

    required_relations
    optional_relations
    explicitly_not_claimed

    assumptions
    obligations
}
```

Example:

```text
required:
    ContextualRefinement under O_public, C_safe
    RepresentationEquivalence under O_logical

optional:
    TraceEquivalence under O_external

not_claimed:
    TimingEquivalence
    AllocationEquivalence
    ByteIdentity
```

## Protected properties

RFC 0004 protected properties may be represented directly or translated into required relation dimensions.

## Transformation search

Forge can search transformations that intentionally alter unprotected dimensions while preserving required relations.

This makes the optimization search space explicit.

---

# Part XXVIII — Lowering correctness

## Relation-per-stage model

Lowering may establish a chain:

```text
SemanticCore
    R1
HIR
    R2
SSA
    R3
BackendIR
    R4
MachineArtifact
```

Each stage may use a different state space and witness type.

## Simulation-based lowering

Simulation is a natural foundation because it permits structural differences between source and target execution states.

## End-to-end theorem

Where relation composition rules permit, stage-local witnesses may compose into an end-to-end preservation or refinement claim.

The composed claim must retain all assumptions introduced by intermediate stages.

## Backend assumptions

Backend promises such as:

- no-overflow;
- no-alias;
- alignment;
- in-bounds;
- fast-math relaxation;

may change the relation being claimed.

RFC 0006 evidence-gated promises must therefore reference the exact preservation relation they authorize.

## Undefined behavior avoidance

A lowering cannot prove preservation by relying on target undefined behavior unless the source semantics and transformation contract explicitly permit the corresponding behavior class.

---

# Part XXIX — Recursive refinement and Forge

## Baseline-candidate relations

RFC 0004 repair/refinement candidates should carry explicit relation claims against the baseline.

Example:

```text
CandidateAssessment {
    baseline
    candidate

    relation_claims: [
        ContextualRefinement(public),
        RepresentationEquivalence(logical),
        BoundedAgreement(edge_corpus)
    ]
}
```

## Relation delta

A candidate may:

- preserve a relation;
- strengthen a relation;
- weaken a relation;
- introduce a new relation claim;
- invalidate a prior claim;
- leave a relation unresolved.

This should be represented as a machine-readable **relation delta**.

## Search objectives

Forge may prefer candidates that:

- preserve all mandatory relations;
- strengthen selected refinements;
- reduce assumptions;
- increase relation evidence quality;
- improve quantitative objectives.

RFC 0033 will later define multi-objective selection semantics.

## Candidate rejection

A candidate that violates one protected relation is rejected even if it improves unrelated metrics.

---

# Part XXX — Assurance and promotion

## RFC 0018 integration

A relation claim is a claim subject under RFC 0018.

Evidence can:

- support it;
- refute it;
- conflict;
- remain insufficient.

An assurance profile can require different relation evidence for different actions.

Example:

```text
ExperimentalRepresentationUse:
    require BoundedAgreement(edge_corpus)
    require no known logical counterexample

ProductionRepresentationPromotion:
    require RepresentationEquivalence(logical)
    require independent verifier evidence
    require target applicability evidence
```

## Relation-specific counterexamples

A counterexample artifact must identify the relation and scope it refutes.

Example:

```text
Counterexample {
    relation_claim
    input
    left_observation
    right_observation
    environment
}
```

## Conflicting evidence

One verifier may support a relation while another produces a counterexample.

RFC 0018 then records a `CONFLICTED` claim disposition rather than silently selecting one result.

---

# Part XXXI — Invalidation

## Dependency-sensitive invalidation

Relation claims depend on:

- subject identities;
- relation definitions;
- observation models;
- context models;
- environments;
- targets;
- assumptions;
- witnesses;
- verifier implementations.

Changing any relevant dependency may stale the relation claim.

## Local invalidation

A change to one observation model should not invalidate unrelated relations that do not depend upon it.

## Witness reuse

If a local change affects only part of a relation witness, tooling should eventually support narrow revalidation rather than discarding the entire proof/evidence graph.

This is a research direction, not a current implementation requirement.

---

# Part XXXII — Security consequences

## Security observations

Security-sensitive equivalence may include observations such as:

- timing;
- memory access pattern;
- cache behavior;
- branch behavior;
- network metadata;
- resource consumption;
- error-channel behavior;
- authority use.

RFC 0024 will later define information-flow and side-channel semantics.

RFC 0020 establishes that security claims are different relation scopes rather than implicit properties of ordinary functional equivalence.

## Unsafe contexts

RFC 0015 unsafe/FFI contexts may distinguish representations that safe MNCS contexts cannot.

A contextual-equivalence claim MUST say whether unsafe contexts are included.

## Capability behavior

Two implementations returning identical values are not substitutable if one silently requires broader authority than the contract permits.

## Full abstraction

Secure lowering may eventually require full-abstraction or robust-preservation claims under adversarial target contexts.

This RFC reserves those relation forms without defining the final secure-compilation model.

---

# Part XXXIII — Nondeterminism and probability

## Nondeterministic behavior sets

RFC 0011 requires nondeterminism to remain explicit.

Equivalence for nondeterministic systems may compare:

- sets of possible outcomes;
- trace sets;
- may/must behavior;
- scheduler-quantified behavior;
- fairness-constrained behavior.

The chosen relation must state which interpretation it uses.

## Probabilistic relations

RFC 0011 permits probabilistic choices only under explicit models.

RFC 0020 does not define probabilistic equivalence in detail, but reserves relation kinds such as:

```text
DistributionalEquivalence
ProbabilisticRefinement
ApproximateEquivalence
```

for later quantitative work.

A probability-distance bound is not exact equality.

---

# Part XXXIV — Approximate relations

## Approximate equivalence

Numeric and probabilistic systems often need bounded approximation rather than exact equality.

RFC 0021 will own detailed numeric semantics.

RFC 0020 establishes only that an approximate relation must make its metric/tolerance explicit.

Conceptually:

```text
ApproxEquivalent(A, B, metric, epsilon)
```

## Non-transitivity concerns

Naive epsilon equality may fail transitivity.

Therefore approximate relations MUST declare their algebraic properties instead of being assumed to be equivalence relations.

## Accumulated error

Relation composition may widen bounds.

Composition rules must calculate or conservatively propagate that widening.

---

# Part XXXV — Interfaces with RFC 0019 values and types

## Type identity versus structural equivalence

RFC 0019 permits nominal, opaque, and generative types.

Two types with the same constructors may therefore satisfy structural equivalence without sharing type identity.

## Value equality versus representation equality

Two logical values may be equal while represented by different bit patterns.

Two bit patterns may be identical while interpreted as different logical values under different types.

## Dynamic packages

Equality between existential/dynamic packages may require explicit type witnesses or a relation that does not reveal hidden type identity.

## Views

Two values may expose equivalent semantic views while having different representations.

The view relation must not imply layout equality.

---

# Part XXXVI — Interfaces with modules/components

## Provider refinement

RFC 0014 provider selection may use refinement or substitutability relations.

A provider can satisfy a requirement without being equivalent to another provider.

## Compatibility matrices

Module compatibility may be represented as typed compatibility claims rather than ad hoc version comparisons.

## Dynamic binding

Dynamic provider binding must preserve the exact relation required by the consumer contract.

## Version coexistence

Future live-upgrade semantics may require old/new components to be mutually compatible without being equivalent.

---

# Part XXXVII — Interfaces with memory and concurrency

## Memory observations

RFC 0009 memory provenance and RFC 0010 concurrency may make address, aliasing, ordering, or atomicity observable.

Relation claims must include those dimensions when relevant.

## Weak memory

Two implementations may be equivalent under sequential consistency but not under a weaker target memory model.

Target/context assumptions must therefore be part of the relation scope.

## Linearizability and refinement

Concurrency research includes refinement-like relations such as linearizability.

RFC 0020 does not adopt one concurrency-correctness criterion but provides the typed relation framework into which such criteria can fit.

---

# Part XXXVIII — Interfaces with staging and self-transformation

## Stage-relative relations

RFC 0016 transformations may occur at compile time, generation time, specialization time, or runtime.

A relation claim must state the stage identities of its subjects when stage distinctions matter.

## Self-transformation

A self-transforming system MUST NOT authorize replacement merely because the candidate claims equivalence.

The candidate relation claim is untrusted until evaluated under RFC 0018.

## Bootstrap relations

Self-hosting may require relation claims between bootstrap compiler stages.

Those claims must preserve bootstrap trust boundaries and artifact identities.

---

# Part XXXIX — Machine-native diagnostics

## Relation failure should be structured

A failed relation should produce a structured diagnostic rather than only:

```text
not equivalent
```

Possible diagnostic dimensions include:

```text
relation_kind
observation_kind
context
first_counterexample
trace_divergence
state_correspondence_failure
assumption_failure
witness_failure
scope_mismatch
unsupported_relation_method
```

## Minimal distinguishing observation

Where possible, verifiers should produce the smallest defensible observation that distinguishes the subjects.

This supports RFC 0004 causal slicing and repair.

## Relation mismatch taxonomy

Examples include:

- value mismatch;
- failure mismatch;
- effect mismatch;
- trace-order mismatch;
- termination mismatch;
- authority mismatch;
- representation-observation mismatch;
- context-only mismatch;
- target-only mismatch;
- assumption mismatch.

---

# Part XL — Formal notation direction

The final specification may adopt notation such as:

```text
x ≡ y             definitional equality
x =_P y           propositional equality
A ≅ B             isomorphism
A ≈[O] B          observational equivalence
A ≈[O,C] B        contextual equivalence
A ⪯[O] B          refinement
A ⪯[O,C] B        contextual refinement
R1 ≈rep[O] R2     representation equivalence
P <:beh Q         behavioral substitutability
```

These symbols are illustrative only.

The machine-readable artifact MUST NOT depend on glyph interpretation; it should use explicit relation identifiers.

---

## Theoretical synthesis

RFC 0020 draws selectively from multiple research traditions.

### Setoids and generalized rewriting

Setoid-style reasoning demonstrates that rewriting can operate over explicit equivalence relations rather than only built-in equality, provided operators respect those relations.

MNCS adopts:

- explicit relation definitions;
- relation-respecting morphisms/congruence rules;
- relation-aware substitution.

MNCS does not require all executable program relations to live inside the proof kernel.

### Intensional type theory

Intensional type theory motivates the distinction between definitional equality and proof-bearing propositional equality.

MNCS adopts the boundary while leaving RFC 0007's exact calculus open.

### Homotopy type theory and univalence

Univalence motivates principled transport across equivalences.

MNCS treats this as a research direction for appropriate logical strata rather than declaring all machine-relevant equivalences to be identities.

### Nominal techniques

Nominal sets, freshness, and alpha-equivalence motivate binding-aware semantic equality independent of superficial variable names.

### Observational/contextual equivalence

Programming-language semantics motivates defining behavioral sameness through what contexts can observe.

MNCS makes observation and context models first-class relation parameters.

### Logical relations

Logical relations provide compositional witnesses for representation independence, parametricity, and contextual reasoning.

MNCS treats them as reusable witness methods.

### Bisimulation and coinduction

Bisimulation provides a natural relation for stateful and potentially infinite transition behavior.

MNCS distinguishes strong/weak variants through explicit hidden-transition policy.

### Trace semantics

Trace equivalence/refinement models externally visible histories and intermediate events.

This is essential for I/O, concurrency, protocols, failure, and distributed systems.

### Simulation and verified compilation

Forward/backward simulation provides a practical foundation for proving semantics preservation between different operational state spaces.

MNCS uses this as a primary lowering-correctness direction.

### Refinement calculus

Refinement calculus motivates directional, transitive program improvement where a concrete implementation may reduce nondeterminism while preserving allowed behavior.

### Behavioral subtyping

Behavioral subtyping motivates substitutability based on client-visible contracts rather than structural resemblance alone.

### Full abstraction

Full abstraction motivates stronger translation claims that preserve and reflect contextual distinctions across language levels.

### Quotient constructions

Quotients motivate explicit abstraction over distinctions that a system intentionally chooses to ignore, provided all operations respect the quotient relation.

### Abstract interpretation

Abstract interpretation motivates sound approximation relations distinct from equality.

### Approximate/metric relations

Metric and approximate reasoning motivates tolerance-parameterized relations whose algebraic properties must be stated explicitly.

---

## What becomes primitive versus derived

RFC 0020 should avoid making every named relation a primitive kernel object.

A likely architecture is:

### Primitive framework

The semantic core needs primitive support for:

- relation definitions;
- typed subjects;
- directionality;
- observation/context/environment scopes;
- assumptions;
- witness/evidence references;
- algebraic-property declarations;
- composition/congruence rules;
- claim identity.

### Relation-specific libraries/verifiers

Specific relations can then be defined by versioned semantic modules/verifier contracts:

- alpha-equivalence;
- bounded execution agreement;
- structural equality;
- representation equivalence;
- trace refinement;
- simulation;
- contextual refinement;
- ABI compatibility;
- abstract-interpretation soundness.

### Proof-kernel primitive equality

RFC 0007 retains its own small trusted equality mechanisms.

The generic RFC 0020 relation framework references proof objects but does not place every relation algorithm inside the trusted kernel.

---

## Proposed canonical objects

A future implementation may introduce objects resembling:

```text
RelationId
RelationDefinition
RelationClaim
RelationSubject
ObservationModel
ContextModel
EnvironmentScope
QuantifiedDomain
RelationWitness
RelationObligation
RelationEvidenceLink
CongruenceRule
CompositionRule
Counterexample
RelationDelta
TransformationContract
```

Specialized witness forms may include:

```text
EqualityWitness
IsomorphismWitness
LogicalRelationWitness
SimulationWitness
BisimulationWitness
TraceWitness
RefinementWitness
RepresentationWitness
SubstitutabilityWitness
CompatibilityWitness
AbstractionWitness
BoundedAgreementReport
```

---

## Relationship to existing RFCs

### RFC 0001 — Semantic Foundation

RFC 0020 gives formal structure to semantic relationships that RFC 0001 currently treats more generally.

### RFC 0002 — Contract and Evidence Model

A relation claim is a property/claim subject whose supporting evidence must retain method and scope.

### RFC 0003 — Verified Intermediate Representation

IR transformations should name the exact relation they claim to preserve.

### RFC 0004 — Recursive Introspection and Refinement

Candidate comparison and promotion become relation-aware rather than relying on a generic “equivalent” result.

### RFC 0006 — Machine-Intent Expressions

Lowering envelopes and backend promises should identify required preservation/refinement relations.

### RFC 0007 — Proof-Carrying Dependent Core

Definitional and propositional equality remain proof-core concepts. Broader program relations stay outside the kernel unless reduced to kernel-checkable objects.

### RFC 0008 — I/O, Resource, Effect, Event Semantics

Effects/events become observable dimensions for behavioral relations.

### RFC 0009 — Memory, Reference, Provenance, Storage Semantics

Memory identity and provenance remain distinct from logical equality. Address or provenance may enter observation models where relevant.

### RFC 0010 — Concurrency

Bisimulation, trace relations, and refinement can be parameterized by concurrency semantics, causality, atomicity, and memory models.

### RFC 0011 — Failure, Recovery, Nondeterminism, External Observation

Failure, nondeterminism, ambiguous outcomes, and observation frontiers become explicit relation dimensions.

### RFC 0012 — Executable Semantic Core

Transition systems provide the operational basis for simulation, bisimulation, trace, and observational relations.

### RFC 0013 — Abstraction and Polymorphism

Parametricity/logical relations can justify representation independence and abstraction-preserving substitution.

### RFC 0014 — Modules and Components

Compatibility and substitutability become explicit relation classes for provider/requirement matching.

### RFC 0015 — Trust Boundary, Unsafe, FFI, ABI

Unsafe contexts and ABI compatibility use relation-specific semantics and may narrow contextual equivalence.

### RFC 0016 — Staging and Self-Transformation

Generated/specialized/self-transformed candidates must carry relation claims before promotion.

### RFC 0017 — Runtime, Environment, Target

Relation validity may depend on runtime and target facts.

### RFC 0018 — Assurance

Evidence over relation claims is evaluated under action-relative assurance policy.

### RFC 0019 — Values, Data, Types, Representation

RFC 0020 defines how logical values/types and distinct representations can be equal, equivalent, related, or observationally distinguishable.

---

## Future RFC boundaries

RFC 0020 intentionally leaves several specialized semantics to later RFCs.

### RFC 0021 — Numeric semantics

Will define exact and approximate numeric equality/error semantics for integers, floats, decimals, fixed point, intervals, and reproducibility.

### RFC 0022 — Termination/liveness

Will define termination, productivity, progress, liveness, and bounded-computation relations in detail.

### RFC 0023 — Time

Will define clock/deadline/temporal observations that can parameterize RFC 0020 relations.

### RFC 0024 — Information flow/security

Will define noninterference and side-channel-sensitive equivalences/refinements.

### RFC 0026 — Persistence

Will define durable-state/crash-consistency relationships.

### RFC 0027 — Serialization

Will define wire/schema compatibility and encode/decode equivalence.

### RFC 0028 — Distribution

Will define distributed observational/refinement relationships under partitions, messages, consistency, and replication.

### RFC 0030 — Deployment/live upgrade

Will define state-migration and version-coexistence compatibility/refinement.

### RFC 0032 — Quantitative cost

Will define cost/resource orderings and quantitative refinement.

---

## Non-goals

RFC 0020 does **not**:

- choose the final definitional equality of RFC 0007;
- adopt unrestricted univalence as global machine identity;
- declare structural equality to override nominal/generative identity;
- define one universal contextual equivalence for all effects/targets/security models;
- define the final concurrency equivalence;
- define the final distributed equivalence;
- define numeric floating-point equality;
- define probabilistic distance semantics;
- define the final information-flow/noninterference calculus;
- require every compiler pass to be fully abstract;
- place all relation checkers inside the trusted proof kernel;
- treat bounded testing as proof of universal equivalence;
- assume compatibility is symmetric or transitive;
- assume approximate equality is transitive;
- erase provenance when two artifacts are proven equivalent;
- select final glyph notation for relations;
- require one proof method for every relation.

---

## First implementation experiment

The first implementation should remain deliberately narrow.

### Goal

Demonstrate that the current MNCS model can represent and evaluate multiple distinct relations between the same pair of subjects without collapsing them into one `equivalent` bit.

### Required objects

Implement versioned research artifacts for:

```text
RelationDefinition
RelationClaim
ObservationModel
BoundedAgreementReport
Counterexample
RelationDelta
```

Optionally include a minimal:

```text
SimulationWitness
```

if the existing body/HIR/SSA evaluators make it practical.

### Relation kinds in the pilot

Support at least:

```text
ArtifactIdentity
StructuralEquality
BoundedAgreement
ObservationalEquivalenceCandidate
RefinementCandidate
RepresentationEquivalenceCandidate
```

The `Candidate` suffix should make clear that the first implementation is experimental and may rely on bounded evidence rather than universal proof.

### Observation profiles

Define at least two profiles:

```text
O_functional = {
    return_value,
    declared_failure
}
```

and:

```text
O_trace = {
    return_value,
    declared_failure,
    recorded_effect_events
}
```

### Test subjects

Use a small existing deterministic arithmetic/body corpus plus one effect-recording example.

Construct subjects where:

1. two artifacts are not byte-identical but agree under `O_functional`;
2. two subjects agree on returned values but differ under `O_trace`;
3. a deterministic candidate removes one behavior from a deliberately nondeterministic reference and is treated as a refinement candidate rather than equivalence;
4. two RFC 0019 representations decode to the same logical value while differing under a representation-sensitive observation;
5. one intentionally incorrect candidate produces a concrete counterexample.

### Bounded evidence rule

The pilot verifier MUST report finite comparison as:

```text
BoundedAgreement
```

not universal `ObservationalEquivalence` unless an independent proof path exists.

### Counterexample scope

A counterexample to `O_trace` equivalence must not automatically mark `O_functional` equivalence as refuted unless the distinguishing observation is included in `O_functional`.

### Relation delta

For one Forge candidate, emit:

```text
RelationDelta {
    preserved
    strengthened
    weakened
    refuted
    unresolved
}
```

for the small set of relation claims.

### RFC 0018 gating

Define two assurance policies:

```text
ExperimentalCandidateInspection
ProductionLikePromotionStudy
```

The first may accept bounded relation evidence.

The stronger policy must refuse to relabel bounded agreement as universal equivalence.

### Success criteria

The experiment succeeds if:

- relation kind is explicit in every claim;
- observation scope changes relation identity;
- the same subjects can simultaneously satisfy one relation and fail another;
- a directional refinement claim is not rendered as symmetric equality;
- finite tests remain bounded evidence;
- counterexamples are relation-scoped;
- relation claims participate in RFC 0018 freshness/assurance;
- relation deltas are machine-readable;
- the CLI can explain why two subjects are related or distinguishable.

---

## Suggested CLI experiments

Possible commands include:

```text
mncs relation show <claim>
mncs relation compare <left> <right> --relation bounded-agreement --observation functional
mncs relation compare <left> <right> --relation bounded-agreement --observation trace
mncs relation explain <claim>
mncs relation delta <baseline> <candidate>
```

These names are illustrative.

The important requirement is machine-readable output.

---

## Machine-readable explanation model

An explanation should answer:

```text
relation requested
subjects compared
observation/context scope
method used
witness/evidence used
assumptions
result
counterexample if any
relations not established
```

Example:

```text
requested:
    BoundedAgreement

observation:
    functional-v0

result:
    PASS

scope:
    corpus sha256:...

not established:
    ObservationalEquivalence(universal)
    TraceEquivalence
    TimingEquivalence
```

This explicit negative statement is useful because machines otherwise tend to over-generalize a PASS result.

---

## Open research questions

### Relation ontology

- Which relation kinds should be built into the canonical semantic schema versus defined as library-level relation definitions?
- Should relation definitions themselves be first-class semantic components?
- How should third-party relation kinds be namespaced and versioned?

### Identity

- Which identity schemes should be content-derived versus generative?
- When should semantic identity ignore metadata?
- How should canonical identity interact with alpha-equivalence and quotient structures?

### Equality

- What exact definitional equality will RFC 0007 use?
- Will any proof strata support univalence or observational equality?
- How should proof irrelevance affect equality-witness identity?

### Observation models

- Should observation models form an explicit refinement/containment lattice?
- Can stronger observation equivalence automatically discharge weaker observation claims when a projection theorem is available?
- How should newly introduced observable dimensions invalidate historical claims?

### Context models

- How should context classes be represented finitely?
- Can capability restrictions characterize context classes effectively?
- How should unsafe/foreign contexts interact with otherwise abstract representations?

### Simulations

- Which simulation variants are needed for HIR→SSA→backend lowering?
- How should divergence and stuttering be represented?
- When does forward simulation suffice versus requiring backward simulation?

### Nondeterminism

- Which may/must or trace-set semantics should be canonical for refinement?
- How should scheduler assumptions participate in relation identity?

### Concurrency

- Which bisimulation/refinement notions are appropriate for RFC 0010's weak-memory and causality semantics?
- Should linearizability, observational refinement, or another concurrency criterion become a standard relation definition?

### Approximation

- How should approximate equivalence compose?
- How should numeric, statistical, and probabilistic error bounds remain distinguishable?

### Congruence

- How are relation-respecting operators registered and verified?
- Can congruence facts be generated automatically from parametricity/type structure?

### Proof/evidence boundary

- Which relations can be reduced to kernel-checkable proof terms?
- Which require external verified checkers?
- Which remain empirical?
- How should certificates be composed across relation chains?

### Counterexamples

- What is the canonical minimal counterexample artifact?
- Can relation counterexamples automatically drive RFC 0004 causal slicing?
- How should partial traces be represented as distinguishing witnesses?

### Relation deltas

- How should a recursive-refinement system compare two large relation graphs efficiently?
- Can unchanged relation witnesses survive local semantic changes?

### Full abstraction

- Which boundaries warrant full-abstraction claims?
- How should source and target context classes be related?
- What secure-compilation properties should later build on this foundation?

---

## Risks

### Relation explosion

A fully explicit system could produce too many relation objects.

Mitigation:

- define canonical reusable relation profiles;
- derive common claims automatically;
- store only semantically relevant relation artifacts;
- use dependency-aware summaries without erasing source evidence.

### Solver/verifier complexity

Contextual equivalence and refinement can be undecidable or impractical in general.

Mitigation:

- permit bounded/specialized methods;
- retain `UNKNOWN`;
- separate relation semantics from proof method;
- require conservative fallbacks.

### Overly weak observations

A system could “prove equivalence” by choosing an observation model that ignores important distinctions.

Mitigation:

- action profiles choose required observation models;
- observation-model identity is explicit;
- security/resource/target profiles can demand stronger observations;
- policy cannot upgrade a weak relation to a stronger one.

### Hidden contextual assumptions

A relation may accidentally rely on clients being well behaved.

Mitigation:

- context model is part of relation identity;
- unsafe/foreign contexts are explicitly declared;
- assumptions remain evidence dependencies.

### Evidence laundering

Bounded empirical results could be relabeled as universal equivalence.

Mitigation:

- separate `BoundedAgreement` from universal relations;
- RFC 0018 forbids policy from fabricating stronger evidence classes;
- relation definitions declare accepted witness methods.

### Identity collapse

Optimization or canonicalization could erase provenance by merging equivalent artifacts.

Mitigation:

- identity and equivalence remain orthogonal;
- relation claims reference stable subject identities;
- provenance survives quotient/canonical projections.

---

## Design consequences for MNCS as a machine-native language

This RFC changes the meaning of “semantic preservation” from a vague compiler promise into an explicit machine object.

Instead of:

```text
input
 -> transform
 -> output
 -> verifier PASS
```

MNCS can represent:

```text
input
    |
    | claimed relation R
    | observation O
    | context C
    | assumptions A
    | witness W
    v
output
```

The relation tells the machine:

- which distinctions must be preserved;
- which distinctions may change;
- which behaviors may be removed;
- which observations matter;
- which contexts are quantified over;
- which state correspondence is used;
- what evidence is required;
- what the claim does **not** establish.

That is particularly important for Forge.

A machine-generated candidate can intentionally look nothing like the baseline internally while still carrying a precise relationship to it.

For example:

```text
baseline:
    linked recursive representation

candidate:
    packed vectorized representation

claims:
    RepresentationEquivalent under logical observations
    ContextualRefinement under safe public contexts
    BoundedAgreement on edge corpus

not claimed:
    LayoutEquivalence
    AddressEquivalence
    TimingEquivalence
```

A verifier can then check the relation that actually matters rather than forcing all valid transformations into structural resemblance.

This is the machine-native inversion:

> **The transformation is free to change everything that the required relation does not protect, while every protected distinction remains explicit and evidence-bearing.**

---

## Constitutional direction

RFC 0020 proposes that the eventual MNCS specification adopt the following durable rules:

1. There is no universal semantic `same_as`.
2. Identity schemes are explicit and do not collapse under equivalence.
3. Proof-theoretic equality is distinct from executable behavioral relations.
4. Behavioral relations include explicit observation scope.
5. Contextual relations include explicit context scope.
6. Environment-sensitive relations include environment/target scope.
7. Directional relations encode semantic roles explicitly.
8. Relation witnesses are proof-relevant where practical.
9. Finite empirical agreement remains bounded evidence.
10. Counterexamples refute only relations whose quantified scope includes the distinguishing case.
11. Refinement may reduce nondeterminism without becoming equality.
12. Mutual refinement may induce relation-specific equivalence without inducing identity.
13. Substitution requires a proven congruence/context rule.
14. Compatibility is contract-class specific.
15. Representation equivalence is observation-relative.
16. Abstract interpretation produces sound approximation relations, not equality.
17. Relation composition is explicit, versioned, and provenance-preserving.
18. RFC 0018 assurance policy decides whether evidenced relation claims authorize actions.
19. Transformations declare both relations they claim and significant relations they do not claim.
20. The language preserves freedom by protecting semantic relations rather than freezing implementation structure.

---

## Conclusion

RFC 0019 answers what MNCS values, types, observations, and representations are.

RFC 0020 answers how two such things may be related.

That distinction is foundational for the rest of the roadmap.

A machine-native language must be able to distinguish:

```text
same entity
same proof-theoretic value
same structure
same modulo names
invertibly related
logically equivalent
indistinguishable under observations
indistinguishable in all admitted contexts
mutually matching transition systems
same observable traces
one safely refines the other
one safely substitutes for the other
compatible under a declared contract
same logical value under different representation
sound abstraction of concrete behavior
agreement only over a bounded experiment
```

without flattening any of them into one `verified = true` bit.

Once these relations are first-class, MNCS transformations can become much more aggressive and much more defensible at the same time.

Forge does not need candidates to resemble the original implementation. It needs candidates to preserve the exact relations required by the program, environment, target, security profile, and promotion policy.

That provides the semantic vocabulary for verified lowering, representation search, recursive repair, optimization, compatibility, live replacement, and eventually machine-generated implementations that may be structurally novel while remaining precisely related to the trusted baseline.