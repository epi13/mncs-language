# RFC 0030: Machine-Native Deployment, Lifecycle, Live Upgrade, Version Coexistence, State Migration, Rollback, and Retirement Semantics

- **Status:** Draft
- **Authors:** MNCS project
- **Target:** pre-specification semantic research
- **Depends on:** RFC 0002, RFC 0004, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0014, RFC 0016, RFC 0017, RFC 0018, RFC 0019, RFC 0020, RFC 0022, RFC 0023, RFC 0024, RFC 0025, RFC 0026, RFC 0027, RFC 0028, RFC 0029
- **Leaves primarily to later RFCs:** build/derivation/reproducibility/supply-chain semantics to RFC 0031; quantitative lifecycle cost/risk/resource accounting to RFC 0032; multi-objective realization ranking and lifecycle-plan selection policy to RFC 0033

## 1. Summary

MNCS must not model deployment as replacing one version number with another.

A running system can contain multiple code versions, protocol versions, schema versions, configuration generations, feature states, deployment generations, persistent representations, execution instances, credentials, and placement realizations simultaneously. During upgrade, those versions can coexist for minutes, days, or indefinitely. The intermediate states are part of the observable program behavior.

This RFC therefore establishes a machine-native lifecycle semantic layer in which releases, artifact versions, deployment generations, execution populations, compatibility relations, update points, version-consistency contracts, state migrations, rollout phases, exposure scopes, activation authority, cutover, finalization, rollback, irreversibility, and retirement are explicit semantic objects.

The central idea is:

> A machine-native deployment is an evidenced transition through a space of mixed-version system states. Programs declare which intermediate states, coexistence relations, migrations, activations, rollback paths, and retirement conditions preserve their semantics; machines may synthesize the lifecycle strategy only inside that envelope.

Equivalently:

> The semantic object is not the new version. It is the path from the old world to the new one.

This RFC deliberately does **not** select rolling update, blue/green deployment, canarying, shadow execution, dynamic software updating, bridge releases, all-at-once replacement, or any other rollout mechanism as the language semantics. Those are realization families or evidence-producing strategies.

The RFC also refuses to let terms such as `deployed`, `ready`, `healthy`, `active`, `serving`, `finalized`, `rollbackable`, `compatible`, `retired`, or `safe upgrade` remain ambient Boolean labels. Each claim must identify its lifecycle state, relation, scope, authority, assumptions, and evidence.

## 2. Constitutional rules

The following rules are intended as architectural constraints, not implementation suggestions.

1. **Artifact version is not deployment generation.** Redeploying identical bits creates a new deployment generation unless policy explicitly identifies them.
2. **Release version is not complete system state.** A running system may contain many component, protocol, schema, configuration, and feature versions simultaneously.
3. **Artifact identity is not execution-instance identity.** One artifact may back many active instances.
4. **Installed is not loaded.** Presence on a resource does not establish runtime use.
5. **Loaded is not active.** Code may be loadable without governing behavior.
6. **Active is not serving.** Active code need not receive authoritative traffic.
7. **Serving is not authoritative.** Shadow or comparison instances may serve internally without controlling externally visible outcomes.
8. **Traffic cutover is not finalization.** All authoritative traffic may use a new version while rollback compatibility is intentionally retained.
9. **Finalization is not retirement.** An upgrade can be finalized while historical decoders, artifacts, or state compatibility remain necessary.
10. **Code deployment is not feature activation.** Behavior may remain gated after code becomes available.
11. **Feature deactivation is not rollback.** Disabling behavior does not undo state transitions or external effects already produced.
12. **Two valid endpoints do not imply a valid transition.** Every required intermediate lifecycle state is part of upgrade correctness.
13. **Upgrade correctness is trace-scoped.** Correctness may depend on the sequence of mixed-version states, not merely source and target configurations.
14. **Type-safe update is not complete semantic upgrade correctness.** Type/representation safety does not establish protocol, security, numeric, temporal, persistence, distributed, or business correctness.
15. **Code replacement is not state migration.** Executable behavior and persistent/runtime state transitions remain separate semantic dimensions.
16. **State migration is not lifecycle finalization.** A transformed state can remain dual-compatible or rollbackable.
17. **State migration may be eager, lazy, background, or virtual.** The strategy is not the semantic contract.
18. **Version coexistence is not version interoperability.** Two versions may run at the same time without communicating.
19. **Version interoperability is not shared-state compatibility.** Peers may speak one another's protocol yet disagree about durable state.
20. **Compatibility is directional.** `V1 -> V2` acceptance does not imply `V2 -> V1` acceptance.
21. **Compatibility is relation-specific.** Wire, state, request, transaction, session, authority, and semantic compatibility are distinct claims.
22. **Version number ordering is not semantic refinement.** `V2 > V1` does not establish that V2 refines, substitutes for, or remains compatible with V1.
23. **Upgrade direction does not grant compatibility.** Intended migration from A to B can still be unrealizable.
24. **Mixed-version validity must be established independently.** All-old and all-new populations being valid does not make mixed populations valid.
25. **Multi-version population is not mixed-version execution for one semantic unit.** Requests, transactions, sessions, actors, or continuations may be pinned to one version while the system population is heterogeneous.
26. **Request-, transaction-, and session-version consistency are distinct contracts.** One does not establish the others.
27. **Active old work can block retirement.** Zero new traffic to a version does not prove no old execution remains.
28. **An update point is a semantic condition, not a source-line annotation alone.** Updateability depends on active state and protected relations.
29. **Quiescence is one realization strategy.** Global quiescence is not universally required for live upgrade.
30. **Rollback is not the inverse of upgrade by default.** Reverse transitions require separate obligations and evidence.
31. **Code rollback is not state rollback.** Reinstalling old code does not restore old durable state.
32. **Rollback is not snapshot restoration.** RFC 0026 recovery and lifecycle downgrade are different operations.
33. **Rollback is not compensation.** External effects remain historical facts unless separately compensated.
34. **Roll-forward repair is not rollback.** A corrective successor may be the only valid recovery path after irreversibility.
35. **Rollback authority is phase-relative.** A transition can deliberately narrow or eliminate rollback after finalization.
36. **Finalization is a semantic boundary.** It marks retirement of assumptions required by an earlier lifecycle state.
37. **Irreversible transitions must be explicit.** Destruction of old state, keys, compatibility, or external-world conditions may not be hidden inside ordinary rollout steps.
38. **Canary success is bounded evidence.** It cannot be promoted to universal safety without an RFC 0018 policy decision.
39. **Canary evidence is exposure-scoped.** Cohort, workload, topology, duration, traffic class, and environment are part of evidence identity.
40. **Shadow output being non-authoritative does not imply shadow execution is effect-safe.** Effect authority must be separately constrained.
41. **Readiness is not semantic correctness.** A ready process can still violate program contracts.
42. **Health is not compatibility.** Two healthy versions can be incompatible with each other.
43. **Rollout progress is not service availability.** Either may hold while the other fails.
44. **Service availability is not upgrade completion.** Availability may be preserved while a rollout stalls.
45. **Deployment orchestration is failure-prone computation.** The lifecycle controller is not an unstated trusted immortal process.
46. **Lifecycle state must survive controller failure where recovery depends on it.** RFC 0026 durability applies to deployment state.
47. **Retried lifecycle steps require idempotency, deduplication, or another explicit retry-safety mechanism.**
48. **Unknown lifecycle outcome is legitimate.** A controller may lose knowledge of whether a remote rollout step occurred.
49. **Placement migration and software upgrade are separate transformations.** `V1@A -> V2@B` contains at least two semantic dimensions.
50. **State migration and placement migration are separate transformations.** Moving bytes is not changing their schema or logical meaning.
51. **Version coexistence can change distributed fault tolerance.** Transitional populations can narrow quorum or availability envelopes.
52. **Replica upgrade order may be constrained by physical failure domains.** RFC 0029 topology evidence can create lifecycle requirements.
53. **Cross-version dependencies generate upgrade-order constraints.** Order must be derived from compatibility relations, not folklore.
54. **Upgrade order should remain a partial order where possible.** Independent transitions need not be serialized artificially.
55. **Cyclic upgrade dependencies may require bridge versions.** A bridge release is a legitimate realization, not an error in the model.
56. **Old executable retirement does not imply old data can no longer require decoding.** Historical compatibility may outlive executable compatibility.
57. **Retirement is granular.** Executable, protocol, schema, state representation, feature, credential, and compatibility shims may retire at different times.
58. **Retirement requires absence-of-dependency evidence.** Lack of recent use is not proof that an artifact is no longer required.
59. **A rollback window is temporal policy, not rollback proof.** RFC 0023 time does not establish reverse compatibility.
60. **Promotion and finalization are distinct authorization decisions.** Full traffic promotion can remain reversible.
61. **Finalization authority may be stronger than canary or rollout authority.** Irreversibility justifies separate policy.
62. **Lifecycle planning and lifecycle checking are distinct.** Forge/AI/solvers may propose plans; narrow checkers decide whether required relations hold.
63. **The fastest rollout may be invalid.** Quantitative benefit never overrides a protected lifecycle relation.
64. **Bounded rollout testing is bounded evidence.** Finite canary, chaos, rollback, and mixed-version tests do not become universal proofs.
65. **Lifecycle evidence is dependency-sensitive.** Schema, protocol, target, topology, trust, or state changes invalidate only claims that depend on them.
66. **A system may remain indefinitely multi-version if its contracts allow it.** Final convergence to one version is not a universal semantic requirement.
67. **Finalization may remove compatibility intentionally.** Removal must be represented as a lifecycle transition, not silently inferred from elapsed time.
68. **A newer artifact cannot authorize itself.** Deployment, activation, traffic promotion, finalization, rollback, and retirement remain RFC 0025 authority decisions.
69. **Release metadata is not build provenance.** RFC 0031 owns artifact derivation and supply-chain evidence.
70. **Lifecycle admissibility precedes lifecycle optimization.** RFC 0030 determines which plans are valid; RFCs 0032–0033 may rank only valid plans.

## 3. Why lifecycle requires a distinct semantic layer

A source program describes a behavior. A release artifact packages one or more realizations of behavior. A deployment introduces those artifacts into a live environment. A lifecycle transition changes which versions, schemas, protocols, features, and instances are permitted to act authoritatively over time.

Conventional systems often flatten those layers into a version string and a deployment script. That is insufficient for MNCS.

Consider a service upgrade:

```text
Before:
    Service V1
    Protocol P1
    Schema S1

Target:
    Service V2
    Protocol P2
    Schema S2
```

Both endpoints can be independently valid. A naive rolling transition can still produce:

```text
Service V1 <-> Service V2
V1 sends P1
V2 accepts only P2
```

or:

```text
V2 writes S2
V1 remains authoritative
V1 cannot interpret S2
```

or:

```text
three replicas normally provide quorum
rollout takes two down concurrently
quorum no longer exists
```

or:

```text
V2 is fully serving
one V1 session is still executing
old code is deleted
session resumes into missing semantics
```

The correctness object is therefore not merely `V2`. It is the sequence of lifecycle configurations through which the running system passes.

This RFC gives that sequence semantic identity and checkable obligations.

## 4. Scope and ownership boundaries

### 4.1 RFC 0004 — recursive refinement

Owns generic observe-localize-propose-verify-compare-promote cycles and protected baselines. RFC 0030 applies those mechanisms to deployment/lifecycle candidates and rollout evidence.

### 4.2 RFC 0008 — effects and capabilities

Owns effects and capability authority. RFC 0030 uses it for deployment, feature activation, traffic switching, shadow-effect suppression, lifecycle state mutation, and retirement actions.

### 4.3 RFC 0011 — failure and recovery

Owns generic failure/recovery outcome taxonomy. RFC 0030 specializes it for lifecycle controllers and deployment-step outcomes.

### 4.4 RFC 0014 — linking/component compatibility

Owns module/component linking compatibility. RFC 0030 owns coexistence of independently running component versions over time.

### 4.5 RFC 0016 — self-transformation/metaprogramming

Owns staged/self-transforming program semantics. RFC 0030 owns operational replacement or coexistence of deployed running software versions.

### 4.6 RFC 0017 — runtime and environment

Owns runtime, platform, target, and environmental facts used to decide lifecycle applicability.

### 4.7 RFC 0018 — assurance

Owns support/refutation/conflict, trust, freshness, evidence appraisal, and action authorization for canarying, expansion, finalization, rollback, and retirement.

### 4.8 RFC 0020 — relations

Owns refinement/equivalence/compatibility relation machinery. RFC 0030 defines lifecycle-specific relation kinds and subjects.

### 4.9 RFC 0022 — progress/liveness/quiescence

Owns generic progress, liveness, fairness, quiescence, and bounded completion. RFC 0030 applies those to rollout progress and update points.

### 4.10 RFC 0023 — time

Owns rollout windows, observation periods, deadlines, rollback windows, leases, temporal validity, and clock uncertainty.

### 4.11 RFC 0024 — information flow

Owns confidentiality/integrity across mixed versions, migration, shadow traffic, feature activation, and newly introduced channels.

### 4.12 RFC 0025 — principal/authority/attestation

Owns who may deploy, activate, promote, finalize, downgrade, rotate, or retire; and which instances/platforms are trusted during transition.

### 4.13 RFC 0026 — persistent state

Owns crash-safe durable execution of state migrations, lifecycle journals, checkpoints, and recovery.

### 4.14 RFC 0027 — schema/wire evolution

Owns directional schema/protocol compatibility, typed migrations, unknown-field preservation, canonical encodings, and state representation relations.

### 4.15 RFC 0028 — distribution

Owns distributed membership, quorum, remote outcomes, consistency, replica coordination, failure detectors, and distributed transaction properties during mixed-version execution.

### 4.16 RFC 0029 — placement/mobility

Owns where old/new instances run, fault-domain placement, device applicability, and any spatial migration occurring during a lifecycle transition.

### 4.17 RFC 0030 — lifecycle

Owns lifecycle configurations, deployment generations, mixed-version populations, activation, rollout, update points, state-migration phase, cutover, finalization, rollback, and retirement.

### 4.18 RFC 0031 — build/supply chain

Will own how release/build artifacts are derived, reproduced, signed, traced, and attested through the software supply chain.

### 4.19 RFCs 0032–0033 — quantitative cost and selection

Will own latency, bandwidth, energy, blast-radius metrics, resource cost, rollout duration, optimization objectives, Pareto tradeoffs, and ranking among semantically valid lifecycle plans.

## 5. Lifecycle semantic planes

RFC 0030 organizes lifecycle into nine interacting planes.

### 5.1 Artifact/version plane

Owns:

```text
Release
VersionIdentity
ArtifactVersion
ArtifactSet
DeploymentGeneration
```

### 5.2 Instance/population plane

Owns:

```text
ExecutionInstance
VersionPopulation
RunningPopulation
ServingPopulation
AuthoritativePopulation
```

### 5.3 Compatibility plane

Owns:

```text
VersionCoexistenceContract
RuntimeCompatibilityMatrix
CompatibilityEnvelope
UpgradeDependencyGraph
BridgeVersion
```

### 5.4 Activation plane

Owns:

```text
Installed
Loaded
Eligible
Active
Serving
Authoritative
FeatureActivation
ExposureScope
TrafficAuthority
```

### 5.5 State-transition plane

Owns lifecycle orchestration around:

```text
StateMigrationBinding
MigrationPhase
DualRepresentationPhase
StateAuthority
StateFinalization
```

while RFCs 0026–0027 own crash-safe migration and data transformation semantics.

### 5.6 Version-consistency plane

Owns:

```text
UpdatePoint
QuiescenceCondition
RequestVersionConsistency
TransactionVersionConsistency
SessionVersionConsistency
ContinuationVersionBinding
```

### 5.7 Rollout plane

Owns abstract transition/exposure structure. Rolling, canary, shadow, blue/green, hot-update, and bridge-release patterns are realizations.

### 5.8 Finalization/retirement plane

Owns:

```text
RollbackWindow
FinalizationPoint
IrreversibleTransition
RetirementCondition
CompatibilityRetirement
```

### 5.9 Assurance plane

Owns lifecycle-domain artifacts consumed by RFC 0018:

```text
DeploymentWitness
UpgradeTraceClaim
CanaryEvidence
RollbackClaim
RetirementClaim
LifecycleCounterexample
LifecycleDelta
```

## 6. Version identity and deployment generation

### 6.1 Version strings are names

A textual version such as:

```text
2.7.4
```

is a useful release identifier but does not establish semantic compatibility, refinement, build identity, or deployed state.

A version identity should be able to reference:

```text
VersionIdentity {
    release_name
    artifact_set_identity
    semantic_interface_identity?
    protocol_contract_identity?
    state_contract_identity?
}
```

Artifact derivation remains RFC 0031.

### 6.2 Deployment generation

Deploying the same artifact twice creates separate deployment events:

```text
DeploymentGeneration {
    generation_id
    artifact_versions
    environment
    placement_snapshot
    configuration_identity
    created_by
    created_at
}
```

Two generations may have byte-identical artifacts while differing in environment, placement, credentials, topology, feature activation, or evidence.

### 6.3 Lifecycle configuration

A running system should be modeled as a vector rather than a scalar version:

```text
LifecycleConfiguration {
    deployment_generations
    instance_versions
    protocol_versions
    schema_versions
    configuration_versions
    feature_states
    authority_state
    migration_phase
    finalization_state
}
```

This is the principal subject of lifecycle transition claims.

## 7. Release as a transition package

A release should carry enough semantic structure to participate in lifecycle reasoning:

```text
Release {
    id
    artifact_set
    versions

    compatibility_contracts
    state_migrations
    feature_gates

    activation_conditions
    rollback_requirements
    finalization_conditions
    retirement_conditions
}
```

A release may omit some of those if no such transition is required.

The important rule is that release metadata may describe transition requirements; it does not prove them.

## 8. Lifecycle states

### 8.1 Artifact availability states

Suggested lifecycle vocabulary:

```text
UNAVAILABLE
AVAILABLE
INSTALLED
LOADED
ELIGIBLE
```

### 8.2 Behavioral states

```text
INACTIVE
ACTIVE
SERVING
AUTHORITATIVE
SHADOW
DRAINING
```

### 8.3 Transition states

```text
PREPARING
COEXISTING
MIGRATING
CUTOVER
FINALIZING
ROLLING_BACK
RETIRING
```

These are not necessarily one global enum. They may be orthogonal state dimensions.

### 8.4 Why orthogonal dimensions matter

A V2 instance may be:

```text
installed = true
active = true
serving = true
authoritative = false
shadow = true
```

while V1 remains authoritative.

Flattening this into `version_state = ACTIVE` loses the essential lifecycle relationship.

## 9. Lifecycle transitions

### 9.1 Core artifact

```text
LifecycleTransition {
    id
    from_configuration
    to_configuration

    preconditions
    allowed_intermediate_states
    forbidden_intermediate_states

    compatibility_requirements
    migration_bindings
    version_consistency_requirements
    availability_requirements
    authority_requirements

    rollback_paths
    finalization_conditions
    retirement_conditions

    protected_relations
    obligations
}
```

### 9.2 Endpoint validity is insufficient

Let:

```text
Valid(A)
Valid(B)
```

Both may hold while:

```text
TransitionValid(A -> B)
```

fails.

The transition relation is evaluated over the allowed trace/configuration space.

### 9.3 Transition traces

```text
LifecycleTrace {
    initial_configuration
    events
    intermediate_configurations
    final_configuration
}
```

A trace may be evaluated under RFC 0020 relations such as:

```text
UpgradeTraceRefines
PerRequestVersionConsistent
AvailabilityPreserved
AuthorityMonotoneWhereRequired
RollbackPreservedUntil
```

## 10. Mixed-version populations

### 10.1 Version population

```text
VersionPopulation {
    version
    instance_set
    serving_set
    authoritative_set
}
```

A valid lifecycle configuration may contain multiple populations.

### 10.2 Coexistence contracts

```text
VersionCoexistenceContract {
    versions
    component_scope
    allowed_interactions
    prohibited_interactions
    state_contract
    protocol_contract
    feature_constraints
}
```

### 10.3 Co-residence is not interoperability

These are separate relations:

```text
CanCoexistPhysically
CanExchangeProtocol
CanShareState
CanServeSameClientSession
CanParticipateInSameTransaction
```

### 10.4 Indefinite multi-version operation

MNCS must permit a system that intentionally remains multi-version if its contracts allow it. Convergence onto one executable generation is a lifecycle policy, not a universal semantic law.

## 11. Compatibility matrices and envelopes

### 11.1 Runtime compatibility matrix

Example:

```text
            server
            V1    V2
client V1   YES   YES
client V2   NO    YES
```

The matrix is directional.

### 11.2 Compatibility envelope

```text
CompatibilityEnvelope {
    runtime_versions
    protocol_versions
    schema_versions
    persistent_state_versions
    feature_states
    action_scope
}
```

### 11.3 Compatibility is action-relative

An older component may be permitted to:

```text
READ
FORWARD
DRAIN
```

while forbidden to:

```text
WRITE
FINALIZE
EXECUTE_NEW_OPERATION
```

This composes naturally with RFC 0018 action authorization.

## 12. Upgrade dependency graphs

### 12.1 Dependency derivation

Compatibility constraints can imply:

```text
UpgradeBefore(A, B)
```

without a manually authored step order.

Example:

```text
Bv2 requires protocol emitted by Av2
Av2 remains compatible with Bv1
```

Therefore:

```text
A -> B
```

is a lifecycle dependency.

### 12.2 Partial order

The dependency graph should support independent transitions:

```text
A -> C
B -> C
A || B
```

where `A || B` means the plan need not order A and B unless another relation requires it.

### 12.3 Cycles

A cycle can indicate:

```text
A2 requires B2
B2 requires A2
```

without a mixed-version-compatible transition.

The correct result may be:

```text
DIRECT_ROLLING_UPGRADE = UNSAT
```

rather than inventing an unsafe order.

## 13. Bridge versions and compatibility shims

A bridge release may intentionally satisfy both old and new contracts:

```text
A1/B1
   ->
A1.5/B1.5
   ->
A2/B2
```

A bridge can contain:

```text
old protocol reader
new protocol writer
old/new state adapter
feature gate
compatibility shim
```

Bridge releases are realization artifacts. They do not alter the canonical compatibility obligations.

Bridge retirement must be explicit after no dependent old/new path remains.

## 14. Update points and active execution

### 14.1 Update point

```text
UpdatePoint {
    scope
    active_version_set
    required_quiescence
    state_conditions
    compatibility_conditions
    protected_relations
}
```

An update point can be discovered, synthesized, or declared, but its validity must be checked against current execution state.

### 14.2 Active frames

Changing inactive code is simpler than changing code represented by active:

```text
stack frames
continuations
closures
actors
callbacks
transactions
sessions
```

RFC 0030 therefore distinguishes:

```text
InactiveCodeReplacement
ActiveCodeTransition
ContinuationMigration
```

### 14.3 Quiescence

```text
QuiescenceCondition {
    scope
    no_active_old_transactions?
    no_old_state_borrows?
    no_incompatible_messages_in_flight?
    no_old_continuations?
}
```

Quiescence is one method for proving an update point safe. It is not the only method.

## 15. Version consistency

### 15.1 Per-request consistency

```text
RequestVersionConsistent(request)
```

means a protected request is interpreted entirely according to one allowed version semantics or an explicitly verified cross-version transition relation.

### 15.2 Transactional version consistency

A transaction that begins under V1 may be required to finish under V1 semantics even if the surrounding system upgrades to V2.

```text
TransactionVersionConsistent(transaction)
```

prevents one transactional region from unknowingly straddling incompatible code versions.

### 15.3 Session version consistency

Long-lived sessions can be pinned:

```text
Session S -> V1
```

while new sessions use V2.

This may permit rollout without global quiescence.

### 15.4 Continuation version binding

Long-lived callbacks, actors, continuations, and timers may carry:

```text
ContinuationVersionBinding {
    continuation
    semantic_version
    compatible_successors
}
```

This prevents retirement from discarding semantics still referenced by delayed work.

## 16. State migration lifecycle

### 16.1 Separation of concerns

RFC 0027 defines the semantic transformation:

```text
StateV1 -> StateV2
```

RFC 0026 defines durable/crash-safe execution.

RFC 0030 defines when V1 and V2 are accepted, which versions may read/write each representation, and when compatibility can be retired.

### 16.2 Migration phases

Suggested lifecycle phases:

```text
PREPARE
OLD_AUTHORITATIVE
DUAL_READ
DUAL_WRITE
BACKGROUND_MIGRATION
NEW_AUTHORITATIVE
FINALIZED
OLD_REPRESENTATION_RETIRED
```

No implementation must use every phase.

### 16.3 Migration strategies

Candidate realizations include:

```text
EagerMigration
LazyMigration
BackgroundMigration
VirtualMigration
DualRead
DualWrite
ReadOldWriteNew
```

The semantic contract is expressed through allowed state forms and authority, not the strategy name.

### 16.4 State authority

```text
StateAuthority {
    representation
    readable_by
    writable_by
    authoritative_for
}
```

This is especially important during dual-format windows.

### 16.5 State migration failure

A lifecycle transition must name what phase is legitimate after crash/recovery. RFC 0026 provides the durability/recovery witness.

## 17. Expand-migrate-contract as a realization family

Many safe changes can be realized conceptually as:

```text
Expand compatibility
Migrate population/state
Contract compatibility
```

For example:

```text
1. deploy readers accepting S1 and S2
2. begin writing S2
3. migrate S1 records
4. verify no S1-only consumer remains
5. retire S1 writes
6. later retire S1 reads
```

The important semantic objects are compatibility envelopes, migration authority, and retirement conditions. The pattern itself is not the ontology.

## 18. Rollout realization families

RFC 0030 recognizes rollout structures without making them language primitives.

### 18.1 All-at-once

Replace an authoritative population as one bounded transition. Suitable only when downtime/coexistence requirements permit it.

### 18.2 Rolling

Gradually change the population while old and new versions coexist.

### 18.3 Parallel environment / blue-green

Maintain old and new environments concurrently, then change traffic authority.

### 18.4 Canary

Expose a bounded cohort/traffic scope to the candidate and collect evidence before broader promotion.

### 18.5 Shadow

Feed candidate execution real or mirrored inputs while withholding authoritative output/effects.

### 18.6 Hot/dynamic update

Change active or resident program code/state without conventional process replacement, subject to update-point and version-consistency obligations.

### 18.7 Bridge release

Introduce an intermediate version specifically to satisfy otherwise incompatible transition edges.

The planner may combine these families.

## 19. Population transitions and availability envelopes

### 19.1 Population state

```text
PopulationState {
    old_instances
    new_instances
    unavailable_instances
    serving_instances
    authoritative_instances
}
```

### 19.2 Transitional availability

A rollout may require:

```text
AvailableInstances >= required_minimum
```

or more abstractly:

```text
AvailabilityRelation(operation_class)
```

RFC 0028 owns the distributed availability meaning.

### 19.3 Progress is separate

A rollout can be stalled while service availability remains satisfied. Conversely, rollout steps may progress while an availability obligation fails.

## 20. Canary semantics

### 20.1 Exposure scope

```text
ExposureScope {
    population_predicate
    traffic_predicate
    region?
    topology_scope?
    workload_scope?
    data_scope?
    time_interval?
}
```

A canary is defined by exposure, not necessarily by a percentage of machines.

### 20.2 Canary evidence

```text
CanaryEvidence {
    candidate_generation
    exposure_scope
    observation_window
    observations
    assumptions
    environment
    dependencies
}
```

### 20.3 Scope discipline

Passing a canary establishes only the claim supported under the sampled exposure and evidence method. Expansion requires RFC 0018 appraisal.

### 20.4 Promotion ladder

Possible actions:

```text
AUTHORIZE_CANARY
AUTHORIZE_EXPANSION
AUTHORIZE_FULL_TRAFFIC
AUTHORIZE_FINALIZATION
```

These may use different assurance profiles.

## 21. Shadow execution semantics

### 21.1 Non-authoritative result

```text
ShadowExecutionContract {
    input_scope
    output_observation
    authoritative_output: false
    effect_policy
}
```

### 21.2 Effect isolation

Shadow execution must not obtain ambient authority to duplicate destructive effects merely because its returned result is ignored.

Potential requirement:

```text
NoAuthoritativeExternalEffects
```

or a narrower allowlist of sandboxed effects.

### 21.3 Shadow evidence

Shadow behavior can produce bounded comparative evidence while remaining a non-authoritative candidate.

## 22. Activation, serving, traffic, and authority

These states must remain separate:

```text
CodeAvailable
CodeActive
FeatureEnabled
ReceivesTraffic
ProducesResponse
ResponseAuthoritative
EffectsAuthoritative
```

A candidate may receive 100% mirrored traffic while holding zero authoritative effects.

Likewise, a V2 population may be fully active before traffic cutover.

## 23. Feature gates as lifecycle transitions

Feature activation may change semantics without changing executable artifacts.

```text
FeatureTransition {
    feature
    old_state
    new_state
    compatibility_requirements
    state_requirements
    rollback_or_disable_semantics
}
```

A feature gate may be used to decouple code availability from behavior activation.

Turning a feature off does not erase state or external effects already produced while it was on.

## 24. Cutover

### 24.1 Cutover event

```text
Cutover {
    from_authority
    to_authority
    scope
    preconditions
    fencing_or_epoch?
    witness
}
```

Cutover can apply to:

```text
traffic
writes
reads
feature authority
state authority
leader authority
```

### 24.2 Cutover is not finalization

After cutover, old compatibility may remain intentionally available for rollback.

## 25. Finalization

### 25.1 Finalization point

```text
FinalizationPoint {
    lifecycle_transition
    preconditions
    retired_assumptions
    newly_authorized_features
    rollback_effect
    irreversible_steps
}
```

### 25.2 Meaning

Finalization means the system deliberately crosses a boundary after which one or more assumptions required by the previous lifecycle phase no longer hold.

Examples:

```text
old write schema disabled
old binary no longer accepted
old migration journal discarded
old index removed
old protocol disabled
old credential authority revoked
```

### 25.3 Finalization authority

Finalization may use a stronger RFC 0018/RFC 0025 policy than ordinary rollout expansion because it can remove recovery options.

## 26. Irreversibility

### 26.1 Explicit artifact

```text
IrreversibleTransition {
    id
    lifecycle_phase
    affected_subject
    reason
    lost_capability
    affected_rollback_paths
}
```

### 26.2 Examples

```text
one-way state transform
old key destruction
old representation deletion
external irreversible effect
protocol compatibility removal
historical decoder removal
```

### 26.3 No hidden irreversibility

A deployment step that silently removes rollback assumptions is invalid unless represented as an explicit irreversible transition or equivalent lifecycle delta.

## 27. Rollback semantics

### 27.1 Rollback is a forward transition

```text
RollbackTransition {
    from_configuration
    target_configuration

    code_compatibility
    state_compatibility
    protocol_compatibility
    placement_requirements
    authority_requirements

    reverse_migrations
    external_effect_constraints
    protected_relations
}
```

### 27.2 Rollback support is phase-scoped

```text
RollbackCapability {
    lifecycle_phase
    target_configuration
    valid_until?
    required_evidence
}
```

A time window alone does not establish that rollback is semantically possible.

### 27.3 Code rollback versus state rollback

```text
Install(V1)
```

is not a successful rollback if V1 cannot interpret current state.

### 27.4 Snapshot restore

Restoring a previous RFC 0026 snapshot is a different operation with different data-loss and external-effect semantics.

### 27.5 Roll-forward

When rollback is invalid, Forge may propose a corrective successor:

```text
V2 -> V2.1
```

without mislabeling it as rollback.

## 28. Retirement semantics

### 28.1 Retirement condition

```text
RetirementCondition {
    subject
    no_active_execution
    no_new_assignment
    no_state_dependency
    no_protocol_dependency
    no_rollback_dependency
    no_historical_decode_dependency
    no_authority_dependency
}
```

Conditions may vary by retired subject.

### 28.2 Granular retirement

Possible retirement subjects:

```text
ExecutableVersion
ProtocolVersion
SchemaWriter
SchemaReader
StateRepresentation
FeatureFlag
CompatibilityShim
BridgeVersion
Credential
MigrationTool
```

These need not retire simultaneously.

### 28.3 Historical decoding

Old code may be retireable while historical state/log artifacts still require an old schema reader or migration function.

### 28.4 Old sessions

`NoNewTraffic(V1)` does not authorize `Retire(V1)` while a V1-pinned session, continuation, actor, or transaction remains active.

## 29. Lifecycle orchestration is persistent computation

### 29.1 Deployment state

A lifecycle controller may need durable state such as:

```text
DeploymentGeneration
CurrentPhase
CompletedStepIdentities
ActiveRollbackWindow
FinalizationState
```

RFC 0026 owns how that state survives crashes.

### 29.2 Controller failure

A controller can fail after performing a remote step but before recording knowledge of it.

RFC 0028 may then yield:

```text
OUTCOME_UNKNOWN
```

for that step.

The lifecycle model must reconcile or re-observe rather than fabricate success/failure.

### 29.3 Retry-safe lifecycle steps

Each retriable step must carry either:

```text
Idempotent
```

or:

```text
LogicalStepIdentity + DeduplicationContract
```

or another explicit retry-safe mechanism.

## 30. Distributed lifecycle semantics

### 30.1 Membership during rollout

Instances may enter/leave RFC 0028 configurations during rollout. Lifecycle claims must remain scoped to the active configuration/epoch.

### 30.2 Quorum viability

Taking replicas out of service for upgrade can invalidate quorum availability. A deployment plan must respect the distributed consistency/fault model.

### 30.3 Mixed-version distributed behavior

A rolling distributed system may require:

```text
ProtocolCompatible(V1,V2)
ReplicaStateCompatible(V1,V2)
ConsensusMembershipCompatible(V1,V2)
```

These are separate relations.

### 30.4 Fault detector interaction

An upgrading node must not be confused with a failed node unless the failure model explicitly treats maintenance absence as failure.

## 31. Topology-aware lifecycle constraints

RFC 0029 placement evidence may generate lifecycle rules such as:

```text
AtMostOneUnavailablePerFailureDomain
NeverUpgradeAllQuorumMembersTogether
PreserveTrustedVerifierCapacity
PreserveRequiredAcceleratorCapacity
```

A rollout order that is valid by version compatibility alone may still fail topology/fault-domain constraints.

## 32. Placement and lifecycle composition

A transition:

```text
V1 @ NodeA -> V2 @ NodeB
```

must remain decomposable into:

```text
SoftwareVersionTransition
PlacementTransition
```

possibly plus:

```text
StateMigration
CredentialRotation
ProtocolTransition
```

Each dimension carries its own evidence and rollback semantics.

## 33. State, wire, and protocol coexistence

### 33.1 Independent dimensions

A component can be:

```text
Code V2
Wire writer P1
Wire reader P1+P2
State writer S1
State reader S1+S2
```

while another version differs.

The lifecycle model must not infer all those dimensions from one executable version.

### 33.2 Historical compatibility horizon

A lifecycle can retire new writes to an old schema while retaining old reads indefinitely.

```text
Write(S1) = forbidden
Read(S1) = permitted
```

is a valid phase.

## 34. Security and authority during lifecycle change

### 34.1 Separate authorities

Potential authority classes include:

```text
DeployAuthority
ActivateAuthority
CanaryAuthority
TrafficPromotionAuthority
StateMigrationAuthority
FinalizationAuthority
RollbackAuthority
RetirementAuthority
```

Possessing one does not imply the others.

### 34.2 Candidate trust

New instances may require fresh RFC 0025 workload identity and attestation before receiving authoritative traffic.

### 34.3 Information flow

Mixed-version systems may expose old/new fields, routes, logs, timing channels, feature behavior, or migration metadata. RFC 0024 remains authoritative.

### 34.4 Credential retirement

Old deployment credentials may need revocation only after no rollback path depends on them. Conversely security policy may force their earlier revocation and thereby narrow rollback.

## 35. Assurance and lifecycle readiness

### 35.1 Readiness claims

Suggested artifacts:

```text
ReadyForCanary
ReadyForExpansion
ReadyForFullTraffic
ReadyForFinalization
SafeToRetire
```

Each is an RFC 0018 claim with dependencies and evidence.

### 35.2 Canary appraisal

Canary evidence may support expansion but should not automatically authorize finalization.

### 35.3 Increasing irreversibility

Policies may intentionally require stronger assurance for later lifecycle actions:

```text
install < canary < expand < full traffic < finalize < destroy old state
```

This is a policy relation, not a universal fixed ladder.

## 36. Lifecycle planning

### 36.1 Deployment plan

```text
DeploymentPlan {
    id
    source_configuration
    target_configuration

    phases
    transition_edges
    coexistence_sets

    migration_steps
    routing_steps
    feature_gates

    rollback_paths
    finalization_points
    retirement_steps

    requirements
    preferences
    obligations
}
```

### 36.2 Plan synthesis

Candidate planners may include:

```text
rule-based planner
constraint solver
SMT/SAT search
model checker
graph planner
LLM/agent
Forge recursive search
```

They remain untrusted proposal mechanisms.

### 36.3 Plan checking

A narrower checker validates hard relations independent of how the plan was discovered.

## 37. Deployment witness

```text
DeploymentWitness {
    plan
    source_configuration
    target_configuration

    checked_transition_edges
    compatibility_witnesses
    version_consistency_witnesses
    migration_witnesses
    availability_witnesses
    topology_witnesses
    authority_witnesses
    rollback_witnesses

    assumptions
    dependencies
}
```

The witness can reference certificates or verifier results owned by other RFCs.

## 38. Candidate micro-verifiers

### 38.1 Upgrade-order verifier

Checks that every lifecycle phase contains only supported communicating/coexisting version pairs and that dependency ordering is satisfied.

### 38.2 Mixed-version compatibility verifier

Checks declared protocol/state/action relations for each mixed population.

### 38.3 Version-consistency verifier

Checks that a request/transaction/session/continuation cannot cross an incompatible update boundary.

### 38.4 Update-point verifier

Checks active frames/state/quiescence constraints before dynamic replacement.

### 38.5 State-migration binding verifier

Checks that lifecycle phases use RFC 0027 migration relations and RFC 0026 crash-safety evidence appropriate to current authority.

### 38.6 Rollback verifier

Checks whether the current state/protocol/code population still admits the requested target configuration.

### 38.7 Retirement verifier

Checks absence of active/runtime/state/protocol/rollback/historical dependencies before retiring an artifact or compatibility path.

### 38.8 Fault-domain rollout verifier

Uses RFC 0029 topology plus RFC 0028 fault/quorum semantics to reject rollout batches that destroy required independence or quorum.

### 38.9 Shadow-effect verifier

Checks that shadow candidate execution lacks forbidden authoritative effects.

### 38.10 Finalization verifier

Checks finalization preconditions and enumerates rollback paths/compatibility assumptions that will be destroyed.

## 39. Lifecycle counterexamples

A failed lifecycle relation should produce a focused artifact:

```text
LifecycleCounterexample {
    transition
    phase
    configuration
    participating_versions
    violated_relation
    witness_trace
    explanation_slice
}
```

Examples:

```text
V1 client sends P1 to V2 server that accepts only P2
```

or:

```text
V1 executable requested after finalization but only S2 state exists
```

or:

```text
rollout batch removes two replicas from the same quorum set
```

Counterexamples remain relation-scoped.

## 40. Lifecycle deltas

Forge should surface changes through:

```text
LifecycleDelta {
    added_versions
    retired_versions
    changed_compatibility
    changed_state_authority
    changed_feature_authority
    changed_rollback_paths
    new_irreversible_steps
    changed_exposure
    stale_evidence
}
```

Authority broadening or irreversible narrowing should be particularly visible.

## 41. Lowering and preservation

Lifecycle semantics must survive semantic graph -> HIR -> SSA/backend/runtime packaging where applicable.

The compiler/runtime must not erase:

```text
version identities
compatibility requirements
feature gates
update-point constraints
state-migration bindings
rollback/finalization boundaries
retirement requirements
```

when those obligations are required by the supported subset.

A runtime manifest may carry a concrete lifecycle realization while remaining traceable to canonical lifecycle identities.

## 42. Runtime lifecycle manifest

A possible concrete artifact:

```text
RuntimeLifecycleManifest {
    deployment_generation
    artifact_versions
    compatibility_envelope
    state_migration_phase
    feature_states
    authoritative_populations
    rollback_targets
    finalization_state
    retirement_guards
}
```

This is a realization artifact, not the complete source semantics.

## 43. Lifecycle observation

Observation may include:

```text
instance version
active population
migration progress
traffic authority
feature activation
finalization state
```

Observation is an RFC 0008 effect and may itself be stale or incomplete.

The canonical lifecycle model must distinguish desired configuration from observed configuration.

## 44. Desired versus observed lifecycle state

```text
DesiredLifecycleConfiguration
ObservedLifecycleConfiguration
```

are distinct.

A plan can intend:

```text
all V2
```

while observed reality remains:

```text
4 V2
1 V1
1 unknown
```

The unknown instance cannot be silently assumed compliant.

## 45. Unknown lifecycle state

Unknowns must be representable:

```text
InstanceVersion = UNKNOWN
MigrationOutcome = UNKNOWN
CutoverOutcome = UNKNOWN
RollbackOutcome = UNKNOWN
```

These should block stronger actions when the relevant policy requires certainty.

## 46. Lifecycle stabilization

Completion of a rollout may require more than no pending actions.

Potential property:

```text
LifecycleStabilized {
    desired_configuration reached
    no incompatible old execution
    state migration settled
    authority settled
    required evidence current
}
```

RFC 0022 owns the generic stabilization/liveness theory.

## 47. Failure-aware finalization

A finalization plan should remain safe if the controller crashes immediately before or after the finalization point.

If exact outcome knowledge is lost, recovery must determine whether old rollback assumptions were actually destroyed before attempting a downgrade.

This is an excellent RFC 0026/0028 composition case.

## 48. Dynamic software update realization family

Dynamic software update research motivates several useful semantic concerns:

- active code and state can outlive code replacement;
- old state may require explicit conversion;
- update points can constrain legal transition moments;
- version consistency can be scoped to transactions/requests;
- type/representation safety is necessary but not sufficient.

MNCS should adopt those concerns without selecting a particular DSU runtime.

## 49. Consistent-update theory as a general lifecycle relation

Network consistent-update research shows that correct endpoint configurations can still produce invalid behavior while the system is changing.

MNCS generalizes this idea beyond forwarding rules:

```text
For every protected semantic unit U,
its traversal through the lifecycle trace
must satisfy relation R.
```

Possible units:

```text
request
transaction
session
message
replicated operation
state record
continuation
```

Possible relations:

```text
OldOrNewSemantics
PerUnitVersionConsistency
NoForbiddenHybrid
AvailabilityPreserved
SecurityRefinement
```

## 50. Lifecycle realization holes

A program may leave rollout strategy unresolved:

```text
LifecycleStrategy = ?
```

while declaring:

```text
no total outage
request-version consistency
rollback until finalization
crash-safe state migration
quorum preserved
```

Forge can search for a realization later.

## 51. Forge lifecycle search

Forge may search dimensions such as:

```text
rollout family
batch size
version coexistence
bridge releases
feature-gate timing
state-migration strategy
traffic cutover
shadow/canary exposure
finalization timing
retirement sequence
```

Only semantically admissible candidates proceed to RFC 0032/0033 optimization.

## 52. Realization interaction with representation and numeric semantics

A new version may change:

```text
physical representation
numeric realization
backend
accelerator use
```

If the lifecycle transition mixes old/new realizations, protected RFC 0019/0021 relations must still hold.

A faster V2 implementation that breaks a protected numeric relation remains invalid even if rollout mechanics are flawless.

## 53. Realization interaction with time

Lifecycle conditions may use RFC 0023 concepts:

```text
canary observation interval
rollback validity window
lease expiration
migration deadline
feature effective time
retirement-after interval
```

Time never substitutes for compatibility evidence.

## 54. Realization interaction with information flow

Lifecycle transitions can temporarily expand observers:

```text
shadow logs
migration logs
dual-write destinations
old/new telemetry
compatibility proxies
```

RFC 0024 policies must cover those intermediate channels.

## 55. Realization interaction with attestation and trust

A newly deployed V2 instance must not inherit the attestation/trustworthiness of a V1 instance or a different host.

Traffic promotion may depend on fresh:

```text
WorkloadIdentity
AttestationResult
Credential
```

from RFC 0025.

## 56. Realization interaction with persistence

State migration, lifecycle journals, finalization markers, rollback metadata, and retirement records are persistent state where required.

RFC 0026 must establish their crash semantics.

## 57. Realization interaction with serialization

During coexistence, old/new peers may read/write different schemas or wire profiles.

RFC 0027 directional compatibility and migration relations are direct inputs into lifecycle admissibility.

## 58. Realization interaction with distribution

Version-skew, quorum, membership, leader election, replica state, and remote outcome uncertainty all remain RFC 0028 concerns consumed by lifecycle verification.

## 59. Realization interaction with placement

A rollout may temporarily move or duplicate instances, alter fault-domain coverage, or require new hardware capabilities.

RFC 0029 validates those spatial choices.

## 60. Bounded first implementation pilot

The first executable pilot should remain deliberately small.

Introduce versioned artifacts for:

```text
Release
VersionIdentity
DeploymentGeneration
LifecycleConfiguration
LifecycleTransition
DeploymentPhase
VersionPopulation
CompatibilityMatrix
CompatibilityEnvelope
UpgradeDependencyGraph
UpdatePoint
VersionConsistencyClaim
StateMigrationPhase
FeatureActivation
ExposureScope
FinalizationPoint
RollbackTransition
RetirementCondition
DeploymentWitness
LifecycleCounterexample
LifecycleDelta
```

The pilot should not claim production deployment orchestration.

## 61. Fixture A — artifact identity versus deployment generation

Deploy identical artifact V1 twice:

```text
V1 / G1
V1 / G2
```

Require:

```text
ArtifactIdentity(G1.artifact, G2.artifact) = same
DeploymentGenerationIdentity(G1, G2) = different
```

## 62. Fixture B — installed is not active

```text
V2 installed
V1 authoritative
```

Then activate V2 as a distinct lifecycle transition.

A verifier must refuse to infer active behavior from installation.

## 63. Fixture C — mixed population is not a scalar version

```text
3 × V1
2 × V2
```

The lifecycle configuration remains explicit rather than collapsing into `system_version = V2`.

## 64. Fixture D — valid endpoints, invalid rolling transition

All-V1 passes.

All-V2 passes.

Mixed V1/V2 fails because:

```text
V1 emits P1
V2 accepts only P2
```

The rolling candidate is rejected while a non-coexisting realization may remain possible.

## 65. Fixture E — transactional version consistency

A transaction begins under V1.

The system activates V2 globally.

A naive candidate allows half the transaction to execute under V2 assumptions.

Reject.

A pinned/compatible candidate completes entirely under one allowed semantic version.

Accept.

## 66. Fixture F — explicit state transformation

```text
StateV1 { name }
    -> migration ->
StateV2 { first_name, last_name }
```

Show that code transition and state transition are independently represented and that the migration relation carries its loss/assumption evidence.

## 67. Fixture G — directional version skew

```text
V1 <-> V2 : supported
V1 <-> V3 : unsupported
```

Accept rolling V1 -> V2.

Reject direct rolling V1 -> V3.

## 68. Fixture H — generated upgrade order

Compatibility dependencies generate:

```text
Bridge reader
    before
Service V2
    before
Retire old writer
```

Check the partial order independently of planner choice.

## 69. Fixture I — cyclic dependency and bridge release

Direct transition:

```text
A1/B1 -> A2/B2
```

has no valid rolling order.

Introduce bridge versions and prove a valid coexistence path.

## 70. Fixture J — finalization removes rollback

Before finalization:

```text
Rollback(V2 -> V1) = supported
```

After one-way state finalization and retirement of V1 compatibility:

```text
Rollback(V2 -> V1) = rejected
```

The loss of rollback is an explicit lifecycle delta.

## 71. Fixture K — code rollback is not state rollback

V2 writes S2.

V1 reads only S1.

Attempting to reinstall V1 must not produce `ROLLBACK_SUCCESS`.

A counterexample names current state incompatibility.

## 72. Fixture L — bounded canary evidence

Expose V2 to a declared cohort and observation window.

Canary behavior satisfies the scoped test/appraisal profile.

Result supports only the canary-scoped claim. It does not establish universal correctness.

## 73. Fixture M — shadow effect isolation

Mirror a production request into V2 shadow execution.

The shadow candidate may compute a response but lacks authority for the destructive external effect.

A candidate with ambient write authority is rejected.

## 74. Fixture N — old execution blocks retirement

All new traffic goes to V2.

One V1-pinned session remains active.

Require:

```text
TrafficCutoverComplete = true
V1Retirable = false
```

## 75. Fixture O — crash during migration

Crash during a lifecycle state migration.

RFC 0026 recovery returns to or resumes from an allowed migration phase.

A plan that assumes controller/migrator immortality is rejected.

## 76. Fixture P — unknown remote rollout outcome

Controller asks worker to activate V2.

Worker activates V2.

Response is lost.

Controller state is:

```text
ActivationOutcome = UNKNOWN
```

until reconciliation observes current state.

## 77. Fixture Q — fault-domain rollout failure

Three replicas occupy three failure domains.

A candidate batch takes two quorum-critical replicas unavailable simultaneously.

Reject using RFC 0028 quorum and RFC 0029 topology evidence.

## 78. Fixture R — full traffic is not finalization

100% of authoritative traffic reaches V2.

V1 remains installed and S1 compatibility remains intact for rollback.

Require:

```text
FullTraffic = true
Finalized = false
V1Retired = false
```

## 79. First Forge lifecycle study

Use a stateful replicated service:

```text
Current:
    Service V1
    Schema S1
    Protocol P1

Target:
    Service V2
    Schema S2
    Protocol P2

Requirements:
    no total outage
    request version consistency
    rollback until explicit finalization
    crash-safe state migration
    quorum remains viable
    fault-domain separation remains valid
    new feature disabled until all authoritative peers support P2

Lifecycle strategy:
    ?
```

Forge considers:

```text
all-at-once
naive rolling
blue/green
hot update
bridge + rolling
canary bridge + rolling
```

Expected qualitative outcomes:

```text
all-at-once:
    reject if no-outage requirement fails

naive rolling:
    reject if V1/V2 protocol coexistence fails

hot update:
    reject if no valid update point/state transformation exists

blue/green:
    semantically admissible if state/traffic constraints hold

bridge + rolling:
    semantically admissible if compatibility/quorum constraints hold

canary bridge + rolling:
    same semantic admissibility plus bounded production evidence
```

RFC 0032/0033 may later rank valid candidates.

## 80. Second Forge study — rollback evolution

Start from an accepted lifecycle plan.

Before finalization:

```text
Rollback(V2 -> V1) = AUTHORIZABLE
```

Execute:

```text
one-way state migration
old writer retirement
finalization
```

Ask again.

Expected:

```text
Rollback(V2 -> V1) = REJECTED
reason:
    V1 cannot interpret authoritative durable state
```

Forge may propose:

```text
RollForward(V2 -> V2.1)
```

This proves rollback is lifecycle-state dependent rather than a permanent Boolean property.

## 81. Third Forge study — topology-aware rollout

Use a replicated component whose instances span failure domains.

Generate candidate rollout batches.

A fast candidate upgrades multiple replicas in one protection set simultaneously and violates quorum/fault tolerance.

Reject it.

A slower candidate preserves required independence and is admissible.

This demonstrates semantic admissibility preceding performance ranking.

## 82. Fourth Forge study — active-version consistency

A long-running operation begins on V1.

Forge compares:

```text
A: immediate in-place code replacement
B: pin old operation, route new operations to V2
C: verified continuation migration
```

A fails the protected version-consistency relation.

B and C may remain admissible if their independent obligations pass.

## 83. Machine-native theory synthesis

RFC 0030 draws from, but does not reduce itself to, several research traditions and production precedents.

### 83.1 Dynamic software updating

Dynamic software updating establishes the importance of explicit code/state transitions and active-execution updateability rather than treating live upgrade as byte replacement.

Relevant directions include early Dynamic Software Updating work, Proteus, contextual effects / Transactional Version Consistency, and Kitsune-style state transformation.

### 83.2 Consistent updates

Consistent network-update theory demonstrates that endpoint correctness does not imply transition correctness. RFC 0030 generalizes per-packet/per-flow consistency ideas into per-request, transaction, session, message, and state-transition lifecycle relations.

### 83.3 Version-skew policy

Systems such as Kubernetes explicitly define allowed component version skew and derive upgrade order constraints from those relations. RFC 0030 generalizes that into version-coexistence contracts and upgrade dependency graphs.

### 83.4 Hot code and state change

Erlang/OTP demonstrates a realization family where processes can be suspended, code/state transformed, and resumed. MNCS treats the phases and obligations as semantic objects while remaining runtime-neutral.

### 83.5 Finalization/version gating

Database systems such as CockroachDB demonstrate the operational value of separating new binaries running from version finalization that enables irreversible migrations/features and removes old-version rollback.

### 83.6 Blue/green, canary, and shadow deployment

Modern release practice demonstrates parallel environments, bounded exposure, gradual promotion, and shadow comparison as useful realization families. RFC 0030 treats them as evidence/exposure/cutover structures, not the ontology.

## 84. Research references and precedents

The following are starting points for RFC 0030 research and implementation experiments:

- Michael Hicks et al., dynamic software updating research and explicit state transformation.
- Gareth Stoyle, Michael Hicks, Gavin Bierman, Peter Sewell, and Iulian Neamtiu, **Mutatis Mutandis: Safe and Predictable Dynamic Software Updating**, POPL 2005 / Proteus direction: <https://www.cs.umd.edu/~mwh/papers/StoyleHBSN05.html>.
- Iulian Neamtiu et al., **Contextual Effects for Version-Consistent Dynamic Software Updating** / Transactional Version Consistency direction: <https://mhicks.me/papers/neamtiu08context.html>.
- Christopher M. Hayden et al., **Kitsune: Efficient, General-purpose Dynamic Software Updating for C** and follow-on work: <https://www.cs.umd.edu/~mwh/papers/hayden14kitsune-journal.html>.
- Erlang/OTP release handling and state/code change: <https://www.erlang.org/doc/system/release_handling.html>.
- Mark Reitblatt et al., consistent network updates / abstractions for network update: <https://reitblatt.com/papers/>.
- Kubernetes version-skew policy: <https://kubernetes.io/releases/version-skew-policy/>.
- Kubernetes rolling Deployment semantics: <https://kubernetes.io/docs/concepts/workloads/controllers/deployment/>.
- CockroachDB major-version upgrade/finalization behavior: <https://www.cockroachlabs.com/docs/stable/upgrade-cockroach-version>.
- Google SRE Workbook, canarying releases: <https://sre.google/workbook/canarying-releases/>.
- AWS blue/green deployment guidance as an operational realization precedent: <https://docs.aws.amazon.com/whitepapers/latest/blue-green-deployments/introduction.html>.
- Protocol Buffers evolution guidance as a precedent for long-lived mixed-version and historical-data compatibility: <https://protobuf.dev/best-practices/dos-donts/>.

These sources inform the theory stack but do not define MNCS semantics.

## 85. Open research questions

RFC 0030 intentionally leaves several questions experimental.

1. What is the smallest canonical lifecycle artifact set that still captures meaningful live-upgrade semantics?
2. Which version-consistency relations should be built in versus defined through RFC 0020 relation schemas?
3. Can Forge synthesize bridge releases or compatibility adapters and produce independently checkable equivalence/refinement evidence?
4. How should active continuation migration be represented without overcommitting to a runtime stack model?
5. Which lifecycle properties admit compositional verification by component, and which require whole-population traces?
6. How should irreversible-transition authority interact with RFC 0018 assurance policies?
7. What is the minimum trusted checker for deployment-plan validity?
8. Can update-point discovery be derived automatically from effects, state ownership, and active-version constraints?
9. How should lifecycle plans express intentionally indefinite multi-version operation?
10. How should long-lived external clients constrain retirement when their exact versions are unknown?
11. What evidence is sufficient to infer that an old protocol/schema decoder remains unnecessary?
12. How should lifecycle state interact with model/data-version changes for MNEL-style learned components?
13. Can a generalized per-semantic-unit consistent-update relation subsume request, transaction, session, and packet-like upgrade consistency without becoming too abstract to check?
14. How should lifecycle deltas expose changes in rollback possibility so autonomous systems cannot cross irreversible boundaries accidentally?
15. Which quantitative rollout metrics belong in RFC 0032 without leaking optimization policy back into semantic admissibility?

## 86. Non-goals

RFC 0030 does not attempt to:

- define one deployment controller;
- define Kubernetes semantics as MNCS semantics;
- require hot code replacement;
- require rolling updates;
- require eventual convergence onto one version;
- define artifact build provenance or software supply-chain attestations;
- define concrete service-mesh traffic-routing APIs;
- define database-specific migration languages;
- define placement/topology semantics already owned by RFC 0029;
- define distributed consensus/quorum semantics already owned by RFC 0028;
- define schema migration correctness already owned by RFC 0027;
- define crash-safe persistence already owned by RFC 0026;
- define one global semantic versioning policy;
- assign one universal deployment risk score;
- treat canary success as proof of universal correctness; or
- allow lifecycle automation to bypass existing authority/evidence boundaries.

## 87. Initial implementation acceptance criteria

A first RFC 0030 implementation should be considered successful when it can:

1. represent artifact version and deployment generation separately;
2. represent a lifecycle configuration containing multiple active versions;
3. distinguish installed, active, serving, authoritative, finalized, and retired state;
4. represent a transition with allowed/forbidden intermediate configurations;
5. reject a rolling transition whose endpoints are valid but mixed-version protocol relation fails;
6. represent directional version compatibility and derive a simple upgrade partial order;
7. represent at least one cyclic dependency that becomes realizable only after introducing a bridge version;
8. check one request/transaction/session version-consistency relation;
9. bind one RFC 0027 state migration and RFC 0026 crash-safety claim to lifecycle phases;
10. represent a bounded canary exposure and prevent its evidence from becoming universal automatically;
11. enforce shadow execution without authoritative effects;
12. distinguish full traffic cutover from finalization;
13. make finalization explicitly remove at least one rollback path;
14. reject code rollback when current authoritative state is incompatible;
15. reject retirement while an old session/continuation remains active;
16. recover or reconcile one crash/unknown-outcome lifecycle step;
17. reject one rollout batch using RFC 0028 quorum and RFC 0029 fault-domain evidence;
18. produce a deterministic deployment witness for one accepted plan;
19. produce a scoped lifecycle counterexample for one rejected plan; and
20. expose lifecycle deltas including rollback narrowing and stale evidence.

## 88. 1.0 research-threshold direction

A future MNCS 1.0 should require, at minimum:

- a versioned lifecycle semantic model distinguishing release/artifact/deployment generation/instance/population identities;
- versioned lifecycle configuration and transition artifacts with explicit intermediate-state semantics;
- explicit installed/loaded/active/serving/authoritative/finalized/retired distinctions;
- directional runtime compatibility matrices/envelopes and upgrade dependency graphs;
- mixed-version coexistence semantics independent of endpoint validity;
- request/transaction/session or equivalent version-consistency relations;
- update-point/quiescence semantics for active execution where live replacement is supported;
- lifecycle binding to typed state migration and crash-safe durable migration;
- explicit exposure/canary/shadow semantics with evidence scope and effect authority;
- explicit traffic/state/feature cutover semantics;
- explicit finalization and irreversible-transition artifacts;
- rollback modeled as a separately checked transition, not an automatic inverse;
- granular retirement conditions for executable, protocol, schema/state, and compatibility artifacts;
- persistent/failure-aware lifecycle orchestration semantics;
- topology/quorum-aware rollout constraints;
- untrusted deployment-plan synthesis separated from independent plan checking;
- bounded rollout/canary evidence preserved as bounded evidence; and
- at least one Forge study that rejects a faster/easier lifecycle realization because an intermediate-state, compatibility, state, authority, version-consistency, quorum, or rollback relation fails.

## 89. Required 1.0 demonstrations

The roadmap should eventually require concrete demonstrations that:

1. identical artifact bytes can participate in different deployment generations;
2. a running system can contain multiple explicit software versions without being collapsed to one scalar version;
3. `Installed != Active != Serving != Authoritative`;
4. all-old and all-new endpoint validity can coexist with a refuted mixed-version transition;
5. a transaction/request/session can remain version-consistent while the surrounding population upgrades;
6. a naive active update can violate version consistency while pinning or a verified continuation migration preserves it;
7. directional version-skew rules generate an independently checkable upgrade order;
8. a cyclic upgrade dependency can be reported unrealizable until a bridge version is introduced;
9. state migration phase is distinct from code deployment and finalization;
10. canary success remains scoped to its exposure/evidence rather than becoming universal promotion authority;
11. a shadow candidate without effect isolation is rejected even though its output is non-authoritative;
12. 100% traffic cutover can occur while the lifecycle remains unfinalized and rollbackable;
13. finalization explicitly destroys or narrows a rollback path;
14. reinstalling old code is rejected as rollback when authoritative state is incompatible;
15. a V1 session/continuation can block V1 retirement after all new traffic uses V2;
16. lifecycle controller failure can produce `UNKNOWN` remote-step outcome and recover by reconciliation rather than fabricated knowledge;
17. a rollout plan is rejected because its temporary population violates quorum/fault-domain requirements;
18. lifecycle topology/evidence changes invalidate only dependent rollout claims;
19. a deployment planner may remain untrusted while a smaller verifier checks its witness; and
20. bounded canary/mixed-version/rollback tests remain bounded evidence.

## 90. Closing direction

RFC 0030 makes software evolution itself part of the program's semantic world.

The language should not force a programmer or agent to decide first whether an upgrade is rolling, blue/green, canary, hot, or all-at-once. It should ask for the protected lifecycle relations:

```text
Which old/new versions may coexist?
Which requests may cross version boundaries?
Which state representations remain readable/writable?
Which migration phases are legal?
Which populations remain authoritative?
Which failures must remain survivable?
When is rollback still possible?
What action destroys that possibility?
When may old code, state, protocols, and shims disappear?
```

Then Forge may synthesize the transition realization.

The defining machine-native principle is:

> Programs specify the semantic envelope of change. Machines may choose how to cross from one valid world to another only if every protected intermediate state and transition relation remains justified.
