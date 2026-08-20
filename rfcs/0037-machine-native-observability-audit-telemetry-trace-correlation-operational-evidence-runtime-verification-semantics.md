# RFC 0037 — Machine-Native Observability, Audit, Telemetry, Trace Correlation, Operational Evidence, and Runtime-Verification Semantics

- **Status:** Draft
- **Track:** Foundational semantics
- **Depends on:** RFC 0002, RFC 0004, RFC 0008, RFC 0010, RFC 0011, RFC 0017, RFC 0018, RFC 0020, RFC 0023, RFC 0024, RFC 0025, RFC 0026, RFC 0027, RFC 0028, RFC 0029, RFC 0030, RFC 0031, RFC 0032, RFC 0033, RFC 0034, RFC 0036
- **Primary ownership:** observation identity and completeness; telemetry-signal semantics; instrumentation; correlation; operational provenance; audit semantics; runtime monitoring; diagnosability; sampling; retention; operational evidence

## Abstract

MNCS needs a semantic account of what a running system can reveal about itself.

Conventional observability stacks often collapse several distinct ideas into one operational surface:

- an event occurred;
- a probe observed the event;
- a telemetry record was produced;
- a collector received the record;
- a trace correlated the record with others;
- a metric aggregated a population of records;
- an audit system retained an accountable record;
- a monitor evaluated a temporal property;
- a diagnosis engine inferred a cause;
- an operator or agent acted on the resulting conclusion.

Those are not the same thing.

This RFC establishes a machine-native operational-evidence layer in which:

1. an event is distinct from an observation of that event;
2. an observation is a scoped projection of execution, not execution ground truth;
3. missing telemetry is not negative evidence unless completeness is established;
4. traces express explicit correlation and causal relations rather than a timestamp-sorted fiction;
5. sampling, loss, retention, redaction, aggregation, normalization, and inference remain visible in observation provenance;
6. audit integrity is distinct from audit truth and audit completeness;
7. runtime-verification verdicts are scoped to monitorable properties and finite/incomplete observation prefixes;
8. detection, localization, diagnosis, attribution, and causal explanation remain separate claim classes;
9. instrumentation is itself an effect with authority, confidentiality, perturbation, and quantitative cost;
10. observation plans may be synthesized and adapted by Forge, but their conclusions remain independently checkable evidence under RFC 0018.

The constitutional statement is:

> **Observability is not the volume of telemetry a system emits; it is the evidenced ability of an authorized observer to distinguish operationally relevant states and histories from the information the system makes available.**

A stronger machine-native formulation is:

> **MNCS treats runtime observation itself as a verifiable program. Machines may decide what to instrument, correlate signals, derive diagnoses, adjust sampling, and request new evidence dynamically, but every operational conclusion must retain the exact observer, observation model, completeness, causality, sampling, provenance, integrity, and uncertainty assumptions that made the conclusion possible.**

---

## 1. Motivation

MNCS already has semantics for:

- effects and authority;
- concurrency and causality;
- failures and recovery;
- time and uncertainty;
- information flow;
- principals and authorization;
- persistence and crash consistency;
- distributed operation identity;
- placement and migration;
- lifecycle transitions;
- provenance;
- quantitative metrics;
- experiment semantics;
- language evolution.

A running MNCS system still needs to answer operational questions such as:

```text
What happened?
What did we actually observe?
What may have happened in an unobserved region?
Which events are causally related?
Which events are only correlated?
Which records are sampled, aggregated, redacted, inferred, or incomplete?
Which authority-sensitive actions are auditable?
Can fault F be distinguished from fault G under the active observation plan?
Did runtime property P become violated?
Is a monitor verdict conclusive yet?
Which additional observation would most reduce diagnostic uncertainty?
What operational evidence justified an automated rollback or promotion?
```

A traditional answer such as:

```text
check the logs
```

is insufficient for a verification-native system.

MNCS needs to know what those logs mean, which execution they relate to, whether they are complete, what observer created them, what transformations they passed through, and what conclusions can be validly derived from them.

---

## 2. Constitutional rules

The following rules are normative design constraints for RFC 0037.

### 2.1 Event and observation are distinct

```text
ExecutionEvent != Observation
```

An event may occur without being observed.

Several observers may observe one event.

One observation may summarize multiple events.

### 2.2 Observation is not system state

An observation is produced by an observer under an observation model.

It is a projection of execution.

Two distinct execution histories may be indistinguishable under one observer.

### 2.3 Not observed does not mean did not happen

The inference:

```text
NoObservation(E) => NoEvent(E)
```

is invalid unless an explicit completeness claim authorizes it.

### 2.4 Correlation is not causality

Sharing a trace identifier, request identifier, job identifier, or temporal window does not establish causal influence.

### 2.5 Timestamp order is not causality

RFC 0010 causal order and RFC 0023 clock observations remain distinct.

### 2.6 Receipt order is not occurrence order

Collector ingestion order is not execution order.

### 2.7 Telemetry record is not audit fact

Persisting or signing a record does not make its semantic claim true.

### 2.8 Audit integrity is not audit truth

Cryptographic integrity can establish origin or tamper evidence while the underlying statement remains false or incomplete.

### 2.9 Audit inclusion is not audit completeness

A proof that event `E` is included in a committed audit log does not prove that every event that should have been recorded is present.

### 2.10 Sampling is not event absence

An unsampled event may still have occurred.

### 2.11 Aggregation is lossy unless proven otherwise for a query

A metric is not an event history.

### 2.12 Inference is not direct observation

An anomaly detector, causal-reconstruction heuristic, or diagnosis model produces an inference artifact, not a direct event observation.

### 2.13 Instrumentation may perturb execution

Instrumentation effects are observable resource/effect facts and may invalidate equivalence assumptions between instrumented and uninstrumented runs.

### 2.14 Observation authority differs from execution authority

A principal authorized to perform an action is not automatically authorized to observe every argument, intermediate value, secret, or cross-tenant consequence of that action.

### 2.15 Runtime monitor is not enforcement authority

A monitor may detect or diagnose without being authorized to terminate, revoke, rollback, quarantine, or deploy.

### 2.16 Runtime observation remains evidence

RFC 0037 produces operational evidence.

RFC 0018 determines the assurance and action policy applicable to that evidence.

---

## 3. Non-goals

RFC 0037 does not define:

- the meaning of effects or capabilities — RFC 0008;
- happens-before or memory consistency — RFC 0010;
- failure semantics — RFC 0011;
- general evidence algebra — RFC 0018;
- equality or observational equivalence — RFC 0020;
- clock or event-time semantics — RFC 0023;
- information-flow authorization — RFC 0024;
- principal or credential semantics — RFC 0025;
- persistence durability — RFC 0026;
- telemetry encoding or schema evolution — RFC 0027;
- distributed operation/message/attempt semantics — RFC 0028;
- placement identity — RFC 0029;
- lifecycle or rollout semantics — RFC 0030;
- build provenance — RFC 0031;
- metric quantity meaning — RFC 0032;
- optimization/selection policy — RFC 0033;
- controlled experiment methodology — RFC 0034;
- language-profile evolution — RFC 0036.

This RFC consumes those meanings and establishes the semantics of operational observation over them.

---

## 4. Core conceptual model

RFC 0037 models operational evidence through the following chain:

```text
ExecutionHistory H
       |
       | observed by
       v
Observer O + ObservationModel M
       |
       v
RawObservations
       |
       +--> normalization
       +--> redaction
       +--> correlation
       +--> aggregation
       +--> runtime monitoring
       +--> diagnosis / inference
       v
DerivedOperationalArtifacts
       |
       v
RFC 0018 evidence graph
       |
       v
authorized operational action, if policy permits
```

Every transformation retains provenance.

No downstream artifact loses the distinction between:

- directly observed;
- derived;
- inferred;
- sampled;
- incomplete;
- redacted;
- aggregated;
- integrity-protected;
- independently corroborated.

---

## 5. Observation as projection

Let an execution history be `H` and an observation model be `O`.

Conceptually:

```text
O(H) = Ω
```

where `Ω` is the observation stream visible to that observer.

Two histories may satisfy:

```text
H1 != H2
O(H1) = O(H2)
```

Therefore they are observationally indistinguishable under `O`.

RFC 0037 does not define a new universal equality.

It instantiates RFC 0020 relations such as:

```text
ObservationallyIndistinguishableUnder<O>(H1, H2)
```

This relation is observer-relative.

---

## 6. Observability

The control-theoretic intuition of observability is retained:

> can the hidden state relevant to a question be distinguished from the available output observations?

MNCS generalizes this to arbitrary operational properties.

```text
ObservabilityClaim {
    subject
    property_or_state
    observer_model
    instrumentation_model
    observation_channel
    domain
    assumptions
    status
}
```

Possible dispositions include:

```text
OBSERVABLE
NOT_OBSERVABLE
PARTIALLY_OBSERVABLE
UNKNOWN
```

`OBSERVABLE` is never unqualified.

It always names the property/state family and observer model.

---

## 7. Diagnosability

Observability and diagnosability are distinct.

Diagnosability asks whether a fault can be distinguished from competing histories after sufficient observation.

```text
DiagnosabilityClaim {
    subject
    fault_class
    competing_faults
    observer_model
    observation_plan
    horizon_or_conditions
    status
}
```

Possible results:

```text
DIAGNOSABLE
NOT_DIAGNOSABLE
CONDITIONALLY_DIAGNOSABLE
UNKNOWN
```

A system may be capable of detecting abnormal behavior without being capable of distinguishing which modeled fault occurred.

---

## 8. Detection, localization, diagnosis, attribution, explanation

RFC 0037 distinguishes the following claim classes:

```text
DetectionClaim
LocalizationClaim
ClassificationClaim
AttributionClaim
CausalExplanationClaim
```

Example:

```text
latency anomaly detected
    !=
subsystem localized
    !=
storage stall diagnosed
    !=
storage stall attributed as cause
    !=
complete causal explanation of user failure
```

Each stronger claim requires its own evidence.

---

## 9. Observation identity

A minimal observation artifact is:

```text
Observation {
    observation_id

    subject_id
    observer_id

    signal_kind
    semantic_observation_point?

    value_or_payload

    event_time?
    observation_time
    ingestion_time?

    target_context
    runtime_context

    observation_model
    completeness_class

    provenance
}
```

An observation may reference an execution event identity if known.

It does not require that identity to exist for all signal types.

---

## 10. Identity distinctions

The following identities must not collapse:

```text
LogicalOperationIdentity
ExecutionAttemptIdentity
ExecutionEventIdentity
ObservationIdentity
TelemetryRecordIdentity
TraceIdentity
SpanIdentity
MetricSampleIdentity
AuditRecordIdentity
DiagnosisIdentity
```

RFC 0028 owns logical operation and attempt identity.

RFC 0037 reuses them when operational artifacts observe those operations.

---

## 11. Observation completeness

Completeness is not one Boolean.

```text
ObservationCompleteness {
    domain
    observer
    interval_or_frontier
    signal_kind
    class
    loss_evidence
}
```

Initial classes:

```text
COMPLETE_UNDER_MODEL
PARTIAL
SAMPLED
KNOWN_LOSS
LOSS_POSSIBLE
UNKNOWN
```

A downstream claim must never silently upgrade a weaker completeness class.

---

## 12. Negative evidence

Negative operational evidence is permitted only when justified.

Example:

```text
No WriteEvent observed
```

supports:

```text
NoWriteOccurred
```

only when the observation model establishes completeness for all relevant `WriteEvent`s over the queried domain.

Without that:

```text
NoWriteOccurred = UNRESOLVED
```

This rule applies equally to:

- logs;
- traces;
- audit events;
- distributed messages;
- runtime property events;
- fault signals.

---

## 13. Telemetry signal kinds

RFC 0037 defines a common operational family while preserving signal kind.

```text
TelemetrySignal =
    EventRecord
  | LogRecord
  | MetricObservation
  | Trace
  | Span
  | ProfileObservation
  | AuditObservation
  | RuntimeMonitorObservation
```

These may share semantic conventions.

They do not become interchangeable.

---

## 14. Event records

An event record refers to a discrete semantic or realization event.

```text
EventRecord {
    event_kind
    event_identity?
    subject
    attributes
    temporal_context
    causal_context?
    observation_provenance
}
```

Where possible, event kinds should correspond to semantic events rather than human message strings.

Example:

```text
TransactionCommitted {
    transaction_id
    commit_stage
}
```

is preferable to:

```text
"transaction committed"
```

---

## 15. Log records

A log record is a structured operational assertion.

```text
LogRecord {
    logger
    subject
    severity_or_class?
    structured_fields
    human_rendering?
    provenance
}
```

The human rendering is not the primary semantic object.

A log assertion may be false, stale, incomplete, or produced by defective instrumentation.

Therefore:

```text
LogRecord("user authenticated")
```

does not itself establish RFC 0025 authentication truth.

---

## 16. Metric observations

Metrics are aggregated quantitative observations.

```text
MetricObservation {
    metric_identity
    quantity
    aggregation
    population_scope
    window
    sampling_model
    aggregation_provenance
}
```

Metric quantity semantics come from RFC 0032.

RFC 0037 owns the fact that the quantity was operationally observed and how the observation was produced.

---

## 17. Aggregation

Aggregation is modeled explicitly:

```text
Aggregate<A>(ObservationSet) -> MetricObservation
```

The aggregation artifact records:

```text
AggregationProvenance {
    source_set_or_population
    aggregation_function
    window
    sampling
    approximation?
    information_loss
}
```

An aggregate may support some queries and make others impossible.

---

## 18. Metric exemplars

RFC 0037 supports explicit bridges from aggregates to concrete observations.

```text
ObservationExemplar {
    aggregate_observation
    concrete_observation
    relation
}
```

An exemplar does not imply that the concrete observation explains the whole aggregate population.

---

## 19. Profile observations

A profile is a sampled view of execution state.

```text
ProfileObservation {
    profile_kind
    sample_policy
    sample_population
    stack_or_state_samples
    temporal_window
    provenance
}
```

A profile is not an exact trace.

A frequency in a profile is not an event count unless the profile semantics explicitly establish that relation.

---

## 20. Traces

A trace is a correlated graph of operational observations.

```text
Trace {
    trace_id
    spans
    causal_links
    correlation_links
    completeness
    sampling
    provenance
}
```

A trace is not defined as a timestamp-sorted list.

---

## 21. Spans

```text
Span {
    span_id
    trace_id?
    operation_identity?
    attempt_identity?

    start_observation?
    end_observation?

    temporal_interval
    observer
    attributes

    parent_relation?
    links

    sampling
    completeness
}
```

A span may be partial.

Missing end data does not imply operation failure unless a separate rule establishes it.

---

## 22. Correlation relations

RFC 0037 distinguishes:

```text
TraceCorrelated(A, B)
SpanParent(A, B)
SpanLinked(A, B)
CausallyPrecedes(A, B)
SharesLogicalOperation(A, B)
SharesAttempt(A, B)
SharesResource(A, B)
CorrelationHypothesis(A, B)
```

These relations are not interchangeable.

---

## 23. Trace identifiers

A trace identifier establishes correlation context.

It does not establish:

- authentication;
- authority;
- causality among all members;
- completeness;
- trustworthy upstream provenance.

Correlation-context origin should therefore be represented:

```text
CorrelationContextOrigin =
    LOCAL_GENERATED
  | TRUSTED_UPSTREAM
  | UNTRUSTED_UPSTREAM
  | RESTARTED_AT_BOUNDARY
  | UNKNOWN
```

---

## 24. Trace propagation

```text
CorrelationPropagation {
    source_context
    boundary
    destination_context
    propagation_policy
    result
}
```

Results may include:

```text
PROPAGATED
RESTARTED
DROPPED
REDACTED
BROKEN
UNKNOWN
```

Broken propagation is itself operational evidence.

---

## 25. Causality before clocks

Where causal facts are available, RFC 0010 happens-before/causal relations take precedence over naive timestamp sorting.

Example:

```text
A sends message M
B receives M
```

establishes a causal edge even when clock skew yields:

```text
timestamp(B) < timestamp(A)
```

RFC 0023 still supplies physical-time constraints and uncertainty.

---

## 26. Time roles in operational evidence

RFC 0037 uses explicit temporal roles:

```text
EventTime
ObservationTime
IngestionTime
StorageCommitTime
QueryTime
```

No generic `timestamp` is sufficient when these roles matter.

---

## 27. Out-of-order observation

Operational streams may contain:

```text
late
out-of-order
duplicate
missing
```

records.

RFC 0037 uses RFC 0023 frontier/watermark semantics where appropriate.

A runtime monitor or query must not assume receive order equals event order.

---

## 28. Observation frontiers

```text
ObservationFrontier {
    stream_or_partition
    event_time_bound
    completeness_assumption
    provenance
}
```

A frontier may support claims such as:

```text
No event earlier than T is expected under model M
```

It does not create universal certainty outside that model.

---

## 29. Instrumentation

Instrumentation is modeled as an operational effect.

```text
InstrumentationPlan {
    observation_goals
    semantic_observation_points
    realized_probes
    sampling
    export
    retention
    authority
    information_flow_policy
    expected_cost
}
```

Instrumentation is not free and not semantically invisible by default.

---

## 30. Semantic observation points

A semantic observation point identifies the meaning to be observed independently of realization details.

```text
SemanticObservationPoint {
    identity
    subject_semantics
    event_or_state_relation
}
```

Examples:

```text
CapabilityGranted
TransactionDurabilityEstablished
LifecycleTransitionEntered
MessageDelivered
RecoveryCompleted
```

---

## 31. Realized probe points

A semantic observation point may be realized through:

```text
structured runtime event
compiler-inserted hook
kernel tracepoint
eBPF probe
hardware counter
sampling profiler
external observer
```

The realized location is not semantic identity.

```text
ProbeBindingWitness {
    semantic_observation_point
    realization_point
    target
    relation
    evidence
}
```

---

## 32. Instrumentation effect

```text
InstrumentationEffect {
    probe_or_plan
    effect_set
    resource_cost
    timing_perturbation?
    scheduling_perturbation?
    information_exposure
}
```

Effect semantics are owned by RFC 0008.

Quantitative cost is owned by RFC 0032.

RFC 0037 records the observation-specific relationship.

---

## 33. Instrumented vs uninstrumented execution

The relation:

```text
InstrumentedExecution ≡ UninstrumentedExecution
```

must not be assumed.

Where required, a relation-specific preservation claim must be established.

For example:

```text
FunctionalBehaviorPreserved
```

may hold while:

```text
TimingPreserved
```

fails.

---

## 34. Observation authority

Observation requires authority when it exposes protected state.

```text
ObservationAuthority {
    principal
    observation_scope
    permitted_fields
    permitted_subjects
    permitted_retention
}
```

RFC 0025 owns principal/authorization semantics.

RFC 0024 owns confidentiality/integrity policy.

RFC 0037 binds those policies to observation plans.

---

## 35. Execution authority != observation authority

The following inference is invalid:

```text
MayExecute(Action X)
    =>
MayObserve(AllInputsAndOutputsOf X)
```

A component may lawfully execute on secret data without being permitted to export that data into telemetry.

---

## 36. Telemetry information flow

Every telemetry attribute is subject to RFC 0024.

A trace context, log field, profile symbol, stack frame, metric label, audit record, or diagnosis may carry confidential information.

Telemetry must not be treated as a privileged bypass around information-flow controls.

---

## 37. Redaction

Redaction is an explicit derived-observation transformation.

```text
RedactionTransformation {
    input_observation
    policy
    removed_or_transformed_fields
    output_observation
    information_loss
}
```

Raw and redacted observations have distinct identities.

The output retains derivation provenance without revealing forbidden raw values to unauthorized observers.

---

## 38. Normalization

Normalization converts operational records into a common semantic form.

```text
Normalization {
    source_schema
    source_observation
    target_semantic_profile
    normalized_observation
    transformation_witness
}
```

Normalization may be lossless or lossy.

RFC 0027 owns encoding/schema compatibility.

RFC 0036 owns language/profile evolution when telemetry semantics are part of a language profile.

---

## 39. Operational provenance

RFC 0037 introduces observation-production provenance distinct from RFC 0031 build provenance.

Example:

```text
KernelEvent
   -> Probe
   -> RawObservation
   -> Collector
   -> NormalizedObservation
   -> Redaction
   -> ExportedObservation
   -> Aggregation
   -> Metric
   -> Diagnosis
```

Every arrow is explicit.

---

## 40. Observation derivation classes

Initial classes:

```text
DIRECT_OBSERVATION
NORMALIZED
REDACTED
AGGREGATED
RECONSTRUCTED
INFERRED
CORRELATED
```

A derived observation does not become direct merely because it is later persisted.

---

## 41. Inference artifacts

```text
OperationalInference {
    model_or_rule
    inputs
    output_claim
    uncertainty
    model_version
    provenance
}
```

Examples:

```text
FaultInference
AnomalyInference
CorrelationHypothesis
CriticalPathClaim
StateReconstructionClaim
```

These must remain distinct from directly observed events.

---

## 42. Anomaly detection

An anomaly detector may produce:

```text
AnomalyClaim {
    feature_vector
    baseline
    score
    threshold
    workload_context
    target_context
    detector_version
}
```

A high anomaly score is not itself a fault diagnosis.

RFC 0018 determines how much evidential weight the claim receives.

---

## 43. Baseline identity

An anomaly claim is baseline-relative.

```text
BaselineModel {
    identity
    population
    workload
    target
    time_validity
    feature_semantics
}
```

A stale or mismatched baseline can invalidate only the dependent anomaly claims.

---

## 44. Sampling

Sampling is explicit operational policy.

```text
SamplingPolicy {
    identity
    stage
    selection_rule
    probability_or_budget
    population_scope
    reason
}
```

Sampling decisions are artifacts:

```text
SamplingDecision {
    subject_or_record
    policy
    KEEP | DROP
    probability?
    reason
}
```

---

## 45. Head and tail sampling

RFC 0037 distinguishes:

```text
HeadSamplingPolicy
TailSamplingPolicy
AdaptiveSamplingPolicy
```

Head sampling chooses before complete trace information exists.

Tail sampling chooses after later trace evidence becomes available.

They create different selection distributions.

---

## 46. Sampling bias

A retained telemetry set must not be treated as a representative population unless the sampling model supports that inference.

Example:

```text
keep 100% failures
keep 1% successes
```

The stored trace failure ratio is not the production failure ratio.

RFC 0034/0032 may derive corrected estimates when sampling probabilities and assumptions permit.

---

## 47. Observation loss

Observation loss differs from intentional sampling.

```text
ObservationDrop {
    stage
    observation_or_count?
    reason
    known_scope
}
```

Reasons may include:

```text
BUFFER_FULL
RATE_LIMIT
NETWORK_FAILURE
EXPORT_FAILURE
SCHEMA_REJECTION
COLLECTOR_FAILURE
STORAGE_FAILURE
UNKNOWN
```

---

## 48. Known loss vs possible loss

RFC 0037 distinguishes:

```text
KNOWN_LOSS
POSSIBLE_LOSS
UNKNOWN_LOSS
```

A collector that reports `100 records dropped` provides different evidence from a pipeline that may silently drop records.

Both weaken completeness, but in different ways.

---

## 49. Telemetry-pipeline health

The telemetry pipeline is itself an operational subject.

```text
TelemetryPipeline {
    producer
    local_buffer
    exporter
    transport
    collector
    processors
    storage
}
```

Its health affects downstream evidence quality.

Telemetry about telemetry may be bounded by explicit meta-observation levels to avoid infinite self-observation recursion.

---

## 50. Observation meta-level

```text
ObservationMetaLevel {
    level
    subject_pipeline?
}
```

A system may define:

```text
level 0: application/runtime observation
level 1: telemetry-pipeline observation
level 2: limited supervisory observation
```

No requirement exists for infinite reflective instrumentation.

---

## 51. Retention

```text
TelemetryRetentionPolicy {
    signal_kind
    population
    raw_retention
    derived_retention
    aggregation_transition?
    deletion_policy
    authority
}
```

Retention affects query availability.

Retention expiry does not establish event nonexistence.

---

## 52. Collection completeness vs retention completeness

The following are distinct:

```text
CollectionCompleteness
RetentionCompleteness
QueryAvailability
```

A system may have collected an event completely and later deleted it under policy.

A query performed after deletion cannot treat the missing record as proof the event never occurred.

---

## 53. Retention conflicts

Retention policy may conflict with:

- privacy/erasure obligations;
- audit retention requirements;
- cost budgets;
- legal/compliance requirements;
- evidence preservation needs.

RFC 0037 surfaces such conflicts.

It does not silently privilege audit over privacy or vice versa.

---

## 54. Audit semantics

Audit is a specialized operational evidence class for accountable actions.

```text
AuditEvent {
    audit_event_id
    action
    subject
    principal
    authority_context
    target
    outcome
    logical_operation?
    execution_attempt?
    causal_context
    temporal_context
    observation_provenance
}
```

Audit events should refer to semantic action identities where possible.

---

## 55. Audit action phases

The following may be distinct audit events:

```text
AuthorizationGranted
ActionInvoked
ActionAccepted
ActionCommitted
ExternalEffectObserved
ActionCompensated
ActionRolledBack
```

A generic statement such as:

```text
"user deleted record"
```

may be too ambiguous for machine-native audit semantics.

---

## 56. Audit integrity

Audit integrity may include:

```text
record authentication
tamper evidence
append-only contract
checkpointing
inclusion proofs
consistency proofs
```

These properties do not establish semantic truth of the record's assertion.

---

## 57. Audit checkpoints

```text
AuditCheckpoint {
    log_identity
    position_or_tree_size
    commitment
    signer_or_witness?
    time
}
```

Potential witnesses:

```text
AuditInclusionWitness
AuditConsistencyWitness
```

Cryptographic semantics remain under RFC 0025.

Persistence semantics remain under RFC 0026.

---

## 58. Append-only, immutability, tamper evidence

These are distinct:

```text
PhysicalImmutability
AppendOnlySemanticContract
TamperResistance
TamperEvidence
```

A physically rewriteable store may still provide strong tamper evidence through commitments.

A physically immutable medium may still contain false or incomplete records.

---

## 59. Audit inclusion vs completeness

An inclusion witness proves a scoped statement:

```text
Record R is included in checkpoint C
```

It does not prove:

```text
all required audit records are included in C
```

Audit completeness requires its own observation/coverage model.

---

## 60. Audit coverage

```text
AuditCoverageClaim {
    accountable_event_class
    instrumentation_or_semantic_hook
    domain
    completeness
}
```

Example:

```text
Every CapabilityGrant transition emits AuditEvent A
```

may be structurally proven for a semantic realization.

Whether the collector retained every emitted event remains a separate operational claim.

---

## 61. Authentic audit record != true statement

A compromised or defective authorized component can emit an authentic but false audit assertion.

Therefore:

```text
AuthenticRecord
!=
TrueStatement
```

Independent cross-checks may refute the semantic content while record authenticity remains supported.

---

## 62. Runtime verification

Runtime verification evaluates formal properties over observed execution traces.

```text
RuntimeMonitor {
    monitor_id
    property
    observation_model
    monitorability
    state
    frontier
}
```

The monitor consumes operational observations.

---

## 63. Runtime property

```text
RuntimeProperty {
    semantic_property
    event_projection
    temporal_semantics
    domain
}
```

The property itself may originate from earlier RFC contracts.

RFC 0037 does not redefine those underlying semantics.

---

## 64. Monitor verdicts

Initial verdict family:

```text
VIOLATED
SATISFIED_ON_COMPLETE_DOMAIN
SATISFIED_ON_PREFIX
PENDING
INCONCLUSIVE
UNKNOWN
```

Profiles may refine these further.

No generic `PASS` is sufficient for all temporal properties.

---

## 65. Safety and liveness observation asymmetry

A safety violation may often be established by a finite bad prefix.

Example:

```text
Abort(T)
Commit(T)
```

may conclusively violate:

```text
NeverCommitAfterAbort
```

A liveness property such as:

```text
EveryRequestEventuallyResponds
```

may remain unresolved on a finite prefix.

No response observed *yet* is not necessarily violation or satisfaction.

---

## 66. Monitorability

```text
MonitorabilityClaim {
    property
    observer_model
    domain
    result
}
```

Possible results:

```text
MONITORABLE
PARTIALLY_MONITORABLE
NOT_MONITORABLE
UNKNOWN
```

A property may be formally meaningful while not operationally decidable from the active observer.

---

## 67. Incomplete and out-of-order monitor semantics

A runtime monitor must account for:

```text
sampling
known loss
possible loss
late events
out-of-order events
frontier uncertainty
```

Where completeness is insufficient, verdicts remain correspondingly weak.

---

## 68. Runtime counterexamples

When a property is violated, the monitor should produce an explicit counterexample slice where practical.

```text
RuntimeCounterexample {
    property
    observations
    causal_or_temporal_relation
    completeness_assumptions
}
```

A minimized or causal slice may be derived later under RFC 0004/0034 mechanisms.

---

## 69. Monitor authority

A monitor may be authorized only to:

```text
OBSERVE
DIAGNOSE
RECOMMEND
```

while another principal is required for:

```text
ENFORCE
REVOKE
ROLLBACK
QUARANTINE
PROMOTE
```

Monitor verdict does not self-authorize enforcement.

---

## 70. Monitor-triggered action chain

When an operational observation causes an action, the evidence chain should remain reconstructible:

```text
Observations
    -> MonitorVerdict
    -> PolicyEvaluation
    -> Authorization
    -> OperationalAction
    -> AuditEvent
```

This is a primary machine-native audit goal.

---

## 71. Fault hypotheses

```text
FaultHypothesis {
    fault_class
    subject
    expected_observation_signature
    competing_hypotheses
    evidence
}
```

A hypothesis may be:

```text
SUPPORTED
REFUTED
CONFLICTED
UNRESOLVED
```

using RFC 0018 dispositions.

---

## 72. Diagnosis

```text
Diagnosis {
    detected_condition
    candidate_faults
    discriminating_evidence
    unresolved_alternatives
    attribution_scope
}
```

A diagnosis should retain alternatives that could not be distinguished.

No forced single-cause result is required.

---

## 73. Causal explanation

Causal explanation is intentionally stronger than diagnosis.

A system may diagnose:

```text
DatabaseStall
```

without proving that it caused:

```text
UserRequestFailure
```

Causal explanation requires the relevant RFC 0010/RFC 0020 relation and appropriate evidence.

---

## 74. Correlation hypotheses

When explicit context is missing, a heuristic may infer correlation using:

```text
request identifiers
timestamps
payload hashes
network tuples
resource identity
```

The result is:

```text
CorrelationHypothesis
```

not a definitive trace edge.

---

## 75. Critical-path inference

A distributed trace may support a derived critical-path claim.

```text
CriticalPathClaim {
    trace
    spans_or_events
    causal_constraints
    temporal_constraints
    uncertainty
}
```

Critical path is a derived operational relation, not raw trace structure.

RFC 0032 owns span/work/latency quantities.

---

## 76. Complex operational patterns

RFC 0037 permits higher-level stream patterns such as:

```text
A then B without C within Δt
```

represented by:

```text
OperationalPattern
PatternMatch
```

Pattern matching does not by itself establish causal explanation.

---

## 77. Approximate operational observations

At scale, RFC 0037 permits approximate structures:

```text
histograms
sketches
Bloom filters
HyperLogLog
heavy-hitter summaries
```

They produce:

```text
ApproximateObservation {
    structure
    error_model
    scope
    provenance
}
```

Approximation does not automatically mean weak evidence.

The error model is explicit.

---

## 78. Observation cost

Observation consumes resources.

```text
ObservationCost {
    plan_or_probe
    CPU
    memory
    network
    storage
    energy
    latency_perturbation
}
```

RFC 0032 owns metric semantics and feasibility.

RFC 0037 binds cost to observation choices.

---

## 79. Attribute cardinality

High-cardinality fields can cause explosive telemetry cost.

```text
AttributeCardinalityClaim {
    attribute
    population
    estimate_or_bound
}
```

This may feed RFC 0032 resource accounting and RFC 0033 observation-plan selection.

---

## 80. Observation plans

Observation is itself programmable.

```text
ObservationPlan {
    operational_questions
    required_properties
    candidate_observation_points
    selected_probes
    sampling
    aggregation
    retention
    authority
    information_flow_constraints
    expected_diagnostic_power
    expected_cost
}
```

This is the machine-native replacement for a global `debug=true` flag.

---

## 81. Adaptive observation

An observation plan may change during execution.

Example:

```text
normal mode:
    low-cost metrics

anomaly detected:
    enable detailed spans

hypotheses narrowed:
    enable storage probe

fault localized:
    disable expensive probes
```

Represented by:

```text
AdaptiveObservationPolicy
ObservationPlanTransition
```

---

## 82. Observation plan transition

```text
ObservationPlanTransition {
    old_plan
    new_plan
    trigger
    authority
    expected_cost_delta
    expected_information_gain
}
```

Activating instrumentation may itself require lifecycle or authority checks.

---

## 83. Value of information

RFC 0033 owns value-of-information decision semantics.

RFC 0037 exposes candidate information actions:

```text
EnableProbe
IncreaseSampling
RetainLonger
CollectRawInsteadOfAggregate
CorrelateAdditionalContext
RunOperationalQuery
```

Forge may ask which action most reduces diagnostic uncertainty subject to cost and policy.

---

## 84. Minimal diagnosable observation plan

One useful search problem is:

```text
Find observation plan P
such that modeled faults F are diagnosable
and cost(P) is admissible.
```

RFC 0037 defines diagnosability.

RFC 0032 defines cost.

RFC 0033 chooses among admissible plans.

Search remains outside the TCB.

---

## 85. Instrumentation synthesis

Forge may synthesize:

```text
InstrumentationPlanCandidate
```

from:

```text
semantic observation requirements
fault model
runtime target
security policy
cost budget
```

A smaller checker validates:

- allowed observation points;
- authority;
- information flow;
- claimed coverage/diagnosability;
- probe bindings;
- cost evidence references.

---

## 86. Observation contracts

MNCS may permit semantic requirements such as:

```text
Observe<CapabilityGrant>
Observe<LifecycleAuthoritativeChange>
Observe<ExternalSideEffect>
```

These are not commands to log a string.

They are obligations that the realized system preserve an observation capability satisfying a declared observer relation.

---

## 87. Observation requirement classes

Initial classes may include:

```text
BEST_EFFORT_DEBUG
OPERATIONAL
SLO_SUPPORT
SECURITY_AUDIT
COMPLIANCE
SEMANTIC_CONTRACT
```

Each may carry different:

- completeness;
- durability;
- integrity;
- retention;
- sampling;
- authorization requirements.

---

## 88. Observation coverage

```text
ObservationCoverage {
    semantic_domain
    observation_points
    instrumentable_set
    observable_set
}
```

Observation coverage is not execution coverage.

A path may be fully instrumentable but never executed.

---

## 89. Instrumentable, executed, observed

These claims are distinct:

```text
Instrumentable(X)
Executed(X)
Observed(X)
```

`Instrumentable(X)` means an observation mechanism exists.

`Executed(X)` comes from execution semantics/evidence.

`Observed(X)` requires an observation artifact.

---

## 90. Audit coverage vs observation coverage

Audit coverage is specialized:

```text
Every event in accountable class A
must produce audit event B
```

General observation coverage may be best-effort or sampled.

The two must not be conflated.

---

## 91. Telemetry semantic profiles

Telemetry fields and event kinds should be versioned.

```text
TelemetrySemanticProfile {
    profile_id
    event_kinds
    field_meanings
    correlation_semantics
    units
    lifecycle
}
```

RFC 0027 owns wire/schema evolution.

RFC 0036 owns version relations when telemetry semantics are part of language/spec evolution.

---

## 92. Historical telemetry semantics

Historic records retain the semantic profile under which they were produced.

A new collector may normalize old telemetry into a new profile.

The normalized record remains derived from the original and does not retroactively redefine it.

---

## 93. Mixed telemetry profiles

A distributed system may temporarily emit multiple telemetry semantic profiles.

```text
Service A -> Profile T1
Service B -> Profile T2
```

A normalization layer may bridge them if compatibility is established.

Profile coexistence is not schema erasure.

---

## 94. Operational query

```text
OperationalQuery {
    question
    subject
    observation_domain
    required_completeness
    temporal_scope
    observer
}
```

A query result should carry:

```text
OperationalQueryResult {
    answer
    evidence
    completeness
    uncertainty
    unresolved_regions
}
```

---

## 95. Unknown regions

An operational explanation should be allowed to say:

```text
known causal subgraph
known correlation subgraph
missing region
possible alternatives
```

A perfectly total timeline is not required.

Fabric and Forge should prefer explicit uncertainty over fabricated continuity.

---

## 96. Reconstructed state

State reconstruction from telemetry is an inference:

```text
StateReconstructionClaim {
    target_state
    observations
    model
    ambiguity
}
```

A reconstructed state is not direct state observation unless the observer actually captured that state.

---

## 97. Duplicate delivery

Two identical telemetry payloads may represent:

```text
duplicate delivery of one observation
```

or:

```text
two distinct identical observations
```

Deduplication therefore requires identity evidence.

Raw value equality is insufficient.

---

## 98. Retry tracing

RFC 0028 identities should remain explicit:

```text
LogicalOperation O
    Attempt A1 -> timeout
    Attempt A2 -> success
```

Operational summaries may state:

```text
O succeeded
```

while traces preserve the failed attempt.

No summary may erase the failed attempt from audit/diagnostic evidence.

---

## 99. Placement and migration continuity

When an operation migrates:

```text
Host A -> Host B
```

physical identities may change while logical operation identity remains stable.

Trace continuity must preserve that distinction.

No assumption is made that process ID, host ID, clock domain, or runtime instance remains constant.

---

## 100. Lifecycle observability

RFC 0030 lifecycle transitions may be observed and audited.

Examples:

```text
Eligible
Serving
Authoritative
Draining
Finalized
Retired
```

A telemetry label saying `serving=true` is not itself the semantic lifecycle state unless it is correctly bound to the lifecycle transition semantics.

---

## 101. Operational evidence graph

Operational evidence should compose as an RFC 0018 graph.

Example:

```text
ExecutionEvent
    -> DirectObservation
    -> NormalizedObservation
    -> TraceCorrelation
    -> RuntimeMonitorVerdict
    -> Diagnosis
    -> AuthorizedRollback
```

Each relation remains explicit.

---

## 102. Evidence invalidation

If one observation becomes stale, invalid, reclassified, or untrusted, only dependent operational claims should invalidate.

Example:

```text
clock synchronization evidence invalidated
```

may invalidate:

```text
physical-latency reconstruction
```

without invalidating:

```text
explicit message happens-before relation
```

---

## 103. Operational evidence and experiment evidence

RFC 0034 and RFC 0037 may use the same instrumentation mechanisms.

They differ in ownership:

```text
RFC 0034:
    deliberately designed experiment

RFC 0037:
    operational observation during execution
```

Production traffic is not automatically a controlled experiment.

---

## 104. Correlation != causal effect

An operational change followed by improved latency does not prove the change caused the improvement.

Causal-effect claims require explicit experiment/causal semantics.

RFC 0037 may report:

```text
ObservedCorrelation
```

without upgrading it to:

```text
CausalEffect
```

---

## 105. Operational evidence quality

RFC 0037 may characterize evidence quality through dimensions such as:

```text
completeness
sampling
integrity
observer authority
observer trust
provenance
freshness
schema/profile compatibility
known loss
independent corroboration
```

It does not collapse them into one universal `quality` scalar.

RFC 0018 decides assurance policy.

---

## 106. Operational counterexamples

```text
OperationalCounterexample {
    claim
    observations
    violated_relation
    scope
    completeness
}
```

One valid runtime observation may refute a universal operational claim when the relation permits it.

Passing observation does not establish universal truth.

---

## 107. Operational deltas

```text
ObservationDelta {
    old_observation_model
    new_observation_model
    added_points
    removed_points
    sampling_change
    retention_change
    authority_change
    information_flow_change
}
```

```text
TelemetryDelta {
    old_profile
    new_profile
    semantic_changes
    compatibility
}
```

These feed RFC 0036 evolution analysis where applicable.

---

## 108. Semantic planes

RFC 0037 defines eleven conceptual planes.

### 108.1 Observation plane

```text
Observer
ObservationPoint
Observation
ObservationModel
ObservationCompleteness
```

### 108.2 Telemetry plane

```text
EventRecord
LogRecord
MetricObservation
Trace
Span
ProfileObservation
```

### 108.3 Correlation plane

```text
CorrelationContext
SpanParent
SpanLink
CausalLink
CorrelationHypothesis
```

### 108.4 Instrumentation plane

```text
InstrumentationPlan
SemanticObservationPoint
ProbeBinding
InstrumentationEffect
ObservationDrop
```

### 108.5 Audit plane

```text
AuditEvent
AuditCheckpoint
AuditInclusionWitness
AuditConsistencyWitness
AuditCoverageClaim
```

### 108.6 Runtime-verification plane

```text
RuntimeProperty
RuntimeMonitor
MonitorState
MonitorVerdict
MonitorabilityClaim
```

### 108.7 Diagnosis plane

```text
FaultHypothesis
DetectionClaim
LocalizationClaim
DiagnosabilityClaim
Diagnosis
CausalExplanationClaim
```

### 108.8 Sampling/retention plane

```text
SamplingPolicy
SamplingDecision
RetentionPolicy
ObservationGap
ObservationFrontier
```

### 108.9 Provenance plane

```text
ObservationDerivation
Normalization
Redaction
Aggregation
OperationalInference
```

### 108.10 Query/explanation plane

```text
OperationalQuery
OperationalQueryResult
StateReconstructionClaim
CriticalPathClaim
```

### 108.11 Assurance plane

```text
OperationalEvidence
ObservabilityWitness
RuntimeCounterexample
AuditCounterexample
ObservationDelta
```

---

## 109. Core artifact vocabulary

The bounded core should reserve at least:

```text
Observer
ObservationPoint
Observation
ObservationModel
ObservationCompleteness

TelemetrySignal
EventRecord
LogRecord
MetricObservation
Trace
Span
ProfileObservation

CorrelationContext
SpanLink
CausalLink
CorrelationHypothesis

InstrumentationPlan
SemanticObservationPoint
ProbeBindingWitness
InstrumentationEffect
ObservationDrop

SamplingPolicy
SamplingDecision
ObservationGap
ObservationFrontier
TelemetryRetentionPolicy

AuditEvent
AuditCheckpoint
AuditInclusionWitness
AuditConsistencyWitness
AuditCoverageClaim

RuntimeProperty
RuntimeMonitor
MonitorVerdict
MonitorabilityClaim
RuntimeCounterexample

FaultHypothesis
DiagnosabilityClaim
Diagnosis

ObservationProvenance
OperationalInference
OperationalEvidence

ObservabilityWitness
OperationalCounterexample
ObservationDelta
TelemetryDelta
```

---

## 110. Status/disposition vocabulary

RFC 0037 avoids overloaded booleans.

Initial status families include:

### Observation completeness

```text
COMPLETE_UNDER_MODEL
PARTIAL
SAMPLED
KNOWN_LOSS
LOSS_POSSIBLE
UNKNOWN
```

### Monitor verdict

```text
VIOLATED
SATISFIED_ON_COMPLETE_DOMAIN
SATISFIED_ON_PREFIX
PENDING
INCONCLUSIVE
UNKNOWN
```

### Diagnosability

```text
DIAGNOSABLE
NOT_DIAGNOSABLE
CONDITIONALLY_DIAGNOSABLE
UNKNOWN
```

### Correlation certainty

```text
EXPLICIT
SUPPORTED
HYPOTHESIZED
REFUTED
UNKNOWN
```

These are not interchangeable.

---

## 111. Micro-verifiers

Forge should rely on narrow verifiers wherever possible.

Candidate verifier families:

```text
observation-identity verifier
completeness verifier
sampling-weight verifier
trace-causality verifier
correlation-context verifier
probe-binding verifier
observation-authority verifier
information-flow verifier
audit-inclusion verifier
audit-consistency verifier
audit-coverage verifier
runtime-monitor verifier
monitorability verifier
diagnosability verifier
redaction-provenance verifier
aggregation-provenance verifier
telemetry-profile compatibility verifier
```

Complex search may be untrusted.

Checking should be narrower.

---

## 112. Search/check separation

Potential untrusted producers include:

```text
LLM diagnosis
ML anomaly detector
trace reconstruction heuristic
causal discovery tool
instrumentation planner
sensor selection optimizer
sampling optimizer
correlation heuristic
```

They may propose:

```text
DiagnosisCandidate
CorrelationCandidate
InstrumentationPlanCandidate
ObservationPlanCandidate
```

A smaller checker validates the explicit claims that can be checked.

Uncheckable heuristic confidence remains heuristic evidence.

---

## 113. Theories and realization families

RFC 0037 draws selectively from the following theory families.

### 113.1 Control-theoretic observability

Useful idea:

```text
internal states can be distinguished from output observations
```

MNCS generalizes this to semantic state/property distinguishability.

### 113.2 Discrete-event-system diagnosability

Useful ideas:

- observable/unobservable event partitions;
- fault distinguishability;
- delayed diagnosis;
- dynamic observers;
- sensor selection.

### 113.3 Distributed tracing

Useful ideas:

- trace/span identities;
- context propagation;
- low-overhead ubiquitous instrumentation;
- sampled correlation across components.

### 113.4 Trace-context standards

Useful ideas:

- portable trace identifiers;
- parent identifiers;
- vendor/context propagation;
- boundary privacy concerns.

These are interoperability realizations, not MNCS causal truth.

### 113.5 Open telemetry semantic conventions

Useful ideas:

- shared signal semantics;
- traces, metrics, logs, events, profiles, resources;
- stable machine-readable field meanings.

MNCS remains semantically independent of one telemetry ecosystem.

### 113.6 Runtime verification

Useful ideas:

- monitors over execution traces;
- finite-prefix semantics;
- safety counterexamples;
- monitorable property classes.

### 113.7 Three-valued temporal monitoring

Useful ideas:

```text
true / false / unknown
```

for incomplete and delayed streams.

### 113.8 Event-time stream semantics

Useful ideas:

- event time;
- processing time;
- frontiers;
- watermarks;
- late data;
- out-of-order data.

### 113.9 Provenance graphs

Useful ideas:

- entity;
- activity;
- agent;
- derivation;
- attribution;
- generation.

### 113.10 Transparency logs

Useful ideas:

- Merkle commitments;
- append-only consistency;
- inclusion proofs;
- witnessed checkpoints.

These support audit integrity, not truth/completeness.

### 113.11 Sampling theory

Useful ideas:

- sampling probability;
- selection bias;
- weighted inference;
- stratified/adaptive sampling.

### 113.12 Dynamic instrumentation

Useful realization families:

- compiler hooks;
- tracepoints;
- eBPF;
- Kprobes;
- DTrace-style probes;
- hardware tracing;
- profilers.

Realization points must not define semantic observation identity.

### 113.13 Complex-event processing

Useful ideas:

- temporal stream patterns;
- online event correlation;
- windowed pattern recognition.

### 113.14 Streaming sketches

Useful ideas:

- bounded-memory telemetry;
- approximate cardinality;
- heavy hitters;
- histograms/sketches.

### 113.15 Probabilistic diagnosis and anomaly detection

Useful as untrusted inference mechanisms with explicit model/version/baseline provenance.

### 113.16 Adaptive observer synthesis

Useful ideas:

- minimal sensor sets;
- dynamic observation plans;
- fault distinguishability under cost constraints.

### 113.17 Value-of-information instrumentation

Useful idea:

```text
collect the observation most likely to change the decision
```

integrated with RFC 0033.

---

## 114. Explicitly rejected conflations

The following equivalences are rejected:

```text
Event == Observation
Observation == Truth
NoObservation == NoEvent
LogRecord == SemanticFact
TraceId == Causality
TimestampOrder == HappensBefore
SpanTree == FullCausalGraph
SamplingDrop == EventAbsence
Metric == EventHistory
Exemplar == PopulationExplanation
Profile == Trace
SignedAuditRecord == TrueAuditStatement
AuditInclusion == AuditCompleteness
AppendOnly == PhysicallyImmutable
AnomalyScore == Diagnosis
Diagnosis == CausalExplanation
InstrumentedBehavior == UninstrumentedBehavior
ObserverAuthority == ExecutionAuthority
MonitorVerdict == EnforcementAuthority
OperationalTraffic == ControlledExperiment
Correlation == CausalEffect
```

---

## 115. Bounded implementation pilot

The first implementation should deliberately remain narrow.

### 115.1 Required artifacts

Implement canonical representations for:

```text
Observation
ObservationCompleteness
SemanticObservationPoint
ProbeBindingWitness
EventRecord
MetricObservation
Trace
Span
CorrelationContext
CausalLink
SamplingPolicy
SamplingDecision
ObservationDrop
AuditEvent
AuditCheckpoint
RuntimeMonitor
MonitorVerdict
DiagnosabilityClaim
ObservationProvenance
OperationalEvidence
ObservationDelta
```

### 115.2 Required verifiers

Implement narrow checks for:

```text
observation identity
sampling disposition
trace link consistency
explicit causal-edge validity for bounded fixtures
probe binding
basic observation authority fixtures
audit checkpoint inclusion
runtime safety-property monitor
basic diagnosability table
provenance-chain integrity
```

### 115.3 No requirement yet

The first pilot does not require:

- production OpenTelemetry integration;
- eBPF deployment;
- global distributed tracing;
- ML diagnosis;
- cryptographic production audit log;
- full temporal-logic monitor synthesis.

Those are later realizations.

---

## 116. Required fixtures

### Fixture A — event != observation

One semantic event `E1` occurs.

Two collectors observe it.

Expected:

```text
ExecutionEvent = E1
Observation = O1
Observation = O2
```

The system must not create two execution events.

### Fixture B — unsampled != absent

Event `E2` occurs.

Sampling policy drops it.

Query:

```text
Did E2 occur?
```

Expected operational conclusion from retained telemetry alone:

```text
UNKNOWN
```

not `NO`.

### Fixture C — complete negative evidence

A bounded execution domain proves all `WriteEvent`s are instrumented and collection is complete.

No write observation exists.

Expected:

```text
NoWriteOccurred = SUPPORTED
```

under that exact completeness model.

### Fixture D — trace correlation != causality

Three spans share one trace identifier.

Only `A -> B` has an explicit causal edge.

Expected:

```text
TraceCorrelated(A,C) = true
CausallyPrecedes(A,C) = UNRESOLVED
```

### Fixture E — clock skew

Message propagation proves:

```text
A happens-before B
```

while physical timestamps show:

```text
t(B) < t(A)
```

Expected:

- causal edge retained;
- clock inconsistency/uncertainty represented;
- no timestamp-sorted causal inversion.

### Fixture F — retry identity

```text
LogicalOperation O
Attempt A1 -> timeout
Attempt A2 -> success
```

Expected:

- operation ultimately successful;
- failed attempt retained;
- trace/audit identities not collapsed.

### Fixture G — tail-sampling bias

Policy:

```text
keep all failures
keep 1% successes
```

Expected:

- stored-trace ratio not accepted as production failure rate;
- sampling metadata preserved;
- corrected estimate permitted only through explicit statistical inference.

### Fixture H — metric exemplar

A latency histogram points to slow trace `T`.

Expected:

```text
T is an exemplar of an observed slow execution
```

but not:

```text
T explains the entire p99 population
```

### Fixture I — instrumentation perturbation

A race fails without tracing and disappears under heavy tracing.

Expected:

```text
UninstrumentedExecution = FAIL
InstrumentedExecution = PASS
```

No behavioral equivalence inferred.

### Fixture J — known probe loss

Probe reports `n` missed events.

Expected downstream completeness:

```text
KNOWN_LOSS
```

and any completeness-dependent negative claim becomes invalid.

### Fixture K — runtime safety violation

Property:

```text
NeverCommitAfterAbort
```

Observed:

```text
Abort(T)
Commit(T)
```

Expected:

```text
MonitorVerdict = VIOLATED
```

with explicit counterexample observations.

### Fixture L — unresolved liveness

Property:

```text
EveryRequestEventuallyResponds
```

Prefix ends with outstanding request.

Expected:

```text
PENDING or INCONCLUSIVE
```

not PASS.

### Fixture M — out-of-order monitoring

Collector receives `Response` before late `Request`.

Event-time/causal context later establishes the request preceded the response.

Expected monitor semantics use the declared frontier, not ingestion order.

### Fixture N — audit inclusion != completeness

Event `E` has a valid inclusion witness.

A required event `F` was never submitted.

Expected:

```text
E inclusion = SUPPORTED
Audit completeness = REFUTED or UNSUPPORTED
```

according to coverage evidence.

### Fixture O — tamper detection

Modify a committed audit record.

Expected:

```text
AuditIntegrityCounterexample
```

### Fixture P — authentic false audit assertion

An authorized logger signs a false statement.

Expected:

```text
Record authenticity = SUPPORTED
Semantic statement truth = REFUTED
```

### Fixture Q — redaction provenance

Raw observation contains secret field.

Policy generates redacted observation.

Expected:

- unauthorized observer sees only redacted artifact;
- derivation identity survives;
- raw value is not leaked through provenance metadata.

### Fixture R — diagnosis insufficiency

One latency anomaly is consistent with two modeled faults.

Expected:

```text
FAULT_DETECTED
NOT_DIAGNOSABLE
```

### Fixture S — adaptive observation

Initial plan cannot distinguish `CacheSlow` from `DatabaseSlow`.

A second probe makes them distinguishable.

Expected:

- diagnosability delta recorded;
- observation-cost delta recorded;
- new plan requires explicit authorization.

### Fixture T — inference != observation

Anomaly model outputs:

```text
P(DatabaseFailure) = 0.92
```

Expected artifact:

```text
FaultInference
```

not `ObservedDatabaseFault`.

### Fixture U — profile != trace

A CPU profile reports function `F` in 40% of samples.

Expected:

- no claim that exactly 40% of requests called `F`;
- profile sampling semantics retained.

### Fixture V — retention expiry

Event was collected, then raw record expires.

Expected:

```text
CollectionCompleteness may remain supported historically
QueryAvailability = false
```

No `NoEvent` conclusion.

### Fixture W — mixed telemetry profile

Service A emits telemetry profile T1.

Service B emits T2.

Normalization maps both to T3.

Expected:

- original identities retained;
- normalized artifacts marked derived;
- no retroactive profile rewrite.

### Fixture X — monitor without enforcement authority

Monitor detects violation.

Expected:

- verdict created;
- automatic kill/rollback rejected unless separate authorization exists.

---

## 117. Forge study 1 — synthesize observability

Model faults:

```text
WorkerCrash
NetworkPartition
VerifierTimeout
StaleCapability
PersistentStoreStall
```

Candidate observation points:

```text
worker lifecycle
message send/receive
failure-detector suspicion
capability validation
transaction journal phase
queue latency
```

Forge searches for observation plans.

RFC 0037 checks diagnosability.

RFC 0032 quantifies overhead.

RFC 0024 rejects plans leaking protected information.

RFC 0025 checks observation authority.

RFC 0033 selects among admissible plans.

Required result:

at least two valid plans survive so the system demonstrates diagnosability is not itself preference.

---

## 118. Forge study 2 — causal reconstruction of a Fabric job

One Fabric job contains:

```text
Planner
   -> workers W1 W2 W3
   -> micro-verifiers
   -> aggregator
```

Inject:

```text
retry
worker restart
clock skew
out-of-order collector delivery
sampled-out span
known message telemetry loss
```

Forge reconstructs:

```text
known causal graph
known correlation graph
unknown regions
inferred hypotheses
operational evidence
```

Required rule:

Forge must not fabricate a complete total timeline.

---

## 119. Forge study 3 — runtime verification of persistence invariant

Use an RFC 0026 invariant such as:

```text
AcknowledgedCommit requires DurabilityEstablished
```

Monitor:

```text
Commit
Persist
Ack
Crash
Recovery
```

One execution satisfies the property.

Another produces:

```text
Ack
Crash
```

without the required durable state.

Required:

explicit runtime counterexample bound to the RFC 0026 semantic relation.

---

## 120. Forge study 4 — trace continuity across migration

Logical operation migrates:

```text
Host A -> Host B
```

Change:

```text
process identity
host identity
clock domain
runtime instance
```

Preserve:

```text
LogicalOperationIdentity
```

Required:

trace correlation continuity without physical-identity conflation.

---

## 121. Forge study 5 — telemetry profile migration

Historic telemetry T1 remains T1.

New profile T2 changes field conventions.

Normalization produces T2-compatible derived records.

Required:

```text
OriginalObservation != NormalizedObservation
```

with explicit provenance and compatibility relation.

---

## 122. Forge study 6 — audit autonomous promotion

Forge promotes candidate `C`.

Required chain:

```text
Candidate C
    -> RFC 0033 selection witness
    -> RFC 0025 authorization
    -> RFC 0030 promotion action
    -> RFC 0037 audit event
    -> audit checkpoint
```

Later reconstruction must answer:

- what was promoted;
- which evidence existed;
- which selection policy chose it;
- who/what authorized it;
- which lifecycle action occurred;
- whether the audit record is integrity-protected.

---

## 123. Forge study 7 — adaptive diagnosis

Start with cheap metrics.

Detect p99 anomaly.

Hypotheses:

```text
SchedulerPressure
NetworkCongestion
DatabaseStall
```

Forge computes value of information and enables targeted probes.

Additional evidence refutes two hypotheses.

Expensive probes are disabled after diagnosis.

Required outputs:

```text
ObservationPlanTransition
Diagnosis
ObservationCostDelta
AuthorityWitness
```

---

## 124. Forge study 8 — operational evidence disagreement

Two independent observers report conflicting outcomes for one semantic event.

Required:

- retain both observations;
- create conflict in RFC 0018 evidence graph;
- do not select one by collector priority unless policy/evidence supports it;
- enable targeted follow-up observation if value-of-information policy permits.

---

## 125. Forge study 9 — telemetry-pipeline failure

Operational system remains healthy while collector loses 20% of records.

Required:

- system-health and telemetry-health remain distinct;
- downstream completeness degrades;
- absence-based diagnoses are suppressed;
- telemetry pipeline's own evidence is retained.

---

## 126. Forge study 10 — observation budget

Define:

```text
CPU overhead <= 2%
network telemetry <= 5 Mbps
storage <= 100 GiB/day
```

Need to diagnose a specified fault set and audit authority changes.

Forge searches:

```text
probe placement
sampling
aggregation
retention
```

RFC 0037 decides whether plans satisfy observation/diagnosability obligations.

RFC 0032 decides quantitative feasibility.

RFC 0033 chooses among surviving plans.

---

## 127. 1.0 research threshold

RFC 0037 is not ready for the research 1.0 threshold until all of the following are demonstrated.

### 127.1 Identity

- event identity differs from observation identity;
- logical operation differs from attempt identity;
- telemetry record differs from semantic event identity;
- trace/span/metric/audit identities remain distinct.

### 127.2 Completeness

- unsampled observation cannot become event absence;
- known loss degrades completeness mechanically;
- one bounded complete domain permits a valid negative-evidence claim;
- retention expiry is not event nonexistence.

### 127.3 Correlation and causality

- trace correlation is distinct from causality;
- one clock-skew fixture preserves happens-before despite reversed physical timestamps;
- span parentage is not assumed to represent the full causal graph;
- correlation hypotheses remain weaker than explicit links.

### 127.4 Instrumentation

- semantic observation point differs from physical probe point;
- one probe-binding witness exists;
- one instrumented/uninstrumented perturbation case exists;
- observation authority is independently checked;
- protected telemetry fields obey RFC 0024.

### 127.5 Sampling

- head/tail sampling semantics are represented separately;
- biased retained traces cannot be naively interpreted as population rates;
- sampling drop differs from pipeline loss.

### 127.6 Audit

- audit integrity differs from audit truth;
- inclusion differs from completeness;
- one tamper-detection fixture exists;
- one authentic-but-false audit record fixture exists;
- audit coverage is represented separately from collector completeness.

### 127.7 Runtime verification

- one finite safety counterexample produces `VIOLATED`;
- one liveness prefix remains `PENDING` or `INCONCLUSIVE`;
- monitorability is explicit;
- out-of-order observation does not force receipt-order semantics;
- monitor verdict does not self-authorize enforcement.

### 127.8 Diagnosis

- detection and diagnosis are distinct;
- at least one fault set is not diagnosable under a weak observation plan;
- an added observation makes it diagnosable;
- anomaly inference remains distinct from direct observation;
- causal explanation is not fabricated from correlation alone.

### 127.9 Provenance

- raw, normalized, redacted, aggregated, and inferred observations retain distinct identities;
- an operational provenance chain can be traversed backward;
- invalidating one source observation invalidates only dependent derived claims.

### 127.10 Cross-RFC integration

An end-to-end study must demonstrate:

```text
runtime semantic events
  -> RFC 0037 observations
  -> causal/correlation graph
  -> runtime monitor or diagnosis
  -> RFC 0018 evidence appraisal
  -> RFC 0033 decision if more observation/action is needed
  -> RFC 0025 authorization
  -> RFC 0030 operational action
  -> RFC 0037 audit record
```

with no stage laundering one claim class into another.

### 127.11 Adaptive observability

At least one study must show:

```text
initial observer insufficient
  -> value-of-information request
  -> authorized observation-plan change
  -> improved diagnosability
  -> quantitative overhead accounted
  -> expensive observation later removed
```

---

## 128. Cross-RFC ownership boundaries

### RFC 0002 / RFC 0018

Own general evidence composition, contradiction, assurance, and action-relative authorization.

RFC 0037 produces operational evidence and evidence-quality facts.

### RFC 0004

Own recursive refinement and causal repair.

RFC 0037 supplies operational counterexamples, diagnoses, and evidence requests that may trigger refinement.

### RFC 0008

Own effect/resource/capability semantics.

RFC 0037 owns instrumentation and observation as applications of those effects/capabilities.

### RFC 0010

Own concurrency and causality.

RFC 0037 observes/reconstructs causal edges but does not redefine them.

### RFC 0011

Own failure semantics.

RFC 0037 detects/diagnoses failures from operational evidence.

### RFC 0017

Own runtime/target/environment identity.

RFC 0037 scopes observations to them.

### RFC 0020

Own equality/equivalence/refinement/observational relations.

RFC 0037 instantiates those relations for observation indistinguishability and correlation/diagnosis claims.

### RFC 0023

Own clock/time/frontier semantics.

RFC 0037 uses explicit event/observation/ingestion/query time roles.

### RFC 0024

Own confidentiality/integrity/information-flow policy.

RFC 0037 applies those policies to telemetry and observation channels.

### RFC 0025

Own principal identity, authentication, authorization, credential, attestation, and cryptographic trust.

RFC 0037 applies them to observers, auditors, monitor actions, and audit-integrity evidence.

### RFC 0026

Own durable storage semantics.

RFC 0037 owns the operational meaning of stored telemetry/audit records.

### RFC 0027

Own serialization/schema/wire evolution.

RFC 0037 owns telemetry semantics carried by those schemas.

### RFC 0028

Own distributed logical operations, attempts, messaging, partial failure, and consistency.

RFC 0037 correlates operational observations of those objects.

### RFC 0029

Own placement/topology/migration identity.

RFC 0037 observes placement and preserves operational correlation across movement.

### RFC 0030

Own deployment/lifecycle transitions and action semantics.

RFC 0037 observes/audits them and may provide evidence that triggers separately authorized lifecycle actions.

### RFC 0031

Own artifact/build/derivation provenance.

RFC 0037 owns observation-production provenance.

### RFC 0032

Own metric quantities, budgets, resource cost, performance, and energy.

RFC 0037 owns how those quantities are observed operationally.

### RFC 0033

Own preference, value-of-information, observation-plan selection, and decision policy.

RFC 0037 defines the candidate observations and their diagnostic meaning.

### RFC 0034

Own deliberate tests, fuzzing, experiments, benchmarks, and empirical methodology.

RFC 0037 owns operational observation during actual execution.

### RFC 0036

Own evolution/version relations for telemetry semantic profiles where they are part of the language/spec ecosystem.

RFC 0037 owns the operational interpretation of each profile.

---

## 129. Security and privacy considerations

Observability expands information exposure.

Therefore the safest operational posture is not necessarily maximum telemetry.

RFC 0037 requires explicit reasoning about:

- PII in logs;
- secret values in traces;
- credential/token leakage;
- stack/profile leakage;
- cross-tenant identifiers;
- trace-context trust boundaries;
- observer privilege;
- retention duration;
- audit data access;
- side channels created by instrumentation;
- diagnostic inference revealing protected state.

A plan that increases diagnosability while violating RFC 0024 is inadmissible.

---

## 130. Reliability considerations

The telemetry system can fail independently of the observed system.

Operational tooling must therefore represent:

```text
system healthy / telemetry unhealthy
system unhealthy / telemetry healthy
system unhealthy / telemetry unhealthy
```

rather than using telemetry availability as a proxy for system availability.

---

## 131. Performance considerations

Observability overhead can be substantial.

The RFC deliberately makes:

```text
probe count
sample rate
attribute cardinality
network volume
storage retention
aggregation cost
monitor cost
```

visible to RFC 0032.

No assurance policy should require unbounded observability by accident.

---

## 132. Reproducibility considerations

Operational observations may depend on:

- observer version;
- runtime version;
- telemetry semantic profile;
- instrumentation realization;
- sampling policy;
- collector version;
- normalization/redaction pipeline;
- clock synchronization;
- retention state.

Those identities should be available in provenance so an investigation can reconstruct what the observer actually meant at the time.

---

## 133. Language-evolution considerations

RFC 0036 applies when:

- semantic event names change;
- observation contracts change;
- default instrumentation changes;
- telemetry field meanings change;
- monitor property syntax changes;
- audit requirements change.

Historic telemetry retains historic meaning.

Migration/normalization produces derived artifacts rather than retroactive reinterpretation.

---

## 134. Why this is machine-native

A human-oriented observability system typically optimizes for dashboards and readable incident response.

MNCS instead needs observation artifacts that machines can reason about compositionally.

A machine-native operational answer can look like:

```text
Question:
    Why did Job J17 fail?

Observation model:
    O7

Completeness:
    worker lifecycle complete
    verifier audit complete under profile A
    message telemetry known 0.3% loss
    spans tail-sampled

Facts:
    worker W2 restarted
        DIRECT_OBSERVATION

    attempt A4 did not acknowledge
        DIRECT_OBSERVATION

    retry A5 followed restart
        CAUSAL_EDGE

    message M8 delivery
        UNKNOWN
        observation gap overlaps expected delivery window

    verifier V3 rejected candidate
        DIRECT_OBSERVATION
        audit inclusion verified

Inference:
    WorkerFailure
        SUPPORTED

    LostMessage
        UNRESOLVED

    DatabaseStall
        anomaly score 0.81
        not direct observation

Next evidence action:
    enable network-delivery probe on W2

Expected diagnostic value:
    high

Cost:
    +0.4% CPU
    +18 MiB/min telemetry

Authority:
    permitted for 20-minute diagnostic window
```

That is the intended level of semantic precision.

---

## 135. Design principle for Forge

Forge should not ask:

```text
What logs should I add?
```

It should ask:

```text
Which operational distinctions must become observable?
Which candidate observation points expose those distinctions?
Which plans are authorized?
Which plans preserve confidentiality?
Which plans meet cost budgets?
Which plans make the target faults diagnosable?
Which next observation has the highest value of information?
```

This reframes observability as verified observation design.

---

## 136. Design principle for Fabric

Fabric should propagate stable operational identities across:

```text
planner
worker
model call
verifier
retry
migration
aggregation
```

without flattening:

```text
logical job
logical operation
attempt
span
host
process
model invocation
```

into one generic trace identifier.

---

## 137. Design principle for autonomous operation

Autonomous agents should be able to act on operational evidence only through explicit chains:

```text
observation
 -> evidence appraisal
 -> decision policy
 -> authorization
 -> action
 -> audit
```

No model confidence should directly mutate the system.

---

## 138. Open research questions

The RFC intentionally leaves several questions open for experiments:

1. What is the smallest semantic observation vocabulary needed for the 1.0 core?
2. Which runtime properties should have built-in monitor compilation?
3. How much of diagnosability can be statically checked from semantic event models?
4. Can semantic observation-point lowering be proved through LLVM/native backends?
5. How should observation completeness compose across independently sampled distributed components?
6. Which cryptographic audit structure best fits low-cost MNCS nodes?
7. How should privacy-preserving correlation work across trust boundaries?
8. Which approximate telemetry structures provide the best evidence/cost tradeoff?
9. Can Forge synthesize minimal diagnosable observation plans reliably?
10. How should operational evidence transport across language/runtime upgrades be constrained?
11. Can observation-plan changes themselves be safely live-applied through RFC 0030?
12. How should diagnosis confidence interact with RFC 0018 assurance without creating a scalar-trust shortcut?
13. Which operational facts are stable enough to become part of MNCS semantic contracts?
14. How should multi-node observation completeness be represented under partitions?
15. How should audit completeness be checked when producers can fail before emission?

---

## 139. Summary of mandatory distinctions

The implementation must preserve all of the following distinctions:

```text
Event
Observation
TelemetryRecord
AuditRecord

Correlation
Causality

EventTime
ObservationTime
IngestionTime
QueryTime

Sampling
Loss
RetentionExpiry

Raw
Normalized
Redacted
Aggregated
Inferred

Detection
Localization
Diagnosis
Attribution
CausalExplanation

AuditIntegrity
AuditTruth
AuditCompleteness

MonitorVerdict
EnforcementAuthority

ObservationCapability
ObservationOccurrence
ExecutionOccurrence

OperationalEvidence
ControlledExperimentEvidence
```

---

## 140. Constitutional conclusion

RFC 0037 establishes operational observation as an explicit semantic and evidential layer.

It rejects the idea that logs, metrics, traces, or signed audit records are self-authenticating descriptions of reality.

It instead requires the system to carry:

```text
observer
observation model
signal kind
semantic observation point
causal/correlation relation
sampling
loss
retention
instrumentation effect
information-flow policy
provenance
integrity
monitor semantics
diagnosability
uncertainty
```

alongside operational conclusions.

The defining rule is:

> **Not observed does not mean did not happen; observed does not mean understood; correlated does not mean caused; recorded does not mean true.**

The machine-native objective is therefore not maximum telemetry.

It is **minimum sufficient, explicitly authorized, semantically meaningful observation that can support the operational distinctions the system actually needs to make.**
