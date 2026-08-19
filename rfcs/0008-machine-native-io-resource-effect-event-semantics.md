# RFC 0008: Machine-Native I/O, Resource, Effect, and Event Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.4–1.0
- **Depends on:** RFC 0002, RFC 0003, RFC 0006, RFC 0007

## Summary

MNCS Language should not define I/O primarily as a library of syscalls, a single `IO<T>` effect, a
mandatory async runtime, or a fixed hardware mechanism. It should instead represent interaction
with the external machine as a typed semantic graph of **effects, authority, resource ownership,
resource-state transitions, protocols, event dependencies, contextual requirements, obligations,
and evidence**.

The source or semantic artifact states *what interaction is required* and *which properties must
hold*. The compiler, runtime, Forge, or target-specific backend may then select a verified physical
realization such as:

- synchronous or asynchronous syscalls;
- polling, interrupts, or adaptive completion;
- buffered or direct I/O;
- memory mapping;
- memory-mapped device registers;
- kernel submission/completion queues;
- scatter/gather I/O;
- DMA or peer-to-peer DMA;
- registered or pinned buffers;
- zero-copy receive/transmit;
- kernel bypass;
- RDMA;
- accelerator or GPU transfers; or
- a conservative portable fallback.

The central rule is:

> **MNCS I/O models ownership, authority, state transition, causality, and observable effects;
> syscalls and hardware mechanisms are realizations.**

This RFC proposes a type-system direction capable of expressing that model. It combines ideas from
linear and uniqueness typing, quantitative types, algebraic effects and effect rows, graded effects,
typestate, capabilities, region typing, session/protocol typing, coeffects/contextual typing,
dependent types, and explicit asynchronous event graphs. None of those traditions is adopted
wholesale. Each contributes a dimension that is useful to machine-authored systems code.

The goal is not merely memory safety. The type and evidence system should make enough machine facts
explicit that autonomous generation, optimization, scheduling, verification, and target-specific
lowering have substantially more information than conventional source code normally provides.

## Motivation

Most existing I/O interfaces expose decisions chosen for human programmers or historical operating
system APIs. A call such as `read(fd, buffer, n)` hides many properties that a machine optimizer may
need to reconstruct:

- whether the operation may run concurrently with another read;
- who owns the destination buffer while the request is in flight;
- whether the buffer may be pinned, registered, shared, or device-visible;
- whether copying is semantically required or merely an implementation choice;
- whether completion must be interrupt-driven, polled, or either;
- whether order matters among independent requests;
- whether cancellation is permitted and what state it leaves resources in;
- whether a device, file, channel, or memory range is actually authorized;
- whether the operation is idempotent or externally irreversible;
- whether a protocol step is legal in the current state;
- whether locality, NUMA placement, alignment, cacheability, or topology matters;
- what failure and partial-completion states are possible;
- whether a latency, throughput, energy, memory, or determinism objective applies; and
- which of those claims are proved, checked, assumed, measured, or unknown.

A human-oriented language often relies on comments, library conventions, runtime state, framework
APIs, compiler inference, or unsafe code to recover those facts. A machine-native language can make
them first-class semantic objects.

RFC 0006 already establishes that machine intent should expose exact semantics, capabilities,
requirements, lowering envelopes, and evidence rather than encode optimization intent in accidental
source patterns. RFC 0007 establishes a proof-carrying dependent core while explicitly keeping
state, I/O, concurrency, foreign calls, and target-specific execution in an effectful executable
stratum. This RFC defines the bridge between those ideas for external interaction.

## Design goals

MNCS I/O should make the following possible:

1. **Precise machine reasoning.** The compiler should not need to infer critical ownership,
   authority, ordering, protocol, or effect facts from conventions when the generator already knows
   them.
2. **Verified implementation freedom.** One semantic operation may admit many target realizations,
   provided obligations are discharged and declared behavior is preserved.
3. **Asynchronous composition by default.** Independent I/O should remain visibly independent until
   an actual dependency requires synchronization.
4. **Resource safety by construction.** Buffers, handles, queue entries, mappings, channels, device
   regions, and completion tokens should carry machine-checkable usage and state information.
5. **Authority minimization.** Possessing data is not equivalent to possessing authority to perform
   an effect. Authority must be explicit and scope-bounded.
6. **Protocol correctness.** Legal sequences of interaction should be representable in types and
   proof obligations instead of only in runtime documentation.
7. **Optimization without semantic laundering.** Latency, throughput, zero-copy, polling, DMA, or
   kernel-bypass preferences cannot silently weaken correctness or security requirements.
8. **Portability with specialization.** Target-specific mechanisms may be selected aggressively when
   evidence supports them, while conservative fallbacks remain semantically equivalent where
   required.
9. **Proof-aware generation.** The authoring machine may choose an implementation that is easier to
   prove, has a smaller invalidation radius, or needs less authority even when raw runtime speed is
   not maximal.
10. **Inspectable artifacts.** Humans need not author the canonical machine form, but the resulting
    resource transitions, effects, authority, dependencies, obligations, and selected lowering must
    remain inspectable.

## Non-goals

This RFC does not:

- standardize final surface syntax;
- select one operating system, async executor, device API, or kernel interface;
- require every program to use zero-copy, DMA, polling, or memory mapping;
- make empirical latency or hardware behavior formally true merely because it was measured;
- place arbitrary device behavior inside the trusted proof kernel;
- require every effectful program to be total;
- require one universal scheduler; or
- guarantee that all desirable I/O properties are decidable statically.

Unsupported properties may remain `UNKNOWN`, trigger runtime checks, force a safer lowering, or
reject promotion under the active assurance policy.

## Core design principle: semantic I/O above physical I/O

A semantic operation should describe the interaction independently of a particular realization.
Conceptually:

```text
IOOperation {
    operation: SemanticOperation,
    inputs: [TypedValue],
    outputs: [TypedValue],

    effects: EffectRow,
    authority: [CapabilityRequirement],

    consumed_resources: [ResourceUse],
    produced_resources: [ResourceProduction],
    state_transitions: [ResourceTransition],

    protocol_steps: [ProtocolTransition],
    dependencies: EventRelation,
    completion: CompletionSemantics,
    cancellation: CancellationSemantics,
    failure: FailureSemantics,

    context_requirements: ContextRow,
    machine_intent: IntentSet,
    resource_budget: ResourceBudget,

    requirements: PropertySet,
    lowering_envelope: LoweringEnvelope,
    portability_envelope: PortabilityEnvelope,

    obligations: [ObligationRef],
    evidence_policy: EvidencePolicy,
}
```

The physical realization is a separate artifact:

```text
Realization {
    semantic_operation_identity,
    target_profile,
    mechanism,
    scheduler_strategy,
    memory_strategy,
    completion_strategy,
    generated_artifact,
    discharged_obligations,
    remaining_assumptions,
    measurements,
    evidence,
}
```

The semantic identity must not change merely because the compiler chooses `pread`, `io_uring`, an
NVMe queue, DMA, RDMA, MMIO, a userspace driver, or a portable fallback.

## A multidimensional type system

MNCS should resist collapsing all relevant information into a single nominal type constructor such
as `IO<T>` or `Future<T>`. A machine-facing type judgment may need multiple dimensions.

Conceptually:

```text
Γ ; A ; R ; E ; P ; C ; D ⊢ expression : T
```

where:

- `Γ` — ordinary value/type environment;
- `A` — authority/capability environment;
- `R` — quantitative resource and ownership environment;
- `E` — effect row or effect graph;
- `P` — protocol/typestate environment;
- `C` — contextual or coeffect requirements;
- `D` — event dependency/causality environment; and
- `T` — logical result type, potentially dependent.

This notation is illustrative. The eventual calculus may encode some dimensions inside others, but
implementations must not erase the semantic distinctions merely for surface convenience.

### Value types

Ordinary logical values remain necessary:

```text
Bytes<16384>
Packet
FileOffset
Result<T, E>
Vector<T, N>
```

Dependent indices may bind lengths, alignments, protocol identities, device identities, or ranges
when those facts materially affect safety or lowering.

### Ownership and quantity

Resources should be able to carry usage disciplines such as:

- unrestricted/copyable;
- affine — use at most once;
- linear — use exactly once through a transition;
- unique/exclusive — no competing alias may exercise conflicting authority;
- shared-read;
- borrowed under a bounded lifetime or event scope; and
- graded/quantitative — usable according to an explicit resource count or grade.

Illustrative types:

```text
Unique<Buffer<B>>
SharedRead<Region<R>>
Linear<QueueSlot<Q>>
Affine<CancelToken<E>>
Credits<SubmissionQueue<Q>, 32>
```

A machine author should not need to remember that an in-flight DMA destination must not be mutated.
The resource transition should remove the relevant writable capability from the originating context
until completion produces a new valid resource state.

### Capability and authority types

Possession of a reference does not imply permission to perform every operation on the referenced
resource.

Illustrative capabilities:

```text
Read<File<F>>
Write<File<F>>
Receive<Nic<N>>
Transmit<Nic<N>>
Map<Device<D>, Range<R>>
MmioRead<Device<D>, RegisterSet<S>>
MmioWrite<Device<D>, RegisterSet<S>>
Submit<Queue<Q>>
Admin<Device<D>>
```

Capabilities must be forge-resistant semantic objects constructed by trusted rules, explicit
boundary import, proof, or authorized runtime checks. They may themselves be linear, affine,
borrowed, delegated, attenuated, or persistent.

Authority should compose with RFC 0006 security-bound values and effect ceilings. A function that
has `Read<File<F>>` should not gain network-send authority merely because its implementation has
access to a broad runtime object.

### Effect rows and algebraic effect structure

A single undifferentiated `IO` effect hides too much. MNCS should research open effect rows or an
equivalent effect graph able to distinguish effects such as:

```text
read(File<F>)
write(File<G>)
receive(Nic<N>)
transmit(Nic<N>)
map(Device<D>)
mmio_read(Device<D>, Registers<S>)
mmio_write(Device<D>, Registers<S>)
allocate(Region<R>)
pin(Region<R>)
spawn(TaskClass<C>)
clock_read(Clock<K>)
random(EntropySource<S>)
foreign(CallSite<X>)
```

Effects may be polymorphic and compositional. A generic transformation may preserve an unknown tail
of effects while proving that it adds none of its own.

Handlers, runtime adapters, or target-specific realizers may interpret abstract effects without
requiring each execution mechanism to be a primitive in the trusted kernel.

### Graded effects

For machine-native optimization and assurance, the language should research effect grades rather
than only yes/no membership.

Examples include:

```text
allocate(bytes <= 4096, count <= 2)
write(File<F>, bytes <= N)
submit(Queue<Q>, outstanding <= 64)
network_send(Peer<P>, packets <= 4)
retry(count <= 3)
```

Grades may support static proof, symbolic bounds, runtime enforcement, or `UNKNOWN`. They can inform
resource scheduling, denial-of-service analysis, bounded-memory guarantees, and implementation
selection.

### Typestate and resource-state types

External resources frequently have legal states. Those states should be explicit rather than hidden
inside runtime objects.

```text
Socket<S, Unbound>
Socket<S, Bound<A>>
Socket<S, Listening>
Socket<S, Connected<P>>
Socket<S, HalfClosed>
Socket<S, Closed>
```

Likewise for device and transfer resources:

```text
Buffer<B, Owned>
Buffer<B, Registered<Device<D>>>
Buffer<B, InFlight<Request<R>>>
Buffer<B, Completed<Request<R>, Bytes<N>>>
Buffer<B, Reclaimed>
```

An operation consumes one state and produces another. Illegal transitions are rejected or produce
explicit obligations when the state is only dynamically known.

### Protocol and session types

Typestate handles one resource. Many I/O systems involve a protocol between multiple actors.
MNCS should reserve a protocol/session dimension able to express legal communication sequences,
choices, recursion, delegation, and termination.

Illustrative protocol:

```text
protocol StorageSession {
    Send<RequestHeader>
    Choose {
        Read  -> Send<ReadRequest>;  Receive<ReadReply>;  Continue
        Write -> Send<WriteRequest>; Receive<WriteAck>;   Continue
        Close -> End
    }
}
```

A channel endpoint should carry its current protocol position. Sending the wrong message, receiving
out of sequence, reusing a consumed endpoint, or terminating early should become a type error,
proof obligation, or explicitly modeled recovery path rather than undefined protocol behavior.

Dependent protocol types may make future states depend on earlier values such as negotiated feature
sets, message lengths, authentication results, queue identifiers, or device capabilities.

### Context and coeffects

Effects describe what computation changes. Machine lowering also depends on what execution
*requires*. MNCS should research a contextual/coeffect dimension for facts such as:

```text
requires_target_feature(avx2)
requires_iommu_domain(D)
requires_numa_node(1)
requires_memory_kind(pinned)
requires_alignment(4096)
requires_cache_policy(device)
requires_interrupt_vector(V)
requires_queue_capacity(Q, 16)
requires_clock_resolution(1_us)
requires_device(Device<Nic0>)
```

These are not ordinary effects. They constrain where and how an operation may execute. A scheduler
or backend may satisfy them, choose another realization with weaker requirements, or reject the
candidate.

### Event and causality types

Asynchronous interaction should expose causality explicitly. Submission produces an event identity
or typed completion capability rather than forcing immediate blocking.

```text
let (event_a, in_flight_a) = submit(read_a, owned_buffer_a)
let (event_b, in_flight_b) = submit(read_b, owned_buffer_b)

let completed_a = await(event_a, in_flight_a)
let completed_b = await(event_b, in_flight_b)

process(completed_a, completed_b)
```

The canonical semantic graph may represent the same operation without textual sequencing:

```text
read A ───┐
          ├── process ─── write C
read B ───┘
```

If no dependency edge exists between `read A` and `read B`, the implementation is free to execute,
batch, reorder, overlap, or distribute them within the declared effect and consistency semantics.

This is a major machine-native opportunity: independent work remains independent until a true data,
resource, protocol, authority, or ordering dependency requires synchronization.

## Asynchrony as semantic default

MNCS should investigate making effectful external operations conceptually asynchronous unless their
semantics inherently require immediate completion.

A synchronous source convenience form may elaborate to:

```text
submit operation
require completion dependency
continue with completed resource
```

rather than defining synchronous blocking as the primitive meaning.

This permits the compiler/runtime to choose among:

- immediate completion;
- blocking syscall;
- task suspension;
- submission/completion queue;
- polling;
- interrupt completion;
- batching;
- coalescing;
- multiple hardware queues;
- work stealing;
- remote execution; or
- explicit rejection when latency or determinism constraints cannot be met.

The semantic model must nevertheless preserve externally observable order where the contract
requires it.

## Resource transitions and uniqueness

Uniqueness and linearity are particularly important for I/O because external agents may concurrently
access memory or device state.

A transfer may have a state machine such as:

```text
Unique<Buffer<B, Owned>>
        |
        | submit
        v
Linear<Buffer<B, InFlight<Request<R>>>>
        |
        | completion
        v
Unique<Buffer<B, Completed<Request<R>, Bytes<N>>>>
        |
        | reclaim/accept
        v
Unique<Buffer<B, Owned>>
```

The writable `Owned` capability is absent while the device owns the transfer. The authoring machine
therefore cannot accidentally mutate a DMA destination unless it explicitly crosses an unsafe or
shared-concurrency boundary with corresponding obligations.

Resource transitions should be compositional. Scatter/gather I/O may consume a vector of distinct
regions; completion may return the same regions with per-segment completion evidence.

## Buffers, regions, provenance, and physical memory properties

A buffer is not merely `Bytes`. Physical realization may depend on properties such as:

- allocation identity;
- provenance;
- virtual and physical contiguity;
- alignment;
- lifetime;
- aliasing;
- mutability;
- pinning/registration;
- sharing domain;
- NUMA location;
- cache policy;
- device visibility;
- IOMMU mapping;
- page size;
- encryption/confidential-computing domain; and
- persistence semantics.

These should be represented as typed facts or evidence-bearing capabilities rather than folded into
raw pointers.

Illustrative construction:

```text
let owned = allocate.bytes<16384>()
let aligned = verify.aligned<4096>(owned)?
let unique = verify.exclusive(aligned)?
let registered = device.register<Nic0>(unique, register_cap)?
```

A target may skip unnecessary runtime work when the required facts are already statically or
evidentially established.

## Memory-mapped I/O

Memory mapping should be a possible semantic relation, not a universal I/O abstraction.

A file mapping may produce a typed region with file-backed persistence semantics. A device mapping
may produce a device-region capability with explicitly different volatility, ordering, cache, and
fault behavior.

Illustrative device region:

```text
DeviceRegion<
    device = Nic0,
    range = BAR0[0x0000..0x3fff],
    permissions = [read, write],
    volatile = true,
    cache = device,
    ordering = target_profile,
>
```

Operations such as:

```text
load.volatile<u32>(region, offset, mmio_read_cap)
store.volatile<u32>(region, offset, value, mmio_write_cap)
```

must preserve target-specific device semantics. Ordinary memory optimizations may not be applied
merely because the representation resembles a pointer.

The mapping capability should bind the exact device, range, permissions, epoch/lifetime, and target
assumptions so that stale evidence cannot silently authorize a new mapping.

## Transfer semantics and DMA

DMA should generally be a realization of a higher-level transfer operation rather than the default
application semantic primitive.

Illustrative operation:

```text
transfer {
    source = Device<Nic0>.rx_queue(3)
    destination = buffer
    bytes = N

    require {
        exclusive(destination)
        destination.capacity >= N
        preserve_bytes
    }

    prefer {
        zero_copy
        device_direct
        asynchronous
        minimize_cpu_cycles
    }
}
```

Potential realizations include:

- ordinary copying read;
- vectored read;
- mapped file access;
- registered-buffer async I/O;
- DMA;
- peer-to-peer DMA;
- RDMA;
- userspace device queue;
- accelerator copy engine; or
- a target-specific fallback.

Choosing DMA may generate obligations concerning pinning, alignment, device visibility, IOMMU
mapping, cache coherence, ownership transfer, completion fencing, maximum transfer size, descriptor
lifetime, and device reset behavior.

A failed DMA obligation must not be converted into an assumption simply because DMA is preferred.
The compiler should select another valid realization or reject the candidate under policy.

## Zero-copy as an optimization property

`zero_copy` should normally be an intent or measurable property, not part of logical application
semantics. The required semantics are usually that the same bytes are transferred with the required
ownership and ordering behavior.

A program may elevate zero-copy to a hard requirement when it is operationally necessary, but doing
so narrows the portability envelope.

The verifier may distinguish:

```text
prefer copies == 0
require copies <= 1
measure copies
```

This separation lets Forge search aggressively without allowing a performance preference to alter
observable behavior.

## Polling, interrupts, and completion strategy

Polling and interrupts should usually be lowering choices.

```text
completion_policy {
    require bounded_completion
    prefer p99_latency
    budget cpu_cycles_per_second <= X
    candidates [interrupt, poll, adaptive_poll]
}
```

A backend may choose based on target capabilities, queue depth, measured workload, energy policy,
latency objectives, and proof obligations. Empirical evidence may justify a performance selection
without becoming formal proof of future timing behavior.

The chosen completion strategy and its evidence belong in the realization identity when they can
affect relevant observable properties.

## Cancellation, timeout, and partial completion

Cancellation cannot be treated as simply destroying a future. External operations may have already
produced irreversible effects.

Every cancellable I/O operation should define:

- whether cancellation is supported;
- whether it is best-effort or guaranteed before a boundary;
- what happens if completion races with cancellation;
- what resource state is returned;
- whether external side effects may already have occurred;
- whether retry is safe;
- whether partial completion is possible; and
- how completion/cancellation evidence is consumed.

Illustrative states:

```text
Pending<R>
Cancelling<R>
Completed<R, Outcome>
Cancelled<R, ExternalEffectState>
Indeterminate<R, RecoveryObligation>
```

The type system should prevent treating `Cancelled` as proof that no external effect occurred unless
that proposition is actually established.

## Failure semantics and external uncertainty

External systems fail in ways pure computation does not. Failure sets should remain explicit:

```text
ReadOutcome<N> =
    Complete<Bytes<N>>
  | Partial<Bytes<K>, K < N>
  | EndOfStream
  | Timeout
  | PermissionRevoked
  | DeviceReset
  | TransportFailure
  | Cancelled<EffectState>
```

Target APIs may expose different native failure codes. Lowering maps them into the declared semantic
failure relation rather than leaking backend-specific ambiguity into the operation.

Unknown physical state after a fault should remain unknown and may produce a recovery obligation.

## Ordering, atomicity, and consistency

The event graph must distinguish different ordering relations instead of treating source order as a
universal fence.

Possible relations include:

```text
happens_before(A, B)
completion_before(A, B)
submit_before(A, B)
visibility_before(A, B, ObserverSet)
protocol_before(A, B)
resource_conflict(A, B)
independent(A, B)
```

Device, filesystem, network, persistent-memory, and distributed operations may have different
visibility and durability models. Those models must be named by the semantic operation or target
assumptions rather than inherited accidentally from CPU memory ordering.

The compiler may reorder only when all relevant relations, effects, authority constraints, and
protocol obligations remain valid.

## Effect conflicts as scheduling facts

Once effects are explicit, effect analysis can become scheduling input.

Two operations may run concurrently when their effect/resource summaries prove non-conflict or when
the declared consistency model permits it.

For example:

```text
read(File<A>, range 0..4096)
read(File<A>, range 8192..12288)
```

may be independent, while:

```text
write(File<A>, range 0..4096)
read(File<A>, range 2048..8192)
```

may require an ordering relation unless a weaker consistency model is declared.

Range-aware effects should therefore be investigated where the proof and representation cost is
justified.

## Protocol-aware concurrency

Session/protocol information can expose concurrency that ordinary control flow hides. Independent
protocol branches or independent channels may execute concurrently; a protocol transition that
requires a response creates a true event dependency.

This may be particularly useful for:

- storage queues;
- network protocols;
- RPC;
- USB transfers;
- accelerator command streams;
- database protocols;
- distributed control planes; and
- MNCS Fabric communication.

The machine generator should be able to synthesize a protocol implementation from a protocol type,
then optimize within the legal transition graph.

## Resource budgets as types and obligations

I/O systems are frequently limited by finite queues and pinned memory rather than by logical type
correctness alone. MNCS should make bounded machine resources representable.

Examples:

```text
QueueCredits<Q, 64>
PinnedBytes<Pool<P>, 8_MiB>
Descriptors<Device<D>, 256>
InterruptVectors<Device<D>, 4>
OpenHandles<Namespace<N>, 128>
```

Submission consumes credits; completion returns them. A static proof may establish that a pipeline
cannot exceed queue capacity, or a runtime policy may enforce a dynamic bound.

Resource exhaustion should be an explicit semantic possibility when it cannot be ruled out.

## Interaction with the proof-carrying dependent core

RFC 0007 separates a small total proof/core stratum from general effectful execution. This RFC
preserves that boundary.

I/O itself is not reduced inside the proof kernel as though external reality were a pure total
function. Instead, effectful operations generate propositions and obligations whose formal parts can
be checked in the core.

Example:

```text
submit_dma(buffer, request)
```

may generate formal obligations such as:

```text
capacity(buffer) >= request.length
aligned(buffer, request.required_alignment)
exclusive(buffer)
request.length <= device.max_transfer
legal_protocol_state(queue)
```

while claims such as:

```text
device completed transfer
packet arrived from physical network
operation completed within 20 us
```

remain external observations unless a formalized model and trust boundary justify stronger claims.

Proof terms, certificates, verifier results, runtime observations, and assumptions remain distinct
evidence classes under RFC 0002 and RFC 0007.

## Quantitative and linear dependent typing

RFC 0007 reserves usage/grade information for future quantitative or linear obligations. I/O is a
strong reason to pursue that research.

A promising direction is a dependent type system where quantities can express both logical
indices and resource usage. Conceptually:

```text
consume 1 QueueCredit<Q>
borrow 1 Unique<Buffer<B>> until Event<E>
produce 1 Completion<E>
return 1 QueueCredit<Q> after Completion<E>
```

This could unify:

- uniqueness;
- linear capabilities;
- queue-resource accounting;
- buffer lifetime;
- event-scoped borrows;
- proof erasure; and
- authority transfer.

The final calculus should be chosen experimentally. The RFC does not require that all grades be
natural numbers or that every resource property be represented in the kernel.

## Effect handlers and realization boundaries

Algebraic-effect-style handlers are worth investigating as an architectural mechanism, especially
for separating semantic effects from platform realization.

An abstract effect such as:

```text
receive<Nic0>(buffer)
```

could be interpreted by different handlers/realizers:

```text
linux_socket_handler
io_uring_handler
userspace_driver_handler
simulator_handler
formal_model_handler
```

However, handlers must not become arbitrary semantic escape hatches. A handler must declare:

- which effects it handles;
- introduced effects;
- required authority;
- resource transitions;
- context requirements;
- preserved requirements;
- target envelope;
- obligations; and
- evidence identity.

This makes handler substitution a proof-carrying transformation rather than an untracked runtime
trick.

## Foreign and unsafe I/O boundaries

MNCS must interoperate with C ABIs, operating systems, drivers, libraries, and devices whose
contracts are not natively typed.

Foreign boundaries should therefore import effects and authority explicitly.

An FFI declaration should be able to state:

```text
foreign read_like(...) {
    effects = [read(fd), write(buffer)]
    consumes = [Exclusive(buffer, range)]
    produces = [Initialized(buffer, bytes_written)]
    may_block = true
    failures = [...]
    assumptions = [abi_contract, kernel_contract]
}
```

An unsafe boundary may assert facts unavailable to the verifier, but those assumptions must remain
named, scoped, invalidatable, and visible in the evidence graph. Unsafe does not mean semantically
untracked.

## IR requirements

The I/O/resource model is useful only if it survives lowering. HIR and verified SSA should preserve
or reference:

- semantic operation identity;
- effect identity and grades;
- authority/capability flow;
- ownership and resource transitions;
- protocol state;
- event identities and dependency edges;
- memory-region/provenance facts;
- contextual requirements;
- failure/cancellation semantics;
- hard requirements versus preferences;
- lowering and portability envelopes;
- obligation identities;
- evidence dependencies; and
- transformation provenance.

An optimizer must not erase the evidence trail proving that a buffer was exclusive before consuming
that fact to perform an alias-sensitive or DMA-sensitive lowering.

## Canonical semantic graph

A machine-native representation should likely store I/O as graph structure even if one surface
syntax renders it sequentially.

Illustrative graph node:

```text
EventNode {
    id: EventId,
    operation: SemanticOperationId,
    input_values: [ValueEdge],
    input_resources: [ResourceEdge],
    required_capabilities: [CapabilityEdge],
    effects: EffectRow,
    protocol_prestate: [ProtocolState],
    protocol_poststate: [ProtocolState],
    context: ContextRow,
    successors: [DependencyEdge],
    completion_relation: CompletionSpec,
    obligations: [ObligationId],
}
```

The graph can expose parallelism, batching opportunities, resource conflicts, critical paths, and
proof boundaries directly to Forge and backend schedulers.

## Example: parallel file reads

Semantic intent:

```text
operation load_pair(file_a, file_b, buf_a, buf_b) {
    let ea = read(file_a, into = move buf_a)
    let eb = read(file_b, into = move buf_b)

    let a = completion(ea)
    let b = completion(eb)

    return decode_pair(a, b)
}
```

Relevant facts:

```text
effects = [read(file_a), read(file_b), write(buf_a), write(buf_b)]
resources = [Exclusive(buf_a), Exclusive(buf_b)]
independent(ea, eb)
require preserve_bytes
prefer minimize_latency
```

Possible realizations include sequential reads, two worker threads, a submission queue, batched
asynchronous I/O, memory mapping, or target-specific direct I/O. Sequential execution is valid but
not semantically required.

## Example: network receive with ownership transfer

```text
let rx = capability.receive<Nic0>()
let buffer = allocate.rx_buffer<2048>()

let pending = receive.submit(
    nic = Nic0,
    queue = 3,
    destination = move buffer,
    authority = rx,
)

match receive.complete(pending) {
    Packet(buffer, bytes, metadata) => process(move buffer, bytes, metadata)
    Cancelled(buffer, state)         => recover(move buffer, state)
    DeviceReset(buffer?, evidence)   => reset_recovery(buffer, evidence)
}
```

The buffer is unavailable to ordinary mutation while in flight. A zero-copy or DMA realization may
consume stronger alignment, registration, or device-visibility capabilities; a copying fallback may
require fewer contextual facts.

## Example: MMIO device command

```text
let region = map.device<Nvme0, DoorbellRegs>(map_cap)?
let q = queue_state<Nvme0, Q1>()

let ready = verify.queue_ready(q)?
let slot = reserve(queue_credit<Q1>)?

store.volatile<u32>(
    region,
    Doorbell<Q1>,
    next_tail,
    mmio_write_cap,
    ordering = release_to_device,
)
```

Obligations may include valid mapping epoch, register offset, queue ownership, queue capacity,
required device ordering, and capability scope. The compiler may not turn the volatile device write
into an ordinary store or reorder it past operations that establish the submission descriptor.

## Example: protocol-typed RPC

```text
endpoint : Session<RpcProtocol, AwaitRequest>

(endpoint, request) = receive(endpoint)
(endpoint, auth) = authenticate(endpoint, request.credentials)
(endpoint, result) = execute_authorized(endpoint, request, auth)
endpoint = send(endpoint, result)
endpoint = continue(endpoint)
```

The endpoint state proves which actions are legal. Authentication may produce a capability consumed
by the protected operation, joining protocol typing with authority typing rather than leaving them
as unrelated runtime conventions.

## Optimization and Forge search

The I/O graph gives Forge a search space with explicit constraints.

A search region may vary:

- sync versus async submission;
- queue depth;
- batching;
- polling versus interrupts;
- buffer size;
- buffer registration;
- memory placement;
- NUMA affinity;
- copy strategy;
- scatter/gather shape;
- kernel versus userspace path;
- device queue selection;
- concurrency width;
- prefetching;
- completion coalescing; and
- fallback policy.

Objectives may include:

```text
latency
p99_latency
throughput
cpu_cycles
energy
memory
pinned_memory
copy_count
proof_cost
proof_check_cost
assumption_count
invalidation_radius
portability
required_authority
```

Hard semantic and security requirements bound the search. Measurements choose among candidates only
within those bounds.

## Machine-native advantage

Human-oriented languages often optimize for source readability and local reasoning. MNCS should
optimize its canonical representation for **machine reconstructibility and machine actionability**.

That means intentionally preserving information that conventional compilers frequently discard or
reconstruct:

- exact resource identities;
- authority flow;
- ownership epochs;
- protocol states;
- causal independence;
- legal reordering envelopes;
- physical-context requirements;
- proof dependencies;
- target assumptions; and
- optimization objectives.

The semantic graph may be more structured than a human would willingly hand-author. That is not a
problem if machine authors generate it reliably and human-facing projections remain available.

## Security consequences

The proposed type system can make several classes of bugs structurally harder:

- use-after-submit of DMA buffers;
- double close or double free;
- stale device mappings;
- writes through read-only authority;
- accidental privilege widening;
- protocol message reordering;
- reuse of consumed nonces or one-shot tokens;
- queue overcommit;
- unsynchronized conflicting I/O;
- incorrect cancellation assumptions;
- unauthorized filesystem or network effects; and
- hidden effects introduced by foreign or handler boundaries.

It does not eliminate malicious hardware, compromised kernels, unsound verifiers, faulty device
models, side channels, or incorrect assumptions. Those boundaries remain explicit evidence and
threat-model concerns.

## Portability and specialization

Every target-specific I/O realization should state its portability envelope. A semantic operation
may offer a realization set such as:

```text
realizations = [
    io_uring_registered,
    posix_pread,
    windows_overlapped,
    userspace_nvme,
    portable_buffered,
]
```

Selection is permitted only when target/context obligations are satisfied. Unsupported specialized
paths fall back, produce `UNKNOWN`, or reject compilation according to policy.

A portability fallback must preserve required semantics, not necessarily performance properties that
were only preferences.

## Type erasure and runtime representation

Rich static semantics need not imply expensive runtime tags everywhere. Proofs, grades,
capabilities, protocol indices, and region facts may be erased when their runtime presence is not
required after checking.

Other objects necessarily have runtime representations, such as OS handles, device descriptors,
completion IDs, queue positions, or revocable capabilities.

Erasure itself should be a verified transformation with explicit conditions. A capability may be
logically non-copyable even if its runtime representation is a machine word.

## Research questions

The implementation program should compare multiple formal approaches rather than prematurely select
one calculus.

Priority questions include:

1. Should effect rows be algebraic sets, ordered effect graphs, indexed relations, or a hybrid?
2. Which resource disciplines need linearity versus affinity versus uniqueness versus quantitative
   grades?
3. Can a quantitative dependent core express queue credits and event-scoped borrows without making
   type checking impractical?
4. How should event completion interact with borrow lifetimes and proof invalidation?
5. Should protocol/session types live in the trusted kernel, elaborate into dependent state
   transitions, or use checkable certificates?
6. Which contextual/coeffect facts belong in types versus target profiles or evidence objects?
7. How much range information should effects carry before proof cost outweighs scheduling benefit?
8. What is the smallest canonical event graph that preserves enough information for aggressive
   lowering and independent verification?
9. How should revocable capabilities be represented when authority can disappear asynchronously?
10. How should device reset invalidate resource-state proofs and mapping capabilities?
11. Can effect handlers be verified compositionally without turning the handler system into a large
    trusted runtime?
12. Which timing, energy, and throughput properties should remain empirical objectives versus
    contractually enforceable bounds?
13. How should distributed I/O represent partial failure and uncertain external state?
14. Can protocol types and capability types share one transition calculus?
15. What subset is practical enough for micro-model generation and local proof search in MNEL?

## Experimental tracks

The RFC should be validated through competing prototypes rather than a single implementation.

### Track A: linear resource core

Prototype unique/linear buffers, handles, queue credits, and event-scoped ownership transfer.

Success criteria:

- reject double use and use while in flight;
- support asynchronous completion returning ownership;
- preserve ownership evidence through SSA; and
- erase proof-only resource structure where valid.

### Track B: effect rows and capability closure

Prototype effect-polymorphic functions plus explicit capability requirements.

Success criteria:

- infer or validate bounded effect sets;
- reject undeclared authority expansion;
- support generic effect-preserving transformations; and
- preserve effect identities in verified IR.

### Track C: typestate and protocols

Prototype a socket or storage protocol with dependent state transitions.

Success criteria:

- reject illegal transition sequences;
- permit machine synthesis from the protocol graph;
- expose independent operations to the scheduler; and
- produce checkable protocol-transition evidence.

### Track D: async event graph

Prototype graph-native file and network I/O independent of one executor.

Success criteria:

- represent true dependencies without accidental sequentiality;
- lower to at least one synchronous and one asynchronous backend;
- prove semantic equivalence for a bounded test set; and
- demonstrate batching/reordering unavailable from a naive sequential form.

### Track E: DMA/registered-buffer lowering

Prototype a transfer semantic operation with copying and device-direct candidates.

Success criteria:

- generate alignment, ownership, registration, and completion obligations;
- select DMA/registered-buffer lowering only when obligations pass;
- fall back correctly when they do not; and
- preserve resource-state evidence across completion.

### Track F: MMIO capability model

Prototype mapped device regions and volatile ordered operations.

Success criteria:

- bind mappings to device/range/permission/epoch;
- reject stale or unauthorized access;
- preserve target ordering requirements; and
- prevent ordinary-memory optimization from changing MMIO semantics.

## Proposed initial semantic primitives

The first prototype should remain small. Candidate primitives include:

```text
resource.move
resource.borrow
resource.return
resource.transition

capability.require
capability.attenuate
capability.delegate

region.allocate
region.verify_aligned
region.verify_exclusive
region.register
region.map

io.submit
io.complete
io.cancel
io.transfer

event.after
event.join
event.race
event.timeout

effect.declare
effect.handle

protocol.transition

load.volatile
store.volatile
```

These names are placeholders. The important requirement is that they lower into stable semantic
identities with explicit obligations rather than becoming syntax-only compiler hints.

## Relationship to conventional paradigms

This RFC intentionally treats familiar I/O approaches as partial solutions at different layers:

- **Monadic I/O** contributes disciplined effect sequencing, but a single `IO<T>` is too coarse for
  the canonical MNCS semantic model.
- **Algebraic effects/effect rows** contribute compositional effect identity and realization
  separation.
- **Uniqueness and linear types** contribute ownership transfer and safe resource lifecycle.
- **Quantitative types** contribute bounded resource accounting and usage grades.
- **Typestate** contributes legal local state transitions.
- **Session types** contribute legal multi-party protocol progression.
- **Capabilities** contribute explicit authority and least privilege.
- **Region types** contribute memory provenance, lifetime, locality, and device-visible storage
  constraints.
- **Coeffects/contextual types** contribute target and environment requirements.
- **Async I/O** contributes non-blocking event structure, but no one executor becomes semantic law.
- **Memory mapping, MMIO, DMA, RDMA, kernel bypass, polling, and interrupts** are generally physical
  realization strategies selected inside a lowering envelope.

The MNCS contribution is the integration of these ideas into one evidence-bearing machine semantic
artifact designed for autonomous generation and refinement.

## Graduation criteria

This RFC should not be accepted merely because the model is expressive. It should graduate only if
experiments demonstrate that the additional semantic structure provides measurable machine benefit.

Evidence should include some combination of:

- fewer invalid generated I/O programs;
- stronger static rejection of resource/protocol errors;
- smaller unsafe boundaries;
- successful generation of proof obligations from ordinary constructs;
- automatic exploitation of parallel I/O from dependency graphs;
- target-specific lowering without source rewrites;
- verified fallback between specialized and portable realizations;
- improved zero-copy/DMA selection safety;
- bounded queue/resource reasoning;
- useful Forge optimization search;
- acceptable proof/checking cost;
- acceptable semantic representation size; and
- independent verifier agreement.

If a type-system feature adds substantial complexity without improving generation, verification,
optimization, or safety, it should remain experimental or be removed.

## Proposed implementation sequence

1. Define stable semantic identities for resources, capabilities, effects, events, and transitions.
2. Add linear/unique resource flow to HIR and verified SSA experiments.
3. Add explicit effect rows and capability requirements.
4. Add async event identities and dependency edges without committing to one executor.
5. Add typestate for handles and buffers.
6. Prototype protocol/session elaboration into state transitions and obligations.
7. Add contextual target requirements for alignment, memory kind, topology, and device features.
8. Implement one portable file/network backend and prove/validate reference behavior.
9. Implement one specialized async backend and compare semantic equivalence.
10. Implement a registered-buffer or DMA-style transfer candidate with fallback.
11. Implement MMIO region capabilities and ordered volatile operations.
12. Expose the event/resource graph to Forge for scheduling and lowering search.
13. Measure proof cost, invalidation radius, runtime performance, and generation quality.
14. Promote only the type dimensions that demonstrably improve machine-native development.

## Open design constraint

The type system must remain usable by machines as well as checkable by a small trusted core.
Expressiveness alone is not success. A theory that makes every operation encode enormous proof terms
or creates unstable inference may be formally elegant but poor for autonomous generation.

MNCS should therefore optimize the language design across multiple dimensions:

```text
semantic precision
proof strength
proof search cost
checking cost
representation density
invalidation radius
optimization freedom
generation reliability
portability
runtime performance
```

That multi-objective tradeoff is itself machine-native: the language should expose enough structure
for models and Forge to choose among implementations rather than force one human-centric compromise
into every program.

## Final direction

MNCS should treat I/O as a first-class resource/effect/event semantics problem rather than a standard
library afterthought.

The intended architecture is:

```text
logical operation
      +
authority
      +
resource ownership/state
      +
effects and grades
      +
protocol position
      +
context requirements
      +
event dependencies
      +
requirements / preferences
      +
obligations / evidence
      |
      v
verified lowering search
      |
      +--> synchronous syscall
      +--> asynchronous queue
      +--> mapped access
      +--> registered buffer
      +--> DMA / RDMA
      +--> MMIO
      +--> kernel bypass
      +--> accelerator path
      +--> conservative fallback
```

The machine author specifies the semantic contract and exposes machine-relevant facts. The compiler
and runtime choose the physical path they can justify. This should become a foundational part of the
MNCS type system, not merely an I/O library design.
