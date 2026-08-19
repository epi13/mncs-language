# RFC 0010: Machine-Native Concurrency, Causality, Atomicity, and Memory Consistency Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.5–1.0
- **Depends on:** RFC 0002, RFC 0003, RFC 0006, RFC 0007, RFC 0008, RFC 0009

## Summary

MNCS Language should define concurrency in terms of **events, causality, interference, resource
conflicts, authority transfer, visibility, atomicity, observation provenance, progress, and explicit
assumptions** rather than defining it primarily through threads, locks, atomics, fences, channels, or
one fixed scheduler.

The central rule is:

> **MNCS concurrency describes the semantic partial order and interference constraints of work;
> threads, locks, atomics, fences, channels, transactions, schedulers, and target memory-order
> instructions are verified realizations.**

A machine-authored program should expose as much true independence as possible. Two operations may be
executed concurrently when the semantic graph establishes that no data dependency, conflicting
memory footprint, authority conflict, protocol constraint, visibility requirement, atomicity
contract, effect conflict, or explicit ordering relation requires serialization.

Conversely, absence of known conflict is not proof of independence. Unknown aliasing, unknown
visibility, unknown interference, or unresolved synchronization obligations must reduce optimization
authority, trigger runtime checks, force stronger synchronization, select a conservative realization,
or reject promotion according to the active assurance policy.

This RFC synthesizes the most promising complementary ideas from:

- **partial orders and event structures** for causality and conflict;
- **RFC 0009 separation/resource semantics** for positive disjointness and resource-local reasoning;
- **rely/guarantee reasoning** for explicit environmental interference contracts;
- **concurrent separation logic** as an external verification and evidence framework;
- **weak-memory event models** for visibility semantics that are not defined by accidental source
  order;
- **linearizability and logical atomicity** for simple abstract specifications of fine-grained
  concurrent implementations;
- **structured concurrency** for explicit lifetimes and accountability of concurrent activities;
- **DAG/work-span theory** for exposing parallelism while leaving scheduler topology as a realization;
- **algebraic reductions and commutativity** for safe reordering beyond physically disjoint memory;
- **progress theory** for blocking, deadlock-freedom, lock-freedom, wait-freedom, and bounded progress;
  and
- **session/resource-transfer semantics** for treating message passing and shared-memory ownership
  transfer within one semantic framework.

None of these traditions is adopted wholesale. The goal is a small, compositional concurrency
ontology that exposes machine-relevant facts to autonomous generators, schedulers, verifiers,
optimizers, runtimes, and target backends.

## Motivation

Conventional languages often ask the author to choose implementation mechanisms before the compiler
knows the full semantic problem:

```text
spawn a thread
protect this object with a mutex
use an atomic here
insert an acquire load there
send this value through a channel
use a transaction around these statements
```

Those mechanisms can be correct, but they conflate several questions:

- Which operations actually depend on one another?
- Which memory footprints overlap?
- Which overlapping updates commute?
- Which authority is transferred or shared?
- Which values must become visible before another event may proceed?
- Which concurrent observations are legal?
- Which operation must appear atomic at the abstract level?
- Which progress property is required?
- Which fairness assumptions are required to claim that progress?
- Which task lifetime owns spawned work?
- Which scheduling, synchronization, and memory-order mechanisms are merely implementation choices?

A machine-native language can preserve these distinctions directly.

For example, the semantic artifact may state:

```text
A must complete before B because B consumes A.output
C and D write disjoint RFC-0009 footprints
E and F update the same abstract counter but commute
G publishes object X
H may consume X only after observing G's publication
I must be linearizable against queue_reference_spec
J may block
K must be lock_free under declared scheduler/hardware assumptions
L belongs to task scope S and cannot outlive S without explicit ownership transfer
```

A realization may then choose among:

```text
inline execution
OS threads
fibers
coroutines
work stealing
GPU workgroups
remote workers
mutexes
reader/writer locks
spin locks
atomic instructions
LL/SC
CAS loops
fences
channels
ownership transfer
message queues
software transactions
hardware transactions
scheduler serialization
single-writer placement
```

provided it preserves the declared semantics and carries the required evidence.

This continues the architectural pattern established by RFC 0006, RFC 0008, and RFC 0009:
semantic requirements are primary; target mechanisms are realizations.

## Design goals

RFC 0010 should make the following possible.

1. **Represent causality explicitly.** Required ordering should be a semantic relation rather than a
   side effect of source sequence or thread placement.
2. **Preserve independence.** Independent operations should remain unordered until a true relation
   requires order.
3. **Exploit RFC 0009 footprints.** Proven disjointness and permission separation should authorize
   safe parallelism without re-inferring alias facts from pointer syntax.
4. **Represent interference contracts.** Components should state or derive what concurrent
   interference they rely upon and what interference they guarantee.
5. **Separate visibility from mechanism.** A program should express which writes an observer must be
   able to see; acquire/release/fences are lowerings, not the primary ontology.
6. **Track observation provenance.** Shared-memory reads should have a legal semantic origin and
   visibility path under the selected model.
7. **Forbid unsupported thin-air behavior.** Observed values must derive from a legitimate semantic
   source rather than circular speculation without origin.
8. **Abstract atomic operations.** Linearizable or logically atomic state transitions should be
   expressible independently of locks, CAS loops, transactions, or scheduler exclusion.
9. **Model concurrent lifetimes.** Spawned activity should normally belong to an explicit structured
   lifetime scope.
10. **Represent progress honestly.** Blocking, deadlock-freedom, lock-freedom, wait-freedom, bounded
    waiting, and fairness assumptions must remain distinct.
11. **Expose schedulable structure.** Work/span, dependencies, resource needs, and preferences should
    be available to Forge and runtime schedulers.
12. **Support deterministic parallelism.** Determinism and schedule-independence should be explicit
    properties rather than assumed consequences of parallel execution.
13. **Exploit algebraic freedom.** Associativity, commutativity, idempotence, and reduction structure
    should authorize safe reordering when proved or otherwise evidenced.
14. **Preserve target portability.** The same concurrency semantics should admit verified lowerings to
    different hardware memory models and runtime strategies.
15. **Produce actionable diagnostics.** Race/conflict reports should identify overlapping resources,
    missing relations, illegal observations, and candidate repair classes.
16. **Keep proof boundaries explicit.** Concurrency verification may use external logics, model
    checkers, solvers, or empirical stress tools without silently enlarging the RFC 0007 kernel.

## Non-goals

This RFC does not:

- select one universal scheduler;
- require OS threads, green threads, actors, channels, locks, STM, or lock-free structures;
- make sequential consistency the only semantic model;
- expose one target's acquire/release/fence vocabulary as the universal source language;
- claim that all concurrency properties are statically decidable;
- make fairness an implicit environmental fact;
- equate memory safety with race freedom or race freedom with full concurrent correctness;
- define distributed-system consistency, consensus, partitions, replication, or failure detectors in
  full; those require later work;
- place arbitrary hardware-memory behavior inside the proof kernel;
- require every program to maximize parallelism;
- promise automatic lock-free or wait-free implementation synthesis; or
- treat benchmark success or stress testing as formal proof.

Unsupported properties may remain `UNKNOWN`, force conservative scheduling or synchronization,
require a runtime guard, or reject a realization under the active assurance profile.

## Selected theoretical synthesis

### Partial orders and event structures

The foundational representation should be relational rather than thread-centric.

A concurrency graph contains events and relations such as:

```text
A -> B
A -> C
B || C
B,C -> D
```

where `->` means a required semantic dependency and `||` means independence has been established for
the relevant relation set.

The canonical graph should preserve only order that is semantically required. Source order may be a
presentation choice and must not automatically become semantic order unless the language construct
or active policy says so.

Event structures additionally contribute explicit conflict. Two events may both be locally enabled
but mutually exclusive because one consumes a resource, commits a choice, or transitions protocol
state in a way that disables the other.

Conceptually:

```text
EventRelation {
    happens_before
    conflicts_with
    mutually_exclusive
    independent_of
}
```

The final calculus may derive some relations rather than store all of them redundantly.

### Separation and resource semantics from RFC 0009

RFC 0009 makes memory footprints, provenance, epochs, permissions, and positive disjointness semantic
facts. RFC 0010 should consume those facts directly.

If two events hold compatible authority over disjoint footprints:

```text
Write<A[0..512)>
*
Write<A[512..1024)>
```

then their memory interaction does not require serialization merely because both derive from the
same larger object.

Likewise, two immutable observations may coexist even when their footprints overlap, provided the
permission algebra and visibility semantics allow it.

This turns alias/disjointness evidence into concurrency authority.

### Rely/guarantee interference contracts

Concurrent components need bounded assumptions about their environment.

A component may declare or derive:

```text
rely {
    configuration remains immutable
    queue remains structurally valid
    counter never decreases
}

guarantee {
    may append(queue)
    never mutate(configuration)
    counter increases by at most 1 per operation
}
```

Rely conditions define interference the component assumes. Guarantee conditions define interference
it may cause.

A composition is valid only when neighboring guarantees satisfy the relies they can affect.

Capabilities, effects, resource footprints, and protocol states should automatically derive as much
of the guarantee as possible. An authoring machine should not manually restate facts already present
in the semantic graph.

### Concurrent separation logic as verification infrastructure

Concurrent separation logics provide strong local reasoning about shared mutable state and
fine-grained concurrency.

MNCS should not place a large concurrent separation logic inside the trusted proof kernel. Instead,
RFC 0010 should define stable obligations that external systems may discharge:

```text
semantic concurrency obligation
        -> external prover / model checker / CSL engine
        -> proof term, certificate, verifier result, or UNKNOWN
        -> MNCS evidence object
```

The method class and assumptions remain explicit under RFC 0002 and RFC 0007.

### Weak-memory event semantics

Concurrency cannot be defined only as sequential execution plus informal reordering. The language
needs explicit relations governing reads, writes, synchronization, and visibility.

The source semantic model should state requirements such as:

```text
write(object) visible_before read(object)
publication P synchronizes consumption C
read R may observe writes satisfying relation M
```

A target realization may use acquire/release instructions, fences, locks, message passing, stronger
atomics, scheduler ordering, or another mechanism.

The semantic artifact should not require an authoring machine to choose a target-memory-order
instruction when what it actually knows is an abstract visibility requirement.

### Linearizability and logical atomicity

Fine-grained implementations should be specifiable against simple abstract state transitions.

Conceptually:

```text
operation queue.push(x) {
    abstract_transition:
        Q -> append(Q, x)

    require:
        linearizable_against(queue_reference_spec)
}
```

or:

```text
require logically_atomic
```

A realization may use a lock, CAS loop, LL/SC, ownership transfer, scheduler exclusion, transaction,
or another mechanism while callers reason against the abstract operation.

Logical atomicity should be investigated because it can make fine-grained implementations compose
more naturally with resource reasoning than treating linearizability only as an external trace
property.

### Structured concurrency

Concurrent activities should normally live inside explicit semantic scopes.

```text
scope S {
    child A
    child B
    child C
}
```

Leaving `S` requires each child to be:

```text
completed
cancelled with defined resource state
or explicitly transferred to another owning scope
```

A task is a semantic activity/lifetime object, not necessarily a thread.

Possible realizations include:

```text
inline call
OS thread
coroutine
fiber
work item
GPU lane/workgroup
remote worker
```

### DAG and work/span semantics

The concurrency graph naturally forms a DAG for acyclic bounded regions, or a recurring event
structure for loops and services.

The language/runtime should be able to record or estimate:

```text
work
span
parallel slack
resource pressure
synchronization cost
```

These quantities are optimization evidence and cost-model inputs, not semantic correctness claims
unless explicitly bounded by contract.

Programs should normally expose available parallelism rather than specify a fixed worker count.

### Algebraic reductions and commutativity

Disjointness is not the only source of safe concurrency.

Two operations may overlap physically but commute semantically.

MNCS should be able to express or prove properties such as:

```text
associative(op)
commutative(op)
idempotent(op)
identity(op, e)
```

A reduction may declare:

```text
reduce sum {
    identity = 0
    combine = add
    properties = [associative]
}
```

If commutativity is also established, additional ordering freedom exists.

For floating point, RFC 0006's explicit numerical semantics and relaxation/error envelopes govern
whether reassociation or reordering is legal.

### Progress theory

Safety does not establish progress.

MNCS should distinguish at least research-level properties such as:

```text
may_block
blocking
deadlock_free
lock_free
wait_free
obstruction_free
bounded_wait(N)
```

A progress guarantee must identify its assumptions:

```text
progress {
    guarantee = lock_free

    assumes {
        ready_tasks_receive_execution
        atomic_primitive_forward_progress
    }
}
```

An implementation must not claim wait-freedom or lock-freedom merely because tests completed.

### Session and resource-transfer semantics

Message passing and shared-memory ownership transfer should not be separate semantic universes.

An operation may express:

```text
transfer Write<A>
from Task1
to Task2
```

A realization may use:

```text
same-address-space ownership transfer
channel
shared queue
IPC
RDMA
remote message
```

RFC 0008 protocol/session semantics should compose with RFC 0010 task/event relations so protocol
position, resource ownership, and concurrency remain consistent.

## Core semantic model

A conceptual concurrency graph may contain:

```text
ConcurrencyGraph {
    events: [ConcurrentEvent],
    task_scopes: [TaskScope],

    dependency_relations: [DependencyRelation],
    conflict_relations: [ConflictRelation],
    independence_facts: [IndependenceFact],

    memory_footprints: [MemoryFootprintRef],
    authority_flows: [AuthorityTransfer],

    visibility_relations: [VisibilityRelation],
    observation_origins: [ObservationOrigin],

    atomicity_contracts: [AtomicityContract],
    isolation_contracts: [IsolationContract],
    interference_contracts: [InterferenceContract],

    determinism_properties: [DeterminismProperty],
    progress_properties: [ProgressProperty],
    fairness_assumptions: [FairnessAssumption],

    requirements: PropertySet,
    preferences: PreferenceSet,

    obligations: [ObligationRef],
    evidence_policy: EvidencePolicy,
    lowering_envelope: LoweringEnvelope,
    portability_envelope: PortabilityEnvelope,
}
```

A concurrent event may be represented conceptually as:

```text
ConcurrentEvent {
    identity,
    operation,

    inputs,
    outputs,

    read_footprints,
    write_footprints,

    consumed_authority,
    produced_authority,

    dependencies,
    conflicts,

    publishes,
    observes,

    task_scope,

    atomicity,
    isolation,
    determinism,
    progress,

    effects,
    obligations,
    evidence,
}
```

These shapes are semantic sketches, not final wire formats or surface syntax.

## Dependency taxonomy

The language should not collapse all order into one anonymous edge when stronger classification helps
verification or lowering.

Initial dependency classes should include at least:

```text
data_before
control_before
resource_before
authority_before
protocol_before
visibility_before
atomic_before
lifetime_before
explicit_before
```

A general `happens_before` relation may be derived from the subset that carries that meaning under the
selected model.

Different relation classes may invalidate different evidence when changed.

For example, changing a scheduling preference should not invalidate semantic data-dependency proof;
changing an RFC 0009 footprint may invalidate independence evidence.

## Independence

Independence should be a positive semantic fact, not merely absence of a known dependency.

Conceptually:

```text
Independent(X, Y)
```

may be established through one or more reasons:

```text
disjoint_footprint
read_read_compatibility
commutative_updates
isolated_views
ownership_partition
protocol_independence
reduction_structure
explicit proof
runtime-checked separation
```

The independence evidence must bind its exact subjects, footprints, epochs, permissions, effects,
and assumptions.

If any consumed fact becomes stale, parallelization authority depending on it becomes stale.

## Conflict

A conflict is a reason two otherwise enabled events cannot legally proceed without additional
coordination.

Conflict classes may include:

```text
overlapping conflicting memory access
exclusive authority consumption
protocol alternative selection
resource capacity exhaustion
atomicity overlap
transaction isolation conflict
lifetime closure
explicit mutual exclusion
```

Conflicts may be resolvable through serialization, stronger synchronization, ownership transfer,
transaction retry, or another legal realization.

## Data races

MNCS should define data races semantically rather than importing undefined behavior as the primary
model.

A conceptual race condition may require:

```text
potentially_concurrent(X, Y)
&& footprints_overlap(X, Y)
&& accesses_conflict(X, Y)
&& no_legal_synchronization_relation(X, Y)
&& no_commutativity_or_isolation_evidence(X, Y)
```

Safe MNCS policy should reject unresolved conflicting races or choose a realization that prevents
them.

Unsafe/foreign boundaries may expose weaker guarantees, but the weakening must be explicit and must
not silently authorize unrelated optimizations.

A race diagnostic should report:

```text
event X
event Y
overlapping footprint
authority held by each event
missing relation or conflicting proof
possible repair classes
```

## Visibility and publication

Programs often care that one event's writes become visible before another event observes them.

The semantic layer should express the relationship directly.

Conceptually:

```text
publication P {
    publishes = object X
}

consumption C {
    requires_visible = X from P
}
```

or canonically:

```text
Visibility(P.write(X), C.read(X))
```

A target backend may realize that relationship with:

```text
release/acquire
stronger atomic ordering
fence sequence
lock handoff
message send/receive
ownership transfer
scheduler serialization
```

The backend must justify that the chosen target mechanism establishes the semantic relation under the
bound target memory model.

## Observation provenance

Every shared-state observation should have a legal source under the concurrency semantics.

Conceptually:

```text
Observation {
    read_event,
    location,
    observed_value,
    source_event,
    source_epoch,
    visibility_path,
    model_identity,
    assumptions,
}
```

This object may exist only in verification/debug artifacts rather than runtime state.

The important language principle is:

> **Every observed value must derive from an initialization, write, declared input,
> nondeterministic source, or explicitly modeled transformation that is legal under the selected
> visibility semantics.**

An address/provenance relation from RFC 0009 is not sufficient by itself to establish that a
particular write may be observed concurrently.

Observation provenance should support diagnostics such as:

```text
Read E82 observed Write E41.
Expected publication relation E55 -> E82 is absent.
The selected weak-memory realization therefore cannot justify the observation.
```

## Thin-air prohibition

The semantic model must not permit values to become valid merely because a cyclic speculative
execution could justify itself.

The initial research rule should require a grounded observation-origin graph.

A value origin may be:

```text
initialization
prior write
external input
explicit nondeterministic source
pure transformation of grounded values
```

Cyclic justification without grounded origin is invalid.

The exact formal treatment is research work and must be tested against established weak-memory
litmus cases before acceptance.

## Memory consistency and target lowering

MNCS should not expose one target's memory model as universal source semantics.

The compiler pipeline should investigate an explicit concurrency/memory-consistency lowering stage:

```text
MNCS semantic event graph
        ↓
concurrency / visibility IR
        ↓
verified memory-model lowering
        ↓
x86 / ARM / RISC-V / GPU / WASM / other target
```

This layer should retain:

```text
event identity
read/write footprints
visibility requirements
atomicity requirements
observation constraints
synchronization obligations
target model identity
selected ordering realization
```

Backend emission of relaxed atomics, fences, acquire/release instructions, or target-specific
metadata must be justified by current evidence and the target profile.

## Sequential consistency

Sequential consistency should be available as an explicit requirement or conservative fallback.

```text
require sequential_consistency(scope = S)
```

However, the canonical concurrency ontology should not assume all executions are a single total order
and then treat weaker behavior as an implementation loophole.

A relational/event-based model is the safer foundation for later target lowering.

## Atomicity

Semantic atomicity must be distinct from one hardware atomic instruction.

An abstract operation may declare:

```text
atomic_transition {
    state: Q -> append(Q, x)
}
```

Possible realizations include:

```text
mutex
reader/writer lock
spin lock
CAS loop
LL/SC
hardware transaction
software transaction
single-owner task
scheduler exclusion
message-passing owner
```

The realization may have many physical steps while presenting one legal abstract transition to
clients.

## Linearizability

An operation may require:

```text
linearizable_against(reference_spec)
```

The evidence must bind:

```text
implementation identity
reference spec identity
operation set
concurrency assumptions
memory model
proof/verifier identity
```

A change to any of those inputs may invalidate the evidence.

## Logical atomicity

MNCS should investigate logically atomic specifications as a compositional interface for
fine-grained concurrent operations.

A caller should be able to reason against an abstract resource transition even when the implementation
uses helping, retries, multiple atomic steps, or other fine-grained techniques.

Logical atomicity is not required to be a proof-kernel primitive. It may be established through an
external concurrent-logic verifier and imported as bounded evidence.

## Synchronization as derived realization

Locks, atomics, fences, and channels should normally be generated from semantic requirements rather
than become the only way to state those requirements.

For example:

```text
exclusive Write<A> during E1..E2
```

may lower to:

```text
no synchronization if static exclusivity is proved
mutex
rwlock
spinlock
ownership transfer
transaction
serialized scheduler placement
```

Likewise:

```text
Visibility(P, C)
```

may lower to the weakest verified ordering mechanism sufficient on the selected target.

A stronger mechanism is allowed when policy permits, but unnecessary synchronization should remain
visible as a measurable cost rather than hidden semantic necessity.

## Structured task scopes

Task scopes provide ownership over concurrent activity.

Conceptually:

```text
TaskScope<S> {
    parent,
    children,
    cancellation_policy,
    failure_policy,
    owned_resources,
    transferred_resources,
    exit_obligations,
}
```

A task may not silently outlive the scope that owns its resources or authority.

Explicit detach/transfer should require a receiving scope and authority transfer, not create an
unowned background activity by default.

This rule should compose with RFC 0008 in-flight I/O and RFC 0009 memory lifetimes.

## Cancellation and concurrency

Cancellation is a state transition, not merely a scheduler signal.

A cancelled task may hold:

```text
memory permissions
locks or exclusion authority
in-flight I/O resources
protocol endpoints
transactional state
completion obligations
```

Cancellation semantics must state what happens to each resource class.

A task scope cannot be considered safely closed merely because execution stopped.

Detailed failure/recovery semantics will be expanded in a later RFC, but RFC 0010 must preserve the
resource and event transitions necessary to model cancellation correctly.

## Determinism

Concurrency properties should distinguish at least:

```text
Deterministic
ScheduleIndependent
DeterministicGiven<ObservationSet>
Nondeterministic<DeclaredSources>
Unspecified
```

A computation may execute under nondeterministic scheduling while remaining schedule-independent.

A parallel map over proven disjoint elements is an example candidate.

A reduction may be schedule-independent only when the declared algebra and numerical semantics justify
reordering.

Determinism evidence must identify external observations, randomness, clocks, I/O, and other
nondeterministic sources excluded from or included in the claim.

## Commutativity

Semantic commutativity can authorize concurrency even when physical footprints overlap.

If operations `A` and `B` satisfy:

```text
A(B(S)) == B(A(S))
```

under the relevant invariant and failure semantics, then ordering may be relaxed where the rest of
the contract allows it.

Commutativity evidence should identify:

```text
operation identities
state domain
invariants
failure semantics
side effects
authority changes
assumptions
```

A commutativity claim about pure abstract state must not silently ignore externally observable effects.

## Reductions

A reduction is a particularly useful machine-native concurrency object.

Conceptually:

```text
Reduction {
    element_type,
    accumulator_type,
    identity,
    combine,
    algebraic_properties,
    numerical_policy,
    determinism_policy,
    resource_budget,
}
```

The compiler may choose tree shape, worker placement, vectorization, batching, or sequential fallback
within the declared algebraic and floating-point semantics.

## Progress properties

Progress claims should be separate from safety and performance.

Initial research vocabulary:

```text
may_block
blocking
deadlock_free
lock_free
wait_free
obstruction_free
bounded_wait(N)
```

Not every target or runtime can establish every property.

An unsupported progress proof remains `UNKNOWN`; a benchmark cannot automatically upgrade it to a
formal claim.

## Fairness assumptions

Fairness must be explicit whenever progress depends upon it.

Potential assumption classes include:

```text
weak_scheduler_fairness
strong_scheduler_fairness
FIFO_queue_service
eventual_device_completion
no_fairness_guarantee
```

The exact vocabulary is future work.

A progress theorem must record the fairness assumptions it consumed.

## Deadlock and wait-for structure

The semantic graph should support derived wait-for/resource graphs.

Potential deadlock evidence should identify cycles among:

```text
tasks
resources
capabilities
protocol states
completion events
transaction dependencies
```

A static deadlock-free proof is desirable but not required for ordinary compilation.

Runtime deadlock detection, model checking, bounded exploration, or conservative `UNKNOWN` may be
used according to policy.

## Message passing and shared memory

MNCS should not create two incompatible concurrency ontologies for shared memory and message passing.

Both can be represented through:

```text
events
authority transfer
resource ownership
protocol transition
visibility/observation relations
```

A channel send may transfer a capability that previously authorized direct shared-memory access.

A remote transport may realize the same semantic transfer with serialization and network effects.

The portability envelope must record where those realizations remain equivalent and what failure
semantics differ.

## Capacity and backpressure

Concurrency consumes resources.

Queues, channels, worker slots, device submissions, memory buffers, and transaction logs may be
bounded.

RFC 0008 graded resources should compose with concurrency:

```text
Credits<Queue<Q>, N>
WorkerSlots<P, N>
OutstandingEvents<S, N>
```

A scheduler or optimizer must not assume infinite buffering.

Backpressure should be representable as a semantic resource relationship rather than accidental
blocking behavior.

## Realization model

A concurrency realization should be a separate artifact from the semantic concurrency graph.

Conceptually:

```text
ConcurrencyRealization {
    semantic_graph_identity,
    target_profile,
    runtime_profile,

    scheduler_strategy,
    worker_topology,
    task_mapping,

    synchronization_strategy,
    atomic_realizations,
    fence_realizations,
    channel_realizations,
    transaction_realizations,

    target_memory_model,
    ordering_instructions,

    discharged_obligations,
    remaining_assumptions,
    measurements,
    evidence,
}
```

Changing worker count alone should not change semantic identity when worker count was merely a
preference. Changing an ordering requirement or atomicity contract does change semantic meaning.

## Requirements, facts, preferences, and realizations

RFC 0010 must retain RFC 0006's distinction.

Examples:

```text
require Visibility(P, C)
fact Disjoint(A, B) established_by proof_42
prefer worker_count = hardware_parallelism
prefer avoid_blocking = true
```

A preference cannot justify a semantic transformation.

A fact can be consumed only while its evidence remains current.

A realization records how requirements were implemented.

## Conservative UNKNOWN

Concurrency is especially vulnerable to optimistic inference.

Examples:

```text
alias(A, B) = UNKNOWN
commute(X, Y) = UNKNOWN
visibility(W, R) = UNKNOWN
progress(queue) = UNKNOWN
```

must not be treated as:

```text
no_alias
commutative
visible
lock_free
```

Instead, `UNKNOWN` may:

- force serialization;
- choose stronger memory ordering;
- require a runtime separation check;
- select a blocking fallback;
- prevent an optimization;
- prevent an assurance claim; or
- reject promotion.

This rule is mandatory.

## Obligation generation

Concurrency constructs should automatically generate obligations.

Examples:

### Parallelization obligation

```text
parallelize(X, Y)
  -> prove dependency compatibility
  -> prove footprint compatibility or legal overlap
  -> prove authority compatibility
  -> prove effect/protocol compatibility
```

### Publication obligation

```text
publish(P, X)
consume(C, X)
  -> establish visibility relation
  -> establish target lowering sufficiency
```

### Atomicity obligation

```text
realize_atomic(op)
  -> preserve abstract transition
  -> establish linearizability/logical atomicity as required
```

### Progress obligation

```text
claim lock_free(op)
  -> identify scheduler assumptions
  -> identify hardware primitive assumptions
  -> provide accepted proof/evidence method
```

### Observation obligation

```text
read R observes value V
  -> identify grounded source
  -> establish legal visibility path
```

## Evidence and invalidation

Concurrency evidence must bind exact dependencies.

Potential invalidation inputs include:

```text
event graph identity
memory footprint identity
RFC 0009 provenance/epoch
permission algebra state
operation implementation
atomic reference specification
rely/guarantee contract
target memory model
backend version
scheduler/runtime profile
fairness assumptions
verifier identity
```

A local footprint change should invalidate only independence/atomicity/visibility evidence that
depends upon that footprint where the dependency graph permits such precision.

This is important for recursive refinement: changing one concurrent component should not require
re-proving unrelated disjoint regions.

## Proof boundary

RFC 0007 remains authoritative.

The MNCS proof kernel should not absorb:

```text
scheduler implementation
weak-memory model checker
concurrent separation-logic automation
lock-free proof search
stress testing
race fuzzing
benchmarking
```

Those systems may produce:

```text
formal proof terms
checkable certificates
soundness-scoped verifier results
empirical observations
assumptions
UNKNOWN
```

The method class remains visible.

## Diagnostics and machine repair

Concurrency diagnostics should be semantic and repair-oriented.

Example:

```text
CONFLICT E41 E82

reason:
    overlapping write/read footprint A[64..128)

missing:
    visibility/order relation

current authority:
    E41 Write<A[0..128)>
    E82 Read<A[64..192)>

candidate repair classes:
    establish disjoint partition
    add publication/consumption relation
    transfer ownership
    serialize events
    prove commutativity/isolation
```

A diagnostic must not automatically choose a repair that broadens authority or weakens semantics.

Forge may generate candidates within the bounded refinement model from RFC 0004.

## Optimization opportunities

The concurrency graph may expose optimization opportunities including:

```text
parallel scheduling
work stealing
vectorization across independent events
batching
coalescing
synchronization elimination
lock elision
weaker verified memory ordering
ownership migration
NUMA-aware task placement
GPU offload
parallel reductions
commutativity-based reordering
queue fusion
pipeline overlap
```

Every optimization remains bounded by semantic requirements and evidence.

## Machine-native cost model

A concurrency realization may be compared across dimensions such as:

```text
runtime latency
throughput
work
span
memory footprint
synchronization count
atomic retry rate
cache traffic
NUMA traffic
energy
code size
proof cost
verification cost
assumption count
invalidation radius
portability
```

No single metric is universally primary.

Forge or another optimizer may select a realization according to explicit objective functions and
assurance policy.

## Interaction with RFC 0008

RFC 0008 asynchronous I/O events should participate in the same causality graph.

An in-flight DMA operation may consume memory authority from RFC 0009 while producing a completion
event under RFC 0008.

RFC 0010 then determines which computations may overlap that event and which completion/visibility
relations are required before memory authority returns.

I/O and computation should not require separate scheduling ontologies.

## Interaction with RFC 0009

RFC 0009 provides:

```text
memory object identity
allocation epoch
provenance
footprint
permissions
disjointness
pinning/stability
```

RFC 0010 consumes those facts to determine:

```text
potential concurrency
conflict
safe sharing
ownership transfer
race freedom
visibility obligations
synchronization requirements
```

RFC 0010 must not redefine address identity or provenance.

## Interaction with RFC 0006

Machine-intent expressions may express concurrency preferences such as:

```text
prefer parallel
prefer low_tail_latency
prefer no_blocking
prefer worker_locality
```

Requirements such as constant-time execution or bounded resource use remain distinct and may restrict
concurrency realizations.

A performance preference never weakens visibility or atomicity semantics.

## Interaction with RFC 0007

Formal claims about concurrency may become propositions in the dependent proof core where practical.

Examples:

```text
Disjoint(FootprintX, FootprintY)
HappensBefore(E1, E2)
Linearizable(Impl, Spec)
```

However, rich weak-memory and liveness reasoning may remain external with checkable certificates or
soundness-scoped evidence.

The trusted kernel should stay small.

## Interaction with future failure/nondeterminism semantics

RFC 0010 intentionally reserves broader questions such as:

```text
partial failure
crash recovery
retry semantics
external nondeterminism
clock/time semantics
randomness
process failure
network partition
```

A future RFC should build on RFC 0010 event identity, task scopes, progress assumptions, and
observation provenance rather than redefining them.

## Candidate semantic laws

The initial research model should test laws such as:

### Causality transitivity

If:

```text
A happens_before B
B happens_before C
```

then:

```text
A happens_before C
```

for relation classes where transitivity is semantically valid.

### Disjoint parallel composition

If:

```text
Footprint(X) * Footprint(Y)
```

and effects/authority/protocol relations are compatible, then memory conflict alone does not require
serialization.

### Authority conservation

Concurrent splitting or transfer of linear/exclusive authority must conserve the permission algebra
from RFC 0009.

### Grounded observation

Every accepted shared observation has at least one legal grounded origin under the selected model.

### Stronger realization preservation

A realization may enforce stronger ordering than required when policy permits, but must not change
observable semantics prohibited by the contract.

### Evidence monotonicity is not assumed

Adding a synchronization edge can remove executions but may also alter liveness/performance and can
invalidate evidence. Stronger ordering is not automatically semantically neutral for every program.

## Experimental tracks

RFC 0010 should begin with bounded experiments rather than immediate full-language implementation.

### Track 1: Event-DAG interpreter

Build a deterministic research representation of events and partial-order dependencies.

Requirements:

- enumerate or sample legal schedules under a finite bound;
- verify dependency preservation;
- compare required outputs across schedules;
- report schedule-sensitive outcomes;
- retain exact event identities.

This is not a production scheduler.

### Track 2: RFC 0009 disjoint-memory parallelizer

Use explicit RFC 0009 footprints and permissions to prove that two bounded operations may execute in
parallel.

Required negative cases:

- overlapping conflicting write/write;
- overlapping write/read;
- stale disjointness evidence;
- wrong allocation epoch;
- UNKNOWN alias relation.

UNKNOWN must select serialization or another conservative fallback.

### Track 3: Visibility lowerer

Represent one semantic publication/consumption pattern and lower it to at least two distinct target
memory-model strategies.

The experiment should record:

```text
semantic visibility relation
target model
selected instructions/IR ordering
proof/verifier evidence
fallback when evidence is missing
```

### Track 4: Race/conflict verifier

Detect conflicting bounded event pairs and produce machine-readable diagnostics with:

```text
overlapping footprint
authority state
missing synchronization relation
candidate repair classes
```

### Track 5: Observation provenance tracer

For bounded shared reads, record the exact write or initialization legally observed.

Reject or flag:

- ungrounded value origin;
- impossible epoch;
- visibility path violation;
- stale source identity.

### Track 6: Abstract atomic object

Implement one small queue/counter object under the same abstract transition contract using at least two
realizations, such as:

```text
lock-based
CAS-based
```

Compare both against one reference semantics and require independent evidence for the claimed
atomicity property.

### Track 7: Deterministic parallel reduction

Represent an associative reduction and compare:

```text
sequential realization
parallel tree realization
```

Measure:

```text
result semantics
work
span
proof burden
synchronization cost
```

Include an intentionally non-associative or strict-floating-point counterexample to ensure algebraic
freedom is not overclaimed.

### Track 8: Structured task lifetime

Build a bounded task scope with child activities, cancellation, resource transfer, and exit
obligations.

Reject orphaned authority and child-owned RFC 0009 resources that escape without explicit transfer.

## Required negative tests

Before any part of RFC 0010 graduates, tests should include at least:

```text
parallelization from UNKNOWN alias relation
stale disjointness evidence used for parallelization
read observing a write with no legal visibility path
cyclic thin-air-style value justification
conflicting exclusive authority in concurrent events
child task outliving owning scope without transfer
lock_free claim with missing progress evidence
fairness-dependent proof with fairness assumption omitted
atomic implementation violating reference transition
commutativity claim ignoring external effect
reduction reassociation violating strict numeric semantics
backend weakening ordering without sufficient evidence
```

Each failure should emit stable machine-readable diagnostics.

## Graduation criteria

A concurrency feature should graduate only when it demonstrates measurable value relative to its
semantic and implementation complexity.

Candidate criteria include:

- increased safely exploitable parallelism;
- reduced false serialization;
- stronger race/conflict diagnostics;
- successful target-independent visibility lowering;
- independent verification of at least one atomic abstraction;
- deterministic schedule exploration over bounded examples;
- conservative behavior under UNKNOWN;
- bounded evidence invalidation after local changes;
- useful machine-generated repair candidates;
- stable canonical identities through HIR/SSA lowering; and
- no reliance on undefined behavior as an optimization contract.

## Open questions

Major research questions remain.

1. Which event relations belong in the canonical semantic graph versus derived IR?
2. What is the smallest weak-memory semantic model that remains expressive enough for target
   portability?
3. How should observation provenance be represented without making artifacts unmanageably large?
4. Which independence facts should be proof terms versus verifier evidence?
5. How should rely/guarantee contracts compose with effect rows and session types?
6. What exact permission algebra from RFC 0009 is sufficient for concurrent sharing?
7. Should logical atomicity be a standard semantic property or an optional verifier extension?
8. How should speculative execution be represented without admitting thin-air values?
9. How should fairness assumptions interact with schedulers selected dynamically at runtime?
10. Which progress properties can be checked compositionally?
11. Should work/span estimates be semantic annotations, evidence objects, or optimizer-only metadata?
12. How should dynamic task graphs preserve stable event identity?
13. How should target-specific GPU/SIMT memory scopes fit the same visibility model?
14. How should transactional isolation levels interact with external I/O and irreversible effects?
15. How should concurrency semantics cross FFI boundaries with weaker memory models?

These questions should be answered experimentally rather than by importing one existing language's
answers wholesale.

## Design principles

RFC 0010 establishes the following provisional principles.

### Independence is semantic

> **Operations may execute concurrently only when their relevant dependencies, resources,
> authority, protocols, effects, visibility, and atomicity constraints establish that concurrency is
> legal.**

Absence of information is not independence.

### Synchronization is derived

> **Programs declare required causality, exclusivity, visibility, and abstract atomicity; locks,
> atomics, fences, channels, and transactions are realizations.**

### Observations have provenance

> **A shared-state observation must have a legal grounded origin and visibility path.**

### Progress has assumptions

> **A liveness claim is incomplete unless the environmental and fairness assumptions required for
> it are explicit.**

### Parallelism does not weaken semantics

> **Performance pressure may influence realization selection but may not convert UNKNOWN into proof,
> relax atomicity, or weaken visibility requirements without an explicit semantic contract permitting
> that change.**

## Provisional direction

The preferred initial direction is therefore:

```text
semantic events
+ explicit partial order
+ RFC-0009 resource footprints and permissions
+ rely/guarantee interference contracts
+ visibility and observation provenance
+ abstract atomic transitions
+ structured task lifetimes
+ progress/fairness contracts
+ proof/evidence-bound independence
        ↓
verified concurrency realization
        ↓
threads / tasks / workers / GPU work
locks / atomics / fences / channels / transactions
        ↓
target memory model
```

The defining thesis is:

> **MNCS concurrency is a semantic partial order of events, resource interactions, authority
> transfers, observations, and abstract state transitions. Parallel execution is permitted wherever
> the graph establishes independence; synchronization and memory-order mechanisms are verified target
> realizations of the graph's required causality and visibility.**
