# RFC 0015: Machine-Native Trust Boundary, Unsafe Operation, Foreign Interface, ABI, and Containment Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.5–1.0
- **Depends on:** RFC 0002, RFC 0003, RFC 0004, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0013, RFC 0014

## Summary

MNCS Language should define unsafe execution and foreign interaction through explicit **trust-boundary semantics** rather than through a lexical escape hatch or an opaque foreign-call primitive.

The central rule is:

> **Crossing a trust boundary does not suspend MNCS semantics. It replaces internally established guarantees with explicit assumptions, translations, guards, containment, and evidence obligations.**

The corresponding unsafe rule is:

> **Unsafe does not mean unchecked. Unsafe means that an obligation cannot be discharged by the ordinary language rules and must be transferred into an explicit boundary evidence regime.**

A foreign call is therefore not merely `arguments -> result`, and an ABI is not the semantic identity of a foreign interface. A foreign interaction may alter memory, ownership, provenance, resources, authority, effects, event order, callback state, failure state, recovery state, external knowledge, and observable traces. Those relationships must remain explicit enough for machines to reason about them.

This RFC distinguishes several dimensions that conventional systems often collapse:

```text
Foreign
    code or behavior governed by another semantic domain

Unsafe
    an ordinary MNCS guarantee has become an explicit boundary obligation

Unverified
    required evidence is not available at the requested assurance level

Untrusted
    behavior may be adversarial within the authority granted

ABI-bound
    a physical calling or representation convention is fixed

Target-specific
    correctness depends on a named target environment
```

These dimensions may overlap, but they are not synonyms.

A handwritten MNCS intrinsic may be unsafe without being foreign. A WebAssembly component may be foreign but strongly contained. A C library may be foreign, ABI-bound, unverified, and operationally trusted by policy. A malicious plugin may be memory-safe while still being semantically untrusted.

The language should preserve those distinctions through the semantic graph, HIR, SSA, linking artifacts, evidence manifests, runtime guards, and target realizations.

## Constitutional principles

### 1. Unsafe transfers obligation; it does not erase it

> **An unsafe operation moves an otherwise unprovable guarantee into an explicit boundary obligation whose disposition remains visible.**

The obligation may be:

```text
proved
certified
verifier_checked
runtime_guarded
contained
empirically_validated
assumed
UNKNOWN
```

according to RFC 0002 evidence classes and the active assurance policy.

### 2. Foreign code never inherits MNCS guarantees automatically

> **A guarantee survives a boundary only when the boundary contract and available evidence establish its preservation.**

Crossing into a C ABI, assembler, accelerator, foreign VM, plugin, device, or other domain does not preserve provenance, ownership, resource, concurrency, failure, or effect guarantees by convention.

### 3. ABI is a realization

> **Calling convention, register use, stack layout, data layout, marshaling, unwind mechanism, and binary representation are target realizations of a semantic boundary contract.**

The semantic interface must remain distinguishable from its ABI realizations.

### 4. Compromise does not amplify authority

> **A component that becomes arbitrary or adversarial remains confined to the authority previously delegated to its containment domain.**

Compromise may invalidate behavioral assumptions. It must not create new capability authority.

### 5. Assumptions are semantic debt

> **Every unverified boundary assumption remains identifiable, scoped, dependency-tracked, and eligible for later replacement by proof, certification, runtime enforcement, containment, or a stronger provider.**

### 6. A boundary may weaken knowledge, never fabricate proof

A raw foreign pointer is not automatically valid MNCS provenance. A foreign error code is not automatically an RFC 0011 failure theorem. A successful call is not automatically proof that every claimed postcondition holds.

### 7. Boundary adaptation is semantic work

Marshaling, ownership transfer, representation conversion, callback wrapping, exception translation, and ABI shims are explicit transformations with obligations and evidence. They are not invisible glue.

## Motivation

MNCS is intentionally designed to carry more semantic information than conventional native ABIs can represent directly.

An MNCS operation may expose or depend upon:

```text
type
logical identity
representation identity
memory provenance
epoch
initialization
ownership
permission grade
resource ownership
authority
capability delegation
effect row
protocol state
concurrency relations
visibility requirements
failure semantics
retry safety
recovery obligations
observation provenance
progress assumptions
proof obligations
evidence identities
```

A conventional native FFI declaration often reduces all of this to something like:

```text
extern foo(void *p, size_t n) -> int
```

That declaration is not sufficient to preserve the semantic claims established elsewhere in MNCS.

Questions immediately arise:

- Does `foo` retain `p` after the call?
- May it mutate the pointed region?
- May it write beyond `n`?
- Does it free the allocation?
- May it synthesize aliases?
- Does it call back into MNCS?
- Can it call back concurrently?
- Does it spawn worker threads?
- Can it block indefinitely?
- Does it unwind across the boundary?
- Does it mutate global process state?
- Does it open files or network connections?
- Is a timeout proof that the operation did not commit externally?
- Can returned addresses be assigned valid MNCS provenance?
- Which representation and alignment rules apply?
- Which compiler, target, ABI, and foreign-library build justify the declaration?
- Which guarantees are proved, documented, tested, assumed, or unknown?

A machine-native language should preserve these facts explicitly so autonomous compilers, verifiers, Forge searches, runtimes, and deployment systems can make conservative decisions.

## Goals

RFC 0015 should make the following possible.

1. **Represent unsafe as an evidence-regime transition.** Unsafe constructs must generate explicit obligations rather than suppress checking.
2. **Represent foreign calls relationally.** Foreign behavior should describe relevant pre/post state, effects, observations, and outcomes.
3. **Preserve boundary-specific trust information.** Memory safety, effect behavior, ABI correctness, functional correctness, constant-time behavior, availability, and other properties remain separately evidenced.
4. **Separate semantics from ABI realization.** One semantic interface may admit multiple target-specific calling and marshaling realizations.
5. **Make ownership transfer explicit.** Borrowed, consumed, retained, returned, aliased, and transferred foreign values must have distinct semantics.
6. **Preserve RFC 0009 provenance rules.** Raw addresses must not become valid MNCS references without sufficient derivation/evidence.
7. **Model callbacks and reentrancy.** Foreign re-entry must participate in task, authority, protocol, and concurrency semantics.
8. **Model unwind and nonlocal control transfer.** Cleanup and authority restoration obligations must not be bypassed by foreign control flow.
9. **Model blocking and asynchronous foreign work.** Scheduler assumptions and progress consequences must be explicit.
10. **Integrate RFC 0011 failure semantics.** Foreign error representations must map to explicit failure/effect/uncertainty states.
11. **Track assumption debt.** Unverified foreign claims must have scope and blast-radius dependencies.
12. **Permit runtime guards and containment.** Missing static trust may be replaced by explicit enforcement when that enforcement is itself justified.
13. **Support proof-carrying and certified artifacts.** Foreign providers may bring machine-checkable evidence.
14. **Support dynamic compromise.** A component may lose behavioral trust without acquiring new authority.
15. **Support adversarial-context reasoning.** Security properties may be required to hold against arbitrary foreign behavior within a granted interface/authority envelope.
16. **Preserve evidence freshness.** ABI, target, layout, compiler, foreign artifact, or adapter changes must invalidate dependent evidence.
17. **Make boundaries machine-inspectable.** A machine should be able to ask exactly which guarantees survive, weaken, or remain unknown.
18. **Expose containment as a realization search space.** Forge may compare in-process FFI, process isolation, Wasm, SFI, capability hardware, or other boundary realizations when semantics permit.

## Non-goals

This RFC does not:

- define every C, C++, Rust, Swift, JVM, CLR, Python, WebAssembly, GPU, or device ABI;
- make C or any other language the canonical foreign language;
- claim that arbitrary native binaries can be fully verified automatically;
- put a complete foreign-language semantics inside the RFC 0007 trusted kernel;
- define distributed consistency, consensus, or full remote-service semantics;
- require all foreign code to use process isolation;
- require CHERI, WebAssembly, SFI, or one sandbox technology;
- require proof-carrying code for every FFI call;
- treat vendor documentation as formal proof;
- make fuzzing or testing equivalent to proof;
- make raw assembly automatically untrusted or automatically unsafe;
- assume memory-safe foreign code is semantically trusted;
- treat ABI compatibility as semantic compatibility;
- allow an `unsafe` annotation to discharge obligations by itself; or
- permit `UNKNOWN` to authorize a stronger guarantee than the evidence establishes.

## Selected theoretical synthesis

### Semantic typing for unsafe abstractions

Unsafe internals should be able to implement safe abstractions when their exported contract is justified.

Conceptually:

```text
UnsafeImplementation<U>
    + Evidence<U refines InterfaceI>
    ->
SafeInterface<I>
```

The unsafe implementation may use lower-level operations whose preconditions are not derivable through ordinary safe typing. Those preconditions become explicit internal obligations.

A conceptual artifact is:

```text
UnsafeImplementation {
    identity
    implementation

    exported_contract

    unsafe_operations
    internal_assumptions

    generated_obligations
    discharged_obligations
    unresolved_obligations

    evidence
}
```

Clients may rely on the exported safe contract only when the implementation's boundary obligations are satisfied at the assurance level required by policy.

This preserves abstraction: callers should not inherit internal unsafe obligations that have been successfully closed by the implementation proof/certificate/guarded boundary.

### Linking types and semantic-domain expansion

Cross-language linking may expose values to operations or behaviors that were impossible in the source semantic domain.

A boundary must therefore record changes such as:

```text
source_domain:
    MNCS

target_domain:
    ForeignC

preserved:
    [value_representation_contract]

not_automatically_preserved:
    [
        provenance,
        exclusive_borrow,
        no_forged_aliases,
        bounded_effects
    ]
```

A conceptual object is:

```text
SemanticDomainTransition {
    source_domain
    target_domain

    guarantees_preserved
    guarantees_weakened
    newly_possible_behaviors

    adapters
    guards
    containment

    obligations
    evidence
}
```

The linker may not claim that a source-domain theorem continues to hold after a boundary unless the boundary relation establishes that preservation.

### Relational external-call semantics

Foreign operations should be relations over relevant machine/world states.

Conceptually:

```text
ForeignOperationRelation {
    inputs

    memory_before
    memory_after

    authority_before
    authority_after

    resources_before
    resources_after

    event_context_before
    event_context_after

    effects
    observations

    concurrency_interference

    failure_outcome
    recovery_state

    result

    assumptions
    evidence
}
```

The relation may be nondeterministic where foreign behavior is intentionally underspecified.

For example:

```text
ForeignRead(fd, buffer)
```

may specify that:

- only the destination buffer range may be written;
- at most `N` bytes become initialized;
- the file descriptor resource remains live;
- the operation may block;
- the operation may fail;
- the result determines the initialized prefix;
- no network authority is introduced; and
- no callback occurs.

A target ABI realization may execute an OS/native library call, but the MNCS semantic identity is the relation.

### Typed low-level artifacts

Low-level artifacts need not imply total loss of semantic structure.

MNCS should allow foreign/low-level artifact classes such as:

```text
RawMachineArtifact
TypedMachineArtifact
CertifiedMachineArtifact
```

A typed or certified artifact may expose facts about:

```text
register usage
stack discipline
memory footprints
control-flow targets
calling convention
capability requirements
preserved registers
clobbers
unwind behavior
proof evidence
```

The available facts determine which boundary obligations can be discharged without broader assumptions.

### Proof-carrying foreign code

A foreign producer may provide a binary plus machine-checkable evidence against a declared policy.

Conceptually:

```text
ForeignArtifact {
    artifact_identity
    binary_identity

    claimed_contract
    certificate_or_proof

    verifier_requirements
}
```

Acceptance becomes:

```text
check(certificate, artifact, policy)
    -> PASS | FAIL | UNKNOWN
```

A `PASS` may authorize a narrower containment boundary or additional safe assumptions, but only for the exact artifact, policy, verifier, target, and dependency identities covered by the evidence.

### Capability-machine and compartment semantics

Hardware capabilities, software fault isolation, page-based process isolation, language sandboxes, and related mechanisms should be viewed as alternative realizations of bounded authority.

For example, the semantic contract may grant:

```text
Read<Input[0..4096)>
Write<Output[0..512)>
```

Possible realizations include:

```text
same-process checked wrapper
MPK/page protection
SFI
separate process
WebAssembly sandbox
capability-machine compartment
hypervisor/VM boundary
```

The semantic authority envelope remains the same even when physical enforcement differs.

### Robust safety against hostile contexts

Some boundaries should support properties of the form:

```text
RobustBoundaryProperty<P> {
    component
    property = P

    under:
        arbitrary_foreign_behavior
        within_granted_interface
        within_granted_authority
}
```

Example:

```text
SecretRegion is never readable by foreign component
```

must remain true even if the component behaves adversarially, provided the containment mechanism is correct.

This is a stronger property than ordinary correctness in cooperative contexts.

### Dynamic compromise

A component may transition from behavior satisfying its contract to arbitrary behavior after memory corruption, undefined behavior, exploit, or another compromise event.

Conceptually:

```text
ForeignComponentState {
    uncompromised
    compromised
}
```

Transition:

```text
uncompromised
    -- compromise_event -->
compromised
```

After compromise:

- behavioral guarantees from the component may be invalid;
- outputs may be adversarial;
- protocol compliance may be lost;
- effect attempts may become adversarial;
- the component must still be unable to exceed the containment authority granted.

This yields a key property:

```text
Compromise(component)
    does_not_imply
AuthorityAmplification(component)
```

### ABI and canonical boundary realization

MNCS should distinguish a semantic boundary from a concrete ABI.

Conceptually:

```text
SemanticOperation:
    Transform(Text) -> Text
```

may admit realizations such as:

```text
SysV_x86_64
Windows_x64
AAPCS64
Wasm_Canonical_ABI
JNI
C_API_with_generated_shim
shared_memory_protocol
custom_device_mailbox
```

The ABI defines physical details. The semantic operation defines observable meaning and obligations.

## Core semantic model

### TrustBoundary

A conceptual boundary object is:

```text
TrustBoundary {
    identity

    source_domain
    target_domain

    semantic_interface

    imported_types
    exported_types

    representation_mapping
    ABI_realization?

    ownership_mapping
    provenance_mapping
    lifetime_mapping

    authority_granted
    authority_returned
    authority_revoked

    effects_allowed

    concurrency_contract
    callback_contract
    reentrancy_contract

    failure_contract
    unwind_contract
    cancellation_contract

    assumptions
    assumption_debt

    runtime_guards
    containment

    obligations
    evidence_policy
    evidence

    target_identity?
    adapter_identity?
    artifact_identity?
    toolchain_identity?
}
```

This is a semantic sketch rather than a final serialization format.

### ForeignOperation

```text
ForeignOperation {
    identity

    boundary

    input_contract
    output_contract

    memory_relation
    resource_relation
    authority_relation

    effect_relation
    observation_relation

    concurrency_relation

    callback_relation
    unwind_relation

    failure_relation
    recovery_relation

    assumptions
    obligations
    evidence
}
```

### ABIAdapter

```text
ABIAdapter {
    identity

    semantic_interface
    ABI_identity

    source_representation
    target_representation

    argument_lowering
    result_lifting

    ownership_translation
    provenance_translation
    lifetime_translation

    error_translation
    callback_translation
    unwind_translation

    target_constraints
    layout_constraints

    obligations
    evidence
}
```

### BoundaryEvidence

```text
BoundaryEvidence {
    identity

    subject_boundary
    subject_artifact

    claims

    evidence_class
    verifier?

    foreign_artifact_identity
    ABI_identity?
    target_identity?
    layout_identity?
    toolchain_identity?
    adapter_identity?

    assumptions
    freshness_dependencies
}
```

### AssumptionDebt

```text
AssumptionDebt {
    identity

    subject
    assumption

    source
    evidence_class

    blast_radius
    dependent_claims

    candidate_repayments {
        proof
        certificate
        runtime_guard
        containment
        verified_provider
        stronger_adapter
    }
}
```

## Unsafe as an evidence-state transition

MNCS should not define unsafe execution as disabling the type/effect/provenance system.

Conceptually:

```text
OrdinaryObligation<O>
    -- cannot_discharge_under_safe_rules -->
BoundaryObligation<O>
```

The boundary obligation then requires a disposition:

```text
BoundaryDisposition {
    proved
    certified
    verifier_checked
    runtime_guarded
    containment_enforced
    empirically_supported
    explicitly_assumed
    UNKNOWN
}
```

A lexical `unsafe` representation, if one exists, should elaborate to this semantic transformation.

The keyword itself is never evidence.

### Example: unchecked indexing

A source-level construct might request:

```text
unsafe_load(buffer, i)
```

The canonical semantics should not mean:

```text
perform load without bounds semantics
```

It should mean:

```text
Load(buffer, i)
requires:
    InBounds(buffer, i)

boundary_disposition:
    caller_assumption | proof | runtime_guard | other accepted evidence
```

If the obligation is unresolved under the active assurance policy, the operation is not promoted as safe executable code.

## Boundary trust is multidimensional

Trust must not collapse into one scalar level.

A boundary may have:

```text
BoundaryTrust {
    memory_safety: proved
    provenance_preservation: runtime_guarded
    ABI_correctness: verified
    effect_contract: assumed
    callback_behavior: verified
    constant_time: UNKNOWN
    availability: UNKNOWN
    functional_correctness: empirically_validated
}
```

Different consumers may require different assurance policies.

A cryptographic consumer may reject `constant_time = UNKNOWN` while an ordinary data-processing consumer accepts it.

A memory-isolation policy may accept functional correctness as `UNKNOWN` if the component is untrusted but strongly contained.

## Guarantee preservation across boundaries

A boundary must record whether each relevant guarantee is:

```text
preserved
translated
weakened
re-established
runtime-enforced
assumed
lost
UNKNOWN
```

Conceptually:

```text
GuaranteeTransition {
    guarantee
    source_status
    target_status

    mechanism
    evidence
}
```

Examples:

```text
exclusive_write_authority
    -> preserved by unique handle translation

stable_address
    -> re-established by pinning before call

no_callback
    -> assumed from foreign specification

write_bound
    -> runtime-enforced by sandbox

provenance
    -> UNKNOWN for arbitrary returned integer address
```

The re-entry into the safe MNCS world can rely only on guarantees that are preserved or re-established at acceptable assurance.

## Ownership semantics across the boundary

Foreign parameters and results must distinguish at least:

```text
borrow_for_call
borrow_until<Event>
shared_borrow
mutable_borrow
consumed
transferred_to_foreign
retained_by_foreign
returned_to_mncs
aliased
copied
```

A conceptual parameter is:

```text
ForeignParameter {
    value

    ownership_mode
    mutation_permission

    retention_policy
    aliasing_policy

    lifetime_bound

    obligations
}
```

### Borrow-for-call

```text
ownership = borrow_for_call
retention = forbidden
```

requires evidence that the foreign implementation cannot access the object after the call terminates.

### Retained foreign reference

```text
ownership = retained_by_foreign
```

creates a longer-lived external owner and must alter RFC 0009 lifetime/authority state.

### Transfer

```text
ownership = transferred_to_foreign
```

must consume the corresponding MNCS ownership/free authority according to the declared contract.

## Foreign provenance translation

RFC 0009's address/identity/provenance separation remains in force at a boundary.

A foreign value such as:

```text
0x7f93...
```

is not automatically:

```text
Reference<T>
```

A foreign address may initially have a weaker semantic class:

```text
ForeignAddress {
    bits
    source_boundary
}
```

Promotion to an MNCS reference requires something like:

```text
ImportReference {
    address

    allocation_identity_or_domain
    extent
    epoch

    lifetime
    permissions

    provenance_source

    obligations
    evidence
}
```

If sufficient provenance cannot be established, the value remains a foreign/raw-address object usable only by operations whose contracts accept that weaker class.

### Non-forging rule

A boundary must never synthesize ordinary MNCS provenance solely from integer equality or address coincidence.

```text
same_address
    !=
valid_provenance
```

This remains true after deallocation/reallocation and across foreign allocators.

## Representation and layout translation

Logical type identity, representation identity, and ABI layout identity remain separate.

A boundary may require explicit transformations such as:

```text
MNCS Text
    -> UTF8_NUL_TERMINATED_C_BUFFER

MNCS Sum<A,B>
    -> tagged_union_layout_v3

MNCS Sequence<T>
    -> pointer_length_pair
```

Each transformation may generate obligations concerning:

```text
encoding
length
NUL constraints
alignment
padding
endianness
valid discriminants
initialization
aliasing
ownership
lifetime
```

Representation freezing occurs only where the boundary requires it.

## Callbacks and reentrancy

A foreign call may re-enter MNCS before the original call returns.

Conceptually:

```text
MNCS
  -> Foreign
       -> Callback(MNCS)
            -> ...
```

Callbacks must be explicit in the contract.

```text
CallbackContract {
    policy:
        forbidden
      | synchronous
      | asynchronous
      | concurrent

    callback_interface

    authority_granted
    effects_allowed

    reentrancy
    task_scope
    lifetime

    protocol

    obligations
}
```

If callbacks are forbidden, evidence or containment must enforce that rule.

If callbacks are permitted, they create normal RFC 0010 task/event relations and RFC 0008 authority/effect transitions.

### Reentrancy

A component may declare:

```text
reentrant = false
```

If foreign behavior can re-enter the component while an invariant is temporarily open, the binding is invalid unless an adapter/guard restores the required atomicity or state protocol.

A callback is therefore a semantic re-entry edge, not merely a function pointer.

## Unwind and nonlocal control transfer

Foreign control transfer mechanisms include:

```text
C++ exception
Rust panic across compatible boundary
setjmp/longjmp
SEH
language VM exception
signal-like escape
foreign runtime abort
```

The boundary must define a policy:

```text
UnwindPolicy {
    forbidden
    translated_to_failure
    adapter_managed
    process_abort
}
```

No nonlocal transfer may silently bypass semantic cleanup obligations.

Before re-entry into safe MNCS execution, the boundary must account for:

```text
resource release
authority restoration
borrow termination
protocol state
transaction/recovery state
child tasks
pinned memory
registered buffers
```

If correct restoration cannot be established, the outcome remains unsafe/failed/unknown according to the governing semantics.

## Blocking, asynchronous work, and scheduler interaction

Foreign operations may have scheduling behavior such as:

```text
nonblocking
may_block
may_block_unbounded
spawns_threads
completes_async
requires_thread_affinity
invokes_callbacks_concurrently
```

These facts belong in the boundary contract.

Example:

```text
ForeignConcurrencyContract {
    blocking = may_block
    spawn = forbidden
    callbacks = synchronous
    affinity = caller_thread
    completion = synchronous
}
```

Unknown concurrency behavior must reduce scheduler/parallelization authority.

A compiler may choose to isolate blocking calls onto a dedicated worker only when that transformation preserves the operation's task/protocol/authority semantics.

## Foreign failures and RFC 0011

Foreign error representations are not themselves MNCS failure semantics.

Examples include:

```text
errno
HRESULT
null
negative integer
exception
HTTP status
OS status
device completion code
```

An adapter maps those representations into:

```text
FailureOutcome {
    cause
    completion_state
    committed_effects
    partial_effects
    uncertain_effects
    resource_state
    authority_state
    retry_semantics
    recovery_semantics
    knowledge_state
}
```

### Timeout rule

A foreign timeout remains subject to RFC 0011:

```text
Timeout
    !=
ProofOfNonCommit
```

If an external effect may have committed before the timeout, the foreign boundary must preserve an ambiguous/unknown effect state.

### Cancellation rule

A foreign cancellation request is not proof that foreign work stopped.

The boundary must model whether cancellation is:

```text
confirmed
best_effort
unsupported
UNKNOWN
```

and whether resources/authority remain in foreign ownership until an explicit completion event.

## Assumption debt

Foreign code frequently requires claims that cannot initially be proved.

Instead of losing those claims inside comments or documentation, MNCS should create explicit assumption debt.

Example:

```text
AssumptionDebt {
    subject = LibraryFoo.function_bar

    assumptions = [
        does_not_retain(pointer),
        writes_at_most(64_bytes),
        never_calls_back_concurrently
    ]

    evidence_class = vendor_documentation

    dependent_claims = [
        SafeWrapper#91,
        Parallelization#22
    ]
}
```

The graph can later replace an assumption with stronger evidence.

Candidate repayment paths include:

```text
formal proof
proof-carrying artifact
specialized verifier
runtime guard
sandbox/containment
verified adapter
provider replacement
```

### Blast radius

Changing or invalidating an assumption should invalidate only claims that actually depend upon it.

This directly uses RFC 0004's dependency-aware invalidation.

## Runtime guards

Runtime enforcement may convert an unverified assumption into an enforced property.

Example:

```text
Assumed<WriteBound<N>>
```

may become:

```text
RuntimeEnforced<WriteBound<N>>
```

through a capability boundary, canary/check mechanism, checked copy, process isolation, or another enforcement realization.

The guard itself has:

```text
Guard {
    property
    mechanism
    failure_behavior
    assumptions
    evidence
}
```

A runtime guard does not prove the foreign component obeys the property. It establishes that the composed boundary prevents or detects violations according to the guard's contract.

## Containment as a realization

When direct foreign trust is insufficient, MNCS may select stronger containment.

Example candidate set:

```text
same_process_raw_ffi
same_process_checked_wrapper
SFI_compartment
Wasm_component
separate_process
capability_compartment
VM_sandbox
```

Forge may evaluate candidates against:

```text
requirements {
    foreign_cannot_read(Secret)
    foreign_cannot_open(Network)
}

preferences {
    minimize_latency
    minimize_copying
    minimize_memory
}
```

A lower-overhead candidate that cannot establish the protected property is invalid regardless of performance.

### Containment result

```text
ContainmentRealization {
    boundary
    mechanism

    enforced_authority
    enforced_memory_domains
    enforced_effects

    failure_isolation
    compromise_model

    assumptions
    obligations
    evidence
}
```

## Dynamic compromise semantics

A deliberate experiment should treat compromise as an ordinary semantic transition of foreign trust state.

Before compromise:

```text
BehavioralAssumptions = ContractC
Authority = A
```

After compromise:

```text
BehavioralAssumptions = ArbitraryWithinBoundary
Authority = A
```

not:

```text
Authority = Universal
```

The security theorem should therefore have a form such as:

```text
for all behaviors B allowed by containment A:
    Secret remains unreachable
    Config remains immutable
    Network remains unavailable
```

This distinguishes correctness trust from authority control.

## Robust-boundary properties

A boundary may require properties that remain true under arbitrary hostile context behavior.

Examples:

```text
Confidential<SecretRegion>
Integrity<ConfigRegion>
NoNetworkEgress
NoCapabilityAmplification
NoProvenanceForgeryIntoSafeDomain
```

The proof/evidence may depend on containment rather than on the foreign component's cooperation.

This distinction should be visible:

```text
PropertySource {
    cooperative_component_contract
    containment_enforcement
    verified_adapter
    hardware_isolation
}
```

## ABI identity and freshness

Boundary evidence is often target-specific.

A claim may depend on:

```text
foreign_artifact_identity
ABI_identity
target_identity
layout_identity
compiler_identity
compiler_flags
linker_identity
adapter_identity
foreign_runtime_version
```

Changing a relevant dependency invalidates the evidence.

For example, evidence for:

```text
x86_64-linux + SysV ABI
```

must not silently authorize:

```text
AArch64 + AAPCS64
```

or a different packing/enum/exception convention.

## Semantic boundary vs ABI realization

One semantic interface may have multiple realizations:

```text
SemanticInterface I
    -> ABIAdapter<SysV>
    -> ABIAdapter<AAPCS64>
    -> ABIAdapter<WasmCanonical>
```

Each realization must establish:

```text
RealizationSatisfiesBoundary(
    semantic_interface,
    ABI_adapter,
    target,
    artifact,
    evidence
)
```

This mirrors RFC 0012's general lowering/refinement architecture.

## Boundary adapters are explicit components

Adapters may perform:

```text
layout conversion
serialization
copying
pinning
buffer registration
ownership attenuation
authority attenuation
error translation
callback wrapping
thread handoff
exception capture
protocol adaptation
```

They should appear as RFC 0014 components/bindings where composition-level visibility matters.

An adapter may reduce authority:

```text
ForeignProvider requires broad filesystem
    -> isolated adapter exposes only Read<FileX>
```

provided the containment mechanism can actually enforce that narrower exported semantic contract.

## Foreign-interface declarations

A source representation may offer concise syntax, but canonical semantics should elaborate foreign declarations into explicit contracts.

A hypothetical source declaration:

```text
foreign C foo(Buffer, usize) -> i32
```

is insufficient by itself.

The semantic artifact should resolve relevant dimensions such as:

```text
ForeignDeclaration {
    operation
    ABI

    argument_contracts
    result_contract

    ownership
    aliasing
    provenance

    effects
    authority

    blocking
    callbacks
    reentrancy

    unwind
    failure_mapping

    assumptions
    evidence
}
```

Inference or defaults may compress source syntax, but RFC 0013's rule still applies:

> inference may hide syntax, never semantics.

## Unsafe boundary closure

A safe abstraction may contain an unsafe region if its exported contract closes all relevant unsafe obligations.

Conceptually:

```text
unsafe region U
    produces obligations O

proof/guards/assumptions E
    discharge O under policy P

therefore:
    SafeBoundary(U, ContractC, E)
```

If one obligation remains `UNKNOWN` and the assurance policy requires proof, the wrapper cannot be promoted as safe.

A less strict policy may permit promotion while recording the unresolved assumption debt.

The semantic graph must preserve that distinction.

## Authority handling

Foreign code receives explicit capabilities/authority, not ambient authority inferred from being linked.

The boundary should record:

```text
authority_granted
authority_consumed
authority_borrowed
authority_returned
authority_revoked
```

### Non-amplification

A valid foreign boundary must satisfy:

```text
EffectiveAuthority(foreign_component)
    <=
GrantedAuthority(boundary)
```

unless a separate explicitly modeled authority source exists.

### Namespace is not authority

RFC 0014 remains in force:

```text
CanReference(symbol)
    !=
CanExercise(capability)
```

FFI symbol visibility cannot implicitly grant filesystem, network, device, memory, or process authority.

## Effects and side effects

Foreign operations must declare or conservatively approximate relevant effects.

Examples:

```text
read<File>
write<File>
network_egress
allocate_foreign_heap
mutate_process_environment
change_signal_handler
spawn_thread
perform_device_io
```

Unknown effects must not be treated as pure.

A highly untrusted native library may require a conservative effect contract such as:

```text
Effects = ANY_WITHIN_CONTAINMENT
```

while containment limits the actual externally possible effects.

## Foreign global state

Foreign runtimes may maintain hidden global state.

The boundary must not pretend a call is referentially transparent merely because its arguments/results look pure.

If foreign global state affects observations, the operation should model that state abstractly or conservatively mark the dependence/effect.

Potential semantic representation:

```text
ForeignStateDomain<LibraryFoo>
```

with transitions:

```text
StateBefore -> StateAfter
```

without requiring MNCS to understand every internal byte.

## Signals, interrupts, and asynchronous foreign entry

Some foreign environments may asynchronously enter handlers or mutate externally visible state.

Such behavior should use RFC 0010 event/causality semantics and explicit boundary contracts.

Possible classes:

```text
AsyncEntry {
    signal
    interrupt
    foreign_runtime_callback
    device_callback
}
```

The boundary should state whether these can occur during an operation and which invariants/authority they may observe.

## Foreign allocation and deallocation

Allocators may live in different semantic domains.

A value allocated by foreign allocator `A` should carry deallocation authority compatible with that allocator/domain.

```text
ForeignAllocation {
    allocation_identity
    allocator_domain
    extent
    alignment
    ownership
    free_authority
}
```

MNCS must not assume:

```text
free_with_mncs_allocator(foreign_pointer)
```

is legal.

Likewise, a foreign function that assumes deallocation responsibility must consume or receive the correct deallocation authority.

## Stable address and pinning across foreign calls

Some foreign APIs retain addresses or perform asynchronous DMA/I/O.

The boundary must derive stable-address/pinning requirements rather than assuming stack/local storage remains valid.

Example:

```text
foreign_async_submit(buffer)
```

may require:

```text
Pinned<Buffer, until = CompletionEvent>
StableAddress<Buffer, until = CompletionEvent>
ForeignRetention<Buffer, until = CompletionEvent>
```

This composes RFC 0008, RFC 0009, RFC 0010, and RFC 0011.

## Foreign atomics and memory ordering

Foreign code may access shared memory using a memory model or atomic convention different from MNCS expectations.

The boundary must either:

- establish compatible visibility/order semantics;
- route access through an adapter;
- isolate the memory;
- strengthen synchronization; or
- reject the binding.

A declaration that both sides use `atomic` is not sufficient evidence by itself.

## Constant-time and side-channel properties

Security-sensitive boundaries may carry nonfunctional semantic requirements such as:

```text
constant_time
no_secret_dependent_branch
no_secret_dependent_memory_access
bounded_cache_observation
```

These should be separate evidence dimensions.

Memory safety does not prove constant-time behavior.

A foreign cryptographic provider may therefore have:

```text
memory_safety = PASS
functional_tests = PASS
constant_time = UNKNOWN
```

and be rejected by a consumer requiring constant-time evidence.

## Foreign nondeterminism

Foreign operations may introduce nondeterministic inputs such as:

```text
clock
entropy
OS scheduling
external state
foreign runtime decisions
```

These sources should connect to RFC 0011 observation/nondeterminism semantics rather than silently contaminating deterministic computations.

Where record/replay is supported, the boundary should identify which foreign observations must enter the observation ledger.

## Dynamic loading

A dynamically loaded artifact is a runtime RFC 0014 binding plus an RFC 0015 trust boundary.

Before execution authority is granted, the runtime should be able to check:

```text
interface identity
artifact identity
ABI identity
required authority
boundary evidence
assurance policy
```

A name match or symbol lookup is not sufficient semantic compatibility.

## Boundary evidence composition

Evidence may come from several sources:

```text
formal proof
proof-carrying certificate
binary verifier
source verifier
ABI checker
layout checker
runtime monitor
sandbox enforcement
fuzzing/testing
vendor specification
explicit assumption
```

The final boundary claim should identify which source establishes which property.

Example:

```text
BoundaryPropertyEvidence {
    memory_isolation -> sandbox_certificate
    ABI_layout -> generated_layout_checker
    callback_behavior -> vendor_assumption
    functional_result -> differential_tests
}
```

No one evidence source automatically upgrades unrelated claims.

## Evidence freshness and narrow invalidation

A change to:

```text
foreign library binary
compiler
ABI
adapter
layout
wrapper implementation
sandbox policy
runtime monitor
```

should invalidate exactly the claims whose dependency graph includes that artifact.

If only documentation metadata changes, machine-checked binary-layout evidence need not necessarily become stale.

RFC 0004 dependency granularity should be preserved.

## Diagnostics

Boundary diagnostics should identify the missing or conflicting semantic fact.

Example:

```text
Foreign binding rejected:
    operation: foo.process

Required:
    borrow_for_call(BufferR)

Declared foreign behavior:
    may_retain(BufferR)

Conflict:
    BufferR lifetime ends at call return

Candidate repairs:
    transfer ownership
    extend lifetime
    copy into foreign-owned buffer
    select provider that does not retain
```

Another example:

```text
Boundary cannot establish safe provenance:
    returned address = 0x7f93...

Known:
    address bits

Missing:
    allocation identity
    live epoch
    extent
    authority

Result:
    ForeignAddress only
```

Another:

```text
Unsafe wrapper not promotable:
    unresolved obligation: no concurrent callback

Evidence:
    vendor documentation

Policy:
    verifier_required

Candidate repairs:
    runtime callback guard
    isolate provider
    verify provider implementation
```

## Proof architecture

RFC 0015 should keep the RFC 0007 kernel small.

Conceptually:

```text
boundary obligation
      ->
external prover / verifier / certificate checker /
runtime enforcement proof / containment checker
      ->
evidence artifact
      ->
small MNCS checker / policy decision
```

No foreign verifier becomes language semantics merely because it is commonly used.

The language specifies the obligation and evidence requirements.

## HIR and SSA preservation

Boundary semantics must remain visible through lowering.

HIR/SSA should preserve references to:

```text
boundary identity
foreign operation identity
ABI adapter identity
ownership mode
provenance obligations
authority grant
allowed effects
callback/reentrancy policy
unwind policy
failure mapping
assumption debt
evidence references
```

A backend may lower these to ordinary native calls, shims, process messages, Wasm calls, or target-specific intrinsics, but the transformation provenance must remain traceable.

## Compiler obligations

A lowering across a trust boundary should establish relevant claims such as:

```text
ABIArgumentsPreserveSemanticValues
ABIResultPreservesSemanticResult
OwnershipTranslationValid
AuthorityNotAmplified
RequiredCleanupPreserved
FailureMappingSound
CallbackPolicyPreserved
UnwindPolicyPreserved
```

The exact theorem family remains open.

A useful umbrella relation is:

```text
BoundaryRealizationSatisfies(
    semantic_boundary,
    target,
    adapter,
    foreign_artifact,
    containment,
    evidence
)
```

## Forge optimization/search space

Forge may explore boundary realizations including:

```text
raw direct call
checked wrapper
copy-in/copy-out
pinned shared buffer
shared-memory bridge
Wasm component
SFI compartment
separate process
capability compartment
VM boundary
remote/proxy boundary where permitted
```

and adaptation choices such as:

```text
copy vs borrow
marshal vs shared representation
callback vs polling
blocking worker vs direct call
error translation strategy
runtime guards
```

Requirements constrain the legal search space.

Preferences may include:

```text
latency
throughput
copy count
memory
energy
startup cost
proof cost
attack surface
authority surface
```

The fastest unsafe realization is not eligible if it violates protected properties.

## Relationship to RFC 0007

RFC 0007 defines the trusted proof core.

RFC 0015 uses that boundary without importing arbitrary foreign reasoning into the kernel.

Foreign assumptions, certificates, verifier outputs, and runtime guards remain typed evidence classes.

## Relationship to RFC 0008

RFC 0008 provides:

```text
authority
resources
effects
protocols
events
```

RFC 0015 specifies how those cross or are attenuated at foreign boundaries.

Foreign code cannot receive authority merely because it receives an address or symbol.

## Relationship to RFC 0009

RFC 0009 provides:

```text
identity
allocation epochs
provenance
permissions
lifetime
pinning
stable address
```

RFC 0015 preserves those distinctions across FFI and explicitly models where the foreign domain cannot provide equivalent guarantees.

## Relationship to RFC 0010

RFC 0010 governs:

```text
concurrency
event order
visibility
progress
fairness
```

Foreign blocking, callbacks, worker creation, atomics, async completion, and reentrancy must participate in those semantics.

## Relationship to RFC 0011

RFC 0011 governs:

```text
failure
uncertainty
retry
recovery
external observation
```

Foreign status codes and exceptions are representations that must be mapped into those semantic outcomes.

## Relationship to RFC 0012

RFC 0012 provides the executable semantic machine.

A foreign operation is a typed semantic request interpreted by a boundary/world relation rather than an unmodeled host-language side effect.

## Relationship to RFC 0013

RFC 0013 provides abstraction and evidence semantics.

Unsafe implementations may refine safe interfaces when boundary obligations are satisfied.

Foreign wrappers may hide ABI details while preserving their semantic assumptions/evidence dependencies.

## Relationship to RFC 0014

RFC 0014 provides component/linking semantics.

Foreign providers are components whose bindings include RFC 0015 boundary contracts.

Dynamic loading is runtime binding plus trust-boundary validation.

Adapters should be explicit components where composition-level identity matters.

## Experimental tracks

### Experiment 1 — unsafe wrapper closure

Implement a small unsafe primitive behind a safe abstraction.

Requirements:

- generate explicit obligations for the unsafe operation;
- discharge them with an independently checkable method;
- expose only the safe contract to callers;
- invalidate the safe-wrapper evidence after changing the unsafe implementation.

### Experiment 2 — foreign memory ownership

Bind to a foreign library with variants that:

```text
borrow_for_call
retain_pointer
consume_pointer
```

Verify that MNCS lifetime/ownership state differs correctly for each case.

### Experiment 3 — provenance import

Return raw addresses from a foreign stub.

Verify:

- raw address is not accepted as ordinary `Reference<T>`;
- valid reference import requires allocation/extent/lifetime/authority evidence;
- stale allocation epochs remain rejected even when the address is reused.

### Experiment 4 — dual ABI realization

Implement one semantic operation through two ABI realizations.

For example:

```text
native x86_64 ABI
Wasm/canonical component ABI
```

or two native target ABIs.

Check that both refine the same semantic operation while maintaining distinct ABI/build identities.

### Experiment 5 — callback and reentrancy

Use a foreign provider that unexpectedly invokes a callback.

A declaration that forbids callbacks must fail.

An explicit callback contract should allow the interaction only when authority, lifetime, task scope, and reentrancy obligations are valid.

### Experiment 6 — unwind boundary

Inject a foreign nonlocal exit.

Verify that an unsupported unwind crossing is rejected and that an explicit adapter can translate the foreign event into an RFC 0011 failure while preserving cleanup obligations.

### Experiment 7 — assumption debt

Wrap a foreign operation using documented-but-unproved assumptions.

Requirements:

- produce explicit assumption-debt records;
- show dependent safe claims;
- replace one assumption with a runtime guard;
- demonstrate narrow evidence invalidation rather than full-program invalidation.

### Experiment 8 — proof-carrying/certified artifact

Create or emulate a foreign artifact plus certificate for a small safety policy.

Verify the certificate before granting the authority permitted by the policy.

Use wrong-artifact and stale-certificate negative cases.

### Experiment 9 — containment tournament

Run the same semantic foreign component through several realization strategies:

```text
direct raw FFI
checked in-process wrapper
Wasm/SFI-style isolation
separate process
```

Compare:

```text
latency
copying
memory
attack surface
authority surface
evidence cost
```

Reject candidates that cannot satisfy hard containment requirements.

### Experiment 10 — dynamic compromise

Use a deliberately vulnerable foreign component.

Grant only:

```text
Read<Input>
Write<Output>
```

Then deliberately compromise it.

Required property:

```text
component may become arbitrary
but cannot:
    read Secret
    write Config
    open Network
    forge valid MNCS provenance
    acquire additional authority
```

This is a central experiment.

### Experiment 11 — boundary evidence freshness

Generate boundary evidence tied to:

```text
artifact
ABI
target
adapter
toolchain
```

Change each independently and verify only relevant evidence becomes stale.

### Experiment 12 — foreign ambiguous outcome

Create a foreign operation that commits an external effect and loses acknowledgment.

Verify the adapter produces:

```text
MAY_HAVE_APPLIED / UNKNOWN
```

rather than ordinary failure, preserving RFC 0011 retry safety.

## Mandatory negative tests

The RFC prototype should reject or conservatively classify at least the following cases.

- `unsafe` keyword treated as proof.
- foreign call assumed pure because no effects were declared.
- foreign pointer retained despite `borrow_for_call` contract.
- MNCS deallocator used on foreign allocation without compatible free authority.
- raw integer/address promoted directly to safe reference.
- stale foreign pointer resurrected after address reuse.
- callback occurs when callbacks are declared forbidden.
- callback duplicates linear authority.
- reentrant callback observes temporarily broken invariant.
- foreign unwind bypasses resource cleanup.
- `longjmp`-like escape bypasses borrow termination.
- blocking foreign call treated as nonblocking without evidence.
- foreign thread creation omitted from effect/concurrency model.
- timeout translated to definite non-commit without evidence.
- cancellation request treated as confirmed cancellation.
- foreign error code mapped to retry-safe failure without retry evidence.
- provider gains filesystem/network authority from namespace visibility.
- compromised component escapes declared containment authority.
- runtime guard failure is ignored.
- assumption debt disappears behind a safe wrapper without accepted disposition.
- proof/certificate for artifact A reused for artifact B.
- ABI evidence reused after layout/target change.
- two different ABI realizations accidentally share physical realization identity.
- representation conversion permits invalid discriminant.
- NUL-terminated adapter permits interior-NUL semantic mismatch without declared policy.
- callback function pointer outlives borrowed MNCS closure.
- foreign atomics assumed compatible solely because both sides use the word `atomic`.
- foreign global state omitted from determinism/effect reasoning.
- `UNKNOWN` memory behavior used to prove disjointness.
- `UNKNOWN` effect behavior used to prove confinement.
- empirical tests upgraded to formal proof.
- vendor documentation silently treated as machine-checked certificate.
- content hash treated as proof of semantic correspondence.
- process sandbox treated as proof of foreign functional correctness.

## Evaluation criteria

RFC 0015 experiments should measure whether the proposed model can:

1. distinguish unsafe from foreign, untrusted, unverified, ABI-bound, and target-specific behavior;
2. preserve explicit ownership/lifetime/provenance across foreign calls;
3. represent callbacks/reentrancy/unwind without hidden control flow;
4. preserve RFC 0011 ambiguity and retry semantics;
5. track assumption debt and blast radius precisely;
6. invalidate evidence narrowly after ABI/artifact changes;
7. enforce authority non-amplification after dynamic compromise;
8. support at least two different boundary realizations for one semantic interface;
9. permit Forge to compare containment strategies without changing semantic identity;
10. produce useful machine-readable diagnostics for missing boundary facts; and
11. avoid placing arbitrary foreign semantics inside the trusted proof kernel.

## Open questions

The following remain deliberately open for experiment.

1. What is the minimal canonical trust-boundary object required by HIR/SSA?
2. Should `unsafe` be a source keyword, a capability, an evidence-mode construct, or several representations of one canonical operation?
3. Which boundary obligation classes should be kernel-checkable versus certificate/verifier checked?
4. What is the minimal foreign-memory/provenance import calculus?
5. How should arbitrary C aliasing claims be represented without over-committing to C semantics?
6. Should foreign state domains be modeled as abstract resources, effects, or dedicated world-state nodes?
7. How should callbacks compose with structured concurrency and cancellation?
8. What should be the normative rule for cross-boundary panics/exceptions where the target ABI technically permits them?
9. Which containment mechanisms can support proof-carrying authority non-amplification?
10. How should hardware capability evidence be represented portably?
11. How should foreign binary verification results compose with source-level wrapper proofs?
12. When may a runtime guard legitimately upgrade a property from `UNKNOWN` to enforced?
13. How should side-channel properties be scoped and invalidated across ABI/toolchain changes?
14. Should remote/process/device interfaces eventually share the same generalized boundary calculus?
15. How much C/C++ object-model semantics should MNCS model directly versus isolate behind explicit adapters?
16. How should target ABI specifications themselves be versioned and evidenced?
17. How should dynamically loaded providers publish trustworthy manifests before capability grant?
18. Which boundary states may safely be elided from production runtime after compile-time proof?

## Likely handoff

After RFC 0015, the next major foundational work should investigate **staging, metaprogramming, introspection, code generation, and self-transformation semantics**.

That work should build on the rule established here that generated boundary adapters, shims, manifests, and proofs remain explicit semantic artifacts rather than untracked compiler magic.

## Research influences

This RFC is informed by, but does not normatively adopt wholesale:

- semantic typing and RustBelt-style reasoning for unsafe abstractions;
- linking types for cross-language semantic exposure;
- CompCert-style relational external-call semantics;
- typed assembly languages;
- proof-carrying code;
- capability-machine and CHERI-style bounded authority;
- robustly safe compilation;
- dynamic-compromise secure-compilation models;
- WebAssembly Component Model / canonical ABI ideas;
- separation logic and provenance reasoning;
- session/protocol semantics;
- capability and effect systems; and
- the prior MNCS RFCs for evidence, authority, memory, concurrency, failure, execution, abstraction, and composition.

These are evidence and design resources, not additions to the RFC 0007 proof kernel by fiat.

## Final thesis

> **MNCS treats unsafe and foreign execution as explicit trust-boundary transitions. A boundary records which guarantees survive, which weaken, which become assumptions, which are restored by translation or runtime enforcement, what authority is delegated, how representations and provenance cross domains, and what evidence justifies re-entry into the safe semantic world. Unsafe code is therefore not outside MNCS semantics; it is where MNCS semantics become most explicit.**
