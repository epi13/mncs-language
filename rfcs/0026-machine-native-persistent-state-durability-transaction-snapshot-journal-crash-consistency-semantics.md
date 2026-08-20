# RFC 0026: Machine-Native Persistent State, Durability, Transaction, Snapshot, Journal, and Crash-Consistency Semantics

- **Status:** Draft
- **Area:** Semantic foundation / persistent state / transactions / recovery
- **Depends on:** RFC 0001, RFC 0002, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0017, RFC 0018, RFC 0019, RFC 0020, RFC 0023, RFC 0024, RFC 0025
- **Informs:** RFC 0027, RFC 0028, RFC 0030, RFC 0031, RFC 0032, RFC 0033, RFC 0034 and later storage/distributed/tooling work

## Abstract

MNCS requires a machine-native theory for answering questions such as:

- which logical state exists independently of its physical storage representation;
- which state transitions are visible to concurrent computation;
- which state transitions have crossed a declared persistence boundary;
- which failures a particular durability claim actually survives;
- which physical intermediate states may exist while a logical transition is being persisted;
- which post-crash states are legal;
- how a recovery procedure maps an interrupted persistent state back into a legal logical state;
- how transaction atomicity, isolation, serialization, visibility, durability, acknowledgement, and recovery differ;
- how memory ordering and persistence ordering differ;
- how write-ahead logging, undo logging, redo logging, copy-on-write, shadow paging, persistent-memory transactions, checkpoints, and log-structured storage can realize the same higher-level contract;
- how snapshots, checkpoints, backups, generations, roots, journals, and canonical histories differ;
- how persistent references remain valid or become stale after rollback, replay, root replacement, or generation change;
- how crash testing differs from a proof of crash safety;
- how a compiler transformation can preserve ordinary functional behavior while breaking persistence behavior;
- how Forge may search storage realizations without weakening required crash/durability relations; and
- how local persistent-state semantics hand off to RFC 0028 without pretending that a local storage transaction atomically includes arbitrary remote effects.

This RFC establishes the semantic vocabulary and constitutional constraints for those questions. It deliberately does **not** make PostgreSQL, ARIES, SQLite, ZFS, PMDK, transactional memory, a particular file system, a particular database, or any specific persistent-memory instruction set the MNCS storage ontology. Those systems and theories are treated as evidence that several distinct persistence semantics and realization families are useful.

The central principle is:

> **Logical state, visible state, serialized state, committed state, acknowledged state, durable state, physically persisted state, snapshot state, crash state, and recovered state are distinct semantic concepts.**

A second constitutional principle is:

> **Durability is always relative to an explicit failure model and persistence domain. There is no universal `durable = true`.**

A third is:

> **Concurrent memory order and persistence order are distinct. A transition may be visible before it is durable, and source write order does not by itself establish persistence order.**

A fourth is:

> **Recovery correctness is part of program semantics. A system is not crash-safe merely because its normal execution is correct.**

And a fifth is:

> **The language specifies which logical histories may survive which failures. The implementation may choose the journal, snapshot, copy-on-write, flush, checkpoint, root-switch, transaction, and recovery realization only inside that declared crash-consistency envelope.**

These distinctions are required if MNCS is to permit machine-generated storage realizations while preserving explicit failure semantics, auditability, and independently checkable recovery evidence.

---

## 1. Problem statement

Conventional software frequently compresses many persistence questions into one overloaded word:

```text
commit()
```

Depending on the system, that may mean any mixture of:

```text
transaction logically accepted
serialization order selected
changes visible to readers
journal record emitted
journal record flushed
main data written
persistence barrier completed
client reply sent
checkpoint advanced
recovery can now retain the transaction
```

Those are not the same event.

Likewise:

```text
persistent = true
```

may ambiguously mean:

```text
old logical versions remain addressable
state was copied to a file
state crossed the kernel page cache
state crossed a storage-controller cache
state reached a power-failure-protected domain
state survives process failure
state survives OS failure
state survives power loss
state survives device loss
state exists in a remote replica
```

Such ambiguity is unacceptable in a machine-native language because generated transformations may preserve one interpretation while silently invalidating another.

MNCS therefore needs explicit semantic objects for:

```text
logical state
logical transition
transaction intent
transaction attempt
isolation contract
serialization relation
visibility
failure model
persistence domain
persistence ordering
failure atomicity
durability
acknowledgement
journal state
snapshot state
checkpoint state
crash state
recovery procedure
recovery result
persistent generation
persistent root
persistent-reference validity
```

The language must be able to express a requirement such as:

```text
UpdateIndex:
    isolation = Serializable

    failure_atomicity:
        after PowerFailure recovery may expose
        old_state OR new_state
        but no torn intermediate state

    durability:
        acknowledged update survives PowerFailure

    snapshots:
        previous generation retained

    recovery:
        latest acknowledged transaction

    physical strategy:
        unresolved
```

and then allow Forge to compare:

```text
redo WAL
undo journal
copy-on-write root switch
shadow paging
persistent-memory transaction
hybrid realization
```

without treating the implementation mechanism as the meaning of the program.

---

## 2. Non-goals

RFC 0026 does not:

- define cross-node replication, quorum semantics, consensus, network partition behavior, or distributed transactions; those belong to RFC 0028;
- define final serialization/wire/schema formats; those belong to RFC 0027;
- define all storage-device commands, cache-flush instructions, file-system APIs, or database interfaces;
- require a write-ahead log;
- require copy-on-write;
- require byte-addressable persistent memory;
- require ACID as one indivisible language primitive;
- define application invariants as an opaque database `consistency` bit;
- equate MVCC with any single isolation level;
- equate snapshot isolation with serializability;
- equate a snapshot with a checkpoint or backup;
- equate a journal with an event log or audit log;
- equate crash consistency with corruption resistance;
- guarantee physical erasure merely because logical deletion is durable;
- make bounded crash testing equivalent to a universal proof;
- define detailed storage cost, write-amplification, latency, wear, or recovery-time optimization; those belong primarily to RFC 0032/0033;
- define deployment/live-upgrade state migration; that belongs to RFC 0030;
- define software-supply-chain artifact retention; that belongs to RFC 0031;
- extend local durability atomically across arbitrary RFC 0008 external effects; or
- collapse every storage implementation into one universal `commit` event.

---

## 3. Constitutional distinctions

MNCS MUST preserve the following distinctions unless an explicit relation proves otherwise:

```text
LogicalState
    != VolatileRealization
    != PersistentRealization
    != Snapshot
    != Checkpoint
    != Backup
    != RecoveryJournal

VersionPersistence
    != DurablePersistence

MemoryOrder
    != PersistOrder

Visible
    != Durable

Serialized
    != Durable

Acknowledged
    != Durable

ConcurrencyAtomicity
    != FailureAtomicity

Atomicity
    != Isolation
    != Durability

StableSnapshot
    != SerializableHistory

Snapshot
    != Checkpoint
    != Backup

Journal
    != EventHistory
    != AuditHistory

LogicalDeletion
    != PhysicalErasure

RuntimeAddress
    != PersistentReference

FunctionalEquivalence
    != CrashEquivalence

RepresentationEquivalence
    != PersistenceEquivalence

LocalTransactionCommit
    != ExternalEffectCompletion
```

The compiler MUST NOT infer equality between these concepts merely because a particular backend currently implements them at the same point.

---

## 4. Seven semantic planes

RFC 0026 organizes persistent-state semantics into seven planes.

### 4.1 Logical-state plane

Owns:

```text
State
StateVersion
StateTransition
Invariant
TransactionIntent
```

### 4.2 Transaction plane

Owns:

```text
Transaction
TransactionAttempt
ReadSet
WriteSet
IsolationContract
SerializationClaim
VisibilityPoint
Abort
Savepoint
```

### 4.3 Persistence plane

Owns:

```text
FailureModel
PersistenceDomain
PersistOrderConstraint
DurabilityPoint
DurabilityClaim
FailureAtomicityClaim
AcknowledgementPoint
```

### 4.4 Journal/snapshot plane

Owns:

```text
RecoveryJournal
JournalRecord
JournalPosition
Checkpoint
Snapshot
SnapshotDerivation
PersistentRoot
Generation
```

### 4.5 Crash/recovery plane

Owns:

```text
CrashState
CrashContract
RecoveryProcedure
RecoveryDependency
RecoveryResult
CrashCounterexample
```

### 4.6 Realization plane

May include:

```text
WAL
undo journal
redo journal
ARIES-style recovery
copy-on-write
shadow paging
persistent-memory transaction
log-structured storage
hybrid strategies
```

### 4.7 Assurance plane

Owns evidence references such as:

```text
CrashProof
PersistOrderCertificate
IsolationCertificate
BoundedCrashExploration
RecoveryTrace
TargetApplicabilityEvidence
```

RFC 0018 remains responsible for deciding which evidence is sufficient for an action.

---

## 5. Logical state

A logical state is semantic information.

```text
State {
    id
    schema_or_type
    invariants
}
```

It is not identical to:

```text
memory bytes
file bytes
database pages
log records
root blocks
NVM cache lines
```

Those are candidate representations or storage realizations.

RFC 0019's value/representation separation therefore applies directly to persistent state.

---

## 6. State versions

Persistent systems frequently retain several logical versions.

MNCS SHOULD represent version identity explicitly:

```text
StateVersion {
    id
    state_identity
    predecessor_versions
    transition_ref
    generation?
}
```

Version identity may be:

- generative;
- content-derived;
- log-position-derived;
- root-derived;
- transaction-derived; or
- another versioned realization.

No one identity strategy is selected by this RFC.

---

## 7. Version persistence is not durability

A version-persistent data structure retains old logical versions after updates.

That property can hold entirely in volatile memory.

MNCS therefore distinguishes:

```text
VersionPersistenceClaim
```

from:

```text
DurabilityClaim
```

A functional persistent tree in DRAM may satisfy the first and fail every power-loss durability claim.

---

## 8. State transitions

Logical update intent SHOULD be represented independently of physical writes:

```text
StateTransition {
    id
    source_state
    target_state
    operation
    invariant_obligations
}
```

Example:

```text
Transfer(A, B, 50)
```

may physically require:

```text
journal append
account-page writes
index updates
checksums
allocation metadata
root update
```

The semantic transition identity should trace through all such realizations.

---

## 9. Application invariants

Transactions do not manufacture application consistency.

Application invariants remain ordinary semantic properties:

```text
Invariant {
    subject
    predicate
}
```

A transaction mechanism may establish atomicity/isolation/durability while a verifier proves:

```text
InvariantPreserved(transition)
```

MNCS MUST NOT define one opaque `database_consistent = true` bit as a substitute.

---

## 10. Volatile realization

A logical state can have a volatile realization:

```text
VolatileRealization {
    state_version
    runtime_region
}
```

Visibility to threads does not establish persistence.

This is the first place where RFC 0010 and RFC 0026 deliberately diverge.

---

## 11. Persistent realization

A persistent realization binds logical state to a persistence mechanism:

```text
PersistentRealization {
    state_version
    storage_identity
    persistence_domain
    representation
}
```

Its correctness may require:

- representation validity;
- persistent-reference validity;
- persist ordering;
- crash invariants;
- recovery metadata; and
- target-specific evidence.

---

## 12. Failure models

Every durability or crash-safety claim MUST name a failure model.

A minimal vocabulary may include:

```text
ProcessFailure
RuntimeFailure
OSFailure
PowerFailure
TornWriteFailure
VolatileCacheLoss
DeviceFailure
MediaCorruption
AdministrativeDeletion
```

This list is extensible.

A claim valid for one failure model does not automatically widen to another.

---

## 13. Failure scopes

Failure models may contain scope information:

```text
FailureScope {
    failed_components
    preserved_components
    fault_assumptions
}
```

For example:

```text
PowerFailure {
    volatile_cpu_cache = lost
    power_protected_domain = preserved
}
```

The exact components are supplied by RFC 0017 runtime/target facts.

---

## 14. Persistence domains

A persistence domain describes a boundary across which state survives a specified failure set.

```text
PersistenceDomain {
    id
    survives
    loses
    target_binding
}
```

Potential realizations include:

```text
CPU cache
power-protected memory-controller domain
kernel page cache
storage controller cache
device media
local durable object
```

RFC 0028 will later extend this model to remote/replicated durability.

---

## 15. Durability claims

A durability claim MUST include both failure model and persistence domain.

```text
DurabilityClaim {
    subject
    failure_model
    persistence_domain
    validity_scope
    witness
    evidence
}
```

A naked:

```text
durable = true
```

is invalid canonical semantics.

---

## 16. Durability is not universality

The following can all be simultaneously true:

```text
DurableUnder<ProcessFailure>
DurableUnder<OSFailure>
DurableUnder<PowerFailure>
UNKNOWN under DeviceFailure
REFUTED under PoolDestruction
```

The language SHOULD preserve this multidimensionality.

---

## 17. Memory visibility

RFC 0010 controls when writes become observable to concurrent computation.

RFC 0026 consumes those semantics but does not redefine them.

A write can be:

```text
visible to another core
```

while remaining:

```text
not durable under PowerFailure
```

This state is legitimate when the contract allows it.

---

## 18. Persistence order

RFC 0026 introduces a distinct relation:

```text
PersistBefore(A, B)
```

meaning:

> A must become persistent under the selected persistence model before B is permitted to become persistent in a way that would make recovery depend on A.

This relation is not inferred from source sequencing.

---

## 19. Memory order is not persist order

Canonical semantics MUST preserve:

```text
MemoryOrder(A, B)
```

separately from:

```text
PersistOrder(A, B)
```

A target may provide mechanisms that make them coincide in a narrow case.

That coincidence is evidence, not language truth.

---

## 20. Persist-order constraints

A persist-order obligation may be represented as:

```text
PersistOrderConstraint {
    predecessor
    successor
    failure_model
    reason
}
```

Example:

```text
PersistBefore(payload, valid_bit)
```

The compiler should preserve this obligation through lowering.

---

## 21. Persist-order witnesses

Possible witness families include:

```text
TargetInstructionWitness
StorageProtocolWitness
JournalOrderingWitness
RootSwitchWitness
ProofCertificate
BoundedPersistenceExploration
```

The witness type is not the semantic property.

---

## 22. Flushes and fences

Operations such as:

```text
cache flush
persistence barrier
fsync
fdatasync
FUA
device flush
```

are realization mechanisms.

The language's semantic requirement is usually a persistence relation or durability boundary, not a hard-coded instruction.

RFC 0008 owns authority/effect semantics for invoking such mechanisms.

---

## 23. Durability points

A transaction or transition may designate a semantic durability point:

```text
DurabilityPoint {
    transition
    failure_model
    persistence_domain
}
```

After this point, the contract may require recovery to include the transition.

---

## 24. Visibility points

A visibility point means:

> other authorized computations may observe the new logical state according to the isolation model.

It does not imply durability.

```text
VisibilityPoint != DurabilityPoint
```

unless an explicit contract relates them.

---

## 25. Serialization points

A serialization point identifies where a transaction is ordered in a serializable history.

It need not coincide with:

```text
VisibilityPoint
DurabilityPoint
AcknowledgementPoint
```

The language must allow them to differ.

---

## 26. Acknowledgement points

An acknowledgement is an externally observable claim about transaction progress.

```text
AcknowledgementPoint {
    transaction_attempt
    asserted_properties
}
```

Examples:

```text
ACCEPTED
VISIBLE
DURABLE_UNDER_POWER_FAILURE
```

A generic `SUCCESS` response should be lowered into an explicit declared contract where persistence matters.

---

## 27. `AckImpliesDurability`

A useful requirement is:

```text
AckImpliesDurability {
    failure_model
    persistence_domain
}
```

If declared, no acknowledgement covered by the relation may occur before the specified durability obligation is established.

---

## 28. Asynchronous durability

MNCS also permits weaker contracts where:

```text
VisibilityPoint < AcknowledgementPoint < DurabilityPoint
```

or another order is allowed.

Such a realization is not incorrect merely because it can lose recently acknowledged work after power failure—unless the application declared a stronger contract.

The weakness must remain explicit.

---

## 29. The word `commit`

Canonical semantics SHOULD avoid an unqualified universal `Commit` primitive.

Where human syntax uses `commit`, elaboration should resolve it to a more explicit transaction profile containing relevant combinations of:

```text
serialization
visibility
durability
acknowledgement
recovery inclusion
```

Ambiguous commit meaning is a diagnostic-worthy condition.

---

## 30. Transaction identity

MNCS SHOULD distinguish:

```text
TransactionIntentIdentity
TransactionAttemptIdentity
TransactionOutcomeIdentity
```

A retry after failure may be another attempt at the same logical intent.

That does not make the attempts identical.

---

## 31. Transaction intents

```text
TransactionIntent {
    id
    operation
    target_state
    invariant_requirements
    isolation_requirement
    durability_requirement
}
```

Intent survives as provenance even when multiple execution attempts occur.

---

## 32. Transaction attempts

```text
TransactionAttempt {
    id
    intent
    execution_context
    start_state
    read_set?
    write_set?
}
```

Attempts may:

```text
commit
abort
crash
remain outcome-unknown to an observer
```

---

## 33. Transaction outcomes

A transaction observer may need:

```text
TransactionOutcome =
    COMMITTED
    ABORTED
    UNKNOWN
```

`UNKNOWN` is especially important when a transaction becomes durable but the observer loses the acknowledgement.

MNCS MUST NOT force such cases into `ABORTED` merely because no reply was observed.

---

## 34. ACID decomposition

MNCS does not use `ACID` as one opaque property.

Instead:

```text
Atomicity
ApplicationInvariantPreservation
Isolation
Durability
```

are independent claims.

Application-level consistency belongs to contracts/proofs rather than a magical transaction-engine guarantee.

---

## 35. Transaction atomicity

Transaction atomicity describes whether logical transaction effects occur together according to the transaction model.

Crash-free transactional atomicity alone does not establish failure atomicity.

---

## 36. Failure atomicity

A failure-atomic transition constrains legal post-failure states.

Example:

```text
FailureAtomicityClaim {
    transition: S1 -> S2
    failure_model: PowerFailure
    allowed_recovery_states: {S1, S2}
}
```

A torn mixture of `S1` and `S2` is excluded.

---

## 37. Concurrency atomicity is not failure atomicity

A data structure may appear atomic to concurrent threads yet recover into a torn persistent state after power failure.

Conversely, a persisted state transition may be failure-atomic while exposed to concurrent code under a weaker isolation relation.

MNCS treats these independently.

---

## 38. Isolation contracts

A transaction SHOULD carry an explicit isolation requirement where concurrent transactions interact.

A minimum vocabulary may include:

```text
ReadCommitted
SnapshotIsolation
Serializable
StrictSerializable
Opaque
```

The exact supported subset can grow experimentally.

---

## 39. Read committed

`ReadCommitted` permits a transaction to observe committed states according to the profile but does not require one stable snapshot for the whole transaction.

It is a weaker relation than serializability.

---

## 40. Snapshot isolation

`SnapshotIsolation` provides each transaction a stable snapshot plus write-conflict rules according to the selected model.

It does not automatically prove equivalence to some serial history.

---

## 41. Snapshot isolation is not serializability

This distinction is constitutional.

A system may satisfy:

```text
StableSnapshot
```

while admitting:

```text
WriteSkew
```

or another serialization anomaly.

Forge must not upgrade the claim based on implementation branding or MVCC usage.

---

## 42. Serializability

A serializable history is behaviorally related to some serial execution of the participating transactions under the declared observation model.

```text
SerializationClaim {
    history
    serial_order_witness?
    relation
}
```

RFC 0020 owns the general relation framework.

---

## 43. Strict serializability

Where real-time invocation/response order matters, a stronger relation may require the serialization order to respect externally observed real-time precedence.

RFC 0023 supplies temporal semantics.

This RFC reserves the distinction without requiring every storage system to provide it.

---

## 44. Opacity

Opacity strengthens transactional reasoning by requiring even transactions that later abort to observe a state consistent with the transactional specification.

This is useful where an aborted/speculative transaction can still:

```text
fault
leak information
loop
perform local effects
```

before abort becomes known.

---

## 45. MVCC

Multiversion Concurrency Control is a realization family.

It may support:

```text
stable snapshots
snapshot isolation
serializable execution with additional validation
```

or another contract.

`MVCC` is not a semantic isolation level.

---

## 46. Locking realizations

Lock-based realizations such as strict two-phase locking are also implementation strategies.

The language specifies the isolation relation, not mandatory lock algorithms.

---

## 47. Optimistic realizations

Optimistic concurrency with commit-time validation is another realization family.

Its acceptance depends on whether the resulting history satisfies the requested relation.

---

## 48. Durable linearizability

For concurrent persistent objects, MNCS SHOULD reserve a relation in which:

- operations satisfy linearizable concurrent semantics; and
- completed operations required by the durability contract remain represented after crash/recovery.

```text
DurablyLinearizable
```

is stronger than ordinary linearizability.

---

## 49. Buffered durable linearizability

A weaker profile may allow recovered state to be a consistent but slightly older prefix than the latest completed operations.

```text
BufferedDurablyLinearizable
```

must remain distinct from:

```text
DurablyLinearizable
```

and from ordinary crash consistency.

---

## 50. Latest-state recovery is not universal

Some legal durability contracts require:

```text
RecoveredLatestAcknowledged
```

Others permit:

```text
RecoveredConsistentPrefix
```

The language must express which one applies.

---

## 51. Durable history requirements

A contract may constrain which acknowledged/serialized history prefix must survive:

```text
DurableHistoryRequirement {
    history_scope
    required_prefix_rule
    failure_model
}
```

Detailed quantitative data-loss windows belong primarily to RFC 0032.

---

## 52. Recovery journals

A recovery journal is storage used to justify/reconstruct legal persistent state after failure.

```text
RecoveryJournal {
    id
    ordering
    record_schema
    retention
}
```

It is not automatically the canonical logical event history or an audit log.

---

## 53. Journal records

```text
JournalRecord {
    id
    journal
    position
    transaction_ref?
    recovery_semantics
}
```

Recovery semantics may be:

```text
undo
redo
logical operation
physical write
compensation
checkpoint metadata
```

---

## 54. Journal positions

Journal positions SHOULD normally be logical sequence identities, not physical timestamps.

```text
JournalPosition
```

may be monotonic within a journal even when RFC 0023 wall-clock information is unavailable or uncertain.

---

## 55. Journal position is not time

```text
JournalPosition(500)
```

and:

```text
PhysicalInstant(T)
```

are distinct.

A mapping may be recorded for observability, but ordering in the journal does not require synchronized clocks.

---

## 56. Write-ahead logging

A WAL-style realization establishes a dependency such as:

```text
PersistBefore(recovery_record, dependent_data_state)
```

where recovery would be impossible or incorrect if the data reached the persistence domain first.

The semantic rule is the ordering obligation, not the name `WAL`.

---

## 57. WAL commit records

A WAL realization may use a commit marker or equivalent record that establishes transaction inclusion in recoverable history.

Its exact meaning must be explicit:

```text
JournalCommitRecord
```

does not automatically mean main data pages are checkpointed.

---

## 58. Redo logging

A redo strategy records sufficient information to reproduce durable committed changes after failure.

Recovery applies valid committed redo records to reconstruct target state.

Redo capability is a realization property.

---

## 59. Undo logging

An undo strategy records sufficient information to remove incomplete changes that may have reached persistent storage.

Its critical persist ordering differs from redo-only approaches.

The language should permit either when the high-level crash contract is satisfied.

---

## 60. Undo + redo

A combined strategy may support flexible buffer/write policies and partial rollback.

Again, this is realization machinery rather than the transaction ontology.

---

## 61. ARIES-style recovery

RFC 0026 SHOULD reserve an ARIES-inspired realization family containing concepts such as:

```text
LogSequence
AnalysisPhase
RedoPhase
UndoPhase
CompensationRecord
TransactionTable
DirtyStateTracking
CheckpointRecord
```

The RFC does not require exact ARIES algorithms.

The machine-native lesson is that recovery can be an explicit algorithm over versioned, ordered recovery evidence rather than a best-effort reboot side effect.

---

## 62. Compensation records

Recovery actions that themselves become durable may need explicit compensation/recovery records so repeated recovery remains well-defined.

```text
CompensationRecord
```

is distinct from an application compensation effect under RFC 0011.

---

## 63. Recovery itself may crash

A recovery procedure MUST be modeled as failure-prone computation unless a stronger environment assumption explicitly removes that failure class.

The model must therefore permit:

```text
crash
recovery starts
crash during recovery
recovery restarts
```

without abandoning the crash contract.

---

## 64. Recovery idempotence

One useful recovery relation is:

```text
RecoveryIdempotent
```

where applying recovery again does not produce a new semantic state under the declared relation.

This is useful but not the only way to make repeated recovery safe.

---

## 65. Recovery convergence

A more general contract may require:

```text
RecoveryConvergesToValidState
```

even if each recovery pass performs additional durable progress.

This can accommodate multi-stage recovery.

---

## 66. Crash contracts

MNCS SHOULD introduce a proof-relevant crash contract:

```text
CrashContract {
    subject
    precondition
    normal_postcondition
    crash_condition
    recovery_procedure
    recovery_postcondition
    failure_model
    assumptions
    witness
    evidence
}
```

This generalizes the intuition of Crash Hoare Logic into the MNCS artifact model.

---

## 67. Crash condition

A crash condition describes persistent states that may legitimately exist if execution is interrupted.

For a failure-atomic transition:

```text
crash_condition:
    old_state
    OR recoverable_journal_state
    OR new_state
```

but not arbitrary byte-level mixtures.

---

## 68. Normal postcondition and recovery postcondition

These MUST remain distinct.

Normal execution may guarantee:

```text
state = S2
```

while recovery after interruption may guarantee:

```text
state ∈ {S1, S2}
```

or another declared relation.

---

## 69. Crash Hoare Logic lineage

Crash Hoare Logic is a major theoretical inspiration because it adds crash conditions and recovery reasoning to ordinary pre/postcondition reasoning.

MNCS SHOULD preserve the same conceptual distinction even if the proof language differs.

Proof search remains outside the trusted kernel where practical; checked proof terms/certificates may integrate with RFC 0007.

---

## 70. Concurrent crash invariants

Concurrent storage needs invariants that survive both:

```text
interleaving
```

and:

```text
crash/recovery
```

RFC 0026 SHOULD permit proof systems in the Perennial/Iris family to establish such claims without making any one logic mandatory.

---

## 71. Reusable crash-safe abstraction boundaries

A verified journal layer may export a contract such as:

```text
AtomicDurableBatch
```

Higher layers should be able to depend on that claim rather than re-prove every physical write ordering.

This is essential for compositional storage reasoning.

---

## 72. Recovery dependency graphs

Recovery operations may depend on one another:

```text
Recover(B)
    before
Recover(A)
```

MNCS MAY represent:

```text
RecoveryDependencyGraph
```

so invalidation, localization, and recovery planning reuse the semantic graph.

---

## 73. Recovery results

```text
RecoveryResult {
    id
    crash_state
    base_state
    journal_range
    procedure
    recovered_state
    replayed_records
    undone_records
    unresolved_damage
    evidence
}
```

The result is a first-class artifact, not merely `restart succeeded`.

---

## 74. Partial recovery

A system may recover only part of the declared state.

Possible dispositions include:

```text
RECOVERED
RECOVERED_TO_PREFIX
RECOVERED_WITH_DECLARED_LOSS
FAILED
UNRESOLVED
```

RFC 0011 owns the generic failure outcome framework.

---

## 75. Recovery relations

A successful recovery SHOULD establish an RFC 0020 relation, for example:

```text
RecoveredStateRefinesAllowedCrashHistory
```

or:

```text
RecoveryEquivalentToCommittedPrefix
```

A generic boolean `recovery_ok` loses too much meaning.

---

## 76. Durable prefix property

Many log-based contracts can be expressed as:

```text
RecoveredHistory = committed_history[0..k]
```

for some legal prefix `k`.

The system must reject states where later dependent transactions appear while required earlier history is missing, unless the declared history relation permits such reordering.

---

## 77. Crash consistency does not imply freshness

A recovered state may be structurally and transactionally valid yet older than the latest visible or acknowledged state under a weak asynchronous durability profile.

MNCS therefore separates:

```text
CrashConsistent
```

from:

```text
LatestAcknowledgedRecovered
```

---

## 78. Snapshot semantics

A snapshot is an identified logical state view retained independently from the current mutable view.

```text
Snapshot {
    id
    state_version
    consistency_scope
    creation_relation
    retention_policy
}
```

It is not defined by one physical copying mechanism.

---

## 79. Snapshot consistency classes

Useful scopes may include:

```text
CrashConsistentSnapshot
TransactionConsistentSnapshot
ApplicationConsistentSnapshot
```

The exact vocabulary is extensible.

A file-system snapshot does not automatically establish application consistency.

---

## 80. Copy-on-write snapshots

Copy-on-write trees are a major snapshot/durability realization family.

A typical high-level strategy is:

```text
retain old root
write new blocks elsewhere
persist new consistent graph
atomically select new root
```

The snapshot semantics remain logical even when the physical implementation shares most blocks.

---

## 81. Shadow paging

Shadow paging is another realization family in which modifications occur in alternate pages and a root or page-table switch selects the new version.

Its correctness can be expressed through root-selection and persist-order claims.

---

## 82. Persistent roots

MNCS SHOULD reserve:

```text
PersistentRoot {
    id
    state_version
    generation
    storage_ref
}
```

A root is a durable selection mechanism for a logical state graph.

---

## 83. Root-switch atomicity

A copy-on-write or shadow realization may reduce a complex update to:

```text
Persist(new_graph)
PersistBefore(new_graph, root_switch)
FailureAtomic(root_switch)
```

This pattern should be expressible without exposing the exact storage layout in logical source.

---

## 84. Generations

A generation is a logical durable-state marker:

```text
Generation {
    id
    predecessor?
}
```

It is useful for:

```text
snapshot identity
persistent reference validation
recovery ordering
stale-state detection
```

Generation order is not wall-clock time.

---

## 85. Checkpoints

A checkpoint is primarily a recovery optimization boundary.

```text
Checkpoint {
    id
    state_or_root_ref
    journal_position
    recovery_assumptions
}
```

It may reduce the journal suffix required for recovery.

---

## 86. Checkpoint is not snapshot

A checkpoint need not preserve a user-addressable historical state.

A snapshot need not serve as the engine's recovery checkpoint.

The two may share representation but remain semantically distinct.

---

## 87. Snapshot is not backup

A snapshot may share the same:

```text
device
controller
pool
administrative account
physical failure domain
```

as live state.

Therefore snapshot existence does not establish independent backup durability.

Cross-failure-domain backup/replication semantics hand off to RFC 0028.

---

## 88. Snapshot derivation

A snapshot SHOULD preserve provenance:

```text
SnapshotDerivation {
    source_state
    transaction_prefix
    journal_position?
    creation_method
}
```

This makes point-in-time reasoning auditable.

---

## 89. Point-in-time recovery

A base snapshot plus an accepted journal prefix may derive a historical state:

```text
RecoveredAt {
    base_snapshot
    journal_prefix
    recovery_target
}
```

The resulting state's provenance should identify both sources.

---

## 90. Log compaction

Recovery history may be compacted:

```text
journal prefix H
    -> snapshot S
```

provided a declared relation such as:

```text
RecoveryEquivalent(H, S)
```

is established.

This does not imply audit-history equivalence.

---

## 91. Recovery log is not audit log

A recovery journal may be truncated after checkpointing.

An audit policy may require records to remain.

Thus:

```text
RecoveryRetention
```

and:

```text
AuditRetention
```

are different obligations.

---

## 92. Recovery log is not event sourcing

In event-sourced systems, the event log may be the canonical logical history.

In WAL systems, the log may be merely a recovery representation.

MNCS must not infer one from the other.

---

## 93. Physical versus logical journal records

MNCS SHOULD distinguish:

```text
PhysicalRecoveryRecord
LogicalRecoveryRecord
HybridRecoveryRecord
```

where the choice affects compatibility, replay, migration, and proof obligations.

RFC 0027 later owns the actual encoding.

---

## 94. Log-structured storage

A complete log-structured storage layout is another physical realization family.

It must not be confused with merely maintaining a recovery journal alongside another storage format.

---

## 95. Persistent-memory transactions

Byte-addressable persistent memory permits realizations where ordinary loads/stores participate in durability protocols.

MNCS treats techniques such as:

```text
undo logging
redo logging
transactional libraries
cache-line flushes
persistence barriers
```

as target-specific realizations of higher-level persist-order and failure-atomicity contracts.

---

## 96. Power-fail atomicity

Persistent-memory realizations frequently need a property such as:

```text
PowerFailAtomic(update)
```

This remains distinct from concurrent atomicity.

---

## 97. Persist-before example

Logical update:

```text
record.payload = P
record.valid = true
```

Crash safety may require:

```text
PersistBefore(payload, valid)
```

Candidate A:

```text
persist valid
persist payload
```

may recover as:

```text
valid = true
old payload
```

and must be rejected.

Candidate B:

```text
persist payload
persistence barrier
persist valid
```

may satisfy the contract on an applicable target.

The exact machine instructions remain realization-specific.

---

## 98. Persistent references

RFC 0009 owns reference semantics generally.

RFC 0026 adds a persistent validity dimension:

```text
PersistentReference {
    target_identity
    generation
    representation
}
```

A reference valid before rollback may become stale afterward.

---

## 99. Runtime pointers are not persistent references

A process virtual address is not automatically meaningful after restart.

The language SHOULD require an explicit persistent-reference realization for data intended to survive process identity/address-space changes.

---

## 100. Generation validation

Recovery may invalidate references into discarded generations.

```text
ReferenceValidAfterRecovery(ref, recovered_generation)
```

is therefore an explicit claim.

---

## 101. Persistent object identity

Persistent object semantic identity may survive physical relocation.

```text
ObjectIdentity
    != PersistentAddress
```

This extends RFC 0009/RFC 0020 identity distinctions into durable state.

---

## 102. Reclamation

Old state may become logically unnecessary for recovery.

That does not automatically authorize physical deletion.

```text
RecoveryReclaimable
```

must remain distinct from:

```text
PhysicallyErasable
```

because retention policies, snapshots, audit obligations, or other references may still apply.

---

## 103. Logical deletion

A durable logical deletion establishes that the current logical state no longer contains an object under the declared view.

It does not prove old bytes are erased from:

```text
journal
snapshot
copy-on-write block
backup
free-space residue
```

---

## 104. Erasure

Physical/logical erasure requires stronger semantics than ordinary deletion.

RFC 0026 reserves concepts such as:

```text
ErasureRequirement
ErasureBlocker
ErasureEvidence
```

while RFC 0024 owns the confidentiality policy motivating erasure.

---

## 105. Snapshot-retention conflicts

If a current-state deletion conflicts with a retained snapshot, the system should expose a condition such as:

```text
ErasureBlockedBySnapshot
```

rather than claim universal deletion success.

---

## 106. Checksums

Checksums can establish representation-integrity evidence.

They do not prove:

```text
application invariants
transaction serializability
semantic correctness
```

A valid checksum over the wrong logical state remains the wrong logical state.

---

## 107. Crash consistency versus corruption resistance

A crash-safe data structure may still be corrupted by:

```text
media error
bit rot
malicious write
controller fault outside the model
```

Thus:

```text
CrashConsistent
```

and:

```text
CorruptionResistant
```

are independent claims.

---

## 108. Local transactions and external effects

A local transaction cannot silently include arbitrary external RFC 0008 effects in the same failure-atomic domain.

Example:

```text
transaction {
    update durable state
    send network message
}
```

There exists a crash boundary between the two unless a stronger distributed protocol explicitly closes it.

RFC 0028 owns that distributed problem.

---

## 109. Durable intent

A local system may instead persist:

```text
Intent {
    perform external action X
}
```

before executing the action.

After recovery, the durable intent can drive retry/reconciliation.

But:

```text
IntentDurable
    != ExternalActionOccurred
```

must remain explicit.

---

## 110. Retry identities

Retries SHOULD preserve:

```text
logical intent identity
```

while generating new:

```text
execution attempt identities
```

This enables deduplication/reconciliation without pretending repeated executions are identical.

---

## 111. Idempotence

MNCS MAY express:

```text
IdempotentUnder<R>(operation)
```

where repeated application is equivalent under relation `R`.

Retry-safe is broader than idempotent and must not be treated as a synonym.

---

## 112. Savepoints

A savepoint is a local transaction rollback boundary:

```text
Savepoint {
    transaction_attempt
    logical_state
}
```

It does not automatically create a durable snapshot or external commit.

---

## 113. Nested transactions

Nested transaction semantics may include:

```text
closed nesting
open nesting
subtransactions
partial rollback
savepoint-only nesting
```

The RFC reserves the distinctions rather than assuming all nesting is equivalent.

---

## 114. Abort versus crash

A normal abort is an intentional transaction outcome.

A crash is an execution failure.

They may reuse rollback mechanisms but remain semantically different under RFC 0011.

---

## 115. Compiler preservation

Persistence obligations MUST survive HIR, SSA, backend lowering, optimization, and realization selection where the supported subset exposes persistence semantics.

A compiler must not remove or reorder persistence operations merely because crash-free functional behavior appears unchanged.

---

## 116. Functional equivalence is not crash equivalence

Two programs can return identical values on all crash-free executions while differing after interruption.

RFC 0020 relation examples:

```text
FunctionalEquivalent(A, B) = true
CrashEquivalent(A, B) = false
```

This should become a required research demonstration.

---

## 117. Representation equivalence is not persistence equivalence

A layout transformation may preserve logical value observations but change how many physical writes participate in one update.

For example:

```text
single-record representation
    -> split-record representation
```

may invalidate prior failure-atomicity reasoning.

RFC 0019 representation evidence must therefore not silently widen into crash-equivalence evidence.

---

## 118. Persist-order verifier

RFC 0026 strongly motivates a dedicated micro-verifier:

```text
mncs-language.persist-order
```

Inputs:

```text
required PersistBefore edges
target persistence model
lowered operations
barriers/flushes/journal dependencies
```

Outputs:

```text
PASS
FAIL
UNKNOWN
```

with a proof/certificate or counterexample where available.

---

## 119. Crash-schedule explorer

Another micro-verifier should explore bounded crash schedules:

```text
crash after operation i
crash after persisted subset S
crash during recovery step r
```

and report:

```text
BoundedCrashConsistencyEvidence
```

This is empirical/bounded evidence, not a universal theorem.

---

## 120. Persistence-order exploration

For weak or target-dependent persistence models, bounded analysis may enumerate permitted persisted subsets/orderings.

A counterexample might be:

```text
CrashCounterexample {
    crash_point
    persisted_subset
    persist_order
    recovered_state
    violated_invariant
}
```

This is highly suitable for Forge-guided repair.

---

## 121. Crash counterexamples

Counterexamples MUST remain scoped by:

```text
failure_model
persistence_domain
target
transaction profile
recovery procedure
```

A failure under power-loss persistence does not automatically refute process-crash safety.

---

## 122. Bounded evidence remains bounded

The following is illegal reasoning:

```text
crashed candidate at 1,000 points
all passed
therefore universally crash safe
```

The correct conclusion is:

```text
bounded crash corpus passed
```

RFC 0018 decides what that evidence authorizes.

---

## 123. Proof-relevant crash evidence

Strong claims may be supported by:

```text
CrashHoareProof
ConcurrentCrashProof
PersistOrderCertificate
ModelCheckCertificate
JournalCorrectnessProof
RootSwitchProof
IsolationProof
DurableLinearizabilityProof
```

The language does not require one proof technology.

---

## 124. Proof search versus checking

As elsewhere in MNCS:

```text
proof search
```

may be performed by:

```text
Forge
SMT solver
model checker
theorem prover
LLM-guided synthesis
specialized analyzer
```

while:

```text
proof/certificate checking
```

should remain smaller and independently implementable where feasible.

---

## 125. Target applicability

Persistence evidence is often target-sensitive.

A proof or certificate may depend on:

```text
CPU persistence model
cache architecture
storage controller guarantees
filesystem semantics
mount options
device flush behavior
power-protection assumptions
compiler/backend pipeline
```

RFC 0017 supplies these facts.

---

## 126. Evidence invalidation

If a relevant target fact changes, RFC 0018 should invalidate only affected persistence claims.

Example:

```text
storage controller no longer power-protected
```

may invalidate:

```text
DurableUnder<PowerFailure>
```

without invalidating:

```text
SerializableHistory
```

for the same implementation.

---

## 127. Persistence deltas

Candidate transformations SHOULD expose:

```text
PersistenceDelta {
    durability_claims_added
    durability_claims_removed
    failure_models_changed
    persist_order_changes
    recovery_relation_changes
    snapshot_changes
    acknowledgement_changes
}
```

These become first-class Forge comparison inputs.

---

## 128. Authority over persistent state

RFC 0025 supplies principal identity and authorization evidence.

RFC 0008 supplies effect/capability semantics.

RFC 0026 therefore treats persistent mutation as authority-bearing:

```text
PersistCapability<StateScope>
```

or equivalent effect authorization.

No transaction mechanism overrides authority policy.

---

## 129. Persistent state and information flow

RFC 0024 labels and observer policies remain attached to persisted information.

Persistence does not declassify data.

Journal/snapshot copies may inherit equal or stronger confidentiality requirements.

---

## 130. Journals can multiply information exposure

A value overwritten in current state may remain recoverable from:

```text
undo log
redo log
snapshot
copy-on-write generation
backup
```

RFC 0024 information-flow policy should therefore consider persistence-history channels explicitly.

---

## 131. Durable trust state

RFC 0025 objects such as:

```text
revocation state
principal-key binding
trust-domain configuration
```

may themselves require RFC 0026 durable transaction semantics.

A crash while rotating a root key, for example, must not create ambiguous half-updated trust state unless the contract explicitly permits it.

---

## 132. Durable policy state

Assurance and security policies may also be persistent state.

Mutation of such policy often deserves stronger:

```text
failure atomicity
audit retention
threshold authority
```

than ordinary cached application state.

The semantics support this without hard-coding one policy store.

---

## 133. Recovery and identity

A recovered process/runtime is not necessarily the same execution instance under RFC 0025.

Persistent state identity may survive while:

```text
ExecutionInstanceIdentity
```

changes.

This distinction matters for session credentials and replay protection.

---

## 134. Recovery and time

Recovery order SHOULD not depend on wall-clock ordering unless explicitly required.

Journal positions and generations provide logical ordering.

RFC 0023 timestamps may augment provenance but do not automatically define commit order.

---

## 135. Recovery and nondeterminism

If recovery has legal choices, the set of allowed recovered states must be declared.

Unspecified recovery nondeterminism is not a substitute for a crash contract.

RFC 0011 owns general nondeterministic outcome semantics.

---

## 136. Recovery and failure escalation

A recovery attempt may discover:

```text
corruption outside failure model
missing journal dependency
stale persistent reference
unsupported format
```

The resulting failure should be explicit and must not be silently converted to an arbitrary older state unless policy authorizes that fallback.

---

## 137. Serialization handoff to RFC 0027

RFC 0026 owns:

```text
which logical state must persist
which recovery relation must hold
```

RFC 0027 owns:

```text
how that state/journal/snapshot is encoded
canonical bytes
schema evolution
wire/storage encoding compatibility
```

A valid serialization does not by itself establish crash safety.

---

## 138. Distribution handoff to RFC 0028

RFC 0026 owns local durability.

RFC 0028 will own:

```text
replicated state
quorum durability
consensus commit
network partitions
cross-node failure
replicated logs
distributed transactions
```

A local durable record does not prove global durable agreement.

---

## 139. Lifecycle handoff to RFC 0030

RFC 0030 will use RFC 0026 state identities, snapshots, generations, crash contracts, and recovery semantics to define:

```text
live state migration
upgrade rollback
version coexistence
migration durability
```

RFC 0026 does not choose upgrade policies.

---

## 140. Quantitative handoff to RFC 0032

RFC 0032 will quantify:

```text
write amplification
journal growth
checkpoint cost
recovery work
latency
wear
memory/storage overhead
```

RFC 0026 defines semantic validity independent of those costs.

---

## 141. Optimization handoff to RFC 0033

RFC 0033 may select among semantically valid storage realizations using multi-objective criteria.

It must never treat a faster invalid crash realization as Pareto-preferred over a valid one.

Semantic constraints dominate optimization preferences.

---

## 142. Forge realization search

A persistent-state contract may leave realization open:

```text
PersistentStateContract {
    isolation: Serializable
    failure_atomicity: old | new
    durability: survive PowerFailure
    acknowledgement: only_after_durable
    snapshot: retain previous generation
    recovery: latest acknowledged

    journal_strategy: ?
    root_strategy: ?
    physical_representation: ?
}
```

Forge may search:

```text
redo WAL
undo journal
copy-on-write tree
shadow paging
persistent-memory transaction
hybrid design
```

without changing the logical contract.

---

## 143. Candidate validity before optimization

Forge MUST first determine whether a candidate satisfies required persistence relations.

Only semantically valid candidates proceed to performance comparison.

This ordering is mandatory.

---

## 144. Faster but weaker acknowledgement candidate

Candidate A:

```text
async WAL
ack before durable flush
```

Candidate B:

```text
sync WAL
ack after durable log record
```

If the contract requires:

```text
AckImpliesDurability<PowerFailure>
```

Candidate A is invalid even if substantially faster.

---

## 145. Durable but nonserializable candidate

Candidate C:

```text
snapshot isolation
strong power-loss durability
```

may still fail a:

```text
Serializable
```

requirement.

Durability does not compensate for an isolation failure.

---

## 146. Functional but crash-unsafe candidate

Two candidates may be equal under crash-free return-value observations while differing under crash injection.

Forge must preserve both results:

```text
FunctionalEquivalent = established
CrashEquivalent = refuted
```

No universal `equivalent` promotion is permitted.

---

## 147. First persistence micro-verifier suite

RFC 0026 motivates at least four verifier roles:

```text
persist-order verifier
bounded crash explorer
recovery-relation checker
transaction-history checker
```

They may be separate processes/languages/toolchains.

Their results remain method-scoped evidence.

---

## 148. First implementation artifact set

The first bounded implementation SHOULD include versioned artifacts for:

```text
StateVersion
StateTransition
TransactionIntent
TransactionAttempt
TransactionOutcome
IsolationContract
FailureModel
PersistenceDomain
PersistOrderConstraint
DurabilityPoint
AcknowledgementPoint
FailureAtomicityClaim
JournalRecord
JournalPosition
Snapshot
Checkpoint
PersistentRoot
Generation
CrashContract
RecoveryResult
CrashCounterexample
PersistenceDelta
```

Not every field needs final specification quality.

The objective is to validate the distinctions mechanically.

---

## 149. Fixture A — volatile versus durable

Initial state:

```text
S1
```

Execution:

```text
produce S2
make S2 visible
crash before durability point
```

Expected recovery under declared model:

```text
S1
```

The fixture demonstrates:

```text
Visible != Durable
```

---

## 150. Fixture B — durability before acknowledgement

Execution:

```text
persist S2
establish DurabilityPoint
emit acknowledgement
crash
```

Expected:

```text
S2 survives
```

under the selected failure model.

---

## 151. Fixture C — acknowledgement before durability

Execution:

```text
emit acknowledgement
crash before durability
```

Expected:

- legal only under a contract allowing acknowledgement loss; and
- rejected under `AckImpliesDurability`.

---

## 152. Fixture D — failure-atomic two-field update

State:

```text
Record {
    value
    generation
}
```

Crash injection occurs between all physical writes.

Recovery must produce:

```text
old complete record
OR
new complete record
```

never a mixed version.

---

## 153. Fixture E — WAL ordering

Candidate bad:

```text
data reaches persistence domain
before required recovery log
```

Expected:

```text
FAIL
```

Candidate good:

```text
log durable
before dependent data
```

Expected:

```text
PASS
```

for the bounded model.

---

## 154. Fixture F — snapshot isolation anomaly

Construct a write-skew-style fixture where:

```text
StableSnapshot = established
Serializable = refuted
```

The result must preserve both relations independently.

---

## 155. Fixture G — copy-on-write root

Retain:

```text
Root G1 -> S1
```

Construct/persist:

```text
S2
```

then select:

```text
Root G2 -> S2
```

Crash exploration should show old or new root according to the root-switch contract, never an invalid mixed graph.

---

## 156. Fixture H — checkpoint

Given:

```text
snapshot/base state + journal 1..100
```

create checkpoint at:

```text
journal position 80
```

Recovery should require only the valid suffix after the checkpoint while producing the same declared logical state relation.

---

## 157. Fixture I — crash during recovery

Inject a second crash while recovery is replaying records.

Expected:

```text
restart recovery
converge to legal state
```

or an explicit supported weaker recovery relation.

---

## 158. Fixture J — stale persistent reference

Reference:

```text
ref -> generation 7
```

Recovery selects:

```text
generation 6
```

Expected:

```text
reference rejected as stale/inapplicable
```

rather than silently dereferenced.

---

## 159. Fixture K — logical deletion versus snapshot

State contains secret `X`.

Current generation deletes `X`.

Older retained snapshot still contains `X`.

Expected:

```text
LogicalDeletion = established
PhysicalErasure = not established
ErasureBlockedBySnapshot = established
```

---

## 160. Fixture L — checksum versus semantic invariant

Persist a structurally valid record with a valid checksum but an application invariant violation.

Expected:

```text
ChecksumValid = established
InvariantPreserved = refuted
```

---

## 161. Fixture M — functional versus crash relation

Candidate A and Candidate B match across all crash-free bounded executions.

Crash point C produces a torn state only in B.

Expected:

```text
BoundedFunctionalAgreement = established
CrashRelation(B) = refuted
```

No universal equivalence claim.

---

## 162. First Forge experiment

The first cross-component Forge experiment SHOULD compare three implementations of one logical atomic map update:

```text
A. undo journal
B. redo WAL
C. copy-on-write root
```

Required semantics:

```text
isolation:
    Serializable

failure model:
    PowerFailure

failure atomicity:
    old map OR new map

acknowledgement:
    after durability

snapshot:
    previous generation retained
```

Forge should evaluate:

```text
crash relation
persist ordering
recovery relation
snapshot relation
target applicability
evidence quality
```

and MAY record non-authoritative measurements for:

```text
latency
write amplification
recovery work
```

The experiment MUST reject a candidate that is faster but violates any protected persistence relation.

---

## 163. Diagnostics

Diagnostics SHOULD be semantic and repair-oriented.

Examples:

```text
E2601 visibility established but required durability unresolved
E2602 acknowledgement precedes required durability point
E2603 durability claim omits failure model
E2604 durability claim omits persistence domain
E2605 source order does not establish required persist order
E2606 required recovery record may persist after dependent data
E2607 transaction satisfies snapshot isolation but not required serializability
E2608 failure-atomicity counterexample recovered torn state
E2609 checkpoint does not preserve declared recovery relation
E2610 snapshot does not establish backup independence
E2611 runtime pointer used as persistent reference
E2612 persistent reference targets stale generation
E2613 recovery may crash without declared crash-recovery continuation
E2614 bounded crash corpus passed; universal crash proof still unresolved
E2615 candidate preserves functional relation but violates crash relation
E2616 logical deletion does not satisfy requested erasure policy
E2617 external effect lies outside local transaction atomicity domain
E2618 credential/trust-state update can recover into inconsistent authority state
E2619 target persistence assumption is stale
E2620 journal compaction preserves recovery but not requested audit history
```

Diagnostics MUST distinguish semantic requirement failure from missing evidence and unsupported target mechanisms.

---

## 164. Security consequences

The model intentionally makes several dangerous shortcuts visible:

- a visible state is not automatically durable;
- a client acknowledgement can be invalid under the requested durability contract even when crash-free execution succeeds;
- a cryptographically protected journal can still encode an invalid logical state;
- a retained snapshot can prevent confidential erasure;
- a persistent trust-store update can be crash-unsafe even if each individual credential validates;
- a compiler may preserve ordinary values while breaking persist order;
- recovery code is part of the trusted state-transition surface; and
- a local transaction cannot manufacture distributed atomicity over network effects.

---

## 165. Scope boundaries

### RFC 0008

Owns persistent I/O, synchronization, flush, device, filesystem, and storage effects/capabilities.

### RFC 0009

Owns reference, storage identity, location, provenance, lifetime, and object relationships consumed by persistent-state semantics.

### RFC 0010

Owns concurrency, memory visibility, atomicity, causality, and memory ordering. RFC 0026 adds persistence ordering and crash relations without redefining volatile concurrency.

### RFC 0011

Owns failure kinds, retries, compensation, nondeterminism, and external failure observations.

### RFC 0012

Owns executable transition semantics on which persistent-state execution and recovery operate.

### RFC 0017

Owns runtime/target facts about cache persistence, devices, filesystems, storage controllers, power protection, and supported persistence mechanisms.

### RFC 0018

Owns evidence appraisal, freshness, conflict, trust, and action-relative assurance over persistence claims.

### RFC 0019

Owns logical values/types and representation realizations; persistence places additional obligations on durable representations.

### RFC 0020

Owns relation machinery for crash equivalence, recovery refinement, durable linearizability, snapshot equivalence, and bounded agreement.

### RFC 0023

Owns physical/logical time, deadlines, temporal validity, and timestamps attached to persistence evidence.

### RFC 0024

Owns confidentiality/integrity policy over current state, journals, snapshots, backups, retained versions, and erasure requirements.

### RFC 0025

Owns principal identity, authentication, delegation, trust and attestation for actors mutating persistent state.

### RFC 0027

Will own canonical storage/wire encodings, schema contracts, serialization validity, and data evolution.

### RFC 0028

Will own replicated state, consensus, partitions, remote durability, distributed logs, membership and distributed transactions.

### RFC 0030

Will own deployment/live-upgrade state migration and rollback using RFC 0026 snapshots/generations/recovery semantics.

### RFC 0031

Will own build/artifact persistence, lineage and supply-chain attestations.

### RFC 0032

Will own quantitative storage cost, latency, write amplification, wear, capacity and recovery-work models.

### RFC 0033

Will own multi-objective selection among semantically valid persistence realizations.

---

## 166. 1.0 research threshold implications

RFC 0026 should contribute the following minimum requirements to the MNCS 1.0 research threshold:

- a versioned persistent-state model distinguishing logical state/version, transaction intent/attempt/outcome, volatile realization, persistent realization, snapshot, checkpoint, journal, root, generation and recovered state;
- a versioned durability model requiring explicit failure models and persistence domains rather than a universal `durable` bit;
- explicit separation of memory visibility/order from persistence visibility/order;
- a versioned transaction/isolation model distinguishing at minimum failure atomicity, visibility, serializability, snapshot isolation, durability and acknowledgement;
- a versioned crash/recovery model with `CrashContract`, crash condition, recovery procedure, recovery result and relation-scoped counterexamples;
- at least one fixture proving `Visible != Durable`;
- at least one fixture where acknowledgement before durability is accepted under a weaker profile and rejected under `AckImpliesDurability`;
- at least one persist-order study where source/memory order is insufficient and an explicit persistence ordering witness is required;
- at least one failure-atomic update whose bounded crash exploration permits old/new state but rejects a torn mixture;
- at least one isolation study where snapshot isolation is established while serializability is refuted;
- at least one recovery study that survives a crash during recovery or otherwise proves repeated recovery convergence under the declared model;
- at least one snapshot/checkpoint study demonstrating they are distinct semantic objects;
- at least one persistent-reference generation study that rejects a stale reference after rollback/recovery;
- at least one RFC 0020 study where two implementations preserve a functional relation while one fails the required crash relation;
- at least one Forge study comparing WAL/journal/copy-on-write-style realizations and rejecting a faster candidate that violates a protected durability/crash relation;
- at least one bounded crash-testing result that remains explicitly bounded rather than being upgraded to universal crash safety;
- documented failure-model, persistence-domain, target, journal, checkpoint, recovery, snapshot, reference, and retention assumption limits; and
- documented handoff boundaries showing that local durability does not silently imply replicated/distributed durability.

---

## 167. Open research questions

The RFC intentionally leaves several questions open for experimentation:

- which persistence relations belong directly in the proof kernel versus specialized certificate checkers;
- how broad the initial failure-model vocabulary should be;
- whether persistence domains should be structural target facts or first-class capabilities;
- which isolation levels deserve canonical relation definitions versus library-defined profiles;
- how to combine serializability and crash-recovery relations compositionally;
- how much persist-order information belongs in HIR versus SSA/backend-specific artifacts;
- whether a native persistent-root abstraction should become part of the first executable semantic core;
- how journals, snapshots and persistent generations should interact with recursive semantic identities;
- how persistent pointers/handles should integrate with RFC 0009 provenance/lifetime models;
- how erasure obligations should propagate through snapshots, journals and future replicated copies;
- how recovery proofs should compose across abstraction layers;
- how bounded persistence-order exploration should generate minimal crash counterexamples;
- how exact persistent-memory target semantics should be represented across architectures;
- how state migration in RFC 0030 will preserve or deliberately transform crash contracts;
- how RFC 0028 distributed logs and consensus should reuse local durable-state artifacts;
- how persistent trust/credential state should use stronger crash profiles than ordinary application data;
- how much storage-history provenance should remain after compaction; and
- how Forge should estimate proof/evidence cost when choosing among semantically valid durability strategies.

None of these open questions weakens the constitutional distinctions established here.

---

## 168. Summary of constitutional rules

1. **Logical state is not physical storage.**
2. **Version persistence is not durability.**
3. **Visible is not durable.**
4. **Serialized is not durable.**
5. **Acknowledged is not durable unless the contract says so.**
6. **Durability is relative to a failure model.**
7. **Durability is relative to a persistence domain.**
8. **Memory order is not persist order.**
9. **Source write order does not establish persistence order.**
10. **Concurrency atomicity is not failure atomicity.**
11. **Atomicity is not isolation.**
12. **Atomicity is not durability.**
13. **Isolation is not durability.**
14. **Application consistency is not magically provided by a transaction engine.**
15. **Snapshot isolation is not serializability.**
16. **MVCC is a realization, not an isolation guarantee.**
17. **Failure-atomic state constrains legal crash states.**
18. **Crash consistency does not imply latest-state recovery.**
19. **Crash consistency does not imply zero acknowledged-data loss unless required.**
20. **Recovery correctness is part of program semantics.**
21. **Recovery may itself crash.**
22. **A recovery journal is not automatically an event log.**
23. **A recovery journal is not automatically an audit log.**
24. **A checkpoint is not a snapshot.**
25. **A snapshot is not a backup.**
26. **A snapshot consistency claim names the invariants it preserves.**
27. **A runtime pointer is not automatically a persistent reference.**
28. **Persistent references may be generation-scoped.**
29. **A checksum does not establish semantic correctness.**
30. **Crash consistency is not corruption resistance.**
31. **Logical deletion is not physical erasure.**
32. **Snapshot retention may block erasure.**
33. **Recovery reclaimability is not physical-erasure authority.**
34. **Functional equivalence does not imply crash equivalence.**
35. **Representation equivalence does not imply persistence equivalence.**
36. **A compiler may not erase persistence obligations because crash-free behavior is unchanged.**
37. **Local durable atomicity does not include arbitrary external effects.**
38. **Durable intent is not external-effect completion.**
39. **`UNKNOWN` transaction outcome is legitimate.**
40. **Bounded crash testing is evidence, not universal proof.**
41. **Persistence evidence is target-sensitive where the realization is target-sensitive.**
42. **A faster realization cannot weaken a protected crash/durability relation.**
43. **A stronger durability relation cannot compensate for a failed isolation relation.**
44. **A stronger isolation relation cannot compensate for a failed durability relation.**
45. **Canonical semantics should not use an ambiguous universal `commit` meaning.**
46. **Checkpoint/replay policies are realization choices unless explicitly observed.**
47. **Journal positions are logical sequence identities, not clocks.**
48. **Persistent roots/generations may survive relocation without preserving physical identity.**
49. **RFC 0018 decides what persistence evidence authorizes.**
50. **RFC 0028, not RFC 0026, owns replicated/distributed durability.**

---

## 169. Closing principle

RFC 0026 can be summarized as:

```text
Logical transition
    -> transaction/isolation contract
    -> persistence/failure contract
    -> candidate realization
    -> persist-order + crash/recovery evidence
    -> RFC 0018 appraisal
    -> action/deployment authority
```

The machine-native objective is:

> **Specify which logical histories may survive which failures, keep visibility, acknowledgement, persistence, snapshots and recovery explicitly distinct, and let machines search the storage realization space only inside that declared crash-consistency envelope.**

MNCS should therefore be able to replace hand-written folklore such as:

```text
write
fsync
hope
```

with explicit semantic claims such as:

```text
PersistBefore(log, data)
FailureAtomic(old | new)
AckImpliesDurability<PowerFailure>
RecoveredStateRefinesCommittedPrefix
```

whose evidence can be checked, invalidated, compared, and refined independently of whichever journal, filesystem, NVM, database, or future machine-generated storage structure currently realizes them.