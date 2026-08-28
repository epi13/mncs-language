# RFC 0013: Machine-Native Abstraction, Polymorphism, Interface, and Evidence Semantics

- **Status:** Draft — Track 1 (Explicit universal abstraction) and Track 12
  (bounded specialization/provenance) **implemented experimentally** in
  Source Profile 0.10; Tracks 2–11 and 13–15 remain future work
- **Target:** Cross-cutting 0.4–1.0
- **Depends on:** RFC 0001, RFC 0002, RFC 0003, RFC 0005, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012

## Summary

MNCS Language should define abstraction as an explicit semantic contract over the dimensions an implementation is permitted to hide, inspect, quantify over, specialize, or leave unresolved for later realization.

The central rule is:

> **Abstraction hides implementation, not semantic obligations. Polymorphism describes dimensions that remain unconstrained; specialization is a verified realization of those dimensions.**

A machine-authored program should be able to reuse logic without collapsing all forms of genericity into one trait, class, template, interface, or source-level inference mechanism. MNCS should distinguish at least:

- universal/parametric type polymorphism;
- constrained polymorphism through explicit evidence;
- value/index polymorphism;
- effect polymorphism;
- capability/authority polymorphism;
- usage/resource/quantity polymorphism;
- representation polymorphism;
- structural row polymorphism;
- existential/generative abstraction;
- controlled type-indexed relationships;
- introspective polymorphism that explicitly breaks parametricity; and
- realization holes intentionally left open for Forge or another optimizer.

The semantic core should make these dimensions explicit even when source syntax omits them. Inference is therefore a representation and elaboration convenience, not hidden semantic meaning.

The proposal synthesizes a deliberately small set of complementary theories:

- **relational parametricity** for stating what generic code cannot observe;
- **System F/Fω-style explicit polymorphism** as inspiration for a small elaboration core;
- **existential and generative abstraction** for hiding representation while preserving contracts and identity;
- **evidence/dictionary passing** for constrained polymorphism;
- **System-FC-style equality evidence/coercions** for explicit type equalities and safe casts;
- **row polymorphism** for open structural interfaces, effects, capabilities, and contextual requirements;
- **quantitative/graded polymorphism** for ownership, usage, authority, and resource dimensions;
- **effect polymorphism** for generic composition without collapsing all effects into one monolithic class;
- **bidirectional typing and explicit constraint solving** for practical elaboration without making complete inference a semantic requirement;
- **representation polymorphism** for preserving logical identity while physical representations remain selectable; and
- **verified specialization/refinement** so monomorphization, dictionary passing, vtables, JIT specialization, multi-versioning, partial evaluation, and proof-directed lowering are realizations rather than generic semantics.

Associated types/type functions and implicit resolution are useful but should remain bounded and evidence-bearing. They must not introduce unconstrained type-level rewriting or hidden semantic choices.

The key machine-native extension is that genericity should preserve **search space**. A generic artifact does not merely permit source reuse; it can intentionally leave type, representation, allocator, execution, effect-handler, or target dimensions open until more evidence, workload information, target facts, or optimization intent is available.

## Motivation

Conventional language abstraction mechanisms are usually optimized primarily for human convenience:

```text
class/interface
trait
type class
template
generic parameter
module signature
implicit argument
```

Each mechanism is useful, but conventional designs frequently conflate several distinct semantic questions:

- Which properties of a type may generic code observe?
- Which operations are available because of a constraint?
- Which implementation of a constraint was selected?
- Is that implementation choice semantically observable?
- Which effects are introduced or propagated?
- Which capabilities or authority are required?
- How many times may a value or resource be consumed?
- Is the abstraction independent of runtime representation?
- May an implementation inspect the type identity or representation?
- Is a hidden type nominally fresh or merely structurally compatible?
- Is a generic parameter logically quantified or intentionally unresolved for later optimization?
- Does specialization preserve the abstraction's semantics?
- Which proof/evidence objects justify equality, coercion, interface satisfaction, or specialization?

Human-oriented languages often hide answers to these questions in compiler conventions, coherence rules, source syntax, implementation heuristics, or undocumented assumptions.

That is a poor foundation for a machine-native language.

MNCS should make the semantic dimensions explicit so autonomous generators, Forge, verifiers, optimizers, runtimes, and backends can reason about abstraction directly.

For example, a generic algorithm may semantically state:

```text
forall T
forall representation R
forall effect-row E
forall usage q

requires Ordered<T>
requires Represents<R, T>
requires Movable<R>

body may observe T only through Ordered<T>
body may observe R only through load/store/iterate
body propagates E
body consumes input with usage q
```

The machine may then prove that the algorithm is representation-parametric and compare multiple realizations:

```text
AoS
SoA
packed SoA
SIMD blocked
NUMA-local
GPU device buffer
compressed representation
```

without changing the logical abstraction.

Likewise, a constrained operation may appear in source as:

```text
sort(xs)
```

while canonical semantics records:

```text
Instantiation {
    abstraction = sort
    type_arguments = [Widget]
    selected_evidence = [WidgetOrdering#93]
    effect_arguments = [{}]
    authority_arguments = [...]
    representation_arguments = [R17]
    unresolved_obligations = []
}
```

Source omission does not imply semantic omission.

## Design goals

RFC 0013 should make the following possible.

1. **Explicit abstraction dimensions.** Type, value, effect, authority, usage, representation, protocol, and contextual polymorphism should be distinguishable.
2. **Parametricity as a semantic property.** Generic code should carry machine-usable facts describing what it cannot observe.
3. **Explicit constrained evidence.** Constraints should elaborate to concrete evidence, capabilities, proofs, or unresolved obligations rather than compiler folklore.
4. **Evidence identity.** Selected implementations of semantic constraints must have stable identities when their choice can affect behavior.
5. **No hidden obligations.** Abstraction may hide implementation structure but must not hide required effects, authority, assumptions, failure behavior, or proof obligations.
6. **Existential/generative abstraction.** Components should be able to hide representation and maintain fresh nominal identity where required.
7. **Open structural composition.** Effects, capabilities, interfaces, and contexts should support row-like polymorphism where appropriate.
8. **Quantitative abstraction.** Reusable code should be able to abstract over usage/ownership/resource grades without prematurely requiring copying or exclusivity.
9. **Representation freedom.** Logical type identity should not imply one physical representation unless representation observability is explicitly required.
10. **Controlled introspection.** Type or representation inspection should require explicit semantic authority and should invalidate corresponding parametricity claims.
11. **Verified specialization.** Specialized implementations must preserve or refine generic semantics and retain provenance to the generic definition and evidence set.
12. **Search-space preservation.** Generic artifacts should be able to leave realization dimensions unresolved for later machine optimization.
13. **Inference independence.** Complete inference is not required for semantic correctness. Canonical artifacts may be fully explicit.
14. **Bidirectional elaboration.** Human and agent source representations may use inference, but elaboration should produce explicit canonical obligations/evidence.
15. **Coherence as semantics.** Implicit resolution must not select semantically distinct evidence without an explicit coherence or disambiguation rule.
16. **Controlled type-level computation.** Associated types/type functions may exist only under termination, confluence, ambiguity, and evidence constraints appropriate to the active assurance level.
17. **Proof integration.** Equality/coercion/constraint evidence should compose with RFC 0007 without requiring a large new trusted kernel.
18. **HIR/SSA preservation.** Instantiation, evidence selection, representation bindings, and specialization provenance must remain traceable through lowering when semantically relevant.
19. **Independent implementation.** The abstraction/elaboration rules should be precise enough for independent frontends and checkers to agree on canonical output.
20. **Machine inspectability.** Machines should be able to query why an abstraction is parametric, which dimensions are open, which evidence was selected, and what specialization freedom remains.

## Non-goals

This RFC does not:

- declare MNCS to use Rust traits, Haskell type classes, C++ templates, Java generics, or ML modules wholesale;
- require one final human-readable generic syntax;
- require full Hindley–Milner inference;
- require every abstraction dimension to be inferred;
- require runtime dictionary passing;
- require monomorphization;
- require nominal or structural typing exclusively;
- permit unconstrained runtime type reflection;
- permit type inspection while retaining false parametricity claims;
- require literal fractional permissions for quantitative polymorphism;
- require associated types or open type functions in the initial executable subset;
- make implicit resolution authoritative when multiple semantically distinct candidates exist;
- place arbitrary constraint solving or type-level computation inside the proof kernel;
- require every generic artifact to be specialized;
- claim representation independence when code observes representation identity, size, layout, alignment, or other exposed details; or
- permit specialization to silently widen effects, authority, failure surface, or observations.

Unsupported abstraction properties may remain `UNKNOWN`, require explicit arguments/evidence, select conservative realizations, or reject promotion under the active assurance profile.

## Constitutional principles

### Abstraction hides implementation, not obligations

An interface may hide how something is implemented. It may not erase facts such as:

```text
requires Network
requires Write<DeviceD>
may_fail ExternalUnknownCommit
may_allocate <= 4 KiB
assumes Protocol<P>
```

Those facts remain visible to the semantic graph and authority/effect/proof systems even if callers cannot inspect internal representation.

### Inference may hide syntax, never semantics

A source representation may omit generic/evidence arguments for compactness.

Canonical semantics should retain every argument that affects meaning:

```text
Source:
    sort(xs)

Canonical:
    sort<
        T = Widget,
        OrderingEvidence = WidgetOrdering#93,
        Effects = {},
        Authority = {},
        Representation = R17
    >(xs)
```

Inference is therefore an elaboration transformation.

### Parametricity is optimization evidence

If an abstraction is proven parametric over dimension `D`, transformations may exploit the fact that the abstraction cannot distinguish changes to `D` except through explicitly declared operations.

For example, representation parametricity can authorize storage-layout search.

### Genericity preserves search space

A generic artifact may intentionally preserve unresolved realization dimensions rather than prematurely selecting them.

Conceptually:

```text
Algorithm<
    T,
    Representation = ?,
    Allocator = ?,
    ParallelStrategy = ?,
    EffectHandler = ?
>
```

Some parameters are semantic universals; others are bounded realization holes. Their roles must be explicitly distinguished.

### Implicit syntax does not imply implicit identity

Evidence may be omitted in source syntax, but canonical semantics must identify which evidence object or coherence domain justifies the operation when that identity can affect behavior.

## Selected theoretical synthesis

### Relational parametricity

Relational parametricity should be the primary theory for describing what generic code cannot observe.

Conceptually:

```text
forall T. T -> T
```

cannot manufacture or inspect arbitrary `T` values except through operations explicitly supplied by the abstraction environment.

MNCS should generalize this idea across multiple dimensions:

```text
ParametricOver<Type<T>>
ParametricOver<Representation<R>>
ParametricOver<EffectRow<E>>
ParametricOver<Usage<Q>>
```

A parametricity claim is evidence-bearing. It may be established by a type-theoretic theorem, verifier, syntactic discipline, abstraction checker, or another declared method.

The claim has dependencies and can become stale if the abstraction gains operations that inspect the relevant dimension.

Examples of operations that may break type parametricity include:

```text
InspectTypeIdentity<T>
ReflectFields<T>
TypeCase<T>
```

Examples that may break representation parametricity include:

```text
InspectRepresentation<R>
ObserveSize<R>
ObserveAlignment<R>
ObserveLayout<R>
ExposeRawAddress<R>
```

The capability to perform such inspection must be explicit.

### Explicit universal polymorphism

The canonical abstraction core should represent universal parameters explicitly.

Conceptually:

```text
forall T : Type.
forall F : Type -> Type.
forall n : Nat.
...
```

System F/Fω provides useful inspiration for keeping this core small and compositional.

MNCS does not commit to literal System Fω syntax or exact metatheory. RFC 0007 dependent types, RFC 0008 effects/resources, and RFC 0009 authority already require a richer environment.

The useful rule is that many surface mechanisms should elaborate into one explicit abstraction representation rather than creating unrelated generic, module, trait, and interface semantics.

### Distinct polymorphism dimensions

RFC 0013 should not treat all variables as equivalent generic type parameters.

Initial parameter classes should include:

```text
TypeParameter<T>
ValueParameter<n : Nat>
TypeConstructorParameter<F : Type -> Type>
EffectRowParameter<E>
CapabilityRowParameter<A>
UsageParameter<Q>
RepresentationParameter<R>
ContextParameter<C>
ProtocolParameter<P>
EvidenceParameter<V>
RealizationHole<H>
```

These dimensions differ in what they may affect and how they are resolved.

#### Parametric type polymorphism

```text
forall T.
```

The implementation learns nothing about `T` beyond universally valid type structure and explicit constraints.

#### Constrained polymorphism

```text
forall T where Ordered<T>.
```

The implementation may use only semantics exposed by selected `Ordered<T>` evidence.

#### Value/index polymorphism

```text
forall n : Nat.
Vector<T,n>
```

The abstraction may depend on a typed value/index.

#### Effect polymorphism

```text
forall E : EffectRow.
```

The abstraction may preserve, transform, restrict, or compose unknown effects according to its contract.

#### Authority polymorphism

```text
forall A : CapabilityRow.
```

The abstraction can operate without requiring one closed authority set.

#### Usage/resource polymorphism

```text
forall q : Usage.
```

The abstraction can remain agnostic about copyability, linearity, affinity, borrow grade, or resource quantity where the body permits.

#### Representation polymorphism

```text
forall R where Represents<R,T>.
```

The abstraction depends on logical `T` but remains independent of physical representation except through declared representation operations.

#### Realization holes

```text
exists optimization_choice H within Envelope<E>.
```

A realization hole is not ordinary logical polymorphism. It is an explicitly bounded implementation choice left for a later selection stage.

### Constrained polymorphism and evidence passing

Type-class-style evidence passing provides the key model: a constraint should elaborate to an explicit semantic argument.

Source:

```text
sort<T>(xs)
where Ordered<T>
```

Canonical meaning:

```text
sort<T>(
    ordered_evidence : Evidence<Ordered<T>>,
    xs : Sequence<T>
)
```

MNCS should generalize evidence beyond conventional method dictionaries.

Examples include:

```text
Eq<T>
Ord<T>
Hash<T>
Serializable<T,Format>
Movable<R>
StableAddress<R,Scope>
EffectSafe<F,E>
RepresentationCompatible<R,T>
ProtocolParticipant<T,P>
ConstantTime<Operation,TargetClass>
ProofAvailable<Property>
```

Evidence objects may contain operations, proof references, verifier identities, assumptions, associated semantic facts, or combinations thereof.

The canonical graph records their identity and dependency set.

### Equality evidence and coercions

Non-syntactic type equalities should not become implicit compiler trust.

Conceptually:

```text
Evidence<A == B>
```

can authorize a coercion:

```text
coerce<A,B>(equality_evidence, value)
```

The evidence may be kernel-checkable, certificate-backed, verifier-produced, assumed under an unsafe/foreign boundary, or unresolved.

Different evidence classes retain RFC 0002/RFC 0007 trust distinctions.

A coercion without adequate evidence must be rejected or remain outside the safe language subset.

### Existential abstraction

Existentials allow an implementation to hide a type while exposing operations and properties.

Conceptually:

```text
exists State.
{
    initial : State,
    step : State -> Input -> State,
    observe : State -> Output
}
```

Consumers cannot depend upon the hidden representation except through the exported interface.

This enables implementation replacement while preserving semantic identity when refinement evidence establishes the interface contract.

### Generative abstraction

Some hidden types must be not merely unknown, but freshly distinct.

Examples include:

```text
security_domain
protocol_session
address_space
resource_namespace
cryptographic_key_domain
```

A generative abstraction creates a fresh nominal identity even if another abstraction has structurally identical representation.

Conceptually:

```text
FreshTypeIdentity<K>
```

must not unify with another independently generated `FreshTypeIdentity<K2>` without explicit equality evidence.

This is important for confinement and authority boundaries.

### Structural and nominal semantics

MNCS should support both structural and nominal relationships.

Structural requirements answer:

```text
does this object/evidence provide these operations/properties?
```

Nominal identity answers:

```text
is this the specific semantic domain/protocol/resource/type identity required here?
```

They must not be conflated.

A structural interface can support flexible machine composition, while nominal identities preserve domains where accidental substitution would be unsound.

### Row polymorphism

Open rows should be investigated for effects, capabilities, contexts, and structural interfaces.

Examples:

```text
Effects {
    read<File>,
    allocate,
    ... E
}
```

```text
Capabilities {
    Read<A>,
    Write<B>,
    ... C
}
```

```text
Interface {
    compare : T x T -> Ordering,
    hash : T -> Hash,
    ... R
}
```

A consumer can require a minimum row without closing the entire set.

Row identity and row equality must be canonicalized deterministically.

Unknown row tails cannot be silently assumed empty.

### Effect polymorphism

Generic composition should preserve effects rather than collapse effectful behavior into one universal type.

Conceptually:

```text
map :
    forall T,U,E.
    (T -> Computation<U,E>)
    -> Sequence<T>
    -> Computation<Sequence<U>,E>
```

The exact effect expression may include transformations, unions, masking, or handler effects under RFC 0008/RFC 0012.

An abstraction may state whether it:

```text
preserves E
extends E
handles subset H of E
forbids effects F
```

These are semantic claims, not source annotations only.

### Authority polymorphism

Authority should support analogous open composition.

For example:

```text
forall A.
fn audit(x)
requires { Read<Log>, ... A }
```

A generic abstraction must not amplify authority merely because additional capabilities are present in the ambient environment.

Authority polymorphism therefore composes with non-amplification rules from earlier RFCs.

### Quantitative and usage polymorphism

Reusable code should be able to quantify over usage disciplines.

Conceptually:

```text
forall q : Usage.
fn transform(x : Use<T,q>) -> Use<U,q>
```

The exact usage algebra remains delegated to the type/resource model, but candidate dimensions include:

```text
linear
affine
unrestricted
shared-read
exclusive-write
bounded-count(n)
borrowed(scope)
resource-grade(q)
```

The abstraction body determines which grades are legal.

If the body duplicates a resource, the checker must not pretend it remains polymorphic over linear usage.

This prevents source convenience from introducing hidden `Copy`-like requirements.

### Representation polymorphism

Logical type identity must remain distinct from representation identity.

Conceptually:

```text
TypeIdentity<T> != RepresentationIdentity<R>
```

An abstraction may quantify over representations satisfying a contract:

```text
forall R.
requires Represents<R,T>
requires Alignment<R> >= 16
requires Movable<R>
```

but not otherwise inspect `R`.

Potential realizations include:

```text
boxed
unboxed
AoS
SoA
packed
SIMD blocked
compressed
NUMA-local
shared-memory
GPU/device
persistent
```

Representation changes may be legal without changing logical identity when all observable semantics remain preserved.

### Representation parametricity as optimization authority

If an abstraction is proven representation-parametric, Forge may search representation space without re-proving unrelated functional semantics from scratch.

The abstraction contract defines its permitted representation observations.

For example:

```text
representation interface {
    load
    store
    iterate
}
```

If the body does not observe layout, size, address, alignment beyond declared constraints, or other representation identity, then multiple physical layouts may realize the same abstraction.

This creates a powerful optimization relationship:

```text
logical algorithm
    + representation-parametricity evidence
    + RFC 0006 objectives
    + RFC 0009 memory constraints
        ->
Forge representation search
```

### Type/representation introspection

Sometimes non-parametric specialization is intentional.

MNCS should support it explicitly rather than pretending the abstraction remains parametric.

Capabilities may include:

```text
InspectTypeIdentity<T>
InspectRepresentation<R>
InspectLayout<R>
```

Acquiring or using such authority changes the abstraction's semantic facts.

A function that branches on `T` is not parametric over `T`.

A function that branches on `R` is not representation-parametric over `R`.

These facts should be mechanically visible to optimizers and verifiers.

### Associated types and controlled type functions

Some abstractions naturally define dependent type relationships.

Conceptually:

```text
Collection<T> {
    associated IteratorType
}
```

or:

```text
Device<D> {
    associated BufferRepresentation
}
```

RFC 0013 should reserve this capability but constrain it.

A type-level function should carry metadata such as:

```text
TypeFunction {
    identity
    inputs
    output_kind
    reduction_rules
    termination_policy
    confluence_policy
    overlap_policy
    evidence
}
```

The language must not assume arbitrary type-level computation terminates or is confluent.

Early executable subsets should prefer closed, terminating, deterministic relations.

### Bidirectional elaboration

MNCS should not require global complete inference.

A practical elaborator may use bidirectional judgments conceptually like:

```text
Gamma |- expression => Type ; Constraints
Gamma |- expression <= Type ; Constraints
```

Constraint solving then produces:

```text
Evidence
Obligations
UNKNOWN
```

The elaboration pipeline is therefore:

```text
source / agent representation
    -> parse/structure
    -> synthesize/check
    -> constraint generation
    -> evidence resolution
    -> obligation generation
    -> explicit canonical abstraction core
```

Different source representations may use different inference aggressiveness while converging to the same canonical semantics.

### Inference as compression

Inference is a compression mechanism for representations, not a language-semantic requirement.

A machine-generated canonical artifact may be more explicit than a human source file because explicitness can improve verification, repair locality, evidence invalidation, and reproducibility.

The syntax tournament in RFC 0005 should therefore measure both token density and omission/repair costs rather than assuming fewer annotations are always better.

### Coherence and evidence identity

Implicit resolution must be coherent.

Suppose two candidates satisfy:

```text
Serialize<T>
```

but produce different wire formats.

Those candidates are not semantically interchangeable merely because their source constraint name matches.

Canonical semantics must distinguish them:

```text
Serialize<T,FormatA>#17
Serialize<T,FormatB>#91
```

Likewise, multiple orderings, hash schemes, floating comparison policies, protocol implementations, or security authorities may satisfy superficially similar interface shapes while changing behavior.

A coherence model should therefore include:

```text
EvidenceIdentity
CoherenceDomain
SelectionPolicy
AmbiguityState
```

If multiple semantically distinct candidates are available and no policy/evidence establishes equivalence, resolution is ambiguous rather than silently compiler-chosen.

### Explicit instantiation

Generic instantiation should be a semantic artifact.

Conceptually:

```text
Instantiation {
    identity
    abstraction_identity

    type_arguments
    value_arguments
    effect_arguments
    authority_arguments
    usage_arguments
    representation_arguments
    context_arguments

    selected_evidence
    equality_evidence

    discharged_constraints
    unresolved_constraints

    assumptions
    generated_obligations
}
```

An instantiation may exist without immediate specialization.

Its identity should include all semantically relevant substitutions/evidence but should not include irrelevant source formatting.

### Specialization is a verified transformation

Generic semantics should not require one specialization strategy.

Possible realizations include:

```text
monomorphization
dictionary passing
vtable/table dispatch
closed dispatch
partial evaluation
JIT specialization
multi-versioning
shared erased implementation
proof-directed specialization
```

A specialization is represented conceptually as:

```text
Specialization {
    identity
    generic_identity
    instantiation_identity

    selected_representation
    selected_realization_choices

    transformed_artifact

    assumptions
    obligations
    refinement_evidence
}
```

The specialization must not introduce new externally observable behavior outside the abstraction's permitted refinement/relaxation envelope.

It must not silently widen:

```text
effects
authority
failure surface
external observations
resource bounds
portability requirements
```

unless the semantic contract explicitly permits that change and the resulting artifact has a distinct semantic identity.

### Genericity as bounded optimization space

RFC 0013 extends genericity beyond source reuse.

An abstraction may intentionally expose bounded realization parameters:

```text
RealizationHole<Representation> {
    require Represents<R,T>
    prefer cache_local
}

RealizationHole<Allocator> {
    require alignment >= 64
}

RealizationHole<ParallelStrategy> {
    require deterministic_result
}
```

These are not universally quantified semantic variables in the same sense as `forall T`.

They describe a search envelope.

Forge may propose candidates within the envelope and compare them using RFC 0006 objectives and evidence policies.

The unresolved hole remains visible until a realization is selected.

### Specialization lattice

A generic artifact may undergo multiple levels of specialization:

```text
Generic
  -> type-instantiated
  -> evidence-instantiated
  -> target-specialized
  -> representation-specialized
  -> workload-specialized
```

Each level should retain provenance and permit evidence invalidation when relevant dependencies change.

A later specialization does not erase its parent relationship.

### Abstraction envelope

The central semantic object proposed by this RFC is an abstraction envelope.

Conceptually:

```text
Abstraction {
    identity

    parameters {
        types
        values
        type_constructors
        effects
        capabilities
        usages
        representations
        contexts
        protocols
        evidence
        realization_holes
    }

    constraints

    provided_interface
    hidden_structure

    observable_properties
    nonobservable_properties

    parametric_dimensions
    introspectable_dimensions

    effect_contract
    authority_contract
    resource_contract
    failure_contract

    evidence_requirements
    generated_obligations

    specialization_envelope
    portability_envelope

    assumptions
    evidence
}
```

This shape is conceptual rather than a committed wire format.

### Interface semantics

An interface should be more than a method list.

Conceptually:

```text
InterfaceContract {
    identity

    operations
    associated_relations

    required_effects
    required_authority
    resource_requirements

    laws
    assumptions
    obligations

    coherence_domain
    evidence_policy
}
```

A law may include properties such as:

```text
associative
commutative
idempotent
total
pure
constant_time
order_consistent
serialization_round_trip
```

Method presence alone does not establish the law.

Laws require evidence under RFC 0002/RFC 0007.

### Interface satisfaction

Satisfaction is a relationship between an implementation/evidence object and an interface contract.

Conceptually:

```text
Satisfies<EvidenceX, InterfaceY>
```

The relationship may depend on target, representation, scope, assumptions, or proof identity.

A structural match of operation names/types may establish only shape compatibility, not semantic law satisfaction.

### Capability-like interfaces

Many conventional interfaces can be represented more precisely as capabilities or evidence rather than nominal traits.

For example:

```text
Read<T>
Write<T>
Serialize<T,F>
Compare<T,O>
```

The abstraction system should reuse earlier capability/evidence semantics rather than inventing a duplicate trait mechanism.

### Abstraction and security

Abstraction boundaries should support least authority.

An implementation may possess broader authority than a consumer interface exposes.

However, hidden authority still remains part of the implementation's semantic contract and audit surface.

A consumer receives only the capabilities exported through the interface.

Existential/generative identities can prevent callers from forging or mixing domain-specific resources.

### Abstraction and failure

Failure behavior is part of abstraction semantics.

An interface that hides implementation must still expose the failure states callers need to handle.

For example:

```text
Operation<T> {
    returns U
    may_fail { Timeout, Unavailable, MAY_HAVE_APPLIED }
}
```

Changing implementation from local memory to remote service cannot silently preserve the same abstraction identity if it introduces semantically new failure/uncertainty behavior outside the existing contract.

### Abstraction and concurrency

An abstraction may expose concurrency properties such as:

```text
thread_safe
linearizable_against Spec
requires_exclusive_access
commutative(op1,op2)
schedule_independent
```

These are semantic laws requiring evidence rather than nominal marker traits.

Specializations must preserve those properties.

### Abstraction and RFC 0012 execution

Instantiation/elaboration should ultimately produce ordinary RFC 0012 executable semantics.

Abstraction itself is not a second execution universe.

Generic definitions elaborate to parameterized semantic computations.

Selected evidence/capabilities become explicit inputs or semantic facts.

Specialized realizations refine the same observable behavior envelope.

### Abstraction and RFC 0007 proof core

Proof-relevant parameters may include:

```text
Proof<P>
Evidence<Equality<A,B>>
Evidence<Satisfies<X,I>>
```

Kernel-checkable terms remain in RFC 0007's trusted stratum.

Dictionary objects, verifier results, empirical evidence, and assumptions remain outside the kernel unless translated into a checkable proof/certificate.

RFC 0013 therefore depends on proof evidence without expanding the kernel to include arbitrary instance resolution or optimization.

## Canonical semantic objects

### Parameter declaration

```text
AbstractionParameter {
    identity
    class
    kind_or_type
    variance?
    quantity?
    constraints
    observability
}
```

### Constraint

```text
Constraint {
    identity
    proposition_or_interface
    dependencies
    required_evidence_class
    scope
}
```

### Evidence binding

```text
EvidenceBinding {
    constraint_identity
    evidence_identity
    method_class
    assumptions
    freshness
}
```

### Parametricity fact

```text
ParametricityFact {
    abstraction_identity
    dimension
    allowed_observations
    forbidden_observations
    scope
    dependencies
    evidence
}
```

### Realization hole

```text
RealizationHole {
    identity
    class
    candidates_or_generator?
    hard_requirements
    preferences
    search_budget
    authority_budget
    evidence_policy
}
```

These shapes remain provisional.

## Constraint resolution

Constraint resolution should produce one of:

```text
RESOLVED(evidence)
AMBIGUOUS(candidates)
UNSATISFIED
UNKNOWN
```

`UNKNOWN` is not `UNSATISFIED`.

`AMBIGUOUS` is not permission to select arbitrarily unless the active coherence policy explicitly allows equivalent candidates and equivalence itself is evidenced.

Resolution results should be deterministic for a fixed canonical environment and policy.

## Coherence domains

A coherence domain defines where uniqueness or equivalence of evidence is required.

Possible examples:

```text
GlobalCoherence<Ord<T>>
ModuleLocalCoherence<Serialize<T,F>>
ExplicitOnly<SecurityPolicy<T>>
```

The final taxonomy remains research work.

The semantic requirement is that hidden instance selection cannot alter behavior unpredictably.

## Variance and subtyping

RFC 0013 should reserve explicit treatment of variance and subtyping where useful, but avoid making a large nominal subtyping hierarchy foundational.

Structural inclusion, capability attenuation, existential abstraction, and refinement may solve many cases more cleanly.

If subtyping is introduced, variance must be derived/checked with effects, authority, mutability, and representation constraints in view.

Unknown variance cannot authorize coercion.

## Higher-rank polymorphism

Higher-rank polymorphism should be semantically representable even if source inference requires annotations.

For example:

```text
fn use_twice(
    f : forall T. T -> T
)
```

The canonical representation is explicit.

The elaborator may require type annotations in ambiguous source forms.

This reinforces the principle that inference convenience must not constrain the semantic core.

## Higher-kinded polymorphism

Abstraction over type constructors should be supported at the semantic level where needed:

```text
forall F : Type -> Type.
```

The initial executable subset may restrict kind complexity and inference.

Kind/universe rules must compose with RFC 0007 limits.

## Dependent abstraction

RFC 0007 already permits dependent relationships in the logical core.

RFC 0013 should allow executable abstractions indexed by values when required:

```text
forall n : Nat.
Buffer<T,n>
```

However, runtime value dependence, proof erasure, and executable relevance must be explicit.

The abstraction system must distinguish:

```text
compile/proof-time index
runtime-relevant index
erased evidence
runtime evidence
```

## Evidence erasure

Evidence may be semantically relevant without requiring runtime representation.

An evidence-erasure transformation requires proof that the runtime behavior does not depend upon the erased representation except through already-established facts.

Examples include proof terms used only to authorize bounds-check removal.

Other evidence, such as a comparison dictionary containing executable methods, cannot be erased unless specialized/compiled away with equivalent behavior.

Erasure is therefore a verified transformation.

## Dictionary/evidence specialization

An evidence-passing generic may be specialized by inlining selected evidence operations.

For example:

```text
sort<T>(ordering, xs)
```

may specialize `ordering.compare` into direct target instructions.

The result remains linked to the evidence identity so changing ordering semantics invalidates dependent specializations.

## Representation specialization

A representation-polymorphic abstraction may be specialized to one layout.

The specialization must prove:

```text
Represents<R,T>
```

and any required refinement/equivalence claims.

Layout-dependent backend promises must be bound to the selected representation identity.

## Multi-versioning

An abstraction may admit multiple simultaneously valid specializations selected at runtime or load time.

Conceptually:

```text
SpecializationSet {
    baseline
    variants
    dispatch_condition
    dispatch_evidence
}
```

The dispatch itself must preserve the abstraction contract.

Target feature detection is an observation and must obey RFC 0011 external-observation semantics where relevant.

## Partial evaluation

Partial evaluation is a specialization technique, not generic semantics.

Known parameters may be propagated and computations reduced if RFC 0012 refinement preserves observable behavior.

A partial evaluator must not execute effects at specialization time unless staging semantics explicitly permits it in a later RFC.

## Specialization and Forge

Forge should consume abstraction metadata directly.

Useful queries include:

```text
which dimensions remain parametric?
which realization holes remain open?
which evidence objects constrain specialization?
which representations are legal?
what is the optimization objective?
which proofs will become stale if this specialization changes?
```

Forge may propose:

```text
representation substitution
evidence specialization
target specialization
parallel strategy
allocator strategy
closed dispatch
vectorization strategy
```

but promotion requires the same bounded observe–propose–verify–compare–promote discipline established elsewhere in MNCS.

## HIR and SSA requirements

HIR/SSA should preserve or trace at least:

- generic definition identity;
- instantiation identity;
- selected evidence identities;
- type/effect/authority/representation substitutions when semantically relevant;
- erased-vs-runtime evidence decisions;
- specialization provenance;
- refinement obligations;
- unresolved realization holes that survive to later stages; and
- assumptions introduced by specialization.

The IR need not retain high-level generic syntax after specialization, but traceability must remain available.

## Semantic identity

Generic identity should be distinct from specialization identity.

Conceptually:

```text
GenericSemanticIdentity
InstantiationIdentity
SpecializationIdentity
RealizationIdentity
```

Two instantiations with semantically equivalent but differently formatted source are the same only if canonical substitutions/evidence identities agree under the defined equivalence relation.

Two specializations may have different implementation identities while refining the same instantiation.

## Evidence invalidation

Changes to evidence invalidate only dependent artifacts.

For example, changing:

```text
OrderingEvidence<Widget>
```

should invalidate specializations and proofs that consume that ordering, but should not invalidate unrelated representation-parametric proofs that never reference ordering.

This aligns with RFC 0002/RFC 0004 dependency-aware invalidation.

## Failure modes of abstraction

The language/toolchain should diagnose at least:

```text
AMBIGUOUS_EVIDENCE
MISSING_EVIDENCE
STALE_EVIDENCE
PARAMETRICITY_VIOLATION
REPRESENTATION_OBSERVATION_VIOLATION
AUTHORITY_AMPLIFICATION
EFFECT_WIDENING
USAGE_VIOLATION
UNPROVEN_COERCION
GENERATIVE_IDENTITY_MISMATCH
NONTERMINATING_TYPE_FUNCTION
NONCONFLUENT_TYPE_FUNCTION
UNRESOLVED_REALIZATION_HOLE
SPECIALIZATION_REFINEMENT_FAILURE
```

Diagnostics should identify the exact abstraction dimension and evidence dependencies involved.

## Negative semantic requirements

The following must not be accepted as ordinary safe abstraction behavior.

### Hidden type reflection

A function claimed parametric over `T` may not inspect `T` through an undeclared host/runtime reflection facility.

### Hidden representation observation

A representation-parametric function may not branch on object size/layout/address unless the relevant introspection operation is explicitly part of the abstraction.

### Evidence laundering

A structurally compatible method table must not satisfy semantic laws merely because operation signatures match.

### Arbitrary implicit choice

Two semantically distinct valid evidence objects must not be resolved by compiler accident when no coherence rule selects one.

### Hidden Copy requirement

An abstraction claimed usage-polymorphic may not duplicate linear/affine inputs and then silently insert a copy requirement.

### Effect hiding

A generic interface may not erase effects introduced by its implementation.

### Authority hiding

A public abstraction may hide implementation details, but its required authority must remain visible to semantic authority checking.

### Unsafe equality

Type equality must not be inferred from representation coincidence.

### Generativity collapse

Two fresh abstract identities must not unify merely because their representations are equal.

### Specialization drift

A faster specialization must not introduce behavior outside the generic abstraction's observable envelope.

### Stale parametricity

If type/representation introspection is later added to an abstraction, old parametricity evidence must become stale.

### Unbounded type-level computation

The checker must not silently diverge because a user/agent introduced unconstrained recursive type functions.

## Initial experimental tracks

RFC 0013 should advance through bounded experiments rather than committing the entire abstraction system simultaneously.

### Track 1 — Explicit universal abstraction

Implement a small explicit `forall`-style semantic abstraction with type instantiation.

Acceptance criteria:

- canonical identity for generic definition;
- canonical identity for instantiation;
- deterministic checking;
- accepted/rejected fixtures; and
- HIR traceability.

### Track 12 — Generic specialization and provenance
Deterministic monomorphization with canonical instantiation identity,
specialization provenance, and call-site rewriting.

Acceptance criteria:

- canonical instantiation identity from canonical arguments;
- specialization identity linked to generic definition and instantiation;
- call-site rewriting to specialized functions;
- provenance preserved through HIR/SSA/selected-SSA.

## Implementation Status (Track 1 + Track 12)
**Track 1 — Explicit universal abstraction: IMPLEMENTED (Profile 0.10)**

- Explicit generic parameters (`<T>`, `<N: Nat>`) on functions;
- Generic calls with explicit arguments (`f<4>`, `f<Point, 4>`);
- Canonical generic definition identity anchored to declaring module;
- Canonical instantiation identity from declaration-ordered canonical arguments (`type:...|value:...`);
- Deterministic specialization with canonical substitution (type params → `GenericParam`, value params → concrete `u32`);
- Specialization identity (`specialization:<generic_id>::<hash>`) and instantiation identity (`instantiation:<generic_id>::<hash>`);
- Call-site rewriting in concrete functions (`f<4>` → `f__spec_<hash>`) with provenance preserved in `generic_args`, `instantiation`, `specialization` fields;
- Canonical identity for generic parameters, instantiations, specializations in semantic identity map;
- Generic specialization provenance in `Program.generic_specializations`, `HighLevelIr.generic_specializations`, `SsaModule.generic_specializations`;
- Recursive instantiation detection with diagnostic `MNE227` (expanding substitution) and cycle limit;
- HIR/SSA provenance for generic parameters, instantiations, specializations through `GenericParam` identity kind;
- `sequence-index-bounds` and `view-range-valid` obligations correctly handled in generic context;
- Negative diagnostics include `MNE220` (missing args), `MNE221` (count mismatch), `MNE222` (kind mismatch), `MNE224` (non-constant value arg), `MNE225` (bound out of range), `MNE227` (expanding/recursive), `MNE228` (empty name), `MNE229` (unsupported kind), and `MNE232` (type param used as bound);
- Five executable backends are audited against the generic specialization corpus; each result retains returned/unsupported/unknown status and unresolved obligations rather than treating matching status as success;
- Stdlib arity-specific families (`contains4`, `sum4`, `min4`, `max4`, `count4`, `contains_byte4`, `count_nonzero4`, `count_true4`, `any_true4`, `all_true4`) delegate to N-parameterized generics (`contains<N>`, `sum<N>`, `min<N>`, `max<N>`, `count<N>`, `contains_byte<N>`, `count_nonzero<N>`, `count_true<N>`, `any_true<N>`, `all_true<N>`);
- Geometry stdlib sequence helpers generalized (`first_point<N>`, `contains_any<N>`, `union<N>`) with `*4` wrappers delegating to generic.
1632: 
**Track 12 — Generic specialization and provenance: IMPLEMENTED**

- Deterministic monomorphization with canonical instantiation identity;
- Specialization provenance tracked from generic definition through HIR/SSA;
- Call-site rewriting preserves specialization provenance in `instantiation`/`specialization` fields;
- Deterministic ordering of specializations via canonical generic identity and declaration-ordered arguments.
1638: 
**Tracks 2–11, 13–15: Future work**

- Constrained polymorphism (`Ordered<T>`, `Eq<T>`, etc.) with evidence passing;
- Coherence/ambiguity resolution;
- Parametricity verifier;
- Effect-row, authority, usage, representation polymorphism;
- Higher-kinded polymorphism;
- Existential/generative abstraction;
- Associated types/constrained type functions;
- Controlled introspection;
- Evidence erasure and dictionary specialization;
- Multi-versioning and partial evaluation;
- Forge-driven realization search.

Implement one constrained interface such as `Ordered<T>`.

Compare:

```text
explicit evidence argument
source-level inferred evidence
specialized evidence
```

All must elaborate to equivalent canonical semantics where applicable.

### Track 3 — Coherence/ambiguity experiment

Create two semantically distinct `Ordered<T>` or `Serialize<T,F>` instances.

Verify that ambiguous source is rejected unless evidence or a coherence domain resolves the selection.

### Track 4 — Parametricity verifier

Build a bounded verifier that checks whether a generic abstraction observes its type parameter only through declared operations.

Fixtures should include:

```text
PASS: identity-like generic
PASS: generic map
FAIL: hidden type reflection
UNKNOWN: unsupported foreign call
```

### Track 5 — Effect-row polymorphism

Implement one generic combinator that preserves an unknown effect row.

Demonstrate a pure instantiation and an effectful instantiation using the same semantic generic definition.

### Track 6 — Usage polymorphism

Implement a small usage-polymorphic abstraction and reject a body that duplicates an input while claiming compatibility with linear usage.

### Track 7 — Representation polymorphism

Represent one logical sequence under at least two physical representations while preserving the same generic algorithm identity.

Verify representation-parametric code behaves equivalently/refines the same semantics.

### Track 8 — Forge representation search

Use a representation-polymorphic kernel with at least two legal representations.

Have Forge compare candidates under explicit objectives while verification checks semantic preservation.

### Track 9 — Existential/generative abstraction

Implement one hidden state type and one fresh nominal domain.

Verify callers cannot inspect hidden representation or mix independently generated domains.

### Track 10 — Specialization refinement

Produce at least two realizations of one generic instantiation:

```text
dictionary-passing baseline
monomorphized/specialized variant
```

Check both against the same RFC 0012 behavior/refinement contract.

### Track 11 — Evidence invalidation

Change one evidence object and demonstrate narrow invalidation of only dependent instantiations/specializations/proofs.

### Track 12 — Controlled associated relation

Prototype one closed terminating associated type relation.

Reject an intentionally nonterminating or ambiguous type-level definition.

## Required fixtures

The RFC should not advance beyond Draft without executable fixtures covering at least:

1. unconstrained identity-like parametric function;
2. constrained generic using explicit evidence;
3. inferred source elaborating to explicit evidence;
4. ambiguous evidence rejection;
5. equality coercion with valid evidence;
6. equality coercion without evidence rejection;
7. generative type mismatch;
8. effect-row propagation;
9. usage-polymorphism duplication rejection;
10. representation-polymorphic execution across two layouts;
11. explicit representation inspection breaking parametricity;
12. specialization preserving generic behavior;
13. specialization widening an effect and being rejected;
14. stale evidence invalidating specialization authority;
15. unresolved realization hole blocking final target emission where no fallback exists; and
16. `UNKNOWN` parametricity/evidence never being treated as `PASS`.

## Formalization direction

The eventual calculus should likely distinguish environments such as:

```text
Gamma    ordinary values/types
Theta    abstraction/type parameters
A        authority parameters
R        resource/usage parameters
E        effect-row parameters
Rep      representation parameters
C        contextual/coeffect parameters
V        evidence bindings
H        realization holes
```

A generic checking judgment might resemble conceptually:

```text
Gamma ; Theta ; A ; R ; E ; Rep ; C ; V ; H
    |- abstraction
    : Interface
    ! Obligations
```

Instantiation may produce:

```text
instantiate(abstraction, substitutions, evidence)
    -> executable semantic artifact
       + obligations
       + unresolved holes
```

The exact notation and calculus remain research work.

The key requirement is that explicit canonical elaboration preserve all semantically relevant dimensions.

## Interaction with earlier RFCs

### RFC 0002 — Contract and Evidence Model

Constraints, parametricity facts, interface laws, equality evidence, and specialization proofs reuse evidence identities, freshness, dependency tracking, and `PASS`/`FAIL`/`UNKNOWN` semantics.

### RFC 0003 — Verified IR

Generic/instantiation/specialization identities must remain traceable through verified IR artifacts.

### RFC 0004 — Recursive Introspection and Refinement

Specializations are candidate transformations subject to bounded refinement/promotion and narrow invalidation.

### RFC 0005 — Source Representations

Source syntax may experiment with implicit/explicit generic notation, but all candidates must elaborate to the same canonical abstraction semantics.

### RFC 0006 — Machine Intent

Realization holes and specialization envelopes consume optimization intent, preferences, hard requirements, portability envelopes, and target constraints.

### RFC 0007 — Proof-Carrying Dependent Core

Kernel-checkable equality/parametricity/law proofs may be represented in the proof core; general evidence resolution and optimization remain outside it.

### RFC 0008 — I/O/Effects/Resources

Effect/capability/resource polymorphism reuses existing rows, authority, and quantitative resource concepts.

### RFC 0009 — Memory/Representation

Representation polymorphism, mobility, layout constraints, provenance, and storage realization search compose directly with memory semantics.

### RFC 0010 — Concurrency

Interfaces may expose linearizability, commutativity, thread-safety, progress, and scheduling-independent properties as evidence-bearing laws.

### RFC 0011 — Failure/Recovery

Failure surfaces and retry/recovery semantics remain visible through abstraction boundaries.

### RFC 0012 — Executable Semantic Core

Generic definitions and instantiations ultimately elaborate into RFC 0012 executable semantics. Specializations must simulate/refine those semantics.

## Security implications

RFC 0013 should improve security by making abstraction boundaries authority-aware.

Potential benefits include:

- hidden representation cannot be inspected without explicit authority;
- generative identities can isolate security domains;
- capability requirements remain visible through interfaces;
- implicit evidence selection is auditable;
- specialization cannot silently widen authority;
- representation changes remain bounded by explicit contracts; and
- generic code proven parametric has a smaller observable attack surface.

However, incorrect parametricity or coherence evidence could authorize unsafe transformations. Evidence identities and invalidation therefore remain security-relevant.

## Optimization implications

RFC 0013 should expose large machine-usable optimization spaces.

Forge may use:

- parametricity to prove non-observation;
- representation polymorphism to search layouts;
- effect polymorphism to specialize pure/effectful paths;
- authority polymorphism to minimize capability surfaces;
- evidence specialization to inline operations;
- usage polymorphism to choose move/copy/borrow strategies;
- existential abstraction to replace implementations behind stable contracts; and
- realization holes to delay decisions until target/workload evidence exists.

Optimization freedom must always be bounded by the abstraction contract and refinement evidence.

## Portability implications

A generic semantic abstraction can admit different realizations across targets while retaining one logical identity.

Representation, dispatch, allocator, and effect-handler strategies may vary.

Target-specific facts must not leak into a parametric dimension unless explicitly exposed.

Portable fallback remains required where declared by the abstraction's portability envelope.

## Open questions

1. What is the minimal explicit polymorphic core compatible with RFC 0007's dependent type theory?
2. Which abstraction dimensions belong in the proof core versus executable elaboration metadata?
3. Should capability/effect/context rows share one generalized row calculus or remain separate typed row kinds?
4. What quantitative algebra is sufficient for early usage polymorphism?
5. Which parametricity claims can be established syntactically versus requiring theorem proving?
6. How should higher-order effects interact with parametricity?
7. How should existential packages interact with resource ownership and deallocation authority?
8. What coherence domains are necessary for practical machine-generated code?
9. Should evidence selection always be globally canonical within a build, or can local explicit evidence safely override defaults?
10. Which associated-type/type-function features are worth their proof/inference cost?
11. How should variance/subtyping interact with effects, authority, and mutable representations?
12. How should representation polymorphism interact with FFI/ABI boundaries where layout becomes observable?
13. How should dynamic/JIT specialization be represented when target observations occur at runtime?
14. Which realization holes may survive into deployable artifacts?
15. Can Forge automatically synthesize representation-independent adapters when changing hidden existential implementations?
16. What evidence is sufficient to treat two instance dictionaries as semantically coherent/equivalent?
17. How should abstraction-level laws be versioned when evidence methods improve?
18. Should proof erasure and evidence erasure be represented by one generalized relevance system?
19. How should dependent indices that are runtime-relevant participate in specialization identity?
20. How much explicitness produces the best agent generation/repair performance in the syntax tournament?

## Acceptance criteria

RFC 0013 should not advance beyond Draft solely because the type system can parse generic syntax.

A Proposed transition should require at minimum:

- one explicit universal abstraction/instantiation model;
- deterministic canonical identities for generic definitions and instantiations;
- one evidence-passing constrained abstraction;
- explicit ambiguity/coherence behavior;
- at least one machine-checkable parametricity fact;
- one effect-polymorphic combinator;
- one quantitative/usage-polymorphism experiment;
- one representation-polymorphic abstraction across two realizations;
- one existential/generative abstraction experiment;
- one specialization with refinement evidence against the generic semantics;
- narrow invalidation after evidence changes;
- negative fixtures demonstrating that `UNKNOWN` or structural shape alone cannot satisfy stronger semantic claims; and
- independent documentation of which parts are kernel-checked, externally verified, assumed, or experimental.

An Accepted transition should additionally require evidence that the abstraction system improves machine generation, verification, specialization, or optimization without creating unacceptable proof, inference, coherence, or implementation complexity.

## Initial implementation recommendation

The first executable subset should remain deliberately narrow.

Recommended order:

1. explicit `forall T` semantic abstraction;
2. explicit instantiation;
3. one `Ordered<T>`-style evidence constraint;
4. bidirectional elaboration for that subset;
5. canonical evidence identity/coherence diagnostics;
6. one effect-row parameter;
7. one usage parameter;
8. one representation parameter with two concrete representations;
9. one existential package;
10. one verified specialization path.

Do not begin with unrestricted associated types, overlapping implicit instances, higher-order type functions, or broad type-level metaprogramming.

The system should first prove that explicit abstraction semantics, evidence identity, parametricity, and specialization/refinement compose cleanly with RFC 0012.

## Conclusion

MNCS should not treat generics primarily as a source-code reuse feature.

A machine-native abstraction system should preserve the exact dimensions that remain unknown or hidden, record the evidence that makes constrained operations legal, expose the limits of observation through parametricity, and retain implementation choices as bounded realization space until selection is justified.

The foundational rule remains:

> **Abstraction hides implementation, not semantic obligations. Polymorphism describes dimensions that remain unconstrained; specialization is a verified realization of those dimensions.**

The machine-native extension is equally important:

> **Genericity should preserve search space rather than prematurely choose implementation.**

If this direction succeeds, the same logical algorithm can remain stable while MNCS changes representation, evidence realization, target strategy, allocator, dispatch, or other implementation dimensions under explicit contracts and refinement evidence. Abstraction then becomes not only a mechanism for reusable code, but a formal boundary within which Forge and other machines can safely search for better realizations.
