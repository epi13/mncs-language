# RFC 0017: Machine-Native Runtime, Environment, Target, and Execution-Context Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.3–1.0
- **Depends on:** RFC 0001, RFC 0002, RFC 0003, RFC 0004, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0013, RFC 0014, RFC 0015, RFC 0016

## Summary

MNCS Language should treat the runtime, operating system, execution environment, target architecture, platform, allocator, scheduler, collector, loader, async reactor, device stack, and other execution-support mechanisms as **verified realizations of explicit semantic requirements**, not as mandatory language ontology.

The central rule is:

> **An MNCS program specifies what must be provided by its execution context. A target and environment specify what they can provide. Runtime construction is the evidence-bearing process of satisfying the former from the latter.**

The shorter form is:

> **Runtime is a realization, not an ontology.**

RFC 0017 therefore establishes the direction for:

- contextual/coeffect requirements;
- target and environment contracts;
- semantic service requirements;
- capability and authority provisioning;
- quantitative resource requirements;
- target-property evidence;
- runtime component composition;
- runtime synthesis;
- target and runtime realization holes;
- multiversioning and late target selection;
- environment construction and initialization;
- runtime least authority;
- portable semantic identity across heterogeneous targets; and
- refinement evidence connecting target/runtime realizations back to the RFC 0012 executable semantic core.

MNCS must not constitutionally require Linux, POSIX, libc, an allocator implementation, a scheduler implementation, garbage collection, reference counting, a particular async runtime, a dynamic loader, a filesystem, a network stack, a process abstraction, virtual memory, or even an operating system.

A program that needs none of those things should not acquire them merely because the language implementation traditionally ships a large runtime. A program that does require them should express the required semantics rather than naming one accidental implementation unless that exact implementation identity is itself a hard requirement.

The intended result is a continuum from near-bare-metal artifacts through embedded systems, capability systems, library operating systems, conventional hosted environments, WebAssembly-style component worlds, accelerators, and rich general-purpose operating systems, all connected to the same semantic program through explicit environment contracts and refinement evidence.

## Motivation

The preceding RFCs define most of the semantic content needed to describe computation:

- RFC 0008 defines effects, authority, resources, protocols, and event semantics;
- RFC 0009 defines memory, reference, provenance, lifetime, storage, and representation relationships;
- RFC 0010 defines concurrency, causality, visibility, progress, and atomicity;
- RFC 0011 defines failure, recovery, uncertainty, and external observations;
- RFC 0012 defines one executable semantic machine;
- RFC 0013 defines abstraction, polymorphism, evidence, and realization holes;
- RFC 0014 defines component composition and provider/requirement refinement;
- RFC 0015 defines foreign/trust boundaries and containment; and
- RFC 0016 defines staging, late specialization, and stage-scoped transformations.

What remains underspecified is the relationship between those semantic requirements and the concrete world in which a program executes.

Without an explicit model, an implementation will eventually smuggle assumptions into the language through convention:

- every program has an allocator;
- every allocation is backed by a process heap;
- every task is backed by an OS thread or one named async runtime;
- every clock has POSIX-like semantics;
- every target has an OS;
- every target supports virtual memory;
- every program can discover all local devices;
- every runtime component is privileged;
- every scheduler has the same fairness properties;
- every file operation has filesystem semantics inherited from the host;
- every network operation implies a kernel network stack;
- every executable starts through one loader model;
- every architecture name implies a fixed set of extensions and memory semantics; or
- every target property is trustworthy merely because a target triple names a familiar platform.

Those assumptions would conflict with the machine-native direction already established.

MNCS instead needs to answer questions such as:

- Which semantic services does this computation require?
- Which contextual facts must exist for it to execute?
- Which authority must be provisioned?
- Which resources and quantitative bounds are hard requirements?
- Which target features are required and which are merely preferences?
- What is the difference between a resource existing and a component being authorized to use it?
- Which runtime components are required to bridge semantic operations to a target?
- Can a runtime closure be synthesized rather than fixed?
- Can one semantic program retain its identity across several targets?
- What evidence establishes that a target actually has an optimization-relevant property?
- When may target choice remain unresolved until link, deployment, load, or runtime?
- How should multiversioned target realizations retain shared semantic identity?
- How are runtime initialization and authority distribution represented?
- How are resource requirements verified rather than merely estimated?
- How does target uncertainty constrain optimization?
- How does runtime code participate in ordinary trust and authority analysis?
- What exactly must be invalidated when target, platform, runtime, configuration, or machine-instance facts change?

RFC 0017 is intended to make those questions part of the semantic system rather than deployment folklore.

## Constitutional principles

### Runtime is realization, not ontology

> **No allocator, scheduler, collector, reactor, operating system, loader, filesystem, network stack, or process model is inherently required by MNCS.**

Such mechanisms may be selected as realizations of semantic requirements.

### Environment requirements are explicit

> **A program may rely only on environmental services, contextual facts, resources, target properties, and authority represented in its requirement closure.**

Ambient host behavior is not specification.

### Existence is not authority

> **A resource being physically present, discoverable, mapped, or exported by a platform does not grant permission to use it.**

Availability and authority are distinct semantic facts.

### Target names are not target evidence

> **Architecture, operating-system, board, platform, and machine names identify candidates. Properties used for correctness or optimization require explicit facts and evidence.**

`riscv64`, `x86_64`, `linux`, or a machine hostname are not proofs of any stronger property.

### Runtime components obey ordinary MNCS semantics

> **Runtime providers are ordinary components. They consume authority, emit effects, require resources, carry assumptions, may cross trust boundaries, and require evidence like any other component.**

There is no privileged runtime exemption.

### Target and runtime selection are realization problems

> **Where several targets or runtime closures satisfy the same semantic requirements, selection may remain open and be optimized under RFC 0006 objectives.**

### Unknown removes specialization authority

> **If a target or environment property required by a specialization is `UNKNOWN`, the specialization is not justified.**

The system may preserve a portable realization, generate guarded variants, gather evidence under policy, or reject the target. It must not silently assume the favorable case.

### Portable semantics precede target realization

> **A semantic artifact may retain one logical identity across different verified target/runtime realizations.**

Linux, WASI, seL4-like capability environments, embedded firmware, bare metal, GPUs, accelerators, and future targets are realizations when their contracts satisfy the same semantic requirements.

### Minimal closure is preferred over ambient closure

> **The default runtime construction should include only services required by the selected semantic and realization contracts, plus explicitly accepted implementation necessities.**

A dependency should not enter the runtime merely because it is conventional.

## Non-goals

RFC 0017 does not:

- require a universal target-description language to be completed immediately;
- require all hardware microarchitectural details to become language semantics;
- require all resource bounds to be statically provable;
- define a complete distributed-systems execution model;
- define consensus, replication, partitions, distributed clocks, or consistency models;
- require a production exokernel, unikernel, or capability kernel;
- require the runtime to be dynamically synthesized on every execution;
- require every service to be implemented in MNCS Language;
- require one scheduler, collector, allocator, loader, or device model;
- require bare-metal support for the first implementation;
- claim that a target architecture's formal ISA model proves an entire physical machine correct;
- make benchmarking evidence equivalent to proof;
- make configuration names evidence of properties;
- require every machine instance to expose sensitive physical topology to every component;
- make target discovery itself ambient authority; or
- replace RFC 0015 trust-boundary semantics for foreign operating systems, kernels, hypervisors, drivers, runtimes, firmware, or device code.

## Vocabulary and object model

RFC 0017 distinguishes several objects that conventional toolchains frequently collapse.

### `ArchitectureContract`

Describes architectural instruction and memory semantics relevant to realizations.

Examples include:

- word sizes;
- scalar arithmetic behavior;
- architectural memory model;
- atomic operations;
- architectural extensions;
- privilege mechanisms;
- exception/trap semantics;
- vector/accelerator instructions; and
- address-space properties.

An architecture contract is not a specific board or physical machine.

### `PlatformContract`

Describes system-level resources and mechanisms around an architecture.

Examples include:

- available RAM regions;
- interrupt controllers;
- timers;
- IOMMUs;
- NICs;
- storage devices;
- boot mechanisms;
- coherent or noncoherent DMA;
- device addressability;
- firmware interfaces; and
- protection domains.

### `MachineInstanceProfile`

Describes one concrete machine instance at a point or validity interval in time.

It may include:

- architecture and platform identities;
- actual memory quantity;
- core count;
- NUMA topology;
- cache information where policy permits exposure;
- accelerator inventory;
- available devices;
- firmware revision;
- configuration identity;
- current feature enablement;
- measured or certified properties; and
- validity/freshness metadata.

Machine-instance facts are not automatically visible or usable by every program.

### `ExecutionRequirement`

Declares what an artifact needs from its execution context.

A requirement may include:

- semantic services;
- contextual/coeffect facts;
- authority;
- resource bounds;
- progress guarantees;
- timing guarantees;
- target properties;
- representation properties;
- isolation/assurance requirements; and
- optimization preferences.

### `ExecutionEnvironment`

Describes the services, resources, authority, guarantees, and target facts actually exposed to a particular unit.

This is intentionally narrower than the physical machine.

### `RuntimeClosure`

A graph of components selected to satisfy semantic service requirements that are not directly realized by the exposed target/environment.

### `RuntimeRealization`

The complete evidence-bearing binding from an execution requirement through a target/environment and runtime closure to an executable realization.

### `TargetEvidence`

Evidence supporting a target, platform, configuration, or machine-instance property.

### `TargetHole`

An unresolved target selection constrained by semantic requirements and objectives.

### `RuntimeHole`

An unresolved service-provider/runtime choice.

### `RealizationSet`

A family of verified realizations of one semantic artifact, usually distinguished by target or environment conditions.

## Theoretical synthesis

### Coeffects and contextual requirements

Coeffect systems are the primary theoretical inspiration for environmental requirements.

Effects describe what a computation may do. Coeffects describe what contextual resources or properties a computation requires.

MNCS should preserve this distinction.

For example:

```text
Effects {
    Observe<Clock>
    Send<Socket>
}

ContextRequirements {
    Has<MonotonicClock>
    Has<NetworkTransport>
}
```

The operation and the required environment are related, but they are not identical objects.

A semantic operation may have several environmental realizations. `Observe<Clock>` might be implemented through a hardware counter, kernel clock service, hypervisor clock, WASI clock import, device timer, or validated external service depending on the contract.

### Combined effect/coeffect reasoning

Execution judgments should support both environmental requirements and outward effects.

The multidimensional direction established by earlier RFCs remains useful:

```text
Gamma ; A ; R ; E ; P ; C ; D |- computation : T
```

`C` may now include explicit execution-context properties such as:

```text
Has<AllocatorContract<A>>
Has<MonotonicClock<Resolution <= 1ms>>
Has<Atomic<u64>>
Has<VectorWidth >= 256>
Has<AsyncWakeup>
Has<ProtectionDomain>
```

while `E` describes effects actually emitted by execution.

### Capability and authority systems

Environment provisioning should follow capability-system principles:

- resources and services are explicitly referenced;
- authority is explicitly granted;
- authority may be attenuated;
- possession of a name does not imply use authority;
- runtime providers receive only authority required by their service contracts; and
- final authority closure is inspectable.

Declarative capability-distribution models such as capDL-like descriptions are useful inspiration for environment construction.

### Exokernel-style protection/policy separation

Exokernel theory provides a strong architectural direction:

- keep protected primitive resource access small;
- avoid imposing one high-level resource policy in the foundational runtime;
- permit application-specific libraries or components to realize higher-level abstractions; and
- protect resources independently of the policy used to manage them.

MNCS need not implement an exokernel to adopt this semantic separation.

### Library-OS and unikernel specialization

Library operating systems and unikernel architectures show that operating-system services can be linked as application-specific libraries rather than requiring one monolithic ambient runtime.

MNCS should generalize this into runtime-closure synthesis.

A program requiring only allocation, a monotonic clock, and byte-stream I/O should be able to receive a closure containing only the components necessary to realize those contracts under the chosen target.

### Component/world environment interfaces

Component systems in which artifacts declare imports and exports provide useful inspiration for execution environments.

MNCS should go further than structural imports by attaching behavioral, authority, failure, resource, progress, and assurance semantics to environment requirements.

### Abstract-machine refinement

RFC 0012's semantic operations remain normative.

A runtime or target operation is valid only when its behavior refines the corresponding semantic operation under the applicable observation contract.

Conceptually:

```text
TargetRuntimeOperation
    <=
SemanticOperation
```

where `<=` denotes the appropriate refinement/simulation relation rather than textual or instruction equality.

### Formal ISA semantics

Formal architecture-description systems such as Sail demonstrate that ISA behavior can have machine-readable executable/formal semantics.

MNCS should permit target contracts to bind to architecture semantics and evidence rather than depending solely on target-name conventions.

A formal ISA model is evidence about architectural semantics, not proof of every implementation, firmware component, peripheral, or physical machine behavior.

### Resource-aware type systems

Resource-aware type systems and amortized resource analysis provide inspiration for deriving quantitative resource bounds from program structure.

MNCS should permit execution requirements such as:

```text
Heap <= 128 MiB
PinnedMemory <= 4 MiB
Stack <= 64 KiB
OutstandingIO <= 32
TaskCount <= 256
```

with evidence status attached to each claim.

A proven bound, a conservative static estimate, an empirical measurement, and an optimization forecast are distinct evidence classes.

## Environment requirement semantics

### Requirements should be semantic, not product names

Prefer:

```text
requires {
    Allocate<Region>
    Clock<Monotonic, Resolution <= 1ms>
    AsyncTasks
    ByteStream<Input>
    ByteStream<Output>
}
```

rather than:

```text
requires {
    Linux
    glibc
    Tokio
    jemalloc
}
```

unless one of those specific implementation identities is itself deliberately required.

### Hard requirements, preferences, and realization freedom

Execution requirements must distinguish:

```text
HardRequirement
Preference
OptimizationObjective
RealizationHint
```

For example:

```text
hard {
    Atomic<u32>
}

prefer {
    SIMDWidth >= 256
}
```

A target lacking the preference remains valid.

A target lacking the hard requirement is invalid unless another verified realization supplies equivalent semantics.

### Requirement closure

The complete requirement closure of a program is not merely its direct source annotations.

Conceptually:

```text
ExecutionRequirementClosure(P)
```

is derived from:

- P's semantic operations;
- selected component contracts;
- target-independent implementation requirements;
- runtime-provider requirements;
- foreign/trust-boundary contracts;
- stage-resolved choices;
- resource contracts;
- progress/timing requirements; and
- assurance policy.

The closure must remain inspectable and attributable to its sources.

A dependency that introduces a clock, network stack, allocator, dynamic loader, or additional authority should make that expansion visible.

## Availability, discovery, binding, and authority

MNCS should distinguish at least:

```text
Exists<R>
Available<R, Environment>
Discoverable<R, Subject>
Bound<R, Subject>
Authorized<A, Subject>
```

These facts are not interchangeable.

A machine may physically contain a network interface while a sandboxed program's environment does not expose it.

An environment may expose a named network service while the program lacks transmit authority.

A management component may discover devices without receiving authority to configure them.

This distinction is essential for least authority and secure deployment.

## Architecture, platform, machine, and environment layers

The following hierarchy is recommended:

```text
ArchitectureContract
    -> PlatformContract
        -> MachineInstanceProfile
            -> ExecutionEnvironment
                -> RuntimeClosure
                    -> RuntimeRealization
```

Each layer narrows or realizes facts from the preceding layers.

### Architecture is not platform

`RISC-V`, `Arm`, `x86-64`, `Wasm`, or another architecture identity does not imply a device inventory, kernel, memory capacity, scheduler, or runtime.

### Platform is not machine instance

A board/platform contract may describe possible resources. A concrete machine instance establishes actual configuration and version facts.

### Machine is not environment

A machine may possess many capabilities that an execution environment deliberately withholds.

### Environment is not runtime closure

An environment exposes target-level resources/services. A runtime closure may add higher-level service abstractions using those primitives.

## Target profile semantics

A target profile may expose facts in dimensions such as:

```text
TargetProfile {
    architecture
    architecture_semantics

    word_width
    endian
    integer_semantics

    memory_model
    atomics
    vector_capabilities

    privilege_modes
    protection
    virtual_memory

    interrupts
    timers

    DMA_properties
    cache_coherence_properties

    devices
    accelerators

    resource_limits

    target_guarantees
    assumptions
    evidence
}
```

The exact schema should evolve conservatively.

MNCS should expose only properties that are useful to correctness, compatibility, resource planning, or realization selection.

Microarchitectural details should not become language semantics merely because they can be measured.

## Target facts and evidence

### Target property claims

A target property should have:

```text
TargetFact {
    subject_identity
    property
    value

    scope
    validity_interval

    evidence
    assumptions
}
```

Examples:

```text
Supports<Atomic<u64>>
MemoryModel<RVWMO>
VectorWidth<256>
DMAVisible<RegionR, Device0>
Coherent<CPU, Device0>
PageSize<4096>
```

### Evidence classes

Target facts may be supported by:

- formal architecture semantics;
- vendor certification;
- platform configuration manifests;
- signed machine inventories;
- kernel/hypervisor attestations;
- verifier-checked probes;
- runtime feature detection;
- empirical measurement; or
- explicit assumptions.

Their strengths differ.

### Configuration-specific evidence

Target evidence should bind to the configuration on which it depends.

Conceptually:

```text
TargetEvidence {
    architecture_identity
    platform_identity
    machine_identity?
    configuration_identity
    firmware_identity?
    runtime_or_kernel_identity?
    toolchain_identity?

    property
    evidence
}
```

A changed configuration invalidates only evidence whose dependency graph includes that change.

### `UNKNOWN`

If a property is unknown:

```text
TargetFact<P> = UNKNOWN
```

then correctness-critical transformations requiring P are unavailable.

Permitted responses include:

- retain a more portable realization;
- produce guarded versions;
- obtain additional evidence under policy;
- defer selection to a later RFC 0016 stage; or
- reject the candidate environment.

`UNKNOWN` must not be treated as likely-true.

## Quantitative resource requirements

### Resource dimensions

RFC 0017 should support requirements and limits over resources including, where useful:

- total memory;
- heap-like dynamic memory;
- stack;
- static storage;
- pinned memory;
- DMA-visible memory;
- executable memory;
- file/storage space;
- outstanding I/O operations;
- task/thread counts;
- queue depth;
- descriptor/handle counts;
- device queues;
- bandwidth;
- latency/deadline budgets;
- startup budget;
- energy budgets where modeled; and
- proof/verification/runtime-management budgets where relevant.

### Proven bounds versus estimates

Every quantitative claim must retain evidence classification.

For example:

```text
ResourceBound {
    heap <= 64 MiB
    status = Proved
}
```

is distinct from:

```text
ResourceEstimate {
    heap ~= 64 MiB
    confidence = 0.85
}
```

A hard deployment policy may accept the first but not the second.

### Compositional resource closure

Runtime and application components contribute resource requirements.

The deployment analysis should compute a conservative closure while respecting sharing and exclusivity semantics rather than blindly summing every local bound.

Generated obligations should identify where exact composition is unresolved.

## Runtime services as ordinary components

Runtime services should be represented through RFC 0014 component contracts.

Candidate service categories include:

```text
Allocator
Scheduler
AsyncReactor
Clock
EntropySource
FileStore
NetworkTransport
NameResolver
Collector
DynamicLoader
TLSProvider
DeviceDriver
InterruptDispatcher
TimerService
LoggingSink
PersistentStore
```

These categories are examples, not mandatory ontology.

A service component declares:

- what semantic operation it provides;
- which lower-level services it requires;
- effects;
- authority;
- resource use;
- failure/recovery semantics;
- progress semantics;
- target constraints;
- assumptions; and
- evidence.

### Runtime components are not privileged

An allocator does not receive network authority unless its contract legitimately requires and is granted such authority.

A scheduler does not automatically receive filesystem authority.

A collector does not automatically receive authority over all foreign memory.

A logger does not automatically receive secrets merely because it is part of the runtime.

### Runtime trust is multidimensional

Runtime components may be:

- proved;
- certified;
- verifier-checked;
- runtime-guarded;
- contained;
- tested;
- assumed; or
- unknown

in different semantic dimensions.

The label `runtime` does not promote trust.

## Memory-management realizations

RFC 0009 established that memory-management strategy is not language ontology.

RFC 0017 preserves that rule.

A requirement such as:

```text
ManagedLifetime<T>
```

may be realized by:

- tracing GC;
- reference counting;
- automatic reference counting;
- region/arena discipline;
- static lifetime;
- ownership/uniqueness;
- stack discipline; or
- target-specific storage management

when refinement evidence establishes compatibility.

No mandatory MNCS garbage collector is implied.

## Scheduler and concurrency realizations

RFC 0010 defines semantic concurrency/progress requirements.

RFC 0017 should permit realizations such as:

- OS threads;
- user-space threads;
- work-stealing schedulers;
- event loops;
- interrupt-driven state machines;
- deterministic cooperative schedulers;
- GPU workgroups/wavefronts;
- actor-like runtimes;
- single-threaded state-machine lowering; or
- target-specific task engines.

The realization must satisfy the program's semantic progress, ordering, visibility, fairness, and resource requirements.

A scheduler name is not proof of those properties.

## Hosted and freestanding as a continuum

MNCS should not require a fundamental hosted/freestanding language split.

Instead:

```text
RequiredEnvironment(program)
```

determines the necessary execution context.

A computation with a nearly empty environmental requirement may realize directly on hardware primitives or a very small support layer.

A computation requiring filesystems, dynamic loading, networking, threading, clocks, entropy, and rich process isolation requires a correspondingly richer closure.

Both are the same language.

## Runtime synthesis

### Synthesis problem

Given:

```text
ProgramRequirements R
TargetProfile T
EnvironmentCapabilities E
CandidateProviders P
Policy Policy
Objectives O
```

find:

```text
RuntimeClosure C
```

such that:

```text
C + E on T satisfies R
```

and all compatibility, authority, resource, trust, and evidence obligations pass.

Among valid closures, Forge may optimize objectives such as:

- latency;
- throughput;
- memory;
- startup time;
- binary size;
- energy;
- authority surface;
- assumption debt;
- proof cost;
- recovery behavior;
- portability; or
- deployment simplicity.

### Runtime closure is a graph

```text
RuntimeClosure {
    root_requirements

    selected_providers
    provider_bindings
    adapters

    target_bindings
    authority_bindings
    resource_bindings

    residual_requirements
    realization_holes

    effect_closure
    authority_closure
    resource_closure
    assumption_closure

    obligations
    evidence
}
```

### Minimal closure

The synthesis process should prefer, subject to objectives and policy, not to include services that are semantically unnecessary.

This is not an absolute code-size minimization rule. A larger provider may be selected when it better satisfies reliability, performance, proof, compatibility, or maintenance goals.

The important point is that unused ambient services are not silently assumed.

## Target and runtime realization holes

### `TargetHole`

A semantic artifact may remain target-polymorphic:

```text
TargetHole {
    requirements
    candidate_domains
    resolution_stage
    objectives
    evidence_policy
}
```

RFC 0016 determines when such a hole may be resolved.

### `RuntimeHole`

A service requirement may remain unresolved:

```text
RuntimeHole<AllocatorContract>
```

until enough target/environment facts exist to select a provider.

### Late resolution

Resolution may occur at:

- compile time;
- link time;
- deployment time;
- load time; or
- runtime

when the resulting choice remains explicit and evidence-bearing.

A late choice must not retroactively justify an earlier transformation that required facts unavailable at that earlier stage.

## Multiversioning

A semantic operation may have several verified realizations:

```text
RealizationSet<SemanticOperation#42> {
    ScalarV1
    SIMD128V1
    SIMD256V1
    SIMD512V1
    GPUV1
}
```

Each member carries refinement evidence against the same semantic identity.

```text
ScalarV1 <= SemanticOperation#42
SIMD256V1 <= SemanticOperation#42
GPUV1 <= SemanticOperation#42
```

Runtime/load-time dispatch becomes selection among already justified realizations according to target evidence.

### Guarded selection

If a target feature is discovered dynamically, a dispatch guard must be part of the realization semantics.

The system must prove that:

- the guard correctly establishes the required target fact;
- the selected realization is valid under that fact;
- fallback behavior is valid when the fact is absent; and
- dispatch itself preserves required observations.

## Environment construction

Runtime initialization is semantic work and should not be hidden behind one startup convention.

Conceptually:

```text
EnvironmentConstruction {
    requested_environment

    target_resources
    authority_sources

    construction_plan
    initialization_steps

    resulting_environment

    cleanup_or_shutdown_plan?

    obligations
    evidence
}
```

Possible initialization actions include:

- allocating regions;
- establishing protection domains;
- mapping code/data;
- binding capabilities;
- configuring devices;
- creating queues;
- registering memory;
- initializing scheduler structures;
- starting reactors;
- loading certificates;
- installing dispatch tables; and
- establishing trust boundaries.

Those operations participate in ordinary effect, authority, failure, and recovery semantics.

## Environment satisfaction

The core relationship is conceptually:

```text
EnvironmentProvision
    satisfies
ExecutionRequirement
```

or:

```text
Gamma |- EnvironmentProvision <= ExecutionRequirement
```

where satisfaction includes all relevant dimensions.

A compatibility artifact should report dimensions independently:

```text
EnvironmentCompatibility {
    services: PASS
    context: PASS
    authority: PASS
    resources: PASS
    progress: PASS
    timing: PASS
    target: PASS
    representation: PASS
    assurance: PASS
    evidence: PASS
}
```

Any dimension may instead be `FAIL` or `UNKNOWN`.

`UNKNOWN` cannot be collapsed into `PASS` under a strict policy.

## Service realization

A semantic service contract should describe behavior, not merely an API shape.

For example:

```text
MonotonicClockRequirement {
    operation: ObserveTime
    monotonic: true
    resolution <= 1ms
    may_move_backward: false
    failure_contract: NeverFails
}
```

Candidate providers might include:

- architectural counter;
- kernel monotonic clock;
- hypervisor clock;
- WASI clock import;
- external timing service with adapter; or
- synthesized logical clock under suitable semantics.

Provider selection requires refinement of the required contract.

## Device and accelerator semantics

Devices should enter execution through the same requirement/provision model.

A program may require:

```text
ComputeAccelerator<CapabilitySet<C>>
```

rather than naming one specific GPU model.

Or it may deliberately require a specific device identity when necessary.

Device use must preserve:

- authority;
- memory/provenance constraints;
- DMA visibility/coherence requirements;
- concurrency/event semantics;
- failure/recovery semantics;
- target assumptions; and
- trust boundaries for drivers/firmware.

An accelerator being physically present does not grant discovery or use authority.

## Runtime and foreign boundaries

Many runtime providers will initially be foreign implementations:

- OS services;
- libc;
- kernel syscalls;
- device drivers;
- firmware;
- WebAssembly hosts;
- hypervisors;
- GPU runtimes;
- vendor libraries; and
- conventional language runtimes.

RFC 0015 applies normally.

A target/runtime provider does not become semantically trusted merely because it is necessary for execution.

Its assumptions and containment remain visible in the final runtime realization.

## Target-independent versus target-specific artifacts

MNCS should distinguish:

```text
SemanticArtifact
```

from:

```text
TargetRealization
```

The former describes required behavior.

The latter commits to target/environment facts.

Target-specific facts may enter only through explicit realization or staging transitions.

This preserves portability and prevents target assumptions from contaminating higher-level semantic identity.

## Representation constraints

Some environments impose representation constraints.

Examples include:

- alignment;
- address-space class;
- DMA registration;
- device-visible layout;
- endian requirements;
- vector-lane organization;
- ABI representation; and
- persistent-storage representation.

RFC 0009 and RFC 0013 representation semantics remain authoritative.

A target/runtime may select representation where polymorphism permits, but it may not change observable logical semantics without the appropriate refinement/approximation envelope.

## Deployment semantics

Deployment should be considered a realization stage rather than external orchestration folklore.

A deployment decision may bind:

- target hole;
- platform identity;
- machine instance;
- runtime closure;
- service providers;
- capability distribution;
- resource budgets;
- containment domains; and
- multiversion selections.

The deployment artifact should record those bindings and their evidence.

This direction is particularly useful for heterogeneous fleets in which one semantic workload may be eligible for several machines with different target/runtime profiles.

RFC 0017 does not define fleet scheduling policy in full, but it should make the information needed by such scheduling machine-readable.

## Architecture selection and deployment search

Given several compatible target environments, Forge or a Fabric-style scheduler may select among them under objectives.

For example:

```text
CandidateEnvironment A:
    higher performance
    larger authority surface
    lower proof assurance

CandidateEnvironment B:
    lower performance
    smaller authority surface
    stronger assurance
```

Both may satisfy hard requirements.

The choice belongs to machine intent and policy, not semantic compatibility alone.

## Resource-aware deployment rejection

A target must be rejected before execution when a hard resource requirement is proven incompatible.

Example:

```text
Program proven requirement:
    RAM >= 48 MiB

Environment hard limit:
    RAM <= 32 MiB
```

Result:

```text
FAIL(ResourceCompatibility)
```

An estimate alone should not be promoted into a proof of incompatibility unless policy explicitly permits conservative empirical criteria.

## Timing and progress requirements

Timing and progress requirements should remain explicit and evidence-sensitive.

Examples:

```text
Deadline<10ms>
WaitFree<OperationX>
FairScheduling<TaskClassA>
MayBlock<OperationY>
```

A runtime provider may satisfy them only with appropriate evidence and assumptions.

Hardware clocks, schedulers, kernels, hypervisors, and interrupt systems may all contribute assumptions.

RFC 0010 and RFC 0011 remain authoritative for progress and failure semantics.

## Assurance requirements

A program or deployment policy may require assurance properties such as:

```text
MemoryIsolation >= Certified
ControlFlowIntegrity >= RuntimeEnforced
AllocatorMemorySafety >= Proved
ForeignBoundaryContainment >= Required
```

These are semantic deployment constraints, not merely audit metadata.

A faster provider that fails the assurance requirement is not compatible.

## Evidence freshness and invalidation

Execution realization evidence must be dependency-tracked.

Potential evidence dependencies include:

- semantic artifact identity;
- target architecture model;
- target extension set;
- platform configuration;
- machine-instance inventory;
- firmware identity;
- hypervisor identity;
- kernel/runtime identity;
- runtime component versions/semantic identities;
- adapters;
- target backend;
- compiler/toolchain identity;
- measured feature probes; and
- assurance policy.

A change should invalidate only evidence that depends on the changed fact.

For example, changing an unrelated NIC should not invalidate arithmetic-lowering evidence unless that evidence actually depended on the NIC or platform identity containing it.

## Reproducibility

RFC 0014 distinguishes semantic, build, and artifact identity.

RFC 0017 extends this to environment realization.

A reproducible target realization claim should identify:

- semantic input artifact;
- target profile;
- environment profile;
- runtime closure;
- toolchain;
- configuration; and
- build/initialization recipe.

Reproducing bytes is not equivalent to proving target compatibility.

Target compatibility is not equivalent to reproducing bytes.

Both claims may be useful and should remain distinct.

## Canonical artifacts

RFC 0017 should eventually define deterministic, versioned schemas for artifacts such as:

### `ExecutionRequirement`

```text
ExecutionRequirement {
    identity

    semantic_services
    contextual_requirements

    authority_requirements
    resource_requirements

    progress_requirements
    timing_requirements

    target_constraints
    representation_constraints

    assurance_requirements

    preferences
    objectives

    assumptions
    evidence_dependencies
}
```

### `ArchitectureContract`

```text
ArchitectureContract {
    identity
    semantic_model_identity

    instruction_properties
    memory_model
    atomic_properties
    privilege_properties
    extensions

    assumptions
    evidence
}
```

### `PlatformContract`

```text
PlatformContract {
    identity
    architecture

    memory_regions
    devices
    timers
    interrupts
    DMA_properties
    protection_properties

    resource_bounds
    guarantees

    assumptions
    evidence
}
```

### `MachineInstanceProfile`

```text
MachineInstanceProfile {
    identity
    platform

    configuration
    enabled_features
    resources
    device_inventory

    validity
    evidence
}
```

### `ExecutionEnvironment`

```text
ExecutionEnvironment {
    identity

    target
    machine_instance?

    exposed_services
    contextual_facts

    granted_authority
    resource_limits

    guarantees
    assumptions

    evidence
}
```

### `RuntimeClosure`

```text
RuntimeClosure {
    root_requirements

    components
    bindings
    adapters

    authority_closure
    effect_closure
    resource_closure
    assumption_closure

    residual_requirements
    realization_holes

    obligations
    evidence
}
```

### `RuntimeRealization`

```text
RuntimeRealization {
    semantic_artifact
    execution_requirement

    architecture
    platform
    machine_instance?
    execution_environment

    runtime_closure
    target_realizations

    initialization_plan
    dispatch_plan

    compatibility
    obligations
    evidence

    semantic_identity
    realization_identity
}
```

### `TargetFact`

```text
TargetFact<P> {
    subject
    property: P
    value

    scope
    validity

    assumptions
    evidence
}
```

### `TargetHole`

```text
TargetHole {
    semantic_subject
    hard_requirements
    candidate_domain
    resolution_stage
    objectives
    evidence_policy
}
```

### `RealizationSet`

```text
RealizationSet {
    semantic_identity

    realizations[] {
        artifact
        target_condition
        refinement_evidence
    }

    dispatch_contract?
}
```

## Judgment families

The final formal notation remains open, but RFC 0017 should support judgment families conceptually equivalent to the following.

### Requirement derivation

```text
Gamma |- Program P requires EnvironmentRequirement R
```

### Target fact

```text
Evidence |- Target T provides Fact F
```

### Service provision

```text
Gamma |- Provider P <= ServiceRequirement S
```

### Environment satisfaction

```text
Gamma |- Environment E <= Requirement R
```

### Runtime closure validity

```text
Gamma |- RuntimeClosure C realizes R on T
```

### Resource compatibility

```text
Gamma |- ResourceProvision RP satisfies ResourceRequirement RR
```

### Target specialization

```text
Gamma ; TargetFacts TF |- SpecializedArtifact A' <= SemanticArtifact A
```

### Dispatch validity

```text
Gamma |- Dispatch D selects only valid members of RealizationSet RS
```

### Environment construction

```text
Gamma |- ConstructionPlan CP : InitialState -> Environment E
```

These judgments should generate evidence/obligations rather than merely booleans.

## HIR and SSA requirements

Execution-context semantics must survive lowering.

HIR should preserve or reference:

- semantic service requirements;
- target-independent context requirements;
- target facts consumed by specialization;
- runtime bindings;
- authority requirements;
- resource requirements;
- assumptions;
- realization identities; and
- evidence dependencies.

SSA and target IR may erase high-level structures only after required relationships have been discharged or represented through evidence links.

A backend may not silently introduce a runtime dependency absent from the recorded closure.

For example, lowering a pure arithmetic operation must not silently add heap allocation unless the selected realization explicitly introduces and satisfies that requirement.

## Backend contracts

Backends themselves have environment and target contracts.

A backend may claim support for:

```text
ArchitectureContract A
TargetSubset T
```

under assumptions.

Backend selection must therefore be evidence-bearing.

A backend name or compiler flag does not prove correctness for every target configuration.

Backend-generated target promises should be conservative.

## Runtime realization and Forge

Forge may treat runtime construction as a bounded search problem.

Candidate dimensions may include:

- allocator;
- scheduler;
- async mechanism;
- clock provider;
- storage provider;
- network provider;
- collector strategy;
- loader strategy;
- target architecture;
- target machine;
- vector width;
- accelerator choice;
- containment boundary; and
- stage at which choices are resolved.

Hard semantic requirements constrain the search.

RFC 0006 objectives rank valid candidates.

Proof/evidence cost itself may be an optimization dimension.

A candidate with insufficient evidence remains unpromotable under the active assurance policy regardless of benchmark performance.

## Machine-native runtime synthesis

A particularly important consequence of this RFC is that runtime construction can become a semantic synthesis task.

Instead of:

```text
compiler -> standard runtime -> OS
```

MNCS should be able to construct:

```text
semantic program
    -> requirement closure
    -> target/environment facts
    -> provider search
    -> runtime closure
    -> initialization/authority plan
    -> verified target realization
```

This architecture permits highly specialized runtimes without requiring semantic forks of the language.

## Bare-metal and near-zero-runtime realizations

RFC 0017 should intentionally permit artifacts with little or no conventional runtime.

If a program's semantic requirements can be directly realized by architectural/platform primitives, the runtime closure may be nearly empty.

This does not imply that bare-metal execution is always desirable or easy.

It establishes that MNCS semantics do not require unnecessary hosted abstractions.

## Conventional hosted realizations

Conventional operating systems should remain first-class practical targets.

Linux, Windows, BSD-like systems, WASI hosts, mobile systems, and other rich environments may supply broad service sets.

MNCS may select their services when they refine required contracts.

The language should not make those environments second-class merely because it also supports minimal realizations.

## Capability-oriented environments

Capability kernels and compartmentalized systems provide especially natural realizations because authority distributions can align closely with RFC 0008 capabilities.

RFC 0017 should permit generated environment manifests describing:

- objects/resources;
- protection domains;
- capability grants;
- service bindings; and
- initialization order.

A loader or environment constructor may then instantiate that state.

## WebAssembly-style environments

Component-world systems are useful targets because the host imports needed by a component are already structurally explicit.

MNCS should layer richer behavioral, authority, resource, failure, and assurance contracts on top of such target environments.

A Wasm import signature alone is not sufficient to prove semantic service compatibility.

## Fleet and heterogeneous execution

Although full fleet scheduling is outside this RFC, target/environment artifacts should be usable by Fabric-like systems.

A heterogeneous fleet node can advertise machine-readable:

- architecture;
- platform;
- resources;
- available service providers;
- model/accelerator resources where relevant;
- trust/assurance properties;
- containment mechanisms; and
- target evidence freshness.

A workload can advertise its `ExecutionRequirement`.

Placement then becomes compatibility plus objective optimization rather than hostname-specific scripting.

This direction should remain compatible with later distributed-system RFCs.

## Security consequences

### Least-authority runtime construction

The final realization should expose only required authority plus explicit infrastructure authority justified by selected providers.

### Hidden authority expansion is an error

If a runtime provider introduces `Network<Egress>`, that authority must appear in the closure.

A dependency cannot hide its privilege behind the label `runtime`.

### Target discovery is authority-bearing

Reading detailed machine topology, devices, firmware, or security state may itself reveal sensitive information.

Discovery interfaces therefore require explicit authority and may expose filtered views.

### Runtime compromise does not imply unlimited machine authority

RFC 0015 applies to runtime components.

Containment may limit a compromised provider to its granted authority.

### Initialization authority is temporary where possible

Environment construction may require greater authority than steady-state execution.

Such authority should be attenuated or revoked after initialization when the realization permits.

## Failure and recovery consequences

Environment construction and runtime services can fail.

RFC 0011 semantics apply.

Examples include:

- memory allocation failure;
- device initialization ambiguity;
- service startup timeout;
- partial queue registration;
- failed capability transfer;
- loader failure;
- plugin/runtime provider failure; and
- target-feature probe uncertainty.

A failed initialization step must not silently imply that every external effect was unapplied.

Recovery authority remains explicit.

## Determinism and target/environment choice

Target selection may introduce implementation nondeterminism.

If several realizations are semantically equivalent under the observation contract, the choice may remain implementation choice.

If different target realizations expose observably different permitted behavior, the semantic artifact must classify that nondeterminism or constrain selection.

Runtime synthesis must not broaden observable behavior outside the source realization envelope.

## Interaction with staging

RFC 0016 provides the phase structure for late target/runtime selection.

Target facts have stage availability.

For example:

```text
ArchitectureFact
    known at compile stage

MachineSIMDWidth
    known at load stage

CurrentFreeMemory
    known only at runtime
```

A transformation may depend only on facts available at its stage.

Cross-stage persistence rules apply to target evidence.

## Interaction with abstraction and representation polymorphism

RFC 0013 representation polymorphism allows target-specific representation selection without changing logical semantics.

A target realization may choose:

- scalar versus vector representation;
- packed versus unpacked layout;
- CPU versus GPU buffer layout;
- static versus dynamic memory placement; or
- region strategy

when the abstraction contract permits it.

Inspecting target representation breaks representation parametricity and must be explicit.

## Interaction with component composition

Runtime synthesis is component composition under additional target/environment constraints.

Service providers are ordinary RFC 0014 providers.

Bindings produce evidence.

Adapters are explicit.

Compatibility uses refinement.

Provider choice may remain a realization hole.

## Interaction with trust boundaries

Operating systems, kernels, hypervisors, device drivers, firmware, and existing runtimes frequently form RFC 0015 boundaries.

Their guarantees may be:

- proved;
- certified;
- runtime enforced;
- contained;
- assumed; or
- unknown.

Those facts become part of runtime realization evidence and assumption debt.

## Interaction with executable semantics

RFC 0012 remains the normative description of program behavior.

RFC 0017 does not create a second execution meaning.

It defines how the abstract semantic operations of RFC 0012 are supplied by a concrete target/environment/runtime graph.

Every target realization must preserve the source behavior/refinement contract.

## Flagship experiment: one semantic artifact, radically different environments

The central RFC 0017 experiment should define one nontrivial semantic program:

```text
SemanticProgram<P>
```

with requirements resembling:

```text
Allocate
Clock<Monotonic>
AsyncTasks
ByteStream<Input>
ByteStream<Output>
```

Then realize the same semantic identity through several substantially different environments, for example:

### Realization A: conventional Linux userspace

Possible providers:

- process memory / mmap-like allocation realization;
- kernel monotonic clock;
- epoll/io_uring/thread runtime candidate;
- ordinary file/socket byte streams.

### Realization B: WebAssembly/WASI-style component environment

Possible providers:

- host allocator/memory model;
- imported monotonic clock;
- component task/stream adapters;
- host byte-stream services.

### Realization C: minimal library-OS/unikernel-style closure

Only required service libraries are linked.

### Realization D: capability-oriented/seL4-like environment

Resources and authority are explicitly provisioned to service components and application components.

The test must establish:

```text
same semantic program identity
same required observable behavior
```

while allowing different:

- runtime closure;
- allocator;
- scheduling realization;
- syscall/device path;
- authority layout;
- initialization plan; and
- containment structure.

Each realization must produce evidence conceptually equivalent to:

```text
RuntimeRealizationX <= SemanticProgram<P>
```

The experiment is successful only if target/runtime-specific machinery remains out of the semantic program identity while all target assumptions remain explicit.

## Experimental tracks

### 1. Coeffect inference

Derive an execution requirement closure from semantic operations and component dependencies.

Verify that an unused filesystem/network service does not appear.

### 2. Minimal-runtime synthesis

Given several service providers, synthesize a runtime closure containing only the services required by the selected program realization.

### 3. Authority-environment construction

Generate a declarative capability/authority layout and construct the environment from it.

Verify steady-state authority is narrower than initialization authority where possible.

### 4. Allocator tournament

Provide several allocator realizations with different:

- target requirements;
- memory bounds;
- performance;
- trust/evidence; and
- failure semantics.

Use Forge objectives to select among valid candidates.

### 5. Scheduler tournament

Run the same RFC 0010 semantic concurrency contract through several scheduler realizations and verify progress/observation requirements.

### 6. Target-profile experiment

Realize the same semantic subset on two architectural contracts with different feature sets.

Ensure optimizations consume explicit target evidence.

### 7. Resource-bound rejection

Use a proven resource bound to reject a target before execution.

Compare against an empirical estimate that is insufficient under strict policy.

### 8. Late target-hole resolution

Keep target selection unresolved through an earlier stage and bind it at deployment using RFC 0016 stage rules.

### 9. Verified multiversion dispatch

Generate scalar and SIMD realizations of one semantic operation.

Select at load/runtime using target evidence and verify the dispatch guard.

### 10. Unknown-target-feature test

Deliberately remove evidence for a target property and verify that optimization authority disappears without changing semantic correctness.

### 11. Runtime least-authority test

Show that allocator, scheduler, logging, and network providers receive disjoint authority according to their contracts.

Attempt cross-service unauthorized access and reject it.

### 12. Environment evidence invalidation

Change one target/runtime configuration fact and verify that only dependent realization evidence becomes stale.

### 13. Near-bare-metal realization

Use a small semantic program whose requirements can be directly realized with platform primitives and demonstrate a near-zero conventional runtime closure.

### 14. Runtime synthesis comparison

Ask Forge to produce different valid runtime closures under:

- minimum memory;
- minimum latency;
- minimum authority surface; and
- maximum assurance

objectives.

### 15. Machine-instance versus environment test

Give a machine a device that the environment intentionally hides.

Verify `Exists<Device>` does not imply `Available`, `Discoverable`, or `Authorized`.

### 16. Foreign runtime provider experiment

Use a conventional foreign runtime provider through RFC 0015 and preserve its assumptions/containment in the final runtime realization rather than laundering it into trusted runtime state.

## Mandatory negative fixtures

The prototype should include negative fixtures that reject or conservatively classify at least the following cases.

1. A target name is treated as proof of an architectural extension.
2. A machine physically has a NIC and the program is implicitly granted network authority.
3. A target property is `UNKNOWN` but target-specific lowering assumes it is true.
4. A backend silently introduces heap allocation without updating execution requirements.
5. An allocator silently receives network authority.
6. A scheduler silently receives filesystem authority.
7. A logging runtime receives secret-region authority without contract justification.
8. A runtime provider imports a new effect but the final effect closure omits it.
9. A transitive runtime dependency adds authority without closure expansion.
10. A resource estimate is presented as a proved hard bound.
11. A target with insufficient proven memory is accepted under a hard resource requirement.
12. A target-hole is resolved using facts unavailable at the transformation stage that relied upon them.
13. SIMD lowering occurs before SIMD support evidence exists.
14. A load-time probe result is retroactively used to justify compile-time optimization evidence.
15. A multiversion dispatch can select a realization whose target condition is false.
16. A dispatch guard is unverified but treated as proof of target capability.
17. A target realization changes observable failure behavior outside the source envelope.
18. A runtime scheduler weakens required progress guarantees.
19. A runtime clock can move backward while satisfying a monotonic-clock requirement.
20. A runtime provider's failure semantics are ignored because its API type matches.
21. A machine-instance firmware update leaves stale firmware-dependent evidence valid.
22. An unrelated machine-instance change invalidates every proof instead of narrow dependent evidence.
23. A target architecture identity is conflated with a platform identity.
24. A platform identity is conflated with a machine-instance identity.
25. A machine-instance profile is treated as the execution environment without authority filtering.
26. Discoverability is treated as authority.
27. Service availability is treated as binding.
28. A capability binding exists but resource limits are incompatible.
29. A resource physically exists but is outside the environment's allowed region.
30. A foreign OS service is automatically marked trusted because it is part of the runtime.
31. A foreign driver is treated as preserving MNCS provenance without RFC 0015 evidence.
32. A runtime closure silently depends on a dynamic loader absent from requirements.
33. A near-bare-metal build receives a full hosted runtime by default without an explicit realization reason.
34. A hosted build is rejected merely because MNCS permits bare-metal realization.
35. GC is assumed mandatory for `ManagedLifetime` despite a valid ownership/region realization.
36. OS threads are assumed mandatory for concurrency despite a valid state-machine realization.
37. Runtime initialization performs an external effect but its failure/uncertainty is omitted.
38. Initialization authority persists into steady state without justification where attenuation was required.
39. Environment construction partially configures a device and failure is modeled as `NotApplied` without evidence.
40. A runtime provider violates a target representation constraint but links because function types match.
41. Device presence implies device configuration authority.
42. A GPU realization is selected without verifying memory-transfer/provenance requirements.
43. A DMA realization assumes coherence when coherence is `UNKNOWN`.
44. A proof tied to one kernel/hypervisor configuration is reused under another configuration without dependency validation.
45. Semantically equivalent target variants are given different logical program identities without a reason.
46. Observably different target variants are given one equivalence claim without refinement/approximation evidence.
47. A runtime synthesis candidate wins on performance despite failing a hard assurance requirement.
48. Assumption debt is omitted from the final runtime realization.
49. Benchmark evidence is treated as proof of a hard timing bound without policy.
50. Runtime service selection exceeds the transformation/search budget and silently falls back to an unverified provider.

## Acceptance criteria for the RFC direction

The direction should not advance beyond Draft/Proposed without bounded experiments demonstrating the following.

1. Execution requirements can be represented independently of a concrete runtime product.
2. Requirement closure can be derived for a supported semantic subset.
3. At least two different runtime providers can satisfy one service requirement through refinement.
4. Availability and authority are represented separately.
5. Architecture, platform, machine instance, environment, and runtime closure are distinct identities.
6. Target-specific optimization consumes explicit target evidence.
7. `UNKNOWN` target properties block unsupported specialization.
8. Quantitative resource requirements retain evidence classification.
9. Runtime providers appear in ordinary authority/effect/assumption closures.
10. A minimal runtime closure can omit semantically unused services.
11. At least one runtime choice can remain a realization hole until a later stage.
12. Multiversioned realizations retain shared semantic identity with separate refinement evidence.
13. Environment construction can be represented as an effectful, authority-bearing process.
14. Foreign runtime providers retain RFC 0015 trust-boundary status.
15. Narrow evidence invalidation works for at least one target/configuration change.
16. The flagship multi-environment experiment demonstrates one semantic program across substantially different runtime/environment realizations.

## Research implementation direction

A practical implementation path could proceed in bounded increments.

### Phase A: requirement artifacts

Add machine-readable execution requirement artifacts for the current supported semantic subset.

Do not build a general runtime synthesizer first.

### Phase B: target profiles

Define a small target-profile schema sufficient for existing prototype backends and experiments.

Start with facts actually used by transformations.

### Phase C: service provider contracts

Represent a small set of runtime services, likely allocator/clock/byte-stream/task scheduling, as RFC 0014 components.

### Phase D: closure solver

Construct a bounded provider-selection solver that returns:

- selected providers;
- residual requirements;
- authority/effect/resource closures;
- assumptions; and
- evidence obligations.

### Phase E: late target realization

Integrate one RFC 0016 target hole and multiversion dispatch experiment.

### Phase F: heterogeneous realizations

Build the flagship experiment across at least two radically different environments before attempting four.

### Phase G: runtime synthesis tournament

Allow Forge to compare several valid closures under machine-intent objectives.

## Relationship to the roadmap

RFC 0017 strengthens several existing roadmap requirements.

### 0.3 high-level IR

HIR can represent environment requirements, target constraints, and runtime service links rather than hard-coding one execution substrate.

### 0.5 backend experiment

Backend lowering can consume target contracts and emit evidence-bound promises.

### 0.6 self-description

The compiler can expose which target/runtime facts each transformation requires.

### 0.7 target realizations

This RFC supplies the semantic structure for multiple target-specific realizations, scalar/SIMD variants, and optimization regions.

### 1.0 threshold

A credible 1.0 research threshold should include:

- versioned target/environment contract schemas;
- conservative target-fact use;
- runtime requirement closure;
- bounded unsafe/foreign runtime boundaries;
- explicit runtime/provider identities;
- environment compatibility diagnostics;
- resource/authority closure; and
- documented limits of target/runtime assurance.

## Open questions

Important questions intentionally remain experimental.

### How rich should coeffects become?

The environment-requirement system must remain expressive enough for target/runtime contracts without becoming an unbounded dependent description of the entire world.

### Which target properties belong in semantics?

Expose only properties that correctness, compatibility, or selected optimization rules actually depend on.

### How much resource analysis belongs in the type system?

Some bounds may be types/grades. Others may be evidence artifacts derived by analyses.

### Should runtime closure identities be semantic identities?

The semantic program identity should remain independent of valid runtime choices. Runtime realization identity should record the chosen closure.

### How should dynamic resource availability interact with hard requirements?

Static capacity and transient availability are different. Later work may need runtime admission-control semantics.

### How should energy/power constraints be represented?

The framework should permit them but avoid pretending inaccurate empirical models are proofs.

### How should target attestation work?

Attestation may become an evidence source, especially for remote or confidential computing environments, but is not required by this RFC.

### How far should target discovery be sandboxed?

Detailed machine information may itself be privileged or sensitive.

### How should live runtime replacement work?

Deployment/upgrade/live replacement deserves separate treatment and should not be silently folded into initial environment realization.

## Deferred work

RFC 0017 intentionally defers:

- full distributed-system environment semantics;
- remote consistency/consensus/replication;
- live migration;
- rolling upgrade and hot replacement semantics;
- complete temporal ontology and real-time calculus;
- persistent-storage transaction semantics beyond existing effect/failure contracts;
- universal hardware-description modeling;
- Byzantine hardware/runtime assumptions;
- confidential-computing attestation protocols;
- full fleet placement/scheduling semantics;
- complete energy/power semantics; and
- a production universal runtime synthesizer.

## Relationship to prior RFCs

### RFC 0002 — evidence

Target facts, runtime compatibility, resource bounds, and environment satisfaction carry explicit evidence and freshness dependencies.

### RFC 0004 — refinement governance

Runtime/target alternatives are candidates subject to bounded search, comparison, and promotion rather than silent self-modification.

### RFC 0006 — machine intent

Objectives rank valid target/runtime candidates after hard semantic constraints are satisfied.

### RFC 0008 — effects/resources/authority

Execution environments provision the semantic services, resources, and authority required by effects.

### RFC 0009 — memory/provenance/storage

Memory management, placement, representation, DMA, and storage mechanisms remain target/runtime realizations under explicit provenance constraints.

### RFC 0010 — concurrency

Schedulers and synchronization mechanisms realize semantic causality/progress contracts.

### RFC 0011 — failure/recovery

Runtime and initialization failure retain explicit effect state, uncertainty, retry authority, and recovery semantics.

### RFC 0012 — executable semantics

The target/runtime realizes the normative semantic machine; it does not redefine execution.

### RFC 0013 — polymorphism and realization holes

Target/runtime choices can remain polymorphic/open until enough facts exist to specialize safely.

### RFC 0014 — components

Runtime services are provider contracts and closures are evidence-bearing component graphs.

### RFC 0015 — trust boundaries

Foreign kernels, OSes, drivers, firmware, runtimes, and host services retain explicit trust-boundary semantics.

### RFC 0016 — staging

Target facts and runtime choices may become available at different phases, with explicit stage-scoped evidence and late realization.

## Central thesis

The complete direction of RFC 0017 can be summarized as:

> **MNCS programs should describe the semantic environment they require, not the conventional software stack they happen to run on. Architectures, platforms, machine instances, execution environments, and runtime components expose evidence-bearing contracts. The system may then construct, select, or synthesize the smallest valid realization that satisfies those contracts while respecting authority, resources, failure semantics, target evidence, assurance policy, and machine-intent objectives. Runtime mechanisms remain replaceable realizations of one portable executable semantics.**
