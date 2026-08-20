# RFC 0035: Machine-Native Elaboration, Scope, Binding, Inference, Constraint, Defaulting, Coherence, and Resolution Semantics

- Status: Draft
- Track: Semantic foundation / elaboration
- Depends on: RFC 0001–0034 as referenced below
- Intended owner: `mncs-language`

## 1. Summary

MNCS needs a principled account of how conveniently incomplete, implicit, overloaded, polymorphic, proof-bearing, capability-sensitive, and machine-oriented source becomes a fully explicit semantic artifact.

This RFC defines elaboration as an evidenced relation between a source term and an explicit core term under a versioned binding environment, elaboration profile, and constraint system. It treats parsing, scope resolution, elaboration, constraint generation, constraint solving, defaulting, evidence search, coercion insertion, and lowering as distinct transformations.

The central rule is:

> **Surface omission is not semantic omission. Every implicit choice that affects meaning must either be uniquely determined by declared semantics, be made by an explicit versioned policy, or survive as an explicit unresolved choice. An elaborator may propose meaning, but it may not silently invent it.**

The intended architecture is proof-producing elaboration:

```text
SurfaceProgram
      |
      v
Scope / name resolution
      |
      v
ResolvedSurfaceProgram
      |
      v
Constraint generation
      |
      v
ConstraintGraph
      |
      v
Inference / evidence search / solver coordination
      |
      v
ElaborationCandidate
      |
      v
Explicit CoreProgram + ElaborationWitness
      |
      v
Small independent core checker
```

The elaborator may use unification, logic programming, SMT, Datalog, proof search, heuristics, specialized solvers, search, caches, or machine-learning assistance. Those mechanisms are not semantic authority. The accepted meaning is the explicit artifact that survives independent checking.

## 2. Motivation

Conventional languages routinely let compilers make hidden decisions about:

- which declaration a textual name refers to;
- which overload or method was selected;
- which type arguments were inferred;
- which implicit arguments were inserted;
- which interface, trait, or typeclass evidence was selected;
- which coercion path was inserted;
- which effect row or capability requirement was inferred;
- which region, universe, numeric type, or lifetime was chosen;
- which default was applied;
- which search rule or solver heuristic broke a tie.

That is tolerable only when the language has very strong coherence guarantees and the chosen meaning remains stable and independently reconstructible.

MNCS has stronger requirements. The system deliberately supports machine-generated programs, recursive refinement, proof-carrying semantics, implicit evidence, target-independent intent, deferred realization choices, and aggressive automation. Those goals make hidden elaboration authority especially dangerous.

A machine-native language should permit terse source while retaining explicit semantic accountability.

The language should be able to explain a source fragment such as:

```text
foo(1)
```

with a semantic record resembling:

```text
Reference:
    foo -> Binding B71

Overload candidates:
    foo<u32>
    foo<u64>
    foo<BigInt>

Expected constraints:
    return type satisfies StorageWord

Interface requirements:
    Hashable<T>
    Serializable<T>

Rejected:
    u32     -> range requirement fails
    BigInt  -> representation requirement fails

Selected:
    u64     -> unique surviving semantic candidate

Inserted evidence:
    Hashable<u64>      -> E12
    Serializable<u64>  -> E19

Inserted literal type:
    1 : u64

Defaults:
    none

Unresolved realization choices:
    LocationVariable L7

Core:
    foo<u64>(E12, E19, literal_u64(1), location=L7)

Status:
    UNIQUE

Witness:
    W104
```

The important property is not verbosity in source. The important property is that every meaning-affecting choice becomes explicit semantic structure.

## 3. Constitutional principles

RFC 0035 adopts the following rules.

### 3.1 Parsing is not name resolution

Parsing determines syntactic structure.

Name resolution determines which declaration a reference denotes.

### 3.2 Name resolution is not elaboration

Resolution binds references to declarations.

Elaboration determines the explicit semantic term, including inferred parameters, evidence, coercions, defaults, and solved semantic variables.

### 3.3 Elaboration is not lowering

Elaboration establishes meaning.

RFC 0003 and RFC 0006 lower already-explicit meaning into verified implementation representations.

### 3.4 Constraint generation is not constraint solving

The compiler must be able to represent the constraints produced by source independently of the mechanism used to solve them.

### 3.5 Search success is not uniqueness proof

Finding one legal interpretation does not establish that it is the only legal interpretation.

### 3.6 Search failure is not proof of nonexistence

A solver timeout, budget exhaustion, heuristic miss, or unsupported fragment must not be reported as semantic impossibility.

### 3.7 Ambiguity is a legal compiler result

When materially different legal meanings remain and no explicit policy determines one, the correct result is `AMBIGUOUS`.

### 3.8 Incidental ordering may not determine meaning

Filesystem order, declaration visitation order, hash-map iteration, cache order, thread scheduling, solver branching order, or LLM proposal order must not select between semantically distinct interpretations.

### 3.9 Inference may discover obligations but may not weaken them

Elaboration can infer an effect, capability, numeric, region, proof, placement, or interface obligation. It cannot silently erase or relax one.

### 3.10 Inference may discover authority requirements but may not manufacture authority

If an operation requires an RFC 0008/RFC 0025 capability and no valid capability exists in context, elaboration fails or remains unresolved. It must not synthesize ambient authority.

### 3.11 Defaults are semantic policy

A default is an explicit, versioned resolution rule. It is not the absence of semantics.

### 3.12 Explicit and defaulted choices retain distinct provenance

Two forms may elaborate to the same core while retaining different source/elaboration histories.

### 3.13 Intentional realization freedom must survive elaboration

A semantic hole that prevents the source from having complete meaning is different from an intentionally deferred realization variable owned by RFC 0019, RFC 0029, RFC 0032, RFC 0033, or another realization layer.

### 3.14 Deterministic elaboration and coherent elaboration are distinct

Two elaborators may produce different explicit core syntax yet preserve one meaning if the results are related by the required RFC 0020 relation.

### 3.15 The elaborator should remain outside the trusted semantic core where possible

The preferred architecture is an untrusted or weakly trusted elaborator producing an explicit core term plus a compact witness for a smaller checker.

## 4. Elaboration as a semantic relation

The conceptual judgment is:

```text
Gamma ; Env ; Profile |- source ~> core : Type ▷ Constraints ; Witness
```

The notation is intentionally schematic.

It means:

- `Gamma` is the local semantic context;
- `Env` is the versioned elaboration environment;
- `Profile` is the versioned set of elaboration/default/coherence rules;
- `source` is the resolved or partially resolved source term;
- `core` is an explicit semantic term;
- `Type` is the inferred or checked semantic type;
- `Constraints` are generated and discharged obligations;
- `Witness` records the derivation and inserted choices.

Elaboration is not assumed to be a total function.

A request may produce:

```text
UNIQUE
COHERENT_MULTIPLE
AMBIGUOUS
UNSAT
UNKNOWN
INCOMPLETE
```

where:

- `UNIQUE` means one semantic elaboration survives under the declared equivalence criterion;
- `COHERENT_MULTIPLE` means multiple derivations or core terms survive but are evidenced equivalent under the required relation;
- `AMBIGUOUS` means multiple materially distinct valid meanings remain;
- `UNSAT` means the explicit constraint set is proven inconsistent;
- `UNKNOWN` means the available solver/search mechanisms cannot decide;
- `INCOMPLETE` means required semantic holes remain intentionally present in a development artifact.

`UNKNOWN` is not `UNSAT`.

`COHERENT_MULTIPLE` is not `UNIQUE`.

`INCOMPLETE` is not a deferred realization variable.

## 5. Binding identity

### 5.1 Textual names are not semantic identities

A declaration should receive a stable semantic `BindingId` distinct from its source spelling.

```text
Binding {
    id: BindingId
    spelling: String
    namespace: NamespaceId
    scope: ScopeId
    phase: ScopePhase
    declaration: SemanticNodeId
}
```

Renaming a binder need not alter semantic identity when the surrounding artifact relation permits alpha-equivalent renaming.

### 5.2 References are explicit

```text
Reference {
    id: ReferenceId
    spelling: String
    source_scope: ScopeId
    namespace: NamespaceId
    phase: ScopePhase
}
```

A resolved reference should identify its binding and a resolution witness.

### 5.3 Alpha equivalence

The implementation may use:

- stable binding IDs;
- de Bruijn levels/indices;
- locally nameless representation;
- nominal techniques;
- another verified representation.

RFC 0035 requires alpha-invariant semantic reasoning. It does not prescribe one internal representation.

## 6. Scope graphs and resolution paths

A declarative scope-graph model is the preferred conceptual starting point.

```text
ScopeGraph {
    scopes
    declarations
    references
    edges
    import_relations
    namespace_rules
    phase_rules
}
```

Useful edge kinds may include:

```text
LEXICAL_PARENT
IMPORT
QUALIFIED_IMPORT
REEXPORT
MODULE_MEMBER
PRELUDE
MACRO_HYGIENE
PHASE_BRIDGE
```

The final vocabulary remains experimental.

### 6.1 Resolution witness

```text
ResolutionWitness {
    reference: ReferenceId
    binding: BindingId
    path: [ScopeEdge]
    shadowed_candidates: [BindingId]
    excluded_candidates: [ResolutionExclusion]
    environment: ElaborationEnvironmentId
    profile: ElaborationProfileId
}
```

This enables machine-readable answers to:

- why did this name resolve here?
- which imports were traversed?
- which bindings were shadowed?
- which candidates were excluded and why?
- which environment/profile made this result valid?

### 6.2 Shadowing

Shadowing is not implementation traversal order. It is a resolution rule.

A profile may permit lexical shadowing, forbid particular shadowing classes, or require explicit qualification in ambiguous scopes.

### 6.3 Imports

If two visible imports expose indistinguishable declarations under the same resolution priority and namespace, the result is ambiguous unless:

- one declaration is explicitly more specific under a declared relation;
- a profile contains an explicit precedence rule;
- both candidates are proven coherent under the required semantic relation.

### 6.4 Resolution stability

RFC 0035 introduces scoped stability claims such as:

```text
StableUnderUnrelatedExtension
StableUnderNonVisibleExtension
StableUnderImportExtension
StableUnderInstanceExtension
```

A claim must name the permitted extension class.

No universal `stable=true` is defined.

## 7. Phase-sensitive binding

Machine-native programs may contain bindings available at different phases:

```text
SOURCE
ELABORATION
COMPILE_TIME
PROOF
RUNTIME
```

The final phase vocabulary remains open.

A compile-time proof helper need not exist at runtime.

A runtime capability must not be accidentally captured by an elaboration-time computation.

The resolution witness therefore includes phase identity where phase affects visibility.

Macro hygiene remains owned by RFC 0016. RFC 0035 consumes generated binding/hygiene information and resolves the resulting references.

## 8. Elaboration environment

Meaning is not determined by source text alone.

RFC 0035 introduces:

```text
ElaborationEnvironment {
    id
    visible_bindings
    imported_modules
    exported_interfaces
    interface_instance_set
    prelude_profile
    language_feature_profile
    defaulting_profile
    solver_profile
    phase_environment
}
```

The same source under different environments may legitimately elaborate differently.

That difference must alter elaboration identity.

A hidden ambient prelude is therefore not outside semantics. It is an explicit environment input.

## 9. Elaboration profile

```text
ElaborationProfile {
    id
    language_version
    resolution_policy
    defaulting_policy
    coherence_policy
    generalization_policy
    numeric_literal_policy
    coercion_policy
    interface_resolution_policy
    universe_policy
    solver_contracts
}
```

Profiles are versioned.

RFC 0036 owns how profile semantics evolve across language/specification versions.

## 10. Bidirectional typing

Bidirectional typing is the preferred initial inference discipline.

Two principal modes are:

```text
SYNTHESIZE:
    Gamma |- e => T

CHECK:
    Gamma |- e <= T
```

### 10.1 Synthesis

Synthesis derives a type from a term where enough local information exists.

Examples may include:

- variables;
- annotated expressions;
- explicit constructors;
- projections;
- sufficiently explicit function applications.

### 10.2 Checking

Checking verifies an expression against an expected type.

This is especially useful for:

- literals;
- lambdas;
- higher-rank values;
- dependent constructors;
- holes;
- overloaded expressions where context narrows candidates.

### 10.3 Expected types are semantic inference inputs

```text
let x: u64 = 1
```

should check `1` against `u64` rather than first choosing a numeric default.

That differs from:

```text
let x = 1
```

where defaulting or ambiguity may remain.

### 10.4 Annotations are control points

Annotations are not merely human documentation.

They may deliberately reduce elaboration search and stabilize meaning.

## 11. Local inference and principality

MNCS should prefer local, predictable inference over magical whole-program inference.

The language should infer what is uniquely justified and request explicit information where global search would become:

- ambiguous;
- unstable;
- nonprincipal;
- excessively expensive;
- semantically surprising.

### 11.1 Principal solutions

Where a fragment admits a principal type or principal constraint solution, RFC 0035 should preserve that stronger property.

```text
PrincipalSolution {
    solution
    generality_relation
    witness
}
```

### 11.2 No principal solution

Advanced combinations such as subtyping, overloading, GADTs, higher-rank polymorphism, dependent typing, and effects may destroy principality.

`NO_PRINCIPAL_SOLUTION` is a legitimate result.

A required annotation is preferable to arbitrary selection.

## 12. Metavariables

Elaboration introduces explicit metavariables.

```text
MetaVariable {
    id
    kind
    context
    origin
    state
}
```

Candidate kinds include:

```text
TERM
TYPE
EFFECT
UNIVERSE
REGION
CAPABILITY
INTERFACE_EVIDENCE
ROW
```

States include:

```text
UNSOLVED
SOLVED
BLOCKED
AMBIGUOUS
CONFLICTED
DEFERRED_REALIZATION
```

The last state is intentionally distinguished from ordinary unsolved semantic metavariables.

## 13. Semantic holes

Typed holes are first-class development artifacts.

```text
Hole {
    id
    kind
    expected_type
    context
    constraints
    source_origin
}
```

Example:

```text
map(?, xs)
```

may produce:

```text
Hole H17
expected:
    A -> B
context:
    xs : List<A>
    f1 : A -> B
    f2 : A -> C
```

A development artifact can remain structurally valid with holes when its assurance/profile permits incomplete source.

Executable artifacts may forbid semantic holes.

### 13.1 Holes are not deferred realization variables

A hole such as:

```text
?body : Output
```

means semantic content is missing.

A location variable such as:

```text
LocationVariable L
```

may be fully legitimate semantic output because RFC 0029 intentionally keeps location open for realization search.

RFC 0035 must preserve that distinction.

## 14. Constraint generation

Elaboration emits explicit constraints instead of embedding all decisions in procedural compiler state.

```text
Constraint {
    id
    relation
    arguments
    owner_rfc
    origin
    phase
    status
}
```

Potential examples include:

```text
T1 == List<T2>
T2 implements Serializable
Effect(e) subset_of E
Region(r1) outlives Region(r2)
Universe(u1) < Universe(u2)
Capability(FileRead) available_in Gamma
NumericRelation R holds
```

The semantic owner matters.

RFC 0035 coordinates inference over these constraints but does not redefine the relations owned by earlier RFCs.

## 15. Constraint origin

Every generated constraint should retain provenance.

```text
ConstraintOrigin {
    source_node
    elaboration_rule
    imported_fact
    parent_constraints
}
```

That enables explanations such as:

```text
Constraint C18 exists because call node N44 invokes operation ReadFile,
which declares capability requirement FileRead under RFC 0008.
```

Compiler diagnostics should be projections of semantic explanation artifacts wherever possible.

## 16. Constraint graph

```text
ConstraintGraph {
    constraints
    metavariables
    dependency_edges
    solver_assignments
}
```

The graph supports:

- incremental solving;
- localized invalidation;
- parallel solving of independent components;
- explanation extraction;
- unsatisfiable cores;
- cross-solver communication;
- reproducibility analysis.

## 17. Solver outcomes

Every solver family must preserve at least:

```text
SAT
UNSAT
UNKNOWN
```

with optional richer states.

`UNKNOWN` includes cases such as:

- unsupported theory;
- timeout;
- bounded search exhaustion;
- resource budget exhaustion;
- external solver unavailable;
- intentionally incomplete search.

None of these prove `UNSAT`.

## 18. Constraint solutions

```text
ConstraintSolution {
    assignments
    discharged_constraints
    residual_constraints
    assumptions
    witnesses
    solver_evidence
}
```

A solution does not need to contain one monolithic proof object. It may reference narrow certificates/checks from specialized solvers.

## 19. Conflict explanations

For proven inconsistency:

```text
ConstraintConflict {
    conflicting_constraints
    source_origins
    minimality_claim
    witness
}
```

Possible minimality claims include:

```text
UNSAT_CORE
LOCALLY_MINIMAL
MINIMAL_UNDER_MODEL
UNKNOWN_MINIMALITY
```

Do not report generic `minimal=true` without a model.

## 20. Specialized solver architecture

RFC 0035 favors cooperating specialized solvers over one implicit universal algorithm.

Possible solver families include:

```text
NameResolutionSolver
FirstOrderUnifier
HigherOrderPatternUnifier
GeneralHigherOrderSearch
RowSolver
EffectSolver
UniverseSolver
RegionSolver
InterfaceEvidenceSolver
NumericConstraintAdapter
SMTAdapter
DatalogResolver
```

The list is not normative.

### 20.1 Cross-solver facts

When one solver communicates a fact to another, the fact should remain explicit and provenance-bearing.

```text
SolverFact {
    proposition
    producer
    dependencies
    witness
}
```

This is especially important for incremental and distributed elaboration.

## 21. Unification

### 21.1 First-order unification

The initial implementation should support a clear first-order unification fragment with:

- rigid variables;
- flexible metavariables;
- substitutions;
- occurs check;
- explicit failures.

### 21.2 Higher-order pattern unification

Higher-order pattern unification is a promising tractable fragment for dependent elaboration.

The language should distinguish this fragment from general higher-order search.

### 21.3 General higher-order search

General higher-order unification/search may be incomplete or heuristic.

Its result must not inherit the completeness claims of the pattern fragment.

```text
UnificationFragment {
    FIRST_ORDER
    HIGHER_ORDER_PATTERN
    GENERAL_HIGHER_ORDER
}
```

## 22. Definitional equality

RFC 0007 and RFC 0020 remain authoritative for definitional/propositional equality and normalization relations.

RFC 0035 may ask whether two terms are definitionally equal during elaboration.

It must not create a private equality relation with different semantics.

## 23. Generalization and instantiation

### 23.1 Generalization

Elaboration may generalize semantic variables where the language fragment permits.

```text
GeneralizationDecision {
    variables
    effect_context
    policy
    permitted
    witness
}
```

### 23.2 Effect-aware generalization

The initial design must not assume unrestricted ML-style generalization in the presence of effects and mutable state.

RFC 0008 effect information feeds the generalization policy.

Value-restriction and relaxed-value-restriction theories are explicit research inputs.

### 23.3 Instantiation

Instantiation replaces polymorphic variables with explicit semantic arguments.

The witness should record inferred instantiations when they affect meaning.

## 24. Higher-rank polymorphism and skolemization

Higher-rank checking may introduce rigid skolems.

```text
Skolem {
    id
    kind
    scope
    origin
}
```

Skolems must not escape their permitted scope.

Bidirectional checking is the preferred discipline for limiting uncontrolled higher-rank inference.

## 25. Universes

If the RFC 0007 dependent core uses universe levels, source syntax may omit explicit levels while elaboration introduces:

```text
UniverseVariable
UniverseConstraint
UniverseSolution
```

Example:

```text
Type u
Type v
u < v
```

### 25.1 Universe defaulting

If the language adopts a canonical minimal-universe policy, that policy must be explicit and versioned.

Otherwise multiple universe assignments may remain ambiguous.

## 26. Row polymorphism

Rows are a promising foundation for:

- extensible records;
- variants;
- effect rows;
- capability sets.

Potential constraints include:

```text
RowEquals
RowContainsLabel
RowLacksLabel
RowExtends
```

RFC 0019 owns record/variant semantics and RFC 0008 owns effect meaning. RFC 0035 owns inference over row variables where adopted.

## 27. Effect inference

Elaboration may infer effect sets/rows from explicit operations.

Example:

```text
Effect(e) = { IO, Network }
```

The inferred result becomes explicit in the elaborated artifact or witness.

Effect inference does not authorize the effects.

Authority remains governed by RFC 0008 and RFC 0025.

## 28. Capability inference and implicit authority

Surface syntax may permit implicit capability parameters:

```text
fn read_config(using cap: FileReadCapability) -> Config
```

A call may omit the explicit argument only when the context contains one or more eligible capabilities.

Elaboration must:

1. enumerate eligible evidence/capabilities;
2. verify authority and scope;
3. apply coherence/specificity rules;
4. insert the chosen capability explicitly into the core;
5. fail or report ambiguity when no unique coherent choice exists.

It must never construct new authority simply because an operation needs it.

## 29. Interface/typeclass evidence search

RFC 0013 owns abstraction, polymorphism, interfaces, and evidence semantics.

RFC 0035 treats interface resolution as evidence search.

```text
InterfaceGoal {
    predicate
    context
    world
}

InterfaceCandidate {
    implementation
    prerequisites
    specificity
}

InterfaceEvidence {
    goal
    implementation
    prerequisite_evidence
    proof_path
}
```

Surface:

```text
x == y
```

may elaborate conceptually to:

```text
eq<T>(EqEvidence17, x, y)
```

The core term makes the implicit evidence explicit.

## 30. Interface implementation identity versus evidence identity

An implementation artifact and the evidence that it satisfies an interface are not automatically the same object.

```text
InterfaceImplementation
InterfaceWitness
```

remain separable.

This allows stronger assurance profiles to require evidence beyond implementation presence.

## 31. Coherence and instance resolution

A central goal is that adding unrelated code should not silently change existing meaning.

RFC 0035 studies Rust-style coherence/orphan reasoning, Haskell-style evidence search, Scala-style contextual resolution, canonical structures, functional dependencies, associated types, and alternative evidence-search systems.

No one existing system is adopted wholesale.

### 31.1 Overlap

If two implementations overlap, the language may:

- reject the overlap;
- choose a uniquely more-specific candidate;
- allow the overlap only when evidence establishes semantic coherence;
- use an explicitly declared priority profile.

Search order is not a policy.

## 32. Specificity

```text
MoreSpecific(A, B)
```

must be a formal relation.

If one applicable candidate is uniquely maximal, it may be selected.

If multiple incomparable maximal candidates remain, the result is `AMBIGUOUS` unless coherence is established.

## 33. Open-world and closed-world evidence search

Resolution must identify its world assumption.

```text
ResolutionWorld {
    CLOSED
    OPEN
}
```

In a closed world, a complete search may prove no applicable instance exists.

In an open world:

```text
NoVisibleInstanceCurrently
```

is weaker than:

```text
NoPossibleInstanceExists
```

Negative constraints must preserve this distinction.

## 34. Recursive evidence search

Evidence goals may be recursive.

```text
A requires B
B requires A
```

Resolution must track goal stacks, cycles, search depth, and budgets.

RFC 0022 owns the semantics of bounded search and budget exhaustion.

`SEARCH_BUDGET_EXHAUSTED` therefore yields `UNKNOWN`, not proof of instance absence.

## 35. Associated types, projections, and determination

If interfaces expose associated types, elaboration may generate:

```text
ProjectionConstraint
NormalizationConstraint
```

Example:

```text
Iterator<C>::Item
```

Functional-dependency-like determination can also reduce ambiguity:

```text
DeterminationConstraint {
    determining_arguments
    determined_arguments
}
```

RFC 0035 reserves both research directions without selecting a final surface syntax.

## 36. Overload resolution

```text
OverloadSet {
    reference
    candidates
}

OverloadCandidate {
    binding
    applicability_constraints
    specificity
    required_evidence
    required_coercions
}

OverloadResolution {
    accepted
    rejected
    reason
    witness
}
```

Operator overloading and method resolution are instances of the same general elaboration problem.

No operator symbol receives hidden exception semantics merely because it is punctuation.

## 37. Method resolution

If MNCS supports method syntax such as:

```text
x.foo()
```

resolution may involve:

- inherent methods;
- interface methods;
- extension methods;
- explicit dereference or reference adaptation;
- implicit evidence.

Every meaning-affecting adaptation should appear in the elaboration trace or explicit core.

## 38. Coercion insertion

Coercions are semantic transformations.

```text
CoercionCandidate {
    from
    to
    transformation
    prerequisites
    relation
}

CoercionPath {
    edges
    result
}
```

Inserted coercions should become explicit in core or witness form.

### 38.1 Coercion coherence

If both:

```text
A -> B -> D
```

and:

```text
A -> C -> D
```

are legal, RFC 0035 requires one of:

- a declared unique priority;
- one uniquely more-specific path;
- evidence that both paths preserve the same required semantic relation;
- `AMBIGUOUS_COERCION`.

Graph traversal order may not decide.

## 39. Coercion versus subtyping

RFC 0020 owns subtyping/refinement relations.

A subtype relation and a coercion are distinct:

- subtyping may permit substitution without runtime transformation;
- a coercion may allocate, convert, validate, wrap, project, or otherwise transform.

RFC 0032 may quantify coercion cost.

A coercion is not assumed zero-cost merely because the compiler inserts it automatically.

## 40. Numeric literal elaboration

RFC 0021 owns numeric meaning.

RFC 0035 owns how an untyped or abstract literal receives a numeric semantic domain where source context leaves that implicit.

Potential destinations include:

```text
u8
i64
ExactNat
BigInt
Float
Decimal
```

No universal numeric default is assumed.

## 41. Defaulting

```text
DefaultingRule {
    id
    ambiguity_class
    preconditions
    chosen_result
    profile
}
```

A default applies only when its preconditions hold.

The elaboration witness records every default application.

### 41.1 Explicit versus defaulted

```text
1i64
```

and:

```text
1
```

under an `i64` default profile may yield the same explicit core numeric type while retaining distinct provenance:

```text
ExplicitChoice(i64)
DefaultedChoice(i64, D7)
```

### 41.2 Versioned defaults

Changing a default profile can change source meaning.

The profile version therefore contributes to elaboration identity and is governed by RFC 0036 evolution policy.

## 42. Canonical elaboration

Where possible, a profile may define a canonical elaboration among semantically coherent alternatives.

Reasons include:

- deterministic fingerprints;
- build caching;
- reproducibility;
- incremental compilation;
- cross-implementation comparison.

Canonicalization is explicit policy. It must not fabricate coherence where none exists.

## 43. Elaboration coherence

The strongest general concept is:

```text
For all valid elaborations E1, E2 of source S under environment Env/profile P,
Core(E1) ≡R Core(E2).
```

where `R` is the required RFC 0020 relation.

```text
CoherenceClaim {
    source
    environment
    profile
    elaboration_class
    relation
    witness
}
```

### 43.1 Determinism versus coherence

`ElaborationDeterministic` means one explicit result under a given execution/profile.

`ElaborationCoherent<R>` means all permitted results preserve one meaning under `R`.

Determinism is stronger in one dimension and weaker in another: a deterministic compiler can consistently choose an arbitrary meaning from an ambiguous language, which is not semantic coherence.

## 44. Ambiguity witness

```text
AmbiguityWitness {
    source
    candidate_a
    candidate_b
    first_distinguishing_choice
    relation
    environment
    profile
}
```

The purpose is to make ambiguity actionable.

Forge can propose explicit annotations or qualifications that collapse the candidate set.

## 45. Incoherence counterexample

Where multiple valid elaboration paths produce observably distinct cores:

```text
ElaborationCounterexample {
    source
    core_a
    core_b
    relation
    distinguishing_observation
    derivation_a
    derivation_b
}
```

This is stronger and more useful than generic `compiler ambiguity` text.

## 46. Semantic inference versus realization search

This boundary is central.

RFC 0035 answers:

> What does the source mean?

RFC 0029/RFC 0032/RFC 0033 may answer:

> How should that meaning be physically realized or selected?

Example:

```text
Map<K,V>
```

may intentionally leave open:

```text
hash table
tree
distributed map
custom specialized representation
```

if those are realizations of one semantic abstraction.

RFC 0035 must not choose one merely because it can.

## 47. Resolution-phase classification

Constraints/metavariables may be classified by when they must resolve:

```text
ELABORATION_REQUIRED
PROOF_REQUIRED
REALIZATION_DEFERRED
RUNTIME_RESOLVED
```

The final vocabulary is experimental.

A successful elaboration may legitimately retain `REALIZATION_DEFERRED` variables when their owner RFC permits it.

## 48. Elaboration must preserve obligations

Elaboration may not erase:

- proof obligations;
- effect requirements;
- authority requirements;
- numeric relations;
- information-flow labels;
- serialization obligations;
- distributed consistency obligations;
- placement constraints;
- quantitative budgets.

If elaboration transforms the representation of an obligation, it must preserve an explicit relation to the source obligation.

## 49. Elaboration provenance

```text
ElaborationWitness {
    source_identity
    environment_identity
    profile_identity
    resolved_bindings
    inferred_parameters
    inserted_evidence
    inserted_coercions
    default_applications
    generated_constraints
    solver_results
    residual_realization_variables
    core_identity
}
```

The exact schema will evolve.

The witness should be canonicalizable under RFC 0027 and provenance-bearing under RFC 0031.

## 50. Elaboration artifact identity

```text
ElaborationArtifact {
    source
    environment
    profile
    core
    witness
}
```

Two identical source texts under different environments are not assumed to have the same elaboration identity.

Two different elaboration executions may produce semantically equivalent cores.

RFC 0020 relations distinguish those cases.

## 51. Elaboration reproducibility

RFC 0035 defines at least two useful relations:

```text
ByteReproducibleElaboration
SemanticReproducibleElaboration<R>
```

The first asks whether canonical elaboration bytes/fingerprints match.

The second asks whether independently generated cores are related by `R`.

RFC 0031 records the derivation/environment/toolchain lineage of the elaborators themselves.

## 52. Incremental elaboration

The explicit binding and constraint graphs permit narrow invalidation.

A change should invalidate only dependent:

- reference resolutions;
- constraints;
- evidence searches;
- overload candidate sets;
- coercion paths;
- defaults;
- elaborated nodes.

This is preferred over global re-elaboration.

## 53. Elaboration cache identity

A cache key must include at least the semantic inputs that can affect meaning, such as:

```text
source identity
elaboration environment identity
elaboration profile identity
```

Caching by source bytes alone is invalid if imports, visible instances, defaults, or profiles can alter meaning.

## 54. Parallel elaboration

Independent constraint-graph components may be solved in parallel.

Shared metavariables, shared evidence goals, world assumptions, or ordering-sensitive profiles create dependencies.

RFC 0010 causality semantics can describe these dependencies where useful.

Parallel scheduling must not change semantic resolution.

## 55. Explanation artifacts and diagnostics

Diagnostics should be generated from structured semantic state where practical.

Example:

```text
UnresolvedMetaVariable M4
blocked by:
    Constraint C11
    Constraint C18
origins:
    call N31
    interface requirement N7
```

UI strings are projections of this state.

They are not the primary semantic representation.

## 56. Typed rejection reasons

Candidate rejection reasons may include:

```text
NAME_NOT_VISIBLE
TYPE_MISMATCH
CAPABILITY_MISSING
AUTHORITY_INVALID
EFFECT_MISMATCH
UNIVERSE_CONFLICT
ROW_CONFLICT
COERCION_AMBIGUITY
INSTANCE_AMBIGUITY
OVERLOAD_NOT_APPLICABLE
OVERLOAD_LESS_SPECIFIC
DEFAULT_NOT_AUTHORIZED
SOLVER_UNKNOWN
WORLD_NOT_CLOSED
GENERALIZATION_FORBIDDEN
```

The initial implementation need only support a bounded subset.

## 57. Machine-assisted elaboration

LLMs, solvers, search agents, and Forge may propose:

- missing type annotations;
- explicit qualifications;
- candidate hole fillings;
- interface evidence;
- proof terms;
- candidate coercions;
- disambiguating source patches.

Confidence from a proposal mechanism is not semantic authority.

Every accepted proposal is re-elaborated/checked under the ordinary rules.

## 58. Tactics and proof elaboration

RFC 0007 owns proof semantics.

A tactic such as:

```text
auto
```

may elaborate to an explicit proof term.

The tactic succeeds semantically only because the generated proof term checks in the RFC 0007 kernel.

The tactic engine remains untrusted search.

## 59. Extension/plugin elaborators

Future DSL or domain-specific elaborators may be permitted to produce:

```text
source extension
    -> explicit core + witness
```

The preferred trust boundary remains the standard core checker.

Plugins should not gain authority to bypass ordinary binding, type, effect, capability, or proof obligations.

## 60. Theory synthesis

RFC 0035 draws selectively from multiple traditions.

### 60.1 Hindley–Milner and Algorithm W

Useful for:

- unification;
- generalization;
- principal types;
- explicit substitutions;
- predictable inference in a well-understood fragment.

MNCS should preserve principality where available without pretending the full language has HM-level inference.

### 60.2 Bidirectional typing

Useful for:

- local inference;
- expected-type propagation;
- higher-rank checking;
- dependent constructors;
- predictable annotation boundaries.

This is a primary design influence.

### 60.3 Local type inference

Useful as a philosophy:

- infer where local information is enough;
- require explicit annotations where global inference becomes unstable or ambiguous.

### 60.4 Dependent-type elaboration

Proof assistants such as Lean, Agda, Coq, and related systems provide useful theory for:

- metavariables;
- implicit arguments;
- expected types;
- universe constraints;
- higher-order pattern unification;
- tactic-generated explicit proof terms.

MNCS should adopt concepts selectively, not implementation folklore wholesale.

### 60.5 Scope graphs

Useful for declarative:

- scope structure;
- imports;
- declaration/reference relationships;
- explicit resolution paths;
- language-specific resolution queries.

### 60.6 Alpha-equivalence representations

de Bruijn, locally nameless, and nominal methods remain candidate implementation theories for binding identity and alpha-invariant reasoning.

### 60.7 Constraint-based inference

Useful for separating declarative meaning from procedural solving.

The compiler should be able to show the constraint graph independently of the solver.

### 60.8 OutsideIn-style modular solving

Useful for integrating richer local assumptions, typeclass-style constraints, GADT-like information, and modular constraint solving.

### 60.9 Constraint Handling Rules

Useful as a declarative model for simplification, propagation, and extensible solver rules.

### 60.10 Higher-order pattern unification

Useful as a tractable dependent-elaboration fragment with stronger guarantees than unrestricted higher-order search.

### 60.11 Typed holes and structured incomplete programs

Hazelnut-like and related typed-hole theories are useful for machine-directed code completion and recursively refined incomplete artifacts.

### 60.12 Typeclasses and dictionary passing

Useful for turning implicit interface requirements into explicit evidence terms.

### 60.13 Trait coherence / orphan-style restrictions

Useful for extension stability and preventing unrelated dependencies from silently changing instance resolution.

### 60.14 Functional dependencies and associated types

Useful for expressing determination relationships and reducing inference ambiguity.

### 60.15 Coercive subtyping

Useful for explicit coherence conditions over inserted coercions.

### 60.16 Row polymorphism

Useful for extensible records, variants, and effect inference.

### 60.17 Value restriction

Useful for sound generalization in the presence of mutable/effectful computation.

### 60.18 Universe constraint solving

Useful for a dependent core with omitted source universe levels.

### 60.19 Datalog and logic programming

Potentially useful for resolution, evidence search, and declarative inference rules.

### 60.20 SMT with certificate/checking boundaries

Useful for arithmetic/refinement constraints while keeping large solvers outside the semantic TCB where possible.

### 60.21 Unsat cores and explanation-producing solving

Useful for repair-oriented compiler diagnostics and causal slices.

### 60.22 Incremental dependency solving

Useful for continuous Forge/IDE elaboration with narrow invalidation.

### 60.23 Proof-producing elaboration

This is the central MNCS synthesis:

> complex inference and search outside the TCB, explicit semantic output and compact independent checking inside it.

## 61. Cross-RFC ownership boundaries

### RFC 0003 / RFC 0006

Own core/HIR/lowering and verified realization transforms.

RFC 0035 completes semantic elaboration before lowering.

### RFC 0004

Owns recursive introspection/refinement.

RFC 0035 exposes holes, ambiguity, constraint conflicts, and explanations as repair targets.

### RFC 0005

Owns source representation and syntax exploration.

RFC 0035 maps source structures into explicit semantics.

### RFC 0007

Owns dependent proof-core semantics, definitional equality, and kernel checking.

RFC 0035 elaborates proof/source terms into kernel-checkable terms.

### RFC 0008

Owns effects, resources, capabilities, and authority requirements.

RFC 0035 can infer/pass them but cannot create authority.

### RFC 0009

Owns memory/reference/provenance/storage semantics.

RFC 0035 may solve reference/region constraints without redefining them.

### RFC 0013

Owns abstraction, polymorphism, interface, and evidence meaning.

RFC 0035 owns inference/instantiation/evidence resolution.

### RFC 0014

Owns modules, components, linking, and dependency semantics.

RFC 0035 consumes exported scopes and interface environments.

### RFC 0016

Owns staging/metaprogramming/macros/self-transformation.

RFC 0035 resolves/elaborates generated source while respecting hygiene/phase information.

### RFC 0018

Owns appraisal of elaboration/solver/witness evidence.

### RFC 0019

Owns value/type/representation semantics.

RFC 0035 infers/checks types and preserves realization freedom.

### RFC 0020

Owns equality, equivalence, refinement, compatibility, and coherence relations.

RFC 0035 consumes those relations when establishing elaboration coherence.

### RFC 0021

Owns numeric semantics.

RFC 0035 owns literal inference/defaulting only.

### RFC 0022

Owns bounded computation/search meaning.

RFC 0035 preserves search-budget exhaustion as `UNKNOWN`.

### RFC 0025

Owns principal identity/authority.

RFC 0035 can only resolve capabilities/evidence already valid in context.

### RFC 0029

Owns placement and location-variable realization.

RFC 0035 preserves intentionally deferred location variables.

### RFC 0031

Owns provenance and reproducibility of elaborators/build artifacts.

RFC 0035 defines elaboration identities/witnesses consumed by that provenance system.

### RFC 0032

Owns quantitative cost.

RFC 0035 does not choose a coercion or elaboration because it is cheaper unless semantic policy already makes that a realization choice.

### RFC 0033

Owns preference/selection among admissible realization candidates.

It must not be used to resolve genuine semantic ambiguity unless the source semantics explicitly delegates that dimension as realization freedom.

### RFC 0034

Owns empirical testing of elaborators and inference behavior.

It does not define elaboration meaning.

### RFC 0035

Owns scope/binding resolution, metavariables, semantic elaboration, inference orchestration, constraints, implicits/evidence, overload/coercion/default insertion, ambiguity, and coherence.

### RFC 0036

Should own evolution of the language/specification and elaboration profiles themselves.

## 62. Initial semantic planes

The initial architecture is grouped into ten planes.

### 62.1 Binding plane

```text
Scope
ScopeGraph
Binding
BindingId
Reference
ResolutionPath
ResolutionWitness
```

### 62.2 Elaboration plane

```text
SurfaceTerm
ResolvedTerm
CoreTerm
Elaboration
ElaborationCandidate
ElaborationWitness
```

### 62.3 Metavariable plane

```text
MetaVariable
TermHole
TypeHole
EffectHole
UniverseHole
DeferredRealizationVariable
```

### 62.4 Constraint plane

```text
Constraint
ConstraintGraph
ConstraintOrigin
ConstraintSolution
ConstraintConflict
```

### 62.5 Inference plane

```text
SynthesizedType
ExpectedType
Generalization
Instantiation
Skolemization
```

### 62.6 Interface/evidence plane

```text
InterfaceGoal
InterfaceCandidate
InterfaceEvidence
SpecificityRelation
```

### 62.7 Overload/coercion plane

```text
OverloadSet
OverloadCandidate
OverloadResolution
CoercionCandidate
CoercionPath
```

### 62.8 Default plane

```text
DefaultingRule
DefaultProfile
PreludeProfile
DefaultApplication
```

### 62.9 Coherence plane

```text
CoherenceClaim
ResolutionStabilityClaim
ElaborationEquivalence
AmbiguityWitness
```

### 62.10 Assurance plane

```text
ElaborationWitness
ResolutionCounterexample
ConstraintCounterexample
ElaborationDelta
```

## 63. Bounded first implementation pilot

The first pilot should be intentionally small while forcing the important distinctions.

Required initial artifacts:

```text
BindingId
Scope
ScopeGraph
Reference
ResolutionWitness

ElaborationEnvironment
ElaborationProfile

MetaVariable
TypeHole

TypeConstraint
InterfaceConstraint
ConstraintGraph
ConstraintSolution
ConstraintConflict

BidirectionalCheck

InterfaceEvidence
OverloadResolution
CoercionInsertion
DefaultingRule

ElaborationWitness
AmbiguityWitness
ElaborationDelta
```

The implementation does not need the full future solver ecosystem.

It does need explicit identities and independently checkable output.

## 64. Required fixtures

### Fixture A — textual name is not binding identity

Source:

```text
let x = 1
{
    let x = 2
    x
}
```

Expected:

```text
outer BindingId = B1
inner BindingId = B2
reference -> B2
```

Rename the inner `x`.

The required alpha/semantic relation remains satisfied.

### Fixture B — import ambiguity

```text
import A.foo
import B.foo
foo()
```

Both declarations are equally visible/applicable.

Expected:

```text
AMBIGUOUS
```

No import-order winner.

### Fixture C — qualification repairs ambiguity

```text
A.foo()
```

collapses the candidate set and produces a new resolution witness.

### Fixture D — bidirectional literal inference

```text
let x: u64 = 1
```

Expected type determines the literal type.

No numeric default is applied.

### Fixture E — default provenance

```text
let x = 1
```

under profile P1 defaults to `i64`.

Store:

```text
DefaultApplied(D1)
```

Compare to explicit:

```text
let x = 1i64
```

Core numeric semantics may match while elaboration provenance differs.

### Fixture F — profile changes meaning

P1:

```text
integer default = i64
```

P2:

```text
integer default = BigInt
```

Identical source receives different elaboration identity.

### Fixture G — unique overload

Candidates:

```text
foo(Int)
foo(String)
```

Argument is `Int`.

Exactly one candidate survives.

### Fixture H — overload ambiguity

Two generic candidates are applicable and neither is more specific.

Expected:

```text
AMBIGUOUS
```

not declaration-order selection.

### Fixture I — explicit interface evidence

Surface:

```text
x == y
```

Core contains explicit `Eq<T>` evidence.

The core checker validates the evidence.

### Fixture J — authority cannot be invented

An operation requires `NetworkCapability`.

No eligible capability exists.

Expected:

```text
UNRESOLVED_CAPABILITY
```

No ambient capability construction.

### Fixture K — overlapping evidence incoherent

Two applicable interface implementations produce evidence `E1` and `E2`.

If:

```text
E1 not_equivalent_R E2
```

expected:

```text
INCOHERENT
```

### Fixture L — overlapping evidence coherent

Same setup, but `E1 equivalent_R E2` is proven.

Multiple search paths may be accepted under a declared coherence profile.

### Fixture M — coercion ambiguity

```text
A -> B -> D
A -> C -> D
```

Both legal, no priority, no equivalence evidence.

Expected:

```text
AMBIGUOUS_COERCION
```

### Fixture N — coherent coercion

The two paths are proven equivalent under the required relation.

Elaboration can be coherent despite multiple paths.

### Fixture O — principal type

```text
lambda x. x
```

infers a principal polymorphic type in the supported fragment.

### Fixture P — no principal solution

Construct a bounded overloaded/subtyping example with several incomparable valid typings.

Require annotation rather than arbitrary choice.

### Fixture Q — effect-sensitive generalization

A mutable/effectful example demonstrates why unrestricted generalization is forbidden.

### Fixture R — open-world instance extension

Initial visible instance set:

```text
{I1}
```

An extension introduces another applicable `I2`.

The system must either reject the incoherent extension or make the old source ambiguous.

It may not silently change implementation.

### Fixture S — solver UNKNOWN

A configured solver cannot decide a constraint within its supported/bounded procedure.

Expected:

```text
UNKNOWN
```

not `UNSAT`.

### Fixture T — typed hole

```text
map(?, xs)
```

records an expected function type and available context.

Forge proposes a candidate and the ordinary checker validates it.

### Fixture U — realization variable survives elaboration

A `LocationVariable L` is intentionally deferred under RFC 0029.

Elaboration succeeds with `L` unresolved because it is not missing semantic content.

### Fixture V — search-order independence

Run two elaborators or the same elaborator under permuted candidate enumeration.

Where the profile promises coherence, results must remain semantically equivalent.

### Fixture W — constraint conflict slice

Generate a larger constraint graph with one inconsistency.

Produce a small conflict/unsat slice with source provenance.

### Fixture X — phase-sensitive reference

A compile-time binding and runtime binding share one spelling.

Resolution must respect phase identity rather than textual coincidence.

## 65. Micro-verifier opportunities

The first micro-verifier family should include narrow deterministic checks such as:

```text
BindingResolutionChecker
ScopePathChecker
AlphaBindingChecker
ConstraintSolutionChecker
FirstOrderUnificationChecker
BidirectionalTypingChecker
InterfaceEvidenceChecker
SpecificityChecker
OverloadResolutionChecker
CoercionPathChecker
DefaultApplicationChecker
CapabilityInsertionChecker
CoherenceRelationChecker
ElaborationWitnessChecker
RealizationDeferralChecker
```

These are intentionally smaller than a complete elaborator.

## 66. Forge study 1 — semantic overload versus realization choice

Provide source:

```text
process(data)
```

and candidates conceptually corresponding to:

```text
CPU
GPU
Distributed
```

The experiment must first classify whether those are:

1. semantically distinct overloads; or
2. realizations of one semantic operation.

If semantically distinct, RFC 0035 resolves or reports ambiguity.

If realization variants, RFC 0035 preserves the semantic operation and RFC 0029/RFC 0033 chooses later.

This is a required demonstration of the meaning-versus-realization boundary.

## 67. Forge study 2 — incomplete machine-generated code

Start with:

```text
fn transform(input: Input) -> Output {
    ?
}
```

The hole records:

```text
expected:
    Output
context:
    input : Input
    db : DatabaseCapability
    codec : CodecEvidence
```

Forge generates several candidates.

Expected outcomes should include:

```text
A: type valid but wrong effect/capability
B: type valid but missing authority
C: ambiguous interface resolution
D: fully elaborated and checkable
```

Only D advances.

## 68. Forge study 3 — constraint explanation and repair

Construct an expression producing dozens of constraints with a small contradictory subset.

The solver reports `UNSAT` with an explanation slice.

Forge proposes a targeted source repair based on the conflict origins.

Re-elaboration validates the repair.

This should demonstrate that compiler diagnostics become semantic repair artifacts.

## 69. Forge study 4 — independent elaborators

Run:

```text
Elaborator A: logic-programming oriented
Elaborator B: SMT/constraint oriented
```

on identical:

```text
source
environment
profile
```

If their core outputs satisfy the required relation:

```text
SemanticElaborationReproducibility = SUPPORTED
```

If not, emit an elaboration-coherence counterexample.

## 70. Forge study 5 — extension stability

Elaborate module M and store resolution/evidence witnesses.

Add an unrelated module or package N with bindings/instances.

Re-elaborate M.

Report:

- which references remained stable;
- which candidate sets changed;
- which changes are legal under the profile;
- whether any meaning changed;
- which witness dependencies caused invalidation.

This gives RFC 0036 concrete evolution machinery.

## 71. Forge study 6 — default profile evolution

Elaborate the same source under two explicit default profiles.

Demonstrate:

- profile identity affects elaboration identity;
- explicit annotations remain stable where intended;
- unannotated ambiguous/defaulted constructs may change;
- RFC 0036 can later require migration diagnostics.

## 72. Forge study 7 — AI-assisted disambiguation without AI authority

Provide intentionally ambiguous source.

An AI proposes an annotation.

The system records:

```text
Proposal source: AI
Proposal confidence: optional metadata
```

but only accepts the new meaning after ordinary elaboration/core checking establishes uniqueness/coherence.

This demonstrates that AI confidence never becomes language semantics.

## 73. Security considerations

Elaboration is security-sensitive because hidden resolution can accidentally create authority, choose unsafe interfaces, introduce coercions, or alter information flow.

Required properties include:

- implicit capability passing cannot create authority;
- plugin elaborators cannot bypass the core checker;
- imported evidence retains principal/provenance identity;
- defaults cannot weaken security obligations;
- open-world extensions cannot silently redirect privileged operations under a claimed coherence profile;
- solver-generated evidence is checked under RFC 0018/RFC 0025 trust policy where appropriate.

RFC 0024 remains authoritative for confidentiality/integrity properties.

## 74. Performance considerations

Elaboration can be expensive, especially with:

- higher-order search;
- interface evidence recursion;
- large scope graphs;
- SMT constraints;
- multiple solver families;
- dependent types.

RFC 0032 owns quantitative elaboration/search cost.

RFC 0035 requires only that performance optimization not silently change meaning.

Incremental constraint graphs, caching, parallel independent solving, and local inference are preferred architectural techniques.

## 75. Failure and recovery

Elaboration infrastructure itself may fail.

Possible operational outcomes include:

```text
SOLVER_UNAVAILABLE
BUDGET_EXHAUSTED
CACHE_INVALID
PLUGIN_FAILED
WITNESS_CHECK_FAILED
```

These are not semantic `UNSAT` unless a valid semantic contradiction witness exists.

RFC 0011 owns general failure semantics.

## 76. Serialization and canonicalization

RFC 0027 should provide canonical encodings for:

- binding identities;
- elaboration environments;
- profiles;
- constraints;
- witnesses;
- core artifacts.

This is needed for reproducible fingerprints and RFC 0031 provenance.

## 77. Assurance

RFC 0018 may define profiles such as:

```text
DevelopmentElaboration
    holes allowed
    heuristic solver UNKNOWN allowed

BuildElaboration
    no semantic holes
    all required constraints discharged
    witness check required

HighAssuranceElaboration
    independent elaborator or stronger solver evidence
    coherence checks
    canonical profile
```

RFC 0035 does not define a universal assurance level.

## 78. Proposed relation family

Useful RFC 0020-instantiated relations include:

```text
AlphaEquivalent
ResolutionEquivalent
ElaborationEquivalent<R>
ElaborationRefines<R>
CoercionPathsCoherent<R>
InterfaceEvidenceCoherent<R>
StableUnderExtension<E>
DefaultProfileCompatible<P1,P2>
SemanticElaborationReproducible<R>
```

The exact names may change.

## 79. Proposed status vocabulary

Potential elaboration-level dispositions include:

```text
UNIQUE
COHERENT_MULTIPLE
AMBIGUOUS
UNSAT
UNKNOWN
INCOMPLETE
```

Candidate-level dispositions may include:

```text
APPLICABLE
INAPPLICABLE
LESS_SPECIFIC
INCOHERENT
BLOCKED
UNKNOWN
```

These are distinct from RFC 0018 claim dispositions and verifier `PASS`/`FAIL`/`UNKNOWN`.

## 80. Non-goals

RFC 0035 does not:

- select the final surface grammar;
- define all types or values;
- redefine equality;
- define runtime memory semantics;
- define effects/capabilities;
- choose one universal solver;
- require Hindley–Milner inference for the full language;
- require typeclasses/traits as the final interface syntax;
- require scope graphs as the only possible implementation;
- require all realization variables to resolve during elaboration;
- allow performance heuristics to decide semantic ambiguity;
- define language evolution policy, which belongs to RFC 0036.

## 81. Implementation sequence

### Phase 1 — binding identity

Implement:

- `Scope`;
- `BindingId`;
- `Reference`;
- explicit lexical resolution;
- resolution witnesses;
- alpha-equivalence fixture.

### Phase 2 — bidirectional core elaboration

Implement:

- synthesis/checking;
- explicit expected types;
- type metavariables;
- first-order unification;
- explicit core output.

### Phase 3 — constraints

Implement:

- constraint identities;
- origins;
- dependency graph;
- SAT/UNSAT/UNKNOWN;
- small conflict explanations.

### Phase 4 — evidence/overload

Implement:

- one interface-evidence family;
- explicit dictionary/evidence insertion;
- overload candidates;
- specificity;
- ambiguity.

### Phase 5 — coercion/defaulting

Implement:

- explicit coercion insertion;
- coercion ambiguity/coherence fixtures;
- one versioned numeric default profile;
- default provenance.

### Phase 6 — holes and Forge

Implement:

- typed hole artifacts;
- context/constraints;
- Forge proposal flow;
- ordinary checker validation.

### Phase 7 — coherence/reproducibility

Implement:

- independent elaboration comparison;
- extension-stability fixture;
- semantic reproducibility relation.

## 82. RFC 1.0 research threshold

RFC 0035 should not be considered sufficiently established for the MNCS 1.0 research threshold until the prototype demonstrates all of the following.

### Binding and resolution

- versioned `Scope`, `BindingId`, `Reference`, and `ResolutionWitness` artifacts;
- textual renaming that preserves binding semantics under alpha-equivalence;
- lexical shadowing through explicit rules;
- ambiguous imports producing `AMBIGUOUS` rather than order-dependent selection;
- qualification producing a new unique resolution witness;
- phase-sensitive resolution;
- at least one scoped extension-stability claim.

### Elaboration and inference

- a bidirectional synthesis/checking core;
- expected-type-guided literal elaboration;
- explicit metavariable identity and states;
- first-order unification with occurs check;
- at least one principal type example;
- at least one valid fragment where no principal solution exists and an annotation is required;
- effect-aware generalization or a bounded fixture demonstrating why unrestricted generalization is unsound.

### Constraint architecture

- explicit generated constraints with source provenance;
- a versioned `ConstraintGraph`;
- solver results preserving `SAT`, `UNSAT`, and `UNKNOWN`;
- budget exhaustion/unsupported solving remaining `UNKNOWN` rather than `UNSAT`;
- one conflict explanation/unsat-core-style fixture;
- independently checked solved substitutions or certificates for the initial fragment.

### Interface evidence and authority

- one implicit interface/evidence goal elaborated to an explicit core evidence argument;
- one missing-capability case that proves elaboration cannot manufacture authority;
- overlapping evidence that is rejected as incoherent when candidate evidence differs materially;
- a coherent-multiple evidence case or an explicit refusal if the initial profile forbids overlap;
- bounded recursive evidence search with cycle/budget reporting.

### Overloads, coercions, and defaults

- unique overload selection from applicability/specificity rather than enumeration order;
- ambiguous overload fixture;
- explicit coercion insertion;
- ambiguous coercion paths rejected without priority/coherence evidence;
- coherent coercion paths accepted only under an explicit RFC 0020 relation;
- versioned defaulting profile;
- explicit-versus-defaulted provenance distinction;
- profile-change fixture showing identical source can receive a different elaboration identity.

### Holes and realization freedom

- typed semantic holes carrying expected type/context;
- development profile permitting holes without treating them as executable final code;
- Forge-generated hole candidate rechecked by the ordinary elaborator/core checker;
- at least one RFC 0029 or RFC 0019-style realization variable surviving RFC 0035 intentionally;
- a mechanical distinction between `UNSOLVED_SEMANTIC` and `REALIZATION_DEFERRED`.

### Coherence and reproducibility

- an `ElaborationWitness` containing binding, inference, evidence, coercion, default, and solver provenance for the implemented subset;
- search-order permutation that cannot change meaning under a coherent profile;
- independent elaboration implementations or strategies compared under an RFC 0020 relation;
- either semantic reproducibility support or a concrete coherence counterexample;
- cache/environment identity that changes when meaning-affecting import/instance/default inputs change.

### Forge capstone

Forge must demonstrate an end-to-end workflow where:

1. intentionally terse or incomplete source is parsed;
2. binding resolution produces explicit witnesses;
3. constraints and metavariables are generated;
4. one or more elaboration candidates are proposed by untrusted search;
5. an ambiguity or conflict is exposed structurally rather than hidden;
6. Forge proposes an explicit repair/annotation or hole filling;
7. a smaller checker validates the resulting explicit core;
8. intentional realization variables remain unresolved for later RFCs;
9. the final artifact records an elaboration witness suitable for RFC 0031 provenance.

The capstone must demonstrate that aggressive automation can reduce source burden without transferring semantic authority to the automation mechanism.

## 83. Relationship to RFC 0036

RFC 0035 deliberately makes the following versioned and inspectable:

- elaboration profiles;
- default profiles;
- resolution rules;
- coherence policies;
- import/prelude environments;
- solver contracts;
- core elaboration witnesses.

RFC 0036 can therefore reason about language/specification evolution in terms of explicit deltas rather than undocumented compiler behavior.

Questions for RFC 0036 include:

- when may a new default alter old source meaning?
- when may new visible instances create ambiguity?
- how are deprecated resolution rules represented?
- when must source be migrated or pinned to an older profile?
- what compatibility relation must hold between elaboration profiles?

RFC 0035 does not answer those evolution-policy questions.

## 84. Closing direction

A conventional compiler often treats elaboration as invisible convenience:

```text
source
  -> compiler magic
  -> typed IR
```

MNCS should instead make the hidden semantic choices inspectable:

```text
source
  -> bindings
  -> constraints
  -> candidate interpretations
  -> evidence search
  -> inserted arguments / coercions / defaults
  -> explicit core
  -> witness
  -> independent check
```

The defining sentence is:

> **Elaboration is the evidenced transformation from conveniently incomplete source to explicit semantic meaning; inference may remove syntactic burden, but it may never remove accountability for the choices it makes.**

The stronger architectural rule is:

> **MNCS should permit machines to infer aggressively while trusting inference minimally: every successful elaboration should terminate in an explicit core artifact whose bindings, inserted arguments, evidence, coercions, defaults, solved constraints, and intentionally deferred realization variables can be checked independently of the algorithm that discovered them.**
