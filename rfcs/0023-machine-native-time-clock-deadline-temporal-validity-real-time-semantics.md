# RFC 0023: Machine-Native Time, Clock, Deadline, Temporal Validity, and Real-Time Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.2–1.0
- **Depends on:** RFC 0001, RFC 0002, RFC 0003, RFC 0004, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0013, RFC 0014, RFC 0015, RFC 0016, RFC 0017, RFC 0018, RFC 0019, RFC 0020, RFC 0021, RFC 0022

## Summary

MNCS Language should not treat “time” as one universal scalar or one ambient `Timestamp` type.

The central rule is:

> **Causal order, logical time, physical elapsed time, synchronized reference time, civil time, event time, processing time, deadlines, validity intervals, execution schedules, and bounded computation are distinct semantic concepts.**

A second rule is equally important:

> **A timestamp is not truth about when something happened. It is a clock observation produced by an identified clock under a particular synchronization, uncertainty, resolution, stability, and trust model.**

RFC 0023 establishes a machine-native temporal framework in which programs can specify temporal meaning and acceptable timing envelopes while leaving clocks, schedules, task placement, preemption, synchronization mechanisms, and target-specific execution strategy open where the contract permits.

The intended result is a system capable of representing questions such as:

- Did event `A` causally precede event `B`, or do their physical timestamps merely compare that way?
- Which clock domain produced a timestamp?
- Can two clock readings be compared at all?
- How uncertain is a synchronized physical-time reading?
- Is a clock monotonic, civil, reference, logical, virtual, simulated, replayed, or event-time based?
- Does a timestamp denote an instant, an interval estimate, an event-time fact, an observation time, or a knowledge time?
- Is a temporal comparison definitely true, definitely false, or unresolved because uncertainty intervals overlap?
- What is the difference between a deadline and a timeout?
- Is a task periodic, sporadic, aperiodic, or event-triggered?
- What does missing a deadline mean for this operation?
- Does an implementation merely satisfy the same deadline, or is it temporally trace-equivalent to another implementation?
- Can physical execution move inside a temporal envelope while preserving logical effect time?
- Can an untrusted Forge scheduler generate a checkable schedule witness?
- Is a timing guarantee still applicable after the CPU, scheduler, compiler, clock source, or synchronization policy changes?
- Does a temporal validity interval still definitely contain the current time once clock uncertainty is considered?
- Is a stream event late in event time, processing time, or both?
- Is a watermark a clock reading or an evidence claim about temporal completeness?
- Can the same program run under physical, simulation, replay, or virtual time without changing its logical temporal contract?

This RFC does not require one universal clock, one scheduler, one real-time operating system, one synchronization protocol, or one civil-time representation. It establishes a semantic framework within which multiple temporal domains and realization strategies can coexist without being conflated.

---

## Motivation

MNCS already models causality, external observation, runtime context, assurance, relation scope, completion/liveness, target realizations, and bounded computation.

However, the existing RFC set does not yet provide one constitutional answer to questions such as:

- what an instant means independently of a clock implementation;
- whether two timestamps are comparable;
- whether physical-time order implies causal order;
- how uncertainty in clock synchronization affects temporal claims;
- how monotonic elapsed time differs from UTC/civil time;
- how logical time differs from physical time;
- how validity intervals differ from creation timestamps;
- how deadline semantics differ from timeout policy;
- how periodic release differs from completion deadline;
- how logical effect time differs from physical instruction execution time;
- how a runtime proves that a schedule satisfies timing requirements;
- how simulation/replay time can preserve temporal semantics without wall-clock waiting;
- how event time differs from processing time in unbounded streams;
- how timestamp or schedule evidence becomes stale when its environment changes; and
- how Forge may search schedules without silently weakening protected timing behavior.

Conventional systems often collapse these concerns into APIs such as:

```text
now()
Instant
Timestamp
sleep(10ms)
timeout(100ms)
```

Those APIs are useful implementation mechanisms, but by themselves they do not express the temporal ontology required for machine-generated, evidence-bearing, target-sensitive software.

MNCS should instead let a program express **what must happen when, relative to which temporal domain, under which uncertainty and assumptions**, while letting realization remain open where possible.

---

## Constitutional principles

### 1. No universal timestamp

> **Every temporal observation belongs to a declared clock or temporal domain.**

A scalar value without clock identity, scale, or interpretation is incomplete.

### 2. Causal order is not physical-time order

> **Physical timestamp comparison must never fabricate causality.**

RFC 0010 owns causal semantics. RFC 0023 defines how temporal domains may reflect, approximate, or remain independent from causal order.

### 3. Clock readings are observations

> **A physical clock reading is evidence produced by a clock realization, not privileged truth.**

Clock identity, synchronization state, uncertainty, resolution, rate/stability information, and observation provenance may affect what conclusions are authorized.

### 4. Monotonic time is not civil time

Elapsed-time measurement and human calendar/reference time are different semantic operations.

### 5. Instants are not durations

They have different algebras and must not be silently interchanged.

### 6. Duration arithmetic requires compatible domains

Subtracting unrelated clock readings is invalid unless a clock relation or mapping establishes meaning.

### 7. Resolution is not accuracy

A clock exposing nanosecond digits does not thereby establish nanosecond temporal truth.

### 8. Accuracy is not stability

A stable clock may have offset. An accurate clock may later drift.

### 9. Deadlines are not timeouts

A deadline is a temporal requirement. A timeout is an operational policy for reacting after a wait interval.

### 10. Period is not deadline

Release cadence and required completion time are separate.

### 11. Computation budget is not physical deadline

RFC 0022 semantic-step/fuel bounds must not be silently converted into RFC 0023 physical-time claims.

### 12. Logical effect time is not physical execution time

A valid implementation may execute work at different physical times while preserving the declared logical temporal interface.

### 13. Schedule synthesis is not schedule verification

Expensive or untrusted scheduling search may occur outside the trusted verifier if the resulting schedule can be checked against explicit assumptions.

### 14. Temporal validity is not creation time

An artifact may be created, observed, verified, effective, valid, received, and expired at different times.

### 15. Temporal uncertainty propagates

A temporal conclusion must not claim more precision or ordering certainty than its source clock evidence justifies.

### 16. Temporal comparison may be unresolved

Overlapping clock uncertainty intervals must not be forced into arbitrary total order.

### 17. Reading a clock is an effect

Clock observation belongs to RFC 0008 effect/capability semantics rather than being treated as a pure function.

### 18. Simulation and replay clocks are legitimate realizations

A temporal program need not advance at wall-clock speed.

### 19. Event time is not processing time

The time represented by data and the time a runtime receives/processes that data are different facts.

### 20. Temporal-frontier claims are knowledge, not clocks

A watermark/frontier expresses bounded knowledge about what earlier events remain possible under a model.

### 21. Timing guarantees are environment-sensitive where their evidence is environment-sensitive

Changing target hardware, scheduler, compiler, runtime, clock source, synchronization policy, or workload assumptions may invalidate temporal evidence.

### 22. Performance never authorizes temporal weakening

A faster implementation cannot silently violate protected timing, jitter, event-order, or validity semantics.

---

## Seven semantic planes

RFC 0023 proposes seven semantically distinct planes.

### Temporal-order plane

Owns relations such as:

```text
causal order
logical order
declared temporal precedence
```

Causal order itself remains defined by RFC 0010.

### Clock plane

Owns:

```text
clock domains
clock readings
clock uncertainty
clock quality
synchronization state
clock mappings
```

### Temporal-quantity plane

Owns:

```text
instants
durations
intervals
boundary policies
```

### Application temporal plane

Owns:

```text
release constraints
periods
minimum interarrival
jitter
deadlines
validity intervals
event time
processing time
knowledge time
```

### Execution plane

Owns:

```text
physical execution windows
logical effect times
schedules
resource windows
platform timing assumptions
```

### Verification plane

Owns:

```text
timed state machines
clock constraints
schedulability claims
schedule witnesses
temporal counterexamples
```

### Assurance plane

Owns integration with RFC 0018 for:

```text
clock trust
clock freshness
schedule applicability
WCET applicability
synchronization evidence
uncertainty claims
promotion authority
```

---

## Candidate semantic objects

A future canonical model may include objects conceptually similar to:

```text
ClockDomain
ClockIdentity
ClockCapability
ClockObservation
ClockQuality
ClockSynchronizationState
ClockRateBound
ClockRelation
ClockMapping
ClockEvidence

LogicalInstant
PhysicalInstant
ReferenceInstant
CivilDateTime
VirtualInstant
SimulationInstant
ReplayInstant

Duration
TemporalInterval
TemporalBoundaryPolicy
TemporalConstraint

EventTime
IngestionTime
ProcessingTime
ObservationTime
KnowledgeTime
ValidTime

ReleaseConstraint
Period
MinimumInterarrival
JitterConstraint
Deadline
DeadlinePolicy
TimeoutPolicy
TemporalTaskContract

TimedStateMachine
TimedTransition
ClockConstraint
TemporalInvariant

Schedule
ScheduleEntry
ScheduleWitness
SchedulabilityClaim
WorstCaseExecutionBound
TemporalResourceWindow

TemporalFrontier
TemporalApplicability
TemporalRelationClaim
TemporalCounterexample
TemporalDelta
```

This RFC does not freeze exact surface syntax or serialization names.

---

# Part I — temporal order and logical time

## Causal order

RFC 0010 defines causality and happens-before relations.

RFC 0023 must preserve the rule:

```text
causal_before(A, B)
```

is not derivable merely from:

```text
physical_timestamp(A) < physical_timestamp(B)
```

unless additional evidence establishes that the clocks and event model justify that inference.

## Logical time

Logical time represents ordering or progress inside a declared logical clock domain.

Conceptually:

```text
LogicalInstant<ClockIdentity>
```

A logical instant need not correspond to elapsed seconds.

## Scalar logical clocks

A scalar logical clock may preserve causal precedence:

```text
A -> B
implies
L(A) < L(B)
```

but the converse must not be assumed.

## Causal/vector clocks

Richer logical clocks may retain partial-order/concurrency information rather than imposing a total order.

RFC 0023 should permit such clock realizations without requiring one canonical vector-clock shape.

## Clock identity

Clock readings from unrelated logical domains are not directly comparable:

```text
LogicalInstant<C1>(42)
LogicalInstant<C2>(42)
```

A relation or mapping is required before cross-domain comparison.

## Ordering convenience versus semantic causality

A runtime may impose a total order for serialization or indexing convenience.

That total order must not silently become a causal claim.

---

# Part II — hybrid clocks

## Hybrid logical/physical clocks

MNCS should permit temporal realizations combining:

```text
physical-time proximity
+
logical causality-preserving metadata
```

Conceptually:

```text
HybridClockObservation {
    physical_component
    logical_component
    clock_identity
    uncertainty
}
```

A hybrid clock is one temporal realization, not the universal MNCS clock.

## Causality and physical proximity remain separate properties

A hybrid representation may provide useful ordering and physical-time approximation simultaneously.

The system must still preserve which conclusions derive from which component.

---

# Part III — physical clock observations

## Physical clocks as evidence sources

A physical/synchronized clock should conceptually return a bounded observation such as:

```text
ClockObservation {
    earliest
    latest

    clock_identity
    time_scale

    resolution
    uncertainty_model

    synchronization_state
    observation_provenance
}
```

The intended claim is:

```text
actual_reference_time is within [earliest, latest]
```

under the observation's declared assumptions.

## Exact-instant interfaces may be derived views

A target API may expose a scalar time value for convenience.

Canonical semantics must retain whether that value represents:

```text
exact logical instant
center estimate
lower bound
upper bound
opaque platform timestamp
```

rather than erasing the distinction.

## Uncertainty intervals

Clock uncertainty should be represented structurally, not hidden in comments.

A comparison between two physical observations may therefore yield:

```text
DEFINITELY_BEFORE
DEFINITELY_AFTER
OVERLAPS
UNRESOLVED
```

as appropriate to the relation model.

## Example

```text
A in [10.000, 10.005]
B in [10.010, 10.015]
```

can establish:

```text
A definitely before B
```

whereas:

```text
A in [10.000, 10.008]
B in [10.005, 10.012]
```

cannot establish a strict physical order without stronger evidence.

---

# Part IV — clock quality

## Distinct clock-quality dimensions

Candidate dimensions include:

```text
resolution
accuracy
uncertainty
precision
stability
drift bound
synchronization age
traceability
```

They must remain distinct.

## Resolution

Resolution describes the smallest distinguishable/reportable increment of a clock interface.

It does not prove accuracy.

## Accuracy/uncertainty

Accuracy concerns relation to a reference. Uncertainty bounds how wrong the observation may be under a model.

## Stability

Stability concerns how a clock rate changes over time.

A stable clock may still have a fixed offset from a reference.

## Drift

Clock drift should be representable as a rate relationship.

Conceptually:

```text
ClockRateBound {
    relative_to
    minimum_rate
    maximum_rate
    validity_interval
}
```

## Growing uncertainty

A clock realization may define uncertainty growth after synchronization:

```text
uncertainty(t)
    <= initial_uncertainty + drift_bound * elapsed
```

The exact model remains realization-specific and evidence-bearing.

---

# Part V — synchronization and reference time

## Synchronization is explicit

A clock must not silently claim global reference quality.

A synchronization state may identify:

```text
protocol
reference source
last synchronization
estimated offset
estimated rate error
uncertainty
validity interval
```

## Clock synchronization as evidence

A claim that two physical clocks are mutually comparable depends on synchronization evidence.

Such evidence may become stale independently of the clocks themselves.

## NTP/PTP-like realizations

Network clock synchronization protocols such as NTP- or PTP-like mechanisms are valid implementation families.

They do not become language semantics.

The language owns the required temporal relation and acceptable uncertainty.

## Atomic/reference time scales

MNCS should permit explicit reference time scales such as:

```text
TAI-like continuous atomic scale
UTC-like civil/reference scale
GPS-like scale
custom reference scale
```

without assuming every application uses the same scale.

---

# Part VI — time scales and civil time

## Instants require a coordinate system

An absolute/reference instant is meaningful only relative to a declared temporal coordinate system.

Conceptually:

```text
ReferenceInstant<TimeScale>
```

## Civil time as representation

Civil date/time should be treated as a representation of temporal information under:

```text
time scale
timezone rules
calendar rules
leap-second policy
versioned conversion data
```

not as the primitive arithmetic domain for elapsed time.

## Civil time versus elapsed duration

Subtracting two civil date strings is not automatically equivalent to measuring physical elapsed duration.

A conversion contract must specify the time scale and calendar/timezone interpretation.

## Leap adjustments

Civil/reference time scales may contain adjustments not present in continuous atomic scales.

Duration arithmetic must preserve that distinction.

## Timezone rule versioning

Timezone databases and civil-time conversion rules may change.

A civil conversion therefore may depend on versioned external data and should preserve that dependency where reproducibility matters.

---

# Part VII — monotonic clocks

## Monotonic elapsed time

A monotonic clock is suitable for:

```text
start/end elapsed measurement
timeouts
local deadline timers
profiling
```

where backward civil-time corrections must not alter elapsed duration.

## Monotonic does not mean globally synchronized

A local monotonic clock may be incomparable to another node's monotonic clock.

## Monotonic instant identity

Conceptually:

```text
MonotonicInstant<ClockIdentity>
```

prevents accidental cross-clock subtraction.

## Duration observation

A measured duration should retain clock provenance when needed:

```text
DurationObservation {
    clock_identity
    start
    end
    duration
    uncertainty
}
```

---

# Part VIII — instants, durations, and intervals

## Instants

An instant identifies a point in a declared temporal domain.

## Durations

A duration represents elapsed quantity.

Candidate algebra:

```text
Instant + Duration -> Instant
Instant - Instant -> Duration
Duration + Duration -> Duration
Duration - Duration -> Duration
```

Operations such as:

```text
Instant + Instant
```

have no default semantic meaning.

## Temporal intervals

Intervals should be first-class:

```text
TemporalInterval {
    start
    end
    boundary_policy
}
```

## Boundary policy

Examples:

```text
closed
open
left_closed
right_closed
```

Boundary semantics matter for validity and deadline edges.

## Interval relations

Candidate relations include:

```text
before
after
meets
overlaps
during
contains
starts
finishes
equals
```

These are semantic relations, not just repeated endpoint arithmetic.

---

# Part IX — temporal occurrence versus temporal validity

## Multiple timestamps for one artifact

An artifact may carry different temporal facts:

```text
created_at
observed_at
received_at
verified_at
valid_from
valid_until
expires_at
recorded_at
```

They must not be collapsed.

## Observation interval

Evidence may have an interval during which an observation was gathered.

## Validity interval

Evidence may apply during a different interval.

Conceptually:

```text
EvidenceTemporalScope {
    observation_interval
    validity_interval
}
```

## Freshness as applicability

Freshness should be represented as a temporal applicability predicate rather than scattered age arithmetic.

Candidate outcomes:

```text
FRESH
STALE
NOT_YET_VALID
UNRESOLVED
```

## Uncertainty at validity boundaries

If current-time uncertainty overlaps an expiration boundary, `UNRESOLVED` may be the only defensible conclusion.

RFC 0018 determines whether unresolved freshness authorizes the action.

---

# Part X — valid time and knowledge time

## Valid time

Valid time answers:

> When was this fact considered true in the modeled world?

## Knowledge/record time

Knowledge time answers:

> When did the system learn, store, or attest to this fact?

## Why the distinction matters

This separation is useful for:

```text
configuration history
policy changes
evidence corrections
historical data
transactions
backfilled observations
late-arriving events
```

## Temporal correction

A later observation may revise knowledge about an earlier valid-time interval.

The system should preserve both timelines rather than rewriting history into one timestamp.

---

# Part XI — deadlines and timeout policies

## Deadline

A deadline is a requirement that a specified event/state/observation occur no later than a temporal bound.

Conceptually:

```text
Deadline {
    subject
    bound
    clock_domain
    miss_semantics
}
```

## Timeout policy

A timeout defines an operational reaction after a wait duration or instant:

```text
TimeoutPolicy {
    wait_bound
    on_timeout
}
```

## Timeout does not imply deadline miss

A timeout may fire early to permit fallback before the true deadline.

## Deadline result

A deadline checker may produce:

```text
DeadlineResult {
    required_by
    observed_completion
    clock_domain
    uncertainty
    lateness
    disposition:
        MET
        MISSED
        UNRESOLVED
}
```

## Uncertain boundary

If completion uncertainty overlaps the deadline boundary, the result must not be forced into `MET` or `MISSED` without additional policy/evidence.

---

# Part XII — release, period, interarrival, and jitter

## Release constraint

Defines when a task becomes temporally eligible or logically released.

## Period

Defines recurring cadence:

```text
Periodic {
    period
    phase
}
```

## Sporadic arrival

Defines a minimum interarrival separation:

```text
Sporadic {
    minimum_interarrival
}
```

## Aperiodic arrival

Aperiodic work may lack a predictable period but still carry deadlines or service bounds.

## Jitter

Jitter expresses permitted deviation around a temporal reference/envelope.

Conceptually:

```text
JitterConstraint {
    nominal
    lower_deviation
    upper_deviation
}
```

## Period is not deadline

A task may have:

```text
period = 10ms
deadline = 8ms
```

which is semantically different from a 10ms deadline.

---

# Part XIII — deadline criticality

## No universal realtime boolean

MNCS should not reduce timing semantics to:

```text
realtime = true
```

## Deadline miss semantics

A temporal contract should describe the meaning of missing a bound:

```text
forbidden
result_invalid
degraded_quality
best_effort
fallback_required
compensation_required
```

## Hard/firm/soft categories as derived profiles

Traditional real-time categories may be useful names, but the canonical meaning should come from explicit miss semantics rather than one enum alone.

---

# Part XIV — timed state machines

## Timed transition systems

MNCS should support analyzable temporal state models:

```text
TimedStateMachine {
    states
    clocks
    transitions
    guards
    resets
    invariants
}
```

## Clock variables

A timed state machine may use logical model clocks that are distinct from runtime clock implementations.

## Guards

Example:

```text
x <= 50ms
```

may guard a transition.

## Resets

Transitions may reset model clocks:

```text
on request:
    x := 0
```

## State invariants

A state may require:

```text
x <= 50ms
```

for all time spent in that state.

## Semantic versus realized clocks

Timed-model clocks are semantic verification devices and need not map one-to-one to hardware clocks.

---

# Part XV — metric temporal properties

## Qualitative liveness handoff

RFC 0022 owns properties such as:

```text
Eventually(P)
P LeadsTo Q
```

## Quantitative temporal bounds

RFC 0023 adds bounded forms such as:

```text
EventuallyWithin<50ms>(P)
LeadsToWithin<P,Q,[0ms,50ms]>
```

## Persistent temporal constraints

Examples:

```text
Always(request -> response within 50ms)
```

```text
state S holds continuously for at least 100ms
```

## Scope

The exact temporal-logic surface remains open. The canonical model must preserve the bounded temporal meaning.

---

# Part XVI — non-Zeno and time-divergence properties

## Zeno behavior

A timed model may admit infinitely many transitions within finite modeled time.

That must not be ignored.

## Non-Zeno/time-divergence claims

Candidate properties include:

```text
NonZeno
TimeDivergent
```

## Relationship to RFC 0022

RFC 0022 owns completion/progress. RFC 0023 owns whether modeled physical/logical time itself advances appropriately.

A productive computation need not terminate, but its timed realization may still require non-Zeno progress.

---

# Part XVII — logical execution time

## Semantic effect time versus physical execution

MNCS should permit a task to declare logical temporal behavior independently of the exact physical instruction schedule.

Conceptually:

```text
LogicalEffectInstant
PhysicalExecutionInterval
```

are distinct.

## Example

A control cycle may specify:

```text
input logically sampled at t
output logically visible at t + 10ms
```

while a verified implementation physically executes computation anywhere inside an allowed interval.

## Benefits

This separation permits:

```text
rescheduling
parallelization
core migration
precomputation
task fusion
task splitting
```

where the logical temporal interface remains unchanged.

## Representation analogy

RFC 0019 separates logical value from physical layout.

RFC 0023 analogously separates logical temporal behavior from physical schedule.

---

# Part XVIII — temporal envelopes

## Prefer envelopes where punctuality is unnecessary

A program should not require exact punctual execution unless exact punctuality is semantically necessary.

Example:

```text
execute in [9.9ms, 10.1ms]
```

may be preferable to:

```text
execute exactly at 10ms
```

## Temporal slack

Slack is the admissible scheduling region between release, execution needs, and deadlines.

It should remain machine-visible because it is optimization freedom.

## Protected timing versus preference

A temporal contract may distinguish:

```text
hard bound
preferred phase
preferred latency
```

rather than forcing all timing hints into hard semantics.

---

# Part XIX — schedule-carrying realizations

## Schedule as realization

A schedule is a realization of temporal requirements, not the temporal requirements themselves.

Conceptually:

```text
Schedule {
    target
    entries
    communication_windows
    assumptions
}
```

## Schedule witness

An implementation may carry:

```text
ScheduleWitness {
    temporal_contract
    target_identity
    schedule
    schedulability_evidence
    dependency_set
}
```

## Synthesis outside the trusted base

Schedule search may use:

```text
heuristics
SMT
MILP
constraint programming
AI search
evolutionary search
domain-specific algorithms
```

without making those search procedures trusted.

## Verification

A smaller checker validates that the submitted schedule satisfies the declared timing contract under explicit assumptions.

## Analogy to proof-carrying code

Schedule-carrying execution follows the same architecture as proof-carrying transformations:

```text
untrusted search
-> checkable witness
-> narrow verifier
-> assurance policy
```

---

# Part XX — temporal task sets

## Task contract

Conceptually:

```text
TemporalTaskContract {
    release_model
    period_or_interarrival
    deadline
    jitter
    dependencies
    miss_semantics
}
```

## Scheduler realizations

Candidate realizations may include:

```text
static table
rate-monotonic-like
 deadline-monotonic-like
EDF-like
partitioned scheduler
global scheduler
time-triggered schedule
custom generated schedule
```

These are implementation families, not language semantics.

## Schedulability relation

A schedule/scheduler establishes something like:

```text
SchedulableUnder<TaskSet, Target, Scheduler, Assumptions>
```

rather than simply setting:

```text
schedulable = true
```

---

# Part XXI — execution-time evidence

## Worst-case execution bounds

Schedulability commonly depends on execution-time bounds.

MNCS should represent them as evidence-bearing objects:

```text
WorstCaseExecutionBound {
    subject
    target
    bound
    assumptions
    compiler_identity
    pipeline_identity
    evidence
}
```

## Target specificity

A WCET result for one target/compiler/pipeline does not automatically apply to another.

## Staleness

Changes to:

```text
CPU
frequency policy
compiler
backend
optimization flags
memory subsystem
scheduler
runtime
```

may invalidate execution-time evidence.

## RFC 0032 boundary

RFC 0023 consumes quantitative execution bounds for temporal verification.

RFC 0032 owns the deeper quantitative cost/resource theory.

---

# Part XXII — physical and distributed resource windows

## Temporal resource windows

A schedule may constrain when resources may be used:

```text
CPU core window
network transmission window
accelerator window
bus window
shared-device window
```

## Distributed schedule scope

RFC 0023 may represent temporal communication windows and target identities.

RFC 0028 owns distributed failure, consistency, messaging, and consensus semantics.

## Composability

A component may export a temporal interface describing:

```text
required input window
produced output window
resource reservation
jitter tolerance
```

without exposing its internal schedule.

---

# Part XXIII — time-triggered and event-triggered execution

## Time-triggered

Example:

```text
release every 10ms
```

## Event-triggered

Example:

```text
release when packet arrives
```

## Hybrid

Example:

```text
on packet arrival:
    respond within 2ms
```

MNCS should support all three models.

---

# Part XXIV — synchronous/reactive logical ticks

## Logical ticks

Some systems are best described by logical ticks rather than physical timestamps.

Conceptually:

```text
LogicalTick<ClockDomain>
```

## Tick semantics

Operations may be logically simultaneous within one tick even if physical execution is sequential.

## Binding to physical time

A realization may later establish:

```text
1 logical tick = 10ms physical period
```

under a schedule/clock contract.

## Deterministic reactive semantics

Logical ticks can support deterministic control/dataflow behavior independent of instruction interleaving.

---

# Part XXV — multiple clock domains

## Multiple clocks are normal

A system may contain:

```text
audio clock
video clock
sensor clock
network synchronized clock
CPU monotonic clock
logical control clock
simulation clock
```

There is no ambient universal `now`.

## Clock handles/capabilities

Clock observation should identify the requested domain explicitly.

Conceptually:

```text
read(clock_capability)
```

## Cross-domain operations

Comparison or conversion requires a declared `ClockRelation` or `ClockMapping`.

---

# Part XXVI — clock mapping

## Mapping object

Conceptually:

```text
ClockMapping {
    source_clock
    destination_clock

    offset_model
    rate_model
    uncertainty

    validity_interval
    evidence
}
```

## Dynamic mappings

Clock mappings may change over time.

## Stale mappings

A previously valid mapping cannot be assumed indefinitely.

## Composition

Mappings may compose only when uncertainty and validity semantics are preserved correctly.

---

# Part XXVII — clock reads as effects

## Clock observation capability

RFC 0008 should eventually expose something conceptually similar to:

```text
ClockReadCapability<ClockDomain>
```

## Purity boundary

A pure computation cannot silently observe ambient physical time.

## Deterministic replay

Explicit clock effects allow replay to substitute a replay/simulation clock without modifying ordinary pure semantics.

## Timer effects

Timers, sleeps, alarms, and deadlines may also require explicit temporal/runtime capabilities.

---

# Part XXVIII — simulation time

## Simulation clock

A simulation clock advances according to a simulation/runtime controller rather than physical wall time.

## Temporal equivalence

A simulation may preserve logical temporal relations while running faster, slower, paused, or stepwise relative to physical time.

## Example

A simulated `1 second` wait may advance simulation time immediately without consuming one wall-clock second.

## Use cases

```text
tests
model checking
RAVEL experiments
Fabric experiments
digital twins
control-system simulation
fault injection
```

---

# Part XXIX — replay time

## Recorded temporal trace

A replay trace may preserve:

```text
event order
logical timestamps
relative delays
clock observations
```

## Replay speed

Replay may execute:

```text
real-time
accelerated
slowed
paused
single-step
```

without changing recorded logical temporal relationships.

## Replay clock

A replay clock is a distinct clock domain.

It must not be mistaken for current physical time.

---

# Part XXX — virtual time

## Virtual-time domains

Virtual time may represent structured progress such as:

```text
epoch
iteration
nested loop coordinate
dataflow timestamp
transaction phase
```

It need not be scalar or physically measured.

## Partial order

A virtual-time domain may preserve a partial order rather than total ordering.

## Use in graph/dataflow execution

Fabric-like execution can use virtual time to reason about which work may still affect which future computations.

---

# Part XXXI — event time, ingestion time, and processing time

## Event time

When the represented real/logical event is considered to have occurred.

## Ingestion time

When the runtime receives the event.

## Processing time

When a computation processes the event.

## Observation time

When a particular observer recorded/verified it.

## Example

```text
event_time      = 10:00:00
ingestion_time  = 10:00:05
processing_time = 10:00:07
```

These are separate temporal facts.

## Out-of-order data

Processing order must not silently rewrite event-time order.

---

# Part XXXII — temporal frontiers and watermarks

## Frontier as knowledge claim

A temporal frontier expresses what earlier event times are still considered possible under a model.

Conceptually:

```text
TemporalFrontier {
    temporal_domain
    frontier
    model
    evidence
}
```

## Not a clock reading

A frontier/watermark is an epistemic claim about temporal completeness, not a measurement of physical time.

## Uncertainty

A frontier may itself be uncertain or assumption-scoped.

## Retractions/corrections

Systems that permit late data may revise previous results instead of treating a frontier as irreversible truth.

---

# Part XXXIII — late data policies

## Late relative to what?

An event may be late relative to:

```text
event-time window
processing-time policy
deadline
frontier assumption
external contract
```

These are distinct.

## Candidate policies

```text
discard
revise result
retract result
compensate
route to late-data path
accept with degraded status
```

RFC 0011 owns failure/recovery behavior after the temporal classification is established.

---

# Part XXXIV — temporal relation claims

## RFC 0020 integration

Temporal relationships should be explicit RFC 0020 relation claims.

Conceptually:

```text
TemporalRelationClaim {
    left_subject
    right_subject

    relation_kind
    temporal_domain
    observation_scope

    clock_model
    uncertainty_model
    assumptions
    evidence
}
```

## Candidate relations

```text
Before
After
Within<Duration>
BeforeWithin<Duration>
Overlaps
SameLogicalTick
TemporallyEquivalent
DeadlineRefines
```

## Counterexamples are relation-scoped

A timing mismatch may refute temporal equivalence without refuting functional equivalence.

---

# Part XXXV — temporal equivalence and refinement

## Functional versus temporal equivalence

Two implementations may return the same values while producing them at different logical times.

Therefore:

```text
FunctionalEquivalent(A,B)
```

does not imply:

```text
TemporallyEquivalent(A,B)
```

## Deadline refinement

Two implementations may satisfy the same deadline while not matching exact timing traces.

## Jitter refinement

A candidate may reduce jitter and thereby refine a weaker temporal envelope without being timing-identical.

## Observation scope

Temporal equivalence must state whether observations include:

```text
exact timestamps
deadline satisfaction
ordering
jitter
latency distribution
clock readings
```

---

# Part XXXVI — schedule validity and assurance

## Assurance-gated timing claims

A schedule witness is not automatically authorized for deployment.

RFC 0018 may require:

```text
current target identity
current scheduler identity
current WCET evidence
trusted schedule checker
fresh clock evidence
independent witness
```

## Experimental versus production profiles

A sandbox profile may permit bounded empirical timing evidence.

A stronger profile may require independently checked schedulability and current WCET/clock evidence.

## Conflict preservation

If one verifier says the schedule is valid and another provides a counterexample, the relation becomes conflicted under RFC 0018 rather than being flattened into one score.

---

# Part XXXVII — temporal evidence invalidation

## Environment changes

Temporal evidence may become stale when any relevant dependency changes:

```text
hardware
clock source
clock synchronization
frequency policy
scheduler
runtime
compiler
backend
optimization pipeline
memory topology
network topology
workload model
```

## Local invalidation

Only dependent temporal claims should be invalidated where the dependency graph permits.

## Reverification

Forge may regenerate or recheck schedule/clock evidence after invalidation.

---

# Part XXXVIII — temporal diagnostics

Diagnostics should distinguish causes such as:

```text
CLOCK_DOMAIN_MISMATCH
CLOCK_MAPPING_MISSING
CLOCK_MAPPING_STALE
CLOCK_UNCERTAINTY_TOO_LARGE
TEMPORAL_ORDER_UNRESOLVED
INVALID_DURATION_OPERATION
DEADLINE_MISSED
DEADLINE_UNRESOLVED
TIMEOUT_FIRED
PERIOD_DEADLINE_CONFLICT
JITTER_BOUND_VIOLATED
TIMED_INVARIANT_VIOLATED
ZENO_BEHAVIOR_POSSIBLE
SCHEDULE_UNSATISFIABLE
SCHEDULE_WITNESS_INVALID
WCET_EVIDENCE_STALE
CLOCK_READ_UNAUTHORIZED
TEMPORAL_VALIDITY_EXPIRED
TEMPORAL_VALIDITY_UNRESOLVED
FRONTIER_ASSUMPTION_INVALID
LATE_EVENT_POLICY_REQUIRED
```

A diagnostic should preserve:

```text
subject
clock/temporal domain
required relation
observed relation
uncertainty
assumptions
evidence dependencies
counterexample trace
suggested obligation
```

---

# Part XXXIX — Forge temporal search

## Temporal realization holes

Programs may intentionally leave:

```text
schedule = ?
core_assignment = ?
clock_realization = ?
preemption_policy = ?
communication_window = ?
```

unresolved.

## Search dimensions

Forge may search:

```text
static schedule
priority assignment
EDF-like schedule
core placement
task fusion
task splitting
preemption points
accelerator placement
network windows
clock realization
synchronization strategy
```

## Protected semantics

Search may optimize only inside the declared temporal envelope.

## Candidate rejection

A candidate that is faster overall but violates a protected deadline/jitter/event-order requirement is invalid.

## Witness generation

Successful candidates should produce checkable temporal evidence rather than only benchmark results.

---

# Part XL — machine-native temporal optimization

## Temporal slack as freedom

Temporal slack should be visible to optimization.

Example:

```text
release = 0ms
deadline = 10ms
execution bound = 3ms
```

leaves a feasible scheduling region.

## Future multi-objective optimization

RFC 0033 may optimize:

```text
latency
energy
thermal load
network contention
throughput
reliability
```

inside the temporal feasibility region.

RFC 0023 defines the temporal constraints, not the objective optimizer.

---

# Part XLI — target/runtime interaction

## RFC 0017 facts

Runtime/target descriptions may expose:

```text
available clocks
clock resolution
clock synchronization
scheduler classes
core counts
interrupt model
frequency policy
```

## Temporal realization applicability

A schedule or clock realization must declare which runtime facts it consumes.

## Fallback

If a target cannot satisfy the required temporal contract, the realization should be rejected or an explicit fallback selected.

No silent degradation.

---

# Part XLII — FFI and external clock boundaries

## Foreign temporal semantics

Foreign APIs may expose timestamps, deadlines, or timers with weaker/ambiguous semantics.

RFC 0015 governs the trust boundary.

## Required adapter

An adapter should declare:

```text
source clock meaning
time scale
monotonicity
uncertainty
conversion policy
failure behavior
```

## No opaque timestamp laundering

A foreign integer timestamp must not silently become a trusted MNCS reference instant.

---

# Part XLIII — persistence and temporal durability boundary

RFC 0023 may represent timestamps and validity intervals attached to state.

RFC 0026 will own:

```text
durable transaction time
snapshot time
journal ordering
crash-consistent temporal state
```

RFC 0023 should avoid prematurely defining persistent-history semantics.

---

# Part XLIV — distributed-time boundary

RFC 0023 establishes:

```text
clock domains
clock mappings
uncertainty
hybrid/logical/physical time
communication timing windows
```

RFC 0028 will own:

```text
partial failure
message semantics
consistency
replication
consensus
failure detectors
distributed guarantees
```

Temporal information may be an input to those semantics, but does not replace them.

---

# Part XLV — lifecycle/live-upgrade boundary

RFC 0030 will own deployment/live-upgrade timing such as:

```text
activation windows
version coexistence
state migration windows
rollback deadlines
```

RFC 0023 supplies the temporal vocabulary.

---

# Part XLVI — quantitative-cost boundary

RFC 0032 owns:

```text
asymptotic complexity
resource accounting
WCET methodology
energy cost
performance models
```

RFC 0023 consumes evidenced timing bounds where needed for deadlines/schedules.

---

# Part XLVII — temporal authority

## Observation authority

Reading a physical clock requires observation authority appropriate to RFC 0008/0017.

## Schedule mutation authority

Changing runtime scheduling is different from merely observing temporal state.

## Deadline policy authority

Changing a protected deadline broadens semantics and should be a visible semantic delta.

## Clock-trust authority

A subsystem cannot arbitrarily declare an untrusted clock globally authoritative.

RFC 0018 trust policy governs which principals/verifiers may establish temporal claims.

---

# Part XLVIII — temporal deltas

Forge should be able to compare baseline/candidate temporal semantics.

Conceptually:

```text
TemporalDelta {
    changed_clock_requirements
    changed_deadlines
    changed_periods
    changed_jitter
    changed_validity
    changed_schedule
    changed_assumptions
    changed_temporal_relations
    changed_evidence
}
```

A delta that broadens timing freedom or weakens protected deadlines should be explicit.

---

# Part XLIX — examples

## Example A — monotonic versus civil time

```text
measure elapsed operation duration
```

requires a monotonic clock.

Substituting a civil clock that can adjust backward is invalid unless the contract explicitly permits that behavior.

## Example B — uncertain physical ordering

```text
A in [10.000,10.008]
B in [10.005,10.012]
```

Strict physical order remains unresolved.

## Example C — deadline versus timeout

```text
final deadline = 500ms
primary attempt timeout = 300ms
```

A timeout can trigger fallback while the actual deadline remains satisfiable.

## Example D — period versus deadline

```text
period = 10ms
deadline = 8ms
```

The task repeats every 10ms but each job must finish in 8ms.

## Example E — logical execution time

Two physical schedules execute a task at different moments but both expose the result at the same declared logical effect time.

They may be temporally equivalent under the logical-observation model.

## Example F — simulation time

A controller simulation advances 60 simulated seconds in 200ms wall time while preserving its logical 10ms control period.

## Example G — stale schedule evidence

A schedule verified for one CPU/frequency policy becomes inapplicable after a frequency-management change.

## Example H — uncertain expiration

Current synchronized time estimate overlaps an evidence expiration boundary.

Temporal applicability is `UNRESOLVED`, not arbitrarily fresh/stale.

---

# Part L — first implementation pilot

The first executable experiment should remain deliberately bounded.

## Required artifact subset

Introduce versioned experimental artifacts for:

```text
ClockDomain
ClockObservation
ClockQuality
ClockMapping
Instant
Duration
TemporalInterval
Deadline
TimeoutPolicy
Period
JitterConstraint
TimedConstraint
ScheduleWitness
TemporalRelationClaim
```

Exact names may change.

## Fixture 1 — monotonic versus civil clock

Demonstrate that a monotonic duration measurement cannot be transparently replaced by civil-time arithmetic.

## Fixture 2 — definite ordering under uncertainty

```text
A=[10.000,10.005]
B=[10.010,10.015]
```

establishes definite ordering.

## Fixture 3 — unresolved ordering

```text
A=[10.000,10.008]
B=[10.005,10.012]
```

must produce `UNRESOLVED` for strict order.

## Fixture 4 — deadline versus timeout

Use a primary attempt that times out before a larger final deadline and verify that the timeout event is not itself a deadline violation.

## Fixture 5 — periodic task

Model:

```text
period = 10ms
deadline = 8ms
```

as two independent fields/constraints.

## Fixture 6 — timed state machine

Model:

```text
request -> response within 50ms
```

with one satisfying trace and one violating trace.

## Fixture 7 — schedule witness

Create a small two- or three-task schedule with:

```text
release
execution bound
deadline
```

and produce one valid schedule plus one invalid schedule with a localized counterexample.

## Fixture 8 — logical execution time

Show two different physical schedules that preserve the same logical effect times.

## Fixture 9 — simulation clock

Run the same temporal program against a simulation clock without wall-clock waiting.

## Fixture 10 — freshness near expiration

Use RFC 0018 evidence with a validity interval and a clock uncertainty interval overlapping expiration.

The result must be `UNRESOLVED` rather than forced fresh/stale.

## Assurance profiles

At least two RFC 0018 policies should exist:

```text
experimental_temporal_use
stronger_temporal_promotion
```

The stronger profile should require current target/schedule/clock dependencies and refuse stale or unresolved timing evidence.

---

# Part LI — Forge scheduling pilot

A useful bounded Forge experiment is:

```text
Task A:
    release 0ms
    execution_bound 2ms
    deadline 4ms

Task B:
    release 2ms
    execution_bound 3ms
    deadline 9ms

Task C:
    periodic 10ms
    execution_bound 2ms
    deadline 8ms

Target:
    2 cores

schedule:
    ?
```

Forge generates candidates.

Candidate 1:

```text
misses deadline B
-> rejected
```

Candidate 2:

```text
meets modeled deadlines
but depends on stale execution-time evidence
-> unresolved
```

Candidate 3:

```text
meets all deadlines
uses current target facts
schedule witness independently checks
-> eligible for RFC 0018 authorization
```

This experiment should demonstrate the full machine-native pattern:

```text
semantic timing requirements
-> unresolved realization hole
-> untrusted schedule search
-> checkable witness
-> assurance decision
```

---

## Interaction with RFC 0008

RFC 0008 owns clock/timer observation effects and capabilities.

RFC 0023 defines the temporal meaning of the values/constraints those effects expose.

---

## Interaction with RFC 0010

RFC 0010 owns causality, concurrency, atomicity, and memory consistency.

RFC 0023 must never infer causality merely from physical timestamp order.

---

## Interaction with RFC 0011

RFC 0011 owns timeout consequences, late-result failure/recovery, and nondeterministic behavior.

RFC 0023 classifies temporal conditions such as timeout firing, deadline miss, or late arrival.

---

## Interaction with RFC 0012

RFC 0012 owns executable state-transition semantics.

RFC 0023 adds temporal constraints/annotations/relations over transitions where required.

---

## Interaction with RFC 0017

RFC 0017 owns runtime and target facts such as available clocks, scheduler environment, and hardware identity.

RFC 0023 consumes those facts for temporal realization applicability.

---

## Interaction with RFC 0018

RFC 0018 owns whether clock, schedule, synchronization, and WCET evidence authorizes a particular action.

RFC 0023 supplies temporal claim types and dependencies.

---

## Interaction with RFC 0020

RFC 0020 owns temporal equivalence/refinement relation structure.

RFC 0023 defines the temporal observations and constraints those relations refer to.

---

## Interaction with RFC 0022

RFC 0022 owns termination, productivity, progress, liveness, fairness dependencies, and finite semantic computation bounds.

RFC 0023 owns physical/logical clocks, durations, deadlines, timed liveness bounds, temporal validity, and real-time realizations.

In particular:

```text
1000 semantic steps
```

must not silently become:

```text
1 millisecond
```

without an explicit evidenced relation.

---

## Interaction with RFC 0026

RFC 0026 will own persistent temporal state such as transaction/snapshot/journal timing where durability matters.

---

## Interaction with RFC 0028

RFC 0028 will own full distributed time-dependent failure and consistency semantics.

RFC 0023 supplies clock/uncertainty/temporal-window vocabulary.

---

## Interaction with RFC 0030

RFC 0030 will own deployment/live-upgrade temporal lifecycle semantics.

RFC 0023 supplies generic temporal constraints and validity intervals.

---

## Interaction with RFC 0032

RFC 0032 will own quantitative resource/cost/WCET theory.

RFC 0023 consumes evidenced execution bounds for temporal constraints.

---

## Interaction with RFC 0033

RFC 0033 will own multi-objective realization search over latency, energy, throughput, cost, and risk.

RFC 0023 defines temporal feasibility constraints and slack.

---

## Security consequences

Time is frequently part of security logic.

Examples include:

```text
credential expiration
nonce validity
replay windows
lease expiry
rate limits
time-based access policy
certificate validity
```

RFC 0023 requires such logic to identify its clock/time domain and uncertainty assumptions rather than relying on ambient wall-clock folklore.

Detailed information-flow/cryptographic semantics belong to later RFCs.

---

## Reproducibility consequences

Temporal behavior can affect reproducibility.

A replayable computation should distinguish:

```text
replay of logical event time
replay of recorded physical clock observations
new execution under current wall time
```

Reproducible temporal tests should use explicit simulation/replay clocks where possible.

---

## Self-transformation consequences

A self-transforming system may modify scheduling, deadlines, clock requirements, or timing implementation.

Such changes are temporal semantic deltas and must pass the same relation/evidence/assurance rules as other transformations.

An optimizer cannot claim a behavior-preserving transformation while silently weakening temporal guarantees that are part of the observation model.

---

## Machine-native advantages

RFC 0023 enables a program to express:

```text
what must happen when
```

without prematurely expressing:

```text
how hardware spends every microsecond making it happen
```

This gives Forge freedom to search:

```text
schedules
core assignments
preemption strategies
clock realizations
communication windows
accelerator placement
```

while preserving explicit temporal meaning.

This is the temporal counterpart to RFC 0019 representation freedom and RFC 0021 numerical realization freedom.

---

## Rejected simplifications

### One universal `Timestamp`

Rejected because clock identity, time scale, uncertainty, and interpretation matter.

### Physical-time order as causality

Rejected because distributed causality is not equivalent to timestamp sorting.

### Civil time for elapsed measurements

Rejected because civil/reference clocks may adjust and have conversion semantics unsuitable for monotonic duration.

### `deadline == timeout`

Rejected because a timeout is a reaction policy while a deadline is a requirement.

### `period == deadline`

Rejected because recurring release cadence and per-job completion bounds differ.

### `realtime = true`

Rejected because miss semantics, period, deadline, jitter, clock quality, and scheduling assumptions are multidimensional.

### Schedules as source semantics

Rejected as the default because schedule should remain a realization where the temporal contract permits flexibility.

### Schedulability by benchmark only

Rejected because finite empirical timing does not establish universal real-time guarantees.

### Ambient `now()` in pure code

Rejected because clock observation is an effect.

### Watermark as time truth

Rejected because a temporal frontier is an epistemic completeness claim, not a clock reading.

---

## Open research questions

The Draft intentionally leaves open:

- exact canonical clock-domain representation;
- how much physical-time uncertainty should be interval-based versus richer statistical models;
- how clock mappings compose under asymmetric uncertainty;
- which civil/reference time scales become built-in versus library-defined;
- whether timezone/civil conversion belongs in the core or standard library;
- exact temporal-interval algebra exposed canonically;
- exact timed-state-machine representation;
- which metric temporal-logic fragment is practical for the core toolchain;
- how logical execution time should interact with effects and failure;
- schedule-witness serialization/checking format;
- how scheduler proofs compose across modules/components;
- how WCET evidence is imported without expanding the proof kernel;
- how multiple logical clock domains compose in one component;
- how replay/simulation clocks interact with external I/O;
- how virtual-time frontiers integrate with Fabric graph scheduling;
- how uncertainty at deadline boundaries should interact with application policy;
- when temporal properties should be proof-kernel checkable versus verifier/certificate based;
- which temporal claims require trusted hardware time sources;
- how timing side-channel observations interact with RFC 0024; and
- how distributed clock semantics refine into RFC 0028 consistency models.

---

## Research checklist

RFC 0023 is not complete merely because timestamps can be parsed.

Research should demonstrate:

- explicit clock-domain identity;
- inability to compare unrelated clock domains without a mapping;
- monotonic/civil distinction;
- uncertainty-bearing physical observations;
- definite and unresolved temporal ordering;
- instant/duration type separation;
- interval validity and boundary semantics;
- deadline versus timeout distinction;
- period versus deadline distinction;
- at least one timed state machine;
- at least one metric temporal property;
- one non-Zeno/time-divergence check or reserved fixture;
- logical effect time separated from physical execution time;
- a checkable schedule witness;
- target-sensitive timing evidence invalidation;
- simulation/replay clock substitution;
- event-time versus processing-time distinction;
- temporal frontier represented as evidence rather than time truth;
- RFC 0020 temporal relation claims;
- RFC 0018 assurance gating; and
- diagnostics that preserve clock/evidence/counterexample scope.

---

## Initial acceptance direction

A future move from **Draft** toward **Proposed** should require at least:

1. a versioned clock/instant/duration/interval artifact model;
2. explicit clock-domain comparison rules;
3. uncertainty-bearing physical clock observations;
4. a temporal applicability/freshness model integrated with RFC 0018;
5. explicit deadline, timeout, period, and jitter semantics;
6. a bounded timed-state-machine representation;
7. one independently checkable schedule witness;
8. one logical-execution-time fixture with multiple physical schedules;
9. one simulation/replay clock realization;
10. RFC 0020 temporal relation integration;
11. target/environment dependency invalidation; and
12. accepted, rejected, and unresolved temporal fixtures.

---

## Final direction

MNCS should treat temporal meaning as a first-class semantic contract while refusing to collapse causal order, physical clocks, logical time, civil time, deadlines, validity, event time, schedules, and execution budgets into one primitive.

The machine-native principle is:

> **Programs specify temporal meaning and acceptable temporal envelopes; machines search for and verify clocks, schedules, and execution realizations that satisfy them.**

The program should say **what must happen when**.

The realization should remain free to decide **how the machine spends time making that true**, subject to explicit clocks, uncertainty, target facts, temporal relations, evidence, and assurance.