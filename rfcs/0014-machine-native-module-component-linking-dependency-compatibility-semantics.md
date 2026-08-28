# RFC 0014: Machine-Native Module, Component, Linking, Dependency, and Compatibility Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.6–1.0
- **Depends on:** RFC 0001, RFC 0002, RFC 0003, RFC 0004, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0013

## Summary

MNCS Language should define software composition as **evidence-bearing binding between separately checkable semantic contracts**, not primarily as textual imports, package-name resolution, linker symbol matching, or version-number comparison.

The central rule is:

> **Composition binds semantic requirements to semantic providers. Names, paths, repositories, package labels, version strings, linker symbols, files, and distribution formats may identify or transport candidates, but only semantic contract satisfaction establishes compatibility.**

RFC 0014 distinguishes four primary objects that conventional ecosystems often collapse together:

```text
Module
Component
Unit
Package
```

A **Module** is a semantic namespace and abstraction structure.

A **Component** is a separately checkable, potentially open semantic provider/consumer contract with explicit requirements and provisions.

A **Unit** is a component or component graph with a particular set of requirement bindings, adapters, evidence, residual holes, and composition identity.

A **Package** is an immutable distribution/build closure containing selected modules, components, units, source or semantic artifacts, build recipes, target/toolchain identities, evidence, and realized artifacts.

These categories must not be treated as synonyms.

The proposal synthesizes complementary ideas from:

- ML and System-Fω-oriented module calculi;
- MixML-style open components, recursive linking, and explicit definedness;
- Backpack-style signatures, units, separate verification, and later instantiation;
- assume/guarantee component contracts;
- I/O automata and behavioral interface theories;
- refinement and simulation as compatibility/substitution relations;
- generative and applicative module semantics;
- linking types and explicit semantic-boundary crossing;
- content-addressed and functional package/build models;
- RFC 0007 proof/evidence composition;
- RFC 0008 authority/resource closure;
- RFC 0012 executable/trace refinement; and
- RFC 0013 realization holes and evidence-bearing specialization.

The intended result is a composition model in which machines can inspect exactly:

- what a component requires;
- what it provides;
- what authority and effects each binding introduces;
- which assumptions are discharged by provider guarantees;
- which obligations remain unresolved;
- which adapters mediate mismatches;
- which identity relationships are generative or applicative;
- whether a provider actually refines a requirement;
- what evidence justifies the binding;
- which client evidence depends only on an interface versus an implementation;
- what artifact/build/toolchain identities realize the semantic unit; and
- which compatible provider choices remain open to Forge optimization.

## Constitutional principles

RFC 0014 establishes five initial principles.

### 1. Names are not compatibility

> **A dependency name, package version, symbol, path, repository, or nominal label identifies a candidate or discovery route; it does not prove that the candidate satisfies a semantic requirement.**

### 2. Linking does not erase boundaries

> **Component composition preserves authority, effects, assumptions, evidence, protocol obligations, memory/resource constraints, concurrency semantics, and failure/recovery behavior across the link graph.**

### 3. Interfaces describe minimum dependency

> **A component may rely only on semantic properties exposed by the interface or requirement against which it was verified.**

### 4. Binding is evidence-bearing refinement

> **A provider may fill a requirement only when its provision refines that requirement under the active assurance policy, with explicit evidence or an explicitly unresolved obligation.**

### 5. Dependency choice is a realization problem

> **Where multiple compatible providers exist, MNCS may preserve the choice as a bounded realization hole and select among providers using target facts, machine intent, authority limits, evidence, and optimization objectives.**

These principles continue the recurring MNCS distinction between semantic requirements and physical or ecosystem realizations.

## Motivation

### Profile 0.6 implementation slice

The first bounded implementation slice is now exercised in the reference
compiler. A host-provided `ModuleResolver` performs deterministic,
acyclic, transitive elaboration-time linking. Missing modules, duplicate
module identities, import collisions, and resolver payloads whose declared
module is incompatible with the requested name fail closed with diagnostics;
an optional final `.vN` version tail is accepted only as a discovery alias,
not as version selection.

The linked `Program` records sorted, deduplicated dependency identities.
Imported functions retain their declaring namespace through semantic identity,
graph, obligations, HIR, SSA, evidence, and the body/SSA reference executors.
The CLI's source-study report exposes requested name, declared name, module
identity, source identity, source logical name, and semantic fingerprint for
each resolved module. `examples/source/library-consumer.mncs` is the bounded
external-consumer witness, and `crates/mncs-compiler/tests/module_imports.rs`
and `crates/mncs-cli/tests/library_resolution.rs` provide the focused checks.

This slice intentionally does not claim qualified calls, selective or renamed
imports, provider contracts, semantic requirement satisfaction, package
selection, version-range resolution, or dynamic bindings. Those remain RFC
0014 work beyond the Profile 0.6 discovery/linking boundary.

Conventional module, package, and linker systems answer several different questions using overlapping mechanisms:

```text
Where is this name defined?
Which file should be compiled?
Which package version should be installed?
Which symbol should satisfy this relocation?
Can this implementation actually replace that one?
What authority does this dependency require?
Which behavioral assumptions does the consumer make?
Which proof obligations remain valid after replacement?
What exact artifact was built?
Can the build be reproduced?
```

The first four questions are mostly discovery and realization questions. The remaining questions are semantic.

A machine-native language should not infer semantic compatibility from ecosystem conventions when the language already carries contracts, effects, capabilities, memory semantics, protocols, failure semantics, evidence, and refinement relationships explicitly.

For example, two functions may have the same ordinary type:

```text
read : Key -> Value
```

while differing materially in semantic contract:

```text
Provider A:
    effects = { Storage }
    authority = { Read<LocalStore> }
    failure = { NotFound, Corrupt }
    progress = bounded_wait(5ms)

Provider B:
    effects = { Storage, Network }
    authority = { Read<RemoteStore>, Network<Egress> }
    failure = { NotFound, Timeout, AmbiguousCommit }
    progress = may_block
```

If a consumer was verified under a contract forbidding network authority, Provider B must not become compatible merely because symbol names and value-level types line up.

Likewise, a package version such as:

```text
storage-2.3.1
```

is useful human metadata, but it is not a semantic theorem about compatibility.

MNCS should be able to ask the stronger question:

```text
ProviderContract ⊑ RequiredContract ?
```

and produce:

```text
PASS
FAIL
UNKNOWN
```

with dimension-specific evidence and diagnostics.

## Design goals

RFC 0014 should make the following possible.

1. **Separate semantic modules from files and packages.** A module must be an abstraction/namespace object, not a required filesystem unit.
2. **Represent open components.** Components may be separately verified while some requirements remain unresolved.
3. **Represent concrete units.** A particular set of requirement bindings must have explicit identity and evidence.
4. **Preserve package/build identity separately.** Distribution, build, target, and artifact identity must not be conflated with semantic identity.
5. **Support separate verification.** A client verified only against an interface should not depend unnecessarily on provider implementation identity.
6. **Use refinement for compatibility.** Providers may be stronger than requirements without requiring contract equality.
7. **Expose multidimensional compatibility.** Type, effects, authority, protocol, behavior, failure, memory, resource, target, and evidence dimensions must remain distinguishable.
8. **Compose assumptions and guarantees.** Provider guarantees should discharge matching consumer assumptions where justified.
9. **Preserve authority boundaries.** Name visibility or package dependency must never implicitly grant authority.
10. **Compute link closures.** Final units should expose transitive effect, authority, assumption, target, and unresolved-obligation closures.
11. **Represent adapters explicitly.** Protocol, representation, version, FFI, or effect conversion must be a semantic component with obligations and evidence, not invisible linker glue.
12. **Support generative and applicative identity.** Component instantiation should explicitly determine whether abstract identities are fresh or reusable.
13. **Support bounded recursive linking.** Cyclic components must have explicit definedness/initialization/fixed-point semantics rather than relying on accidental initialization order.
14. **Preserve unresolved choices.** Compatible provider choices may remain open across compile/link/deploy phases when explicitly represented.
15. **Support dynamic semantic binding.** Runtime plugins/providers should be checked against the same contract model before authority is granted.
16. **Support machine-searchable dependency resolution.** Provider selection should be constraint solving under requirements, preferences, evidence policy, and target facts.
17. **Preserve reproducibility claims accurately.** Content identity, build derivation, semantic correspondence, and reproducibility must remain separate claims.
18. **Enable semantic compatibility analysis.** Version labels may remain, but compatibility should be derivable from contract/refinement deltas where possible.
19. **Integrate with recursive evidence invalidation.** Changes should invalidate only dependent client facts and binding evidence.
20. **Remain independently implementable.** The composition model must be precise enough for independent resolvers/linkers/verifiers to agree on supported cases.

## Non-goals

This RFC does not:

- choose one package registry;
- require Git, GitHub, Nix, Cargo, npm, or another ecosystem;
- select final surface syntax for imports or package manifests;
- make semantic versioning invalid or useless as human policy metadata;
- require every component to be dynamically loadable;
- require all recursive component graphs to be accepted;
- claim arbitrary cross-language code is safe merely because an adapter exists;
- define the complete FFI or ABI model; RFC 0015 is expected to deepen that boundary;
- require all artifacts to be reproducible on every host;
- make content hashes proof of source-to-binary correspondence;
- require dependency selection to maximize performance;
- require all compatible providers to be interchangeable under every objective;
- make package-manager resolution part of RFC 0007's proof kernel;
- allow unresolved component requirements to execute without a valid runtime binding; or
- allow `UNKNOWN` compatibility to silently become `PASS`.

Unsupported or unresolved composition facts remain explicit.

## Semantic object taxonomy

### Module

A module is a semantic namespace and abstraction unit.

Conceptually:

```text
Module {
    identity

    provided_values
    provided_types
    provided_abstractions

    required_values
    required_types
    required_evidence

    hidden_structure
    visibility
    semantic_dependencies
}
```

A module may correspond to one source file, many source files, generated semantic artifacts, or no human-authored source file at all.

Therefore:

```text
ModuleIdentity != FileIdentity
```

and:

```text
ModuleIdentity != PackageIdentity
```

### Component

A component is a separately checkable open composition object.

Conceptually:

```text
Component {
    identity

    modules

    requires {
        interfaces
        types
        operations
        effects
        authority
        resources
        memory_properties
        protocols
        concurrency_properties
        failure_properties
        assumptions
        evidence
        target_properties
    }

    provides {
        interfaces
        types
        operations
        effects
        authority_contracts
        guarantees
        protocols
        evidence
    }

    hidden_structure

    realization_holes

    initialization_contract
    compatibility_policy
    evidence_manifest
}
```

A component may be open:

```text
Component<Open>
```

when requirements remain unbound.

A component is closed only when every execution-relevant requirement is legally bound or otherwise discharged:

```text
Component<Closed>
```

Open components may still be typechecked, analyzed, proven, optimized within their declared assumptions, and distributed as semantic artifacts.

### Unit

A unit records a specific instantiation/linkage of one or more components.

Conceptually:

```text
Unit {
    identity

    components
    bindings
    adapters

    residual_requirements

    effect_closure
    authority_closure
    assumption_closure
    target_closure

    generated_obligations
    evidence

    initialization_plan
    realization_holes
}
```

Two units instantiated from the same component with different bindings are distinct units unless their declared applicative identity semantics establish equivalence.

### Package

A package is an immutable distribution/build closure.

Conceptually:

```text
Package {
    package_identity

    human_metadata {
        name
        version_label
        authorship?
        description?
    }

    modules
    components
    units

    semantic_artifacts
    source_artifacts

    dependency_closure
    build_recipe
    toolchain_requirements
    target_requirements

    evidence_manifests
    realized_artifacts
}
```

A package is not itself a proof that its contents are semantically compatible with a consumer.

## Selected theoretical synthesis

### ML and Fω-oriented module calculus

ML-style module theory provides a strong foundation for signatures, abstract types, hiding, higher-order modules, sharing constraints, generativity, and separate compilation.

RFC 0013 already establishes the direction that rich abstraction mechanisms should elaborate toward a relatively small explicit polymorphic/evidence-bearing core rather than becoming independent semantic islands.

RFC 0014 should apply the same principle at module scale:

```text
module syntax / component syntax
        ↓ elaboration
explicit abstraction + requirement/provision artifacts
```

A module declaration is therefore not trusted magic. Its abstract types, evidence parameters, hidden representation, and provided operations should be explicit in canonical semantics.

### MixML-style open components and definedness

Open components are a central requirement.

A component should be able to declare:

```text
requires {
    Storage : StorageContract
}
```

without selecting a concrete `Storage` implementation.

The unresolved requirement is semantically similar to a component-scale RFC 0013 realization hole:

```text
ComponentRequirementHole<StorageContract>
```

Binding:

```text
Storage <- ProviderP
```

is legal only if ProviderP satisfies the requirement.

MixML-style definedness also provides useful inspiration for recursive linking. A required name may be declared before its definition is supplied, but the system must track whether execution can observe it before it becomes defined.

MNCS can model definedness as an explicit property/resource rather than relying on source order.

### Backpack-style signatures and units

A component should be verifiable against a signature/requirement interface independently of the implementation eventually supplied.

Example:

```text
component ImageProcessor {
    requires {
        Decoder : ImageDecoder<JPEG>
        Storage : BlobStorage
    }

    provides {
        ResizeService
    }
}
```

The component can be verified while `Decoder` and `Storage` remain open.

Later:

```text
unit U72 {
    component = ImageProcessor

    bindings {
        Decoder <- SIMDDecoder#42
        Storage <- NVMeStorage#81
    }
}
```

creates a distinct unit identity.

This enables actual separate verification rather than merely separate parsing/compilation.

### Assume/guarantee contracts

Component-scale composition should use assume/guarantee reasoning.

A consumer may state:

```text
assume {
    acknowledged_write_is_durable
    clock_is_monotonic
}
```

A provider may state:

```text
guarantee {
    acknowledged_write_is_durable
    clock_is_monotonic
}
```

Binding can discharge assumptions only when the guarantee matches the same proposition, scope, identity, freshness, and required evidence class.

Composition should generate obligations such as:

```text
Guarantees(Provider) satisfies Assumptions(Consumer)
```

Unmatched assumptions remain explicit in the unit closure.

### I/O automata and behavioral interfaces

Operation type compatibility is insufficient for stateful or protocol-sensitive components.

RFC 0014 should adopt the useful distinction between:

```text
accepted/input actions
provided/output actions
internal actions
```

For example:

```text
ComponentBehavior {
    inputs {
        Request
        Cancellation
    }

    outputs {
        Response
        Failure
    }

    internal {
        CacheUpdate
        Retry
    }
}
```

RFC 0008 protocol/session semantics and RFC 0010 event/causality semantics provide the deeper behavioral rules.

Behavioral compatibility may therefore require trace/refinement evidence, not merely matching operation names.

### Refinement and simulation

Compatibility is primarily a refinement relation.

Conceptually:

```text
Γ ⊢ ProviderContract ⊑ RequirementContract
```

A provider may guarantee more than a consumer requires.

For example:

```text
Requirement:
    acknowledged writes are durable

Provider:
    acknowledged writes are durable
    data encrypted at rest
```

may be compatible if the extra guarantee does not introduce forbidden effects, authority, representation commitments, failure behavior, or other observable obligations.

The exact variance/refinement rules are dimension-specific.

RFC 0012's simulation/refinement machinery should be reused for behavioral dimensions where executable traces matter.

### Generative and applicative instantiation

Component construction must explicitly distinguish generative from applicative identity behavior.

An applicative instantiation may permit:

```text
instantiate F<A>
instantiate F<A>
```

to share some abstract identities when the same semantic arguments and evidence are supplied.

A generative instantiation requires fresh identities:

```text
instantiate SecureDomain<>
instantiate SecureDomain<>
```

produces:

```text
Domain#1 != Domain#2
```

This is important for:

- cryptographic/security domains;
- protocol sessions;
- address spaces;
- capability namespaces;
- isolated resource domains;
- nominal abstract state; and
- other places where accidental equality would violate isolation.

The generative/applicative choice must be semantic and explicit.

### Linking types and boundary changes

A link may expose a component to behaviors that were not expressible under the environment in which it was verified.

Therefore:

> **A link may not silently enlarge the semantic universe of a component.**

If a component assumes all references obey RFC 0009 provenance and another component is foreign or unsafe code capable of violating that assumption, the boundary requires an explicit adapter/foreign contract.

Matching ordinary input/output types is insufficient.

RFC 0015 should deepen the full foreign/unsafe model; RFC 0014 establishes the general rule that semantic boundary expansion must be explicit.

### Content-addressed and functional package/build models

Packaging and builds need precise identities.

Content addressing is useful for immutable artifact identification and dependency closure, but a content hash does not prove source-to-binary correspondence or semantic compatibility.

MNCS should therefore distinguish content identity from semantic and build claims.

## Requirement and provision model

An operation requirement should be able to carry all relevant semantic dimensions.

Conceptually:

```text
OperationRequirement {
    identity
    type

    effects
    authority

    memory_contract
    resource_contract

    concurrency_contract
    progress_contract

    failure_contract
    recovery_contract

    determinism_contract
    protocol_contract

    representation_constraints
    target_constraints

    assumptions
    required_evidence
}
```

A provider carries:

```text
OperationProvision {
    identity
    operation

    type
    effects
    authority

    memory_guarantees
    resource_guarantees

    concurrency_guarantees
    progress_guarantees

    failure_guarantees
    recovery_guarantees

    determinism_guarantees
    protocol_guarantees

    representation_properties
    target_properties

    guarantees
    evidence
}
```

These are conceptual shapes, not final serialized schemas.

## Multidimensional compatibility

A composition checker should not return a single opaque boolean when more precise information is available.

Conceptually:

```text
CompatibilityAssessment {
    type
    abstraction
    behavior
    effects
    authority
    memory
    resources
    protocol
    concurrency
    progress
    failure
    recovery
    determinism
    representation
    target
    evidence
}
```

Each dimension may be:

```text
PASS
FAIL
UNKNOWN
```

with evidence and explanation.

A provider is promotable into a requirement only when the active assurance policy accepts every required dimension.

`UNKNOWN` must not be silently treated as compatibility.

### Example diagnostic

```text
Binding rejected

Requirement:
    Storage#11

Candidate provider:
    FastStore#92

PASS:
    type
    persistence
    protocol
    failure

FAIL:
    authority

Reason:
    provider requires Network<Egress>
    consumer envelope forbids Network

Candidate repair classes:
    choose local provider
    introduce authority-attentuating proxy if semantics permit
    widen consumer authority contract with explicit approval
```

Diagnostics should be machine-readable and suitable for Forge search/refinement.

## Binding semantics

A binding is itself a semantic artifact.

```text
Binding {
    identity

    requirement
    provider

    substitutions
    adapters

    authority_transfer
    capability_attenuation

    assumption_discharge

    compatibility_obligations
    compatibility_evidence

    residual_unknowns
}
```

A binding must not become invisible after linking.

It contributes to unit identity, evidence dependency, authority/effect closure, and later invalidation.

## Adapters are explicit components

Automatic glue may be useful, but invisible glue is not acceptable canonical semantics.

If:

```text
Consumer requires CelsiusSensor
```

and:

```text
Provider supplies FahrenheitSensor
```

an adapter should exist explicitly:

```text
Adapter {
    input_contract = FahrenheitSensor
    output_contract = CelsiusSensor

    transformation
    effects
    authority
    obligations
    evidence
}
```

Then the semantic graph is:

```text
Provider
    -> Adapter
    -> Consumer
```

The same rule applies to:

- representation conversion;
- protocol adaptation;
- version compatibility shims;
- effect translation;
- authority attenuation;
- serialization/marshaling;
- FFI boundaries; and
- target-specific shims.

An adapter may itself fail compatibility if it widens authority or weakens semantics beyond the consumer requirement.

## Recursive linking and definedness

Recursive component graphs require explicit semantics.

Example:

```text
A requires B
B requires A
```

This is not automatically illegal and not automatically legal.

A strongly connected recursive link group should carry something like:

```text
RecursiveLinkGroup {
    components

    shared_contracts
    definedness_state

    initialization_protocol
    fixed_point_semantics

    guardedness_requirements
    termination_requirements?

    obligations
    evidence
}
```

Potential legal cases include:

- type-only recursive relationships;
- protocol-level mutually recursive interfaces;
- guarded lazy values;
- explicitly fixed-point-defined services; and
- mutually initialized components with proven safe initialization dependencies.

Potential illegal cases include:

- observation of a value before definition;
- authority use before capability establishment;
- circular proof assumptions with no grounded evidence;
- unguarded recursion where a required total initialization contract applies; and
- initialization cycles that can expose invalid resource state.

Definedness should be a tracked semantic fact, not a comment about initialization order.

## Namespace visibility and authority are separate

This distinction is constitutional.

A component may be able to reference another component without possessing authority to use its protected effects/resources.

Therefore:

```text
CanReference<X>
```

is distinct from:

```text
CanUseAuthority<A>
```

and:

```text
import Database
```

must never imply:

```text
Write<Database>
```

or:

```text
Network<DatabaseHost>
```

Imports establish semantic reference/name availability.

Capabilities and explicit authority-transfer bindings establish authority.

## Link closures

A unit graph should expose conservative closures over its selected composition.

At minimum:

```text
RequiredAuthority(Unit)
RequiredEffects(Unit)
Assumptions(Unit)
RequiredTargets(Unit)
UnresolvedObligations(Unit)
```

Conceptually:

```text
RequiredAuthority(Unit) =
    closure(
        component_authority,
        adapter_authority,
        runtime_requirements,
        selected_provider_requirements
    )
```

This makes transitive privilege expansion visible.

For example, a program intended to be offline should fail a no-network policy if a transitive provider introduces:

```text
Network<Egress>
```

regardless of whether the top-level source mentions networking.

The same closure mechanism can drive:

- Fabric sandbox generation;
- deployment manifests;
- security review;
- target capability selection;
- runtime admission checks; and
- proof/evidence scope.

## Identity model

One hash or version string cannot answer every identity question.

RFC 0014 should distinguish at least the following layers.

### Interface identity

```text
InterfaceIdentity
```

Identifies the semantic requirements/provisions visible to clients.

### Component semantic identity

```text
ComponentSemanticIdentity
```

Identifies the semantic implementation and hidden structure of the component.

### Instantiation/unit identity

```text
InstantiationIdentity
```

Identifies how component holes were bound, including generative identities and selected evidence.

### Package identity

```text
PackageIdentity
```

Identifies an immutable distribution closure and its declared metadata/dependency structure.

### Build identity

```text
BuildIdentity
```

Identifies build recipe, target, compiler/backend, features, pass pipeline, environmental assumptions, and other build-relevant inputs.

### Artifact identity

```text
ArtifactIdentity
```

Identifies exact realized bytes or another exact deployment artifact representation.

### Evidence identity

```text
EvidenceIdentity
```

Identifies proof/certificate/verifier/empirical artifacts and their subjects/scopes/dependencies.

These identities are related but not interchangeable.

A human version label such as:

```text
2.1.0
```

is metadata, not any of the identities above.

## Reproducibility and artifact correspondence

RFC 0014 should distinguish at least:

```text
ContentIdentical<A,B>
```

from:

```text
ReproducibleUnder<BuildEnvironment>
```

from:

```text
ArtifactCorrespondsTo<SemanticIdentity>
```

and:

```text
ArtifactRefines<SemanticContract>
```

A content hash establishes content identity.

It does not by itself establish that the content came from the claimed source semantics or that a compiler lowering was correct.

Those stronger statements require independent evidence under RFC 0007/0012.

## Dependency resolution as constraint solving

Traditional package resolution often uses name/version constraints:

```text
foo >= 2.0 && < 3.0
```

MNCS should permit richer requirements.

Example:

```text
requires StorageProvider {
    semantics:
        DurableKV

    authority:
        forbid Network

    target:
        linux-x86_64

    resources:
        memory <= 128 MiB

    requirements:
        CrashAtomic

    preferences:
        p99_latency < 2 ms
        minimize proof_cost

    evidence_policy:
        require verifier PASS for crash atomicity
}
```

Resolution becomes:

```text
find Provider
such that
    Provider ⊑ Requirement
```

plus RFC 0006 optimization objectives among compatible candidates.

A dependency is therefore often a component-scale realization hole.

## Semantic versioning and compatibility

Semantic version labels remain useful for communication and ecosystem policy.

They must not be the canonical compatibility theorem.

A provider with a major version difference may still formally satisfy the exact contract a client requires.

A patch version may still violate a client contract if it widens effects, authority, failure modes, or observable behavior.

Therefore the canonical compatibility question is:

```text
NewProvider ⊑ RequiredContract ?
```

### Machine-derived compatibility classification

Given:

```text
old_contract
new_contract
```

an analyzer should eventually be able to classify:

```text
backward_compatible = PASS | FAIL | UNKNOWN
forward_compatible = PASS | FAIL | UNKNOWN
```

for a specified client contract or interface population.

Human SemVer labels may then be generated or checked against those semantic results.

This does not require universal decidability.

## Evidence composition through the link graph

Each component may carry an evidence manifest.

Linking must not simply union those manifests and label the final unit “verified.”

Instead:

```text
LinkEvidence {
    component_evidence
    binding_evidence
    adapter_evidence
    compatibility_evidence

    discharged_assumptions
    retained_assumptions

    generated_obligations
    unresolved_obligations
}
```

Linking can generate new proof obligations.

Example:

```text
Consumer assumes X
Provider guarantees X
```

may discharge X if evidence scope/freshness/identity matches.

But:

```text
Consumer assumes Y
Provider does not guarantee Y
```

must leave Y unresolved.

A component's proof cannot silently prove a stronger statement about the linked whole than its scope permits.

## Separate verification and narrow invalidation

This is a primary machine-native benefit.

Suppose component B is verified only against:

```text
Interface(A)#19
```

Provider A changes internally:

```text
Implementation A1 -> A2
```

but the interface contract and all interface evidence identities relevant to B remain unchanged.

Then B should not necessarily require:

```text
re-typecheck
re-prove
re-analyze
```

Evidence invalidation should follow the semantic dependency graph.

This supports significantly narrower invalidation than package-level or artifact-level rebuild rules.

A change to private implementation facts may invalidate:

```text
ComponentSemanticIdentity(A)
BuildIdentity(A)
ArtifactIdentity(A)
```

while preserving:

```text
InterfaceIdentity(A)
```

and client evidence that depends only on the interface.

This behavior should be tested explicitly.

## Partial linking and binding stages

A unit does not need to bind every compatible realization hole at the earliest possible phase.

Binding stages may include:

```text
design time
compile time
link time
deployment time
load time
runtime
```

The exact phase semantics will interact with RFC 0016 staging.

RFC 0014 establishes that residual requirements may remain explicit:

```text
PartiallyLinkedUnit {
    Storage = NVMeStore
    Scheduler = ?
    Serializer = BinaryV4
}
```

Such a unit must not execute an operation whose unresolved requirement is necessary for that execution unless a legal binding is first established.

Residual holes remain part of unit identity and deployment requirements.

## Dynamic binding

Dynamic/plugin loading is runtime semantic binding, not a type-safety escape hatch.

A dynamically loaded provider may carry:

```text
ComponentManifest
InterfaceIdentity
EvidenceManifest
TargetIdentity
```

The runtime can check:

```text
Provider ⊑ Requirement
```

before granting authority or making the provider reachable.

Possible outcomes include:

```text
PASS
FAIL
UNKNOWN
```

An active assurance policy determines whether `UNKNOWN` permits a runtime guard, sandboxed fallback, restricted capability grant, or rejection.

Dynamic binding must never grant authority merely because a shared-library symbol exists.

## Forge-driven provider selection

Provider selection is a natural Forge search problem.

Suppose several providers satisfy:

```text
StorageContract
```

All may be semantically valid while differing in:

```text
latency
throughput
memory
energy
binary size
startup cost
proof cost
evidence freshness
target availability
authority surface
failure behavior
recovery time
```

RFC 0014 should allow:

```text
DependencyHole<StorageContract>
```

with:

```text
requirements { ... }
preferences { ... }
budgets { ... }
```

Forge can evaluate candidates and propose a binding.

Promotion requires ordinary compatibility/refinement evidence.

The search process is not itself proof.

This turns dependency resolution into a form of **verified architecture synthesis** rather than merely “select the newest version.”

## Canonical composition artifacts

The eventual canonical representation should include stable, versioned artifacts for at least:

```text
ModuleContract
ComponentContract
ComponentRequirement
ComponentProvision
Binding
Adapter
RecursiveLinkGroup
UnitGraph
CompatibilityAssessment
LinkEvidence
PackageManifest
BuildIdentity
ArtifactManifest
DynamicBindingRequest
DynamicBindingResult
```

These artifacts should support deterministic serialization and semantic identities consistent with RFC 0002/0004.

## Formal judgments to investigate

The exact calculus remains open, but RFC 0014 should investigate judgments in the following families.

### Provision refinement

```text
Γ ⊢ P ⊑ R
```

Provider `P` satisfies requirement `R`.

### Binding validity

```text
Γ ; A ; E ⊢ bind(R <- P) ✓
```

Binding is valid under authority/effect/environment context.

### Unit closure

```text
Γ ⊢ link(C1,...,Cn,B1,...,Bm) => U
```

Linking components and bindings produces a unit plus generated obligations.

### Assumption discharge

```text
Γ ⊢ Guarantee(P,X) discharges Assumption(C,X)
```

subject to identity/scope/evidence checks.

### Substitutability

```text
Γ ⊢ P2 refines P1 for Interface I
```

so clients relying only on I may preserve applicable evidence.

### Recursive-link validity

```text
Γ ⊢ μ Components : ValidRecursiveUnit
```

under definedness/guardedness/initialization constraints.

### Package realization

```text
SemanticUnit ; BuildInputs
    -- build -->
Artifact
```

with a distinct evidence relation for claimed correspondence/refinement.

## Interaction with earlier RFCs

### RFC 0002 — contracts and evidence

All requirements, provisions, binding evidence, compatibility assessments, and package/build claims inherit explicit identity, scope, freshness, and method semantics.

### RFC 0004 — recursive introspection and refinement

Composition graphs, bindings, adapters, closures, and compatibility failures should participate in causal slicing, repair proposals, evidence invalidation, and bounded promotion.

### RFC 0006 — machine intent

Provider selection and adapter realization may optimize declared preferences only after hard compatibility requirements are satisfied.

### RFC 0007 — proof core

Kernel-checkable proof terms may discharge selected composition obligations, but dependency resolution, package search, build execution, and empirical benchmarking remain outside the proof kernel.

### RFC 0008 — effects, authority, resources, protocols

Component contracts carry effect/capability/resource/protocol surfaces. Link graphs compute conservative closures and explicit authority transfers.

### RFC 0009 — memory and provenance

Interfaces may expose memory/provenance/lifetime requirements. Adapters and foreign boundaries must preserve or explicitly weaken them.

### RFC 0010 — concurrency

Components may expose concurrency, atomicity, progress, fairness, interference, and visibility contracts. Composition must preserve compatibility across these dimensions.

### RFC 0011 — failure/recovery

Provider compatibility includes failure surfaces, retry semantics, ambiguous outcomes, recovery contracts, and external-knowledge assumptions.

### RFC 0012 — executable semantics

Behavioral compatibility may use trace simulation/refinement against the normative semantic machine.

### RFC 0013 — abstraction and realization holes

Component requirements generalize explicit abstraction/evidence to software-architecture scale. Dependency holes are a specialized form of realization hole with component contracts as constraints.

## Security and confinement consequences

RFC 0014 should make dependency composition part of the security model.

A final unit can expose:

```text
AuthorityClosure
EffectClosure
ExternalEndpointClosure
ForeignBoundaryClosure
UnsafeAssumptionClosure
```

These can be used to prove or conservatively establish properties such as:

```text
NoNetwork(Unit)
NoFilesystemWrite(Unit)
NoForeignCode(Unit)
NoRawDeviceAccess(Unit)
```

A transitive dependency that violates one of those claims must invalidate the corresponding evidence.

Component substitution must not amplify authority unless the consumer/policy explicitly permits the amplification.

Adapters intended to attenuate authority require evidence that they do not leak the stronger authority surface through another channel.

## Diagnostics

Composition diagnostics should identify:

- requirement identity;
- provider identity;
- failing semantic dimension;
- compared properties;
- evidence used;
- evidence missing/stale;
- authority/effect expansion;
- relevant assumptions/guarantees;
- candidate adapters or provider alternatives;
- causal dependency slice; and
- whether the problem is `FAIL` or `UNKNOWN`.

A machine-generated diagnostic should be useful directly to Forge or another agent without reparsing prose.

## Experimental tracks

RFC 0014 should begin with bounded experiments rather than implementing a complete package ecosystem.

### Experiment 1 — open component verification

Create one component with an unresolved provider contract.

Verify it against the requirement without selecting an implementation.

Instantiate it independently with two compatible providers and demonstrate that the component's client-facing proof depends on the requirement identity rather than either implementation identity.

### Experiment 2 — semantic substitution

Create Provider A and stronger Provider B.

Prove or mechanically establish:

```text
B ⊑ Requirement
A ⊑ Requirement
```

Replace A with B while preserving applicable client evidence.

### Experiment 3 — authority closure

Construct a dependency graph that is initially local-only.

Introduce one transitive provider requiring:

```text
Network<Egress>
```

Verify that the linked unit's authority closure changes and invalidates a prior `NoNetwork` claim.

This is a central experiment.

### Experiment 4 — recursive linking

Model:

- one legal type/protocol recursive component pair;
- one legal guarded/definedness-safe runtime cycle; and
- one illegal use-before-definition cycle.

Verify the checker distinguishes them.

### Experiment 5 — evidence composition

Create a consumer assumption discharged by a provider guarantee.

Then remove or stale the provider evidence and verify only dependent link evidence becomes invalid.

### Experiment 6 — explicit adapter

Make two components incompatible directly but compatible through an adapter.

Verify the final unit contains adapter identity, authority/effects, obligations, and evidence.

### Experiment 7 — generative/applicative identity

Instantiate an applicative component twice with identical inputs and a generative component twice with identical inputs.

Demonstrate expected identity equalities/inequalities mechanically.

### Experiment 8 — separate-verification invalidation

Change a provider's private implementation while preserving its interface identity.

Verify unrelated client evidence remains fresh.

Then change a relevant interface guarantee and verify exactly dependent clients invalidate.

This is a central experiment.

### Experiment 9 — package/build identity

Produce two builds from the same semantic unit under:

- identical build inputs; and
- changed compiler/target/pass-pipeline identity.

Demonstrate separation among semantic identity, build identity, and artifact identity.

### Experiment 10 — Forge dependency tournament

Provide several compatible implementations of one requirement.

Use RFC 0006 objectives such as:

```text
minimize authority surface
minimize p99 latency
limit memory
require fresh proof evidence
```

Select a candidate while retaining the same requirement identity.

### Experiment 11 — dynamic binding

Load a provider manifest at runtime.

Check compatibility before authority grant.

Demonstrate PASS, FAIL, and UNKNOWN policy paths.

### Experiment 12 — semantic compatibility/versioning

Compare old/new provider contracts and derive a compatibility result for one specific client requirement.

Show that compatibility classification can disagree with a simple version-number heuristic.

## Mandatory negative tests

The initial test suite should include intentionally rejected or unresolved cases.

### Name-only compatibility

Two components share an expected name or symbol but do not satisfy the same semantic contract.

Expected: rejection.

### Type-only compatibility

Provider has the same value-level type but widens forbidden effects or authority.

Expected: rejection.

### Hidden authority amplification

A transitive dependency introduces network/raw-device/filesystem authority without appearing in the root source.

Expected: unit authority closure expands; restrictive policy fails.

### `UNKNOWN` compatibility used as PASS

A behavioral/evidence dimension is unresolved and resolver attempts promotion anyway.

Expected: rejection under strict assurance policy.

### Adapter erased from canonical graph

Tooling generates conversion glue but omits adapter identity/effects/evidence.

Expected: invalid canonical unit.

### Circular evidence

Consumer assumption is supposedly discharged by provider evidence that itself depends on the consumer assumption.

Expected: unresolved or rejected unless a valid fixed-point proof rule explicitly permits it.

### Use-before-definition cycle

Recursive components execute a required value before its definedness obligation is established.

Expected: rejection.

### Generative identity collapse

Two generative component instantiations are treated as the same nominal domain.

Expected: rejection.

### Applicative false inequality

Two applicative instantiations that should share declared identity are assigned incompatible abstract identities without a declared generative source.

Expected: diagnostic/inconsistency.

### Name visibility grants authority

Importing/referencing a component implicitly grants protected capability authority.

Expected: rejection.

### Stale interface evidence

Provider interface changed but old compatibility proof remains attached.

Expected: stale evidence invalidation.

### Over-invalidation

Provider private implementation changes while interface and relevant evidence remain stable, yet all clients are invalidated indiscriminately.

Expected: test failure for narrow invalidation objective.

### Content hash treated as compiler proof

Artifact hash is used as proof that binary refines semantic source.

Expected: rejection.

### SemVer used as sole compatibility proof

Resolver accepts provider solely because version range matches while semantic contract fails.

Expected: rejection.

### Dynamic plugin gets authority before compatibility check

Runtime grants capability to plugin and validates later.

Expected: rejection.

### Unresolved component executes

A runtime path reaches an operation whose requirement remains unbound.

Expected: explicit failure before effect/authority use; no implicit host fallback.

## Open design questions

RFC 0014 intentionally leaves several questions for experiment.

1. Which exact subset of ML/Fω module machinery belongs in the canonical core versus surface elaboration?
2. How much recursive linking should the first executable subset permit?
3. Which compatibility dimensions can share one generic refinement lattice and which require specialized relations?
4. Should interface identities include evidence policy, or should evidence policy remain a separate consumer-side parameter?
5. What exact rules determine applicative unit identity under evidence changes?
6. How should component-level realization holes interact with RFC 0013 type/representation realization holes?
7. Which package/build metadata is semantic, which is provenance, and which is merely human-facing annotation?
8. How should optional requirements and soft dependencies be represented without weakening soundness?
9. What runtime admission policy is appropriate for `UNKNOWN` dynamic-provider dimensions?
10. How should recursive component initialization interact with RFC 0010 structured concurrency and RFC 0011 recovery?
11. Which compatibility claims are kernel-checkable versus certificate/verifier-level evidence?
12. How should semantically equivalent but artifact-different providers share caches and evidence?
13. What phase boundaries for partial linking belong here versus RFC 0016 staging?
14. How should distributed/remote components be represented before a later distributed-systems RFC?
15. What is the smallest component calculus that still preserves all required authority/evidence/identity information?

## Initial decision direction

The first implementation should remain deliberately bounded.

Recommended initial subset:

1. one versioned component-contract artifact;
2. one requirement and one provider per experiment;
3. explicit `PASS/FAIL/UNKNOWN` compatibility by dimension;
4. type/effect/authority/assumption/guarantee/evidence dimensions first;
5. one explicit binding artifact;
6. one unit graph with authority/effect/assumption closures;
7. one separate-verification cache keyed by interface identity;
8. one explicit adapter case;
9. one generative identity case;
10. one simple recursive-definedness rejection case;
11. one package/build/artifact identity experiment; and
12. one Forge-compatible provider-selection request/result artifact.

Protocol traces, full recursive linking, dynamic loading, behavioral simulation, and broad package ecosystem tooling should remain later increments until the smaller calculus is stable.

## Expected consequence

If this direction succeeds, MNCS software architecture becomes a semantic graph rather than a convention layered on source files and package metadata.

A machine can ask:

```text
Why is this dependency here?
Which requirement does it satisfy?
Which provider was selected?
Could another provider replace it?
Which authority does it add?
Which assumptions does it discharge?
Which proofs depend on its interface?
What invalidates those proofs?
Which adapter sits at this boundary?
Which unresolved choices remain?
Why was this provider preferred?
What exact semantic unit produced this artifact?
```

and receive checkable answers from canonical artifacts.

That is the intended meaning of machine-native composition.

## Proposed principle

> **MNCS composition is evidence-bearing binding between separately verifiable semantic contracts. Modules organize abstraction; components expose requirements and provisions; units record concrete bindings; packages preserve immutable distribution/build identity; and provider selection remains a verified realization problem wherever compatibility leaves multiple valid choices.**
