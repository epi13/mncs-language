# RFC 0024: Machine-Native Information Flow, Confidentiality, Integrity, Declassification, and Side-Channel Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.2–1.0
- **Depends on:** RFC 0001, RFC 0002, RFC 0003, RFC 0004, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0013, RFC 0014, RFC 0015, RFC 0016, RFC 0017, RFC 0018, RFC 0019, RFC 0020, RFC 0021, RFC 0022, RFC 0023

## Summary

MNCS Language should treat information security as an explicit semantic discipline over **permitted influence, permitted observation, deliberate release, integrity elevation, observer/channel models, and evidence**, rather than as a collection of access-control checks or post-hoc security annotations.

The central rules are:

> **Security policy attaches to information and influence, not merely to storage locations, objects, or APIs.**

> **Confidentiality constrains who or what may learn information; integrity constrains who or what may influence information. These are orthogonal policy dimensions.**

> **Every information-flow guarantee is relative to an explicit observer, observation model, channel model, and release policy. There is no universal `secure = true`.**

A value may be unreadable to a principal yet still leak through control flow, termination, timing, failures, allocation, scheduling, message size, or other observable behavior. Conversely, a value may be directly readable but still subject to downstream dissemination restrictions. Access authority and information-flow authority therefore remain distinct.

RFC 0024 establishes a machine-native framework in which MNCS can express and reason about:

- confidentiality and integrity policies;
- policy lattices and explicit `can-flow-to` relations;
- data, control, effect, failure, temporal, scheduling, and resource influence;
- explicit observer and attacker models;
- semantic noninterference and related relational security properties;
- termination-sensitive versus termination-insensitive security;
- timing, memory-address, scheduling, and other side-channel observations;
- declassification and endorsement as bounded reclassification events;
- robust declassification, transparent endorsement, and stronger nonmalleable-flow profiles;
- dynamic security policies and runtime-created label contexts;
- static, dynamic, hybrid, and multi-execution enforcement strategies;
- quantitative leakage claims where strict noninterference is intentionally too strong;
- security-preserving lowering and transformation relations;
- security counterexamples and relation deltas;
- target-sensitive side-channel evidence;
- assurance-gated security claims through RFC 0018; and
- bounded Forge search over security-preserving realizations.

This RFC does not define the final principal/authentication/credential/attestation model. Those concerns are reserved for RFC 0025. RFC 0024 may refer to abstract principal or authority identities where required to state ownership and release requirements, but it does not decide how those identities are authenticated.

---

## Motivation

MNCS already has most of the semantic machinery needed to make information security substantially more precise than conventional language-level security annotations:

- RFC 0008 provides explicit effects and capabilities;
- RFC 0009 provides provenance and storage identity;
- RFC 0010 provides concurrency, causality, and scheduler-visible behavior;
- RFC 0011 provides failures, nondeterminism, and external observations;
- RFC 0015 provides unsafe/foreign trust boundaries and containment;
- RFC 0017 provides runtime and target identities;
- RFC 0018 provides evidence, trust, freshness, and action-relative authorization;
- RFC 0020 provides typed relations and explicit observation models;
- RFC 0022 provides termination/progress semantics; and
- RFC 0023 provides timing, clock, deadline, and temporal-observation semantics.

Without RFC 0024, those mechanisms still lack one constitutional answer to questions such as:

- if secret data controls a branch but is never copied into a public value, did information flow?
- if two programs return the same result but one takes secret-dependent time, which security relation fails?
- if a value is encrypted, hashed, aggregated, or compressed, has confidentiality been weakened, preserved, or transformed?
- who may intentionally release a function of protected information?
- what exactly is being released: a value, a predicate, an equivalence class, or a bounded amount of information?
- when is attacker influence strong enough to make declassification unsafe?
- when may validated untrusted input be endorsed as sufficiently high integrity?
- how do runtime-created tenants, plugins, sandboxes, and principals affect flow policy?
- how should security policy survive HIR, SSA, backend lowering, optimization, and target realization?
- what does “constant time” mean relative to branches, memory addresses, clocks, microarchitecture, or physical leakage?
- how should finite empirical side-channel evidence differ from a universal noninterference theorem?
- how can Forge search secure implementations without laundering successful tests into downgrade authority?

Conventional systems frequently answer these questions through combinations of:

- ACLs;
- trusted framework conventions;
- taint bits;
- ad-hoc `secret` annotations;
- sanitizer lists;
- cryptographic assumptions;
- manually reviewed declassification points;
- ambient timing assumptions; and
- broad claims such as “constant time” or “side-channel safe.”

Those mechanisms can be useful, but they conflate **policy**, **semantic property**, **enforcement mechanism**, **observer model**, and **evidence**.

MNCS should separate them.

---

# Constitutional principles

## 1. Access is not flow

> **Permission to read an object does not automatically authorize arbitrary dissemination of information derived from that object.**

Access control answers whether an operation may directly access a resource. Information-flow policy answers how information may influence future values, effects, traces, and observations after access occurs.

## 2. Confidentiality is not integrity

> **Confidentiality and integrity are distinct semantic dimensions.**

Confidentiality constrains observation.

Integrity constrains influence.

A value may be highly confidential but low integrity, highly trusted but public, both, or neither.

## 3. Security labels are policy, not truth

> **A label such as “trusted” does not establish that a value is correct.**

Integrity policy expresses who or what may influence a value, not whether the value's proposition is true.

Truth/evidence remains governed by RFC 0018 and the relevant verifier semantics.

## 4. Derived information carries policy

> **Computation does not erase information-flow obligations.**

If a result semantically depends on protected inputs, policy must follow that influence unless an explicit authorized reclassification applies.

## 5. Control flow carries information

> **Implicit control dependence is information influence.**

A public action selected by a secret branch may reveal the secret even when no explicit secret-to-public assignment occurs.

## 6. Effects carry information

A protected value may influence:

```text
file access
network traffic
messages
resource acquisition
clock reads
logging
exceptions
scheduler requests
foreign calls
```

and therefore create an observable flow.

## 7. Termination can carry information

RFC 0022 termination/divergence/progress behavior may be part of an observer model.

Termination-sensitive and termination-insensitive security claims remain distinct.

## 8. Time can carry information

RFC 0023 timing observations may be information channels.

Functional equivalence does not imply timing noninterference.

## 9. Failure can carry information

RFC 0011 failure variants, traps, retries, recovery paths, or visible error classes may reveal protected information.

## 10. Scheduling can carry information

RFC 0010 scheduler-visible ordering, interleavings, completion order, contention, and synchronization behavior may form observable channels.

## 11. Resource behavior can carry information

Allocation count, memory footprint, output size, retry count, CPU work, energy, or other resource behavior may become observable channels where the observer model includes them.

Detailed quantitative resource semantics remain reserved for RFC 0032.

## 12. No universal side-channel-free claim

> **A side-channel claim is valid only for its declared observer, channel, environment, and target model.**

Proof of branch-trace independence does not establish power-analysis resistance.

## 13. Constant-time is a scoped relational property

MNCS should not interpret `constant_time` as literal equality of wall-clock duration unless that is explicitly the claimed relation.

A practical security contract may instead require secret independence of specified digital observations such as:

```text
branch trace
memory-address trace
instruction-class trace
message length
```

## 14. Declassification is not a cast

> **Confidentiality downgrade is an authorized semantic release event.**

It must identify what information is released and under which authority/scope.

## 15. Endorsement is not a cast

> **Integrity upgrade is an authorized semantic reclassification event.**

It must identify why a lower-integrity influence is now acceptable for a higher-integrity context.

## 16. Validation is not endorsement

A verifier may establish a property of untrusted data without granting authority to upgrade the data's integrity label.

## 17. Reclassification authority is bounded

Authority to release one function of a secret does not imply authority to release the underlying secret.

Authority to endorse one validated property does not imply authority to trust arbitrary derived data.

## 18. Declassification must resist attacker influence where required

Where a confidentiality downgrade occurs in an attacker-influenced context, the policy may require robust-declassification guarantees.

## 19. Security inference may strengthen protection but may not fabricate downgrade authority

> **The compiler, Forge, or an analyzer may infer that stronger confidentiality or weaker integrity is required. It may not infer authority to weaken confidentiality or raise integrity merely because testing looks safe.**

## 20. Encryption does not automatically declassify

Cryptographic transformation does not erase information-flow policy.

Whether ciphertext may be exposed depends on an explicit release policy and the relevant cryptographic assumptions/evidence.

## 21. Hashing does not automatically declassify

A hash can reveal information, especially for small or guessable secret domains.

Hashing is computation, not release authority.

## 22. Aggregation does not automatically declassify

A mean, count, histogram, summary, or compressed representation may be intentionally releasable, but that is a policy decision.

## 23. Finite observation remains finite evidence

Millions of paired executions without observed leakage do not establish universal noninterference.

## 24. Security claims may be target-sensitive

A source-level proof about control/address traces may remain valid across targets; a microarchitectural timing claim may not.

Target dependence must be explicit.

## 25. Functional preservation does not imply security preservation

A lowering or optimization may be functionally correct while introducing a new observable security channel.

## 26. Security counterexamples are relation-scoped

A timing counterexample refutes a timing-sensitive security claim. It does not automatically refute a narrower value-only claim.

## 27. Policy change does not erase observer knowledge

Revoking future flow authority cannot retroactively make already learned information unknown.

## 28. Observer power is explicit

No security property may silently assume which traces, clocks, failures, addresses, or resources an attacker can observe.

## 29. Provenance is not flow authority

Knowing where data came from does not establish where it is permitted to go.

## 30. Security evidence is not security policy

Evidence may establish that a candidate satisfies a policy. Evidence must not invent or weaken the policy itself.

---

# Scope

RFC 0024 owns:

- information-flow policy algebra;
- confidentiality and integrity dimensions;
- flow labels and policy composition;
- information influence through values, control, effects, failure, termination, timing, scheduling, and declared resource channels;
- observer and channel models for security claims;
- semantic noninterference families;
- declassification and endorsement semantics;
- robust/nonmalleable reclassification profiles;
- dynamic flow-policy semantics;
- quantitative leakage claim structure;
- side-channel claim scoping;
- security-preserving lowering obligations;
- security counterexamples and deltas;
- security evidence integration with RFC 0018; and
- bounded machine-native security realization search.

RFC 0024 does not finalize:

- principal authentication;
- credential formats;
- certificate-chain semantics;
- remote attestation protocols;
- cryptographic algorithm correctness;
- key lifecycle and cryptographic trust roots;
- durable erasure;
- persistent database transaction semantics;
- detailed resource-cost semantics;
- final source syntax; or
- one mandatory IFC enforcement technique.

---

# Seven semantic planes

RFC 0024 proposes seven distinct security planes.

## Policy plane

Defines:

```text
confidentiality policy
integrity policy
flow ordering
policy composition
ownership/authority references
reclassification requirements
```

The policy plane states what information/influence is permitted.

## Influence plane

Defines semantic dependence channels such as:

```text
data dependence
control dependence
effect dependence
failure dependence
termination dependence
temporal dependence
scheduling dependence
resource dependence
```

The influence plane states what can affect what.

## Observer plane

Defines:

```text
observer identity/class
visible values
visible traces
visible failures
visible timing
visible addresses
visible schedule events
visible resource facts
```

The observer plane states what distinctions an attacker or relying principal can make.

## Relational-security plane

Defines security properties over one or more executions:

```text
noninterference
termination-sensitive noninterference
termination-insensitive noninterference
observational determinism
security refinement
quantitative leakage
```

## Reclassification plane

Defines intentional policy changes:

```text
declassification
endorsement
policy expiration
policy revocation
integrity downgrade
erasure intent
```

## Realization plane

Defines implementation/enforcement strategies:

```text
static IFC
dynamic IFC
hybrid IFC
proof-carrying IFC
secure multi-execution
isolation
oblivious algorithms
constant-observation realizations
```

The realization plane must not redefine the policy.

## Assurance plane

Defines:

```text
proof artifacts
certificates
verifier results
empirical observations
target applicability
freshness
promotion policy
```

through RFC 0018.

---

# Candidate semantic objects

A future canonical representation may include objects conceptually similar to:

```text
FlowLabel
FlowPolicy
FlowOrder
FlowJoin
FlowMeet
ConfidentialityPolicy
IntegrityPolicy
PolicyOwnerRef
AuthorityRequirement

InfluenceEdge
InfluenceGraph
ControlContext
ProgramCounterLabel

ObserverModel
ChannelModel
AttackerModel
ObservationProjection
KnowledgePartition

InformationFlowClaim
NoninterferenceClaim
ObservationalDeterminismClaim
QuantitativeLeakageClaim

DeclassificationPolicy
DeclassificationEvent
ReleaseFunction
ReleaseRelation
EndorsementPolicy
EndorsementEvent
ReclassificationClaim

DynamicLabelContext
CurrentFlowContext
ClearanceBound

SecurityCertificate
SecurityCounterexample
SecurityDelta
EmpiricalLeakageObservation
SideChannelClaim
```

These names are provisional. The semantic distinctions are not.

---

# Part I — policy lattices and flow ordering

## Flow relations

RFC 0024 should support an abstract relation:

```text
L1 can_flow_to L2
```

or symbolically:

```text
L1 ⊑ L2
```

The exact orientation must be standardized once the canonical model is implemented.

The semantic meaning should be unambiguous:

> information protected by `L1` may influence a sink protected by `L2` without reclassification.

## Join

When information from two differently protected values is combined, the result normally requires a policy at least as restrictive as both sources.

Conceptually:

```text
label(f(x, y)) >= join(label(x), label(y))
```

where `join` denotes the least policy capable of protecting both influences.

## Meet

A policy lattice may also provide a meet where useful for policy reasoning.

The existence of a lattice operation does not create authority to declassify.

## Monotone inference

Ordinary dependency propagation should be monotone in protection:

```text
more protected input
    ->
result cannot become less protected merely by inference
```

The only deliberate exceptions are explicit reclassification operations with authority.

---

# Part II — confidentiality policy

## Confidentiality as observer restriction

A confidentiality policy constrains which observers may distinguish information.

Conceptually:

```text
ConfidentialityPolicy {
    owner_refs
    permitted_observer_refs
    release_requirements
}
```

This is not necessarily the final representation.

## Decentralized ownership

MNCS should not require one global hierarchy such as:

```text
PUBLIC < CONFIDENTIAL < SECRET < TOP_SECRET
```

Such hierarchies may be represented as one policy family, but the general model should support decentralized policy ownership.

Example:

```text
policy A:
    owner = Payroll
    readers = {Payroll, TaxService}

policy B:
    owner = Legal
    readers = {Legal}
```

A result influenced by both must preserve both policy constraints unless authorized release occurs.

## Multiple policy owners

Policy composition should permit multiple owners to retain independent authority over information.

The compiler must not collapse joint ownership into one arbitrary principal.

---

# Part III — integrity policy

## Integrity as influence restriction

An integrity policy constrains whose influence may contribute to a value, effect, or protected decision.

Conceptually:

```text
IntegrityPolicy {
    accepted_influencers
    endorsement_requirements
}
```

## High confidentiality, low integrity

A system must be able to represent:

```text
secret but attacker-influenced
```

This combination is especially important around declassification.

## High integrity, public

Likewise:

```text
public but high-integrity configuration
```

is common.

Confidentiality and integrity therefore must not share one scalar security level.

## Integrity is not truth

A high-integrity value may still be factually wrong due to trusted bugs or bad assumptions.

Truth/evidence remains separate.

---

# Part IV — abstract principal references

RFC 0024 needs abstract principal/authority references for policy ownership and reclassification requirements.

Candidate concepts include:

```text
PrincipalRef
PolicyOwnerRef
ReleaseAuthorityRef
EndorsementAuthorityRef
```

RFC 0024 does not decide how these identities are authenticated.

RFC 0025 will define the principal/credential/attestation trust model.

A flow policy may therefore say:

```text
release requires AuthorityRef A
```

without defining how A proves itself.

---

# Part V — influence semantics

Information flow should be based on semantic influence rather than only assignment syntax.

## Data dependence

Explicit data flow is the simplest case:

```text
public = secret
```

The resulting public value is influenced by the secret.

## Control dependence

Example:

```text
if secret {
    public = 1
} else {
    public = 0
}
```

The result is secret-influenced even though the secret is not directly assigned.

## Program-counter context

A practical canonical model may track a control security context similar to a program-counter label:

```text
ControlContext {
    influencing_policies
}
```

An operation executed under a secret-influenced branch inherits the relevant control influence unless analysis proves independence.

The precise implementation may use labels, control-dependence graphs, relational analysis, or another verified mechanism.

## Effect dependence

Protected values may influence which effects occur.

Example:

```text
if secret {
    send(packet_A)
} else {
    send(packet_B)
}
```

Even if packet payloads are public, effect selection reveals information.

## Failure dependence

Protected values may influence:

```text
which exception occurs
whether an exception occurs
retry count
rollback path
fatal vs recoverable outcome
```

These are flow channels where visible.

## Termination dependence

Protected values may influence:

```text
return
divergence
quiescence
progress
```

RFC 0022 supplies the underlying completion semantics.

## Temporal dependence

Protected values may influence:

```text
completion time
logical effect time
deadline result
jitter
clock-observable behavior
```

RFC 0023 supplies the temporal semantics.

## Scheduling dependence

Protected values may influence:

```text
which thread wins
which task runs first
lock acquisition order
message order
preemption
```

RFC 0010 supplies concurrency/scheduler semantics.

## Resource dependence

Protected values may influence:

```text
allocation count
memory footprint
output length
number of requests
CPU work
energy
cache occupancy
```

These are potential channels when declared observable.

---

# Part VI — observer and attacker models

## Observer identity/class

Security claims must identify whose observations matter.

An observer can be:

```text
a principal
a component
a sandbox
a network peer
a remote tenant
a co-resident process
a hardware adversary class
```

The final taxonomy remains open.

## Channel model

A channel model enumerates observations available to the observer.

Candidate observations include:

```text
returned values
emitted messages
message size
message timing
failure class
termination
progress
logical time
physical time
branch trace
instruction-class trace
memory-address trace
allocation behavior
scheduler events
cache-visible events
network metadata
power
EM
acoustic
thermal
```

A claim should not implicitly include channels absent from its model.

## Observation projection

Conceptually:

```text
observe(observer, channel_model, execution)
```

projects an execution into the information available to that observer.

RFC 0020 provides the general observation/relation framework.

## Attacker knowledge

An observer can be modeled abstractly by which possible secret states remain distinguishable after observation.

This supports declassification and quantitative information-flow semantics naturally.

---

# Part VII — semantic noninterference

## Basic idea

Noninterference compares executions that differ in protected information while keeping public/relevant inputs equivalent.

Conceptually:

```text
public_equivalent(s1, s2)
protected(s1) may differ from protected(s2)
```

requires:

```text
observe_O(run(s1))
    equivalent_to
observe_O(run(s2))
```

under the declared observation relation.

## Noninterference is relational

It is not a single-trace invariant.

The claim compares multiple executions and therefore belongs to the hyperproperty/relational-security family.

## Hyperproperties

RFC 0024 should explicitly recognize that some security properties quantify over sets or tuples of traces.

Candidate distinction:

```text
SingleTraceSecurityClaim
RelationalSecurityClaim
HyperpropertyClaim
```

Exact naming remains open.

## Observer-relative noninterference

A noninterference claim is incomplete without:

```text
observer
observation model
secret equivalence/public equivalence
execution model
```

---

# Part VIII — termination-sensitive security

## Termination-insensitive noninterference

A termination-insensitive relation may ignore distinctions caused only by nontermination or only compare terminating runs under a declared interpretation.

It is useful but weaker than termination-sensitive security.

## Termination-sensitive noninterference

If an attacker can observe whether computation terminates, the security relation must preserve that observation.

Example:

```text
if secret {
    loop_forever()
} else {
    return 0
}
```

A value-only observer may learn nothing from returned values.

A termination observer can learn the secret.

## No implicit upgrade

A proof of termination-insensitive noninterference must never be presented as termination-sensitive security.

RFC 0020 relation identity should preserve this distinction.

---

# Part IX — timing-sensitive security

## Timing as observation

A program may return identical values while leaking through completion time.

Example:

```text
if secret {
    expensive()
} else {
    cheap()
}
```

Timing-sensitive noninterference adds RFC 0023 timing observations to the observer model.

## Clock scope

Timing claims must identify the relevant clock/temporal model.

A claim about logical step count is not automatically a wall-clock claim.

## Uncertainty

A physical timing observer may have measurement uncertainty.

Quantitative leakage or distinguishability claims must account for that uncertainty where relevant.

---

# Part X — concurrent information flow

## Scheduler nondeterminism

Concurrent programs may leak through nondeterministic scheduling even when individual operations appear secure.

Example:

```text
secret controls which thread becomes enabled first
```

while both executions eventually compute the same ordinary return value.

## Observational determinism

RFC 0024 should reserve a stronger concurrency-sensitive relation such as:

```text
ObservationalDeterminism<ObserverModel>
```

where visible behavior is deterministic modulo the declared low-equivalence relation despite internal scheduling nondeterminism.

## Progress classes

Wait-free, lock-free, starvation-free, or deadlock behavior may itself be observable and therefore security relevant under some observer models.

RFC 0022 owns those progress properties.

---

# Part XI — declassification

Strict noninterference is intentionally too strong for many real programs.

Systems must intentionally release selected information.

## Declassification as explicit release

Conceptually:

```text
DeclassificationPolicy {
    source_policy
    destination_policy
    release_description
    authority_requirement
    location_scope
    temporal_scope
    integrity_requirement
}
```

## Four dimensions

RFC 0024 should model the classic dimensions:

```text
what may be released
who may release it
where release may occur
when release may occur
```

## What

The policy should identify the actual information release.

Examples:

```text
password == guess
salary > threshold
x mod 2
aggregate(data)
```

rather than:

```text
declassify(all_secret_state)
```

unless broad release is genuinely intended.

## Release functions

A release function can specify exactly which function of protected data becomes observable.

Example:

```text
ReleaseFunction(secret) = (secret == candidate)
```

## Release relations

An alternative representation uses an equivalence relation over secret states.

Example:

```text
x ~ y iff x mod 2 == y mod 2
```

The observer may distinguish even from odd, but must not distinguish values within the same class.

This representation connects directly to RFC 0020.

## Who

Declassification authority must be scoped to an abstract authority/principal reference.

RFC 0025 later defines authentication of that authority.

## Where

A policy may allow release only through designated functions, modules, boundaries, or transformation points.

## When

A policy may permit release only during a temporal interval or after an explicit condition.

RFC 0023 provides temporal validity semantics.

## Declassification event artifact

A successful release should produce an auditable semantic artifact:

```text
DeclassificationEvent {
    subject
    source_policy
    destination_policy
    released_information
    authority_ref
    program_location
    temporal_context
    integrity_context
    justification
    evidence
}
```

---

# Part XII — robust declassification

## Attacker influence on release

A dangerous system may have a valid release function but allow low-integrity attacker influence to control what secret is released or when release occurs.

## Robustness requirement

A policy may therefore require:

```text
RobustDeclassification
```

meaning attacker-controlled influence cannot manipulate the downgrading mechanism to expose information beyond the authorized release policy.

## Integrity dependency

This creates a constitutional interaction:

> confidentiality downgrade may require sufficient integrity in the value, release condition, and control context performing the downgrade.

## Example

Unsafe conceptually:

```text
attacker_input chooses which secret field is declassified
```

Safe only if the release policy explicitly permits that choice or if attacker influence is otherwise constrained by the robustness relation.

---

# Part XIII — endorsement

## Integrity reclassification

Endorsement is the integrity-side dual of declassification.

Conceptually:

```text
EndorsementPolicy {
    source_integrity
    destination_integrity
    required_properties
    authority_requirement
    location_scope
    temporal_scope
}
```

## Validation then endorsement

Typical flow:

```text
untrusted input
    -> validation evidence
    -> authorized endorsement
    -> higher-integrity value
```

Validation alone does not perform the policy change.

## Endorsement event

```text
EndorsementEvent {
    subject
    source_integrity
    destination_integrity
    validation_claims
    authority_ref
    control_context
    evidence
}
```

## Over-broad endorsement

A validator establishing:

```text
is_valid_identifier(x)
```

must not automatically authorize:

```text
trusted_admin_command(x)
```

unless the endorsement policy explicitly establishes that stronger relationship.

---

# Part XIV — nonmalleable information flow

RFC 0024 should reserve stronger combined confidentiality/integrity policies that require:

- robust confidentiality downgrading; and
- integrity upgrading that remains transparent to the relevant attacker/observer model.

A candidate profile may be named conceptually:

```text
NonmalleableInformationFlow
```

This should not be the mandatory baseline for every program.

It should be available for high-assurance boundaries where declassification and endorsement interact with adversarial influence.

---

# Part XV — policy change and temporal validity

## Dynamic policy validity

A flow policy may have:

```text
valid_from
valid_until
```

or another RFC 0023 temporal applicability condition.

## Examples

```text
embargo expires
user revokes sharing consent
investigation closes
configuration trust expires
key compromise becomes known
```

## Revocation

Revocation can stop future authorized flow.

It cannot guarantee that previous observers forget already learned information.

## Integrity downgrade

New evidence may require reducing trust in values derived from a compromised principal, key, verifier, or component.

RFC 0018 evidence invalidation can trigger such policy reassessment.

## Confidentiality erasure intent

A future system may represent a requirement that some information become inaccessible after a condition.

Durable erasure semantics are deferred to RFC 0026.

---

# Part XVI — static information-flow control

Static IFC is one realization strategy.

A compiler or verifier may infer labels, control dependencies, and allowed flows before execution.

Benefits include:

- no runtime label-check overhead;
- no runtime label-check channel for fully static cases;
- earlier diagnostics;
- easier proof composition.

Limitations include dynamic principals, runtime-created resources, and target-specific channels.

RFC 0024 does not mandate static IFC.

---

# Part XVII — dynamic information-flow control

Dynamic IFC is another realization strategy.

A runtime may track:

```text
current flow context
current label
clearance bound
object labels
```

as information is observed.

## Floating security context

A computation that observes more sensitive information may raise its current confidentiality context.

Likewise integrity context may weaken as lower-integrity inputs are observed.

## Clearance

A runtime clearance bound may cap how far a dynamic context is allowed to change.

This is useful for containment and denial-of-service control.

## Dynamic checks are observable

Success/failure/timing of dynamic checks may themselves be channels.

Therefore dynamic IFC enforcement remains inside the security semantics rather than outside it.

---

# Part XVIII — hybrid IFC

Many MNCS programs will benefit from combining:

```text
static proof for stable policy
runtime checks for dynamic policy
proof obligations for exceptional cases
isolation for foreign code
```

RFC 0024 should explicitly allow hybrid enforcement.

Forge may select among enforcement realizations under an unchanged security contract.

---

# Part XIX — secure multi-execution and alternative enforcement

Secure multi-execution and related techniques can enforce noninterference by evaluating different security projections rather than only propagating labels.

RFC 0024 should treat such methods as realization strategies.

The property is:

```text
required noninterference relation
```

The realization may be:

```text
static labels
dynamic labels
multi-execution
isolation
proof-carrying transformation
```

as long as the required relation is evidenced.

---

# Part XX — taint analysis

## Taint as approximation

Taint analysis is useful evidence about possible influence.

It is not the semantic definition of information flow.

Example:

```text
secret - secret
```

may remain tainted under a conservative analyzer even when semantic simplification yields a constant zero.

That is acceptable.

## Conservative interpretation

A taint result may say:

```text
possible influence
```

without proving an actual distinguishability channel.

## Taint false negatives

An unsound taint analyzer cannot be treated as authoritative evidence for absence of information flow.

RFC 0018 verifier trust should scope which claims a particular analyzer may establish.

---

# Part XXI — control-context analysis

A practical analyzer may track a `pc`-style label or equivalent control context.

This is one proof method for implicit flows.

The canonical semantics should preserve the underlying notion:

```text
operation is control-dependent on protected information
```

without requiring one specific implementation algorithm.

---

# Part XXII — side channels as observer channels

RFC 0024 should unify side-channel reasoning with the observer model.

A side channel is not a magical separate class of bugs.

It is an observation path not captured by the narrower functional observer model.

## Value channel

```text
returned values
output bytes
messages
```

## Termination channel

```text
returns vs diverges
```

## Failure channel

```text
error kind
trap
retry behavior
```

## Timing channel

```text
completion time
event spacing
deadline miss
```

## Control channel

```text
branch trace
instruction-class trace
```

## Address channel

```text
memory addresses
lookup-table indices
cache-line footprint
```

## Size channel

```text
message length
allocation size
compression output length
```

## Scheduling channel

```text
thread ordering
lock acquisition
preemption pattern
```

## Resource channel

```text
memory use
CPU work
energy
network volume
```

## Physical channel

```text
power
EM
acoustic
thermal
```

Physical-channel support may remain largely evidence-oriented for a long time, but the ontology should leave room for it.

---

# Part XXIII — constant-observation / constant-time profiles

## Avoid universal `constant_time`

A source or function should not carry a naked Boolean:

```text
constant_time = true
```

Instead a security profile should identify protected observations.

Example:

```text
SecretIndependentObservation {
    protected_inputs: key
    observations:
        branch_trace
        memory_address_trace
}
```

## Wall-clock timing

If actual elapsed-time indistinguishability is required, the contract must say so explicitly and use RFC 0023 clock semantics.

## Target dependence

A digital trace property may be target-independent at one IR level.

A variable-latency instruction claim may depend on a particular microarchitecture.

Target-sensitive claims must carry RFC 0017 target identity.

## Speculation

Speculative execution can create observations absent from architectural control flow.

A side-channel observer model may therefore include speculative or microarchitectural traces where the target model supports them.

RFC 0024 does not define all microarchitectural semantics itself.

---

# Part XXIV — cryptographic transformations and flow policy

## Encryption

Encryption may change the information relation between plaintext and ciphertext under cryptographic assumptions.

It does not inherently grant permission to expose ciphertext.

A release policy must still authorize the public observation.

## Hashing

Hashing may preserve enough information for guessing attacks or equality tests.

It does not automatically remove confidentiality.

## MAC/signature

Integrity mechanisms may provide evidence about origin/authenticity under cryptographic assumptions.

They do not automatically establish application-level truth.

RFC 0025 will define principal/credential/cryptographic trust relationships more fully.

## Compression

Compression may leak information through output length.

If length is observable, it belongs in the channel model.

---

# Part XXV — aggregation and partial release

Aggregation may intentionally release less information than raw data.

Examples:

```text
count
average
threshold predicate
histogram
```

But the operation alone does not authorize the release.

An explicit `ReleaseFunction` or `ReleaseRelation` should describe the intended knowledge increase.

Future privacy-specific quantitative mechanisms can build on this structure without changing the constitutional rule.

---

# Part XXVI — quantitative information flow

Strict noninterference answers whether a protected distinction can affect an observation.

Some applications intentionally allow bounded leakage.

RFC 0024 should support explicit quantitative claims.

Conceptually:

```text
QuantitativeLeakageClaim {
    subject
    secret_domain
    observer_model
    channel_model
    metric
    bound
    prior_or_attacker_model
    assumptions
    evidence
}
```

## Metric identity matters

Possible metrics may include:

```text
Shannon-style information
min-entropy/guessing vulnerability
channel capacity
belief change
application-specific inference risk
```

They are not interchangeable.

## No universal leakage score

The system must not collapse:

```text
metric
prior
observer model
secret domain
```

into one naked scalar.

## Empirical quantitative evidence

Measured distributions can provide bounded empirical evidence.

They do not become universal leakage theorems without additional proof.

---

# Part XXVII — relational proof techniques

RFC 0024 should permit multiple proof methods.

Candidate methods include:

```text
security typing
self-composition
relational symbolic execution
product programs
relational Hoare logic
logical relations
bisimulation
model checking
SMT certificates
taint certificates
```

No one method becomes the semantic definition of security.

## Self-composition

A verifier may transform a relational two-run claim into a single combined verification problem.

Such transformations must themselves preserve the security claim and assumptions.

## Product programs

A product program may execute two logical runs side by side and prove synchronized invariants.

Again, this is a proof technique.

---

# Part XXVIII — security certificates

Following RFC 0007 and RFC 0023, expensive security analysis should be separable from checking.

Conceptually:

```text
SecurityCertificate {
    claim
    method
    witness
    dependencies
    target_scope
    observation_scope
}
```

A verifier may check:

```text
flow graph
control dependencies
paired execution relation
schedule-independent property
side-channel trace condition
```

without trusting the process that discovered the witness.

---

# Part XXIX — assurance integration

RFC 0018 controls whether security evidence authorizes an action.

Security evidence can include:

```text
proof term
checkable certificate
static analyzer result
model checker result
bounded paired execution
side-channel measurement
fuzz result
manual review witness
```

These evidence classes should not be flattened.

## Example policies

```text
experimental_security
```

may authorize sandbox testing from conservative static analysis plus bounded observations.

```text
trusted_security_boundary
```

may require stronger relational proof, independent verifier evidence, or exact target applicability.

## Freshness

A microarchitectural side-channel result may become stale when:

```text
CPU revision changes
compiler changes
backend changes
optimization flags change
runtime changes
```

Invalidation should be narrow and dependency-aware.

---

# Part XXX — policy and evidence separation

A security verifier may establish:

```text
candidate satisfies policy P
```

It may not decide:

```text
policy P should be weaker
```

unless a separate authorized policy-change mechanism explicitly permits that action.

This rule is especially important for machine-generated optimization.

---

# Part XXXI — compiler and IR consequences

## Security metadata is semantic

HIR and SSA should preserve security-relevant identities and requirements where the supported subset includes them.

## Security labels through lowering

A logical value with policy:

```text
Confidentiality A
Integrity B
```

must not lose those policy relationships merely because its physical representation changes.

## Control-context preservation

Lowering must preserve or correctly translate control influence.

An optimization that converts control flow into data flow may change the proof method but not the security requirement.

## Security-sensitive transformations

Examples include:

```text
branch -> select
table lookup -> oblivious scan
scalar -> vectorized code
loop fusion
loop splitting
memoization
caching
speculative optimization
```

Each may alter observable channels.

## Functional correctness is insufficient

A pass can preserve return values and still violate:

```text
address-trace noninterference
timing noninterference
failure noninterference
scheduler-visible determinism
```

## Backend obligations

Machine-intent security requirements should survive to the backend.

If the backend cannot establish them, it must choose a conservative realization or leave the obligation unresolved.

---

# Part XXXII — security refinement

RFC 0020 should provide the relation framework for security preservation.

Candidate relations include:

```text
InformationFlowRefinement
ConfidentialityRefinement
IntegrityRefinement
ObserverRestrictedRefinement
SideChannelRefinement
```

A candidate may be functionally equivalent but security-weaker.

A candidate may also be security-stronger while functionally equivalent.

## Example

Baseline:

```text
table[secret_index]
```

Candidate:

```text
oblivious_scan(secret_index)
```

Both may satisfy functional equivalence.

Only the candidate may satisfy a declared address-trace noninterference relation.

---

# Part XXXIII — security deltas

Forge should be able to compare:

```text
SecurityDelta {
    preserved_claims
    strengthened_claims
    weakened_claims
    invalidated_claims
    new_obligations
    new_reclassification_events
    new_channels
}
```

This should remain separate from ordinary semantic, authority, and evidence deltas where useful, while linking to them through identities.

---

# Part XXXIV — security counterexamples

A verifier should return relation-scoped counterexamples.

Examples:

```text
secret inputs s1, s2
public inputs p
observer trace differs at event E
```

or:

```text
branch/address trace differs
```

or:

```text
timing intervals become distinguishable
```

A counterexample should identify:

```text
claim
observer
channel
input relation
trace location
target/environment
```

This is much more useful for recursive repair than a generic “security test failed.”

---

# Part XXXV — diagnostics

Diagnostics should explain the semantic reason for a rejected flow.

Example:

```text
FLOW_REJECTED
source policy: Payroll confidential
sink policy: Public
influence: control dependency through block B17
required authority: Payroll release authority
observed authority: none
```

For declassification:

```text
DECLASSIFICATION_REJECTED
release function: salary > threshold
required integrity: TrustedPayrollContext
current control context: UserControlled
reason: robust declassification requirement not satisfied
```

For side channels:

```text
SIDE_CHANNEL_COUNTEREXAMPLE
observer: co-resident process
channel: memory_address_trace
secret pair: s1, s2
first divergence: load instruction I42
```

Diagnostics should remain deterministic and machine-readable.

---

# Part XXXVI — security and provenance

RFC 0009 provenance can identify input contribution.

That information can support IFC analysis.

But:

```text
provenance known
```

is not:

```text
flow authorized
```

A value may have perfectly known provenance and still violate confidentiality or integrity policy.

Likewise a flow policy can constrain information even when provenance evidence is incomplete.

---

# Part XXXVII — security and effects

RFC 0008 owns effect authorization.

RFC 0024 owns whether performing an authorized effect leaks protected information.

Example:

```text
principal has network-send capability
```

but:

```text
secret may not influence network output
```

The send can be authority-valid but flow-invalid.

Both checks are required.

---

# Part XXXVIII — unsafe and foreign boundaries

RFC 0015 owns FFI/unsafe containment.

RFC 0024 adds information-flow requirements at those boundaries.

Foreign code may be modeled as:

```text
flow-transparent with proof
flow-sanitizing with evidence
untrusted/unknown influence
```

The default should be conservative.

An FFI call should not silently erase flow labels.

---

# Part XXXIX — runtime and target interaction

RFC 0017 provides runtime/target facts.

Security claims may depend on:

```text
CPU model
cache topology
speculation mode
scheduler
isolation settings
memory allocator
clock source
```

Those dependencies must be explicit when they affect the observer model.

---

# Part XL — Fabric integration

Fabric can route security work by claim type.

Examples:

```text
static flow verifier
relational verifier
constant-observation verifier
concurrent observational-determinism verifier
quantitative leakage analyzer
```

Agents need not receive the entire security-tool surface.

Fabric may also preserve observer/channel scope while delegating subclaims.

Security result aggregation must not broaden the scope of the strongest component claim.

---

# Part XLI — Forge integration

Forge should treat security as a protected transformation dimension.

A candidate search may vary:

```text
algorithm
representation
branch structure
memory layout
lookup strategy
parallelization
scheduling
padding
message shape
isolation mechanism
```

while preserving declared security requirements.

## Example realization search

Requirement:

```text
functional correctness
no secret-dependent branch trace
no secret-dependent address trace
```

Candidates:

```text
binary search
hash lookup
linear scan
oblivious scan
bitsliced representation
vectorized full scan
```

Forge rejects candidates that fail required security relations even if faster.

## Security improvement

Forge may also search for a candidate that establishes a stronger security relation than the baseline.

Promotion still requires RFC 0018 policy authorization.

---

# Part XLII — RAVEL integration

RAVEL may use RFC 0024 for bounded adversarial exploration.

Candidate activities include:

```text
paired secret executions
observer-trace mutation
side-channel counterexample search
flow-policy fuzzing
adversarial declassification-context generation
```

RAVEL findings are evidence/counterexamples.

They are not automatically universal proofs.

---

# Part XLIII — machine-generated security policies

MNCS must be especially careful with AI-generated policies.

An agent may propose:

```text
new confidentiality requirement
new integrity restriction
narrower release function
new observer model
```

but policy broadening/weakening must be separately authorized.

## Safe default

When uncertain, machine inference may:

```text
strengthen confidentiality
weaken assumed integrity
add observer channels
add obligations
```

but may not:

```text
remove confidentiality
raise integrity
remove an observer channel
add declassification authority
```

without policy authorization.

---

# Part XLIV — dynamic principals and runtime-created policy

Modern systems create security domains at runtime:

```text
tenants
plugins
sessions
sandboxes
remote peers
jobs
```

RFC 0024 should allow labels referencing runtime principal identities or policy objects.

The authenticity/credential semantics of those principal references remain RFC 0025.

Dynamic policy checks may require runtime evidence.

---

# Part XLV — capability interaction

Capabilities may encode authority to perform operations.

Flow labels encode information dissemination/influence policy.

These should compose.

Example:

```text
Capability: write_public_log
```

is insufficient if the current value/control context carries confidential information not authorized for public release.

Likewise:

```text
Flow permitted
```

does not imply the process possesses the capability required to perform the effect.

---

# Part XLVI — information-flow holes

Just as RFC 0019 permits representation holes and RFC 0023 permits schedule realization freedom, RFC 0024 may permit unresolved security realizations.

Example:

```text
SecurityRequirement {
    secret_independent:
        branch_trace
        address_trace

    realization:
        ?
}
```

Forge can search implementations.

The security requirement remains fixed.

---

# Part XLVII — observer-model holes

Observer scope should not be silently inferred from deployment folklore.

A specification may intentionally leave an observer model unresolved during early research, but such a claim cannot authorize strong security conclusions until the model is fixed.

An RFC 0018 policy may permit experimentation while rejecting production promotion.

---

# Part XLVIII — bounded empirical security evidence

A bounded experiment may execute pairs of secret variants and compare:

```text
values
branches
addresses
timing
failures
```

The result should be explicitly named something like:

```text
BoundedSecurityAgreement
```

or represented through RFC 0020 `BoundedAgreement` plus a security observation model.

It must not be labeled universal noninterference.

---

# Part XLIX — independent security verifiers

Security is a natural area for multiple independent micro-verifiers.

Examples:

```text
flow-lattice checker
control-dependence checker
address-trace checker
paired-execution verifier
declassification-authority checker
endorsement checker
observer-model compatibility checker
```

RFC 0018 can assign different trust scopes to each verifier.

A verifier trusted for explicit-flow checking may not be trusted for microarchitectural timing claims.

---

# Part L — policy normalization and canonicalization

Flow-policy artifacts should have deterministic canonical form.

Equivalent policy spellings should canonicalize where safe.

However, policy normalization must not erase:

```text
owner identity
authority requirement
validity interval
observer class
release function identity
```

where those distinctions affect semantics.

---

# Part LI — policy identity

Policy identity should distinguish:

```text
content identity
generative identity
owner identity
version identity
validity identity
```

where required.

Two structurally identical policies issued by different authorities need not be the same policy object.

RFC 0020 provides the general identity/equivalence framework.

---

# Part LII — release-function identity

Two release functions may be extensionally equivalent over a declared domain yet have different artifact identities.

Example:

```text
x % 2
```

and:

```text
is_even(x) ? 0 : 1
```

may induce the same information partition.

RFC 0020 can express their relation without collapsing provenance.

---

# Part LIII — policy composition across modules

RFC 0014 module/component linking must preserve information-flow policy.

A component requiring:

```text
input confidentiality C
```

and producing:

```text
output confidentiality D
```

needs an explicit flow contract.

Linking should reject incompatible policy compositions rather than relying on runtime surprise.

---

# Part LIV — abstraction boundaries

RFC 0013 abstraction should be able to hide representations while exposing information-flow contracts.

An abstract module may promise:

```text
returns only declassified predicate P
```

without exposing internal secret representation.

Full-abstraction/security relationships may later strengthen this boundary through RFC 0020.

---

# Part LV — higher-order functions and closures

Function values may capture protected information.

A closure that returns a public type may still carry secret influence through its environment.

Therefore function security policy must consider:

```text
captured values
call-time arguments
observable effects
control behavior
```

Closure conversion must preserve those relationships.

---

# Part LVI — dynamic/existential values

RFC 0019 dynamic/existential packages must carry or preserve the security policy associated with hidden values.

Unpacking a dynamic value should not erase confidentiality/integrity obligations merely because the logical type was existentially hidden.

---

# Part LVII — serialization boundary

RFC 0027 will define serialization/wire semantics.

RFC 0024 establishes now that serialization is an information-flow event.

Encoding secret data into bytes does not make it public.

Length or schema choices may also leak information.

---

# Part LVIII — persistence boundary

RFC 0026 will define durable state.

RFC 0024 establishes that:

```text
write secret to disk
```

must preserve flow policy unless an explicit release/cryptographic storage contract applies.

Persistent copies complicate revocation and erasure and are deferred to RFC 0026.

---

# Part LIX — distribution boundary

RFC 0028 will define distributed messaging and consistency.

RFC 0024 establishes that remote transmission is a flow to an observer/principal/network channel.

Network metadata may be a separate observer channel from payload contents.

---

# Part LX — build/supply-chain interaction

RFC 0031 will define build and artifact lineage.

Security claims may depend on compiler/backend/tool identities.

A security certificate tied to one lowering pipeline may become stale after toolchain changes.

---

# Part LXI — quantitative resource interaction

RFC 0032 will define quantitative cost/resource semantics.

RFC 0024 should be able to refer to those quantities as observer channels later.

Example:

```text
secret must not change allocation_count
```

or:

```text
leakage through energy <= bound
```

The security ontology should not need redesign when those metrics arrive.

---

# Part LXII — multi-objective realization search

RFC 0033 will define generalized realization search.

RFC 0024 supplies protected security constraints and security-specific evidence.

Forge may optimize:

```text
performance
energy
memory
latency
```

subject to:

```text
functional relation
numeric relation
temporal relation
security relation
```

A better objective value cannot justify a security violation.

---

# Part LXIII — test/fuzz interaction

RFC 0034 will define empirical test/fuzz evidence.

Security fuzzing can produce:

```text
counterexamples
trace divergences
timing distributions
unexpected declassification paths
```

Such evidence remains empirical unless transformed into a stronger independently checkable claim.

---

# Part LXIV — elaboration and inference

RFC 0035 will define full elaboration/inference semantics.

RFC 0024 establishes constraints on future security inference:

- inferred restrictions may be conservative;
- ambiguous authority must remain unresolved;
- inferred declassification authority is forbidden without explicit policy support; and
- inferred observer narrowing cannot strengthen a security conclusion without evidence.

---

# Part LXV — evolution/versioning

RFC 0036 will define language/spec evolution.

Flow-policy artifacts and security claim kinds must be versioned.

A verifier built for one observer-model version must not silently interpret a newer model as equivalent.

---

# Part LXVI — observability and audit

RFC 0037 will define broader telemetry/audit semantics.

Security audit artifacts should preserve:

```text
reclassification events
security counterexamples
policy changes
security evidence dependencies
observer scopes
```

Audit telemetry itself may be an information-flow sink and therefore must obey RFC 0024 policy.

---

# Candidate relation taxonomy

RFC 0024 should avoid one universal security relation.

Candidate relations include:

```text
ValueNoninterference<O>
TerminationInsensitiveNoninterference<O>
TerminationSensitiveNoninterference<O>
TimingNoninterference<O, ClockModel>
AddressTraceNoninterference<O, Target>
ControlTraceNoninterference<O>
FailureNoninterference<O>
SchedulerSensitiveNoninterference<O>
ObservationalDeterminism<O>
InformationFlowRefinement<O>
RobustDeclassification<Policy, Attacker>
TransparentEndorsement<Policy, Attacker>
NonmalleableInformationFlow<Policy, Attacker>
QuantitativeLeakageBound<Metric, Bound>
```

These may be represented through RFC 0020 relation definitions rather than all becoming special syntax.

---

# Candidate policy algebra

A minimal bounded model may start with:

```text
ConfidentialityPolicy {
    readers: set<PrincipalRef>
}

IntegrityPolicy {
    influencers: set<PrincipalRef>
}

FlowLabel {
    confidentiality
    integrity
}
```

The full research model should leave room for more decentralized owner-policy structures.

## Flow join

For confidentiality, joining influences normally makes a result at least as restrictive as each source.

For integrity, joining lower-integrity influences may reduce the trust set/guarantee.

The exact canonical lattice orientation must be formalized carefully.

---

# Worked example — explicit leak

```text
secret_key : Secret<Key>
public_out : Public<Bytes>

public_out = bytes(secret_key)
```

Without declassification:

```text
Secret can_flow_to Public == false
```

Result:

```text
FLOW_REJECTED
```

---

# Worked example — implicit leak

```text
if secret_bit {
    public_out = 1
} else {
    public_out = 0
}
```

Control context becomes secret-influenced.

The public write is therefore rejected even though `secret_bit` is not copied directly.

---

# Worked example — allowed predicate release

Secret:

```text
password
```

Authorized release:

```text
password == guess
```

The observer may learn one bit-equivalent predicate result per permitted interaction, under the declared release policy.

The password itself remains protected.

---

# Worked example — parity release relation

Secret integer `x`.

Policy permits release of parity.

Define:

```text
x ~ y iff parity(x) == parity(y)
```

Security requires the observer to be unable to distinguish secret values inside the same equivalence class.

This maps directly onto RFC 0020 relation machinery.

---

# Worked example — robust declassification failure

```text
field = attacker_input
release(secret_record[field])
```

Even if `release` is an authorized primitive, attacker control over `field` may violate robust-declassification policy.

The rejection should identify the low-integrity control influence.

---

# Worked example — validation versus endorsement

```text
x = untrusted_input
proof = validate_identifier(x)
```

This establishes:

```text
ValidIdentifier(x)
```

It does not automatically establish:

```text
HighIntegrity(x)
```

An explicit endorsement policy must authorize that upgrade.

---

# Worked example — timing leak

```text
if secret {
    repeat 1000 times: hash()
} else {
    hash()
}
return 0
```

Value-only noninterference may hold.

Timing-sensitive noninterference fails under an observer with sufficient timing resolution.

Both relation results should coexist.

---

# Worked example — termination leak

```text
if secret {
    while true {}
}
return 0
```

Termination-insensitive profile may ignore the distinction.

Termination-sensitive profile rejects it.

RFC 0022 supplies the termination observation.

---

# Worked example — address-trace leak

```text
return table[secret_index]
```

Functional correctness may be perfect.

Address-trace noninterference fails if the observer sees table access addresses.

Alternative:

```text
oblivious_scan(table, secret_index)
```

may satisfy the address-trace relation at higher execution cost.

Forge can search that tradeoff while keeping security mandatory.

---

# Worked example — concurrent scheduling leak

Two threads produce identical final values.

Secret input influences which thread becomes runnable first.

If output ordering or scheduler events are observable, a concurrency-sensitive security claim may fail even though final-value noninterference holds.

---

# Worked example — ciphertext exposure

```text
ciphertext = encrypt(secret, key)
```

The compiler must not silently relabel ciphertext `Public` merely because encryption occurred.

A policy may explicitly authorize exposing ciphertext under a cryptographic security claim and observer model.

That release remains separate from the encryption operation itself.

---

# Worked example — hash exposure

```text
public = hash(secret_pin)
```

If `secret_pin` has a small domain, the hash can reveal it through enumeration.

Hashing therefore does not implicitly declassify.

---

# Worked example — stale side-channel evidence

A branch/address analysis is proven for backend pipeline `B1`.

A later optimization pipeline `B2` introduces a lookup table.

The security evidence depending on `B1` becomes stale.

RFC 0018 invalidates the affected security claim without necessarily invalidating unrelated functional evidence.

---

# First implementation experiment

The first RFC 0024 implementation should be deliberately bounded.

It should demonstrate that the semantic distinctions are executable before attempting a complete IFC language.

## Phase 1 — versioned labels and policies

Introduce versioned research artifacts for:

```text
FlowLabel
ConfidentialityPolicy
IntegrityPolicy
FlowRelation
```

Use a deliberately small principal-reference model.

Do not finalize RFC 0025 identity semantics.

## Phase 2 — explicit data flow

Support one small body/HIR subset where values carry flow labels.

Reject a direct secret-to-public assignment.

Accept public-to-secret flow under the chosen confidentiality orientation.

## Phase 3 — control influence

Add a bounded program-counter/control-context model.

Reject a public assignment selected by a secret condition.

Provide deterministic diagnostics identifying the control dependency.

## Phase 4 — integrity flow

Add one untrusted-to-trusted rejection case.

Keep confidentiality and integrity checks separate.

## Phase 5 — declassification

Introduce:

```text
DeclassificationPolicy
DeclassificationEvent
ReleaseFunction
```

Permit release of one narrow predicate.

Reject an over-broad release.

## Phase 6 — robust release

Create a fixture where an attacker-controlled value selects what secret information is released.

Reject it under a robust-declassification profile.

## Phase 7 — endorsement

Introduce one validation artifact plus explicit endorsement authority.

Demonstrate:

```text
validation alone -> insufficient
validation + authorized endorsement -> accepted
```

## Phase 8 — observer profiles

Define at least three observer/channel profiles:

```text
value_only
value_plus_termination
value_plus_branch_and_address_trace
```

Use RFC 0020 observation models where possible.

## Phase 9 — termination leak

Construct one program that:

- satisfies value-only bounded comparison;
- fails termination-sensitive security.

Do not collapse the results.

## Phase 10 — address-trace leak

Compare:

```text
direct table lookup
oblivious full scan
```

Both should satisfy a bounded functional corpus.

Only the oblivious candidate should satisfy the bounded address-trace profile.

## Phase 11 — bounded relational execution

Extend or wrap the existing bounded reference execution machinery to run paired secret variants under the same public input.

Produce:

```text
BoundedAgreement
```

or an RFC 0020-equivalent artifact for the security observer model.

Explicitly state that this is empirical/bounded evidence, not universal noninterference.

## Phase 12 — security relation claims

Create RFC 0020 relation claims for at least:

```text
ValueNoninterference
TerminationSensitiveNoninterference
AddressTraceNoninterference
```

The same candidate pair should satisfy one relation while failing another.

## Phase 13 — security counterexample

Return a structured counterexample showing:

```text
secret pair
public input
observer model
first divergent observation
```

## Phase 14 — assurance profiles

Add RFC 0018 policies such as:

```text
experimental_information_flow
trusted_information_flow
```

The experimental profile may authorize bounded sandbox use.

The stronger profile must remain unresolved without stronger evidence.

## Phase 15 — transformation experiment

Give Forge two candidate implementations:

```text
table lookup
oblivious scan
```

Require:

```text
functional relation
address-trace noninterference
```

Allow performance preference.

Reject the faster candidate if it violates the protected security relation.

---

# Initial fixture suite

The bounded experiment should include at least:

```text
1. direct explicit confidentiality leak
2. implicit control-flow confidentiality leak
3. valid public-to-secret flow
4. low-integrity-to-high-integrity rejection
5. authorized predicate declassification
6. over-broad declassification rejection
7. attacker-controlled declassification rejection
8. validation without endorsement rejection
9. validation plus authorized endorsement acceptance
10. termination leak
11. address-trace leak
12. oblivious address-trace-preserving candidate
13. concurrent scheduler-visible leak or bounded trace analogue
14. bounded paired execution that passes but remains non-universal
15. stale target/backend security evidence case
```

---

# Success criteria for the first experiment

The experiment succeeds if MNCS can demonstrate all of the following:

1. confidentiality and integrity are represented independently;
2. labels propagate through explicit data dependence;
3. at least one implicit control flow is detected;
4. ordinary label propagation may strengthen restrictions but cannot invent declassification;
5. declassification names a narrow release function or relation;
6. declassification authority and integrity context are distinct inputs;
7. validation alone does not perform endorsement;
8. one program passes a narrow observer profile while failing a broader one;
9. bounded empirical paired execution remains explicitly bounded;
10. a security counterexample identifies the first divergent observation;
11. target/backend-dependent security evidence can become stale without destroying unrelated claims;
12. Forge rejects a faster candidate that violates a protected security relation; and
13. RFC 0018 distinguishes sandbox security evidence from stronger promotion authority.

---

# Non-goals for the first experiment

The initial pilot does not attempt to provide:

- a production secrecy type system;
- complete alias-sensitive IFC;
- complete concurrent noninterference;
- verified microarchitectural leakage freedom;
- cryptographic proof automation;
- physical power/EM analysis;
- distributed multi-principal authentication;
- durable information erasure;
- differential privacy;
- one universal quantitative leakage metric; or
- proof of a production compiler's end-to-end secure compilation.

---

# Rejected directions

## Rejected: one `secret` Boolean

A Boolean cannot represent confidentiality ownership, integrity, release policy, observer scope, or dynamic policy.

## Rejected: access control as the whole security model

Access checks do not control downstream information influence.

## Rejected: one scalar security level

Confidentiality and integrity are orthogonal and decentralized ownership matters.

## Rejected: universal `secure = true`

Security depends on observer/channel/target/release scope.

## Rejected: universal `constant_time = true`

Constant-observation claims must identify what observations are protected.

## Rejected: declassification as unchecked cast

That destroys the semantic security boundary.

## Rejected: endorsement as unchecked cast

Trust upgrades require authority and evidence.

## Rejected: encryption automatically makes data public

Cryptographic transformation is not release authority.

## Rejected: hashing automatically makes data public

Hashes can reveal information.

## Rejected: sanitizer automatically raises integrity

Validation and policy authority remain distinct.

## Rejected: finite tests prove noninterference

Finite experiments are empirical evidence only.

## Rejected: compiler may infer permissive policy from observed success

Inference may conservatively strengthen protection, not fabricate downgrade authority.

## Rejected: side-channel claims without target/observer scope

Such claims are too ambiguous to authorize machine transformations.

---

# Open research questions

RFC 0024 intentionally leaves several questions open.

## Policy representation

Should the canonical confidentiality model begin with reader sets, owner-policy conjunctions, acts-for style constraints, or another compact form?

## Integrity duality

How closely should integrity policy mirror confidentiality policy in the first executable core?

## Principal abstraction

What is the minimum abstract principal vocabulary RFC 0024 needs before RFC 0025 is formalized?

## Relational kernel boundary

Which noninterference proofs should become kernel-checkable proof terms versus independently checkable certificates?

## Hyperproperty representation

Should the canonical model explicitly represent k-safety/hyperproperty arity or rely on RFC 0020 relation definitions?

## Dynamic labels

How much runtime label machinery belongs in the first language runtime versus Forge/library realizations?

## Secure multi-execution

When should multi-execution be considered a normal realization rather than an experimental containment strategy?

## Concurrency

Which observational-determinism or scheduler-security relation should become the first concurrent IFC target?

## Side-channel abstraction

How should architectural traces, microarchitectural traces, and physical observations relate without pretending one target model captures all leakage?

## Quantitative leakage

Which leakage metrics are worth standardizing as first-class vocabularies versus leaving to verifier-specific schemas?

## Policy change

How should revocation and dynamic relabeling interact with historical observer knowledge and persistent state?

## Speculation

What target facts are required to make speculative-execution side-channel claims reproducible and checkable?

## Optimization

Which compiler transformations should be required to produce security-preservation witnesses by default?

---

# Research inspirations

RFC 0024 is informed by, but does not mechanically adopt, several major lines of work:

- Denning-style lattice-based information-flow models;
- Biba-style integrity policies;
- Goguen–Meseguer noninterference;
- decentralized information-flow control and the Myers–Liskov decentralized label model;
- Jif-style confidentiality declassification and integrity endorsement;
- Sabelfeld–Sands declassification dimensions;
- robust declassification;
- transparent endorsement and nonmalleable information flow;
- hyperproperties;
- observational determinism for concurrent IFC;
- dynamic/floating-label systems such as LIO;
- secure multi-execution;
- relational program logics and self-composition;
- quantitative information flow; and
- relational constant-time / digital side-channel verification.

The purpose of the synthesis is not to reproduce one prior language.

The purpose is to provide a machine-native security ontology compatible with MNCS evidence, relation, runtime, time, termination, refinement, and realization semantics.

---

# Relationship to MNCS research philosophy

MNCS should not ask a model or compiler the vague question:

```text
Is this program secure?
```

It should ask questions such as:

```text
Does protected input S influence public return value under observer O?

Does S influence termination?

Does S influence memory-address trace on target T?

Which function of S is intentionally released?

Which authority permits that release?

Can attacker-controlled input alter the release beyond policy D?

Which low-integrity inputs influence trusted result R?

Which security relations are preserved by candidate transformation C?

What evidence supports those relations?

Is that evidence strong enough under RFC 0018 to authorize this deployment action?
```

That is the machine-native direction.

The language states **permitted influence and permitted knowledge**.

Forge searches implementations.

Verifiers establish relation-scoped evidence.

RFC 0018 decides whether that evidence authorizes use.

No layer silently converts successful execution into permission to weaken security policy.

---

# Constitutional summary

RFC 0024 proposes the following durable commitments:

1. information-flow policy follows information and influence, not just storage objects;
2. access authority and flow authority remain distinct;
3. confidentiality and integrity remain independent semantic dimensions;
4. data, control, effects, failures, termination, timing, scheduling, and declared resource behavior may all carry information;
5. observer and channel scope are part of every meaningful security claim;
6. noninterference is a relational/hyperproperty-style semantic property, not synonymous with taint analysis;
7. termination-sensitive, timing-sensitive, scheduler-sensitive, and address-trace security remain distinct relations;
8. declassification is an authorized narrow release, not a cast;
9. endorsement is an authorized integrity upgrade, not a cast;
10. validation evidence does not itself grant endorsement authority;
11. robust/nonmalleable profiles protect reclassification against adversarial influence;
12. static, dynamic, hybrid, multi-execution, isolation, and proof-carrying IFC remain realization strategies;
13. side channels are modeled through explicit observation channels rather than one universal side-channel-free bit;
14. encryption, hashing, compression, and aggregation do not automatically erase confidentiality policy;
15. quantitative leakage claims must identify metric, attacker/observer model, domain, and assumptions;
16. finite empirical security testing remains finite evidence;
17. lowering and optimization must preserve declared security relations in addition to functional semantics;
18. target-sensitive security evidence carries target/runtime dependencies and freshness;
19. machine inference may conservatively strengthen protection but cannot invent downgrade or endorsement authority;
20. Forge may search aggressively over security-preserving realizations while security requirements remain fixed; and
21. RFC 0018 remains the authority that decides whether available security evidence is sufficient for a particular action.

The resulting architecture allows MNCS to move beyond `secure = true` toward a system that can state exactly **who may learn what, who may influence what, which observations count, which releases are deliberate, which reclassifications are authorized, which channels remain protected, and what evidence supports every such claim**.
