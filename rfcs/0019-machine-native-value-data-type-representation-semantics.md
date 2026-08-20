# RFC 0019: Machine-Native Value, Data, Type, and Representation Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.2–1.0
- **Depends on:** RFC 0001, RFC 0002, RFC 0003, RFC 0004, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0013, RFC 0014, RFC 0015, RFC 0016, RFC 0017, RFC 0018

## Summary

MNCS Language should distinguish **logical values, logical types, observations, representations, storage, and serialized encodings** instead of treating them as one conventional language-level notion of a value with a fixed machine layout.

The central rule is:

> **A value is semantic information. A type constrains and organizes the set and structure of legal values. A representation is a realizable physical encoding of those values. Type identity, value identity, representation identity, storage identity, address identity, and serialized identity are distinct unless an explicit semantic contract relates them.**

A second rule is equally important:

> **Physical representation is normally a realization choice, not intrinsic program meaning. A representation may change without changing a logical value when every observation and protected property required by the program remains satisfied and the transformation carries sufficient evidence.**

RFC 0019 establishes the machine-native data universe needed by the rest of the language. It defines a constitutional direction for:

- unit and empty types;
- product, record, tuple, and structural aggregate types;
- sum, variant, option, and result-like types;
- inductive recursive data;
- coinductive and observation-oriented data;
- explicit recursive-type boundaries;
- indexed and refinement-carrying executable data;
- selective union, intersection, and exclusion descriptions;
- nominal, generative, opaque, and abstract identities;
- function values independently of closure representation;
- existential and dynamic packages with explicit type witnesses;
- semantic views and pattern observations independently of physical layout;
- representation requirements and candidate representation realizations;
- representation validity, injectivity, distinguishability, encode/decode, and observation-preservation obligations;
- representation-polymorphic operations;
- explicit representation observation when layout, size, alignment, addressability, or encoding is semantically required;
- search over alternative representations without changing logical program meaning;
- evidence-bearing representation selection and invalidation; and
- bounded interaction with memory, FFI, serialization, proof, staging, optimization, target, assurance, and recursive refinement semantics.

The intended result is not a final type theory, a final memory model, a final ABI, or a final serialization format. It is a stable semantic separation that allows those later choices to evolve without retroactively redefining what ordinary MNCS values mean.

---

## Motivation

The existing RFC set already uses terms such as:

- typed value;
- value identity;
- function value;
- runtime value;
- state value;
- type;
- representation;
- layout;
- canonical representation;
- memory object;
- serialization;
- realization; and
- target representation.

But the language does not yet have one foundational RFC explaining how those concepts relate.

Without such a foundation, implementation choices can silently become semantic commitments.

For example, a conventional compiler may lower:

```text
Option<Pointer<T>>
```

to a nullable pointer.

That can be a good representation.

It should not imply that the logical meaning of `Option<Pointer<T>>` is inherently "one machine pointer where zero means None".

Likewise:

```text
Tree<T>
```

should not mean:

```text
struct Node {
    tag;
    T value;
    Node* left;
    Node* right;
}
```

unless the program explicitly requires that representation.

The logical tree might instead be realized as:

- linked nodes;
- an arena plus indices;
- a packed array;
- a B-tree;
- a compressed trie;
- a structure-of-arrays layout;
- a persistent path-copying structure;
- a target-specific compact encoding; or
- another representation discovered by Forge.

If the program only observes the logical tree interface, those realizations should remain available search candidates.

This distinction matters especially for MNCS because the architecture is intended to support:

- machine-directed realization search;
- evidence-bearing optimization;
- target specialization;
- recursive refinement;
- representation experimentation;
- proof-preserving transformations;
- heterogeneous execution; and
- explicit backend promises.

A language that bakes physical layout into ordinary type meaning too early destroys much of that search space.

Conversely, a language that treats representations as arbitrary compiler secrets cannot support exact machine-intent requirements such as:

- exact layout;
- fixed alignment;
- no boxing;
- stable ABI;
- DMA-safe storage;
- constant-time representation;
- packed wire structure;
- vector width;
- device visibility;
- page locality; or
- representation-sensitive foreign interfaces.

MNCS therefore needs both:

1. strong logical representation independence by default; and
2. explicit representation commitments when programs genuinely require them.

---

## Constitutional principles

### 1. Value is not storage

> **A logical value does not imply an address, allocation, memory object, lifetime, ownership regime, or storage class.**

Conceptually:

```text
Value<T> != MemoryObject<T>
```

RFC 0009 owns allocation, storage objects, references, provenance, lifetime, validity, and memory realization.

RFC 0019 owns the semantic value that storage may encode.

A value may:

- exist only transiently during evaluation;
- be represented in registers;
- be recomputed instead of stored;
- be split across several storage objects;
- be packed with other values;
- be materialized only at a boundary;
- have no stable address; or
- be eliminated entirely by specialization.

None of those transformations should change logical meaning by themselves.

### 2. Type is not representation

> **A logical type does not normally prescribe a machine layout.**

Conceptually:

```text
LogicalType<T> != Layout<T>
```

A type describes semantic membership, structure, available observations, and relevant invariants.

A representation describes one physical way to encode values satisfying that type.

### 3. Representation is a realization

> **A physical representation is selected under requirements, target facts, observation constraints, and evidence.**

A candidate representation is not valid merely because a compiler can generate code for it.

It must satisfy the obligations associated with the logical value and all representation-sensitive requirements.

### 4. Logical identity survives legal representation change

Unboxing, boxing, packing, tag compression, scalar replacement, fusion, closure conversion, defunctionalization, data-layout transformation, SIMD restructuring, device transfer, and other legal representation changes do not automatically create new logical values.

A transformation changes logical identity only when the semantic relation says it does.

### 5. Representation observation is explicit

> **Observing layout is semantically different from observing logical structure.**

Operations such as:

```text
size_of<T>
align_of<T>
layout_of<T>
addressability_of<T>
representation_id<T>
tag_encoding_of<T>
```

cross into representation-sensitive semantics.

Once such an observation matters, the program's representation freedom narrows accordingly.

### 6. Constructors define logical structure, not mandatory layout

A constructor such as:

```text
Node(value, left, right)
```

defines a logical constructor relationship.

It does not necessarily define a struct field order, tag width, pointer graph, or allocation pattern.

### 7. Pattern matching observes semantic views

> **Ordinary pattern matching should decompose logical structure or an explicit semantic view, not accidentally expose physical representation.**

Layout inspection requires a representation-level operation.

### 8. Recursive structure is explicit in canonical semantics

Human syntax may hide recursive fold/unfold operations.

The canonical semantic model should retain enough explicit recursion structure to support:

- reasoning;
- invalidation;
- termination/productivity analysis;
- representation transformation; and
- proof obligation generation.

### 9. Dynamic means existentially unknown, not untyped

A runtime-dynamic value should remain conceptually typed:

```text
exists T. Value<T> × TypeWitness<T>
```

It is not permission to treat arbitrary bytes as arbitrary types.

### 10. Casts require justification

> **A physical bit pattern resembling the representation of `T` does not make it a logical value of `T`.**

Checked conversion, representation reinterpretation, FFI import, deserialization, and unsafe cast operations are distinct and must carry their respective validity conditions.

### 11. Invalid representation is not a value

Allocated bytes may exist without constituting a valid logical value.

This follows RFC 0009's distinction between storage and semantic value.

A representation must satisfy the validity predicate of its represented type before it may be treated as a value of that type.

### 12. Partial computation is not hidden in every value type

> **Divergence, failure, unresolved computation, and nontermination belong to computation semantics unless a datatype explicitly models partial information.**

MNCS should not implicitly add a universal bottom inhabitant to every executable type merely because the implementation can fail to produce a value.

### 13. Representation freedom is bounded by observations

Representation independence is not absolute.

If a program observes:

- exact byte layout;
- address stability;
- object identity;
- pointer equality;
- field offset;
- tag representation;
- timing behavior tied to layout; or
- foreign ABI layout,

then those observations become requirements on legal realizations.

### 14. Representation-sensitive requirements are explicit contracts

A program may intentionally require:

```text
alignment >= 64
contiguous
unboxed
stable_address
abi("C")
fixed_size
constant_time_decode
device_visible
```

Such requirements narrow the realization envelope rather than becoming implicit folklore.

### 15. Representation evidence is scoped and fresh

Representation evidence is subject to RFC 0018.

Evidence that a realization is valid may depend on:

- type definition;
- target;
- compiler;
- backend;
- pass pipeline;
- ABI;
- alignment model;
- storage model;
- runtime;
- unsafe assumptions; or
- proof/checker versions.

Changing a dependency invalidates only the dependent representation authority.

### 16. Logical and representational equality are distinct

RFC 0019 establishes the distinction but deliberately leaves the complete equality taxonomy to RFC 0020.

At minimum:

```text
same logical value
same logical type
same representation
same bits
same storage object
same address
```

must not be treated as interchangeable statements.

### 17. Representation choice may remain underdetermined

A program should be able to state semantic requirements without selecting a representation.

This is a first-class machine-native feature, not an incomplete program.

### 18. The compiler must not invent semantic observability

A backend may introduce tags, boxes, padding, headers, forwarding pointers, descriptor words, or hidden metadata.

Those implementation artifacts do not become legal program observations unless explicitly exposed through a representation contract.

---

## Scope

RFC 0019 defines the semantic relationship among:

```text
logical value
logical type
logical constructor/destructor structure
semantic observation
semantic view
recursive data structure
indexed data
abstract/nominal identity
dynamic package
function value
representation requirement
representation realization
representation witness/evidence
representation observation
```

It does not define the complete semantics of:

```text
allocation
reference provenance
lifetime
pointer validity
foreign ABI
wire serialization
numeric arithmetic
definitional equality
proof universes
subtyping algorithm
full elaboration/type inference
compiler optimization
runtime object management
```

Those belong to existing or later RFCs.

---

## Theoretical synthesis

RFC 0019 intentionally combines several traditions rather than adopting one type-system family wholesale.

### Algebraic data types

Finite structural data should be expressible through a small algebraic basis centered on:

```text
Zero
Unit
Product<A, B>
Sum<A, B>
```

Labeled records and variants can be treated as structured/labeled forms of products and sums.

This gives MNCS a compact, inspectable, machine-friendly logical vocabulary.

It also enables derived structures such as:

```text
Bool
Option<T>
Result<T, E>
tuples
records
enums
variants
```

without committing to machine layout.

### Initial algebras for inductive data

Finite recursive datatypes should draw from initial-algebra semantics.

Conceptually:

```text
List<T> = μX. Unit + (T × X)
```

This provides:

- constructors;
- structural recursion;
- induction principles;
- canonical recursive shape; and
- representation-independent logical structure.

The RFC does not require every implementation to expose category-theoretic notation.

The theory supplies the semantic model.

### Coalgebras for observation-oriented and coinductive data

Potentially infinite, ongoing, or observation-defined structures should draw from coalgebraic/coinductive semantics.

Conceptually:

```text
Stream<T> = νX. T × X
```

The important distinction is that a coinductive semantic object need not imply an infinitely materialized memory value.

It may instead be characterized by observations that remain productively available over time.

This direction is useful for:

- streams;
- generators;
- reactive state;
- event sequences;
- protocol-like observations; and
- persistent observation interfaces.

Detailed temporal and liveness semantics remain outside RFC 0019.

### Iso-recursive core semantics

MNCS should prefer an explicitly representable recursion boundary in canonical semantics.

Conceptually:

```text
fold : F(μX.F(X)) -> μX.F(X)
unfold : μX.F(X) -> F(μX.F(X))
```

Human syntax and elaboration may provide equi-recursive convenience where appropriate.

But canonical semantic artifacts should not silently erase the recursive boundary if doing so would obscure:

- proof obligations;
- normalization;
- termination reasoning;
- representation transformations; or
- identity.

### Selective set-theoretic and semantic typing

MNCS should investigate semantic/set-theoretic descriptions for machine-useful type operations such as:

```text
Union<A, B>
Intersection<A, B>
Difference<A, B>
Negation<A>
```

where the value universe and decision procedures are well-defined.

This supports flow-sensitive narrowing and machine reasoning such as:

```text
x : A | B
branch establishes x notin A
therefore x : B
```

However, type identity is not reduced universally to set equality.

MNCS must also preserve nominal, generative, opaque, capability, resource, and abstraction identities.

### Indexed and GADT-like data

Executable data should be able to carry indices whose constructors establish semantic relationships.

Example:

```text
Vector<T, N>

Empty : Vector<T, 0>
Push  : T × Vector<T, N> -> Vector<T, N + 1>
```

RFC 0019 owns the executable datatype structure and the fact that constructors can refine indices.

RFC 0007 owns the proof theory used to establish, manipulate, and check propositions about those indices.

### Logical relations and representation independence

Representation independence is a central theoretical pillar.

Two realizations may represent the same abstract/logical value space if permitted observations cannot distinguish them except through explicitly representation-sensitive operations.

MNCS should eventually support relations conceptually similar to:

```text
Represents<R, T>
ObservationEquivalent<R1, R2, O>
```

where `O` identifies the observation boundary being preserved.

This allows representations to vary while protecting semantic behavior.

### Representation polymorphism

Runtime representation sometimes matters to generated code.

MNCS should therefore represent physical representation requirements explicitly rather than pretending they do not exist.

However, representation metadata belongs to a realization layer.

Conceptually:

```text
LogicalType T
RepresentationRequirement<T> Q
RepresentationRealization<T> R
Evidence(R satisfies Q)
```

Operations may quantify over representation classes when they genuinely work independently of exact layout.

### Representation injectivity and validity

Representation transformations may remove explicit structure only if required distinctions remain recoverable.

A realization may need obligations such as:

```text
valid
complete
injective_on_observable_distinctions
decodable
constructor_distinguishable
stable_under_required_operations
```

For example, a nullable-pointer representation of `Option<Pointer<T>>` is valid only if the chosen pointer model guarantees that the reserved null representation cannot be confused with a valid `Some` payload.

### Semantic views

A type may expose one or more logical decomposition views independent of storage layout.

Conceptually:

```text
View<Tree<T>, TreeShape<T>>
```

A view can support pattern matching and structural observation without requiring the underlying realization to use the same physical structure.

This is important for data abstraction and representation search.

### Existential and evidence-bearing dynamic values

Runtime-dynamic values should be modeled as typed existential packages rather than as an untyped escape hatch.

Conceptually:

```text
DynamicValue = exists T.
    Value<T>
    × TypeWitness<T>
    × RepresentationWitness<T>
```

The exact witness structure remains open.

A projection into a requested type is a checked operation whose result may be:

```text
MATCH(value)
MISMATCH
UNRESOLVED
```

depending on the available type and representation evidence.

### Gradual information only at explicit boundaries

MNCS should not become a universally gradual type system by default.

However, gradual-typing theory is useful when type information is genuinely incomplete at:

- foreign boundaries;
- dynamic plugin boundaries;
- partially known schemas;
- runtime-generated artifacts;
- migration boundaries; or
- incomplete experimental candidates.

The governing rule is:

> **Unknown type information removes authority; it does not silently convert a program into `Any`.**

### Domain theory used for recursion and approximation, not universal hidden bottom

Domain-theoretic ideas remain useful for:

- recursive semantic equations;
- approximation;
- partial information;
- infinite behavior; and
- denotational models of computation.

But ordinary executable value types should not gain an implicit bottom inhabitant simply because computations can diverge.

RFC 0012's value/computation separation remains authoritative.

### Closure conversion and defunctionalization as realization theories

A function is a semantic value independently of how the runtime realizes it.

A function value may become:

- direct code;
- code plus environment;
- a specialized direct call;
- a defunctionalized constructor;
- a dispatch-table entry;
- a target-specific callable object; or
- no runtime object at all after inlining/specialization.

Function meaning therefore must not be equated with closure layout.

---

## Semantic strata

RFC 0019 proposes six distinct semantic strata.

### Stratum 1: logical value

A `LogicalValue<T>` is semantic information inhabiting logical type `T`.

Examples:

```text
Unit
True
42
Some(Widget)
Node(value, left, right)
```

A logical value does not inherently expose:

- address;
- bit width;
- endianness;
- padding;
- tag encoding;
- allocation;
- storage class;
- object header;
- closure layout; or
- serialization.

### Stratum 2: logical type

A `LogicalType` describes legal values, structure, observations, and invariants.

Initial categories include:

```text
ZeroType
UnitType
ProductType
RecordType
SumType
VariantType
FunctionType
InductiveType
CoinductiveType
IndexedType
NominalType
OpaqueType
ExistentialType
DynamicPackageType
```

Selective union/intersection/exclusion descriptions may be admitted as research extensions.

### Stratum 3: observation interface

An `Observation` describes information that may legally be extracted from a logical value.

Examples:

```text
constructor_of
field
view
index
call
logical_type_identity
```

Representation-sensitive observations are separate:

```text
layout
size
alignment
addressability
physical_tag
physical_encoding
```

This boundary is essential to representation independence.

### Stratum 4: representation realization

A `RepresentationRealization<T>` describes one physical representation strategy for logical type `T`.

It may specify facts such as:

```text
layout
boxing
tagging
packing
alignment
addressability
decomposition
runtime representation class
storage requirements
target requirements
```

This is a candidate realization, not automatically a valid one.

### Stratum 5: storage realization

Storage belongs primarily to RFC 0009.

A representation realization may require storage properties, but the storage object itself remains distinct.

Conceptually:

```text
LogicalValue<T>
    represented_by -> RepresentationRealization<T>
    materialized_in -> StorageObject
```

These edges are not identities.

### Stratum 6: serialized encoding

Serialized/wire encoding is distinct again.

A value may have many serialized encodings that are neither its runtime representation nor its logical identity.

Detailed serialization semantics are reserved for RFC 0027.

---

## Core logical data vocabulary

### Zero

`Zero` has no values.

It is useful for:

- impossible branches;
- uninhabited variants;
- exhaustive reasoning; and
- representing impossible states in structural type equations.

`Zero` must not be confused with nontermination or runtime failure.

### Unit

`Unit` has one logical value.

It represents presence of a constructor or event where no additional payload information is required.

### Product

A product combines values that exist together:

```text
Product<A, B>
```

Labeled products produce record-like structure.

Product semantics do not imply field order or adjacency in memory unless representation constraints say so.

### Sum

A sum represents one of several alternatives:

```text
Sum<A, B>
```

Labeled sums produce variant-like structure.

Sum semantics require logical constructor distinction.

They do not imply a particular tag encoding.

### Records

Records are labeled products with stable semantic field identities.

A record's logical field identity should not depend on physical field offset.

A representation may reorder, split, fuse, compress, or eliminate fields when doing so preserves required observations.

### Variants

Variants are labeled sums with stable semantic constructor identities.

Physical tag numbers are not constructor identities unless explicitly frozen by a representation/ABI contract.

### Tuples

Tuples are ordered structural products.

Their logical order may matter even when physical placement changes.

### Optional and result-like forms

`Option<T>` and result-like types should be derivable from sum semantics rather than privileged by layout.

Implementations remain free to use niche optimization or other compact encodings when representation validity is established.

---

## Inductive data semantics

### Logical definition

An inductive datatype is defined by a finite set of constructors whose recursive occurrences are well-founded under the selected inductive semantics.

Conceptually:

```text
Inductive List<T> where
    Nil
    Cons(head: T, tail: List<T>)
```

The canonical representation should retain:

- datatype identity;
- constructor identity;
- constructor fields;
- recursive positions;
- index relationships where present; and
- generated induction/recursion obligations where appropriate.

### Representation freedom

The logical recursive equation does not force pointer-recursive storage.

Candidate realizations may include:

```text
linked
flat
chunked
rope
arena-indexed
packed
compressed
persistent
```

provided the realization preserves the required logical observations.

### Recursive identity

Recursive self-reference should bind to semantic datatype identity, not textual spelling or machine address.

### Fold/unfold

Canonical semantics should preserve an explicit fold/unfold boundary or an equivalent structure that supports localized reasoning.

Surface syntax may omit it.

---

## Coinductive and observation-defined data

### Purpose

Coinductive structures model entities characterized by continued observation rather than finite construction.

Examples include:

```text
Stream<T>
Reactive<T>
ObservationProcess<S>
```

### No infinite materialization assumption

A coinductive value does not imply infinite resident storage.

The implementation may realize the object as:

- state plus transition;
- generator closure;
- coroutine;
- event source;
- network-backed observation;
- memoized structure; or
- another productive realization.

### Separation from effects and events

RFC 0008 remains authoritative for I/O, effects, resources, and events.

RFC 0019 only establishes that logical data can be observation-defined or coinductive.

### Productivity

Detailed productivity, liveness, and bounded-computation rules are reserved for RFC 0022.

---

## Indexed and refinement-carrying data

### Indexed constructors

Constructors may refine type indices.

Example:

```text
Vector<T, N>

Empty : Vector<T, 0>
Push  : T × Vector<T, N> -> Vector<T, N + 1>
```

### Runtime versus erased indices

An index may be:

- runtime-observable;
- compile-time-only;
- proof-only;
- representation-derived;
- externally witnessed; or
- erased after obligations are discharged.

Those categories must not be conflated.

### Index evidence

If an operation relies on an index fact after erasure, the corresponding evidence relationship must survive sufficiently to justify the operation or backend promise.

### Proof boundary

RFC 0007 defines proof terms, universes, dependent functions, equality proofs, and kernel checking.

RFC 0019 does not redefine them.

---

## Nominal, generative, opaque, and abstract types

### Structural sameness is not always type identity

Two types may expose identical structural value sets while remaining intentionally distinct.

Example:

```text
Meters
Seconds
```

might both be represented by the same numeric carrier but have different semantic identities.

### Generativity

RFC 0013's generative abstraction may produce fresh type identities.

Those identities remain distinct even when realizations are structurally identical.

### Opaque types

An opaque type may hide its logical constructors, its representation, or both from a client.

The exposed interface determines permitted observations.

### Representation independence

Opaque abstraction is one of the strongest use cases for representation independence.

A provider may change representation without invalidating clients whose allowed observations remain preserved.

---

## Set-theoretic descriptions

### Union

`Union<A, B>` denotes values admitted by either description where the involved type identities permit such semantic interpretation.

### Intersection

`Intersection<A, B>` denotes values satisfying both descriptions.

### Exclusion or negation

Difference/negation forms may describe values excluded from another domain when the universe is explicit and the decision procedure is sound for the admitted fragment.

### Flow narrowing

These forms can support machine-generated refinement facts.

Example:

```text
x : A | B
observe not A(x)
=> x : B
```

### Conservative scope

MNCS should not promise arbitrary decidable set-theoretic type equivalence.

Unsupported relations remain explicit obligations or `UNKNOWN` rather than being guessed.

### Nominal preservation

Set-theoretic extensional equivalence does not automatically erase nominal or generative identity.

---

## Function values

### Logical function identity

A function value is characterized by its semantic callable behavior, contract, effect/capability relationships, type, and captured logical dependencies.

It is not defined by a closure-memory layout.

### Candidate realizations

A function may be realized as:

```text
direct code
code + environment
specialized direct code
defunctionalized variant
jump-table entry
vtable-like callable
interpreter object
target-native callable
inlined/no runtime value
```

### Captured authority

If a function captures capabilities or resources, that captured authority remains semantically relevant even if the closure representation disappears.

RFC 0008 and RFC 0013 govern the capability and abstraction implications.

### Function equality

RFC 0019 does not define general function equality.

RFC 0020 will address equality and observational equivalence.

---

## Dynamic and existential packages

### Dynamic package model

A dynamic package should preserve hidden type identity.

Conceptually:

```text
DynamicPackage {
    hidden_type: TypeIdentity
    value: exists T. LogicalValue<T>
    type_witness: TypeWitness<T>
    representation_witness: optional RepresentationWitness<T>
}
```

### No universal unchecked cast

A dynamic package is not an `Any` whose payload may be reinterpreted arbitrarily.

Projection requires a checked relationship between the hidden type and requested type.

### Projection outcomes

A projection may produce:

```text
MATCH(value)
MISMATCH
UNRESOLVED(reason)
```

`UNRESOLVED` is important when the system lacks sufficient witness information to decide the relationship safely.

### Dynamic representation

A dynamic value may use:

- tagged boxes;
- descriptor pointers;
- compact type IDs;
- existential closure representations;
- specialized variants; or
- other runtime encodings.

Those are realizations, not the semantic definition of dynamic typing.

---

## Semantic views

### Purpose

Views permit logical decomposition without exposing physical layout.

Example:

```text
view Tree<T> as
    Empty
    | Node(value: T, left: Tree<T>, right: Tree<T>)
```

The actual representation may be a B-tree or compressed trie.

### View identity

A view has its own semantic identity and version where necessary.

### Multiple views

A type may expose multiple views to different clients or algorithms.

For example, a matrix may expose:

- element view;
- row view;
- block view; and
- sparse-entry view.

### View obligations

A representation claiming to implement a view must establish that each permitted view observation is sound.

### Pattern matching

Ordinary pattern matching should lower through an applicable semantic view or constructor structure.

It should not imply direct field-offset access unless the representation contract permits it.

---

## Observation semantics

### Observation classes

RFC 0019 distinguishes at least:

```text
logical observation
view observation
type-identity observation
representation observation
storage observation
serialization observation
```

### Logical observation

Examples:

```text
constructor identity
field value
index value
function application
view decomposition
```

### Representation observation

Examples:

```text
byte size
alignment
field offset
physical tag
boxing state
runtime representation class
```

### Storage observation

Examples:

```text
address
storage identity
region
lifetime state
provenance
```

These belong primarily to RFC 0009.

### Explicit capability or effect requirements

Some representation or storage observations may require capabilities because they expose information that can affect safety, abstraction, or side channels.

The exact capability model remains coordinated with RFC 0008, RFC 0009, and RFC 0015.

### Observation closure

A representation-preservation claim must name the observation set or interface under which equivalence is claimed.

There is no universal promise that two representations are indistinguishable under every possible low-level observation.

---

## Representation requirements

A representation requirement is a semantic constraint on eligible realizations.

Example categories include:

```text
size bound
alignment bound
contiguity
stable addressability
fixed field order
fixed tag encoding
boxing prohibition
packing requirement
vector width
device visibility
host visibility
DMA eligibility
ABI compatibility
constant-time operation requirement
cache-line isolation
page locality
endianness requirement
atomic update compatibility
```

### Hard requirement versus preference

Consistent with RFC 0006, requirements and preferences remain distinct.

Example:

```text
require alignment >= 64
prefer contiguous
prefer unboxed
```

A candidate that violates a requirement is invalid.

A candidate that violates a preference may remain valid but rank lower.

### Requirement source

Requirements may arise from:

- source semantics;
- machine intent;
- FFI contracts;
- target constraints;
- resource/effect contracts;
- proof obligations;
- deployment environment;
- optimization-region policy; or
- assurance policy.

---

## Representation realization model

A candidate representation realization should eventually carry a versioned object conceptually similar to:

```text
RepresentationRealization {
    realization_id
    logical_type_id
    representation_family
    layout_descriptor?
    tag_strategy?
    boxing_strategy?
    packing_strategy?
    decomposition?
    runtime_representation_class?
    storage_requirements[]
    target_requirements[]
    abi_requirements[]
    supported_observations[]
    representation_observations[]
    obligations[]
    evidence_refs[]
    assumptions[]
}
```

This is illustrative, not a frozen schema.

### Representation family

A family may identify broad realization strategies such as:

```text
boxed
unboxed
packed
tagged
niche_encoded
array_of_structures
structure_of_arrays
chunked
sparse
dense
closure
specialized
```

Family identity must not substitute for actual validity evidence.

### Layout descriptor

A layout descriptor may eventually include:

```text
size
alignment
field placements
tag representation
padding
endianness
bit ranges
addressability
```

when these facts are relevant.

The RFC does not require all representations to expose a fully fixed static byte layout.

### Partial and symbolic layouts

Some realizations may keep layout facts symbolic or constrained until target selection.

Example:

```text
size = pointer_width * 2
alignment >= target.pointer_alignment
```

This should be legal where downstream phases can preserve the unresolved requirements.

---

## Representation validity

A representation must satisfy a validity relationship before it may stand for logical values.

Potential obligations include:

### Coverage

Every logical value required by the represented domain has a legal representation or a declared bounded subset relationship.

### Distinguishability

Logically distinguishable values remain distinguishable under all required observations.

### Decodability

Where decoding is part of the representation contract, a valid physical representation maps back to the intended logical value.

### Valid-state characterization

The set of valid physical states is explicit enough to reject or quarantine invalid states.

### Constructor distinction

Variant constructors remain logically distinguishable even when their physical tags are compressed or implicit.

### Field preservation

Every required logical field observation returns the corresponding logical value.

### Index preservation

Representation-dependent indices remain consistent with logical indices where such a relationship is claimed.

### Operation preservation

Representation-specific operations used as realizations of logical operations preserve their contracts.

### No accidental inhabitants

A physical state not admitted by the logical type must not silently become an ordinary valid value.

### No collapsed inhabitants

Distinct logical values must not collapse to one physical state when a required observation distinguishes them.

---

## Encode/decode relations

For many representations it is useful to reason conceptually about:

```text
encode_R : T -> RepState<R>
decode_R : RepState<R> -> Result<T, InvalidRepresentation>
```

These need not exist as runtime functions.

They may be logical relations or proof specifications.

Possible obligations include:

```text
decode_R(encode_R(v)) = v
```

and, where canonical physical states are required:

```text
encode_R(decode_R(r)) = r
```

for valid `r`.

Not every realization requires a canonical reverse encoding.

The required strength should be explicit.

---

## Niche and compressed representations

### Niche encoding

A representation may reuse otherwise-invalid carrier states to encode logical variants.

Example:

```text
Option<NonNullPointer<T>>
```

may use a null carrier state for `None` if the pointer representation contract guarantees null is not a valid `Some` payload.

### Evidence obligation

The optimization requires evidence for the niche property.

A target or ABI change that invalidates that property must invalidate the realization.

### No folklore

MNCS should not rely on undocumented "this target always does that" assumptions.

The niche must be represented as a fact, requirement, or assumption with provenance.

---

## Boxing and unboxing

### Boxing is not semantic identity

A logical value may move between boxed and unboxed realizations without changing meaning when required observations remain preserved.

### Unboxing obligations

Unboxing may require evidence concerning:

```text
size
alignment
lifetime
addressability
constructor distinction
recursive size
calling convention
GC/runtime interaction
```

### Address-sensitive values

If code observes stable address identity, unboxing may be illegal or require a different semantic relationship.

### Recursive values

Recursive datatypes may require indirection in some realizations but not necessarily at the logical level.

---

## Layout transformations

### Structure of arrays and array of structures

MNCS should permit transformations such as:

```text
Array<Record{x:A, y:B}>
```

into a realization resembling:

```text
Record{x:Array<A>, y:Array<B>}
```

when the observation interface permits it and operations are correspondingly transformed.

### Field reordering

Logical field identity is not physical order.

Field reordering is legal unless:

- representation observation;
- ABI;
- serialization;
- address calculation;
- atomicity;
- side-channel; or
- another explicit requirement

constrains the order.

### Field elimination

A field whose logical value is derivable may be omitted physically if all observations can reconstruct the required value and required complexity/resource constraints remain satisfied.

### Fusion and splitting

A logical value may be spread across multiple machine objects or multiple logical values may share one packed representation when storage and aliasing semantics permit it.

---

## Representation-polymorphic operations

Some operations may work for every realization satisfying a representation class or observation interface.

Conceptually:

```text
forall R.
    Represents<R, T>
    & Supports<R, ObservationSet>
    => Operation<T, R>
```

The exact type syntax is intentionally deferred.

### No hidden representation assumptions

A representation-polymorphic operation must not silently assume:

- pointer width;
- boxing;
- tag position;
- field order;
- contiguous storage; or
- addressability

unless those requirements are declared.

### Specialization

RFC 0016 may specialize a representation-polymorphic operation after realization selection while retaining provenance back to the more general semantic operation.

---

## Representation independence

### Observation-relative relation

Representation independence should be stated relative to permitted observations.

Conceptually:

```text
EquivalentUnder<R1, R2, O>
```

means realizations `R1` and `R2` cannot be distinguished through observation interface `O` except where the semantic relation explicitly allows differences.

### Abstract types

Clients of an abstract type should normally reason through its exported observation interface rather than its representation.

### Provider evolution

A provider may change representation without breaking clients when:

- the logical type contract remains compatible;
- exported observations remain preserved;
- performance/resource promises remain satisfied where contractual;
- FFI/ABI commitments remain satisfied where applicable; and
- assurance policy authorizes the change.

### Optimization consequence

Representation independence creates a safe optimization/search space.

Forge may search inside that space without rewriting the program's declared logical meaning.

---

## Identity taxonomy

RFC 0019 requires at least conceptual separation among:

```text
LogicalTypeIdentity
LogicalValueIdentity
ConstructorIdentity
FieldIdentity
ViewIdentity
RepresentationIdentity
RepresentationFamilyIdentity
LayoutIdentity
StorageObjectIdentity
ReferenceIdentity
AddressIdentity
SerializedEncodingIdentity
```

### Logical value identity

Whether two structurally equal values have the same *identity* is a separate question from equality and depends on the type's semantics.

RFC 0020 will define the equality/equivalence taxonomy.

### Representation identity

Two representations may encode the same logical value while having different representation identities.

### Layout identity

Two realization objects may share identical byte layout but differ in evidence, target applicability, or semantic role.

Therefore layout equality does not necessarily imply representation identity.

### Storage identity

Two storage objects may contain representations of equal logical values without being the same storage object.

---

## Canonical semantic representation

### Canonical logical form

MNCS's canonical semantic artifacts should represent logical datatype structure independently of arbitrary surface syntax.

For example, source forms that denote equivalent labeled products should canonicalize to one semantic product form where their semantic identities agree.

### No premature physical canonicalization

Canonical semantic form must not force a physical layout merely to obtain deterministic serialization of the semantic artifact.

The canonical serialization of the **description** of a type is not the runtime representation of values of that type.

### Stable identities

Type, constructor, field, view, and representation realization objects should receive deterministic identities under the repository's existing identity model once their schema is implemented.

### Versioning

Logical type schema versions and representation-realization schema versions may need independent versioning.

Changing one should not automatically invalidate the other unless the dependency graph says it does.

---

## Type witnesses

### Purpose

A `TypeWitness<T>` is a machine-readable object sufficient for some declared type-identity or type-relationship operation.

It need not expose the full implementation of `T`.

### Witness strength

Different witnesses may establish different facts.

Examples:

```text
identity-only witness
shape witness
constructor witness
runtime descriptor
proof-carrying witness
foreign schema witness
```

### Trust

Witnesses participate in RFC 0018 assurance semantics.

A witness produced by an untrusted foreign runtime is not automatically equivalent to a kernel-derived type fact.

### Erasure

A witness may be erasable if no downstream operation depends on it.

---

## Representation witnesses

A representation witness may establish facts such as:

```text
this storage state realizes T under R
this layout satisfies requirement Q
this tag encoding preserves constructors
this backend realization preserves observation set O
```

### Runtime versus compile-time

A representation witness may exist:

- statically;
- as a proof/certificate;
- as runtime metadata;
- as an attestation;
- as a verifier result; or
- as a combination of these.

The method matters under RFC 0018.

### No universal witness authority

A runtime descriptor cannot automatically establish proof-level properties.

A proof about a representation cannot automatically establish that a deployed process actually instantiated that representation.

The claims are distinct.

---

## Representation search

One of the principal machine-native consequences of RFC 0019 is that representation can become an explicit search dimension.

### Search problem

Given:

```text
logical type T
observation interface O
hard representation requirements Q
preferences P
target context C
resource budgets B
assurance profile A
```

Forge may search for candidate realization `R` such that:

```text
R represents T
R supports O
R satisfies Q
R is applicable under C
R stays within B
Evidence(R) satisfies A
```

Then preferences may rank remaining candidates.

### Example: matrix

```text
Logical value:
    Matrix<f32, 1024, 1024>

Allowed logical observations:
    dimensions
    element values

Hard requirements:
    mutable
    alignment >= 64

Representation hole:
    ?
```

Candidate families may include:

```text
row-major
column-major
blocked
SIMD tiled
GPU tiled
sparse
NUMA partitioned
```

A sparse representation is only valid if the logical and operational semantics admit the zero/default treatment required by that representation.

### Example: records

```text
Array<Particle>

Particle = {
    position: Vec3
    velocity: Vec3
    mass: f32
}
```

Forge may consider:

```text
AoS
SoA
AoSoA
compressed positions
split hot/cold fields
GPU-native buffer
```

without changing the declared logical `Particle` value model.

### Search provenance

Every selected realization should retain:

- candidate identity;
- search context;
- constraints;
- evidence;
- rejected candidates where policy requires;
- target facts; and
- transformation provenance.

This aligns with RFC 0004 recursive refinement and RFC 0018 assurance.

---

## Representation holes

A program may intentionally leave representation unspecified.

Conceptually:

```text
representation_of<T> = unresolved
```

is not a type error when no current phase requires a concrete representation.

### Resolution points

A representation hole may be resolved during:

- specialization;
- target selection;
- backend lowering;
- FFI binding;
- deployment; or
- explicit optimization-region search.

### Constraint accumulation

Requirements may accumulate before resolution.

Example:

```text
R must be contiguous
R must be device-visible
R alignment >= 128
R size <= budget
```

The realization solver then searches within the constrained envelope.

### Unsatisfiable requirements

If no realization satisfies all hard requirements, the system should produce an explicit unsatisfied-representation obligation rather than silently weakening requirements.

---

## Representation fallback

Consistent with RFC 0006, machine-native representation search should permit conservative fallback.

Example:

```text
preferred: niche-encoded unboxed representation
fallback: explicit tagged boxed representation
```

If stronger representation evidence is `UNKNOWN`, the system may select the declared conservative fallback if its own requirements are satisfied.

It must not pretend the stronger representation was proven.

---

## Interaction with RFC 0007 proof core

### Proof obligations

Representation realizations may generate proof obligations concerning:

- encode/decode laws;
- constructor preservation;
- index preservation;
- operation refinement;
- range/niche validity;
- layout constraints; or
- transformation equivalence.

### Proof does not establish deployment fact automatically

A proof that representation `R` is semantically valid does not prove that the deployed runtime actually selected `R`.

That latter statement requires realization/build/runtime evidence.

### Kernel boundary

General representation search, optimization, layout computation, and empirical validation stay outside the proof kernel.

Only independently checkable proof objects enter the kernel trust boundary.

---

## Interaction with RFC 0008 effects/resources

Representations may impose resource and capability requirements.

Examples:

```text
pinned memory
GPU-visible storage
shared memory
atomic access
external device buffer
```

The representation itself does not create authority.

Required capabilities must remain explicit.

A representation search may reject candidates whose resource or authority requirements exceed the allowed envelope.

---

## Interaction with RFC 0009 memory and storage

RFC 0009 remains authoritative for:

- memory object identity;
- references;
- provenance;
- lifetime;
- validity;
- allocation;
- storage regions; and
- storage realizations.

RFC 0019 supplies the logical/representational relationship that memory stores.

### Materialization relation

Conceptually:

```text
Value<T>
  --represented by R-->
RepresentationState<R>
  --materialized in-->
StorageObject
```

### No address by default

A logical value has no address unless materialized in addressable storage and observed through the relevant memory semantics.

---

## Interaction with RFC 0010 concurrency

Representation can affect:

- atomicity;
- tearing;
- cache-line interaction;
- false sharing;
- lock-free feasibility;
- memory ordering implementation; and
- synchronization granularity.

Those effects are realization properties.

A concurrent operation may therefore impose representation requirements such as:

```text
single-copy atomic width
natural alignment
isolated cache line
compatible atomic representation
```

RFC 0010 defines the concurrency semantics; RFC 0019 lets candidate representations advertise and prove the necessary physical properties.

---

## Interaction with RFC 0011 failure and external observation

Representation conversion may fail when:

- foreign bytes are invalid;
- dynamic type witnesses mismatch;
- allocation/materialization fails;
- target constraints are absent;
- decoding detects invalid state; or
- representation evidence becomes stale.

Those outcomes must follow RFC 0011 failure/knowledge semantics.

A failed representation check is not equivalent to proof that the logical program itself is invalid.

---

## Interaction with RFC 0012 executable core

RFC 0012 separates values from computations.

RFC 0019 strengthens that distinction.

The executable core may operate over logical values even when concrete target representation has not yet been selected.

Reference interpreters should avoid accidentally hard-coding backend layouts as semantic behavior.

---

## Interaction with RFC 0013 abstraction and polymorphism

### Type abstraction

RFC 0013 governs abstraction, interfaces, polymorphism, and generativity.

RFC 0019 provides the underlying logical value and representation model.

### Representation abstraction

An interface may expose:

- logical structure but hide representation;
- an abstract logical type and hide structure plus representation;
- selected views but hide constructors;
- representation requirements without revealing realization; or
- explicit realization identity when required.

### Parametricity

Representation-polymorphic code should preserve parametricity until it performs an explicit representation observation.

### Specialization

Specialization may resolve representation variables without changing the generic logical contract.

---

## Interaction with RFC 0014 modules/components/linking

Components may export representation-independent logical types.

They may also export explicit ABI/representation contracts when required.

Link compatibility therefore may need to distinguish:

```text
logical type compatibility
interface compatibility
representation compatibility
ABI compatibility
serialization compatibility
```

These are not the same claim.

A component upgrade that changes internal layout may remain logically compatible while invalidating an ABI-sensitive consumer.

---

## Interaction with RFC 0015 unsafe/FFI/ABI

Foreign interfaces are a primary place where representation becomes semantically constrained.

### FFI boundary

An FFI binding may require:

```text
exact field order
exact size
exact alignment
exact calling representation
endianness
padding constraints
stable address
```

Those requirements narrow the representation envelope.

### Unsafe reinterpretation

Reinterpreting storage under a new logical type crosses RFC 0015's unsafe boundary unless a verified safe conversion exists.

### Boundary validity

Foreign bytes must not automatically become valid MNCS values.

Validation or explicit unsafe assumptions are required.

---

## Interaction with RFC 0016 staging and specialization

Representation selection is a natural staged computation.

A type may remain logically fixed while representation becomes progressively constrained through:

```text
source/elaboration
whole-program analysis
target specialization
backend lowering
runtime specialization
```

Stage transitions must preserve evidence validity.

A representation fact established for one target stage cannot silently migrate to another target without applicable evidence.

Self-transformation may propose new representation realizations but promotion remains assurance-gated.

---

## Interaction with RFC 0017 runtime/target context

Representations may depend on target facts such as:

```text
pointer width
alignment rules
vector width
endianness
atomic widths
address spaces
GPU memory model
ABI family
runtime object model
```

Those facts must come from RFC 0017 target/environment evidence.

A target-dependent representation should carry an applicability envelope.

Changing the target invalidates only representation evidence whose dependencies actually changed.

---

## Interaction with RFC 0018 assurance

Representation selection may authorize different actions under different assurance profiles.

Example:

```text
experimental execution:
    bounded differential evidence acceptable

trusted backend promise:
    kernel proof or independently checked certificate required
```

A representation candidate can therefore be:

```text
logically plausible
experimentally authorized
production unresolved
```

without contradiction.

### Conflict preservation

If one verifier supports representation validity and another produces a counterexample, RFC 0018 may classify the relevant claim as `CONFLICTED`.

The representation must not be promoted merely because support count is larger.

---

## Interaction with future RFC 0020 equality semantics

RFC 0019 intentionally avoids freezing full equality rules.

RFC 0020 should define relations among:

```text
definitional equality
propositional equality
logical value equality
structural equality
nominal identity
observational equivalence
representation equivalence
layout equality
bit equality
storage identity
reference identity
behavioral refinement
substitutability
```

RFC 0019 establishes only that these relations are distinct and that representation change normally requires an appropriate equivalence/refinement relation rather than raw bit equality.

---

## Interaction with future RFC 0021 numeric semantics

RFC 0019 treats numeric types structurally and nominally but does not define detailed arithmetic behavior.

RFC 0021 should own:

- integer overflow modes;
- floating-point NaNs/infinities;
- signed zero;
- rounding;
- decimal/fixed-point semantics;
- approximation/error bounds; and
- reproducibility.

A numeric representation realization may be constrained by those semantics.

---

## Interaction with future RFC 0027 serialization

Runtime representation and wire serialization are deliberately different.

Example:

```text
logical type: Message
runtime representation: packed target-native object
wire encoding: canonical little-endian schema v3
```

Neither representation identity implies the other.

Serialization must validate decoded values before they enter the logical value domain.

---

## Compiler and backend consequences

### HIR

HIR should eventually represent:

- logical type identity;
- logical constructor/field identity;
- explicit views where used;
- representation requirements; and
- unresolved representation variables where legal.

### SSA

SSA may progressively lower logical values into representation-specific values.

Traceability must preserve the relationship back to logical identities.

### Backend promises

Backend attributes that depend on representation facts must cite applicable evidence.

Examples:

```text
alignment
nonnull
dereferenceable
noalias
inbounds
noundef
vector alignment
```

RFC 0006 and RFC 0018 remain authoritative for promise gating.

### No backend ontology leakage

LLVM, Cranelift, WebAssembly, CPU register classes, or a particular VM object model should not become the language's fundamental value ontology.

They are realizations.

---

## Forge integration

Forge is expected to be a principal consumer of RFC 0019.

### Representation candidate generation

Forge may generate candidate representations from:

- known families;
- target facts;
- observed workload;
- profiler evidence;
- learned transformations;
- symbolic search;
- proof-guided search; or
- machine-generated novel layouts.

### Candidate isolation

Candidate representations remain untrusted until validated.

### Verification

Forge should request evidence for:

- validity;
- observation preservation;
- required layout facts;
- target applicability;
- complexity/resource bounds; and
- protected semantic properties.

### Promotion

Promotion must satisfy RFC 0004 and RFC 0018 policy.

### Novel machine-native representations

The ontology intentionally does not restrict representation families to human-designed structures.

A machine-generated representation may be admitted if it can express its requirements, observations, validity relationship, and evidence sufficiently for the system to reason about it.

---

## Fabric integration

Fabric may use RFC 0019 to route representation-sensitive tasks.

Examples:

- select a layout-analysis verifier;
- request target-specific representation evidence;
- compare candidate realizations;
- route dynamic-type validation;
- schedule bounded differential testing; or
- isolate an unsafe representation experiment.

Fabric should not itself invent semantic validity merely because several workers agree.

The evidence graph and assurance policy remain authoritative.

---

## RAVEL integration

RAVEL may use the representation model for adversarial and mutation-based testing.

Potential studies include:

- invalid tag states;
- niche collisions;
- padding assumptions;
- endian mismatch;
- stale layout metadata;
- aliasing introduced by packing;
- constructor collapse;
- dynamic-type spoofing;
- invalid foreign bytes;
- closure-environment mismatch; and
- representation-sensitive side effects.

RAVEL results remain empirical evidence unless reduced to stronger independently checkable artifacts.

---

## Security consequences

Representation semantics has direct security significance.

### Type confusion

Separating logical type from raw storage makes unchecked reinterpretation explicit rather than normal.

### Invalid states

Representations must characterize invalid physical states sufficiently to prevent them from silently entering the logical value domain.

### Pointer and capability confusion

A representation that physically resembles a pointer or capability does not grant the semantic authority associated with that object.

### Dynamic type spoofing

Runtime type identifiers require scoped trust and validation.

A forged type tag must not automatically authorize projection.

### Padding and uninitialized bytes

Logical equality should not automatically depend on padding bytes.

Representation-level operations that expose such bytes must be explicit and must respect initialization/security policy.

### Side channels

Representation choices can affect timing, cache behavior, memory footprint, branch patterns, and other side channels.

RFC 0019 records representation-sensitive requirements; full information-flow/side-channel semantics belong to a later security RFC.

### Serialization attacks

Runtime representation must not be assumed safe to transmit or ingest as wire data.

Serialization remains a separate validated boundary.

---

## Diagnostics

Representation-related failures should produce machine-readable obligations rather than vague compiler errors.

Examples:

```text
REPRESENTATION_REQUIREMENT_UNSATISFIED
REPRESENTATION_VALIDITY_UNKNOWN
REPRESENTATION_CONSTRUCTOR_COLLISION
REPRESENTATION_OBSERVATION_NOT_PRESERVED
REPRESENTATION_TARGET_INAPPLICABLE
REPRESENTATION_EVIDENCE_STALE
DYNAMIC_TYPE_WITNESS_MISSING
DYNAMIC_TYPE_MISMATCH
INVALID_REPRESENTATION_STATE
REPRESENTATION_DEPENDENCE_LEAK
```

A diagnostic should identify:

- logical type;
- candidate representation;
- failed requirement;
- responsible observation;
- relevant evidence/assumption dependencies;
- causal slice where available; and
- conservative fallback if one exists.

---

## Introspection

The machine-native semantic graph should eventually expose queries such as:

```text
what logical type does this representation claim to realize?
which representations are currently admissible for T?
which observations constrain representation choice?
why is this field order fixed?
which candidate is smallest?
which candidate has the strongest assurance?
which evidence establishes this niche?
what becomes invalid if target pointer width changes?
which operations explicitly observe representation?
which logical values cannot be represented by candidate R?
```

This introspectability is a core motivation for making representation a first-class semantic object rather than opaque compiler state.

---

## Machine-generated representation families

MNCS should leave room for representation families that humans would not normally design directly.

A candidate may be structurally unusual as long as it can establish:

```text
represented logical domain
observation mapping
valid-state relation
requirements
resource characteristics
target applicability
transformation provenance
evidence
```

The system should evaluate semantic obligations rather than aesthetic familiarity.

This aligns with the broader MNCS principle that machine-native techniques may graduate through repeated evidence instead of being rejected because they are non-idiomatic to human programmers.

---

## Complexity and cost

Representation selection can trade among:

```text
space
latency
throughput
cache locality
energy
copy cost
serialization cost
verification cost
proof size
compile time
runtime dispatch
```

RFC 0019 records that representations may expose such properties.

The formal quantitative cost model belongs to RFC 0032.

Representation validity must remain separate from performance desirability.

A faster invalid representation is invalid.

---

## Representation migration

Changing a representation across a live or persistent boundary may require migration.

RFC 0019 establishes only the need to relate old and new representation states to the same or refined logical values.

Detailed deployment, live-upgrade, durability, and migration semantics belong to later RFCs.

A representation migration must not be assumed lossless merely because both representations implement the same logical type.

The migration process itself may fail or require stronger evidence.

---

## Representation and persistence

A runtime representation should not automatically be treated as a stable persistent format.

Persistent data may require:

- versioned schema;
- crash consistency;
- canonical encoding;
- migration;
- endianness stability;
- pointer elimination; and
- durability semantics.

Those concerns belong primarily to RFC 0026 and RFC 0027.

RFC 0019 preserves the logical-value relationship across them.

---

## Representation and object identity

Some logical types may intentionally expose persistent object identity.

Others may be pure value types where copying or recomputation preserves all legal observations.

RFC 0019 does not force one object model across all types.

Instead, identity observability should be explicit in the type/interface semantics.

If object identity is observable, representation transformations must preserve the required identity relation or be rejected.

---

## Representation and mutability

Mutability is not equivalent to representation.

A mutable logical object can have multiple realizations.

An immutable logical value may still use mutable internal storage if that mutation is unobservable under the exported semantics and is allowed by effect/authority rules.

Conversely, a physically immutable representation may implement a logically evolving abstraction through replacement or persistence.

The relevant observation and effect contracts decide legality.

---

## Representation and laziness

Laziness should not be built into all values as a hidden implementation property if evaluation order is observable through effects, resource usage, timing requirements, or failure.

A lazy/thunked representation is a candidate realization only when the logical computation semantics permit it.

Pure values with no relevant resource/timing observation may allow more freedom.

RFC 0012 and RFC 0008 govern computation/effect consequences.

---

## Representation and sharing

Physical sharing does not necessarily imply logical aliasing or identity.

Two logically independent immutable values may share physical storage.

Two logically identical values may occupy separate storage.

A representation may introduce sharing only if:

- mutation semantics remain correct;
- identity observations remain correct;
- lifetime/provenance rules remain correct; and
- security/resource constraints permit it.

RFC 0009 remains authoritative for storage aliasing.

---

## Representation and canonicalization

Some logical types may require canonical representations for particular actions.

Examples include:

- hashing;
- signing;
- content addressing;
- deterministic build artifacts; or
- equality acceleration.

Canonical representation is an additional contract, not the default definition of the type.

A type may have:

```text
many valid runtime representations
one canonical serialization
```

without contradiction.

---

## Representation and hashing

Hashing a logical value through raw memory bytes is only valid when the representation contract establishes that those bytes canonically correspond to the logical value for the hashing purpose.

Padding, alternate encodings, endianness, pointer values, and hidden metadata can otherwise break semantic hashing.

Content-addressed identities should use declared canonical forms rather than accidental runtime memory images.

---

## Representation and reflection

RFC 0016 reflection may expose:

- logical type structure;
- constructors;
- fields;
- views;
- representation requirements; and
- selected representation facts.

Reflection over logical structure must remain distinguishable from reflection over physical realization.

A program should not gain representation access merely because it can reflect on its logical type.

---

## Representation and self-transformation

A self-transforming MNCS system may propose changes such as:

```text
linked -> packed
AoS -> SoA
boxed -> unboxed
closure -> specialized direct call
```

The proposal must include:

- affected logical subjects;
- changed representation identities;
- unchanged protected logical properties;
- new requirements/assumptions;
- evidence;
- invalidated evidence; and
- rollback/fallback relationship where applicable.

This turns representation optimization into an auditable semantic transformation rather than an opaque compiler trick.

---

## Candidate object model

The following object vocabulary is intentionally provisional:

```text
LogicalType
LogicalValue
Constructor
Field
RecursiveBinder
Index
TypeIdentity
TypeWitness
NominalIdentity
View
Observation
ObservationSet
RepresentationRequirement
RepresentationPreference
RepresentationVariable
RepresentationFamily
RepresentationRealization
RepresentationState
LayoutDescriptor
RepresentationWitness
RepresentationEvidence
RepresentationValidityClaim
EncodeRelation
DecodeRelation
DynamicPackage
RepresentationTransformation
RepresentationFallback
```

Not every object must survive as a distinct implementation type.

The RFC's requirement is that the semantic distinctions remain representable.

---

## Example: option niche encoding

Logical definition:

```text
Option<NonNullRef<T>> =
    None
    | Some(NonNullRef<T>)
```

Candidate realization:

```text
R_nullable_pointer
```

Requirements:

```text
carrier supports distinguished null state
valid Some payload can never be null
pointer representation applicable to target
```

Obligations:

```text
None -> null
Some(p) -> p
null decodes only to None
all valid p decode only to Some(p)
```

If target pointer semantics change, evidence may become stale.

Fallback:

```text
R_explicit_tagged
```

This is exactly the kind of stronger optimization that should be explicit and evidence-bearing rather than implicit folklore.

---

## Example: tree view over B-tree realization

Logical type:

```text
Tree<T>
```

Logical view:

```text
Empty
Node(value, left, right)
```

Candidate physical realization:

```text
BTreePages<T>
```

The realization may not contain one physical node for every logical `Node` observation.

Instead it must implement the view relation correctly.

A client that only uses the logical view remains representation-independent.

A client requesting physical child pointers would require a different representation contract.

---

## Example: dynamic package

Producer emits:

```text
DynamicPackage {
    hidden_type = PacketV3
    witness = W_packet_v3
    representation = R_packet_compact
}
```

Consumer requests:

```text
project<PacketV3>(package)
```

The system checks the type witness and relevant representation validity.

Possible results:

```text
MATCH(PacketV3 value)
MISMATCH
UNRESOLVED(stale witness)
```

The consumer is not permitted to reinterpret payload storage blindly.

---

## Example: representation-sensitive FFI

Logical type:

```text
Color {
    r: u8
    g: u8
    b: u8
    a: u8
}
```

Ordinary internal code may permit many layouts.

An FFI requirement may state:

```text
layout = C-compatible
field_order = r,g,b,a
size = 4
alignment = 1
```

That requirement applies to the boundary realization.

The program may still use a different internal representation and convert at the boundary if permitted.

---

## Example: function defunctionalization

Logical functions:

```text
f : A -> B
g : A -> B
```

A higher-order structure stores one of them.

Initial realization:

```text
closure { code_ptr, env_ptr }
```

Whole-program specialization discovers only three closure shapes and transforms them into:

```text
Callable = FCase(data_f) | GCase(data_g) | HCase(data_h)
```

with a direct dispatch function.

If behavior, captured authority, effects, and required observations are preserved, logical function meaning remains unchanged.

---

## Example: representation observation narrows search

Program A:

```text
for p in particles:
    update(p)
```

No layout observation exists.

Forge may consider AoS, SoA, AoSoA, and device layouts.

Program B additionally states:

```text
assert field_offset(Particle.mass) == 24
```

That explicit representation observation narrows eligible realizations.

MNCS should make the reason visible in the semantic graph.

---

## Non-goals

RFC 0019 does not attempt to define:

- the final surface syntax for types or constructors;
- one final dependent type calculus;
- one universal subtyping relation;
- one universal semantic-subtyping algorithm;
- the complete equality theory;
- one garbage collector or allocator;
- pointer provenance;
- ownership/lifetime rules beyond their interaction with representations;
- one object model;
- one closure ABI;
- one FFI ABI;
- one serialization format;
- one persistent schema system;
- full numeric semantics;
- full information-flow security;
- full gradual typing;
- universal runtime reflection;
- mandatory category-theory syntax;
- mandatory layout canonicalization;
- universal representation inference; or
- automatic acceptance of machine-generated representations.

---

## Rejected directions

### Fixed layout as ordinary type identity

Rejected as the default because it unnecessarily destroys representation freedom.

Explicit fixed-layout types/requirements remain possible.

### Universal untyped `Any`

Rejected because it erases type identity and encourages unchecked reinterpretation.

Use existential/dynamic packages with witnesses.

### Universal implicit bottom

Rejected for ordinary executable values because it conflates values with failed/diverging computations.

### Compiler-secret representation

Rejected as the sole model because MNCS needs machine-readable realization constraints, evidence, introspection, and recursive refinement.

### Every representation fact in the proof kernel

Rejected because empirical target facts, backend behavior, runtime state, and dynamic realization cannot all be reduced sensibly to proof terms.

Use RFC 0018 evidence classes and keep the proof kernel narrow.

### Set equality as universal type identity

Rejected because nominal, generative, opaque, resource, and capability identities may remain semantically distinct despite equal apparent value sets.

### Physical pattern matching by default

Rejected because it destroys abstraction and representation independence.

Ordinary patterns should observe constructors/views.

---

## Unresolved research questions

### Logical type universe

Which primitive type constructors belong in the first executable core beyond:

```text
Zero
Unit
Product
Sum
Function
Inductive
```

### Recursive semantics

How much iso-recursive structure should remain explicit in canonical IR?

Should some equi-recursive equivalence be computed automatically for selected structural types?

### Coinductive core

Should coinductive data be represented directly in the logical type model or primarily through RFC 0012 computation/transition semantics?

### Set-theoretic fragment

Which union/intersection/negation relations are decidable and useful enough for the first implementation?

### Refinement types

How should executable refinements interact with RFC 0007 propositions and with runtime checks?

### Runtime type witnesses

What is the minimum stable witness vocabulary?

How should witnesses be versioned and authenticated across module/FFI boundaries?

### Representation description language

How expressive should the first representation descriptor be?

Should it model arbitrary bit-level layouts or begin with a bounded family vocabulary?

### Encode/decode proof strength

Which realizations require bijection, injection, partial decoding, observational refinement, or only operation-specific correctness?

### Representation identity

What fields contribute to representation identity versus realization instance identity?

### Observability

What representation facts can safe code observe by default, if any?

Which observations require capabilities or unsafe authority?

### Views

How should multiple views compose?

Can view definitions themselves be optimized or generated?

### Function values

How should semantic function identity be represented without promising decidable function equality?

### Dynamic packages

When is a dynamic projection `MISMATCH` versus `UNRESOLVED`?

What evidence class is required to trust a foreign runtime type witness?

### Representation-polymorphic ABI

Can components remain representation-polymorphic across linking boundaries, and what runtime descriptor cost would that require?

### Representation search

Which representation families should be built in versus discovered dynamically?

How should candidate search avoid combinatorial explosion?

### Evidence

Which representation claims deserve kernel proofs versus certificates versus bounded differential execution?

### Side channels

When do performance/timing differences become semantic observations rather than optimization metrics?

### Persistence

How should persistent representation migration relate to logical value identity?

### Canonicalization

Should some types be able to require a unique canonical runtime representation, and if so how is that distinguished from canonical serialization?

---

## Initial implementation experiment

The first implementation should remain deliberately small.

It should not attempt to build a complete new type checker or automatic representation optimizer.

### Phase 1: versioned logical datatype schema

Add a small experimental schema supporting:

```text
Unit
Product
Record
Sum
Variant
```

with deterministic identities for:

- logical types;
- fields; and
- constructors.

Do not add recursive/coinductive types in the first commit unless the bounded schema remains obviously manageable.

### Phase 2: logical versus representation identity

Introduce explicit:

```text
LogicalTypeId
RepresentationId
```

and verify that two representations can refer to one logical type without changing its identity.

### Phase 3: bounded representation descriptors

Support two or three representation families only, for example:

```text
explicit_tagged
niche_encoded
field_reordered
```

Avoid a universal bit-layout DSL initially.

### Phase 4: representation requirements

Add a small requirement vocabulary such as:

```text
alignment_at_least
fixed_size
field_order
unboxed
```

with hard requirement versus preference distinction.

### Phase 5: one semantic view

Implement one view over a record or variant whose logical decomposition remains stable across two physical layouts.

### Phase 6: representation validity claims

Generate explicit obligations for:

- constructor distinguishability;
- field preservation; and
- applicable target facts.

At least one obligation should be able to remain `UNKNOWN`.

### Phase 7: niche optimization fixture

Use a tiny option-like datatype and compare:

```text
explicit tagged representation
niche representation
```

The niche candidate should be rejected when its reserved-state assumption is absent or stale.

### Phase 8: representation-sensitive observation fixture

Demonstrate that a field-reordering representation is:

- allowed for a representation-independent consumer; and
- rejected for a consumer that explicitly requires one field order.

### Phase 9: bounded reference execution

Run a frozen corpus through both valid realizations and compare logical observations using the existing bounded reference-execution infrastructure.

Treat the result as empirical evidence, not universal proof.

### Phase 10: assurance gate

Use RFC 0018 to define two policies:

```text
experimental_representation
trusted_representation
```

A bounded differential result may authorize the first.

The second should require stronger evidence or remain `UNRESOLVED`.

### Success criteria

The first experiment succeeds if it demonstrates all of the following:

1. one logical datatype has multiple representation identities;
2. representation selection does not change logical type identity;
3. a representation requirement narrows the candidate set;
4. a representation-sensitive observation prevents an otherwise legal transformation;
5. a niche representation is rejected when its validity assumption is unavailable;
6. stale representation evidence is invalidated narrowly;
7. a conservative fallback remains selectable;
8. reference execution observes equivalent logical behavior over a bounded corpus; and
9. assurance policy distinguishes experimental use from stronger promotion.

It does **not** need to demonstrate:

- universal representation equivalence;
- a verified compiler;
- arbitrary recursive datatypes;
- automatic whole-program layout optimization;
- a general proof of representation independence; or
- a production runtime type system.

---

## Longer-term experimental sequence

After the first bounded experiment, likely follow-on studies include:

1. recursive list/tree representations;
2. AoS versus SoA transformations;
3. closure conversion and defunctionalization;
4. indexed vectors with erased versus runtime indices;
5. dynamic packages with type witnesses;
6. foreign-layout validation;
7. target-dependent SIMD representation selection;
8. GPU/device representation requirements;
9. representation-aware proof obligations;
10. machine-generated novel representation candidates; and
11. representation migration across persistent or live-upgrade boundaries.

Each should remain separately evidence-gated rather than being accepted simply because the representation framework exists.

---

## Relationship to the MNCS research philosophy

RFC 0019 follows several broader MNCS principles.

### Semantics before syntax

Logical data structure is defined before choosing human syntax.

### Machine intent is explicit

Representation requirements are declared rather than hidden in idioms.

### Strong optimization requires stronger evidence

A compact or exotic representation does not gain authority merely because it is faster.

### Conservative fallback remains available

When stronger representation obligations are unresolved, a declared conservative representation can remain legal.

### Recursive refinement is bounded

Representation candidates are proposed, evaluated, and promoted through the same bounded refinement model as other machine-generated changes.

### Machine-native does not mean opaque

A representation may be novel or difficult for humans to invent, but its semantic relationships, requirements, evidence, and effects must remain machine-readable and auditable.

### Backend details are not ontology

CPU registers, LLVM structs, object headers, pointer tagging, and VM layouts are realizations rather than the language's foundational definition of value.

---

## Constitutional summary

RFC 0019 proposes the following durable commitments:

> **Values are semantic information, not memory objects.**

> **Types describe logical value domains and structure, not default machine layout.**

> **Constructors and views define semantic observations independently of physical encoding.**

> **Representations are explicit realizations selected under requirements, target context, and evidence.**

> **Representation change is legal only when required logical observations and protected properties are preserved.**

> **Representation-sensitive observation is explicit and narrows optimization freedom.**

> **Dynamic values preserve hidden type identity and witnesses rather than becoming untyped bytes.**

> **Invalid physical states are not logical values.**

> **Storage, address, serialization, ABI, and representation identity are separate concerns.**

> **Representation validity, applicability, and optimization authority are evidence-bearing and assurance-gated.**

> **The language may intentionally leave representation unresolved so Forge and later stages can search the realization space.**

> **Machine-generated representations are judged by semantic obligations, not by familiarity or human idiom.**

This establishes a value/type foundation that preserves high-level semantic rigor while giving MNCS unusually large freedom to discover, verify, and deploy machine-native physical representations.