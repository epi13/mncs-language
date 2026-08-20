# RFC 0029: Machine-Native Placement, Topology, Locality, Mobility, Migration, and Heterogeneous-Execution Semantics

- **Status:** Draft
- **Authors:** MNCS project
- **Target:** pre-specification semantic research
- **Depends on:** RFC 0002, RFC 0004, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0014, RFC 0015, RFC 0017, RFC 0018, RFC 0019, RFC 0020, RFC 0021, RFC 0022, RFC 0023, RFC 0024, RFC 0025, RFC 0026, RFC 0027, RFC 0028
- **Leaves primarily to later RFCs:** deployment/version lifecycle and live software upgrade to RFC 0030; build/supply-chain derivation to RFC 0031; quantitative cost, bandwidth, energy, memory pressure, migration cost, and performance accounting to RFC 0032; multi-objective realization search/ranking to RFC 0033

## 1. Summary

MNCS must not treat physical placement as either invisible scheduler trivia or a fixed source-level hardware address.

Modern execution spans CPUs, NUMA domains, GPUs, accelerators, device-local memories, remote workers, trust domains, failure domains, and dynamically changing resource graphs. Computation and data may move independently. A logical entity may preserve identity while its physical realization moves. Conversely, observing or constraining location can make placement part of the program's semantics.

This RFC therefore establishes a machine-native spatial semantic layer in which logical location, physical topology, execution resources, memory resources, locality, affinity, anti-affinity, fault-domain separation, computation placement, data placement, capability-based heterogeneous execution, mobility boundaries, migration, cutover, placement evidence, and placement realization search are explicit semantic objects.

The central idea is:

> A machine-native program specifies the spatial relations its semantics require, not the hardware coordinates it happens to occupy. Placement, data movement, device selection, and migration remain realization search spaces until observation or contract makes them semantic.

RFC 0029 is the spatial complement to RFC 0023. RFC 0023 separates logical and physical time and keeps temporal realization flexible until timing is observed or constrained. RFC 0029 does the same for space.

The RFC deliberately does **not** select Legion, MPI topology mapping, hwloc, Chapel locales, X10 places, Mobile Ambients, Charm++, Cilk work stealing, StarPU, SYCL, graph partitioning, HEFT, process migration, or VM migration as the language semantics. These are theoretical precedents, realization families, runtime strategies, or witness-producing mechanisms.

Likewise, this RFC does not let terms such as `local`, `near`, `GPU`, `same node`, `migratable`, `affine`, `independent replica`, `best device`, or `placement valid` remain informal Boolean labels. Each such claim must name the relevant topology, location scope, capability set, observation model, migration boundary, or evidence.

## 2. Constitutional rules

The following rules are architectural constraints, not implementation suggestions.

1. **Identity is not location.** Logical identity and physical placement are distinct semantic objects.
2. **Movement is not rebirth.** Moving an entity does not create a new semantic entity unless the declared identity relation says so.
3. **Location may be observable.** If a program observes location, placement ceases to be fully opaque.
4. **Logical topology is not physical topology.** Application structure and machine structure remain separate graphs.
5. **Mapping is explicit.** A logical-topology-to-physical-topology mapping is a realization relation, not implicit equality.
6. **Identifier adjacency is not proximity.** `GPU0` and `GPU1`, or worker IDs with adjacent numbers, do not imply physical nearness.
7. **Containment is not proximity.** Two resources may share a container yet be distant in NUMA, cache, device, network, or fault topology.
8. **Reachability is not affinity.** A resource can access another without that relationship being desirable.
9. **Affinity is not a requirement by default.** Preferences and hard constraints remain distinct.
10. **`local` is not a universal Boolean.** Locality must name a relation or domain.
11. **Topology is evidence-bearing.** Runtime topology facts are environment-sensitive and may become stale.
12. **Placement validity precedes placement quality.** Optimization cannot rescue a semantically invalid placement.
13. **Placement requirements are not preferences.** A mapper may trade off preferences but may not violate hard requirements.
14. **Affinity and anti-affinity are both first-class.** Colocation and separation can each be semantic requirements.
15. **Replica count is not failure independence.** Physical fault-domain separation must be evidenced.
16. **Execution location and data location are separate.** Moving a task need not move its data, and moving data need not move its task.
17. **Logical data is not one physical instance.** One logical value or region may have multiple physical instances.
18. **Replication is not migration.** Copying state while retaining the source is different from moving it.
19. **Migration is not failover.** Proactive movement and failure-triggered replacement remain distinct.
20. **Migration is not live upgrade.** Spatial movement and software-version change remain distinct transformations.
21. **Code movement is not execution-state movement.** Shipping executable code is different from moving a running continuation/process/object.
22. **Passive-state movement is not active-computation migration.** Active state may include continuation, mailbox, timers, capabilities, and pending effects.
23. **Migration closure is explicit.** Reachability alone does not determine what must move.
24. **Migration transparency is observation-relative.** Functional behavior may be preserved while timing, failure exposure, locality, or location observations change.
25. **Placement authority is not implied by execution authority.** Permission to execute does not automatically grant permission to move.
26. **Migration may invalidate evidence.** Trust, attestation, clock, persistence, capability, reference, topology, timing, and side-channel evidence may need revalidation after movement.
27. **Device class is not device capability.** `GPU`, `CPU`, or `accelerator` labels are insufficient proof of feature support.
28. **Capability requirements should be semantic where possible.** Prefer feature requirements over vendor/device names unless the program genuinely depends on the device identity.
29. **No universal device score exists.** Suitability is requirement- and policy-relative.
30. **Capability filtering precedes ranking.** An incompatible resource cannot win an optimization score.
31. **Placement and scheduling are different dimensions.** RFC 0023 primarily answers when; RFC 0029 primarily answers where.
32. **Placement and representation may interact.** Moving computation may change the valid RFC 0019 representation set.
33. **Placement and numeric realization may interact.** Device placement may change which RFC 0021 realization is available.
34. **Placement may introduce serialization obligations.** Crossing memory/process/node/device boundaries can require RFC 0027 wire transformations.
35. **Placement may introduce distributed-semantics obligations.** Moving an interaction across a failure boundary can require RFC 0028 semantics.
36. **Remote placement cannot inherit local-call semantics without proof.** A local shared-memory relation may not be replaced by best-effort remote messaging silently.
37. **Security constraints can become placement constraints.** RFC 0024/0025 trust and information-flow requirements may require colocation, isolation, or destination classes.
38. **Persistent/reference constraints can become placement constraints.** RFC 0009/0026 references may not be usable from every destination.
39. **Clock-domain movement is semantic when time is observed.** RFC 0023 evidence may need rebinding after migration.
40. **Topology change can invalidate only dependent placement evidence.** Revalidation should be dependency-aware rather than global by default.
41. **Static, dynamic, and adaptive placement are all legal realizations.** No one placement lifetime is constitutional.
42. **Work stealing is a scheduling/placement realization, not parallel semantics.** Logical task identity must survive reassignment.
43. **Overdecomposition is a realization strategy, not a language requirement.** Logical tasks may outnumber physical resources, but need not.
44. **Execution slots and physical resources may differ.** Virtual scheduling entities may remap onto different hardware.
45. **Virtual place abstractions are legal.** A place can describe a capability-bearing placement domain rather than a raw coordinate.
46. **Location variables are first-class unresolved realization variables.** Placement may remain symbolic until a phase has enough facts.
47. **Placement constraints require satisfiability checking.** `SAT`, `UNSAT`, and `UNKNOWN` are distinct results.
48. **Placement search and placement checking are separate.** Untrusted heuristics may propose mappings; narrow checkers verify them.
49. **Optimal mapping is not required for semantic validity.** A valid but poor placement remains semantically valid.
50. **A faster placement may still be invalid.** Forge must reject candidates that violate protected spatial or cross-RFC relations.
51. **Migration has phases.** Start, transfer, cutover, destination activation, source retirement, and rollback/abort states are distinguishable.
52. **Temporary duplicate physical instances do not automatically imply replicated logical execution.** Migration can transiently create multiple copies of one logical entity.
53. **Cutover requires authority discipline.** Two active physical instances must not silently acquire simultaneous mutation authority unless replication semantics permit it.
54. **Placement rollback is not historical rollback.** External effects performed before a rollback remain historical facts.
55. **Placement history may be evidence.** Security, reproducibility, debugging, or assurance policies may require where execution occurred.
56. **Placement determinism is not result determinism.** Fixed placement and deterministic behavior are unrelated unless a relation proves otherwise.
57. **Placement reproducibility is scoped.** Two runs may differ spatially yet preserve the declared semantic relation.
58. **Fault-domain topology is not administrative topology.** Physical, trust, tenancy, jurisdictional, and organizational boundaries can overlap but must not be conflated.
59. **A topology may need multiple simultaneous views.** Hierarchies, distance matrices, capability graphs, and failure-domain graphs may each carry different facts.
60. **Physical topology is generally a graph, not only a tree.** Hierarchical containment is useful but insufficient for modern heterogeneous systems.
61. **Topology edge types are explicit.** Access, containment, connectivity, direct transfer, cache sharing, memory sharing, trust sharing, and fault sharing are not one relation.
62. **Distance is dimensioned.** Numeric latency/bandwidth cost belongs to RFC 0032, while RFC 0029 owns the existence and identity of locality/distance relationships.
63. **Hard anti-affinity can arise from independence requirements.** Replica or verifier independence may require distinct physical failure domains.
64. **Placement observations narrow optimization freedom.** Calling `here`, reading device identity, or observing locality creates placement-sensitive semantics.
65. **Placement independence is a claim, not an assumption.** A program can be proven placement-insensitive under an observation model.
66. **Location names are not universal coordinates.** Names require a topology or namespace context.
67. **Mobility boundaries are explicit.** The system must know which state and authority are inside the moving unit.
68. **Capabilities may be preserved, rebound, invalidated, or unavailable after migration.** None of these outcomes is implicit.
69. **Attestation is destination-specific unless evidence proves portability.** Source host attestation does not automatically attest the destination.
70. **A placement candidate that changes the failure model is a semantic transformation.** It requires re-evaluation against RFC 0011/0028 contracts.
71. **A placement candidate that changes observable numeric, timing, security, or persistence behavior requires the corresponding relation witness.** Placement alone cannot waive those obligations.
72. **Bounded placement experiments remain bounded evidence.** Successful tests on one topology do not prove universal placement validity.

## 3. Why placement requires a distinct semantic layer

Conventional software often hides placement at one of two extremes.

At one extreme, placement is treated as invisible runtime policy. Threads, tasks, data, and processes move among resources without the program having any semantic vocabulary to describe the consequences.

At the other extreme, placement is hard-coded into source:

```text
run_on("gpu0")
allocate_on("numa1")
node = "worker-7"
```

Both approaches are too weak for MNCS.

The first approach hides constraints that are increasingly correctness-relevant:

- an accelerator may not satisfy the required numeric relation;
- data may become inaccessible after movement;
- moving a secret-bearing computation may cross a trust boundary;
- moving one half of a local shared-memory interaction to another machine may create partial-failure semantics;
- a replicated service may appear redundant while all replicas share one physical host failure domain;
- a migration can invalidate attestation, clocks, credentials, persistent references, or target evidence.

The second approach destroys realization freedom by prematurely fixing a physical answer when the program may only require a relationship such as:

```text
requires feature F
must be near data D
must be separated from replica R under HostFailure
must remain inside trust domain T
```

MNCS therefore needs a middle layer: spatial contracts are semantic; physical mapping remains a realization.

## 4. Spatial semantic planes

RFC 0029 organizes the domain into nine interacting planes.

### 4.1 Spatial identity plane

Owns:

```text
Place
PlaceClass
LocationIdentity
LocationNamespace
ContainmentRelation
LocationObservation
```

### 4.2 Topology plane

Owns:

```text
Topology
TopologySnapshot
TopologyNode
TopologyEdge
LogicalTopology
PhysicalTopology
AdministrativeTopology
FailureTopology
TopologyMapping
```

### 4.3 Resource plane

Owns:

```text
ExecutionResource
MemoryResource
DeviceResource
ResourceCapabilitySet
ResourceObservation
ResourceAvailability
```

### 4.4 Placement plane

Owns:

```text
ComputationPlacement
DataPlacement
PlacementRequirement
PlacementPreference
LocationVariable
PlacementCandidate
PlacementWitness
```

### 4.5 Locality plane

Owns:

```text
AffinityConstraint
AntiAffinityConstraint
ColocationConstraint
SeparationConstraint
DirectAccessConstraint
CommunicationBoundaryConstraint
FaultDomainConstraint
```

### 4.6 Mobility plane

Owns:

```text
MobilityBoundary
MigrationUnit
MigrationClosure
MigrationPlan
MigrationPhase
MigrationClaim
CapabilityRebinding
SourceRetirement
```

### 4.7 Heterogeneous-realization plane

Owns:

```text
OperationRealization
DeviceRequirement
MemoryRequirement
ResourceCompatibilityClaim
RealizationPlacementBinding
```

### 4.8 Search/realization plane

Owns:

```text
Mapper
PartitioningProposal
PlacementSolverProposal
AdaptivePlacementProposal
MigrationProposal
```

These are untrusted or semi-trusted proposal mechanisms unless independently justified.

### 4.9 Assurance plane

Owns evidence relationships such as:

```text
TopologyEvidence
CapabilityEvidence
PlacementConstraintResult
FaultDomainSeparationClaim
MigrationWitness
PlacementCounterexample
PlacementDelta
TopologyDelta
MigrationDelta
```

through RFC 0018.

## 5. Place and location identity

### 5.1 Place

A `Place` is a semantic location domain that can host computation, data, or both.

It is intentionally more abstract than a physical host address.

Candidate shape:

```text
Place {
    id
    class
    topology_ref
    capability_summary?
    parent_places[]
    evidence?
}
```

A place might denote:

```text
cluster
node
socket
NUMA domain
GPU
accelerator
memory domain
trust enclave
virtual execution locale
runtime scheduling domain
```

### 5.2 PlaceClass

`PlaceClass` describes a category of placement domain without claiming that all members have identical capabilities.

Examples:

```text
NodePlace
NUMAPlace
DevicePlace
MemoryPlace
TrustPlace
VirtualPlace
```

A class label is not a capability proof.

### 5.3 Location identity is scoped

The string:

```text
"gpu0"
```

is not a complete location identity.

A location identity should include sufficient namespace/topology context:

```text
LocationIdentity {
    topology_id
    namespace_id
    local_id
}
```

This prevents accidental equality across different machines or topology snapshots.

### 5.4 Location equality does not imply resource equality forever

A virtual place may remain stable while its physical backing changes.

Therefore:

```text
VirtualPlaceIdentity
PhysicalResourceIdentity
```

remain separate RFC 0020 subjects.

## 6. Location observation and placement opacity

### 6.1 PlacementOpaque

A computation is placement-opaque when its declared observation model does not distinguish admissible placements.

Conceptually:

```text
PlacementOpaqueClaim {
    subject
    admissible_placement_set
    observation_model
    relation
    evidence
}
```

### 6.2 PlacementObservable

A program may explicitly observe:

```text
current place
current device identity
current NUMA domain
current trust domain
current memory domain
```

When this happens, Forge must preserve the observation or prove a relation under which the difference is acceptable.

### 6.3 `here` is domain-scoped

If MNCS eventually exposes a source-level location observation, it should be explicit about the domain:

```text
here<NodePlace>()
here<MemoryPlace>()
here<DevicePlace>()
```

rather than one universal `here` value.

### 6.4 Placement observation narrows realization freedom

The same transformation principle used by RFC 0019 applies:

```text
Unobserved placement
    -> broad realization freedom

Observed placement property
    -> narrowed realization envelope
```

## 7. Logical topology and physical topology

### 7.1 LogicalTopology

A logical topology describes application relationships.

Examples:

```text
pipeline
ring
mesh
stencil grid
tree
producer-consumer DAG
replica group
```

It is not a claim about hardware.

Candidate shape:

```text
LogicalTopology {
    nodes
    edges
    relation_types
    semantic_constraints
}
```

### 7.2 PhysicalTopology

A physical topology describes actual execution/storage resources and relationships.

Candidate shape:

```text
PhysicalTopology {
    resources
    containment_edges
    access_edges
    transfer_edges
    sharing_edges
    failure_edges
    trust_edges
}
```

### 7.3 TopologyMapping

A mapping binds logical entities to physical resources.

```text
TopologyMapping {
    logical_topology
    physical_topology
    node_mapping
    edge_realizations
    preserved_relations
    witness
}
```

The same logical topology may have many legal physical mappings.

### 7.4 Mapping relations are scoped

A mapping may preserve:

```text
adjacency
fault-domain separation
communication-boundary limits
trust-domain containment
direct-access requirements
```

without preserving another preference.

A single `mapping_valid=true` bit is insufficient unless it references the complete placement contract.

## 8. Physical topology as a resource graph

### 8.1 Why a graph

Containment hierarchies are useful:

```text
cluster
  -> node
    -> socket
      -> NUMA
        -> core
```

but modern systems also contain cross-cutting relations:

```text
GPU direct access to one memory domain
RDMA path between selected devices
shared LLC across selected cores
NVLink-like device interconnect
shared power/failure domain
shared trust boundary
```

Therefore the canonical physical model should support a graph.

### 8.2 TopologyEdge

Candidate edge kinds include:

```text
Contains
CanExecuteOn
CanAccess
CanWrite
CanTransferDirectly
SharesCache
SharesMemoryController
SharesPowerDomain
SharesFailureDomain
SharesTrustBoundary
ConnectedBy
NearUnder<MetricClass>
```

Detailed numeric bandwidth/latency belongs to RFC 0032.

### 8.3 Multi-view topology

The same resources may participate in different topology views:

```text
PhysicalContainmentTopology
MemoryAffinityTopology
NetworkTopology
FailureTopology
AdministrativeTopology
TrustTopology
```

The system should not force these into one tree.

### 8.4 Identifier non-semanticity

Resource numbers and names do not establish graph relationships.

```text
resource_id 1
resource_id 2
```

may be arbitrarily distant.

A proximity relation requires topology evidence.

## 9. Topology snapshots and staleness

### 9.1 TopologySnapshot

Topology is runtime/environment state.

Candidate shape:

```text
TopologySnapshot {
    id
    environment_id
    observed_at
    resources
    relations
    evidence
}
```

### 9.2 Dynamic topology

Changes can include:

```text
resource loss
resource addition
accelerator reset
network degradation
NUMA policy change
trust-state change
hotplug
thermal/power reconfiguration
```

### 9.3 Stale placement evidence

A placement claim should name the topology snapshot it relies on.

If the topology changes, RFC 0018 invalidation should target only dependent claims.

### 9.4 Topology truth versus observations

A runtime topology observation may be incomplete or uncertain.

Therefore a topology claim can be:

```text
SUPPORTED
REFUTED
CONFLICTED
UNSUPPORTED
```

under RFC 0018 rather than requiring one omniscient hardware map.

## 10. Placement requirements and preferences

### 10.1 PlacementRequirement

A requirement is mandatory for semantic admissibility.

Examples:

```text
requires capability F
must remain within trust domain T
must use direct-accessible memory
must be separated from R under HostFailure
must colocate with D
must not cross process boundary
```

Candidate shape:

```text
PlacementRequirement {
    subject
    predicate
    scope
    origin
    evidence_requirement?
}
```

### 10.2 PlacementPreference

A preference influences ranking but not validity.

Examples:

```text
prefer near D
prefer same NUMA domain
prefer avoid migration
prefer device class G
```

Candidate shape:

```text
PlacementPreference {
    subject
    predicate
    importance_model?
    origin
}
```

Any numeric preference weighting belongs primarily to RFC 0033.

### 10.3 Requirements may arise from other RFCs

A placement requirement can be generated by:

```text
RFC 0021 numeric relation
RFC 0023 timing requirement
RFC 0024 information-flow boundary
RFC 0025 attestation/trust requirement
RFC 0026 persistent-access requirement
RFC 0028 fault-domain or consistency requirement
```

The origin must remain machine-readable.

## 11. Affinity and anti-affinity

### 11.1 AffinityConstraint

```text
AffinityConstraint {
    subjects
    relation
    strength: requirement | preference
    topology_scope
}
```

Examples:

```text
SameNUMADomain(A, D)
Near(A, B)
SameDevice(A, D)
DirectAccess(A, D)
```

### 11.2 AntiAffinityConstraint

```text
AntiAffinityConstraint {
    subjects
    relation
    topology_scope
    failure_or_security_scope?
}
```

Examples:

```text
DifferentHost(R1, R2)
DifferentPowerDomain(R1, R2)
DifferentTrustDomain(X, Y)
DifferentDevice(A, B)
```

### 11.3 Affinity is relation-specific

`SameNode` does not imply `SameNUMADomain`.

`SameNUMADomain` does not imply `SameL2`.

`SameRack` does not imply `SameFailureDomain` if the failure model is power or network-specific.

No generic `colocated=true` should erase these distinctions.

## 12. Fault-domain placement

### 12.1 FaultDomain

A fault domain is a physical/administrative grouping relevant to a declared failure class.

Examples:

```text
process
host
power supply
rack
switch
availability zone
firmware family
shared hypervisor
```

### 12.2 FaultDomainSeparationClaim

Candidate shape:

```text
FaultDomainSeparationClaim {
    subjects
    failure_model
    topology_snapshot
    separation_relation
    evidence
}
```

### 12.3 Replica independence

RFC 0028 may require:

```text
IndependentUnder<HostFailure>
```

RFC 0029 provides spatial evidence that the selected replica placements satisfy that relation.

Three VMs on one host do not satisfy a three-host separation requirement.

### 12.4 Common-mode dependencies

Spatial independence may still fail for another reason:

```text
same firmware
same power source
same top-of-rack switch
same trust provider
```

RFC 0018 should appraise these dependencies.

RFC 0029 only supplies the physical/topological evidence it owns.

## 13. Security and trust-domain placement

### 13.1 Security-generated placement requirements

RFC 0024 policy can generate constraints such as:

```text
Secret S may execute only within PlaceClass TrustedEnclave
NoCrossTrustDomain(S)
```

### 13.2 Attestation-generated placement requirements

RFC 0025 may require:

```text
Destination must establish AttestationProfile V
```

before a task can be placed or migrated there.

### 13.3 Data sovereignty and administrative boundaries

RFC 0029 should reserve location predicates capable of representing:

```text
jurisdiction
administrative owner
tenant boundary
trust domain
```

without defining legal policy itself.

### 13.4 Security placement is not performance placement

A placement can be physically near yet semantically forbidden.

Optimization cannot override trust or information-flow requirements.

## 14. Location variables and unresolved placement

### 14.1 LocationVariable

Placement should be allowed to remain unresolved:

```text
Task T -> α
Data D -> β

Require:
    DirectAccess(α, β)
```

Candidate representation:

```text
LocationVariable {
    id
    admissible_place_class?
    constraints[]
}
```

### 14.2 Placement holes

A canonical program can preserve:

```text
placement = unresolved
```

through semantic/HIR stages until target/resource facts require realization.

### 14.3 Placement quantification

Useful claims include:

```text
exists placement P satisfying C
```

and stronger:

```text
for all admissible placements P satisfying C,
property Q holds
```

This allows RFC 0020 to distinguish:

```text
PlacementSpecific
PlacementIndependent
```

relations.

### 14.4 Placement SAT results

Constraint solving should preserve:

```text
SAT
UNSAT
UNKNOWN
```

`UNKNOWN` may arise from incomplete topology, solver bounds, or unresolved cross-RFC evidence.

## 15. Computation placement and data placement

### 15.1 ComputationPlacement

```text
ComputationPlacement {
    computation
    place
    realization?
    topology_snapshot
}
```

### 15.2 DataPlacement

```text
DataPlacement {
    logical_data
    physical_instance
    memory_resource
    representation
    topology_snapshot
}
```

### 15.3 Independent movement

For Task `T` and Data `D`, legal strategies may include:

```text
MoveTaskToData
MoveDataToTask
CreatePhysicalInstanceNearTask
ChooseDifferentTaskRealizationNearData
```

RFC 0029 must preserve their distinct identities.

### 15.4 Logical data versus physical instance

One logical data object can have multiple physical instances:

```text
D logical
  -> D_cpu
  -> D_gpu
```

RFC 0028 determines replicated-state semantics if independently evolving logical replicas exist.

RFC 0029 determines where physical instances live and which placements can access them.

## 16. Memory resources and accessibility

### 16.1 MemoryResource

A memory resource is not merely a size.

Relevant capability facts can include:

```text
CPU readable
CPU writable
GPU readable
GPU writable
RDMA reachable
persistent
coherent with resource X
directly addressable by resource Y
```

### 16.2 Device-local memory

A GPU framebuffer or accelerator-local memory is one realization of a `MemoryResource` with a particular capability set.

The language should not hard-code vendor memory names into the semantic ontology.

### 16.3 Accessibility requirement

A task can require:

```text
CanAccess(ExecutionResource, PhysicalInstance)
```

or stronger:

```text
DirectAccess(...)
```

### 16.4 Representation compatibility

A physical representation may be valid in one memory resource and invalid in another.

Therefore RFC 0019 representation selection can depend on RFC 0029 placement.

## 17. Capability-driven heterogeneous execution

### 17.1 Device class is descriptive

Classes such as:

```text
CPU
GPU
FPGA
DSP
NPU
```

are useful indexing terms but insufficient semantic requirements.

### 17.2 DeviceRequirement

Prefer requirements such as:

```text
supports operation O
supports numeric format F
supports atomic primitive A
minimum subgroup width N
minimum accessible memory M
required trust profile T
```

Candidate shape:

```text
DeviceRequirement {
    operation
    required_capabilities
    forbidden_capabilities?
    cross_rfc_obligations[]
}
```

### 17.3 Capability evidence

A target/resource capability fact is environment-scoped evidence:

```text
ResourceCapabilityEvidence {
    resource
    capability
    target_context
    observed_at
    evidence
}
```

### 17.4 Algorithm agility across devices

One logical operation may have multiple realizations:

```text
OperationRealization {
    logical_operation
    implementation
    required_device_capabilities
    required_memory_capabilities
    semantic_relation_claims
}
```

## 18. Heterogeneous realization identity

### 18.1 Logical operation versus device realization

```text
MatrixMultiply
```

can have:

```text
CPU_AVX_realization
GPU_kernel_realization
TensorAccelerator_realization
```

These have different implementation identities and placement requirements while potentially satisfying one RFC 0020 relation.

### 18.2 Numeric interaction

A device realization may narrow precision, rounding, accumulation, or reproducibility.

Therefore:

```text
DeviceCanExecute
```

is weaker than:

```text
DeviceRealizationSatisfies<NumericContract>
```

### 18.3 Timing interaction

A device may satisfy functionality but fail RFC 0023 deadlines.

That is not a placement-schema failure; it is a cross-RFC realization failure.

### 18.4 Security interaction

A device may satisfy functionality and timing but fail the required attestation or side-channel profile.

Again, placement admissibility is cross-RFC.

## 19. Joint realization search

Placement rarely varies independently from other realization dimensions.

MNCS should therefore permit joint search over combinations such as:

```text
Placement × Representation
Placement × NumericRealization
Placement × Schedule
Placement × Serialization
Placement × DistributedRealization
Placement × TrustRealization
```

RFC 0029 does not define the multi-objective optimizer.

It establishes which candidate tuples are semantically admissible.

## 20. Task graphs and dependency graphs

### 20.1 TaskGraph

A placement search can consume:

```text
TaskGraph {
    computations
    data_dependencies
    control_dependencies
    communication_relations
}
```

### 20.2 ResourceGraph

The physical candidate space can be represented as:

```text
ResourceGraph {
    execution_resources
    memory_resources
    topology_relations
    capability_relations
}
```

### 20.3 Placement as mapping

The search problem becomes:

```text
TaskGraph -> ResourceGraph
```

subject to hard semantic constraints.

### 20.4 Search mechanism independence

Mapping can be proposed by:

```text
graph partitioner
hypergraph partitioner
constraint solver
integer program
HEFT-like heuristic
work-stealing runtime
learned model
LLM/agent
custom mapper
```

The proposal mechanism is not the semantic authority.

## 21. Graph and hypergraph partitioning theory

### 21.1 Graph partitioning

Task/data dependencies can be approximated as a graph and divided across placement domains while minimizing cut relations or balancing capacity.

### 21.2 Hypergraph partitioning

Shared data consumed by many tasks can be represented more faithfully as a hyperedge:

```text
D -> {T1, T2, T3, T4}
```

rather than multiple independent pairwise edges.

### 21.3 Partitioning output is a proposal

A partitioner may optimize:

```text
cut count
balance
locality proxy
```

but cannot itself establish semantic placement validity unless its result is checked against the placement contract.

## 22. Communication locality

### 22.1 Communication affinity

If A and B communicate heavily, a preference may be generated:

```text
PreferNear(A, B)
```

### 22.2 Communication boundary requirements

A hard requirement may instead say:

```text
NoCrossNodeCommunication(A, B)
```

or:

```text
NoCrossTrustBoundary(A, B)
```

### 22.3 CommunicationFree

RFC 0029 should reserve a relation such as:

```text
CommunicationFree<BoundaryClass>
```

for regions that must not perform remote or cross-boundary communication.

### 22.4 Verification

A placement checker can combine placement with the task/data graph to prove or refute such a property.

## 23. Placement and RFC 0028 distribution semantics

### 23.1 Crossing a failure boundary

Suppose A and B originally share local memory.

A placement candidate moves B to another host.

The interaction now faces:

```text
message transport
partial failure
remote-outcome uncertainty
possibly serialization
```

### 23.2 Distributed refinement obligation

The placement is valid only if the generated RFC 0028 realization preserves the required relation.

Candidate artifact:

```text
BoundaryCrossingObligation {
    original_interaction
    new_boundary
    required_distributed_relation
}
```

### 23.3 Locality can remove distribution overhead only when semantics permit

The reverse can also happen: two distributed participants may be colocated and realized more cheaply.

But the local lowering must preserve any observable RFC 0028 semantics still required by the program.

## 24. Placement and RFC 0027 serialization

### 24.1 Boundary-generated serialization

Placement can introduce a representation boundary:

```text
same address space -> no wire encoding
separate process -> serialization
GPU/device copy -> device transfer encoding/layout conversion
remote node -> RFC 0027 wire contract
```

### 24.2 Serialization is not automatically required by every movement

Some shared-memory or zero-copy placements may avoid it.

The placement realization should generate only the necessary RFC 0027 obligations.

### 24.3 Canonicality may matter at movement boundaries

If data crossing a boundary is signed, cached by canonical digest, or schema-versioned, migration/placement must preserve the relevant RFC 0027 identity and compatibility claims.

## 25. Placement and RFC 0021 numeric semantics

Device placement can change:

```text
precision
rounding
fused operations
accumulation order
supported exceptional-value behavior
```

Therefore a placement candidate may be functionally executable while numerically invalid.

The placement checker should consume existing RFC 0021 realization claims rather than reinventing numerical verification.

## 26. Placement and RFC 0023 time/scheduling

### 26.1 Where versus when

RFC 0023 primarily establishes temporal contracts and schedules.

RFC 0029 establishes physical execution/resource location.

Joint realization can be represented as:

```text
SpatiotemporalRealization {
    placement
    schedule
}
```

### 26.2 Migration across clock domains

Movement may require:

```text
new clock mapping
new uncertainty evidence
new deadline feasibility evidence
```

### 26.3 Placement can invalidate schedule witnesses

A schedule proven on one processor/device set cannot automatically apply after remapping.

## 27. Placement and RFC 0024 information flow

### 27.1 Physical placement can create channels

Moving code/data can change:

```text
shared cache exposure
memory-access observers
network metadata
co-tenancy
device side channels
```

### 27.2 Security relation preservation

A placement candidate requiring a stronger observer model must regenerate the relevant RFC 0024 claims.

### 27.3 Information-flow labels survive movement

Moving a serialized or in-memory secret does not declassify it.

The destination must satisfy the same or stronger applicable flow policy unless authorized reclassification occurs.

## 28. Placement and RFC 0025 trust/attestation

### 28.1 Destination identity

A migration or placement needs a destination principal/resource identity.

### 28.2 Attestation does not transfer implicitly

Evidence that Worker A runs approved state does not prove Worker B does.

### 28.3 Trust-domain constraint

Candidate placement can require:

```text
WithinTrustDomain(T)
```

or:

```text
AttestationProfile(P)
```

### 28.4 Credential/capability locality

Credentials or capabilities may be host-bound or device-bound.

Movement must not assume portability.

## 29. Placement and RFC 0026 persistence

### 29.1 Persistent reachability

A persistent reference or storage resource may be reachable only from some places.

### 29.2 Migration and persistent state

Moving an active computation that owns durable state may require:

```text
state transfer
shared durable access
new storage realization
```

RFC 0026 defines the crash consistency of the state transition.

### 29.3 Spatial migration is not durable-state migration by itself

RFC 0029 describes where the state instance moves.

RFC 0026 remains authoritative for whether the transfer is atomic/durable/recoverable.

## 30. Static, dynamic, and adaptive placement

### 30.1 StaticPlacement

Resolved before execution and expected not to change within a declared scope.

### 30.2 DynamicPlacement

Resolved at runtime from current topology/resource facts.

### 30.3 AdaptivePlacement

Continuously or periodically reconsidered based on new evidence.

### 30.4 Adaptive placement is a refinement loop

Conceptually:

```text
observe topology/resource state
localize stale placement evidence
propose candidate placement/migration
verify constraints
compare preferences
promote or reject
```

This integrates naturally with RFC 0004.

## 31. Work stealing and dynamic task ownership

### 31.1 Logical ownership versus execution placement

A task can retain logical identity while its execution placement changes.

### 31.2 Work stealing

Work stealing is one realization in which idle workers acquire ready tasks dynamically.

It should not become the language's parallel semantics.

### 31.3 Placement-sensitive tasks

A task with hard device/locality requirements may restrict which workers are allowed to steal it.

### 31.4 Placement witness under dynamic scheduling

A runtime can produce per-execution placement evidence rather than a single compile-time mapping.

## 32. Virtual places and execution slots

### 32.1 VirtualPlace

A stable runtime-facing location abstraction can be backed by changing physical resources.

### 32.2 ExecutionSlot

A scheduler may expose an execution slot that maps onto one or more physical resources over time.

### 32.3 Identity separation

```text
VirtualPlaceIdentity
ExecutionSlotIdentity
PhysicalResourceIdentity
```

remain separate RFC 0020 subjects.

### 32.4 Why this matters

It allows placement contracts to stay stable while runtimes adapt hardware assignment.

## 33. Mobility theory

### 33.1 MobilityBoundary

A mobility boundary identifies the state and authority that conceptually move together.

```text
MobilityBoundary {
    entity
    included_state
    excluded_state
    capabilities
    external_bindings
}
```

### 33.2 MigrationUnit

Potential units include:

```text
Task
Actor
Object
Continuation
Process
AddressSpace
Service
VM-like environment
DataRegion
```

### 33.3 Mobility granularity is semantic

Moving a task is not equivalent to moving a whole address space.

The RFC must retain the unit identity.

### 33.4 Passive versus active movement

```text
PassiveStateMigration
ActiveComputationMigration
```

have different closure and cutover obligations.

## 34. Migration closure

### 34.1 Why closure matters

A running unit can depend on:

```text
stack/continuation
heap state
mailbox
pending messages
timers
capabilities
credentials
file handles
persistent references
shared data
runtime state
```

### 34.2 MigrationClosure

Candidate shape:

```text
MigrationClosure {
    migration_unit
    moved_objects
    retained_remote_bindings
    rebound_objects
    invalidated_objects
}
```

### 34.3 Reachability is not closure

An object reachable from a moving task may remain behind and become a remote reference.

Conversely, a non-directly-reachable runtime dependency may still need rebinding.

Therefore automatic graph reachability alone cannot define closure.

## 35. Migration state machine

### 35.1 MigrationPlan

```text
MigrationPlan {
    entity
    source
    destination
    closure
    transfer_strategy
    cutover_strategy
    authority_strategy
    rollback_policy
    obligations
}
```

### 35.2 Migration phases

At minimum reserve:

```text
PLANNED
TRANSFER_STARTED
STATE_TRANSFERRED
CUTOVER_PENDING
DESTINATION_ACTIVE
SOURCE_RETIRED
ROLLED_BACK
FAILED
UNKNOWN
```

### 35.3 Live migration

A live migration may permit source execution during part of state transfer before a final cutover.

The RFC does not require a particular pre-copy or post-copy algorithm.

### 35.4 CutoverPoint

A `CutoverPoint` establishes which physical realization is authoritative for the logical entity under a declared mutation/observer relation.

## 36. Multiple physical instances during migration

### 36.1 Temporary duplication

During transfer:

```text
source copy
and
destination copy
```

can coexist.

### 36.2 Logical singularity

This does not necessarily mean the logical entity became a replicated state machine.

### 36.3 Mutation authority

The system must specify whether:

```text
source only mutable
destination only mutable
both read-only
replicated/coordinated mutation
```

is permitted during each phase.

### 36.4 Split-brain prevention

If the mobility contract requires one active writer, cutover needs a fencing/epoch or equivalent authority discipline compatible with RFC 0028.

## 37. Capability rebinding during migration

### 37.1 Outcomes

For each capability held by the moving unit, migration can produce:

```text
PreservedCapability
ReboundCapability
UnavailableCapability
InvalidatedCapability
```

### 37.2 Rebinding is explicit

A host-local device handle cannot silently become a destination device handle.

### 37.3 Authority cannot broaden

Migration must not create stronger capability authority than the source possessed unless an independent RFC 0025/RFC 0008 authority path grants it.

### 37.4 Rebinding evidence

A new capability binding may require:

```text
principal authentication
destination resource evidence
trust policy
freshness
```

## 38. Migration and evidence invalidation

Migration can invalidate or stale:

```text
TopologyEvidence
TargetCapabilityEvidence
AttestationEvidence
ClockMappingEvidence
ScheduleWitness
PersistentReferenceValidity
SideChannelClaim
LocalityClaim
FaultDomainSeparationClaim
SerializationBoundaryClaim
```

A `MigrationDelta` should enumerate these consequences.

Candidate shape:

```text
MigrationDelta {
    entity
    source
    destination
    preserved_claims
    invalidated_claims
    new_obligations
    authority_changes
}
```

## 39. Migration transparency

### 39.1 No universal transparency bit

A migration can preserve:

```text
functional behavior
logical identity
application-visible state
```

while changing:

```text
timing
failure exposure
location observations
network metadata
resource availability
```

### 39.2 MigrationPreserves

Use RFC 0020 relation machinery:

```text
MigrationPreserves<Relation, ObservationModel>
```

### 39.3 Strong transparency is expensive

The RFC should not demand all migrations preserve every observation class.

The program declares what is protected.

## 40. Migration rollback

### 40.1 Placement rollback

A failed migration may restore the source placement.

### 40.2 Not historical rollback

If the destination performed external effects before rollback, those effects remain part of history unless separately compensated.

### 40.3 Unknown migration outcome

If source and destination lose communication during cutover, some observer may legitimately have:

```text
MigrationOutcome = UNKNOWN
```

until fencing/state inspection resolves authority.

## 41. Placement history and provenance

### 41.1 PlacementHistory

Some systems need an auditable sequence:

```text
Task T:
  P1
  -> P2
  -> P3
```

### 41.2 Why history matters

Potential uses:

```text
security assurance
reproducibility
debugging
performance analysis
fault diagnosis
billing/accounting later
```

### 41.3 Provenance integration

RFC 0009 owns general provenance.

RFC 0029 contributes spatial history nodes/edges.

## 42. Placement reproducibility

### 42.1 PlacementDeterministic

The same input/topology leads to the same mapping.

### 42.2 PlacementReproducible

Repeated execution uses the same or relation-equivalent placement under a declared topology class.

### 42.3 PlacementIndependentResult

Different placements preserve a declared output/trace relation.

### 42.4 Distinct claims

These must remain separate.

A program can have nondeterministic placement but deterministic results.

A fixed placement can still produce nondeterministic behavior.

## 43. Topology adaptation

### 43.1 TopologyDelta

```text
TopologyDelta {
    before
    after
    added_resources
    removed_resources
    changed_relations
    invalidated_evidence
}
```

### 43.2 Localized re-placement

A topology change should trigger re-evaluation of entities whose placement claims depend on changed resources/relations.

### 43.3 No unnecessary global remap

RFC 0004 refinement should prefer defensible causal slices rather than remapping all work by default.

## 44. Placement satisfiability

### 44.1 Constraint set

A placement problem may include:

```text
capability constraints
colocation
anti-affinity
trust boundaries
fault separation
memory accessibility
remote-boundary restrictions
persistent-reference reachability
```

### 44.2 SAT

A witness exists that satisfies all required predicates.

### 44.3 UNSAT

No admissible mapping exists under the current topology/assumptions.

### 44.4 UNKNOWN

The solver, evidence, topology, or bounded search cannot decide.

### 44.5 Explanation slices

An UNSAT result should provide a machine-readable conflict set where feasible.

Example:

```text
GPU candidates = {A, B}
Trusted-enclave candidates = {C}
Required intersection = ∅
```

## 45. Placement search versus placement checking

### 45.1 Untrusted search

Forge may use:

```text
heuristics
constraint solvers
SMT
MILP
graph partitioners
learned models
LLMs
evolutionary search
runtime feedback
```

### 45.2 Narrow checking

A candidate placement produces a `PlacementWitness` that can be checked by a much smaller verifier.

### 45.3 Checker obligations

The checker should validate at least:

```text
resource existence
capability requirements
hard affinity/anti-affinity
fault-domain requirements
memory accessibility
trust restrictions
boundary-generated obligations
migration validity
cross-RFC protected claims
```

### 45.4 Search may fail without refuting satisfiability

An unsuccessful heuristic search may produce:

```text
UNKNOWN
```

not `UNSAT`.

Only a suitable proof or exhaustive bounded domain can establish `UNSAT`.

## 46. PlacementWitness

Candidate shape:

```text
PlacementWitness {
    contract
    topology_snapshot
    entity_to_place
    data_to_memory
    realization_bindings
    satisfied_requirements
    generated_obligations
    unresolved_preferences
    dependencies
}
```

A witness is not necessarily optimal.

It establishes admissibility under its claim scope.

## 47. PlacementCounterexample

A failed placement should identify the violated spatial relation.

```text
PlacementCounterexample {
    candidate
    requirement
    offending_entities
    topology_slice
    related_cross_rfc_claim?
}
```

Examples:

```text
Task requires FP64 but device lacks supported realization
Replicas share host failure domain
Secret moved outside trusted placement class
Task separated from data despite DirectAccess requirement
Remote placement violates local-only atomic interaction
```

## 48. PlacementDelta

Forge transformations should expose changes such as:

```text
entity moved
new device realization selected
data instance created/deleted
new serialization boundary
new remote interaction
new trust domain
fault-domain independence strengthened/weakened
new migration obligation
```

Candidate shape:

```text
PlacementDelta {
    before
    after
    moved_entities
    changed_affinities
    changed_fault_domains
    changed_boundaries
    changed_realizations
    new_obligations
    invalidated_evidence
}
```

## 49. Lowering preservation

### 49.1 HIR/SSA identity

Location variables, placement requirements, and protected spatial relations should retain stable identities through lowering where applicable.

### 49.2 Backend specialization

A backend may specialize a resolved placement:

```text
CPU code path
GPU kernel
remote message path
```

but must preserve the relevant semantic relation.

### 49.3 Optimizations cannot erase placement obligations

For example, an optimizer may not:

```text
remove a required trust-domain boundary check
move code into a device lacking required numeric semantics
collapse anti-affinity replicas onto one host
```

because ordinary functional output looked unchanged in a bounded run.

## 50. Machine-native micro-verifier opportunities

RFC 0029 should explicitly encourage narrow verifiers for:

```text
PlacementConstraintChecker
CapabilityCompatibilityChecker
TopologyRelationChecker
FaultDomainSeparationChecker
DirectAccessChecker
CommunicationFreeChecker
BoundaryCrossingObligationChecker
MigrationClosureChecker
MigrationAuthorityChecker
MigrationEvidenceInvalidationChecker
LocationObservationPreservationChecker
PlacementRelationChecker
```

The search/planning system may be large and heuristic.

These checkers should remain small where feasible.

## 51. Placement relation families

Useful RFC 0020 relations include:

```text
PlacementEquivalent
PlacementRefines
PlacementInsensitiveEquivalent
PlacementObservationEquivalent
TopologyMappingPreserves<R>
MigrationPreserves<R>
FaultDomainSeparatedUnder<F>
DeviceCompatibleUnder<C>
CommunicationFreeUnder<B>
```

The same candidate pair may satisfy one and fail another.

## 52. Bounded placement testing

Empirical experiments can vary:

```text
topology snapshots
resource removals
migration sequences
placement candidates
device assignments
fault-domain mappings
```

Results remain:

```text
BoundedPlacementEvidence
```

unless stronger proof methods justify a universal claim.

## 53. First bounded implementation pilot

The first implementation pilot should introduce versioned artifacts for:

```text
Place
PlaceClass
TopologySnapshot
TopologyRelation
ExecutionResource
MemoryResource
ResourceCapabilitySet
LocationVariable
ComputationPlacement
DataPlacement
PlacementRequirement
PlacementPreference
AffinityConstraint
AntiAffinityConstraint
FaultDomainConstraint
MobilityBoundary
MigrationPlan
MigrationPhase
MigrationClaim
CapabilityRebinding
OperationRealization
PlacementWitness
PlacementCounterexample
PlacementDelta
TopologyDelta
MigrationDelta
```

The pilot should remain deliberately small rather than implementing a full cluster scheduler.

## 54. Pilot fixture A — identity survives placement change

Initial:

```text
Task T @ CPU0
```

Then:

```text
Task T @ CPU3
```

Required result:

```text
TaskIdentity(T_before, T_after) = preserved
PlacementIdentity(before, after) = different
```

The system must not create a new logical task merely because it moved.

## 55. Pilot fixture B — placement observation narrows equivalence

Two placements produce identical return values.

Program does not observe location:

```text
PlacementInsensitiveEquivalent = PASS
```

Then add:

```text
observe current_device
```

Now:

```text
PlacementObservationEquivalent = FAIL
```

while the narrower functional relation may still pass.

## 56. Pilot fixture C — preference versus requirement

Candidate A:

```text
Task far from Data D
```

Candidate B:

```text
Task near Data D
```

Under:

```text
PreferNear(T, D)
```

both are admissible and B ranks better.

Change to:

```text
RequireDirectAccess(T, D)
```

A becomes invalid.

## 57. Pilot fixture D — topology IDs do not imply proximity

Resources:

```text
CPU0
CPU1
CPU2
```

Topology states:

```text
Near(CPU0, CPU2)
not Near(CPU0, CPU1)
```

A mapper that infers numeric adjacency must be rejected.

## 58. Pilot fixture E — fault anti-affinity

Three service replicas are placed on three VMs.

Topology reveals all three VMs share one physical host.

Required result:

```text
ReplicaCount = 3
IndependentUnder<HostFailure> = REFUTED
```

Then place them on three host failure domains and establish the separation witness.

## 59. Pilot fixture F — capability-based heterogeneous placement

Task requirement:

```text
FP64
Atomic64
AccessibleMemory >= 16 GiB
```

Resources:

```text
GPU A: lacks Atomic64
GPU B: satisfies all
CPU C: satisfies all
```

The system must reject A but retain B and C as admissible despite different device classes.

## 60. Pilot fixture G — unresolved location variables

```text
Task T -> α
Data D -> β
Require DirectAccess(α, β)
```

Forge resolves one valid pair from a topology snapshot.

A second topology removes the compatible memory/resource edge and the same constraint set becomes `UNSAT` or `UNKNOWN` depending on available evidence.

## 61. Pilot fixture H — move task versus move data

Two valid realizations:

```text
R1: MoveTaskToData
R2: MoveDataToTask
```

Both preserve functionality.

The RFC 0029 checker establishes semantic admissibility only.

RFC 0033 may later rank them by cost.

## 62. Pilot fixture I — migration invalidates attestation

Initial:

```text
Task T -> Worker A
Worker A has current acceptable attestation
```

Migration:

```text
T -> Worker B
```

Required result:

```text
A's attestation cannot establish B's platform state
```

A new RFC 0025 attestation obligation is generated.

## 63. Pilot fixture J — migration capability rebinding

Task holds:

```text
LocalDeviceCapability<DeviceA>
```

Migration moves task away from DeviceA.

Valid outcomes:

```text
explicit rebind to authorized DeviceB
or
reject migration
```

Invalid outcome:

```text
silently treat old capability as DeviceB authority
```

## 64. Pilot fixture K — crossing a distributed boundary

Initial:

```text
A and B share one local state region
```

Candidate placement:

```text
A -> Node1
B -> Node2
```

If no RFC 0028 realization can preserve the required local atomic relation, placement must be rejected.

This demonstrates that placement is not always correctness-orthogonal.

## 65. Pilot fixture L — communication-free region

A region is declared:

```text
CommunicationFree<NodeBoundary>
```

Initial placement keeps all needed data local and passes.

Moving one required datum remote produces a concrete placement counterexample.

## 66. Pilot fixture M — migratable logical object

Object O moves:

```text
Node A -> Node B
```

Preserve:

```text
ObjectIdentity
LogicalState
```

Invalidate or refresh:

```text
TimingEvidence
LocationObservation
AttestationEvidence
```

This demonstrates relation-scoped migration transparency.

## 67. Pilot fixture N — migration cutover and fencing

Source instance uses epoch 12.

Destination activation establishes epoch 13.

Any source write with epoch 12 after cutover is rejected.

This prevents a migration-induced split brain and composes with RFC 0028 fencing semantics.

## 68. Pilot fixture O — stale topology evidence

A placement witness is valid under topology revision 7.

Revision 8 removes the direct-access edge used by Task T.

Only T and dependent placement claims become stale.

Unrelated placements remain valid.

## 69. Pilot fixture P — numeric-invalid accelerator

CPU and GPU candidates both execute the kernel.

GPU realization fails the required RFC 0021 numeric relation due to insufficient accumulation semantics.

The GPU placement is rejected even if empirical timing is better.

## 70. Pilot fixture Q — placement introduces serialization

Initial colocated placement passes values directly.

New placement crosses a process boundary and generates an RFC 0027 serialization obligation.

A candidate codec that loses required presence/unknown-field semantics causes the placement realization to fail.

## 71. Pilot fixture R — placement-independent result

Run the same computation over several admissible placements.

Use bounded execution evidence to establish only:

```text
BoundedAgreementUnder<FunctionalObservation>
```

Do not upgrade this to universal placement independence without stronger proof.

## 72. First Forge placement study

A bounded heterogeneous pipeline should include:

```text
Decode
Transform
MatrixKernel
Verify
```

Data:

```text
Input
Matrix
Result
```

Constraints:

```text
MatrixKernel requires NumericRelation R
MatrixKernel requires direct-accessible Matrix storage
Verify requires AttestationProfile V
Result confidentiality must remain within FlowPolicy S
Decode prefers near Input
placement unresolved
```

Topology:

```text
CPU socket A
CPU socket B
NUMA0
NUMA1
GPU0
GPU1
GPU0 memory
GPU1 memory
trusted verifier place
```

Forge considers candidates.

Candidate A:

```text
MatrixKernel -> CPU
```

Semantically valid, perhaps slower.

Candidate B:

```text
MatrixKernel -> GPU0
```

Fast but fails NumericRelation R.

Reject.

Candidate C:

```text
MatrixKernel -> GPU1
```

Passes numeric and memory constraints.

Candidate D:

```text
Verify -> untrusted place
```

Reject despite favorable performance.

The study demonstrates semantic placement synthesis rather than raw device scheduling.

## 73. Second Forge study — adaptive migration

Initial:

```text
Task T -> Worker A
Data D -> Worker A
```

New environment evidence makes Worker B preferable.

Forge proposes:

```text
MigrationPlan(T, A -> B)
```

The verifier discovers:

```text
D remains at A
clock domain changes
attestation must refresh
one local capability cannot migrate
```

Forge then considers:

```text
move D
create new D instance
rebind capability
obtain new attestation
or reject migration
```

Only a candidate satisfying all generated obligations may be promoted.

## 74. Third Forge study — replica independence

RFC 0028 contract:

```text
three replicas
must tolerate one host failure
```

Placement search proposes several mappings.

A fast candidate puts all replicas on one high-capacity server.

Reject.

A slower candidate separates them across independent host failure domains.

Accept.

This demonstrates that topology is part of fault-tolerance evidence.

## 75. Theoretical synthesis

RFC 0029 intentionally combines ideas from several bodies of work without adopting any one system as the ontology.

### 75.1 Legion logical regions and mapping independence

Legion demonstrates a powerful separation between logical task/data structure and physical processor/memory mapping. MNCS adopts the architectural lesson: mapping should remain an independently selected realization whenever correctness semantics permit it.

Reference direction:

- https://legion.stanford.edu/publications/
- https://legion.stanford.edu/overview/index.html

### 75.2 Realm resource and affinity graphs

Realm's processor/memory machine model motivates representing heterogeneous processors, memories, accessibility, and affinity as a graph rather than reducing a machine to one processor hierarchy.

Reference direction:

- https://legion.stanford.edu/tutorial/realm/machine_model.html

### 75.3 MPI virtual topology

MPI distinguishes logical/virtual process topology from physical machine topology and allows runtimes to use that logical topology in mapping decisions.

Reference direction:

- https://www.mpi-forum.org/docs/mpi-4.1/mpi41-report/node219.htm

### 75.4 hwloc topology and distance models

hwloc motivates explicit hierarchical locality, NUMA/cache/device topology, and non-tree distance relationships while warning against treating resource numbering as proximity.

Reference direction:

- https://www.open-mpi.org/projects/hwloc/doc/v2.10.0/a00335.php

### 75.5 Chapel locales

Chapel reifies a useful architecture-facing `locale` concept for task and data locality while making current locale observable. MNCS adopts the lesson that location can be explicit without forcing raw coordinates into every program.

Reference direction:

- https://chapel-lang.org/docs/language/spec/locales.html
- https://chapel-lang.org/docs/technotes/local.html

### 75.6 X10 places, distributions, and place types

X10 demonstrates explicit distributed places and distributions and motivates static/dependent locality information for references.

Reference direction:

- https://research.ibm.com/publications/x10-concurrent-programming-for-modern-architectures
- https://research.ibm.com/publications/type-inference-for-locality-analysis-of-distributed-data-structures

### 75.7 Mobile Ambients

Mobile Ambients provide a theory of nested spatial/administrative boundaries and explicit movement of computation across them. MNCS adopts bounded-location and mobility-boundary ideas without adopting ambient calculus syntax.

Reference direction:

- https://www.microsoft.com/en-us/research/wp-content/uploads/2016/11/fossacs98.pdf

### 75.8 Emerald object mobility

Emerald demonstrates fine-grained object mobility and the possibility of preserving logical identity while active/passive objects move among hosts.

Reference direction:

- https://lazowska.cs.washington.edu/emerald.pdf

### 75.9 Sprite process migration

Sprite demonstrates transparent-ish process migration, residual home-node dependencies, and the difference between moving computation and moving all external resource bindings.

Reference direction:

- https://www2.eecs.berkeley.edu/Pubs/TechRpts/1987/5363.html
- https://www2.eecs.berkeley.edu/Pubs/TechRpts/1990/6382.html

### 75.10 Live virtual-machine migration

Live VM migration motivates phased transfer, cutover, source retirement, and temporary dual physical copies during movement.

Reference direction:

- https://www.cl.cam.ac.uk/research/srg/netos/papers/2005-nsdi-migration.pdf

### 75.11 Charm++ migratable objects

Charm++ motivates overdecomposition into migratable logical work units and runtime remapping for adaptive load/locality behavior.

Reference direction:

- https://charm.cs.illinois.edu/papers/06-18
- https://charm.cs.illinois.edu/papers/04-13

### 75.12 Cilk work stealing

Cilk motivates dynamic task reassignment as a runtime strategy that preserves logical task identity while changing execution placement.

Reference direction:

- https://people.csail.mit.edu/bradley/papers/BlumofeJoKu96.pdf

### 75.13 StarPU heterogeneous scheduling

StarPU demonstrates task/codelet realizations for different processor architectures, runtime scheduling, and automated data movement across heterogeneous memory domains.

Reference direction:

- https://starpu.gitlabpages.inria.fr/
- https://starpu.gitlabpages.inria.fr/features.html

### 75.14 SYCL aspect/capability selection

SYCL motivates device filtering and selection based on supported aspects/capabilities rather than only device names.

Reference direction:

- https://github.khronos.org/SYCL_Reference/iface/device-selector.html
- https://registry.khronos.org/SYCL/specs/sycl-2020/html/sycl-2020.html

### 75.15 Graph and hypergraph partitioning

Graph/hypergraph partitioning provides realization techniques for mapping dependency structures onto resource partitions while balancing capacity and minimizing communication cuts.

Reference direction:

- https://conservancy.umn.edu/bitstreams/4450f49b-2783-4a4e-b1a9-65cda2b403ac/download

### 75.16 HEFT-style heterogeneous DAG scheduling

HEFT demonstrates a practical heuristic for mapping precedence-constrained work onto heterogeneous processors. MNCS treats it as an untrusted proposal strategy, not a semantic guarantee.

Reference direction:

- https://www.researchgate.net/publication/3300636_Performance-effective_and_low-complexity_task_scheduling_forheterogeneous_computing

## 76. Machine-native contribution beyond the precedents

The main MNCS contribution is not another scheduler.

It is a semantic separation among:

```text
what may run where
what must run where
what prefers to run where
what data must reside where
what must remain together
what must remain separate
what may move
what movement preserves
what evidence movement invalidates
which device realizations remain semantically legal
```

and the actual search algorithm used to find a placement.

A program can therefore express:

```text
Computation {
    requires:
        CapabilitySet C
        NumericRelation N
        TrustProfile T

    locality:
        require DirectAccess(Data D)
        prefer Near(Data E)

    separation:
        require DifferentFailureDomain(Replica R)

    mobility:
        permitted under MigrationContract M

    placement:
        unresolved
}
```

without naming a CPU/GPU/node prematurely.

## 77. Search/check architecture

Forge may propose placement using a sophisticated system:

```text
heuristics
historical performance
AI/ML models
constraint solvers
runtime telemetry
```

But the candidate must still reduce to a checkable statement:

```text
entity X -> place P
physical instance D -> memory M
realization R -> device Q
```

plus evidence that all hard requirements and generated cross-RFC obligations hold.

This preserves the broader MNCS trust architecture:

> powerful generation and search outside the trust boundary; narrow deterministic checking inside it where feasible.

## 78. Scope boundaries

### RFC 0008

Owns execution/resource/memory/network effects and capability authority.

RFC 0029 may require or rebind capabilities due to placement but does not redefine their authority model.

### RFC 0009

Owns references, provenance, storage identity, and reference semantics.

RFC 0029 determines spatial accessibility and migration effects on those references.

### RFC 0010

Owns concurrency/causality/atomicity.

RFC 0029 may change the physical resources executing concurrent operations but does not redefine concurrency semantics.

### RFC 0011

Owns generic failure/recovery semantics.

RFC 0029 distinguishes movement, resource loss, and placement failure consequences.

### RFC 0014

Owns component/dependency/linking compatibility.

Placement compatibility is a different relation family.

### RFC 0015

Owns unsafe/foreign boundaries.

Moving across a foreign runtime/device boundary can generate RFC 0015 obligations.

### RFC 0017

Owns runtime/target/environment facts.

Topology snapshots and device capability evidence depend on RFC 0017 environment identities.

### RFC 0018

Owns appraisal of topology, capability, placement, migration, independence, and freshness evidence.

### RFC 0019

Owns logical value/type and representation realization.

RFC 0029 determines where physical representations may reside and which resources can use them.

### RFC 0020

Owns identity/equality/equivalence/refinement/compatibility relations used for placement and migration claims.

### RFC 0021

Owns numeric semantics.

Placement cannot authorize a weaker numeric realization without an RFC 0021 relation.

### RFC 0022

Owns progress/termination/bounded computation.

Placement can affect resource availability but does not redefine liveness semantics.

### RFC 0023

Owns time/clocks/deadlines/schedules.

RFC 0029 owns location; joint spatiotemporal realizations reference both.

### RFC 0024

Owns confidentiality/integrity/side-channel semantics.

Placement can create security constraints or new observer channels.

### RFC 0025

Owns principal identity, credentials, attestation, and trust.

Placement/migration consumes destination identity/trust evidence.

### RFC 0026

Owns local durable-state/crash-consistency semantics.

RFC 0029 does not make state transfer durable by naming a destination.

### RFC 0027

Owns serialization/schema/wire contracts.

Placement can generate serialization boundaries.

### RFC 0028

Owns messaging, partial failure, distributed consistency, replication, consensus, and distributed transactions.

RFC 0029 owns where those participants/replicas physically reside and whether placement satisfies failure/locality constraints.

### RFC 0030

Will own deployment lifecycle, version coexistence, live software upgrade, and state-version migration.

Spatial migration remains RFC 0029 even when a future workflow combines the two.

### RFC 0031

Will own build/artifact lineage and supply-chain semantics.

### RFC 0032

Will own quantitative latency, bandwidth, memory, energy, migration cost, occupancy, and performance accounting.

### RFC 0033

Will own multi-objective realization optimization/Pareto/risk/budget policy over semantically valid candidates.

## 79. Non-goals

RFC 0029 does not attempt to:

- define a complete production scheduler;
- standardize one hardware topology discovery API;
- mandate one cluster resource manager;
- mandate GPUs or any accelerator class;
- choose a graph partitioning algorithm;
- choose a work-stealing algorithm;
- define quantitative performance/cost models;
- define deployment/live-upgrade semantics;
- define distributed consistency itself;
- define wire serialization itself;
- define numeric correctness itself;
- define trust/attestation itself;
- promise transparent migration under every observation model;
- assume all programs are migratable;
- assume placement can always be changed without semantic consequences.

## 80. Open research questions

The following remain intentionally unresolved:

1. Whether `Place` should be a primitive semantic node or a derived view over resource/topology facts.
2. How much locality information should be available statically versus dynamically.
3. Whether location variables should participate directly in the dependent type core or remain contract-level artifacts initially.
4. Which topology edge vocabulary is minimal enough for the semantic core.
5. How to represent multiple simultaneous topology views without creating redundant graph identity.
6. How best to express topology uncertainty and partial observation.
7. Whether mobility boundaries should be nominal, structural, or mixed.
8. Which migration phases deserve canonical identity versus realization-specific detail.
9. How to model migratable continuations without prematurely fixing runtime stack representation.
10. How to preserve proof/evidence references when representations and locations change jointly.
11. Which placement properties should be kernel-checkable versus certificate-checkable.
12. How to expose placement-sensitive source syntax without encouraging hard-coded coordinates.
13. How to combine placement and scheduling search without collapsing RFC 0023/0029 boundaries.
14. How to express jurisdiction/administrative constraints without embedding legal interpretation into the language.
15. How to model device reset or partial accelerator failure as a topology versus failure event.
16. How to model shared accelerators where isolation/failure domains are virtual rather than physical.
17. How to prove migration closure for foreign resources.
18. How to represent topology-independent versus topology-parametric programs compactly.
19. How Forge should cache placement evidence across similar topology snapshots.
20. Which placement facts should graduate from empirical runtime observations into stronger target profiles.

## 81. Proposed 1.0 research-threshold implications

A 1.0 research system should not claim machine-native placement merely because it can schedule jobs.

It should include:

- a versioned spatial model distinguishing logical identity, place identity, physical resource identity, topology identity, and placement identity;
- a versioned topology model supporting at least containment plus one non-tree relation such as accessibility/affinity/fault sharing;
- explicit placement requirements and preferences with different enforcement semantics;
- explicit computation placement and data placement;
- location variables or equivalent unresolved placement holes;
- capability-based heterogeneous resource matching;
- at least one placement study where logical identity survives physical movement;
- at least one observation study where two placements are functionally equivalent but become distinguishable after location observation;
- at least one preference/requirement study where a candidate is merely suboptimal under a preference but invalid under a hard locality requirement;
- at least one topology study proving identifier adjacency does not establish proximity;
- at least one fault-domain study where multiple replicas fail an independence claim because they share a physical failure domain;
- at least one capability-based device study where different device classes remain admissible because each satisfies the actual requirement set;
- at least one placement-hole satisfiability study with `SAT`, `UNSAT`, or explicit `UNKNOWN` behavior;
- at least one communication-free/locality study that produces a placement counterexample after data is moved across the protected boundary;
- a versioned mobility/migration model distinguishing migration unit, closure, transfer, cutover, destination activation, source retirement, rollback, and unknown outcome;
- at least one migration study where principal/task identity survives movement but destination trust/attestation evidence must be refreshed;
- at least one capability-rebinding study that rejects ambient authority transfer during migration;
- at least one migration cutover/fencing study that prevents simultaneous stale-source mutation after destination activation;
- at least one RFC 0028 boundary-crossing study where moving local participants apart is rejected unless a distributed realization preserves the required relation;
- at least one RFC 0027 study where placement introduces a serialization obligation and a semantically invalid codec causes the placement realization to fail;
- at least one RFC 0021 study where a faster accelerator placement is rejected because its numeric realization violates the protected numerical relation;
- at least one topology-change study where only dependent placement evidence becomes stale;
- at least one placement witness produced by untrusted search and independently checked by a narrower verifier;
- at least one Forge placement study comparing multiple heterogeneous candidates and rejecting a faster candidate that violates a hard numeric, trust, memory-access, fault-domain, security, or distributed-boundary relation;
- at least one bounded placement/migration experiment whose evidence remains explicitly bounded rather than being upgraded to universal placement independence.

## 82. Expected implementation sequence

A practical implementation sequence could be:

### Phase A — static artifact model

Implement:

```text
Place
TopologySnapshot
ExecutionResource
MemoryResource
CapabilitySet
LocationVariable
PlacementRequirement
PlacementPreference
ComputationPlacement
DataPlacement
```

### Phase B — checker

Implement a deterministic checker for:

```text
resource existence
capability requirements
hard affinity/anti-affinity
accessibility
fault-domain separation
```

### Phase C — cross-RFC obligations

Generate explicit obligations when placement changes:

```text
numeric realization
serialization boundary
distributed boundary
trust/attestation
clock/schedule
```

### Phase D — migration model

Add:

```text
MobilityBoundary
MigrationPlan
MigrationClosure
MigrationPhase
CapabilityRebinding
MigrationDelta
```

### Phase E — bounded Forge search

Allow one or more untrusted proposal strategies to emit candidate placements.

Verify them with the independent checker.

Do not claim globally optimal scheduling.

## 83. Example machine-native contract

```text
SpatialContract {
    computation: MatrixStage

    requirements:
        device_capability("matrix-fma")
        numeric_relation("error<=1e-7")
        direct_access(MatrixData)
        trust_profile("approved-compute")

    affinity:
        prefer_near(InputData)

    anti_affinity:
        different_failure_domain(VerifierReplica)

    mobility:
        allowed_if:
            destination_attested
            capabilities_rebound
            timing_reverified

    placement:
        unresolved
}
```

Forge is free to choose:

```text
CPU
GPU
accelerator
local worker
remote worker
```

only if the selected combination establishes the declared protected relations.

## 84. Example migration contract

```text
MigrationContract {
    entity: VerifierWorkerTask

    source: α
    destination: β

    preserve:
        logical_identity
        functional_state
        authority_not_broadened
        confidentiality_policy

    refresh:
        destination_attestation
        clock_mapping
        schedule_witness

    rebind:
        local_device_capability

    cutover:
        single_writer_epoch

    source_retirement:
        required
}
```

This expresses the movement semantics without choosing pre-copy, post-copy, process checkpointing, VM migration, or another transfer realization.

## 85. Final architectural position

RFC 0029 should make the following pattern normal throughout MNCS:

```text
semantic computation/data identity
        ↓
spatial requirements + preferences
        ↓
unresolved location variables
        ↓
current topology/resource evidence
        ↓
untrusted placement search
        ↓
placement witness
        ↓
independent constraint + cross-RFC checking
        ↓
approved spatial realization
```

The essential machine-native rule is:

> Search freely over where computation and data live, but never move an entity across a boundary that changes its protected semantics without proving the required refinement.

The result is a language in which placement is neither hidden nor prematurely hard-coded. It becomes a typed, evidence-bearing realization dimension that can remain unresolved until the machine has enough information to choose safely.