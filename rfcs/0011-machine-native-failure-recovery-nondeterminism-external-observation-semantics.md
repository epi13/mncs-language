# RFC 0011: Machine-Native Failure, Recovery, Nondeterminism, and External Observation Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.5–1.0
- **Depends on:** RFC 0002, RFC 0003, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010

## Summary

MNCS Language should define failure and recovery in terms of explicit **execution outcomes, external
effect state, resource state, knowledge state, nondeterministic observations, repeatability,
recovery authority, and evidence** rather than defining failure primarily as an exception, error
value, panic, timeout code, or process exit.

The central rule is:

> **MNCS must distinguish what happened, what was observed, what is known, and what recovery actions
> are safe. Exceptions, retries, logs, transactions, compensation, restart, failover, deduplication,
> checkpoints, and replay are realizations of those semantics.**

A timeout does not prove that an external operation failed. A missing acknowledgement does not prove
that an externally visible effect did not occur. Restoring local state does not imply that external
effects were rolled back. Repeating an operation is not automatically safe merely because the
previous attempt returned an error. An empirical failure rate is not automatically a semantic
probability law. A scheduler choice, a random sample, and an unknown external outcome are different
kinds of nondeterminism and should remain different semantic objects.

This RFC therefore proposes a foundation in which an operation can carry:

- its semantic identity and causal position;
- its completion state;
- external effects known to be unapplied, partially applied, applied, or uncertain;
- the current state of local resources and authority;
- observations and their provenance;
- what the program knows, suspects, or cannot determine about the world;
- retry, resume, compensation, rollback, reconciliation, restart, failover, and escalation rules;
- determinism, nondeterminism, and probabilistic models where explicitly justified;
- recovery obligations and failure assumptions; and
- proof, certificate, verifier, measurement, or unresolved evidence.

The proposal synthesizes complementary ideas from predicate-transformer semantics and explicit
nondeterminism, algebraic effects and handlers, idempotency and operation identity, sagas and
compensation, crash consistency, deterministic record/replay, failure-detector theory,
probabilistic reasoning, and recovery-oriented systems design. None of those traditions is adopted
wholesale.

The goal is not merely ergonomic error handling. The goal is to make failure paths, uncertain
external effects, recovery decisions, and nondeterministic inputs machine-readable enough that an
autonomous generator, verifier, runtime, Forge workflow, or backend can reason about what may safely
happen next without relying on undocumented human convention.

## Motivation

Conventional error models often collapse very different situations into one control-flow category:

```text
operation returned an error
operation timed out
operation was cancelled before starting
operation was cancelled after a partial external effect
remote service processed the request but acknowledgement was lost
process crashed after writing durable state
local state was restored but external I/O already occurred
network partition made remote state unknowable
hardware reset while DMA or device work was in flight
retry may duplicate a non-idempotent effect
compensation succeeded but did not erase the historical external effect
```

Treating all of these as `Error`, `Exception`, or `FAIL` loses information required for safe machine
reasoning.

For example:

```text
send transfer(Request42)
        |
        v
remote service commits transfer
        |
        v
acknowledgement is lost
        |
        v
caller times out
```

The caller may know:

```text
deadline elapsed
acknowledgement not observed
```

but it must not conclude:

```text
transfer did not commit
```

The correct semantic state may be:

```text
external_effect(Request42) = MAY_HAVE_COMMITTED
knowledge(Request42) = UNKNOWN
```

Blindly converting that condition to ordinary failure and automatically retrying can duplicate an
external effect.

Likewise, after a crash, restoring local memory or a checkpoint does not imply that a file write,
message send, device command, payment, database commit, or remote request has been undone. Recovery
must reason about an **external-effect frontier** separately from restored internal state.

MNCS already has the semantic ingredients needed to do better:

- RFC 0002 distinguishes claims, assumptions, evidence, and unresolved status;
- RFC 0006 separates semantic requirements from optimization realizations;
- RFC 0007 preserves a small proof boundary and explicit `UNKNOWN`;
- RFC 0008 models effectful external interaction, cancellation, resource transitions, and partial
  completion;
- RFC 0009 models resource identity, lifetime, provenance, and validity;
- RFC 0010 models causal events, observation provenance, progress, and explicit assumptions.

RFC 0011 extends those principles to execution outcomes and uncertain external state.

## Constitutional principles

### Failure and uncertainty are different

```text
FAILED != UNKNOWN
```

A known failed-before-effect operation is different from an operation whose externally visible
result cannot be determined.

### World state and knowledge state are different

```text
world_state != knowledge_of_world_state
```

The semantic model must be able to represent that an effect occurred even when the local program
cannot prove that it occurred, or that the program suspects failure without knowing failure.

### Retry requires authority

> **An operation may be retried only when its semantic retry contract, current outcome state, and
> current evidence establish that repeating it preserves the required behavior, or when the selected
> realization supplies sufficient deduplication, compensation, reconciliation, or equivalent
> machinery to establish that property.**

Retry is not a generic consequence of failure handling.

### External knowledge has provenance

> **A runtime fact about the external world is an observation with source, causal position, temporal
> scope, and evidence rather than timeless truth.**

### Nondeterminism must be named

> **Every semantically relevant source of nondeterminism should be explicit, bounded by a declared
> model, or preserved as unresolved external uncertainty.**

### Timeout is an observation, not a proof of failure

> **A timeout proves that a deadline elapsed without the required observation. It does not by itself
> prove that the underlying external action failed or did not occur.**

### Compensation is not rollback

A compensating action may restore a business or resource invariant while still leaving externally
observable history. The language must not silently equate compensation with the original effect
never having happened.

### Recovery is ordinary semantics

Failure paths, crash paths, cancellation paths, reconciliation paths, and restart paths are ordinary
semantic transitions that may be generated, verified, tested, compared, and optimized.

## Design goals

RFC 0011 should make the following possible.

1. **Separate outcome from knowledge.** The actual outcome of an operation and the program's current
   knowledge of that outcome must remain distinct.
2. **Represent ambiguous external effects.** `MAY_HAVE_COMMITTED` or an equivalent state must be
   expressible without being collapsed into success or failure.
3. **Make retry safety explicit.** Idempotence, deduplication, replay safety, resumability, and
   irreversible behavior should constrain retry authority.
4. **Represent partial effects.** Partial writes, partially completed protocols, device progress,
   and externally visible prefixes should be first-class.
5. **Model compensation separately from rollback.** Recovery should preserve the difference between
   reversing state and issuing a new compensating effect.
6. **Represent crash consistency.** Persistent and external state that survive volatile execution
   loss must be tracked separately from local restored state.
7. **Name nondeterministic sources.** Scheduler choice, environment choice, random sampling,
   external input, clocks, and unknown outcomes should not share one anonymous category.
8. **Support deterministic replay.** Nondeterministic observations should admit an observation ledger
   sufficient for bounded deterministic replay where the selected environment permits it.
9. **Make external observations scoped.** Facts about devices, files, networks, clocks, services,
   and the environment should have provenance and validity scope.
10. **Preserve explicit `UNKNOWN`.** Missing acknowledgement, incomplete evidence, uncertain commit,
    unknown probability, or unknown replay safety must remove recovery authority rather than create
    optimistic assumptions.
11. **Separate semantic recovery from mechanism.** Retry loops, WAL, journal, transaction, saga,
    restart, checkpoint, message deduplication, and failover remain realizations.
12. **Bind probability to a model.** Probabilistic claims must identify the model/evidence that makes
    a probability meaningful; observed frequency alone is not automatically a semantic law.
13. **Produce actionable diagnostics.** Failures should report external-effect state, uncertainty,
    resource state, retry eligibility, and recovery obligations rather than only a code/message.
14. **Compose with structured concurrency.** Cancellation and task failure must preserve RFC 0010
    scope, authority, progress, and observation semantics.
15. **Keep proof boundaries explicit.** Model checkers, fault injectors, crash explorers, empirical
    testing, and probabilistic estimators remain distinguishable from kernel-checkable proof.
16. **Expose a recovery search space.** Forge should be able to compare recovery realizations by
    authority, latency, storage cost, durability, proof cost, replayability, and external risk.

## Non-goals

This RFC does not:

- select exceptions, `Result`, conditions, panics, or resumable exceptions as the universal surface
  error mechanism;
- claim exactly-once external execution as a generic primitive;
- treat timeout as failure detection with perfect accuracy;
- require every effect to be idempotent;
- require every operation to support rollback or compensation;
- require one transaction, database, log, checkpoint, or journal mechanism;
- make arbitrary remote services transactional;
- require one probabilistic programming model;
- treat benchmark frequency as proof of probability;
- define a complete distributed-consensus or replication model;
- guarantee deterministic replay across uncontrolled hardware or foreign environments without
  sufficient observation capture;
- place operating systems, devices, networks, remote services, or filesystems inside the trusted proof
  kernel; or
- claim that all failure or recovery properties are statically decidable.

Unsupported properties may remain `UNKNOWN`, require reconciliation, choose a conservative recovery
realization, disable retry, force escalation, or reject promotion under the active assurance policy.

## Selected theoretical synthesis

### Predicate-transformer semantics and explicit nondeterminism

MNCS should treat the environment as capable of producing any outcome allowed by the declared model.
Correctness under environmental nondeterminism therefore means satisfying the required postcondition
for every permitted environment outcome unless the contract states otherwise.

The model should distinguish at least:

```text
EnvironmentChoice
ImplementationChoice
ProbabilisticChoice<Model>
UnresolvedExternalState
```

These are semantically different.

An `EnvironmentChoice` is adversarial for safety reasoning: the environment may produce any permitted
outcome, so the implementation must remain correct for all of them.

An `ImplementationChoice` represents realization freedom. Forge, the compiler, or runtime may choose
one valid realization that satisfies the requirements.

A `ProbabilisticChoice<Model>` is justified only when a declared probability model exists.

`UnresolvedExternalState` represents lack of knowledge and must not be assigned an invented
probability merely to simplify optimization.

This distinction prevents three common semantic errors:

```text
possible != probable
unknown != random
implementation freedom != environment uncertainty
```

### Algebraic effects and handlers

Algebraic effect structure is useful for composing failure, cancellation, timeout, and recovery
control without requiring each mechanism to be a language primitive.

Conceptually:

```text
effects {
    MayFail<DeviceReset>
    MayTimeout<Request42>
    MayCancel<ScopeS>
    MayObserve<ClockC>
}
```

A handler may translate these events into local control flow, but handler execution does not
silently change the external-effect state.

For example:

```text
handle Timeout<Request42> {
    ...
}
```

does not imply:

```text
external_effect(Request42) = NOT_APPLIED
```

A timeout handler therefore operates on a richer `ExecutionOutcome` object rather than laundering
uncertain external state into an ordinary exception value.

Algebraic handlers remain an elaboration/control mechanism. They are not the ontology of failure.

### Operation identity, idempotency, and retry semantics

Every externally relevant operation that may need recovery should have stable semantic identity.

The model should reserve retry properties such as:

```text
Pure
ReplaySafe
Idempotent
Deduplicated<Key>
Resumable<CheckpointClass>
Compensatable<Compensation>
AtMostOnceRequired
Irreversible
UnknownRetrySafety
```

These properties are not merely annotations. They affect what recovery transitions are legal.

For example:

```text
retry(OperationX)
```

requires evidence sufficient to establish one of the following kinds of authority:

```text
RetrySafe<OperationX>
Deduplicated<OperationX, Key42>
KnownNotApplied<OperationX>
EquivalentRecovery<OperationX, StrategyY>
```

or another explicitly accepted proof path.

A non-idempotent operation may become safely repeatable under a realization that supplies a stable
request identity and deduplication boundary, but that property belongs to the realization/evidence
pair rather than being assumed from the abstract operation.

### Sagas and compensation

Long-running workflows may contain effects that commit independently and cannot be atomically rolled
back.

MNCS should model compensating transitions explicitly:

```text
effect reserve_inventory
    compensation release_inventory

effect charge_card
    compensation refund_card

effect ship_product
    irreversible_after carrier_acceptance
```

A workflow failure may then produce:

```text
RecoveryFrontier {
    committed = [reserve_inventory, charge_card]
    uncertain = []
    pending = [ship_product]
    compensatable = [charge_card, reserve_inventory]
}
```

Compensation creates new effects. It does not erase the causal history of the effects it compensates.

The semantic model should therefore distinguish:

```text
Rollback
Compensation
Reconciliation
```

with separate postconditions and evidence.

### Crash consistency and durable effect frontiers

Abrupt process or machine failure can destroy volatile execution state while leaving persistent or
external effects visible.

MNCS should model a durable or external-effect frontier independently of local checkpoint state.

Conceptually:

```text
RecoveryPoint<R42> {
    internal_semantic_state
    resource_state
    authority_state
    durable_effect_frontier
    uncertain_effect_frontier
    observation_frontier
}
```

Restart from `R42` means restoring the internal state represented by the checkpoint and then
reconciling all external effects that may have progressed beyond the recorded frontier.

A persistence contract may require properties such as:

```text
FailureAtomic
PrefixRecoverable
CompensatablePrefix
PartialEffectsVisible
NoCrashConsistencyGuarantee
```

`FailureAtomic` means that after a qualifying crash, externally visible state must correspond to an
allowed before/after state, not an arbitrary intermediate state.

A realization may use:

```text
write-ahead logging
copy-on-write
journal
shadow paging
transactional storage
persistent command log
hardware-supported transaction
```

without changing the semantic identity of the recovery contract.

### Failure detectors and suspicion

Timeouts and incomplete communication may provide evidence of failure without proving failure.

The runtime model should preserve states such as:

```text
KnownAlive
SuspectedFailed
KnownFailed
UnknownLiveness
```

where the transition from suspicion to knowledge requires evidence appropriate to the active model.

A deadline event may establish:

```text
DeadlineElapsed<WaitFor<Ack42>>
```

but not automatically:

```text
KnownFailed<RemoteService>
```

This prevents failure suspicion from silently becoming world-state truth.

### Deterministic record/replay

Nondeterministic observations should be identity-bearing semantic objects so bounded executions may be
replayed under the same observation stream where feasible.

A nondeterministic source may include:

```text
NondeterministicSource {
    identity,
    class:
        external_input |
        clock |
        entropy |
        scheduler |
        device |
        network |
        filesystem |
        foreign_runtime |
        environment,
    model,
}
```

An observation may include:

```text
Observation {
    identity,
    source_identity,
    causal_position,
    value_or_proposition,
    temporal_scope,
    model,
    evidence,
    replay_policy,
}
```

An execution may emit:

```text
ObservationLedger {
    O1 = ClockC -> T42
    O2 = NetworkN -> bytes(...)
    O3 = EntropyS -> 0x4188...
    O4 = DeviceD -> READY
}
```

Replaying with a compatible ledger should reproduce the semantic program's observed world-view for
that bounded execution, subject to explicitly declared uncontrolled sources.

The ledger is evidence and diagnostic infrastructure. It is not proof that another uncontrolled
physical run will behave identically.

### Probabilistic semantics

Probability is useful only when its model is explicit.

The language may eventually support contracts such as:

```text
probability<ModelM> {
    failure(DeviceD) <= 1e-6
}
```

but such claims must identify:

- the sample space or process model;
- independence/correlation assumptions where relevant;
- parameter evidence;
- scope and target identity;
- freshness requirements; and
- whether the result is formal, statistical, empirical, or assumed.

Without that model, a failure remains possible/unknown rather than probabilistically quantified.

Observed frequency may contribute evidence but must not silently become a semantic distribution.

### Recovery-oriented design

Crash/restart, failover, compensation, reconciliation, and recovery should be modeled as ordinary
state transitions that can be fault-injected and verified.

A component may expose a recovery contract such as:

```text
recovery {
    if effect = DEFINITELY_NOT_APPLIED:
        retry

    if effect = PARTIALLY_APPLIED:
        compensate_or_resume

    if effect = MAY_HAVE_COMMITTED:
        reconcile

    if local_state = CRASHED and checkpoint = VALID:
        restart_from(checkpoint)

    if effect = IRREVERSIBLE and invariant = UNKNOWN:
        escalate
}
```

The exact surface syntax remains open. The semantic requirement is that recovery actions identify
the preconditions that authorize them and the postconditions they claim.

## Core semantic model

A conceptual execution outcome may be represented as:

```text
ExecutionOutcome {
    operation_identity,
    event_identity,

    completion_state,
    result?,
    failure_class?,

    external_effects: [EffectOutcome],
    partial_effects: [PartialEffect],
    uncertain_effects: [UncertainEffect],

    resource_state,
    authority_state,

    observations: [ObservationRef],
    knowledge_state: [KnowledgeClaim],

    retry_semantics,
    compensation_semantics,
    rollback_semantics,
    reconciliation_semantics,
    restart_semantics,

    recovery_options: [RecoveryAction],

    assumptions,
    obligations,
    evidence,
}
```

This is a semantic sketch rather than final wire format.

### Completion state

The initial ontology should reserve states like:

```text
NotStarted
Executing
Completed
FailedBeforeEffect
PartiallyEffected
Cancelled
TimedOut
Crashed
ExternallyUncertain
```

These may later become a product of smaller orthogonal state dimensions rather than one enum.

For example, timeout and external uncertainty are not necessarily mutually exclusive:

```text
control_state = TIMED_OUT
external_effect_state = MAY_HAVE_COMMITTED
```

The final representation should preserve that compositionality.

### External effect state

An external effect should be able to progress through states such as:

```text
NotAttempted
Submitted
InProgress
DefinitelyNotApplied
PartiallyApplied
Applied
MayHaveApplied
Compensated
CompensationPending
CompensationFailed
Irreversible
```

These labels are provisional. The important distinction is that `MayHaveApplied` is neither success
nor failure; it is uncertainty about world state.

### Knowledge state

The runtime semantic graph should reserve knowledge/evidence states such as:

```text
KnownTrue
KnownFalse
Unknown
ConflictingEvidence
```

This does not require the RFC 0007 proof kernel to become a four-valued logic. These states describe
runtime evidence aggregation and recovery authority.

A fact with conflicting evidence should remain distinguishable from one for which no useful evidence
exists.

## Outcome versus knowledge

A core invariant is that world-state transitions and knowledge-state transitions are distinct.

For example:

```text
World:
    Transfer42 = APPLIED

Knowledge:
    Transfer42 = UNKNOWN
```

is legal.

Likewise:

```text
World:
    Worker7 = HEALTHY

Knowledge:
    Worker7 = SUSPECTED_FAILED
```

may be legal under an imperfect failure detector.

A recovery transformation must not substitute one dimension for the other.

## Ambiguous commit semantics

Ambiguous commit deserves first-class treatment because it is common and dangerous.

Consider:

```text
submit Operation42
remote accepts Operation42
remote commits Operation42
ack lost
caller deadline expires
```

The local outcome should preserve:

```text
control = TIMED_OUT
acknowledgement = NOT_OBSERVED
external_effect = MAY_HAVE_COMMITTED
```

The following transformation is unsound without additional evidence:

```text
TIMED_OUT -> DEFINITELY_NOT_APPLIED
```

A retry therefore requires one of:

- reconciliation proving the original effect absent;
- operation-level idempotence;
- a deduplicated realization keyed by stable operation identity;
- compensation/recovery semantics that make repetition equivalent under the contract; or
- another explicitly verified safety argument.

## Retry authority

Retry should be modeled as an authority-bearing transition.

Conceptually:

```text
RetryAuthority<Operation42> {
    basis,
    scope,
    attempt_limit?,
    deadline?,
    assumptions,
    evidence,
}
```

A retry operation consumes current outcome knowledge and produces a new attempt identity linked to
the same semantic operation identity.

The model should distinguish:

```text
SemanticOperationIdentity
AttemptIdentity
DeduplicationKey
```

A repeated attempt is not a new semantic business operation merely because a transport created a new
packet or process invocation.

### Retry classes

Initial classes should include:

```text
NeverRetry
RetryIfNotApplied
RetryIdempotent
RetryWithDeduplication<KeyClass>
ResumeFrom<CheckpointClass>
ReconcileBeforeRetry
CompensateBeforeRetry
UnknownRetrySafety
```

`UnknownRetrySafety` must disable automatic retry under conservative policy.

## Rollback, compensation, and reconciliation

The language should explicitly distinguish three recovery relations.

### Rollback

Rollback claims that the relevant semantic state can be restored to an earlier state under the
specified observation model.

```text
state_after_rollback observationally_equivalent_to state_before
```

This is a strong claim and may be impossible once irreversible external effects are visible.

### Compensation

Compensation performs new effects intended to restore a higher-level invariant.

```text
charge -> refund
```

is compensation, not rollback, because both effects may remain externally observable.

### Reconciliation

Reconciliation obtains new observations or performs protocol-specific queries to resolve uncertain
external state before deciding what recovery action is safe.

For example:

```text
query status(Operation42)
```

may transform:

```text
MAY_HAVE_COMMITTED
```

into:

```text
APPLIED
```

or:

```text
DEFINITELY_NOT_APPLIED
```

provided the status query itself has sufficient authority and semantics.

## Failure surface

Functions and operations should expose failure/recovery-relevant semantics in a compositional form.

Conceptually:

```text
failure_surface {
    AllocationFailure
    DeviceReset
    Timeout<Request42>
    Cancellation<ScopeS>
    ExternalUnknownCommit<Operation42>
    PartialWrite<FileF>
    ProcessCrash
}
```

Each failure class should be able to state:

```text
external_effect_state
returned_or_lost_resources
authority_state
retry_class
recovery_obligations
observation_requirements
```

A surface `Result<T,E>` may project part of this information for human convenience, but the canonical
semantic artifact must retain the richer state where it matters.

## Failure propagation carries state

Failure propagation should preserve semantic state rather than only control transfer.

Conceptually:

```text
FailureOutcome {
    cause = DeviceReset

    completed_effects = [...]
    uncertain_effects = [...]
    resource_state = [...]
    authority_state = [...]

    retry = UNKNOWN
    recovery_obligations = [...]
}
```

A handler that catches `DeviceReset` receives or can query this semantic state.

## Cancellation semantics

Cancellation composes RFC 0008 and RFC 0010 and must not mean arbitrary abortion.

A cancellation request may encounter an operation in states such as:

```text
NotStarted
CancelableBeforeEffect
InFlightCancelable
InFlightNonCancelable
Committed
MayHaveCommitted
```

The cancellation outcome must therefore state both:

```text
control_result
external_effect_result
```

For example:

```text
cancel acknowledged
```

may still require evidence defining whether the target guarantees:

```text
no future effect
```

or merely:

```text
best effort cancellation request accepted
```

Cancellation of structured child tasks must also preserve resource and authority ownership required
by RFC 0010.

## Observation semantics

### External observations are scoped facts

An external observation should carry at least:

```text
ExternalObservation {
    identity,
    proposition_or_value,
    source,
    event_identity,
    causal_position,
    observed_at,
    validity_scope,
    assumptions,
    evidence,
}
```

For example:

```text
DeviceReady(D)
```

may be established by a status read at event `E12` but remain valid only until another event, device
reset, lease expiry, or explicitly modeled invalidation.

External facts therefore participate in evidence freshness and invalidation much like compile-time
claims.

### Observation ledger

A bounded execution may emit an observation ledger:

```text
ObservationLedger {
    semantic_program_identity,
    execution_identity,
    source_models,
    ordered_or_causally_positioned_observations,
    uncontrolled_sources,
    evidence,
}
```

The ledger should capture only semantic observations required for replay or diagnosis, not necessarily
every physical machine event.

## Determinism taxonomy

RFC 0010 introduces determinism properties for concurrent execution. RFC 0011 generalizes the model
to external observations.

The language should distinguish at least:

```text
Deterministic
ScheduleIndependent
DeterministicGiven<ObservationSet>
Nondeterministic<DeclaredSources>
Probabilistic<Model>
Unspecified
```

A program may be nondeterministically scheduled but still `ScheduleIndependent`.

A program reading the clock may be:

```text
DeterministicGiven<{ClockObservation42}>
```

for replay purposes.

A random sample may be deterministic relative to an explicit entropy observation ledger while the
original source is probabilistic under a declared model.

## Environment, implementation, and probabilistic choice

The canonical semantic graph should not collapse all choice into one node kind.

### Environment choice

```text
EnvironmentChoice {
    alternatives,
    assumptions,
}
```

For safety proof, required properties must hold for every allowed alternative unless a weaker contract
explicitly says otherwise.

### Implementation choice

```text
ImplementationChoice {
    candidates,
    requirements,
    preferences,
}
```

The compiler/Forge may select one verified candidate.

### Probabilistic choice

```text
ProbabilisticChoice<ModelM> {
    alternatives,
    distribution,
    assumptions,
    evidence,
}
```

The distribution must be model-backed.

### Unresolved external state

```text
UnresolvedExternalState {
    possible_states,
    evidence,
}
```

must not be automatically interpreted as a distribution over those states.

## Recovery contracts

A recovery contract may be modeled conceptually as:

```text
RecoveryContract {
    trigger,

    eligible_actions: [
        Retry,
        Resume,
        Rollback,
        Compensate,
        Reconcile,
        Restart,
        Failover,
        Escalate,
        Abandon
    ],

    preconditions,
    postconditions,
    authority_requirements,
    resource_requirements,
    observation_requirements,
    attempt_budget,
    deadline_budget,

    obligations,
    evidence_policy,
}
```

Recovery actions should consume explicit authority. A component should not gain authority to issue a
refund, delete data, restart a device, or query privileged status merely because an error occurred.

## Recovery budgets

Autonomous recovery must be bounded.

The language/runtime should be able to express budgets such as:

```text
max_retry_attempts
max_total_recovery_time
max_compensation_effects
max_reconciliation_queries
max_restart_count
max_external_cost
max_authority_scope
```

These budgets compose with RFC 0004 recursive-refinement limits and RFC 0008 resource/effect budgets.

A recovery loop that exceeds its budget should become an explicit outcome rather than recurse
indefinitely.

## Crash and restart semantics

A crash destroys some execution context but not necessarily all state.

The target profile should classify storage/effect domains such as:

```text
volatile_local
persistent_local
external_service
device_internal
remote_peer
```

A crash event identifies which domains are assumed lost, preserved, or unknown.

For example:

```text
CrashEvent {
    loses = [volatile_local]
    preserves = [persistent_local]
    external = UNKNOWN
}
```

Restart must therefore re-establish local invariants and reconcile uncertain external domains.

## Failure atomicity

Operations should be able to declare failure-atomicity properties independent of implementation.

Initial research classes:

```text
FailureAtomic
PrefixRecoverable
Compensatable
PartialEffectsVisible
NoFailureAtomicityGuarantee
```

A `FailureAtomic` operation promises that after an applicable failure boundary, observers allowed by
the contract see either an allowed pre-state or post-state.

A `PrefixRecoverable` operation may leave a valid prefix that recovery can identify and resume or
compensate.

These properties generate obligations for each allowed failure point.

## External effect frontier

Long-running workflows need an explicit effect frontier.

Conceptually:

```text
ExternalEffectFrontier {
    definitely_applied,
    definitely_not_applied,
    partially_applied,
    uncertain,
    compensated,
    irreversible,
}
```

Checkpointing internal state without checkpointing or reconciling this frontier is insufficient for
correct recovery.

## Reconciliation

Reconciliation should be treated as an observation-producing recovery operation.

It may:

- query remote operation status;
- inspect durable logs;
- compare expected and observed resource state;
- negotiate protocol recovery state;
- inspect device generation/epoch counters; or
- use application-specific evidence.

Reconciliation itself can fail or remain uncertain. It does not guarantee resolution unless its
contract says so.

## Exactly-once language claims

MNCS should avoid an unqualified `ExactlyOnce` primitive for arbitrary external effects.

Instead, a realization may establish an exactly-once-equivalent abstract contract under explicit
assumptions using some combination of:

```text
stable operation identity
deduplication
durable receiver state
transactional commit
idempotent effect
reconciliation
```

The claim must identify its scope and failure model.

This prevents a transport-level delivery property from being confused with exactly-once semantic
execution.

## Idempotency is property-scoped

An operation may be idempotent under one observation model and not another.

For example, setting a database value twice may preserve final database contents while still creating
two audit records or external notifications.

Therefore:

```text
Idempotent<Operation, ObservationModel>
```

is safer than a universal boolean `idempotent` attribute.

The same principle applies to compensation and replay equivalence.

## Nondeterministic source authority

Reading time, randomness, device state, network input, or environment state is an effect and requires
appropriate authority under RFC 0008.

The resulting observation should carry the identity of the source capability used.

This allows policy to distinguish:

```text
cryptographic entropy
simulation entropy
wall clock
monotonic clock
trusted device status
untrusted network input
```

rather than treating all external input as equivalent.

## Time semantics

Time deserves special care because deadlines and replay rely on it.

The semantic model should distinguish at least:

```text
WallClockObservation
MonotonicDurationObservation
LogicalTime
Deadline
LeaseExpiry
```

A deadline is a relation between a start condition and a time source/model. It is not a generic proof
that another component failed.

Replayed execution may reproduce recorded wall-clock observations without claiming that physical time
has been rewound.

## Failure provenance

Failures should carry causal provenance.

A failure diagnostic should be able to answer:

```text
which event detected the failure?
which event caused the underlying condition, if known?
which external effects occurred before detection?
which resources remain valid?
which authority remains available?
which observations justify the classification?
```

Detection and cause are distinct.

For example, a timeout event may detect a problem whose cause is an earlier network partition, remote
crash, overload, or simply delayed scheduling.

## Error projection for surface languages

Human-oriented syntax may still expose familiar constructs:

```text
Result<T,E>
try
catch
?
throw
panic
```

but those forms should elaborate to or project from richer semantic outcomes where external effects
or recovery matter.

A pure local computation that fails before observable effect may collapse safely to a simpler error
value.

The language need not burden all arithmetic or local validation with full distributed recovery state.

The semantic richness should appear where required by the operation's effect/failure surface.

## Safe simplification

The compiler may simplify outcome state when stronger facts make distinctions irrelevant.

Examples:

```text
Pure operation
    => no external-effect uncertainty dimension required

KnownFailedBeforeEffect
    => retry may be derived if resources/authority permit

Idempotent under current observation model
    => MAY_HAVE_APPLIED may still permit retry
```

Simplification requires proof/evidence; it must not erase uncertainty merely for convenience.

## Interactions with RFC 0008 I/O

RFC 0008 already models external operations, cancellation, partial completion, and resource ownership.
RFC 0011 adds explicit outcome/knowledge/recovery semantics.

For an I/O request:

```text
submit IORequest
    -> InFlight<Resource, Request>
```

RFC 0011 determines what completion/cancellation/reset/timeout means for external effect state and
which recovery actions may lawfully reclaim or reuse the resource.

For DMA or device commands, a reset may leave both memory ownership and device-side completion
uncertain. Recovery must not return writable authority to the host until the relevant ownership
obligation is resolved.

## Interactions with RFC 0009 memory

Crash/recovery can invalidate or preserve different allocation epochs and memory regions.

A checkpoint that serializes logical object state does not preserve raw reference provenance unless
the restoration contract establishes a new valid provenance relation.

Restored resources therefore receive new identities/epochs where appropriate rather than reviving
stale references by address reuse.

## Interactions with RFC 0010 concurrency

Failure and cancellation are events in the RFC 0010 concurrency graph.

A child task failure must update its structured scope according to the scope policy:

```text
fail_fast
collect_failures
cancel_siblings
continue_independent_children
```

or another explicit rule.

Failure propagation must preserve:

- event causality;
- external-effect frontiers;
- authority/resource transfer;
- observation provenance;
- progress assumptions; and
- cancellation state.

A timeout waiting on a concurrent event is an observation about progress, not automatically proof
that the producer failed.

## Effect handlers and recovery authority

Handlers should not gain ambient recovery capabilities.

For example:

```text
handler NetworkFailure
```

may have authority to retry a read but not automatically gain authority to refund a payment or reset a
device.

Recovery authority is capability-bounded like all other MNCS authority.

## Proof and evidence model

Failure/recovery claims may be supported by different evidence classes:

```text
kernel-checkable proof
independently checkable certificate
model-checker result
crash-consistency exploration
fault-injection result
property-based test
record/replay corpus
statistical measurement
external service contract
assumption
UNKNOWN
```

These classes are not interchangeable.

For example:

```text
100000 successful injected-crash tests
```

may be strong empirical evidence but does not by itself prove universal crash consistency.

Likewise, a remote provider's documented idempotency guarantee is an explicit external assumption or
contract dependency rather than a theorem about the provider implementation.

## Obligation generation

Constructs should generate obligations automatically.

### Retry

```text
retry op
```

generates obligations such as:

```text
retry authority valid
current outcome permits retry
operation identity stable
resource state permits new attempt
deduplication/idempotency evidence current
attempt budget remains
```

### Compensation

```text
compensate effect
```

generates:

```text
compensation authority valid
compensation applicable to current effect state
postcondition restores declared invariant
compensation itself has modeled failure semantics
```

### Rollback

```text
rollback checkpoint
```

generates:

```text
checkpoint valid
restored internal state consistent
external effect frontier compatible
stale provenance not revived
```

### Reconciliation

```text
reconcile op
```

generates:

```text
status source authoritative for queried property
observation freshness sufficient
mapping from observation to outcome state valid
```

### Probability claim

```text
probability<ModelM>(event) <= p
```

generates:

```text
model identity bound
parameters evidenced
scope current
assumptions explicit
method class appropriate
```

## Failure-aware lowering envelopes

A semantic recovery contract may admit multiple realizations.

Conceptually:

```text
RecoveryRealization {
    semantic_contract_identity,
    target_profile,

    mechanism:
        retry_loop |
        deduplicated_retry |
        checkpoint_resume |
        write_ahead_log |
        journal |
        saga_compensation |
        restart |
        failover |
        reconciliation_protocol,

    authority_used,
    storage_used,
    external_assumptions,
    generated_artifact,
    discharged_obligations,
    remaining_assumptions,
    measurements,
    evidence,
}
```

Forge may compare realizations under objectives such as:

```text
recovery_latency
steady_state_overhead
storage_cost
external_effect_risk
proof_cost
implementation_complexity
authority_required
replayability
availability
```

A faster realization that broadens authority or weakens uncertainty handling must not be selected
unless the semantic contract permits it.

## Conservative behavior for UNKNOWN

`UNKNOWN` is expected in real failure handling.

Examples:

```text
commit state = UNKNOWN
retry safety = UNKNOWN
remote liveness = UNKNOWN
checkpoint freshness = UNKNOWN
probability model = UNKNOWN
```

The default behavior should be conservative:

- disable unsafe retry;
- reconcile if an authorized reconciliation path exists;
- preserve uncertain external effects;
- avoid reclaiming resources whose external ownership is unresolved;
- use stronger durability/recovery mechanisms where declared;
- escalate when policy requires human or higher-authority resolution; or
- reject promotion.

`UNKNOWN` must never be silently coerced to whichever state enables progress.

## Diagnostics

A failure diagnostic should be structured rather than merely textual.

Example:

```text
Diagnostic: RETRY_NOT_AUTHORIZED

operation: Payment42
attempt: Attempt2
control_outcome: TIMED_OUT
external_effect: MAY_HAVE_COMMITTED
acknowledgement: NOT_OBSERVED
retry_safety: UNKNOWN

missing evidence:
    Idempotent<Payment42, CustomerBalanceView>
    OR Deduplicated<Payment42, Key42>
    OR KnownNotApplied<Payment42>

available recovery:
    Reconcile<Payment42>
```

This gives Forge or another agent a bounded repair space instead of encouraging blind retry.

## Negative requirements

The following behaviors must be rejected or remain unresolved under conservative policy.

### Timeout laundering

```text
Timeout -> DefinitelyNotApplied
```

without sufficient evidence.

### Blind non-idempotent retry

```text
MAY_HAVE_APPLIED
+ UnknownRetrySafety
-> retry
```

must be rejected.

### Compensation-as-rollback laundering

A compensating effect must not justify the claim that the original effect never occurred.

### Restoring stale references

A checkpoint restore must not resurrect RFC 0009 references solely because the same machine address
is reused.

### Invented probability

```text
UNKNOWN external state
-> 50/50 distribution
```

must be rejected unless a declared model justifies that distribution.

### Failure suspicion as fact

```text
DeadlineElapsed
-> KnownFailed<Remote>
```

must be rejected unless the active failure model establishes that implication.

### Unbounded automatic recovery

An autonomous retry/restart/compensation loop without a finite policy budget must be rejected where
bounded recovery is required.

### Replay with uncaptured nondeterminism

A replay claim must not be strengthened to deterministic replay when semantically relevant
nondeterministic sources remain uncaptured.

### Unsafe resource reclamation

Resources whose external owner or in-flight effect is uncertain must not regain conflicting authority
without evidence authorizing reclamation.

## Experimental plan

RFC 0011 should advance through bounded experiments rather than immediate full implementation.

### Track 1: ambiguous commit interpreter

Build a minimal semantic operation with states:

```text
NotAttempted
Submitted
Applied
AckObserved
AckLost
TimedOut
MayHaveApplied
```

Inject the sequence:

```text
apply -> lose ack -> timeout
```

and verify that the final semantic state is uncertainty rather than known failure.

Acceptance criteria:

- timeout never proves non-application;
- the effect frontier records the uncertain operation;
- retry is rejected without additional evidence;
- reconciliation can resolve the state in a controlled fixture.

### Track 2: retry-safety verifier

Create fixtures for:

```text
pure operation
idempotent operation
non-idempotent operation
deduplicated operation
unknown retry safety
```

Acceptance criteria:

- automatic retry accepted for appropriately evidenced cases;
- blind retry rejected for uncertain non-idempotent effects;
- stale deduplication evidence invalidates retry authority;
- attempt budgets are enforced.

### Track 3: saga/compensation experiment

Model a three-step workflow with committed effects and compensations.

Inject failure after every step and during every compensation.

Acceptance criteria:

- committed and compensated effects remain historically distinct;
- irreversible steps disable false rollback claims;
- compensation failures remain explicit;
- the recovery frontier is deterministic for a fixed observation stream.

### Track 4: crash-point explorer

Build a bounded persistent-state operation and inject a crash at every semantic persistence boundary.

Acceptance criteria:

- each recovered state belongs to the declared crash-consistency set;
- illegal intermediate states are detected;
- checkpoint restore does not erase external-effect uncertainty;
- proof, model-checking, and empirical exploration results retain distinct evidence classes.

### Track 5: observation-ledger replay

Create a bounded program using:

```text
clock
entropy
network input
scheduler/event choice
```

Capture an observation ledger and replay it.

Acceptance criteria:

- semantic outputs and causal observations reproduce under the same ledger;
- missing observations produce explicit replay failure/UNKNOWN;
- uncontrolled sources remain identified rather than silently ignored.

### Track 6: nondeterminism classifier

Construct equivalent-looking examples of:

```text
environment nondeterminism
implementation choice
probabilistic choice
unresolved external state
```

Acceptance criteria:

- the four classes remain distinguishable through HIR/SSA traceability;
- safety checking quantifies environment alternatives conservatively;
- Forge may choose implementation alternatives;
- unresolved state cannot gain an invented probability distribution.

### Track 7: timeout/failure-detector experiment

Model a healthy remote service whose response is delayed beyond a local deadline.

Acceptance criteria:

- timeout records an observation and suspicion only;
- no known-failure claim appears without stronger evidence;
- a later response can refine knowledge without semantic contradiction;
- diagnostics identify why automatic retry was or was not safe.

### Track 8: recovery-realization tournament

Represent one semantic recovery contract and compare multiple valid realizations where feasible:

```text
idempotent retry
deduplicated retry
checkpoint/resume
log-based recovery
compensation
reconciliation
```

Measure:

```text
steady-state cost
recovery latency
storage cost
authority required
proof/verifier cost
failure coverage
replayability
```

Acceptance criteria:

- all candidates preserve the same declared semantic contract;
- a cheaper candidate that weakens recovery guarantees is rejected;
- accepted/rejected realization history is retained.

## Cross-RFC implementation order

A reasonable first implementation slice is:

1. define identity-bearing `ExecutionOutcome`, `EffectOutcome`, and `Observation` research objects;
2. add `MAY_HAVE_APPLIED` and explicit timeout semantics to one RFC 0008 I/O fixture;
3. implement stable operation identity versus attempt identity;
4. add a minimal retry-safety verifier;
5. emit structured retry-denial diagnostics;
6. add one reconciliation fixture;
7. add a bounded observation ledger and replay interpreter;
8. preserve outcome/observation identities through HIR and SSA traceability;
9. add fault-injection fixtures to Forge;
10. only then expand into compensation, crash consistency, and probabilistic models.

This keeps the initial trusted change narrow while testing the highest-value semantic distinctions.

## Graduation criteria

A feature from this RFC should graduate only when it demonstrates measurable machine benefit relative
to its complexity.

Useful criteria include:

- fewer unsafe retries;
- correct preservation of ambiguous external state;
- improved fault-injection diagnosis;
- deterministic replay coverage;
- narrower recovery authority;
- smaller repair/recovery search regions;
- more precise evidence invalidation;
- reduced manually encoded recovery convention;
- successful alternative recovery realizations over one semantic contract;
- manageable proof/verifier cost; and
- preservation through IR and backend lowering.

No feature graduates merely because its theory is elegant.

## Open research questions

The following remain intentionally unresolved.

1. Should outcome and knowledge use independent finite lattices or one structured product type?
2. Which knowledge states beyond `KNOWN`, `UNKNOWN`, and `CONFLICTED` materially help machine
   generation?
3. How much effect-status history should remain in canonical artifacts versus execution evidence?
4. Should idempotence be represented as a proposition over observation models, a typeclass-like
   property, or a general equivalence relation?
5. Which recovery authorities should be linear or affine?
6. How should operation identity interact with remote deduplication windows and expiring leases?
7. Can recovery contracts compose automatically across nested effect handlers?
8. Which classes of crash consistency admit independently checkable compact certificates?
9. How should replay identity treat target realization changes?
10. How should controlled probabilistic choice interact with RFC 0010 schedule nondeterminism?
11. When can compensation obligations be synthesized automatically from inverses or group-like
    algebraic structure?
12. How should distributed partitions and replication extend the external-effect frontier without
    turning RFC 0011 into a complete distributed-systems specification?
13. When is escalation to a human/operator a semantic recovery action versus an orchestration policy?
14. Which failure assumptions should become target-profile facts and which remain program contracts?
15. How should observation freshness interact with long-running proof/evidence manifests?

## Relationship to future distributed semantics

RFC 0011 intentionally stops short of defining consensus, replication, partition tolerance, quorum
systems, or distributed consistency models in full.

However, it establishes primitives those future semantics will require:

```text
uncertain external outcome
operation identity
observation provenance
failure suspicion
retry authority
reconciliation
external-effect frontiers
compensation
crash/restart
```

A future distributed RFC should build on these rather than redefine failure from scratch.

## Security implications

Failure handling is an authority-sensitive path.

Attackers may intentionally induce:

```text
timeouts
duplicate responses
partial responses
connection resets
resource exhaustion
replay
false status information
```

hoping recovery code will broaden authority, repeat irreversible effects, or bypass invariants.

RFC 0011 therefore requires:

- explicit recovery capabilities;
- stable operation identity where deduplication is claimed;
- evidence-bound reconciliation sources;
- finite recovery budgets;
- no optimistic coercion of uncertainty;
- auditability of recovery actions; and
- preservation of authority changes in semantic deltas.

Recovery must not become a privileged escape hatch around the normal capability model.

## Machine-native implications

The major machine advantage is not a new exception syntax. It is that an autonomous system can
reason directly about a recovery state graph.

Instead of generating:

```text
try request
catch timeout
sleep
retry
```

because that pattern is common in human code, a machine can evaluate:

```text
current external state
+ current knowledge
+ operation repeatability
+ authority available
+ recovery budget
+ target mechanisms
+ evidence
```

and select a recovery action whose preconditions are actually satisfied.

Likewise, Forge can search between:

```text
retry
reconcile
resume
compensate
restart
failover
escalate
```

as constrained semantic alternatives rather than textual code idioms.

This is especially important for recursive refinement: a repair agent must not "fix" a timeout by
adding retries if the original effect may already have committed.

## Proposed thesis

RFC 0011 adopts the following working thesis:

> **MNCS models execution failure as transitions among explicit outcome, external-effect, resource,
> authority, observation, and knowledge states. Nondeterministic observations carry provenance;
> retries and recovery require semantic authority; and exceptions, transactions, logs,
> compensation, restart, deduplication, reconciliation, and replay are verified realizations of those
> contracts.**

That thesis should remain stable even if the final surface syntax, lattice representation, handler
model, or runtime architecture changes.
