# RFC 0028: Machine-Native Distribution, Messaging, Partial Failure, Consistency, Replication, Consensus, and Failure-Detector Semantics

- **Status:** Draft
- **Authors:** MNCS project
- **Target:** pre-specification semantic research
- **Depends on:** RFC 0002, RFC 0004, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0014, RFC 0017, RFC 0018, RFC 0020, RFC 0022, RFC 0023, RFC 0024, RFC 0025, RFC 0026, RFC 0027
- **Leaves primarily to later RFCs:** placement/topology/locality/mobility to RFC 0029; deployment/live-upgrade/state migration to RFC 0030; build/supply-chain derivation to RFC 0031; quantitative distributed cost to RFC 0032; realization selection to RFC 0033

## 1. Summary

MNCS must not model a distributed program as an ordinary local program whose function calls happen to cross a network.

Distribution changes what can be known, what can fail independently, how operations are ordered, how state can diverge, which outcomes are observable, which guarantees are possible under a given timing/failure model, and what kinds of coordination are required to preserve an application contract.

This RFC therefore establishes a machine-native distributed semantic layer in which participants, channels, messages, logical operations, delivery attempts, network assumptions, partial failure, suspicion, configurations, replicas, quorums, consistency relations, convergence, agreement, distributed snapshots, distributed transactions, and coordination requirements are explicit semantic objects.

The central idea is:

> A distributed program specifies allowable knowledge, histories, failures, replica divergence, convergence, consistency, progress, and authority. Machines may choose communication, replication, and coordination mechanisms only inside that declared distributed-semantic envelope.

The RFC deliberately does **not** select Paxos, Raft, PBFT, CRDTs, two-phase commit, leases, gossip, quorum replication, or any other specific algorithm as the language semantics. Those are realization families or witness-producing mechanisms.

Likewise, this RFC does not let terms such as `reliable`, `available`, `exactly once`, `consistent`, `leader`, `failed`, or `distributed transaction` remain informal Boolean labels. Each such claim must identify its semantic scope and assumptions.

## 2. Constitutional rules

The following rules are intended to be architectural constraints, not implementation suggestions.

1. **Remote is not local.** A remote operation is not a local function call with added latency.
2. **Send is not receive.** Sending a message does not establish that any recipient received it.
3. **Receive is not deliver.** Transport receipt does not imply application/runtime delivery.
4. **Deliver is not apply.** Delivering a request does not imply its effect was applied.
5. **Apply is not persist.** Applying an effect does not imply durable survival under RFC 0026.
6. **Persist is not acknowledge.** Durability and acknowledgement are distinct events unless a contract relates them.
7. **Acknowledgement is observer-relative evidence.** A missing acknowledgement does not prove absence of effect.
8. **Message duplication is not duplicate logical effect.** Retransmissions may represent one logical operation.
9. **Retry is not idempotency.** Retry safety may arise from idempotency, durable deduplication, transactional identity, or another evidenced mechanism.
10. **At-least-once delivery is not exactly-once effect.** Delivery and logical effect semantics remain separate.
11. **Silence is not failure.** Lack of communication does not establish participant failure.
12. **Timeout is not failure.** A timeout may feed a failure detector but is not itself a proof of crash.
13. **Suspicion is evidence, not truth.** Failure detectors produce scoped observations/claims under assumptions.
14. **Crash is not partition.** Participant failure and communication failure are distinct.
15. **Crash is not Byzantine failure.** Arbitrary/adversarial behavior is a stronger fault model.
16. **Network synchrony assumptions are explicit.** Synchronous, asynchronous, and partially synchronous models are not interchangeable.
17. **Consensus safety is not consensus liveness.** Agreement may remain protected while progress stops.
18. **Impossibility boundaries are part of semantics.** The system may not silently assume away FLP-style liveness limits.
19. **Leader election is not consensus.** Leadership, replicated-log agreement, and state-machine replication remain distinct.
20. **Replication is not consensus.** A value can be replicated without agreement on a single total command history.
21. **Replication factor is not fault tolerance.** Fault tolerance depends on the fault model, quorum structure, independence, network assumptions, and protocol.
22. **Quorum size is not a quorum proof.** Required intersection properties must be established explicitly.
23. **Replica existence is not freshness.** A replica may be reachable and stale.
24. **Convergence is not linearizability.** Replicas may converge eventually while violating stronger history relations.
25. **Convergence is not invariant preservation.** A deterministic merge can converge to an application-invalid state.
26. **Causal consistency is not sequential consistency.** Distinct consistency relations remain distinct.
27. **Sequential consistency is not linearizability.** Real-time precedence is additional structure.
28. **Session guarantees are not global consistency.** Client-local guarantees may hold over weaker global models.
29. **CAP is not a generic choose-two switch.** Any CAP-related claim must name the consistency, availability, and partition model.
30. **Availability is operation-relative.** A system may preserve some operations while refusing others during failures.
31. **Conflict resolution is program semantics.** LWW, multi-value, application merge, CRDT merge, and consensus resolution produce different abstract behavior.
32. **`last` requires an ordering model.** Last-writer-wins may not silently use physical wall time.
33. **Consensus is a semantic problem; algorithms are realizations.** Paxos/Raft/Viewstamped/PBFT-family mechanisms are not the ontology.
34. **CRDT is a realization family, not a synonym for eventual consistency.** Convergence laws and semantic operations must be explicit.
35. **Coordination is generated only where protected semantics require it.** Distribution alone does not justify coordination everywhere.
36. **Distributed snapshot is not simultaneous physical time.** A consistent cut need not correspond to one global wall-clock instant.
37. **Distributed snapshot is not an RFC 0026 durable snapshot.** Local durable-version snapshots and global consistent cuts are separate concepts.
38. **Local transaction atomicity does not extend across independent failure domains.** Cross-participant atomicity requires additional distributed semantics.
39. **Two-phase commit safety does not imply nonblocking liveness.** Blocking states must remain representable.
40. **Compensation is not rollback.** A compensated history may still expose that forward actions occurred.
41. **Unknown remote outcome is legitimate.** The caller may be unable to determine whether a remote effect occurred.
42. **Authentication does not imply honesty.** RFC 0025 can establish sender identity while Byzantine behavior remains possible.
43. **Configuration is part of claim scope.** Quorum, consensus, leadership, and replication claims are configuration/epoch scoped.
44. **Old leadership does not imply current authority.** Fencing/epoch authority must prevent stale leaders from mutating protected state.
45. **Placement is not distribution semantics.** Physical locality and topology choices remain primarily RFC 0029 concerns.
46. **Bounded distributed testing is bounded evidence.** Finite partition/failure simulation cannot be upgraded into universal correctness.
47. **A faster distributed realization may still be invalid.** Forge must reject candidates that violate protected consistency, availability, failure, coordination, or authority relations.

## 3. Why distribution requires a distinct semantic layer

A local call has a relatively compact outcome space: it either returns, fails, traps, diverges, or produces another locally modeled effect.

A remote operation can create asymmetric knowledge. For example:

1. participant A sends operation `O17` to B;
2. B receives and durably applies `O17`;
3. B sends `SUCCESS`;
4. the reply is lost;
5. A times out.

At the end of this history:

```text
B knows: O17 applied
A knows: request was sent, no response observed
A does not know: whether O17 applied
```

Any language model that maps A's timeout to `O17 failed` has fabricated knowledge.

Similarly, a network partition can leave multiple live participants unable to coordinate. A storage system can converge yet violate an application invariant. A consensus protocol can preserve safety while losing liveness. A quorum can be numerically large yet fail the required intersection theorem. A signed Byzantine message can be authentic and still malicious.

These are not implementation details. They change the observable program semantics.

## 4. Distributed semantic planes

RFC 0028 organizes the domain into nine interacting planes.

### 4.1 Distributed execution plane

Owns:

```text
Participant
DistributedEvent
LogicalOperationIdentity
InvocationAttemptIdentity
MessageIdentity
Request
Response
MessageLifecycle
```

### 4.2 Network plane

Owns:

```text
Channel
NetworkModel
DeliveryContract
OrderingContract
PartitionObservation
CommunicationFailureObservation
```

### 4.3 Failure plane

Owns distributed interpretations of:

```text
CrashFailure
OmissionFailure
PartitionFailure
ByzantineFailure
FailureDetector
SuspicionClaim
```

while generic failure taxonomy remains aligned with RFC 0011.

### 4.4 Membership/configuration plane

Owns:

```text
DistributedConfiguration
ConfigurationEpoch
GroupView
ViewChange
LeaderClaim
FencingEpoch
Lease
```

### 4.5 Consistency plane

Owns distributed history/observation relations such as:

```text
Linearizable
SequentiallyConsistent
CausallyConsistent
ReadYourWrites
MonotonicReads
MonotonicWrites
WritesFollowReads
EventuallyConvergent
```

through RFC 0020 relation machinery.

### 4.6 Replication plane

Owns:

```text
Replica
ReplicaVersion
ReplicationContract
QuorumSystem
ConflictResolutionPolicy
ConvergentReplica
MergeAlgebraWitness
```

### 4.7 Agreement plane

Owns:

```text
ConsensusInstance
ConsensusDecision
ReliableBroadcast
AtomicBroadcast
ReplicatedStateMachine
DistributedAtomicCommit
```

### 4.8 Global-state/workflow plane

Owns:

```text
DistributedSnapshot
StableGlobalProperty
DistributedTransaction
Saga
Compensation
GlobalOutcome
```

### 4.9 Assurance plane

Consumes all of the above through RFC 0018:

```text
fault-model evidence
network-model evidence
quorum-intersection evidence
consensus safety evidence
liveness assumptions
failure-detector evidence
replica-independence evidence
bounded fault-injection evidence
```

No plane is allowed to collapse into one universal `distributed_ok` status.

## 5. Participants and distributed identity

A `Participant` is a semantic actor capable of local computation and distributed communication.

```text
Participant {
    participant_id
    principal_ref?
    configuration_memberships
    local_state_ref?
    runtime_context_ref?
}
```

`ParticipantIdentity` is not necessarily the same as:

```text
machine identity
process identity
workload principal
network address
container identity
replica identity
```

Those relationships are expressed explicitly through RFC 0009, RFC 0017, and RFC 0025.

A participant may move to a different physical host under RFC 0029/0030 without necessarily changing its logical distributed identity.

Conversely, a replacement participant may occupy the same network address without inheriting semantic identity.

## 6. Distributed events

A distributed execution is described by local and communication events rather than one globally serialized machine trace.

Candidate event kinds include:

```text
LocalTransition
MessageCreated
MessageSent
TransportAccepted
MessageReceived
MessageDelivered
OperationApplied
OperationPersisted
AcknowledgementSent
AcknowledgementObserved
FailureSuspected
FailureSuspicionCleared
ConfigurationChanged
LeaderElected
ReplicaMerged
DecisionChosen
CompensationApplied
```

Every event may refer to RFC 0010 causal relations and RFC 0023 temporal observations without confusing the two.

A total-order serialization of all events is not automatically part of semantics. It is only introduced where a particular consistency/agreement relation requires it.

## 7. Message lifecycle semantics

A message has an identity distinct from the logical operation it may carry.

```text
Message {
    message_id
    logical_operation_id?
    attempt_id?
    sender
    recipient_or_group
    schema_ref
    payload_ref
    created_event
}
```

The lifecycle can contain:

```text
CREATED
SENT
TRANSPORT_ACCEPTED
RECEIVED
DELIVERED
APPLIED
PERSISTED
ACKNOWLEDGED
```

These are not globally linear phases in all protocols; they are semantic predicates/events that may be related by a realization-specific protocol.

For example, a receiver may acknowledge receipt before application, or application before persistence, depending on the declared contract.

An acknowledgement therefore requires an explicit meaning:

```text
AcknowledgementSemantics {
    acknowledges:
        Receipt
        Delivery
        Application
        Durability<FailureModel>
        Decision
        CustomPredicate
}
```

## 8. Logical operation identity versus attempts

A retry must not silently create a new logical operation.

```text
LogicalOperationIdentity != InvocationAttemptIdentity != MessageIdentity
```

Example:

```text
operation O17
  attempt A1 -> message M1
  attempt A2 -> message M2
  attempt A3 -> message M3
```

A receiver may use `O17` to deduplicate while preserving the distinct evidence/history of A1/A2/A3.

This model is required for meaningful exactly-once logical-effect claims.

## 9. Idempotency and deduplication

`Idempotent` means repeated application does not change the protected relation after the first relevant application:

```text
f(f(S)) ≈ f(S)
```

under an explicitly declared RFC 0020 relation.

`Deduplicated` means the realization recognizes repeated attempts corresponding to one logical operation and suppresses unwanted repeated effects.

Neither implies the other.

A non-idempotent increment operation can be retry-safe with durable deduplication.

An idempotent operation can still be unsafe if an acknowledgement incorrectly claims durability that has not been established.

## 10. Delivery semantics

Delivery properties should be described compositionally rather than through one `reliable` label.

Possible properties include:

```text
MayDrop
MayDuplicate
MayReorder
AtMostOnceDelivery
AtLeastOnceDelivery
FIFOBySender
CausalDelivery
TotalOrderDelivery
```

`AtMostOnceDelivery` does not necessarily establish eventual delivery.

`AtLeastOnceDelivery` does not establish exactly-once logical effect.

`TotalOrderDelivery` does not imply causal order unless the contract explicitly includes it or the total-order protocol preserves causality.

## 11. Exactly-once terminology

RFC 0028 reserves the following distinctions:

```text
ExactlyOnceDelivery
ExactlyOnceProcessing
ExactlyOnceLogicalEffect
ExactlyOnceDurableEffect
```

A bare `ExactlyOnce` claim is incomplete.

For many stateful services, the useful property will be:

```text
ExactlyOnceLogicalEffect {
    operation_identity
    state_scope
    retry_scope
    durability_requirement?
}
```

This may be realized through at-least-once message delivery plus durable deduplication.

The network need not provide exactly-once delivery.

## 12. Remote outcome knowledge

A remote operation result is observer-relative.

Potential caller-side dispositions include:

```text
NOT_SENT
SENT
RESPONSE_OBSERVED_SUCCESS
RESPONSE_OBSERVED_FAILURE
TIMED_OUT
CONNECTION_LOST
OUTCOME_UNKNOWN
```

Potential server-side dispositions include:

```text
NOT_SEEN
RECEIVED
REJECTED
APPLIED
DURABLE
ABORTED
```

No automatic equivalence exists between the two sets.

`OUTCOME_UNKNOWN` is a legitimate semantic conclusion, not a temporary implementation embarrassment.

## 13. Network models

Every distributed liveness or failure-detection claim that depends on message timing must identify a network/timing assumption.

RFC 0028 recognizes at least:

```text
SynchronousNetwork
AsynchronousNetwork
PartiallySynchronousNetwork
```

### 13.1 Synchronous model

Relevant processing and message-delay bounds are known and assumed to hold.

### 13.2 Asynchronous model

No finite message-delay or process-speed bound may be assumed for correctness/liveness.

### 13.3 Partial synchrony

Bounds may exist but be unknown, or may begin to hold only after an unknown stabilization event.

The exact model may be extended later, but no verifier may substitute one model for another silently.

## 14. FLP as an impossibility boundary

Under a fully asynchronous message-passing model with deterministic consensus and even one possible crash failure, universal consensus termination cannot be promised.

MNCS therefore permits:

```text
ConsensusSafety = SUPPORTED
ConsensusLiveness = UNRESOLVED
```

under one model, while a stronger partial-synchrony/failure-detector/randomization assumption may support liveness under another.

The language must not turn an implementation timeout, leader heartbeat, or optimistic scheduler assumption into an unstated synchrony theorem.

This is an example of an impossibility result becoming an obligation generator:

```text
requested:
    consensus liveness
under:
    asynchronous + deterministic + crash possibility

result:
    additional assumption/mechanism required
```

## 15. Distributed fault models

The following must be distinguished:

```text
CrashStop
CrashRecovery
Omission
SendOmission
ReceiveOmission
Partition
TimingFault
Byzantine
AuthenticatedByzantine
```

Generic failure/result vocabulary remains aligned with RFC 0011.

A realization claiming correctness for `CrashStop` does not inherit correctness for `Byzantine`.

A participant may be correct while communication paths around it fail.

A partition may be asymmetric.

## 16. Failure observations and truth

A participant cannot generally distinguish all causes of silence.

Therefore:

```text
FailureObservation != FailureFact
```

Potential observations include:

```text
NoHeartbeatWithin<Duration>
ConnectionReset
TransportError
NoQuorumResponse
ExplicitShutdownNotice
AttestedRestartEpochChanged
```

These may feed a failure detector.

## 17. Failure detectors

A failure detector is modeled as an evidence-producing distributed mechanism:

```text
FailureDetector {
    detector_id
    subject_set
    failure_model
    network_model
    completeness_model
    accuracy_model
    assumptions
}
```

A detector can produce:

```text
Suspected(P)
SuspicionCleared(P)
```

The semantics must not reclassify suspicion as objective failure truth.

Different completeness/accuracy profiles may support different liveness algorithms.

## 18. Timeout versus suspicion versus failure

The required chain is conceptually:

```text
TimeoutObservation
    -> FailureDetector appraisal
    -> Suspected(P)
    -> protocol reaction
```

not:

```text
TimeoutObservation
    -> Failed(P)
```

A later message can legitimately clear suspicion without creating a contradiction about whether a timeout occurred.

This uses RFC 0018's distinction between observation, appraisal, and action.

## 19. Broadcast semantics

RFC 0028 reserves distinct broadcast relations:

```text
BestEffortBroadcast
ReliableBroadcast
UniformReliableBroadcast
CausalBroadcast
TotalOrderBroadcast
AtomicBroadcast
ByzantineReliableBroadcast
```

The exact formal definitions may evolve, but the properties may not be collapsed into one `broadcast` capability.

## 20. Atomic broadcast and consensus

Under appropriate crash-failure models, atomic broadcast and consensus have close reducibility relationships.

MNCS should preserve these as distinct semantic interfaces while permitting proofs/realizations that reduce one to the other.

A claim such as:

```text
AtomicBroadcastClaim {
    configuration
    delivery
    agreement
    order
    termination
    failure_model
}
```

may depend on a consensus witness.

## 21. Delivery ordering

Ordering contracts may include:

```text
Unordered
FIFOBySender
CausalOrder
TotalOrder
```

For causal delivery:

```text
send(M1) happens-before send(M2)
```

may generate a requirement that an observer delivering both cannot deliver M2 before M1.

Causality is inherited from RFC 0010; RFC 0028 owns the distribution-specific delivery constraint.

A total order among concurrent messages does not imply physical-time order.

## 22. Configurations and epochs

Distributed claims must be scoped to explicit configurations.

```text
DistributedConfiguration {
    configuration_id
    epoch
    participants
    roles
    quorum_system_ref?
    prior_configuration?
}
```

A consensus decision in configuration `C17` is not automatically a decision under `C18`.

A quorum proof over `{A,B,C}` cannot silently survive replacement with `{B,C,D}`.

## 23. Reconfiguration

Membership changes are distributed state transitions.

```text
ConfigurationTransition {
    from
    to
    transition_contract
    overlap_requirement?
    witness
}
```

RFC 0028 establishes the semantic need for safe reconfiguration.

RFC 0030 later owns broader software/deployment lifecycle and live state migration.

## 24. Group views and virtual-synchrony-style abstractions

A group-oriented realization may expose:

```text
GroupView {
    view_id
    configuration
    delivered_prefix_or_relation
}
```

and:

```text
ViewChange
```

so applications can reason about membership and message delivery under coherent view transitions.

Virtual synchrony is an available abstraction family, not a universal requirement.

## 25. Leader election

Leadership is modeled as:

```text
LeaderClaim {
    leader
    configuration
    epoch_or_term
    authority_scope
    evidence
}
```

`LeaderClaim` is distinct from:

```text
ConsensusDecision
ReplicatedLogSafety
MutationAuthority
```

A realization may use leaderless consensus or multi-leader replication where compatible with the required semantics.

## 26. Fencing epochs

To prevent stale leaders from mutating protected resources, MNCS should support monotonic fencing authority:

```text
FencingEpoch {
    resource_scope
    epoch
    issuer_or_derivation
}
```

A protected sink can require:

```text
incoming_epoch >= current_required_epoch
```

or a stronger exact-current relation.

Example:

```text
A elected at epoch 7
partition
B elected at epoch 8
A resumes with epoch 7
resource rejects A
```

This is stronger than relying on A to discover it is no longer leader.

## 27. Leases

A distributed lease is temporary authority:

```text
DistributedLease {
    holder
    resource
    epoch
    validity_interval
    clock_requirements
    revocation_or_expiry_semantics
}
```

Clock correctness belongs to RFC 0023.

Authority belongs to RFC 0025.

RFC 0028 relates those to distributed ownership/coordination semantics.

A lease may not be considered safe if its clock-skew/uncertainty assumptions are unestablished.

## 28. Replicas

A replica is a participant-local realization of some distributed logical state.

```text
Replica {
    replica_id
    participant
    logical_state_ref
    version_or_causal_state
    configuration
}
```

A replica may be:

```text
current
stale
concurrent-with-another-version
unknown
```

according to a declared relation.

Reachability does not imply freshness.

## 29. Replication contracts

A replication contract may state:

```text
ReplicationContract {
    logical_object
    replica_set
    write_behavior
    read_behavior
    conflict_behavior
    convergence_requirement
    consistency_requirement
    failure_model
}
```

Realizations may include:

```text
primary/backup
single-leader
multi-leader
quorum
chain replication
state-machine replication
CRDT
anti-entropy/gossip
```

## 30. Quorum systems

A quorum system is a predicate over participant subsets, not merely a number.

```text
QuorumSystem {
    configuration
    read_quorum_predicate?
    write_quorum_predicate?
    decision_quorum_predicate?
    required_intersections
}
```

Possible obligations include:

```text
forall R,W:
    R ∩ W != empty
```

and:

```text
forall W1,W2:
    W1 ∩ W2 != empty
```

where the protocol requires them.

## 31. Quorum-intersection witnesses

Quorum safety should be independently checkable where practical.

```text
QuorumWitness {
    quorum_system
    property
    proof_or_certificate
}
```

This is an excellent micro-verifier boundary.

An untrusted optimizer may propose weighted, geographic, hierarchical, or custom quorum families.

A small checker verifies the required intersections.

## 32. Fault tolerance and independence

A `FaultToleranceClaim` must include:

```text
fault_model
configuration
participant_count
fault_threshold
network_model
authentication_model?
quorum_model?
independence_assumptions
```

Three replicas using one shared storage controller may not be independent against controller failure.

Three replicas running identical buggy code may not provide independent evidence against common software faults.

RFC 0018's independence semantics apply.

## 33. Shared-memory abstractions over message passing

RFC 0028 permits stronger abstract distributed objects such as:

```text
LinearizableRegister<T>
```

without requiring physical shared memory.

A realization may implement such an object using message-passing quorums or consensus.

This follows the broader MNCS principle:

> semantic abstraction first; realization mechanism second.

## 34. Consistency as a relation family

`consistent = true` is forbidden as a complete contract.

At minimum, RFC 0028 recognizes distinct relations/classes such as:

```text
Linearizable
SequentiallyConsistent
CausallyConsistent
EventuallyConvergent
```

and client/session relations such as:

```text
ReadYourWrites
MonotonicReads
MonotonicWrites
WritesFollowReads
```

The RFC does not claim this is the final exhaustive consistency lattice.

## 35. Linearizability

A linearizability claim is an RFC 0020 relation over concurrent distributed histories requiring operations to appear as if they take effect atomically while respecting relevant real-time invocation/response precedence.

```text
LinearizabilityClaim {
    object
    history_scope
    operation_spec
    temporal_scope
    witness
    evidence
}
```

Linearizability is stronger than eventual convergence and different from sequential consistency.

## 36. Sequential consistency

A sequential-consistency claim requires a single total order compatible with each participant's program order, without necessarily respecting the same real-time precedence required by linearizability.

Therefore:

```text
SequentiallyConsistent
```

may hold while:

```text
Linearizable
```

fails.

## 37. Causal consistency

Causal consistency preserves ordering constraints induced by causal dependencies while allowing concurrent independent operations to be observed in different orders.

```text
CausalConsistencyClaim {
    object_or_system
    observer_scope
    causal_relation_ref
}
```

The causal relation is inherited from RFC 0010, not reconstructed from wall-clock timestamps.

## 38. Session guarantees

RFC 0028 explicitly recognizes useful client-local contracts over weakly replicated systems:

```text
ReadYourWrites
MonotonicReads
MonotonicWrites
WritesFollowReads
```

These are not promoted into global linearizability.

A client may receive a coherent session view while other clients observe temporarily different replica state.

## 39. Eventual convergence

A convergence claim must state its preconditions.

A typical form is:

```text
EventuallyConvergent {
    if:
        no_new_updates
        relevant_messages_eventually_delivered
    then:
        replicas_reach_equivalent_state
}
```

The exact fairness/network assumptions must be explicit.

A system cannot claim eventual convergence under a model in which required messages may be lost forever without some additional recovery/anti-entropy assumption.

## 40. Convergence versus invariants

Convergence proves that replicas eventually agree under the declared relation.

It does not prove that the agreed state satisfies the application invariant.

Example:

```text
Invariant: inventory >= 0
```

Two disconnected replicas each decrement the last unit.

A merge may converge deterministically to `-1`.

Thus:

```text
Convergence = SUPPORTED
InvariantPreservation = REFUTED
```

is legitimate.

## 41. Conflict resolution

Conflict-resolution semantics may include:

```text
RejectConcurrentConflict
LastWriterWins<Order>
MultiValue
SetUnion
ApplicationMerge<Function>
ConsensusResolve
CRDTMerge
```

These are different logical data semantics.

A compiler may not exchange them merely because all eventually produce one value.

## 42. Last-writer-wins

A last-writer-wins policy must identify the ordering relation:

```text
LastWriterWins {
    order:
        LogicalClock
        HybridClock
        PhysicalClockWithUncertaintyPolicy
        ReplicaSequence
        DeterministicTieBreak
}
```

A naked timestamp comparison is not sufficient.

RFC 0023 governs physical/logical time semantics.

## 43. Version metadata and concurrency

Replica-version metadata should preserve whether versions are:

```text
Before
After
Equivalent
Concurrent
Unresolved
```

under the declared causal/version relation.

Concurrent versions are information, not merely an error to erase.

A merge policy may act on them later.

## 44. CRDT semantics

CRDTs are a realization family for replicated abstract data types designed to support convergent behavior without global coordination under stated delivery assumptions.

MNCS should support at least conceptual distinction between:

```text
StateBasedCRDT
OperationBasedCRDT
```

without committing to either.

## 45. State-based merge algebra

A state-based convergent object may expose a merge operation:

```text
merge(a,b) = a join b
```

with obligations such as:

```text
associative
commutative
idempotent
```

and, where used, inflation/partial-order requirements.

These can be represented by:

```text
MergeAlgebraWitness {
    subject
    relation
    laws
    evidence
}
```

The CRDT generator may be untrusted while the algebra checker remains narrow.

## 46. Operation-based commutativity

Operation-based convergence may depend on concurrent operations commuting under a protected relation.

```text
CommutesUnder<R>(op_a, op_b)
```

is an RFC 0020 relation claim.

Forge may search for semantically equivalent commutative forms where permitted.

## 47. Anti-entropy and gossip

Propagation mechanisms such as gossip/anti-entropy are realization choices.

The semantic contract can remain:

```text
EveryRelevantUpdateEventuallyReachesEveryRelevantLiveReplica
```

under explicit network/fairness assumptions.

The language does not hard-code a gossip topology.

Topology remains primarily RFC 0029.

## 48. CALM and monotonicity

RFC 0028 adopts monotonicity as a major coordination-analysis concept.

A monotonic computation is one whose conclusions are not invalidated merely by learning more input facts under the relevant logic/order.

Where supported by the chosen theoretical model, monotonicity can justify coordination-free distributed realization.

MNCS should therefore support:

```text
MonotonicityClaim {
    computation
    information_order
    scope
    witness
}
```

and generate:

```text
CoordinationRequirement = NONE
```

only when the required theorem/assumptions are established.

## 49. Non-monotonicity and coordination obligations

Questions such as:

```text
is this set empty?
is this username globally unused?
have all participants responded?
```

may be non-monotonic because future information can invalidate a provisional conclusion.

The system may then generate:

```text
CoordinationRequirement {
    protected_property
    operation_set
    scope
    reason
}
```

rather than making the programmer choose consensus first.

## 50. Invariant confluence

Invariant confluence asks whether independently valid transactions/operations can be merged while preserving a declared invariant.

```text
InvariantConfluenceClaim {
    invariant
    operations
    merge_relation
    partitioned_execution_model
    witness
}
```

If operations are invariant-confluent under the model, coordination-free execution may be admissible.

If not, the system generates a coordination obligation.

## 51. Coordination as a semantic consequence

One of the primary machine-native principles of RFC 0028 is:

> coordination should be demanded by semantics, not selected first as a library.

Forge should be able to derive a need for coordination from:

```text
non-monotonic logic
non-commuting operations
invariant-confluence failure
strong consistency requirements
single-writer authority
unique-global-name requirements
atomic commit requirements
```

Then realization search can consider:

```text
consensus
quorum
lease
fencing
ownership partition
escrow
serialization
localization
```

## 52. CAP semantics

CAP-related claims must name:

```text
partition model
consistency relation
availability predicate
operation class
participant scope
```

MNCS does not expose a universal:

```text
CAP = CP | AP | CA
```

Instead a theorem/counterexample may show that a requested combination cannot be simultaneously guaranteed under the declared partition model.

## 53. Operation-relative availability

Availability is represented as a relation/predicate over an operation class and failure model.

```text
AvailabilityClaim {
    operations
    failure_model
    participant_scope
    response_requirement
}
```

Example:

```text
ReadCachedProfile:
    available under partition

TransferFunds:
    unavailable under partition unless required coordination is present
```

A system need not have one universal availability status.

## 54. Consensus semantics

Consensus is an abstract agreement problem:

```text
ConsensusInstance<V> {
    instance_id
    configuration
    proposals
    validity_requirement
    agreement_requirement
    liveness_requirement
    failure_model
    network_model
}
```

The result is:

```text
ConsensusDecision<V>
```

with explicit evidence/witnesses.

Paxos, Raft, Viewstamped Replication, randomized protocols, and future mechanisms are realization families.

## 55. Consensus safety

Safety obligations include the declared forms of:

```text
Agreement
Validity
NoConflictingDecision
```

These may remain supported through partitions where liveness is lost.

## 56. Consensus liveness

Liveness is a separate RFC 0022 temporal/progress claim and must name assumptions such as:

```text
partial synchrony
eventual stable leader
sufficient live quorum
failure detector properties
fair message delivery
randomization assumptions
```

If those assumptions are missing or false, liveness may be `UNRESOLVED` while safety remains supported.

## 57. Consensus realizations

Recognized realization families include:

```text
Paxos-family
Raft-family
ViewstampedReplication-family
randomized consensus
hardware/trusted-component assisted consensus
```

The list is illustrative, not normative.

No realization inherits trust merely by name.

## 58. State-machine replication

A replicated state machine combines:

```text
deterministic logical state machine
ordered command history
replica execution
failure model
```

```text
ReplicatedStateMachine {
    machine_spec
    configuration
    ordering_contract
    replica_determinism_requirement
    recovery_contract
}
```

State-machine replication is a realization family for fault-tolerant services, not the only replication model.

## 59. Replica determinism

If replicas are expected to execute an identical command order and reach equivalent state, nondeterministic inputs must be controlled or represented explicitly.

Potential hidden divergence sources include:

```text
ambient clock reads
randomness
unordered iteration
external I/O
local environment differences
floating-point implementation differences
```

Relevant semantics are inherited from RFC 0011, RFC 0021, RFC 0023, and RFC 0017.

## 60. Byzantine fault semantics

A Byzantine participant may behave arbitrarily within the modeled attacker capability:

```text
send conflicting messages
omit messages
forge invalid state transitions
collude
lie about local state
```

Authenticated Byzantine models may prevent identity forgery without preventing dishonest signed messages.

Therefore:

```text
Authenticated(P)
```

never implies:

```text
Honest(P)
```

## 61. Byzantine broadcast and agreement

RFC 0028 distinguishes:

```text
ByzantineReliableBroadcast
ByzantineAgreement
ByzantineStateMachineReplication
```

These are related but not identical primitives.

A BFT realization must name:

```text
fault threshold
participant count
authentication assumptions
network/synchrony assumptions
quorum structure
```

## 62. PBFT-family realizations

PBFT-family protocols are recognized as one realization direction for Byzantine state-machine replication.

They are not the language semantics.

Any future BFT mechanism may satisfy the same abstract distributed contract if its obligations are evidenced.

## 63. Fault-threshold claims

A proper claim resembles:

```text
FaultToleranceClaim {
    configuration
    fault_model
    tolerated_fault_count
    network_model
    authentication_model
    quorum_model
    independence_assumptions
    witness
}
```

A naked statement such as `tolerates 2 faults` is semantically incomplete.

## 64. Common-mode failures

Replica independence must be explicit where it matters.

Examples of shared dependencies:

```text
same power domain
same storage controller
same region
same software bug
same compiler/toolchain
same model-generated defect
same credential issuer
```

Replication cannot be treated as independent evidence when common-mode dependencies dominate the relevant failure class.

RFC 0018 appraises this evidence.

## 65. Distributed snapshots

A distributed snapshot represents a consistent global cut through participant state and in-flight communication.

```text
DistributedSnapshot {
    snapshot_id
    configuration
    local_state_cuts
    channel_state
    consistency_witness
}
```

It is not necessarily taken at one physical time.

It is not the same object as an RFC 0026 persistent snapshot.

A distributed snapshot may itself be durably stored using RFC 0026.

## 66. Consistent cuts

A distributed snapshot must preserve the relevant event-order consistency.

Intuitively, it must not include an event while excluding prerequisite causal history in a way that creates an impossible global state.

The exact relation is expressed with RFC 0010/RFC 0020 machinery.

## 67. Stable global properties

Some properties are stable: once true, they remain true under the modeled execution.

Examples can include:

```text
global termination
deadlock
quiescence
completion of a monotonic distributed phase
```

A distributed snapshot can become a witness for a stable global property.

```text
StableGlobalPropertyClaim {
    property
    snapshot
    stability_witness
}
```

## 68. Distributed transactions

A distributed transaction spans participants that can fail independently.

```text
DistributedTransaction {
    transaction_id
    participants
    local_transaction_refs
    commit_condition
    failure_model
    coordination_protocol_ref?
}
```

RFC 0026 local atomicity is not sufficient to establish distributed atomicity.

## 69. Atomic commit

Distributed atomic commit requires an agreement on whether all required transaction effects become committed or aborted according to the declared commit condition.

```text
AtomicCommitClaim {
    transaction
    safety
    termination
    participant_outcomes
    witness
}
```

Atomic commit and generic consensus are related but remain distinct semantic problems because their validity conditions differ.

## 70. Two-phase commit

Two-phase commit is a realization family for distributed atomic commit.

It may preserve atomic-commit safety while becoming blocked after certain coordinator failures.

Therefore:

```text
AtomicCommitSafety = SUPPORTED
AtomicCommitLiveness = UNRESOLVED/BLOCKED
```

is a legitimate state.

The language must not report only `transaction_ok=false`.

## 71. Consensus-backed commit

A stronger realization can use fault-tolerant agreement mechanisms to reduce coordinator single-point blocking under a corresponding model.

This does not change the abstract atomic-commit semantics; it changes the realization and liveness/fault assumptions.

## 72. Sagas

A saga is a long-running distributed workflow composed of individually committed actions and declared compensations.

```text
Saga {
    saga_id
    forward_steps
    compensations
    allowed_intermediate_visibility
    terminal_business_relations
}
```

A saga is not atomic commit.

## 73. Compensation

A compensation attempts to restore an application-level relation after an already-visible effect.

Example:

```text
charge -> refund
```

may restore a financial balance relation while failing historical equivalence to `charge never occurred`.

Thus compensation should claim a specific RFC 0020 refinement/equivalence relation, not `rollback`.

## 74. Distributed result uncertainty

A participant may legitimately end with:

```text
COMMITTED
ABORTED
COMPENSATED
BLOCKED
OUTCOME_UNKNOWN
```

from its current observer perspective.

Reconciliation may later refine `OUTCOME_UNKNOWN`.

RFC 0018 tracks evidence changes rather than pretending the earlier observer had knowledge it did not possess.

## 75. Distributed durability

RFC 0026 owns local persistence domains and crash consistency.

RFC 0028 composes them into replicated/distributed durability requirements.

A distributed durability claim may require:

```text
k independent durable replicas
quorum-confirmed persistence
tolerated loss domains
current configuration
```

but must not redefine local persistence primitives.

## 76. Replica durability and acknowledgement

A leader acknowledging a write may mean:

```text
leader memory accepted
leader local durable
one follower received
quorum durable
all replicas durable
```

These are distinct acknowledgement semantics.

The protocol contract must state which one is being promised.

## 77. Availability versus durability

A highly available realization may accept operations that are not yet replicated durably enough for a stronger failure model.

A highly durable realization may refuse operations during a partition.

Neither property universally implies the other.

## 78. Serialization and wire semantics

RFC 0027 owns message/schema encoding.

RFC 0028 consumes those wire contracts but adds distributed delivery/version coexistence behavior.

A readable message is not necessarily authorized to execute.

An unknown future field may be safe to forward but unsafe to interpret under an older participant.

Mixed-version distributed protocol behavior remains explicit.

## 79. Schema coexistence in distributed participants

Different participants may temporarily use different compatible schema versions.

RFC 0028 may refer to:

```text
SenderSchema
ReceiverSchema
CompatibilityClaim
```

from RFC 0027 while reasoning about distributed delivery.

The rollout lifecycle remains primarily RFC 0030.

## 80. Information-flow and distributed observations

Messages, metadata, delivery behavior, retries, failure suspicions, replica selection, timing, and quorum participation may all become RFC 0024 observation channels.

A distribution optimization must not weaken a protected information-flow relation silently.

For example, moving from constant fanout to data-dependent replica selection may introduce a new observer channel.

## 81. Distributed identity and authentication

Participants can carry RFC 0025 principals/credentials.

Authentication can establish:

```text
message attributable to principal P
```

but does not establish:

```text
P follows the protocol
P's local state is acceptable
P is non-Byzantine
```

Attestation may provide additional evidence but still requires RFC 0018 appraisal.

## 82. Authority and distributed configuration

Mutation authority may be configuration- and epoch-scoped.

A valid credential from a former leader can be rejected because its fencing epoch is stale.

Thus:

```text
PrincipalAuthority
```

and:

```text
CurrentDistributedAuthority
```

remain distinct.

## 83. Distributed relation claims

Candidate relation types include:

```text
LinearizableHistory
SequentiallyConsistentHistory
CausallyConsistentHistory
SessionGuarantee
Convergence
ReplicaEquivalence
AtomicBroadcastAgreement
ConsensusAgreement
AtomicCommitSafety
DistributedSnapshotConsistency
CompensationRefinement
```

Every relation names its subject, observer/context scope, configuration, network/failure assumptions, witness, and evidence where relevant.

## 84. Counterexamples

Distributed counterexamples should be explicit artifacts.

```text
DistributedCounterexample {
    subject
    violated_relation
    configuration
    network_model
    failure_schedule
    message_schedule
    operation_history
    observer
    minimized_trace?
}
```

Examples include:

```text
split-brain conflicting writes
non-intersecting quorum histories
stale read violating RYW
CRDT merge violating invariant
consensus double-decision
unresolved 2PC after coordinator crash
```

## 85. Bounded failure/partition exploration

A model checker or simulator may explore finite combinations of:

```text
message loss
message duplication
message reordering
participant crash
participant restart
partition
partition healing
leader delay
failure-detector mistakes
```

The result is:

```text
BoundedDistributedEvidence
```

not universal proof.

Successful bounded exploration must remain method/domain scoped under RFC 0018.

## 86. Proof and certificate families

Candidate evidence families include:

```text
quorum intersection certificates
history linearizability witnesses
state-machine invariants
consensus safety proofs
failure-detector assumptions
CRDT algebra law proofs
invariant-confluence proofs
monotonicity proofs
reconfiguration overlap certificates
distributed snapshot consistency certificates
bounded fault-injection reports
model-checker traces
```

No single verifier is privileged as the definition of distributed correctness.

## 87. Micro-verifier opportunities

RFC 0028 exposes several unusually clean micro-verifier boundaries:

### QuorumIntersectionVerifier

Checks required quorum intersections for a configuration.

### HistoryConsistencyVerifier

Checks a bounded history against a declared consistency relation.

### CRDTMergeLawVerifier

Checks associativity/commutativity/idempotence or other declared merge laws.

### CoordinationNecessityVerifier

Checks monotonicity/invariant-confluence witnesses or produces a coordination obligation.

### ConfigurationTransitionVerifier

Checks reconfiguration overlap/epoch rules.

### FencingVerifier

Checks authority epochs against protected resource state.

### DistributedSnapshotVerifier

Checks consistency of a bounded global cut.

### AtomicCommitHistoryVerifier

Checks bounded participant outcomes against atomic-commit safety.

### FaultModelApplicabilityVerifier

Refuses a proof/result when the realization's fault assumptions are weaker than the contract requires.

## 88. Lowering preservation

Distributed semantic obligations must survive lowering.

Compiler/backend transformations must not silently change:

```text
logical operation identity
retry/deduplication identity
message ordering requirements
consistency relation
quorum membership
fencing epoch propagation
failure assumptions
coordination requirements
wire compatibility
security observer scope
```

A transformation that batches/reorders network operations may be functionally equivalent in failure-free local tests yet violate causal or linearizable distributed semantics.

Therefore:

```text
FailureFreeFunctionalEquivalent
```

never implies:

```text
DistributedEquivalent
```

without a declared relation proof.

## 89. Distribution deltas

Forge refinement should expose:

```text
DistributionDelta {
    participant_changes
    message_contract_changes
    consistency_changes
    replication_changes
    quorum_changes
    fault-model changes
    coordination_changes
    authority_epoch_changes
    evidence_changes
}
```

Any weakening of a protected relation must be explicit and policy-gated.

## 90. Forge realization search

A distributed region may leave realization unresolved:

```text
DistributedContract {
    state
    operations
    consistency
    availability
    fault_model
    convergence
    invariants
    coordination_strategy: ?
    replication_strategy: ?
}
```

Forge can evaluate realization families such as:

```text
single leader
multi leader
quorum register
chain replication
state-machine replication
CRDT
gossip/anti-entropy
lease/fencing
atomic broadcast
consensus
distributed commit
```

Candidates remain untrusted until obligations are discharged.

## 91. Forge should prefer minimum required coordination

The optimization problem is not:

```text
choose the strongest distributed protocol
```

It is:

```text
choose a realization satisfying protected semantics
while minimizing unnecessary coordination/cost/risk
```

A consensus-backed log may be valid but unnecessarily expensive for a monotonic grow-only set.

A CRDT may be efficient but invalid for a globally unique claim operation.

## 92. First Forge study: coordination-free cart

Declare:

```text
DistributedStateContract {
    state: ShoppingCart
    partition_writes: allowed
    consistency:
        ReadYourWrites
        CausalConsistency
    convergence: required
    invariant:
        no duplicate cart item identities
    byzantine: not required
    realization: ?
}
```

Candidate A:

```text
consensus-backed replicated log
```

may satisfy the contract but use unnecessary coordination.

Candidate B:

```text
CRDT cart
```

may satisfy the contract with a merge witness.

Candidate C:

```text
last-writer-wins whole-cart register
```

may lose concurrent additions and fail the semantic relation.

Candidate D:

```text
ad hoc asynchronous merge
```

may remain `UNRESOLVED` without convergence evidence.

Forge should select only from admissible candidates and record why.

## 93. Second Forge study: globally unique claim

Declare:

```text
Invariant:
    username globally unique

Operation:
    claim(name)

Partition rule:
    two partitions may not both return success for same name
```

Candidate strategies:

```text
local-only check
LWW register
CRDT set
quorum
consensus
ownership partition
```

Forge should discover that some highly available coordination-free realizations cannot satisfy the required global success semantics under the declared partition model.

It may:

1. choose a coordination strategy;
2. choose an ownership/partitioning realization that changes the admissible operation scope; or
3. report the requirements mutually unrealizable under the current assumptions.

It must not silently weaken uniqueness.

## 94. First bounded implementation pilot

The first executable RFC 0028 slice should remain deliberately small.

Implement versioned artifacts for:

```text
Participant
DistributedConfiguration
ConfigurationEpoch
Channel
Message
LogicalOperationIdentity
InvocationAttemptIdentity
NetworkModel
FailureObservation
FailureDetector
SuspicionClaim
Replica
QuorumSystem
ConsistencyClaim
ConsensusInstance
ConsensusDecision
CoordinationRequirement
DistributedCounterexample
DistributionDelta
```

The initial evaluator/model checker may remain finite and explicit.

## 95. Pilot fixture: send is not effect

History:

```text
A sends O17
B receives O17
B crashes before apply
```

Required conclusion:

```text
Sent(O17) = true
Applied(O17) = false
```

No rule may infer application from transmission.

## 96. Pilot fixture: applied is not observed

History:

```text
A sends O17
B applies O17
B sends success
response lost
```

Required conclusion:

```text
B: APPLIED
A: OUTCOME_UNKNOWN
```

The two observer-relative conclusions coexist.

## 97. Pilot fixture: retries plus deduplication

Operation:

```text
increment counter
logical operation id O17
```

Deliver three attempts.

With durable deduplication, the counter changes once.

Required conclusion:

```text
DuplicateDelivery = observed
ExactlyOnceLogicalEffect<O17> = supported
```

## 98. Pilot fixture: timeout is not failure

History:

```text
A waits for B
A times out
FailureDetector suspects B
later heartbeat arrives
suspicion cleared
```

Required conclusion:

```text
TimeoutObservation retained
Suspected(B) was true under detector output
Failed(B) not established
```

## 99. Pilot fixture: FLP boundary

Model:

```text
asynchronous
deterministic consensus
one possible crash
```

Required behavior:

```text
ConsensusSafety may be checked
UniversalConsensusTermination may not be asserted
```

Change the environment to a supported partial-synchrony/failure-detector profile and permit a scoped liveness witness.

## 100. Pilot fixture: quorum intersection

Build one intentionally invalid quorum configuration and one valid configuration.

The verifier must produce either:

```text
QuorumIntersectionWitness
```

or a concrete pair of non-intersecting quorums.

Do not infer safety from replica count alone.

## 101. Pilot fixture: convergence versus linearizability

Compare:

```text
A: atomic/quorum register
B: asynchronously replicated convergent register
```

Both may establish convergence.

Only A should establish linearizability under the fixture.

Required conclusion:

```text
Convergent != Linearizable
```

## 102. Pilot fixture: session guarantee over weak replicas

Allow temporary replica divergence.

Move one client between replicas while preserving:

```text
ReadYourWrites
MonotonicReads
```

Demonstrate those client-local guarantees without claiming global linearizability.

## 103. Pilot fixture: CRDT merge laws

Use a grow-only set.

Concurrent operations:

```text
A: add(x)
B: add(y)
```

Check merge-law witnesses and convergence to `{x,y}` after exchange.

## 104. Pilot fixture: convergence versus invariant

Use replicated inventory with:

```text
Invariant: inventory >= 0
```

During partition, two replicas independently consume the last unit.

After merge:

```text
Convergence = supported
InvariantPreservation = refuted
```

This must generate a coordination/invariant obligation rather than reject the convergence theorem.

## 105. Pilot fixture: coordination derivation

Case A:

```text
monotonic/invariant-confluent operations
```

Result:

```text
CoordinationRequirement = NONE
```

Case B:

```text
same distribution model
stronger invariant/non-confluent operation
```

Result:

```text
CoordinationRequirement = REQUIRED
```

The difference should be machine-explainable.

## 106. Pilot fixture: split brain and fencing

History:

```text
A leader epoch 7
partition
B becomes leader epoch 8
A later issues write with epoch 7
```

Protected resource must reject A's stale write.

## 107. Pilot fixture: distributed snapshot

Create three participants and in-flight messages.

Produce one consistent cut and one inconsistent arbitrary cut.

The verifier must distinguish them without relying on simultaneous wall-clock timestamps.

## 108. Pilot fixture: 2PC safety versus liveness

Prepare all participants.

Crash coordinator before decision dissemination.

Required result:

```text
atomicity safety retained
some participants blocked/decision unknown
liveness not established
```

Compare later with a stronger consensus-backed commit realization if implemented.

## 109. Pilot fixture: saga compensation

Workflow:

```text
reserve inventory
charge payment
shipping fails
refund payment
release inventory
```

The final business relation may be restored.

Historical equivalence to `nothing happened` must remain false/unclaimed.

## 110. Assurance integration

Every significant distributed claim must be appraisable under RFC 0018.

Examples:

```text
quorum witness valid for configuration C17
failure detector evidence stale after network model changed
consensus proof supports crash-stop, not Byzantine
CRDT law proof current but invariant-confluence evidence missing
bounded partition test passes but universal proof absent
```

Action profiles may authorize:

```text
simulation
sandbox cluster
read-only deployment
production mutation
promotion
```

at different evidence levels.

## 111. Target/runtime integration

RFC 0017 provides target/runtime facts such as:

```text
transport type
network capabilities
failure domains
clock capabilities
storage durability
scheduler/runtime identity
```

RFC 0028 consumes those facts.

A change from reliable LAN assumptions to intermittently connected transport can invalidate distributed liveness/latency/lease evidence while leaving logical program semantics unchanged.

## 112. Relationship to RFC 0008

RFC 0008 owns authority to perform network I/O and other effects.

RFC 0028 does not allow a distributed operation to bypass capabilities merely because a messaging protocol exists.

`SendCapability<Channel>` and similar authorities remain separate from the consistency/replication semantics of what is sent.

## 113. Relationship to RFC 0009

RFC 0009 owns reference identity and provenance.

RFC 0028 may relate:

```text
logical replicated object
replica instances
operation provenance
message provenance
```

without redefining storage/reference identity.

## 114. Relationship to RFC 0010

RFC 0010 owns general concurrency, causality, atomicity, and memory consistency.

RFC 0028 consumes causal relations and extends them into message-delivery, distributed history, replication, and agreement semantics.

It does not redefine happened-before.

## 115. Relationship to RFC 0011

RFC 0011 owns generic failure, recovery, nondeterminism, and observation semantics.

RFC 0028 specializes them for partial failure, observer-relative remote outcome, partitions, suspicion, and Byzantine behavior.

## 116. Relationship to RFC 0022

RFC 0022 owns progress/liveness/fairness terminology.

RFC 0028 uses it for:

```text
consensus termination
broadcast delivery
anti-entropy convergence
atomic-commit blocking
failure-detector liveness assumptions
```

Safety and liveness remain separate.

## 117. Relationship to RFC 0023

RFC 0023 owns clocks/time/deadlines.

RFC 0028 uses it for:

```text
leases
heartbeat timeouts
failure-detector timing
partial-synchrony bounds
LWW physical-time profiles
```

A distributed semantic claim cannot fabricate a global clock.

## 118. Relationship to RFC 0024

RFC 0024 owns information-flow/confidentiality/integrity.

Distributed metadata and behavior can be observer channels:

```text
which replica contacted
retry count
quorum membership
failure suspicion
timing
message size
partition behavior
```

RFC 0028 realizations must preserve protected security relations.

## 119. Relationship to RFC 0025

RFC 0025 owns principal identity/authentication/cryptographic trust.

RFC 0028 uses those identities in authenticated participants, Byzantine protocols, leases, quorum membership, leadership, and fencing.

Authenticated identity never implies honesty.

## 120. Relationship to RFC 0026

RFC 0026 owns local durable state, local transaction semantics, persistence domains, and crash consistency.

RFC 0028 owns:

```text
replicated durability
distributed acknowledgement
distributed atomic commit
cross-participant recovery knowledge
```

without weakening local persistence rules.

## 121. Relationship to RFC 0027

RFC 0027 owns message schema/wire/canonicalization/evolution semantics.

RFC 0028 owns what happens when those messages traverse participants with independent failure/version state.

Wire compatibility does not prove distributed protocol compatibility.

## 122. Boundary with RFC 0029

RFC 0029 should own:

```text
physical placement
topology
locality
migration
mobility
heterogeneous-node placement
network-path locality
```

RFC 0028 can state that participants/channels/failure domains exist, but should not decide where Forge physically places them.

## 123. Boundary with RFC 0030

RFC 0030 should own deployment lifecycle, live upgrade, state migration, rollback, and version coexistence as operational lifecycle concerns.

RFC 0028 defines safe reconfiguration/membership semantics needed by distributed protocols.

## 124. Boundary with RFCs 0032 and 0033

RFC 0032 will own quantitative concerns such as:

```text
latency
bandwidth
replication overhead
coordination cost
availability metrics
recovery cost
energy
```

RFC 0033 will own multi-objective selection among semantically valid realizations.

RFC 0028 determines admissibility first.

## 125. Realization families explicitly kept out of the ontology

The following may be valuable but are not language primitives merely because they are named here:

```text
TCP
QUIC
HTTP/RPC
gRPC
Paxos
Multi-Paxos
Raft
Viewstamped Replication
PBFT
HotStuff-family
Dynamo-style replication
COPS-style causal replication
CRDT libraries
2PC
3PC
Paxos Commit
chain replication
virtual synchrony
```

They can contribute realizations, evidence, or reference behavior.

## 126. Reference theoretical stack

The RFC is informed by the following theory families:

1. Lamport-style distributed event/causal ordering.
2. Synchronous, asynchronous, and partially synchronous message-passing models.
3. Fischer–Lynch–Paterson asynchronous consensus impossibility.
4. Chandra–Toueg unreliable failure detectors.
5. Reliable/causal/atomic broadcast.
6. State-machine replication.
7. Paxos, Viewstamped Replication, and Raft families.
8. Quorum systems and weighted voting.
9. ABD-style emulation of atomic shared objects over message passing.
10. Linearizability.
11. Sequential consistency.
12. Causal consistency.
13. Session guarantees.
14. CAP under precise definitions.
15. Eventual convergence and anti-entropy.
16. CRDT algebra.
17. CALM/monotonicity and coordination-freedom.
18. Invariant confluence and coordination avoidance.
19. Virtual synchrony/group membership.
20. Distributed snapshots/consistent cuts.
21. Byzantine agreement/reliable broadcast.
22. PBFT-family state-machine replication.
23. Leases and fencing epochs.
24. Distributed atomic commit, 2PC, and consensus-backed commit.
25. Sagas/compensation.
26. Proof-producing history/quorum/merge/configuration verifiers.

These theories are complementary. No single one is sufficient as the complete distributed ontology.

## 127. Reference works

The design direction should continue to consult primary sources including:

- Leslie Lamport, *Time, Clocks, and the Ordering of Events in a Distributed System*.
- Fischer, Lynch, and Paterson, *Impossibility of Distributed Consensus with One Faulty Process*.
- Dwork, Lynch, and Stockmeyer, *Consensus in the Presence of Partial Synchrony*.
- Chandra and Toueg, *Unreliable Failure Detectors for Reliable Distributed Systems*.
- Lamport, *Paxos Made Simple* and the underlying Paxos work.
- Oki and Liskov, Viewstamped Replication work.
- Ongaro and Ousterhout, *In Search of an Understandable Consensus Algorithm (Raft)*.
- Herlihy and Wing, *Linearizability: A Correctness Condition for Concurrent Objects*.
- Lamport, *How to Make a Multiprocessor Computer That Correctly Executes Multiprocess Programs*.
- Gifford, weighted voting/quorum replication work.
- Attiya, Bar-Noy, and Dolev, robust shared memory over message passing.
- Terry et al., session guarantees.
- Gilbert and Lynch, CAP formalization.
- DeCandia et al., Dynamo.
- Shapiro et al., CRDT theory/surveys.
- Hellerstein/Alvaro et al., CALM and coordination theory.
- Bailis et al., invariant confluence / coordination avoidance.
- Birman/Joseph, virtual synchrony.
- Chandy and Lamport, distributed snapshots.
- Lamport, Shostak, and Pease, Byzantine Generals.
- Castro and Liskov, PBFT.
- Gray and Cheriton, leases.
- Gray and Lamport, consensus on transaction commit / Paxos Commit.
- Garcia-Molina and Salem, Sagas.

References should remain informative until individual semantic rules are selected and formalized.

## 128. First micro-verifier package

The first RFC 0028 implementation should preferably produce a small verifier package rather than one monolith:

```text
mncs.distribution.quorum-intersection
mncs.distribution.history-consistency
mncs.distribution.failure-model-applicability
mncs.distribution.crdt-merge-laws
mncs.distribution.coordination-requirement
mncs.distribution.configuration-transition
mncs.distribution.fencing
mncs.distribution.snapshot-consistency
```

Each verifier should:

1. receive a versioned request artifact;
2. identify exact subject/configuration/model scope;
3. return `PASS`, `FAIL`, or `UNKNOWN`;
4. produce a witness/counterexample where applicable;
5. identify assumptions;
6. avoid claiming broader relations than it checked.

## 129. Diagnostics

Diagnostics should prefer semantic explanations.

Bad:

```text
cluster invalid
```

Better:

```text
QUORUM_INTERSECTION_REFUTED
configuration: C17
required relation: write/write intersection
counterexample:
    W1 = {A,B}
    W2 = {C,D}
```

Bad:

```text
consensus failed
```

Better:

```text
CONSENSUS_LIVENESS_UNRESOLVED
safety: supported
network model: asynchronous
failure model: one crash allowed
missing requirement:
    synchrony/failure-detector/randomization assumption
```

Bad:

```text
replication conflict
```

Better:

```text
INVARIANT_CONFLUENCE_REFUTED
invariant: inventory >= 0
concurrent operations:
    reserve(1)@A
    reserve(1)@B
merge result:
    inventory = -1
coordination requirement generated
```

## 130. Security and adversarial review

RFC 0028 implementations must assume distributed metadata and control paths can be security-sensitive.

Threats include:

```text
message replay
identity spoofing if RFC 0025 is absent
stale-leader writes
Byzantine equivocation
quorum manipulation
membership poisoning
schema downgrade
failure-detector manipulation
timing/availability side channels
malicious partition behavior
```

The language-level response is not to build one mandatory security protocol. It is to make the assumptions and protected relations explicit enough that RFC 0024/0025 assurance can reason about them.

## 131. Conservative defaults

Where the contract is incomplete, conservative behavior should include:

```text
unknown remote outcome stays UNKNOWN
missing failure model blocks strong fault-tolerance claims
missing quorum proof blocks quorum-derived safety claims
missing synchrony assumption blocks dependent liveness claims
unknown replica freshness blocks claims requiring current state
unknown future protocol fields may be stored/forwarded but not necessarily executed
stale configuration invalidates dependent leadership/quorum evidence
```

The system should not guess distributed semantics from library names.

## 132. Future formalization questions

Later work must refine:

- exact canonical history models for each consistency relation;
- the relation between RFC 0010 histories and RFC 0028 network traces;
- fairness assumptions for delivery and anti-entropy;
- exact failure-detector class representation;
- Byzantine/adaptive/mobile adversary models;
- reconfiguration theorems and joint-consensus-like abstractions;
- compositional proofs between local RFC 0026 durability and distributed durability;
- composition of multiple CRDT/invariant-confluent objects;
- coordination synthesis from declarative invariants;
- cross-schema distributed protocol compatibility;
- observer-relative knowledge models for remote operations;
- exact proof objects suitable for a narrow checker.

These open questions do not weaken the constitutional distinctions established here.

## 133. Non-goals

RFC 0028 does not attempt to:

- select one cluster manager;
- define DNS/service discovery;
- choose TCP/QUIC/HTTP/gRPC;
- choose Paxos/Raft/PBFT as the language's consensus algorithm;
- require CRDTs;
- guarantee global availability;
- guarantee global exactly-once delivery;
- hide partial failure behind ordinary exceptions;
- define physical node placement/topology;
- define deployment/live-upgrade policy;
- define cost/latency optimization;
- make all distributed programs deterministic;
- erase uncertainty from remote outcomes.

## 134. Acceptance direction

This RFC should be considered ready to move beyond Draft only after MNCS has an executable bounded model that demonstrates at minimum:

1. `Sent != Applied`;
2. remote `Applied` with caller `OUTCOME_UNKNOWN` after lost acknowledgement;
3. duplicate delivery plus exactly-once logical effect through explicit operation identity/deduplication;
4. `Timeout != Failure` through a suspicion/clearing fixture;
5. refusal to claim universal deterministic consensus termination under the asynchronous one-crash model;
6. one independently checked quorum-intersection witness and one refuted quorum configuration;
7. `Convergent != Linearizable`;
8. at least one session guarantee over temporarily stale replicas;
9. one CRDT/merge-algebra witness;
10. one convergent-but-invariant-violating counterexample;
11. one monotonic/invariant-confluent region requiring no coordination and one region generating a coordination obligation;
12. stale-leader fencing rejection;
13. one consistent and one inconsistent distributed snapshot;
14. 2PC-style safety retained while liveness blocks after a coordinator failure;
15. saga compensation restoring a declared application relation without claiming historical rollback;
16. one RFC 0018 policy refusing to promote bounded failure testing into universal distributed correctness;
17. one Forge study rejecting a faster/less-coordinated candidate that violates protected semantics.

## 135. Proposed 1.0 research-threshold consequences

A coherent MNCS 1.0 research system should therefore eventually include:

- a versioned distributed execution model distinguishing participant, message, logical operation, attempt, configuration, replica, and observer identities;
- a versioned network/failure model distinguishing synchrony assumptions, communication failures, partitions, crashes, suspicions, and Byzantine behavior;
- explicit message lifecycle/delivery/order semantics;
- explicit retry/deduplication/exactly-once-effect semantics;
- a versioned consistency relation family;
- explicit quorum systems and checkable intersection evidence;
- explicit consensus safety/liveness and impossibility-aware assumption handling;
- explicit failure-detector evidence rather than timeout-as-failure inference;
- explicit configuration epochs/reconfiguration/fencing;
- CRDT/convergence and invariant-preservation distinctions;
- monotonicity/invariant-confluence-based coordination obligations;
- distributed snapshot/global-state semantics;
- distributed atomic-commit and saga/compensation distinctions;
- Byzantine/authenticated-Byzantine distinctions;
- at least one realization-search study that derives coordination requirements from semantic contracts rather than choosing a distributed protocol first.

## 136. Closing principle

The long-term machine-native target is not a language with built-in syntax for every distributed algorithm.

It is a language where a semantic graph can state:

```text
what participants may know
what operations may occur more than once
what outcomes may remain unknown
which histories are legal
which failures are tolerated
which replicas may diverge
which relations must eventually converge
which invariants must always hold
which consistency relation clients require
which availability is required for which operations
which coordination is actually necessary
which assumptions make liveness possible
```

and Forge can then search the realization space.

A CRDT may be selected when algebra and invariants permit coordination-free convergence.

A quorum protocol may be selected when intersection is sufficient.

A lease/fencing scheme may be selected for scoped temporary authority.

Consensus may be introduced only where agreement semantics require it.

A distributed commit protocol may be introduced only where cross-participant atomicity requires it.

If the requested combination is impossible under the declared network/failure model, the correct compiler behavior is not to invent a hidden assumption.

It is to return an explicit unsatisfied obligation or impossibility explanation.

That is the RFC's central machine-native principle:

> **Distribution is a semantic search problem over knowledge, histories, failure and coordination—not a networking library choice.**
