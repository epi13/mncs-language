# RFC 0009: Machine-Native Memory, Reference, Provenance, and Storage Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.4–1.0
- **Depends on:** RFC 0002, RFC 0003, RFC 0006, RFC 0007, RFC 0008

## Summary

MNCS Language should define memory in terms of semantic identity, provenance, authority, lifetime,
validity, disjointness, resource ownership, and storage requirements rather than defining a pointer
as merely an address or selecting one universal memory-management mechanism.

The central rule is:

> **An address is a realization, not an identity. A reference is a provenanced capability over a
> semantic memory object or range.**

A machine-authored program should be able to state what must be true of memory without prematurely
choosing how that memory is physically managed. Stack allocation, arenas, slabs, tracing garbage
collection, reference counting, region allocation, static storage, shared memory, device memory,
pinned buffers, persistent mappings, or target-specific capability hardware may all be valid
realizations when they preserve the same declared semantics.

This RFC synthesizes the most promising ideas from several traditions rather than adopting one memory
model wholesale:

- **separation logic and resource semantics** for local reasoning and explicit disjointness;
- **capability calculi** for authority over access, mutation, relocation, pinning, retyping, and
  deallocation;
- **linear, affine, uniqueness, and quantitative typing** for alias and permission accounting;
- **explicit provenance models** for reference derivation and optimizer-safe alias semantics;
- **region and lifetime systems** for semantic lifetime domains without requiring one allocator;
- **abstract interpretation and finite lattices** for conservative joins and explicit `UNKNOWN`;
- **typed initialization and representation semantics** for distinguishing storage, bytes, and valid
  values; and
- **proof/evidence integration** so stronger memory facts may be proved, checked, inferred, measured,
  or remain unresolved without being silently laundered into compiler promises.

CHERI-like hardware capabilities, Rust-family borrow models, shape analysis, garbage collectors,
arenas, allocators, compactors, and address-space-specific mechanisms are important research inputs
and realizations, but they are not selected as the semantic foundation.

The goal is not merely memory safety. The goal is to preserve enough machine-relevant structure that
an autonomous generator, verifier, optimizer, scheduler, runtime, or backend can reason directly
about aliasing, ownership, relocation, initialization, locality, address stability, lifetime,
external ownership, representation, and proof obligations without reconstructing those facts from
source idioms.

## Motivation

Conventional systems languages often collapse several distinct concepts into a pointer or reference:

```text
identity
address
location
provenance
lifetime
access authority
aliasing relationship
representation
initialization state
storage-management policy
```

These concepts are related but not identical.

A numeric address may change while the semantic object remains the same. Two references may carry the
same machine address while differing in provenance or authority. A block of allocated bytes may exist
without yet containing a valid value. A reference may permit reading but not mutation, relocation, or
deallocation. A region may group objects by lifetime without implying arena allocation. A DMA engine
may temporarily own a buffer while the originating task retains only a completion capability.

Human-oriented languages often make one memory-management discipline central and then add escape
hatches around it. Examples include ownership/borrowing, tracing garbage collection, explicit
allocation, region allocation, reference counting, or manually managed raw pointers. Each solves
important problems, but selecting one as the language's fundamental ontology can hide implementation
freedom that a machine author and machine optimizer could otherwise exploit.

MNCS already separates semantic operations from verified physical realizations in RFC 0006 and RFC
0008. Memory should follow the same principle.

The semantic artifact should describe facts such as:

```text
object A exists through event E9
A may move before E4
A must have a stable address from E4 through E7
A requires at least 64-byte alignment
A is exclusively writable by task T
subranges A[0..N) and A[N..M) are disjoint
A[0..K) is initialized as T
A may be read by device D after registration
A may be deallocated only after all derived live access authority is gone
```

The realization may then choose a stack slot, arena, slab, GC heap, pinned pool, NUMA-local mapping,
device allocation, or another mechanism that satisfies those requirements.

For machine-native generation, this inversion is valuable because machines can manipulate a richer
constraint graph than a human programmer would reasonably maintain by hand.

## Design goals

RFC 0009 should make the following possible.

1. **Separate identity from location.** Semantic object identity must not be defined by a current
   numeric address.
2. **Make provenance explicit.** A reference must carry or derive from authority tied to a bounded
   subject, range, epoch, and derivation context.
3. **Make authority compositional.** Read, write, split, join, pin, relocate, expose-address, retype,
   share, and deallocate authority must be distinguishable.
4. **Make disjointness a first-class fact.** Independent regions or subranges should support local
   proof, parallelization, alias-sensitive optimization, and narrow evidence invalidation.
5. **Track temporal validity.** Reusing the same physical address for a later allocation must not
   revive references to an earlier allocation epoch.
6. **Separate storage from values.** Allocated bytes, initialized representation, and valid logical
   values are distinct states.
7. **Permit multiple memory-management realizations.** The semantic model must not require one
   universal allocator, collector, or ownership implementation.
8. **Support explicit stability and locality requirements.** Pinning, device visibility, NUMA
   locality, cache policy, persistence, and address-space requirements should constrain realizations
   only when semantically relevant.
9. **Preserve proof and evidence structure.** Memory facts consumed by optimization must be bound to
   current scope, dependencies, target, and assumptions.
10. **Remain inspectable.** Humans should be able to inspect object identity, ownership, authority,
    lifetime, provenance, and selected realization even if machines author the canonical form.
11. **Compose with I/O and concurrency.** RFC 0009 must provide the memory resource model required by
    RFC 0008 and the future concurrency/memory-consistency model without prematurely defining that
    entire model here.
12. **Expose a useful search space.** Forge or another machine optimizer should be able to compare
    alternative storage strategies under explicit constraints and evidence.

## Non-goals

This RFC does not:

- select a final surface borrowing syntax;
- declare that MNCS "uses Rust ownership";
- require garbage collection or prohibit it;
- require arenas, regions, reference counting, or explicit `free`;
- define the complete concurrency memory-consistency model;
- define every target address space or cache-coherence mechanism;
- place allocator, collector, hardware, or operating-system correctness inside the proof kernel;
- make arbitrary pointer arithmetic safe merely by tracking an address;
- treat a numeric address as sufficient authority to access memory;
- require shape analysis or theorem proving for ordinary memory operations; or
- promise that every alias, lifetime, layout, or provenance property is statically decidable.

Unsupported properties may remain `UNKNOWN`, require a runtime check, select a conservative lowering,
or reject promotion under the active assurance profile.

## Selected theoretical synthesis

The proposal deliberately combines a small set of theories whose strengths reinforce one another.

### Separation and resource semantics

Separation logic contributes the idea that assertions and ownership can describe distinct portions
of memory independently. The most important MNCS concept is not the surface `*` operator itself but
**local reasoning over explicit memory footprints**.

Conceptually:

```text
Own(A) * Own(B)
```

states that the two owned resources are disjoint under the relevant model.

MNCS should generalize that idea into semantic footprint and resource objects that can be consumed by
verification, optimization, causal slicing, and evidence invalidation.

A resource may be subdivided:

```text
Own(Buffer<B>, 0..4096)
    =>
Own(Buffer<B>, 0..2048)
*
Own(Buffer<B>, 2048..4096)
```

and later rejoined when the required conditions hold.

The key machine benefit is that disjointness becomes a positive fact rather than something an
optimizer repeatedly tries to infer from pointer patterns.

### Capability calculus

Capability-based typed memory management contributes the separation between possession of a
reference and authority to perform operations on the referenced storage.

MNCS should distinguish at least:

```text
Read<A>
Write<A>
Split<A>
Join<A>
Share<A>
Relocate<A>
Pin<A>
ExposeAddress<A>
Retype<A, T>
Deallocate<A>
```

These authorities may be combined, attenuated, delegated, borrowed, consumed, or withheld.

A component that can mutate an object does not automatically gain authority to deallocate it. A
component that can read a buffer does not automatically gain authority to expose its address or pin
it for a device. A runtime that can relocate an object does not automatically gain authority to
change its logical representation.

This distinction composes directly with the authority model established by earlier MNCS RFCs.

### Linear, affine, uniqueness, and quantitative permissions

Memory permissions should be resource-like rather than a collection of unrelated ad hoc modifiers.

The initial model should reserve usage disciplines including:

- unrestricted/copyable observation authority where safe;
- affine capabilities usable at most once;
- linear capabilities that must participate exactly once in a state transition;
- unique/exclusive ownership for conflicting mutation authority;
- shared-read authority;
- bounded borrows tied to a lifetime or event interval; and
- quantitative grades for divisible or countable authority.

The exact algebra remains research work, but its laws must be explicit.

Conceptually:

```text
ExclusiveWrite<A>
    -> split_read
SharedRead<A, q1> + SharedRead<A, q2>
```

may be legal only if the permission algebra proves that the split preserves validity and that
exclusive mutation authority cannot coexist with conflicting readers.

MNCS should not require literal rational fractions. A finite symbolic permission algebra may be
simpler, more checkable, and better suited to machine generation. The essential requirement is that
permission splitting and joining are explicit semantic transformations with stable identities and
obligations.

### Explicit provenance and derivation graphs

Modern aliasing research demonstrates that an address alone is insufficient to describe what a
pointer may legally access. MNCS should make that fact foundational rather than reconstructing it
later around an unsafe pointer model.

A reference derivation may form a graph:

```text
Allocation A @ epoch 7
    |
    +-- Ref P [0..4096)
    |     |
    |     +-- Ref P1 [0..2048)
    |     +-- Ref P2 [2048..4096)
    |
    +-- Ref Q [metadata-only authority]
```

A derived reference carries provenance tied to the subject, epoch, range/path, authority, and
construction rule.

MNCS should borrow the lessons of Stacked Borrows and Tree Borrows without adopting either model as
its surface semantics. Those systems show that reference derivation structure matters to both unsafe
validity and optimization. MNCS already has a semantic graph and can represent provenance directly
rather than requiring a human-facing borrow stack or tree to be reconstructed after parsing.

### Regions as semantic lifetime domains

Region-based memory management contributes a powerful distinction: objects may share lifetime or
resource constraints without their physical management strategy being fixed in source.

MNCS regions should therefore be **semantic domains**, not synonyms for arenas.

A region may state:

```text
Region<R> {
    members
    lifetime_constraints
    authority_boundary
    mobility_policy
    visibility_requirements
    locality_requirements
    resource_budget
}
```

Possible realizations include an arena, stack frame, GC generation, slab, shared-memory segment,
device allocation set, or another strategy.

The compiler may infer regions, preserve explicit regions, split or merge realizable regions when
semantics permit, or reject a candidate whose realization cannot satisfy the region's constraints.

### Conservative abstract interpretation

Precise provenance, aliasing, shape, or lifetime facts will not always survive control-flow joins or
foreign boundaries.

MNCS should represent conservative knowledge directly rather than forcing false precision.

For provenance, an initial lattice might include:

```text
Exact(P)
FiniteSet({P1, P2, ...})
SubjectBound(A)
Unknown
Invalid
```

The exact lattice is open design work. The important rule is monotonic conservatism:

> Loss of precision may remove optimization authority, but it must not manufacture stronger
> provenance or permission.

At a join:

```text
if condition {
    r = reference_from(A)
} else {
    r = reference_from(B)
}
```

MNCS may derive:

```text
Prov(r) = {A, B}
```

rather than inventing a single exact provenance.

`UNKNOWN` must never be interpreted as disjoint, exclusive, initialized, or otherwise safe merely
because a target commonly behaves that way.

### Typed initialization and representation

Memory safety requires more than bounds and lifetime. Storage can exist without containing a valid
logical value.

MNCS should distinguish:

```text
Unallocated
Allocated<Uninitialized>
Allocated<InitializedRepresentation>
Valid<T>
PartiallyInitialized<RangeSet>
Moved
Dead
```

The model may track initialization at object, field, element, or range granularity depending on the
operation and proof budget.

Raw bytes and typed objects are distinct:

```text
Bytes<N>
StorageOf<T>
T
```

Interpreting bytes as `T` is a semantic transition with obligations, not an unchecked cast whose
meaning depends on backend folklore.

### CHERI and capability hardware as realization evidence

CHERI demonstrates that bounds, permissions, and capability provenance can be enforced in hardware.
MNCS should treat that as a particularly strong target realization, not a mandatory platform model.

A semantic memory capability might lower to:

```text
CHERI capability
software bounds check
WebAssembly linear-memory bounds
MPU region
page-table protection
fat pointer
proof-elided native pointer
```

depending on target support and evidence.

The language-level capability must retain the same semantic identity even when its physical
representation changes.

### Shape analysis as optional evidence

Shape analysis and related heap analyses can establish facts such as acyclicity, tree structure,
sharing, or disjoint subgraphs. These facts can be valuable for machine-native optimization but
should not be mandatory parts of ordinary type checking.

A verifier may establish:

```text
shape(root) = acyclic_tree
subtrees(root.left, root.right) = disjoint
```

and produce evidence that permits a parallel or specialized realization.

If the verifier cannot establish the property, the result remains `UNKNOWN`; a conservative
realization remains valid.

## Core semantic taxonomy

RFC 0009 proposes that the language keep the following concepts distinct.

### Logical value identity

A logical value is the language-level meaning of data. Some values are pure and need no stable
identity beyond their content. Other values participate in identity-sensitive semantics.

### Object identity

An object identity names a semantic entity whose state or lifetime matters independently of its
current physical address.

```text
ObjectId<O>
```

Object identity survives relocation when relocation is permitted.

### Allocation identity

An allocation identity names one storage allocation event and its bounded extent.

```text
AllocationId<A>
```

Allocation identity is not reusable merely because an allocator later reuses the same numeric
address.

### Lifetime epoch

An epoch distinguishes temporal incarnations of storage.

```text
Epoch<A, 7>
```

A reference to `A@7` cannot become valid for `A@8` after deallocation/reallocation unless an explicit
boundary operation constructs new provenance under new authority.

### Region identity

A region groups memory objects, allocations, or constraints for lifetime, authority, locality, or
management purposes.

```text
RegionId<R>
```

### Reference identity

A reference is a semantic capability derived for a subject and scope.

```text
ReferenceId<P>
```

Two references may designate the same object or address while differing in authority, provenance,
lifetime, range, or derivation.

### Provenance identity

Provenance records the authority-bearing derivation relationship between a reference and a memory
subject.

```text
ProvenanceId<Pr>
```

### Realization identity

A realization identifies the selected physical memory strategy for a semantic object or region.

```text
RealizationId<M>
```

Changing allocator, collector, placement, target address space, pinning mechanism, or relocation
strategy changes realization identity without necessarily changing semantic object identity.

### Machine address

A machine address is an observable numeric or target-specific location representation.

```text
MachineAddress<Target, Space>
```

An address is not authority by itself.

## Proposed memory object model

Conceptually:

```text
MemoryObject {
    object_identity: ObjectId,
    allocation_identity: AllocationId?,
    region_identity: RegionId?,

    logical_type: TypeRef?,
    representation: RepresentationRef,
    extent: Extent,
    alignment: AlignmentRequirement,

    initialization: InitializationState,
    validity: ValidityState,
    epoch: LifetimeEpoch,

    ownership: OwnershipState,
    permissions: PermissionState,
    provenance_root: ProvenanceRoot,

    mobility: MobilityPolicy,
    stability_requirements: [StabilityConstraint],

    lifetime_constraints: LifetimeConstraintSet,
    locality_requirements: PropertySet,
    visibility_requirements: PropertySet,
    persistence_requirements: PropertySet,

    authority: [CapabilityRef],

    requirements: PropertySet,
    preferences: PreferenceSet,
    lowering_envelope: LoweringEnvelope,
    portability_envelope: PortabilityEnvelope,

    obligations: [ObligationRef],
    evidence_policy: EvidencePolicy,
}
```

Not every field must survive in final syntax or even in one runtime representation. This is a
semantic inventory.

## Proposed reference model

Conceptually:

```text
Reference {
    identity: ReferenceId,
    subject: MemorySubject,
    path_or_range: MemoryPath,

    provenance: ProvenanceRef,
    epoch: LifetimeEpoch,

    access_authority: PermissionSet,
    quantity: PermissionGrade,

    lifetime_bound: LifetimeConstraint,
    stability_dependency: StabilityConstraint?,

    assumptions: [AssumptionRef],
    obligations: [ObligationRef],
}
```

A reference is valid for a particular operation only when the required subject, epoch, range,
initialization, lifetime, authority, and provenance conditions hold.

The language should avoid a single context-free predicate such as `valid(pointer)`. Validity is
operation-relative:

```text
valid_for_read(ref, T, range)
valid_for_write(ref, T, range)
valid_for_retype(ref, T -> U)
valid_for_deallocate(owner)
valid_for_address_exposure(ref)
```

## Permission algebra

The permission system should be defined as an algebra with explicit construction and combination
rules.

Candidate permission atoms include:

```text
Observe
Read
Write
ExclusiveWrite
Split
Join
ShareRead
Relocate
Pin
ExposeAddress
Retype
Deallocate
ForeignTransfer
DeviceTransfer
```

Permission transformations must state:

```text
inputs
outputs
consumed authority
preserved authority
new obligations
invalidated evidence
```

### Splitting

A split may divide one memory footprint into provably disjoint parts:

```text
split(A, [0..N), [N..M))
```

producing separate references only after proving or checking:

```text
0 <= N <= M <= extent(A)
ranges_disjoint
source_permission_permits_split
```

### Joining

Joining requires compatible provenance, epoch, subject, and permission state. Two arbitrary adjacent
addresses are not sufficient.

### Shared observation

Shared immutable observation may be duplicated where the permission algebra and concurrency policy
permit it.

### Exclusive mutation

Exclusive mutation requires absence of conflicting live authority over the same semantic footprint,
not merely the absence of syntactically visible references.

### Attenuation

Authority may be reduced deliberately:

```text
ExclusiveWrite<A> -> Read<A>
```

The reverse direction requires new evidence or a valid join/recovery rule. It is not implicit.

## Footprints and subobjects

Memory operations should identify the semantic footprint they access.

A footprint may be expressed as:

```text
Object<O>
Field<O, f>
Range<O, start, end>
IndexRange<O, i, j>
Region<R>
SymbolicFootprint<F>
```

The model should support exact and conservative footprints.

Disjointness facts may be structural, arithmetic, type-derived, verifier-derived, or runtime-checked.

Examples:

```text
disjoint(Field<O, left>, Field<O, right>)
disjoint(Range<B, 0, 2048>, Range<B, 2048, 4096>)
```

These facts feed alias analysis, parallelization, DMA safety, local proof, and evidence invalidation.

## Provenance rules

### Provenance creation

New provenance may arise only from declared construction rules such as:

- allocation;
- derivation from a currently valid reference;
- splitting a proven footprint;
- authorized region projection;
- foreign import under explicit assumptions;
- MMIO/device mapping under RFC 0008 authority;
- target-specific capability construction; or
- a declared unsafe boundary with generated obligations.

### Provenance preservation

Pointer arithmetic or address manipulation may preserve provenance only when the operation's semantic
rule says it does and the resulting reference remains within its permitted subject/range envelope.

### Address exposure

Converting a reference to an address is an observable operation requiring explicit authority when it
may constrain relocation, expose sensitive layout, or cross a foreign boundary.

```text
address.expose(ref, capability)
```

The result is an address value. It is not automatically a dereferenceable reference.

### Address reconstruction

Reconstructing a reference from an address requires an explicit source of provenance/authority.

```text
reference.from_address(address, provenance_capability)
```

If provenance cannot be established, the result may remain an opaque address or a foreign/unsafe
reference with unresolved obligations. The compiler must not invent provenance because the address
falls numerically inside an allocation.

### Provenance joins

When control flow merges references with different provenance, the result must conservatively reflect
all possible origins or lose optimization authority.

No join rule may silently turn a finite or unknown provenance set into a stronger exact provenance.

## Lifetime and epoch semantics

Lifetime should be represented as constraints and relations rather than only lexical source regions.

Candidate relations include:

```text
created_after(E1)
live_until(E9)
outlives(X, Y)
live_during(EventRange)
invalid_after(Transition)
```

Lexical lifetimes remain a useful elaboration technique but are not the only semantic form.

### Epoch rule

Every allocation or external memory incarnation has an epoch identity sufficient to distinguish
temporal reuse.

After:

```text
deallocate(A@7)
allocate -> A@8 at same physical address
```

references derived from `A@7` remain invalid.

This rule prevents address reuse from laundering temporal provenance.

### Deferred lifetime resolution

When lifetime cannot be resolved statically, the language may:

- insert a bounded runtime check;
- require a managed realization such as tracing GC or reference counting;
- select a longer-lived region;
- reject a candidate realization; or
- preserve an unresolved obligation under a research profile.

The semantic requirement remains stable while the management strategy may change.

## Regions and lifetime domains

Regions are first-class semantic groupings.

Possible region constraints include:

```text
all members die together
members may relocate within region
members must remain NUMA-local
region may be transferred as a unit
region requires bounded total bytes
region must be device-visible
region is persistent across process restart
region forbids address exposure
```

A region may be explicit, inferred, generated by optimization, or introduced during lowering.

### Region splitting and merging

A compiler may split or merge regions only when it preserves:

- object identity;
- lifetime requirements;
- authority boundaries;
- provenance relationships;
- locality and visibility requirements;
- persistence semantics;
- proof/evidence dependencies; and
- any observable allocation/ordering behavior declared by the program.

## Storage, initialization, and valid values

Allocation creates storage, not necessarily values.

Conceptually:

```text
allocate(bytes = N)
    -> Allocation<A>
    + Own<A>
    + Deallocate<A>
    + Uninitialized<A, 0..N>
```

Writing bytes may change initialization state without establishing that the representation is a valid
`T`.

### Initialization ranges

For arrays, buffers, and aggregate construction, initialization may be tracked by ranges or fields:

```text
Initialized<A, [0..417)>
Uninitialized<A, [417..1024)>
```

Tracking precision may vary by assurance profile. A coarse implementation may collapse to whole-object
states and reject transformations that require finer information.

### Logical validity

A representation is a valid `T` only when the type's validity predicate is satisfied.

Potential obligations include:

```text
size_compatible
alignment_compatible
all_required_bytes_initialized
representation_valid_for_T
niche_or_tag_valid
provenance_requirements_satisfied
lifetime_started
```

### Partial construction

Incremental construction should be a normal typed state transition rather than an inherently unsafe
operation.

### Move semantics

Moving a logical value may invalidate the old value state while preserving or changing the underlying
allocation according to the operation.

`Moved` is distinct from `Dead` storage.

## Bytes, representations, and retyping

MNCS should distinguish:

```text
Bytes<N>
StorageOf<T>
Representation<T>
T
```

A retyping operation must state its semantic contract.

```text
retype<Bytes<N>, T>(storage, evidence)
```

may require:

- sufficient extent;
- required alignment;
- initialized bytes;
- representation validity;
- correct endianness where semantically visible;
- compatible lifetime/epoch;
- sufficient authority; and
- provenance compatible with the destination interpretation.

A backend cast is merely one possible realization of an already justified semantic transition.

## Allocation and deallocation

### Allocation

An allocation event should produce explicit resource identities rather than only a pointer.

```text
AllocationResult {
    allocation_identity
    epoch
    storage_extent
    ownership
    deallocation_authority?
    initialization_state
    realization_constraints
}
```

### Deallocation

Deallocation consumes appropriate authority and requires proof/checking of relevant safety
conditions.

Typical obligations include:

```text
subject_is_current_epoch
caller_has_deallocation_authority
no_live_conflicting_access_authority
no_inflight_device_or_foreign_owner
required_destructor_or_finalizer_semantics_completed
persistent_commit_constraints_satisfied
```

The exact obligations depend on the object's policy.

### Deallocation is not universal

Some objects may have no explicit deallocation capability because they are static, process-lifetime,
region-owned, GC-managed, externally owned, or persistent.

The semantic model must support all of those cases without pretending they are manually freed.

## Mobility, relocation, and address stability

### Movability as default freedom

Unless semantic requirements forbid it, a memory object's physical location should not be part of its
identity.

A realization may relocate an object provided all live references, metadata, external contracts, and
observable semantics remain valid.

### Stable-address constraints

Stable address should be explicit and scoped.

```text
StableAddress<A, from = E4, until = E7>
```

Possible causes include:

- foreign code retains an address;
- DMA registration;
- device mapping;
- self-referential representation whose invariant requires fixation;
- ABI requirements;
- explicit address observation whose semantics demand stability.

### Pinning

Pinning is a state transition or capability use that introduces a bounded stability requirement.

```text
pin(A, until = Event42)
    -> FixedAddress<A, Event42>
```

The implementation may satisfy this by pinning a moving heap object, allocating from fixed storage,
registering pages, or selecting another realization.

When the constraint expires, relocation freedom may return if no other requirement forbids it.

## Address spaces and visibility

Memory may inhabit different target address spaces or visibility domains.

Examples include:

```text
host_virtual
host_physical
device_local
device_visible
shared_host_device
persistent
mmio
remote_registered
```

These names are illustrative and target profiles may define additional spaces.

Address-space identity is not automatically part of logical value identity, but it may constrain
valid operations and realizations.

Transfers between spaces may be semantic memory operations or RFC 0008 I/O operations depending on
observability and ownership.

## Locality and topology

Memory requirements may include:

```text
prefer_numa_node(1)
require_same_numa_domain(CPUSet)
prefer_cache_local(Task)
require_alignment(4096)
require_device_visibility(D)
prefer_huge_pages
```

Requirements must be distinguished from preferences.

A failed preference may reduce performance without changing semantics. A failed requirement rejects
the realization or selects a fallback.

## Persistence and crash-visible memory

Persistent memory or mapped durable storage introduces observability beyond ordinary process memory.

RFC 0009 reserves properties such as:

```text
volatile
persistent
flush_required
ordered_persistence
crash_consistent_after(E)
```

but leaves transactional and crash-recovery semantics to a dedicated failure/recovery design.

Backends must not treat ordinary volatile writes as durable merely because a target happens to retain
cache contents under some conditions.

## Foreign and externally owned memory

Foreign memory must cross an explicit trust/evidence boundary.

An import might produce:

```text
ForeignMemory<A> {
    extent = asserted N
    alignment = asserted 64
    lifetime = external_contract L
    mutability = external_contract M
    provenance = imported P
    deallocation = foreign_only
}
```

Assertions remain assumptions unless independently checked or proved.

A foreign address does not become a normal MNCS reference merely through a cast.

Foreign import should identify:

- owner;
- deallocator if any;
- lifetime contract;
- aliasing contract;
- initialization/validity contract;
- thread/device access contract;
- relocation/stability contract;
- provenance construction rule; and
- evidence/assumption boundary.

## Interaction with RFC 0008 I/O

RFC 0008 uses buffers, pinned/registered memory, DMA ownership, MMIO regions, and device-visible
resources. RFC 0009 provides their underlying memory semantics.

A DMA transition may look conceptually like:

```text
UniqueWrite<Buffer<B>>
    -> submit
InFlight<DeviceOwnsWrite<B, Request<R>>>
    -> completion
UniqueWrite<Buffer<B, InitializedRange<N>>>
```

The originating task does not retain conflicting write authority while the device owns the transfer.

MMIO memory is not ordinary RAM. Its semantic operations may require volatile/device ordering and
special provenance supplied by RFC 0008 device capabilities.

## Interaction with the future concurrency model

RFC 0009 intentionally defines ownership, permissions, footprints, provenance, and epochs before the
complete concurrency memory model.

The future concurrency RFC should define:

- atomicity;
- synchronization;
- data-race semantics;
- memory ordering;
- happens-before relations;
- fences;
- progress guarantees; and
- cross-thread permission transfer.

RFC 0009 supplies the semantic subjects those rules operate over.

No rule in RFC 0009 should imply that ordinary shared mutation is race-free merely because two
references are individually valid.

## Interaction with RFC 0007 proof-carrying semantics

Memory operations generate proof obligations where appropriate.

Examples:

```text
index(array, i)
    -> i < length(array)

split(buffer, a, b)
    -> footprints_disjoint(a, b)

retype(storage, T)
    -> representation_valid(T)

free(A)
    -> no_live_conflicting_authority(A)

relocate(A)
    -> no_active_stability_requirement(A)
```

Some obligations may be discharged by the dependent core, some by small certificate checkers, some
by soundness-scoped verifiers, and some only by runtime checks or external assumptions.

The method class must remain explicit.

## Memory evidence

Evidence objects should bind at least:

```text
subject_identity
allocation_identity
epoch
footprint
property
method
assumptions
dependencies
scope
target if relevant
verifier/checker identity
freshness
```

Examples include evidence for:

- disjointness;
- alignment;
- initialization;
- valid representation;
- no-alias conditions;
- region lifetime;
- shape properties;
- address stability;
- device visibility;
- bounds; and
- provenance derivation.

Changing the allocation epoch, region structure, split/join history, reference derivation, target
profile, representation, or dependency slice may invalidate only the evidence that depends on those
facts.

This narrow invalidation is a major reason to make memory structure explicit.

## Preservation through HIR and SSA

The memory model must not disappear after type checking.

HIR and verified SSA should preserve or reference stable identities for:

- memory objects;
- allocations;
- epochs;
- regions;
- references/provenance classes;
- footprints;
- permission transitions;
- initialization state;
- lifetime constraints;
- address-stability requirements;
- relevant obligations and evidence; and
- selected realization assumptions.

Backend alias metadata, in-bounds promises, noalias attributes, alignment assumptions, dereferenceable
claims, or non-null claims may be emitted only from current evidence and valid semantic rules.

An optimizer must not recreate C-like undefined behavior merely to gain performance.

## Memory SSA and effect relationships

RFC 0009 does not require one final memory-SSA representation, but later IR work should represent
memory state changes with enough precision to preserve resource and footprint relationships.

A useful future representation may associate memory versions with semantic footprints rather than one
global opaque memory token.

Independent updates to disjoint footprints should remain visibly independent where practical.

## Management strategy as a realization

MNCS should avoid encoding a universal memory-management mechanism into semantic source.

Candidate realization strategies include:

```text
stack
static
arena / bump allocation
slab / pool
manual allocator
reference counting
tracing garbage collection
generational GC
compacting GC
region reclamation
thread-local storage
shared-memory mapping
persistent mapping
device-local allocation
pinned pool
custom target allocator
```

A memory region may constrain which candidates are legal.

For example:

```text
Object A {
    size <= 48_KiB
    lifetime < E9
    movable = true
    address_observed = false
    sharing = none
    alignment >= 64
    prefer locality(Task7)
}
```

may admit stack, arena, thread-local pool, or moving-GC realization.

Another object:

```text
Object B {
    stable_address during DMA42
    device_visible(NIC0)
    alignment = 4096
    lifetime > DMA42
}
```

may require a registered/pinned pool or equivalent target mechanism.

The semantic object remains the same while realization identity changes.

## Storage selection as a machine search problem

Forge or another optimizer may search memory realizations under explicit objectives.

Conceptually:

```text
memory.choose {
    semantic_subject = Region<R>

    candidates = [stack, arena, slab, moving_gc, rc, pinned_pool]

    require {
        preserve_object_identity
        preserve_lifetime_constraints
        preserve_authority
        preserve_provenance
        peak_memory <= 64_MiB
    }

    prefer {
        minimize allocation_latency
        minimize peak_memory
        maximize locality
        minimize proof_cost
        minimize invalidation_radius
    }
}
```

Performance evidence may rank candidates but cannot replace semantic evidence.

A slightly slower memory strategy may be preferable if it drastically reduces proof cost, authority,
fragmentation risk, invalidation radius, or target assumptions.

## Layout and representation interaction

RFC 0006 already separates logical types from physical layout. RFC 0009 composes with that model.

Changing AoS to SoA, compacting fields, adding padding, changing alignment, or selecting a packed
representation changes storage realization and may affect reference paths, but it must not silently
change logical semantics.

References to logical fields should survive valid representation transformations through semantic
identity and lowering traceability rather than depending on fixed byte offsets until lowering makes
those offsets concrete.

## Unsafe memory operations

MNCS will need bounded unsafe or foreign memory operations, but `unsafe` must not mean "ordinary
semantics are suspended."

An unsafe operation should instead state:

```text
assumptions introduced
normal checks bypassed
required authority
obligations generated
scope of unsafety
properties no longer guaranteed
conservative fallback if any
```

The resulting artifact remains inspectable and proof/evidence-aware.

Unsafe code must not manufacture a proof object merely because the author asserted a condition.

## Address observation and pointer arithmetic

Address observation may be useful for hashing, device protocols, ABI calls, lock-free structures,
serialization of external handles, or low-level diagnostics.

MNCS should distinguish:

```text
address_of(ref)
offset_address(address, delta)
derive_reference(ref, offset)
foreign_address(value)
```

These operations do not have identical provenance semantics.

Arithmetic on an address does not automatically extend the authority of the source reference.

If an algorithm semantically depends on stable address identity, that dependency must be explicit and
should constrain relocation only for the required scope.

## Null, sentinel, and optional references

Nullability should not be an accidental property of every pointer-shaped value.

Conceptually:

```text
Ref<T>
Optional<Ref<T>>
Address
SentinelAddress<S>
```

are different types.

A target may realize `Optional<Ref<T>>` with a null niche when the representation permits it, but the
logical semantics should not require a null address unless declared.

## Zero-sized and abstract objects

The semantic model should support objects whose logical identity exists even if a target realization
requires no storage bytes.

This prevents address equality of zero-sized realizations from accidentally defining semantic object
identity.

## Recursive and self-referential structures

Self-reference should be modeled through semantic object/reference relationships rather than assumed
to require permanent physical pinning.

Some realizations may use handles, indirection tables, offsets, relocation fixups, or stable addresses.

A self-referential representation that truly requires address stability must state that requirement.

## Security implications

Making memory authority explicit can reduce several classes of vulnerability:

- use-after-free through epoch mismatch;
- out-of-bounds access through footprint/bounds obligations;
- double free through linear deallocation authority;
- unauthorized mutation through capability separation;
- stale DMA buffer access through resource-state transitions;
- accidental privilege amplification through reference attenuation rules;
- type confusion through explicit representation-validity obligations; and
- provenance laundering through address reconstruction rules.

This RFC does not claim to eliminate all memory vulnerabilities. Foreign code, unsafe boundaries,
hardware faults, verifier unsoundness, side channels, concurrency bugs, and specification mistakes
remain explicit trust boundaries.

## Machine-native advantages

The proposed model is intentionally richer than a normal human-facing pointer system because machines
can exploit the additional structure.

Potential benefits include:

### Allocation strategy synthesis

The compiler can choose storage based on lifetime, mobility, locality, and proof constraints rather
than source allocation spelling.

### Automatic disjointness exploitation

Explicit footprints permit parallelization, vectorization, DMA overlap, and alias-sensitive lowering
without repeatedly rediscovering separation.

### Narrow evidence invalidation

A local memory transformation can invalidate evidence only for the dependent footprint, epoch,
provenance chain, or realization rather than forcing whole-program revalidation.

### Relocation freedom

Objects remain movable until semantics explicitly require stability.

### Proof-aware placement

A machine may prefer a storage strategy whose lifetime or ownership proof is simpler even when its
raw allocation cost is slightly higher.

### Explicit uncertainty

The compiler can retain `UNKNOWN` provenance or alias information and safely choose a conservative
realization instead of pretending an unproven fact is true.

### Hardware specialization

Capability hardware, MPU protection, tagged memory, bounds-check instructions, or device-specific
memory features may realize language-level capabilities without changing source semantics.

## Initial experimental tracks

RFC 0009 should begin with deliberately bounded experiments.

### Track 1 — Semantic memory identity and epochs

Implement:

- `ObjectId`;
- `AllocationId`;
- `LifetimeEpoch`;
- storage state;
- explicit allocation/deallocation transitions; and
- rejection of stale references after epoch change.

Success criterion:

A deterministic interpreter can distinguish temporal allocation identity even when physical address
values are intentionally reused.

### Track 2 — Provenance graph and conservative joins

Implement a small provenance graph with:

```text
Exact
FiniteSet
SubjectBound
Unknown
Invalid
```

or an equivalent minimal lattice.

Success criterion:

Control-flow joins preserve conservative provenance and refuse optimizations that require stronger
exact provenance.

### Track 3 — Permission split/join and disjoint footprints

Implement one buffer/range model that can:

- establish bounds;
- split a writable resource into disjoint ranges;
- use the ranges independently;
- rejoin them; and
- reject overlap or incompatible provenance.

Success criterion:

The verifier can justify independent operations on split ranges without whole-buffer alias analysis.

### Track 4 — Initialization and retyping

Implement:

- allocated/uninitialized storage;
- initialized-range tracking;
- a small valid-representation predicate; and
- checked transition from bytes/storage to one typed value.

Success criterion:

Reading uninitialized or representation-invalid storage is rejected while incremental construction is
accepted through explicit states.

### Track 5 — Relocating reference interpreter

Build an intentionally adversarial reference interpreter that periodically changes abstract physical
addresses for every object not under a stability constraint.

Success criterion:

Programs whose semantics do not declare address dependence continue to produce identical observable
results. Programs that accidentally rely on physical address identity fail the conformance study.

This is a particularly important MNCS experiment because it tests whether identity and location are
actually separated rather than merely described as separate in the specification.

### Track 6 — Competing storage realizations

Run the same semantic memory graph under at least three realization strategies, for example:

```text
stack/lexical
arena/region
managed/moving or handle-indirected
```

The exact third strategy may change with implementation practicality.

Success criterion:

The observable semantics and required memory claims remain equivalent while realization identities,
addresses, allocation counts, and performance measurements differ.

### Track 7 — I/O pinning bridge

Connect RFC 0008 to RFC 0009 with one registered/pinned-buffer state transition.

Success criterion:

A buffer may be movable before submission, becomes address-stable and unavailable for conflicting
mutation while in flight, and regains the appropriate authority after completion.

## Graduation criteria

A memory feature should advance from experiment toward accepted language semantics only if it
measurably improves at least one of:

- machine-generation correctness;
- verifier precision;
- optimization freedom;
- memory safety;
- evidence locality/invalidation behavior;
- target portability;
- storage efficiency;
- proof cost;
- runtime cost; or
- compositional reasoning;

without imposing disproportionate complexity on the semantic core or proof kernel.

No feature should graduate merely because another language uses it.

## Required negative tests

The prototype should include intentionally rejected cases for:

- use after deallocation/epoch change;
- address reuse with stale provenance;
- overlapping exclusive range splits;
- mutation through read-only authority;
- deallocation without deallocation authority;
- deallocation while an in-flight device capability exists;
- retyping uninitialized bytes;
- retyping invalid representation;
- forging a reference from a numeric address without provenance;
- relocating while a stability requirement is active;
- joining permissions from incompatible subjects or epochs; and
- treating `UNKNOWN` provenance as exact.

## Open research questions

The following remain intentionally unresolved.

### Permission algebra form

Should permissions use a small symbolic lattice, semiring-like quantitative grades, fractional
permissions, or multiple specialized algebras?

### Provenance graph granularity

How much derivation history should survive in canonical semantics versus verifier-owned evidence?

### Region inference

Should regions be mostly explicit semantic objects generated by machines, inferred automatically, or
both?

### Garbage-collected references

How should moving-GC roots and weak references map into provenance and epoch semantics without
inflating the trusted core?

### Cycles and deallocation authority

How should explicit/managed deallocation interact with cycles, finalization, and region ownership?

### Interior references

What is the smallest robust rule for references to subobjects across relocation and representation
changes?

### Pointer equality

Should reference identity equality, object identity equality, and current-address equality be three
separate operations? The initial direction is yes.

### Integer-address interaction

How should bare-metal and MMIO environments construct initial provenance without pretending that a
numeric address is ordinary allocation provenance?

### Memory-mapped persistence

Which persistence and crash-consistency facts belong in memory semantics versus the future failure
and transactional RFC?

### Proof boundary

Which permission/provenance rules belong in the small proof kernel, and which should be validated by
separate small deterministic checkers?

## Alternatives considered

### Adopt Rust ownership and borrowing directly

Rejected as the semantic foundation.

Rust demonstrates the value of ownership and alias discipline, but its borrow syntax and current
provenance work solve constraints arising from a human-oriented systems language with unsafe raw
pointers and compatibility requirements. MNCS can represent provenance, authority, and disjointness
more directly in its semantic graph and later elaborate convenient borrow forms if useful.

### Use tracing garbage collection universally

Rejected.

GC may be an excellent realization for many regions but does not itself express device ownership,
explicit address stability, deallocation authority, MMIO, bounded-memory requirements, or
optimizer-grade alias facts.

### Require manual allocation and deallocation

Rejected.

Explicit allocation is useful at some boundaries but unnecessarily commits semantic source to a
management strategy and increases proof burden when lifetime information could support a managed or
region realization.

### Make regions the universal allocator

Rejected.

Regions are valuable semantic lifetime domains, but equating a region with one arena discipline would
throw away useful realization freedom.

### Treat raw addresses as universal pointers

Rejected.

Numeric location alone does not encode allocation identity, epoch, bounds, provenance, authority,
initialization, or validity.

### Hide all memory semantics behind compiler inference

Rejected.

Inference remains useful, but machine-authored code should be able to state and preserve facts the
generator already knows. Critical authority and correctness should not depend on reverse-engineering
source patterns.

## Research traditions informing this RFC

The following traditions are research inputs rather than normative dependencies:

- Reynolds/O'Hearn-style separation logic and local reasoning;
- capability-based typed memory management by Crary, Walker, Morrisett, and related work;
- Tofte–Talpin region-based memory management and later region inference work;
- linear, affine, uniqueness, fractional-permission, and quantitative type systems;
- Oxide and RustBelt-family formalizations of ownership, borrowing, and aliasing;
- Stacked Borrows and Tree Borrows as operational provenance/alias models;
- abstract interpretation and three-valued heap/shape analyses;
- CHERI capability hardware and software models;
- region-isolation/reference-capability systems such as Verona-related research; and
- typed low-level/assembly-language work that preserves memory safety through compilation.

MNCS should extract the compositional principles from these traditions rather than reproduce their
surface languages.

## Relationship to the MNCS design philosophy

RFC 0009 extends the principle established by RFC 0006 and RFC 0008:

> **The semantic language states what must be true and exposes what the machine is free to choose.**

For memory, the language states:

```text
identity
provenance
authority
lifetime
disjointness
validity
initialization
stability
visibility
locality
persistence
resource bounds
```

and exposes implementation freedom over:

```text
stack vs heap
arena vs GC
RC vs tracing
moving vs fixed
slab vs general allocator
NUMA placement
address-space realization
hardware vs software bounds
pinning mechanism
representation and layout where permitted
```

That separation should make MNCS memory simultaneously safer, more optimizable, more portable, and
more suitable for autonomous generation than a design centered on one human-facing pointer model.

## Proposed initial implementation order

1. Add object/allocation/epoch identities to the executable semantic model.
2. Add bounded memory footprints and allocation state.
3. Add a minimal permission algebra with read, exclusive write, split, join, and deallocate.
4. Add provenance derivation and a conservative join lattice.
5. Add initialization state and one checked retyping operation.
6. Preserve the identities and transitions through HIR and SSA.
7. Add the relocating reference interpreter.
8. Add two or three competing memory realizations over the same semantic fixture.
9. Bridge one RFC 0008 pinned/DMA buffer experiment.
10. Add verifier/evidence artifacts for disjointness, bounds, initialization, and provenance.

Each step should ship accepted and intentionally rejected fixtures before stronger claims are made.

## Conclusion

MNCS should not define memory by choosing a favorite allocator, garbage collector, borrow checker, or
pointer representation.

It should define memory as a semantic graph of **identity, storage, provenance, authority, lifetime,
validity, disjointness, and resource relationships**, then allow verified target-specific mechanisms
to realize that graph.

The most promising foundation is therefore not one existing theory but a deliberate synthesis:

```text
separation/resource semantics
    + capability authority
    + linear/quantitative permissions
    + explicit provenance and epochs
    + semantic regions
    + conservative abstract interpretation
    + typed initialization/representation
    + evidence-preserving lowering
```

That combination gives autonomous machines the facts needed to reason about memory directly while
preserving implementation freedom for stack allocation, arenas, managed heaps, capability hardware,
device memory, and future mechanisms that do not yet exist.
