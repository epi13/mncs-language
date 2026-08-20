# RFC 0032: Machine-Native Complexity, Quantitative Cost, Resource Accounting, QoS, Energy, and Performance Semantics

- **Status:** Draft
- **Authors:** MNCS project
- **Target:** pre-1.0 research roadmap

## Summary

MNCS needs a quantitative semantic layer that can state what a valid program, realization, build, placement, distributed protocol, migration, or verification strategy costs, consumes, reserves, requires, delays, or exposes without collapsing all resource behavior into a single scalar called `cost`.

This RFC establishes quantitative semantics around typed dimensions and units, metric-specific composition algebras, abstract work and span, asymptotic and concrete symbolic complexity, resource capacities and budgets, peak and cumulative usage, reservations and allocations, static and dynamic bounds, amortized and expected costs, workload-scoped measurements, distributions and tail behavior, queueing and deterministic service envelopes, QoS/SLO-style objectives, violation budgets, energy and power, carbon and monetary valuation, verification/evidence cost, quantitative feasibility, provenance of observations, and Forge quantitative-realization analysis.

The primary constitutional rule is:

> **A machine-native program does not have one cost. It has scoped quantitative relations over explicit metrics, workloads, targets, composition laws, bounds, observations, and budgets.**

The second rule is:

> **Work is not time. Capacity is not consumption. Reservation is not allocation. Measurement is not proof.**

The third rule is:

> **Worst-case, best-case, amortized, expected, percentile, observed, and statically bounded quantities are distinct claims even when they share the same physical unit.**

The fourth rule is:

> **RFC 0032 establishes quantitative meaning and hard quantitative admissibility. RFC 0033 owns preference, utility, Pareto policy, risk trade-offs, and final selection among quantitatively admissible candidates.**

The machine-native objective is not to standardize one profiler, cost analyzer, queueing model, resource type system, benchmark suite, scheduler, telemetry backend, SLO system, energy estimator, or optimization objective. Work/span analysis, Automatic Amortized Resource Analysis, quantitative and graded type systems, time-credit logics, abstract interpretation, WCET analysis, network calculus, Real-Time Calculus, queueing theory, probabilistic reward models, Roofline analysis, SRE-style SLO/error-budget systems, energy accounting, and future quantitative methods are evidence or realization families over a smaller semantic model.

Programs and Forge should be able to state hard quantitative requirements independently of how they are proven or measured. Candidate realizations may then be searched freely, but they enter RFC 0033 selection only after all mandatory semantic and quantitative obligations have been discharged.

---

## 1. Scope

RFC 0032 owns the semantics of:

- resource dimensions and unit identities;
- metric identities beyond units;
- metric-specific composition algebras;
- abstract work and dependency span;
- size measures and symbolic resource functions;
- asymptotic, exact, upper, lower, interval, amortized, expected, and statistical resource claims;
- capacity, reservation, allocation, consumption, utilization, headroom, and pressure distinctions;
- resource budgets, quotas, credits, and violation budgets;
- peak, cumulative, instantaneous, rate, average, and distributional quantity shapes;
- workload and request-class scope for quantitative claims;
- benchmark and performance-observation identity;
- statistical aggregation such as mean, median, percentiles, tails, and confidence metadata;
- queueing/service models and operational-law evidence;
- deterministic arrival/service envelopes, backlog, buffer, and delay bounds;
- target-scoped performance, bandwidth, throughput, and bottleneck evidence;
- QoS indicators/objectives and explicit quantitative feasibility;
- energy, power, contextual carbon, and monetary valuation as distinct metrics;
- communication, storage, persistence, build, verification, evidence, model-call, and migration costs;
- resource attribution and accounting policy;
- quantitative observations, witnesses, counterexamples, and deltas;
- Forge quantitative analysis and hard-resource candidate filtering.

This RFC does **not** own:

- generic capability/resource authority: RFC 0008;
- generic causal/concurrent semantics: RFC 0010;
- target/runtime/platform facts: RFC 0017;
- generic evidence trust and assurance: RFC 0018;
- representation semantics: RFC 0019;
- generic equality/refinement/equivalence relations: RFC 0020;
- numeric precision/error semantics: RFC 0021;
- termination/progress meaning and bounded-noncompletion interpretation: RFC 0022;
- time, deadlines, clock semantics, and schedule correctness: RFC 0023;
- confidentiality/integrity of measurements and resource side channels: RFC 0024;
- persistence correctness: RFC 0026;
- distributed correctness: RFC 0028;
- placement validity: RFC 0029;
- lifecycle correctness: RFC 0030;
- build/derivation correctness: RFC 0031;
- multi-objective preference, utility, Pareto ranking, risk appetite, and final realization choice: RFC 0033.

---

## 2. Constitutional distinctions

The following distinctions are normative design rules for the semantic model.

1. **A number plus a unit is not a complete metric.**
2. **A unit is not a metric identity.**
3. **Work is not elapsed time.**
4. **Span is not measured latency.**
5. **Throughput is not latency.**
6. **Service time is not response time.**
7. **Capacity is not reservation.**
8. **Reservation is not allocation.**
9. **Allocation is not live consumption.**
10. **Consumption is not utilization.**
11. **Utilization is not efficiency.**
12. **Absolute use is not resource pressure.**
13. **Peak use is not cumulative use.**
14. **Cumulative use is not a rate.**
15. **Power is not energy.**
16. **Energy is not carbon.**
17. **Resource use is not monetary valuation.**
18. **Big-O is not a concrete resource bound.**
19. **A complexity class is not a concrete cost function.**
20. **Worst-case cost is not amortized cost.**
21. **Amortized cost is not expected cost.**
22. **Expected cost is not worst-case cost.**
23. **Best-case cost is not a lower bound for every execution unless its quantified domain says so.**
24. **Mean is not median.**
25. **Median is not a percentile other than P50 by explicit definition.**
26. **Percentile is not observed maximum.**
27. **Observed maximum is not worst-case bound.**
28. **Measured latency is not WCET.**
29. **WCET is target/model scoped.**
30. **Benchmark evidence is workload scoped.**
31. **Synthetic workload evidence is not automatically production-workload evidence.**
32. **Failure to infer a resource bound does not prove unbounded resource use.**
33. **A static resource bound is not runtime enforcement.**
34. **A runtime limit is not proof that all executions naturally remain under that limit.**
35. **A resource budget is not a resource observation.**
36. **A quota is not a budget.**
37. **A reservation is not a quota.**
38. **A proof credit is not a scheduler token.**
39. **A resource algebra is metric-specific; there is no universal composition rule.**
40. **Parallel composition is not universally addition.**
41. **Sequential composition is not universally addition.**
42. **Inclusive cost is not exclusive cost.**
43. **Attribution is not observation.**
44. **Shared-resource attribution requires an explicit policy.**
45. **One-time cost is not recurring cost.**
46. **Cold-state cost is not warm-state cost.**
47. **Proof-generation cost is not proof-checking cost.**
48. **Evidence cost does not weaken the evidence obligation.**
49. **Network bytes are not network round trips.**
50. **Message count is not bandwidth.**
51. **Storage logical bytes are not physical bytes.**
52. **Replication overhead is not logical dataset size.**
53. **QoS indicator is not QoS objective.**
54. **QoS objective is not violation policy.**
55. **Error-budget consumption is not failure probability.**
56. **Measurement confidence is not semantic assurance.**
57. **Telemetry provenance is part of claim scope when it can change interpretation.**
58. **Instrumentation overhead may change the measured system.**
59. **Resource telemetry may itself form an RFC 0024 observer channel.**
60. **A quantitatively superior candidate remains invalid when it violates any protected semantic relation.**
61. **Hard quantitative feasibility precedes RFC 0033 preference ranking.**
62. **Metric dominance does not itself authorize promotion.**
63. **There is no universal scalar `cost`.**
64. **There is no universal scalar `performance`.**
65. **There is no universal scalar `efficiency`.**

---

## 3. Quantitative semantic stack

RFC 0032 uses ten interacting semantic planes.

### 3.1 Quantity plane

Defines dimensions, units, metric identities, quantity values, and quantity shapes.

### 3.2 Cost-model plane

Defines which semantic/runtime events incur a quantity and how those quantities compose.

### 3.3 Complexity plane

Defines size measures, work, span, asymptotic classes, concrete symbolic bounds, and recurrence-style claims.

### 3.4 Accounting plane

Defines capacity, reservation, allocation, consumption, utilization, attribution, and accounting scopes.

### 3.5 Bound plane

Defines exact, upper, lower, interval, amortized, expected, and method-scoped resource bounds.

### 3.6 Statistical plane

Defines distributions, samples, means, medians, quantiles, tails, variance, confidence metadata, and stochastic claims.

### 3.7 Workload plane

Defines request classes, input distributions, concurrency, arrivals, operation mix, and benchmark scope.

### 3.8 QoS plane

Defines quantitative indicators, objectives, violation/error budgets, windows, and hard feasibility.

### 3.9 Physical-impact plane

Defines power, energy, carbon, and external valuation models.

### 3.10 Assurance plane

Defines evidence-bearing quantitative claims, observations, bound witnesses, counterexamples, and deltas.

No plane may silently authorize another. A `PerformanceObservation` does not become a `ResourceBound`; an `EnergyEstimate` does not become an exact `EnergyObservation`; a `QoSObjective` does not become proof that the objective is satisfied.

---

## 4. Dimensions, units, and metric identity

### 4.1 Dimensions

A `ResourceDimension` describes physical or abstract quantity kind independently from representation.

Candidate base dimensions include:

```text
TIME
DATA
ENERGY
WORK
COUNT
MONEY
CARBON_EQUIVALENT
```

Derived dimensions may include:

```text
DATA / TIME          -> bandwidth
WORK / TIME          -> throughput
ENERGY / TIME        -> power
MONEY / REQUEST      -> monetary cost per request
ENERGY / OPERATION   -> energy efficiency relation
```

The dimensional system should reject semantically meaningless arithmetic unless a declared conversion/valuation relation exists.

### 4.2 Units

A `ResourceUnit` realizes one dimension.

Examples:

```text
ns, us, ms, s
byte, KiB, MiB, GiB
J, kJ
W
request
message
abstract_step
USD
kgCO2e
```

The unit identity includes conversion semantics and version where external standards require it.

### 4.3 Metrics

A `ResourceMetric` is richer than a dimension or unit.

```text
ResourceMetric {
    id
    dimension
    unit
    quantity_shape
    aggregation_semantics
    composition_algebra
    scope_schema
}
```

Examples sharing `ms` as unit but differing in metric semantics:

```text
MeanRequestLatency
P99RequestLatency
ObservedMaximumLatency
WorstCaseExecutionTimeBound
```

### 4.4 Quantity shape

A metric may be:

```text
EXACT_SCALAR
PEAK
CUMULATIVE
INSTANTANEOUS
RATE
AVERAGE
DISTRIBUTION
INTERVAL
SYMBOLIC_FUNCTION
```

The shape affects legal composition and comparison.

---

## 5. Resource algebras

### 5.1 Why one algebra is insufficient

Different metrics compose differently.

Total work commonly composes additively across both sequential and parallel branches:

```text
W(A ; B) = W(A) + W(B)
W(A || B) = W(A) + W(B)
```

Critical-path span commonly uses:

```text
S(A ; B) = S(A) + S(B)
S(A || B) = max(S(A), S(B))
```

Peak memory requires lifetime/overlap information and often cannot be derived by a simple binary arithmetic operator over isolated peaks.

### 5.2 ResourceAlgebra

```text
ResourceAlgebra<R> {
    identity
    sequential_composition
    parallel_composition
    branch_choice_composition
    ordering
    bottom?
    top?
    laws
}
```

The algebra itself is versioned and evidenced where correctness depends on nontrivial assumptions.

### 5.3 Semiring-inspired grades

Quantitative Type Theory and graded modalities motivate semiring-like grade structures for some resource uses.

MNCS may support:

```text
Grade<R, q>
```

where `+` and `*` have resource-specific meaning.

This RFC does not mandate semirings for every metric. Metrics whose composition depends on lifetimes, contention, queue state, or topology may require richer algebras or external witnesses.

### 5.4 Algebra witnesses

Candidate implementations may supply:

```text
ResourceAlgebraWitness {
    metric
    law_set
    proof_or_certificate
}
```

A narrow verifier may check algebraic laws for finite or symbolic domains where feasible.

---

## 6. Abstract work, span, and parallelism

### 6.1 Work

`WorkClaim` measures total charged abstract operations under a `CostModel`.

```text
WorkClaim {
    subject
    cost_model
    size_scope
    bound
    evidence
}
```

Work must not be interpreted as elapsed time.

### 6.2 Span

`SpanClaim` measures longest weighted dependency path under an RFC 0010 causal/dependency relation.

```text
SpanClaim {
    subject
    dependency_graph
    cost_model
    bound
    evidence
}
```

### 6.3 Parallelism

Potential parallelism may be represented as a derived relation such as:

```text
AverageParallelism = Work / Span
```

where meaningful.

This does not prove a target can realize that parallelism efficiently.

### 6.4 Brent-style realization relation

Under suitable scheduling assumptions, abstract work/span can support target-specific execution-time envelopes.

The relevant result must name:

- processor count;
- scheduling model;
- per-operation cost model;
- communication assumptions;
- target applicability;
- overhead model.

### 6.5 Invariance across placement

Different RFC 0029 placements may preserve the same work/span while changing:

- physical latency;
- communication bytes;
- cache behavior;
- energy;
- contention;
- tail behavior.

Thus `WorkEquivalent` and `PerformanceEquivalent` remain distinct RFC 0020 relations.

---

## 7. Cost models and charged events

### 7.1 CostModel

```text
CostModel {
    id
    metric
    charged_events
    event_cost_function
    composition_algebra
    assumptions
    target_scope?
}
```

Candidate charged events include:

```text
semantic_step
allocation
free
load
store
cache_miss
message_send
byte_transfer
round_trip
persistent_flush
verifier_call
model_call
token_processed
```

### 7.2 Abstract versus target cost models

An abstract model may charge one unit per semantic operation.

A target model may charge cycles, bytes, or measured energy according to runtime facts.

These have different claim identities.

### 7.3 Cost-model validity

A model may be useful without perfectly predicting wall time.

For example, `MessageCount` can establish a protocol-complexity fact while remaining insufficient to predict network latency.

### 7.4 Cost-model provenance

Externally calibrated models should record:

- model version;
- target/environment scope;
- calibration workload;
- fitting method;
- measurement provenance;
- uncertainty.

---

## 8. Size measures and input profiles

### 8.1 SizeMeasure

```text
SizeMeasure {
    id
    subject_type
    projection
    dimension
    unit
}
```

Examples:

```text
ListLength
TreeNodeCount
TreeHeight
GraphVertexCount
GraphEdgeCount
ImagePixelCount
SerializedByteLength
RequestPayloadBytes
```

A symbol `n` without size-measure identity is insufficient for strong quantitative claims.

### 8.2 Multivariate size

A graph algorithm may depend on:

```text
n = vertices
m = edges
```

A tree may require:

```text
n = nodes
h = height
```

Resource functions may therefore be multivariate.

### 8.3 Sized-type integration

RFC 0019 indexed data may expose structural measures to resource analysis.

Example:

```text
List<T, length=n>
```

may support:

```text
Work <= 3*n + 2
```

without hard-coding layout semantics into the logical type.

---

## 9. Complexity and symbolic resource bounds

### 9.1 AsymptoticBound

```text
AsymptoticBound {
    subject
    metric
    size_measures
    class
    direction
    assumptions
}
```

Examples:

```text
O(n)
Theta(n log n)
Omega(n)
```

An asymptotic claim does not become a concrete operational bound.

### 9.2 ConcreteBound

```text
ConcreteBound {
    metric
    size_scope
    expression
    relation
}
```

Examples:

```text
Work <= 3*n + 7
Allocations <= n + 1
Messages <= 2*m + n
```

### 9.3 Bound expression families

The first semantic model should admit at least:

```text
constant
affine
polynomial
logarithmic
piecewise
```

and reserve:

```text
recurrence
rational
exponential
user-defined certified function
```

### 9.4 Complexity class versus concrete bound

A `PolynomialTime` profile and a `4*n^2 + 3` bound remain different objects.

Implicit-complexity systems may later provide witnesses for broad resource classes without replacing concrete accounting.

---

## 10. Exact, lower, upper, and interval resource claims

A `ResourceBound` has explicit direction and epistemic method.

```text
ResourceBound {
    metric
    scope
    function_or_value
    direction: EXACT | LOWER | UPPER | INTERVAL
    method
    assumptions
    evidence
}
```

Examples:

```text
Exact(1024 bytes)
Upper(512 MiB)
Lower(900 operations)
Interval([900, 1200] operations)
```

A lower bound and an upper bound may come from different methods and carry different evidence.

---

## 11. Worst-case, best-case, amortized, and expected cost

### 11.1 Worst case

```text
WorstCaseUpperBound
```

quantifies over the declared workload/input domain.

### 11.2 Best case

```text
BestCaseLowerBound
```

refers to the cheapest permitted execution/input under its scope and must not be interpreted as a universal lower bound for arbitrary observations unless the relation says so.

### 11.3 Amortized cost

```text
AmortizedBound {
    operation_family
    sequence_scope
    initial_state
    horizon
    bound
    potential_witness?
}
```

An amortized O(1) operation may still have O(n) individual executions.

### 11.4 Potential witnesses

AARA-style potential can be represented as:

```text
PotentialWitness {
    subject_state
    potential_function
    resource_metric
    conservation_relation
    proof_or_certificate
}
```

### 11.5 Expected cost

```text
ExpectedCostClaim {
    metric
    probability_model
    scheduler_scope?
    expectation_bound
    evidence
}
```

Expected-cost claims do not imply worst-case bounds.

### 11.6 Probabilistic and nondeterministic quantification

For MDP-like systems, claims may require:

```text
MIN_EXPECTED
MAX_EXPECTED
POLICY_SPECIFIC_EXPECTED
```

so scheduler/adversary quantification remains explicit.

---

## 12. Quantitative and graded types

### 12.1 Quantitative use grades

MNCS may represent graded usage information as:

```text
ResourceUse<R, q, T>
```

where `q` belongs to a metric-specific algebra.

### 12.2 Authority versus quantity

A capability may authorize one operation while resource semantics state that operation's quantitative demand.

```text
OneUseCapability
```

is therefore not:

```text
OneUnitOfCost
```

### 12.3 Graded modalities

Candidate semantic forms may include:

```text
Available<T, q>
Consumes<T, q>
Produces<T, q>
```

without selecting final surface syntax.

### 12.4 Proof-level resource credits

Time-credit or resource-credit logics may expose:

```text
ResourceCredit<R, q>
```

as proof objects used to establish runtime bounds.

They are not runtime scheduler resources.

---

## 13. Capacity, reservation, allocation, consumption, utilization, and headroom

### 13.1 Capacity

```text
ResourceCapacity {
    resource
    metric
    quantity
    validity_interval
    evidence
}
```

### 13.2 Reservation

```text
ResourceReservation {
    beneficiary
    resource
    amount
    interval
    authority
}
```

A reservation states capacity set aside, not actual use.

### 13.3 Allocation

`ResourceAllocation` records assigned capacity or created objects/resources.

### 13.4 Consumption

`ResourceConsumption` records actual cumulative or instantaneous use.

### 13.5 Utilization

```text
Utilization = usage / available_capacity
```

under a named scope and interval.

### 13.6 Headroom

`ResourceHeadroom` expresses remaining capacity under a scope/model.

### 13.7 Pressure

Pressure may include queue depth, reclaim activity, throttling, contention, or policy-specific saturation indicators and is not reducible to utilization alone.

---

## 14. Peak, cumulative, instantaneous, and rate semantics

### 14.1 Peak

`PeakResourceClaim` quantifies maximum simultaneous use over a declared interval/execution.

### 14.2 Cumulative

`CumulativeResourceClaim` sums consumption over a trace or interval.

### 14.3 Instantaneous

`InstantaneousResourceObservation` refers to one observation instant/window and does not imply a peak.

### 14.4 Rate

Examples:

```text
bytes / second
requests / second
joules / second
verifier calls / minute
```

Rates require an interval/time base from RFC 0023.

### 14.5 Lifetime-sensitive resources

Peak memory cannot generally be composed from isolated peak values without liveness/overlap information.

A `LifetimeResourceWitness` may summarize:

- acquisition event;
- release event;
- size;
- overlap relation.

---

## 15. Budgets, quotas, credits, and runtime guards

### 15.1 ResourceBudget

```text
ResourceBudget {
    metric
    scope
    limit
    window?
    exceed_semantics
}
```

Examples:

```text
PeakMemory <= 2 GiB
Messages <= 1000 per operation
VerifierCalls <= 20 per refinement cycle
Energy <= 500 J per batch
```

### 15.2 Quota

A quota expresses authority/policy to consume up to some amount; it is not itself proof of need or use.

### 15.3 Reservation

A reservation allocates capacity in advance and remains distinct from quota/budget.

### 15.4 ResourceCredit

A credit can represent logical accounting for future resource obligations.

### 15.5 RuntimeResourceGuard

A runtime guard may enforce a budget by:

```text
reject
throttle
fail
spill
fallback
```

The resulting failures/effects belong to RFC 0011/0008.

### 15.6 Budget exhaustion

`RESOURCE_BUDGET_EXHAUSTED` is a bounded execution outcome and must not imply unbounded or divergent unrestricted behavior under RFC 0022.

---

## 16. Workloads and performance-claim scope

### 16.1 WorkloadModel

```text
WorkloadModel {
    id
    request_classes
    input_size_distributions
    arrival_model
    concurrency_model
    operation_mix
    state_profile
}
```

### 16.2 Request classes

Different operations may require separate latency/throughput/resource claims.

### 16.3 BenchmarkCorpus

A benchmark corpus is a finite experimental input set.

It does not automatically become a workload distribution or universal input domain.

### 16.4 ProductionWorkloadObservation

Production observations may estimate a workload model while retaining their temporal/environment scope.

### 16.5 Workload mismatch

A `WorkloadMismatchCounterexample` may demonstrate that evidence collected under one workload does not satisfy a claim scoped to another.

---

## 17. Performance observations

### 17.1 PerformanceObservation

```text
PerformanceObservation {
    subject
    artifact_or_realization
    target
    placement
    workload
    environment
    metric
    sample_set
    instrument
    observation_interval
    measurement_uncertainty
    provenance
}
```

### 17.2 Instrumentation overhead

Observations should record whether instrumentation can materially perturb:

- execution time;
- cache behavior;
- memory use;
- network traffic;
- scheduling;
- power.

### 17.3 Repeated measurement

Repeated samples improve empirical characterization but do not turn empirical evidence into a universal bound.

### 17.4 Target sensitivity

Changing processor, runtime, memory topology, compiler, placement, or workload can invalidate performance evidence narrowly through RFC 0018.

---

## 18. Statistical and distributional metrics

### 18.1 DistributionClaim

```text
DistributionClaim {
    metric
    workload
    target
    model_or_empirical_distribution
    evidence
}
```

### 18.2 Aggregations

The semantic model should recognize at least:

```text
MEAN
MEDIAN
P90
P95
P99
P999
OBSERVED_MAXIMUM
VARIANCE
```

### 18.3 QuantileClaim

```text
QuantileClaim {
    metric
    percentile
    threshold
    window
    workload
}
```

A quantile claim is not a universal upper bound.

### 18.4 Tail probability

```text
TailProbabilityClaim {
    metric
    threshold
    probability_bound
}
```

### 18.5 Confidence metadata

Statistical confidence intervals, bootstrap intervals, Bayesian credible intervals, or similar methods are method-specific evidence metadata.

They are not RFC 0018 assurance scores.

---

## 19. Tail latency and fan-out

### 19.1 Tail-at-scale effect

Distributed fan-out can turn rare slow suboperations into common end-to-end latency contributors.

### 19.2 FanoutLatencyClaim

```text
FanoutLatencyClaim {
    operation
    fanout
    completion_rule
    child_latency_model
    aggregate_latency_model
}
```

### 19.3 Completion rules

Examples:

```text
ALL
QUORUM(k)
FIRST_SUCCESS
FIRST_VALID
```

These change tail behavior and must align with RFC 0028 correctness semantics.

### 19.4 Hedging/replication

Request hedging may reduce tail latency while increasing work, network use, and load.

RFC 0032 describes those dimensions; RFC 0033 determines whether the trade is desirable.

---

## 20. Queueing semantics and operational laws

### 20.1 Queue model

```text
QueueModel {
    arrival_process
    service_process
    server_count
    queue_discipline
    capacity
    assumptions
}
```

### 20.2 Service time versus response time

```text
ResponseTime = QueueingDelay + ServiceTime
```

under a compatible model.

### 20.3 Little's Law

For stable systems with compatible scopes:

```text
L = lambda * W
```

can be represented as an `OperationalLawWitness` relating occupancy, throughput, and response-time observations.

### 20.4 Scope checking

A verifier must reject application of Little's Law when quantities refer to incompatible populations/windows.

### 20.5 Utilization and saturation

High utilization can cause sharply increasing queueing latency. Quantitative policies must not interpret maximal utilization as universally optimal.

### 20.6 Queueing evidence status

Analytical queueing results depend on stated arrival/service assumptions and remain model-scoped evidence.

---

## 21. Network Calculus and deterministic service envelopes

### 21.1 ArrivalEnvelope

```text
ArrivalEnvelope {
    stream
    lower?
    upper
    time_domain
}
```

### 21.2 ServiceEnvelope

```text
ServiceEnvelope {
    resource
    lower_service
    upper_service?
    time_domain
}
```

### 21.3 Derived bounds

A compatible network/real-time calculus witness may derive:

```text
BacklogUpperBound
BufferUpperBound
DelayUpperBound
```

### 21.4 Hard versus stochastic envelope

A deterministic arrival/service envelope is distinct from a high-percentile empirical model.

### 21.5 Real-Time Calculus

Event-stream and service-envelope analysis can bridge RFC 0023 temporal requirements with RFC 0032 quantitative resource supply/demand.

---

## 22. WCET and target-specific execution bounds

### 22.1 WCETClaim

```text
WCETClaim {
    executable
    target_execution_context
    input_domain
    upper_bound
    analysis_method
    assumptions
    evidence
}
```

### 22.2 Target context

The claim may depend on:

- processor model;
- cache configuration;
- memory timing;
- compiler/backend;
- binary identity;
- scheduling/interference assumptions.

### 22.3 Measured maximum

A benchmark maximum must not satisfy a WCET obligation unless the obligation explicitly asks for bounded empirical evidence instead of a universal bound.

### 22.4 Schedule integration

RFC 0023 may consume WCET/execution-cost evidence to verify deadlines and schedules.

RFC 0032 does not own schedule correctness.

---

## 23. Demand, supply, Roofline, and bottleneck semantics

### 23.1 DemandVector

```text
DemandVector {
    work
    bytes_from_memory
    bytes_to_memory
    network_bytes
    storage_ops
    accelerator_ops
}
```

### 23.2 CapacityVector

```text
CapacityVector {
    compute_rate
    memory_bandwidth
    network_bandwidth
    storage_rate
    capacity_limits
}
```

### 23.3 Operational intensity

Operational intensity may be derived as:

```text
operations / bytes_transferred
```

under an explicit counting model.

### 23.4 Roofline-style claim

A Roofline-like model can bound attainable throughput from operational intensity and hardware ceilings.

It is a target/model-specific performance claim, not intrinsic program semantics.

### 23.5 BottleneckClaim

```text
BottleneckClaim {
    candidate
    workload
    target
    limiting_resource
    model
    evidence
}
```

A bottleneck can change when workload, placement, representation, or target changes.

---

## 24. QoS indicators, objectives, and violation budgets

### 24.1 QuantitativeIndicator

An indicator identifies what is observed.

```text
QuantitativeIndicator {
    metric
    request_class
    measurement_source
    eligibility_filter
}
```

### 24.2 QoSObjective

```text
QoSObjective {
    indicator
    aggregation
    threshold_or_range
    evaluation_window
    required_fraction?
}
```

Example:

```text
99.9% of Read requests < 50 ms over 24 h
```

### 24.3 Objective satisfaction

Objective satisfaction is an RFC 0018 claim based on observations/bounds and remains scope-specific.

### 24.4 ViolationBudget

```text
ViolationBudget {
    objective
    window
    allowed_violation
    observed_or_projected_consumption
}
```

### 24.5 Error budgets

SRE-style error budgets are one realization of `ViolationBudget` for reliability objectives.

They are not failure probabilities.

### 24.6 Lifecycle integration

RFC 0030 may use current violation-budget evidence when deciding whether rollout expansion is authorized.

RFC 0032 does not decide that policy on its own.

---

## 25. Storage and persistence accounting

RFC 0026 defines persistence correctness. RFC 0032 may quantify:

```text
LogicalBytes
PhysicalBytes
PeakStorage
WriteAmplification
ReadAmplification
JournalBytes
SnapshotBytes
CheckpointBytes
RecoveryWork
FlushCount
```

### 25.1 Logical versus physical bytes

Replication, journaling, metadata, checksums, compression, and snapshots may make physical storage differ from logical state size.

### 25.2 Write amplification

A persistence realization may write many physical bytes per logical byte changed.

This is quantitative evidence, not a durability property.

### 25.3 Recovery cost

A recovery strategy can satisfy identical crash semantics while differing in replay work/time/storage.

Those dimensions become RFC 0033 inputs only after RFC 0026 correctness passes.

---

## 26. Network and distributed accounting

RFC 0028 defines distributed correctness. RFC 0032 quantifies distributed realizations.

Candidate metrics include:

```text
MessageCount
ByteCount
PacketCount
RoundCount
RoundTrips
PeakBandwidth
AverageBandwidth
CrossRegionBytes
CrossFailureDomainBytes
QuorumProbeCount
ConsensusWork
```

### 26.1 Round complexity

Round count is distinct from message count.

### 26.2 Bytes versus latency

Few large messages and many tiny dependent messages may transfer the same bytes with very different latency.

### 26.3 Distributed tail

End-to-end latency may depend on the maximum/quorum/order statistic of replica response distributions.

### 26.4 Coordination cost

RFC 0028-generated coordination requirements can carry quantitative realization costs without turning coordination necessity into a preference decision.

---

## 27. Placement and migration accounting

RFC 0029 owns valid placement/migration. RFC 0032 may quantify:

```text
DataMovementBytes
MigrationDuration
CutoverPause
TemporaryDuplicateMemory
RemoteAccessBytes
NUMACrossingCount
DeviceTransferBytes
```

### 27.1 Movement versus steady state

One-time migration cost and steady-state locality cost remain separate.

### 27.2 Placement feasibility

A placement satisfying all semantic requirements may still fail a hard RFC 0032 memory/latency/energy budget.

### 27.3 Topology-dependent cost

Cost evidence must identify the topology snapshot when relationships such as bandwidth or distance matter.

---

## 28. Lifecycle and upgrade accounting

RFC 0030 owns lifecycle correctness. RFC 0032 may quantify:

```text
RolloutDuration
TemporarySurgeCapacity
UnavailableCapacity
DualWriteBytes
DualReadWork
MigrationWork
CanaryExposure
RollbackPreparationStorage
```

These do not alter RFC 0030 meanings such as finalization or rollback validity.

A lifecycle plan can be semantically correct but quantitatively inadmissible under a hard downtime or capacity budget.

---

## 29. Build, derivation, and assurance accounting

RFC 0031 owns derivation correctness. RFC 0032 may quantify:

```text
BuildCPUTime
BuildWallTime
BuildMemory
BuildNetworkBytes
CacheStorage
IndependentRebuildCount
ReproductionCost
AttestationGenerationCost
```

### 29.1 Reproducibility cost

Independent rebuilding can create substantial cost while strengthening assurance.

RFC 0032 records cost; RFC 0018 determines evidential value; RFC 0033 can optimize only within required assurance profiles.

### 29.2 Cache value

Cache hits may reduce build work/time while still requiring RFC 0031 artifact-binding verification.

---

## 30. Verification, proof, and evidence cost

### 30.1 VerificationCost

```text
VerificationCost {
    verifier
    subject
    metric
    quantity
    evidence
}
```

### 30.2 Proof generation versus checking

```text
ProofGenerationCost
ProofCheckingCost
```

remain distinct.

A design with expensive untrusted proof generation and cheap trusted proof checking may be quantitatively attractive for repeated checking.

### 30.3 Evidence storage

Proofs, traces, provenance, attestations, test corpora, and model-checking certificates consume storage.

### 30.4 Reverification cost

Evidence invalidation may cause recomputation. `ReverificationCost` can quantify the blast radius of dependency changes.

### 30.5 Verification budgets

Forge may operate under:

```text
VerifierCallBudget
ProofSearchBudget
ExperimentExecutionBudget
```

These are quantitative constraints, not changes to proof meaning.

---

## 31. Model/agent resource accounting

Machine-native development makes AI-model resources operationally relevant.

Candidate optional metrics include:

```text
ModelCalls
InputTokens
OutputTokens
InferenceTime
InferenceEnergy
ContextBytes
ToolCalls
```

These metrics apply to Forge/Fabric/MNEL workflows where declared and should not infect ordinary program semantics by default.

### 31.1 Token count is not semantic complexity

Token count is a model/interface resource observation, not a proof of code complexity or semantic density by itself.

### 31.2 Model-call budget

Bounded recursive-refinement workflows may consume explicit model-call/resource budgets.

---

## 32. Energy and power semantics

### 32.1 Power

`PowerObservation` is an instantaneous or averaged rate over an interval.

### 32.2 Energy

`EnergyObservation`/`EnergyEstimate` is cumulative energy over a scoped execution.

### 32.3 Estimate versus observation

Model-based energy estimation and direct instrument measurement are distinct evidence classes.

### 32.4 Peak-power requirements

Two candidates may consume equal energy while requiring different peak power.

### 32.5 Energy budgets

Embedded/mobile systems may declare hard energy constraints independently from latency or power constraints.

### 32.6 Minimum initial credit

Energy-game/credit methods may establish:

```text
MinimumInitialResourceCredit
```

for long-lived reactive systems under explicit models.

---

## 33. Carbon and external impact metrics

### 33.1 Carbon as derived context

A `CarbonEstimate` may derive from:

```text
energy
location
time
carbon_intensity_model
data_center_efficiency
hardware_assumptions
```

### 33.2 Carbon is not energy

Identical energy executions can have different contextual carbon estimates.

### 33.3 Carbon evidence freshness

Carbon-intensity inputs are time/location dependent and therefore RFC 0023/0029 scoped.

### 33.4 Method identity

Carbon estimates must name method/model version and uncertainty where applicable.

---

## 34. Monetary valuation

### 34.1 Resource usage first

MNCS should preserve intrinsic resource observations such as:

```text
GPU hours
GB-month storage
network GB
CPU seconds
```

### 34.2 Valuation model

```text
MonetaryValuation {
    usage
    pricing_model
    currency
    validity_interval
    quantity
}
```

### 34.3 Price change

Pricing may change without any artifact/program/resource usage change.

Therefore money is contextual valuation, not universal cost identity.

---

## 35. Resource attribution and accounting policy

### 35.1 Observation versus attribution

A system may observe total CPU usage but require policy to attribute portions to tasks/tenants/components.

### 35.2 AttributionPolicy

```text
AttributionPolicy {
    shared_resource
    subjects
    method
    assumptions
}
```

Possible policies include:

```text
exclusive ownership
proportional split
marginal cost
first-user charge
shared overhead pool
```

### 35.3 Inclusive versus exclusive cost

Function/component/task accounting should expose whether child/callee activity is included.

### 35.4 Shared runtime overhead

GC, runtime services, caches, compaction, telemetry, and kernel work must not be silently attributed without policy.

---

## 36. Cold, warm, one-time, recurring, and amortization scope

### 36.1 State profile

Performance claims may depend on:

```text
COLD
WARM_CACHE
MODEL_LOADED
JIT_READY
STEADY_STATE
```

### 36.2 One-time versus recurring

Initialization/build/migration/proof-generation may be one-time, while request execution is recurring.

### 36.3 AmortizationScope

```text
AmortizationScope {
    operation_family
    horizon
    population
    initial_state
}
```

A cost amortized over one million operations cannot be quoted as the first-request cost.

---

## 37. Quantitative evidence and uncertainty

### 37.1 ResourceObservation

Observations carry:

- metric identity;
- subject;
- workload;
- target/environment;
- time window;
- instrument;
- sample count;
- uncertainty;
- provenance.

### 37.2 Analytical witness

A static resource proof/certificate carries assumptions and method identity rather than sample data.

### 37.3 Statistical uncertainty

Confidence/credible intervals remain statistical metadata.

### 37.4 Measurement uncertainty

Instrument calibration, resolution, sampling, and synchronization uncertainty may be represented.

### 37.5 Evidence disagreement

A static upper bound and benchmark distribution can coexist even when far apart.

They do not automatically conflict because they claim different things.

---

## 38. Quantitative counterexamples

A `QuantitativeCounterexample` should identify:

```text
claim
subject
metric
workload/input
observed_or_derived_value
required_value
method
scope
```

Examples:

- one input exceeding a universal memory bound;
- one sampled request refuting an `AllRequests <= X` claim;
- a placement exceeding a bandwidth requirement;
- a workload model violating an arrival-envelope assumption;
- a candidate crossing an energy budget;
- a profiling result collected under the wrong artifact identity.

Counterexamples are metric- and scope-specific.

---

## 39. Resource deltas

`ResourceDelta` compares quantitative properties between candidates without deciding which candidate is preferred.

```text
ResourceDelta {
    baseline
    candidate
    metric_deltas
    workload_scope
    target_scope
    evidence_changes
}
```

Example:

```text
Work:             -20%
PeakMemory:       +15%
P99Latency:       -10%
Energy:           -5%
ProofCheckTime:    same
```

RFC 0032 may report componentwise dominance when one candidate is no worse in every compared metric and better in at least one.

It does not turn dominance into automatic promotion.

---

## 40. Quantitative feasibility

### 40.1 Hard constraints

```text
QuantitativeRequirement {
    metric
    relation
    threshold
    workload
    target_scope?
    evidence_profile
}
```

### 40.2 Feasibility result

```text
QuantitativeFeasibility {
    candidate
    requirements
    status: SAT | UNSAT | UNKNOWN
    witnesses
    counterexamples
}
```

### 40.3 `UNKNOWN`

Missing/inconclusive performance evidence must preserve `UNKNOWN` rather than fabricating compliance or failure.

### 40.4 Filtering before selection

Only candidates satisfying semantic requirements and mandatory quantitative requirements may enter RFC 0033 preference search.

---

## 41. Quantitative lowering preservation

Resource semantics may be attached at multiple layers:

```text
semantic graph
HIR
SSA
backend IR
binary
runtime observation
```

### 41.1 Abstract claims

Algorithmic work/span may be preserved or transformed through lowering with explicit witnesses.

### 41.2 Target claims

Cycle, cache, power, or WCET claims bind to concrete lowerings/targets.

### 41.3 Optimization obligations

A transformation claiming reduced work or memory must preserve all protected semantic relations under RFC 0020 and may generate new target/evidence obligations.

### 41.4 Hidden cost changes

Compiler transformations may change instruction count, code size, memory traffic, or vectorization without changing logical behavior.

These are realization deltas, not semantic equivalence failures unless the quantitative property is itself protected.

---

## 42. Micro-verifier opportunities

RFC 0032 should encourage narrow independent verifiers for:

- dimensional/unit consistency;
- resource-algebra law checking;
- symbolic-bound certificate checking;
- potential/amortized-bound certificates;
- work/span DAG checking;
- resource-budget satisfaction;
- Little's-Law scope/arithmetic checks;
- deterministic envelope/backlog/delay certificates;
- WCET certificate import/applicability checks;
- quantile computation from frozen corpora;
- measurement-scope/provenance consistency;
- energy integration from frozen power traces;
- resource-delta computation;
- hard-quantitative-feasibility checking.

Expensive inference, simulation, profiling, AI prediction, search, and theorem generation remain outside the small trusted checker whenever feasible.

---

## 43. Forge quantitative-realization search

Forge may search dimensions such as:

```text
algorithm
representation
precision
parallel decomposition
placement
communication strategy
persistence realization
codec
verification strategy
build strategy
lifecycle strategy
```

For every candidate:

1. verify semantic admissibility under earlier RFCs;
2. collect/prove required quantitative facts;
3. verify all hard RFC 0032 constraints;
4. preserve `UNKNOWN` where quantitative evidence is insufficient;
5. emit a `ResourceDelta` versus baseline;
6. pass only admissible candidates to RFC 0033 selection.

Forge must not select a faster candidate that fails a protected numeric, security, durability, distribution, placement, lifecycle, derivation, or quantitative relation.

---

## 44. Bounded implementation pilot

The first implementation pilot should introduce versioned artifacts for:

```text
ResourceDimension
ResourceUnit
ResourceMetric
ResourceAlgebra
ResourceQuantity
CostModel
SizeMeasure
WorkClaim
SpanClaim
ResourceBound
ResourceCapacity
ResourceBudget
ResourceObservation
WorkloadModel
DistributionClaim
QuantileClaim
ExpectedCostClaim
QoSObjective
ViolationBudget
EnergyClaim
ResourceWitness
QuantitativeCounterexample
ResourceDelta
QuantitativeFeasibility
```

The pilot must remain intentionally narrow and should prefer deterministic JSON artifacts and independently checkable arithmetic/certificates over a large performance framework.

---

## 45. Required pilot fixtures

### 45.1 Work is not time

One logical computation executes on two targets:

```text
Work = 10_000 abstract steps
Target A elapsed = 2 ms
Target B elapsed = 6 ms
```

The fixture must establish equal abstract work without claiming elapsed-time equivalence.

### 45.2 Work/span separation

A small DAG has:

```text
Work = 100
Span = 20
```

Different processor-count realizations preserve the abstract quantities while changing target-specific time envelopes.

### 45.3 Cumulative allocation versus peak live memory

A program may allocate cumulatively 1 GiB while never exceeding 64 MiB live at once.

The verifier must reject substitution of one metric for the other.

### 45.4 Amortized versus worst case

A growable structure has cheap ordinary operations and occasional O(n) resize.

The fixture must support an amortized O(1) sequence claim while preserving an O(n) per-operation worst case.

### 45.5 Symbolic bound certificate

A list traversal should produce an independently checked bound such as:

```text
Work <= 3*n + 2
```

### 45.6 Unresolved analysis

A bound inference method fails to derive a bound.

The result must be `UNRESOLVED`, not `UNBOUNDED`.

### 45.7 Benchmark versus static bound

Observed maximum latency is 8 ms while a static/analytical target-specific bound is 12 ms.

Both evidence objects must coexist without conflation.

### 45.8 P99 versus universal bound

A corpus has 99% below 10 ms and at least one observation above 10 ms.

The quantile claim may pass while `AllRequests <= 10 ms` must fail.

### 45.9 Queue saturation

Same service work under two arrival rates should demonstrate identical per-request service work with materially different response latency due to queueing.

### 45.10 Little's Law scope

Compatible occupancy/arrival/latency observations should satisfy a bounded-tolerance Little's Law check.

Mismatched windows/populations must be rejected.

### 45.11 Deterministic versus statistical delay

A hard 20 ms bound and an empirical P99 8 ms claim must remain distinct and simultaneously representable.

### 45.12 Energy versus power

Two executions consume equal total energy but different peak power and elapsed time.

### 45.13 Carbon context

Same energy use under different location/time carbon-intensity models should produce different contextual carbon estimates while preserving identical energy evidence.

### 45.14 Verification generation versus checking

A proof may require expensive generation but cheap checking. The metrics must remain separately accounted.

### 45.15 Hard quantitative rejection

A functionally valid candidate with:

```text
PeakMemory = 600 MiB
Requirement <= 512 MiB
```

must be quantitatively `UNSAT` and excluded before RFC 0033 ranking.

### 45.16 Distribution tail

A fan-out operation should show that end-to-end P99 is not derivable from mean child latency alone.

### 45.17 Network bytes versus rounds

Two protocols transfer similar bytes but require different sequential round counts; the metrics must remain distinct.

### 45.18 Cold versus warm state

One candidate has a high cold-start cost and low steady-state cost. The system must not report one scalar latency without state profile.

### 45.19 Instrumentation overhead

A profiled execution differs measurably from an uninstrumented execution; the observation must retain instrumentation identity.

### 45.20 Resource side-channel boundary

A resource-observation request forbidden by RFC 0024 must remain unauthorized even though RFC 0032 would otherwise support the metric.

---

## 46. First Forge study: constrained equivalent realizations

The first full Forge study should declare one logical operation with several semantically equivalent candidates and hard quantitative requirements.

Example:

```text
Operation:
    AnalyzeBatch

Semantic requirements:
    Relation R

Quantitative requirements:
    PeakMemory <= 4 GiB
    P99Latency <= 100 ms
    Energy <= 50 J

Selection preferences:
    unresolved for RFC 0033
```

Candidate families:

```text
A: CPU scalar
B: CPU SIMD
C: GPU
D: GPU mixed precision
E: distributed workers
```

Expected outcome pattern:

- reject candidates that violate RFC 0021 numerical requirements before quantitative selection;
- reject candidates that fail hard RFC 0032 constraints;
- preserve `UNKNOWN` if evidence is insufficient;
- pass only multiple admissible candidates to RFC 0033.

The study should demonstrate that RFC 0032 does not select a winner merely because one candidate is fastest on one metric.

---

## 47. Second Forge study: verification economics without assurance dilution

Compare three evidence-generation strategies for the same protected claim:

```text
A: bounded empirical corpus
B: static certificate
C: formal proof
```

For each, record:

- generation CPU/time;
- checking CPU/time;
- evidence storage;
- invalidation dependencies;
- reverification cost.

RFC 0018 determines which strategies are sufficient for the intended action.

RFC 0032 must never downgrade evidence requirements because one method is cheaper.

RFC 0033 may compare costs only among strategies satisfying the required assurance policy.

---

## 48. Third Forge study: Fabric job graph

Model a bounded Fabric-style job graph:

```text
planner
  -> specialized workers
  -> verifiers
  -> aggregator
```

Track separately:

```text
model calls
tokens
CPU/GPU work
wall time
messages
bytes
proof/verifier work
energy
tail latency
```

The study should demonstrate:

- total cluster work differs from critical-path latency;
- parallel verification can increase total work while reducing wall time;
- tail latency can dominate completion under fan-out;
- extra verification cost may be required by RFC 0018 even if a cheaper path exists;
- no universal `cost` scalar is introduced.

---

## 49. Theory and prior-art synthesis

RFC 0032 should draw from the following theory families while keeping them as realizations/evidence methods rather than canonical ontology.

### 49.1 Operational cost semantics

Instrument transition/evaluation semantics with charged events so abstract cost is explicit and independent from wall-clock execution.

### 49.2 Work/span parallel complexity

Use total work and critical dependency depth as machine-independent parallel complexity quantities, with Brent-style realization bounds under explicit scheduling assumptions.

### 49.3 Automatic Amortized Resource Analysis

Use the potential method and type-based inference to derive symbolic upper/lower/amortized resource bounds as functions of input sizes.

Representative system: RaML.

### 49.4 Sized-type resource analysis

Use structural size indices and abstract interpretation to derive multivariate resource bounds.

### 49.5 Quantitative Type Theory

Use semiring-graded assumptions to represent quantitative usage in a dependent type-theoretic setting.

### 49.6 Graded modal and substructural types

Use algebra-indexed modalities to express resource usage while retaining metric-specific composition.

### 49.7 Resource-aware session types

Use communication-typed resource bounds for interacting processes where helpful, including probabilistic variants.

### 49.8 Time/resource-credit separation logics

Use logical credits that can be split/combined/spent to prove functional correctness and resource complexity compositionally.

### 49.9 Abstract interpretation and WCET analysis

Use conservative target-sensitive analyses to derive execution/resource bounds with explicit assumptions.

### 49.10 Network Calculus

Use arrival/service curves and min-plus-style algebra to derive deterministic backlog/delay/buffer bounds.

### 49.11 Real-Time Calculus / Modular Performance Analysis

Extend envelope reasoning to event streams and computational resources while preserving hard-bound semantics.

### 49.12 Queueing theory

Use arrival/service/utilization/variability models and operational laws such as Little's Law for workload-scoped performance analysis.

### 49.13 Probabilistic resource analysis

Use expected-cost and stochastic resource reasoning for randomized computation.

### 49.14 Quantitative probabilistic model checking

Use reward/cost structures over probabilistic state-transition systems for expected/cumulative/steady-state resource claims.

Representative systems: PRISM-family model checking.

### 49.15 Weighted transition systems

Use explicit algebraic weights over traces when ordinary Boolean reachability is insufficient.

### 49.16 Tail-at-scale analysis

Treat quantiles and fan-out order statistics as first-class distributed performance concerns rather than extrapolating from averages.

### 49.17 SLI/SLO/error-budget practice

Use explicit indicators, objectives, windows, and bounded violation budgets as a practical QoS realization family.

### 49.18 Roofline and bottleneck models

Relate operational intensity and demand vectors to target resource ceilings.

### 49.19 Energy accounting and energy-credit models

Represent cumulative energy, peak power, estimated/observed energy, and minimum initial resource credit under explicit models.

### 49.20 Contextual carbon accounting

Treat carbon as a derived contextual metric over energy, location, time, facility assumptions, and method version.

### 49.21 Proof-producing quantitative checking

Keep expensive inference, profiling, AI prediction, and optimization outside the TCB while validating small resource-bound certificates and arithmetic conditions independently.

---

## 50. Candidate foreign interfaces / interoperability

RFC 0032 may eventually import/export evidence from:

- profiler traces;
- benchmark suites;
- Linux/perf-style counters;
- OpenTelemetry-style metrics;
- energy meters;
- cloud billing records;
- queueing/simulation tools;
- WCET analyzers;
- AARA/resource analyzers;
- SLO systems;
- carbon-intensity datasets;
- model-inference accounting systems.

Imported metrics must map into explicit MNCS metric identities and scopes. A foreign numeric label must never become semantic truth merely because it uses a familiar field name such as `latency_ms`.

---

## 51. Counterexample and diagnostic vocabulary

Candidate diagnostic codes include:

```text
DIMENSION_MISMATCH
UNIT_INCOMPATIBLE
METRIC_MISMATCH
RESOURCE_ALGEBRA_UNSUPPORTED
BOUND_UNRESOLVED
BOUND_REFUTED
BUDGET_EXCEEDED
CAPACITY_INSUFFICIENT
WORKLOAD_MISMATCH
TARGET_SCOPE_MISMATCH
MEASUREMENT_STALE
MEASUREMENT_PROVENANCE_MISMATCH
QUANTILE_NOT_UNIVERSAL_BOUND
OBSERVED_MAX_NOT_WCET
QUEUE_MODEL_ASSUMPTION_FAILED
ARRIVAL_ENVELOPE_VIOLATED
SERVICE_ENVELOPE_UNSUPPORTED
LITTLE_LAW_SCOPE_MISMATCH
ENERGY_ESTIMATE_NOT_MEASUREMENT
CARBON_CONTEXT_MISSING
ATTRIBUTION_POLICY_MISSING
INSTRUMENTATION_PERTURBATION_UNRESOLVED
RESOURCE_OBSERVER_UNAUTHORIZED
QUANTITATIVE_REQUIREMENT_UNSAT
```

Diagnostics should identify a causal slice through metric, workload, target, evidence, and requirement dependencies.

---

## 52. Relationship to RFC 0004 recursive refinement

A failed quantitative claim may generate a bounded diagnostic/refinement loop:

```text
observe quantitative failure
    -> localize bottleneck/resource obligation
    -> propose candidate
    -> verify semantic relations
    -> re-establish quantitative evidence
    -> compare ResourceDelta
    -> promote only through RFC 0018/0033 policy
```

A performance regression is therefore not merely a benchmark number. It is an evidence-bearing delta linked to the exact artifact, target, workload, and realization.

---

## 53. Relationship to RFC 0007 proof core

Resource proofs may be:

```text
kernel proof terms
independently checkable certificates
external verifier results
empirical observations
```

These retain different evidence classes.

Resource analysis need not expand the trusted proof kernel unless a specific proof language is intentionally admitted.

---

## 54. Relationship to RFC 0018 assurance

RFC 0032 supplies evidence such as:

```text
ResourceBoundWitness
PerformanceObservation
QuantileReport
EnergyMeasurement
```

RFC 0018 determines:

- trust in the producer;
- freshness;
- independence;
- conflict;
- whether the evidence supports a claim;
- whether the claim authorizes an action.

A resource quantity never carries ambient promotion authority.

---

## 55. Relationship to RFC 0020 relations

Quantitative relations may include:

```text
NoMoreWorkThan
NoMorePeakMemoryThan
LatencyRefinesUnder<W,T>
EnergyEquivalentUnder<M>
MetricDominates
BoundedResourceAgreement
```

The same candidates may satisfy one quantitative relation and fail another.

No universal `PerformanceEquivalent` relation should be assumed.

---

## 56. Relationship to RFC 0021 numeric semantics

Resource arithmetic itself requires numeric semantics.

Examples:

- unit conversion;
- percentile interpolation;
- integration of power;
- statistical estimators;
- symbolic polynomial evaluation.

Claims must preserve rounding/error semantics where they affect thresholds.

A candidate that gains speed through invalid numerical approximation remains semantically invalid regardless of quantitative improvement.

---

## 57. Relationship to RFC 0022 bounded computation

Resource budgets can terminate or restrict executions.

`BUDGET_EXHAUSTED` and `RESOURCE_BUDGET_EXHAUSTED` remain bounded observations, not proofs of divergence/unboundedness.

Resource analyzers may themselves run under finite analysis budgets and return `UNRESOLVED`.

---

## 58. Relationship to RFC 0023 time

RFC 0032 distinguishes abstract work from elapsed time, but many metrics use RFC 0023 time domains:

- rates;
- latency;
- observation windows;
- SLO windows;
- power integration;
- queueing response times;
- WCET applicability.

Clock identity/uncertainty remains RFC 0023-owned.

---

## 59. Relationship to RFC 0024 information flow

Resource metrics can leak protected information through:

- timing;
- cache activity;
- allocation size;
- instruction counts;
- power traces;
- scheduler utilization.

RFC 0032 therefore cannot authorize observation merely because a metric exists.

Resource observation must satisfy RFC 0024 observer/channel policy and RFC 0008 capability authority.

---

## 60. Relationship to RFC 0028 distribution

RFC 0028 determines whether a distributed protocol is correct.

RFC 0032 can compare:

- messages;
- bytes;
- rounds;
- quorum probes;
- tail latency;
- coordination work;
- replication storage;
- expected recovery cost.

Correctness requirements remain prior constraints.

---

## 61. Relationship to RFC 0029 placement

RFC 0029 establishes valid placement, locality, and migration.

RFC 0032 quantifies:

- NUMA/device transfer;
- remote communication;
- bandwidth pressure;
- data movement;
- migration cost;
- energy consequences;
- placement-dependent latency.

Topology facts remain RFC 0029/0017 evidence inputs.

---

## 62. Relationship to RFC 0030 lifecycle

RFC 0030 establishes valid transitions.

RFC 0032 quantifies:

- temporary surge;
- downtime;
- rollout duration;
- dual-write/read overhead;
- migration cost;
- canary exposure;
- rollback preparation.

A cheaper transition remains invalid if RFC 0030 intermediate-state semantics fail.

---

## 63. Relationship to RFC 0031 derivation

RFC 0031 establishes artifact derivation/provenance.

RFC 0032 quantifies:

- build resources;
- independent-reproduction resources;
- cache storage;
- proof/attestation generation;
- evidence storage;
- reverification cost.

The existence of a cheaper builder does not weaken provenance or bootstrap requirements.

---

## 64. Boundary with RFC 0033

RFC 0032 ends once it has produced:

- semantically valid candidates;
- quantitative facts/bounds/observations;
- hard quantitative requirement results;
- componentwise comparisons/deltas;
- uncertainty/evidence scope.

RFC 0032 may state:

```text
A:
    latency = 10 ms
    memory = 500 MiB
    energy = 4 J

B:
    latency = 12 ms
    memory = 250 MiB
    energy = 2 J
```

It may not conclude:

```text
A is better
```

without an RFC 0033 preference/utility/selection policy.

This boundary prevents hidden scalarization from becoming semantic truth.

---

## 65. Security and adversarial measurement considerations

Quantitative evidence may be manipulated through:

- benchmark selection;
- omitted outliers;
- warmup selection;
- clock manipulation;
- target substitution;
- instrumentation tampering;
- telemetry filtering;
- workload misclassification.

RFC 0032 should therefore preserve raw measurement provenance and enable RFC 0018 appraisal rather than trusting summarized metrics by default.

---

## 66. Open research questions

Before stabilization, research should answer:

1. Which resource-algebra abstractions are general enough without overfitting semiring theory?
2. Which metric families belong in the kernel semantic model versus libraries/profiles?
3. How should symbolic size/resource expressions be canonically represented?
4. Which AARA/resource certificates can be checked by a very small verifier?
5. How should peak-liveness memory witnesses compose with SSA/lifetime analysis?
6. How should probabilistic cost evidence interact with RFC 0018 uncertainty models?
7. Which queueing/network-calculus models deserve first-class import schemas?
8. How should measurement uncertainty be represented without becoming a universal confidence score?
9. How should resource observations preserve privacy under RFC 0024?
10. How should target/placement changes invalidate quantitative evidence incrementally?
11. How should verification cost be incorporated into recursive refinement budgets?
12. Which metrics should the first Fabric experiment record directly versus derive?
13. How should model-token/inference resources be represented without contaminating general-purpose language semantics?
14. How should monetary/carbon valuation models be versioned and kept clearly contextual?
15. Which quantitative properties should be preserved explicitly through HIR/SSA/backend transformations?

---

## 67. 1.0 research threshold additions

RFC 0032 proposes that a 1.0 research designation additionally require all of the following.

### 67.1 Quantitative identity and algebra

- a versioned quantitative model distinguishing dimension, unit, metric, quantity shape, accounting scope, and resource algebra;
- mechanical dimensional/unit consistency for the supported subset;
- at least two metrics whose composition differs, demonstrating that one universal additive `cost` algebra is rejected;
- explicit hard distinction between abstract work and physical elapsed time.

### 67.2 Complexity and bounds

- versioned `SizeMeasure`, `WorkClaim`, `SpanClaim`, and `ResourceBound` artifacts;
- at least one symbolic concrete bound over an indexed/size-measured input;
- at least one asymptotic claim represented separately from a concrete bound;
- at least one amortized-cost study whose worst-case individual operation remains more expensive;
- at least one resource-analysis failure that remains `UNRESOLVED` rather than becoming `UNBOUNDED`;
- at least one independently checkable resource-bound or potential certificate.

### 67.3 Accounting

- versioned capacity/reservation/allocation/consumption/budget artifacts;
- at least one fixture proving cumulative allocation and peak live memory are distinct;
- at least one hard capacity/budget rejection of an otherwise semantically valid candidate;
- explicit inclusive/exclusive accounting or an equivalent scope model;
- explicit one-time/recurring and cold/warm distinctions where applicable.

### 67.4 Performance measurement

- versioned workload and performance-observation artifacts binding artifact, target, workload, environment, instrument, and observation window;
- at least one empirical result that cannot satisfy a universal resource bound;
- at least one observed-maximum result that cannot satisfy a WCET claim;
- at least one stale/target-mismatched performance observation rejected by applicability checking;
- explicit instrumentation identity for profiled measurements.

### 67.5 Statistical/tail semantics

- versioned distribution/quantile claims;
- at least one P99-or-similar claim that passes while an all-executions upper-bound claim fails;
- explicit mean/percentile/max distinctions;
- at least one fan-out/tail study demonstrating that aggregate latency cannot be inferred from mean child latency alone;
- at least one expected-cost claim kept distinct from worst-case cost.

### 67.6 Queueing and deterministic envelope semantics

- at least one queueing study where per-request work remains constant while response latency changes under load;
- at least one Little's-Law-style operational check with scope validation;
- at least one deterministic arrival/service envelope or equivalent hard backlog/delay/buffer witness;
- explicit separation between deterministic hard bounds and stochastic/high-percentile evidence.

### 67.7 QoS and violation budgets

- versioned quantitative indicator/objective artifacts;
- at least one objective over a request class, percentile/fraction, threshold, and time window;
- at least one violation/error-budget artifact kept distinct from failure probability;
- at least one lifecycle or operational decision consuming RFC 0032 QoS evidence through RFC 0018 without treating the metric as ambient authority.

### 67.8 Energy/power/contextual impact

- explicit `Power != Energy` semantics;
- one pair of executions with equal energy but different peak power or duration;
- an energy estimate and direct energy observation represented as different evidence classes;
- one contextual carbon estimate proving that identical energy can map to different impact under different time/location models;
- explicit separation of intrinsic resource usage from monetary valuation.

### 67.9 Distributed/storage/build/verification accounting

- at least one distributed realization comparing messages, bytes, and rounds as separate metrics;
- at least one persistence realization exposing logical versus physical storage or amplification;
- at least one build/reproduction cost artifact linked to RFC 0031 derivation identity;
- explicit proof-generation versus proof-checking cost;
- at least one recursive-refinement run operating under explicit verifier/experiment/model-call budgets.

### 67.10 Forge quantitative filtering

- at least one Forge study with multiple semantically valid candidate realizations and multiple hard quantitative requirements;
- at least one candidate rejected only because a hard quantitative requirement fails;
- at least one faster candidate rejected earlier because a non-quantitative semantic relation fails;
- at least two candidates surviving RFC 0032 so RFC 0033 must make the actual preference decision;
- one `ResourceDelta` comparing surviving candidates without scalarizing them;
- one quantitative witness generated by an untrusted analysis/search tool and independently checked by a smaller verifier;
- one bounded benchmark/measurement result that remains explicitly bounded empirical evidence rather than being upgraded to universal performance truth.

---

## 68. First implementation sequence

A conservative implementation sequence is:

1. `ResourceDimension`, `ResourceUnit`, `ResourceMetric`, `ResourceQuantity`;
2. dimensional/unit checker;
3. `CostModel`, `SizeMeasure`, `WorkClaim`, `SpanClaim`;
4. simple affine/polynomial `ResourceBound` representation;
5. bounded certificate checker for one traversal example;
6. `ResourceCapacity`, `ResourceBudget`, `QuantitativeFeasibility`;
7. `WorkloadModel`, `PerformanceObservation`;
8. quantile computation over a frozen corpus;
9. `ResourceDelta`;
10. first Forge CPU/SIMD/GPU/distributed realization study;
11. one verification-cost study;
12. one queueing/envelope study after the core artifacts stabilize.

This sequence deliberately avoids making a general profiler or optimizer before metric identity and hard quantitative semantics are stable.

---

## 69. Non-goals for the first implementation

The first implementation does not need:

- universal automatic complexity inference;
- universal WCET proof;
- a complete statistical framework;
- production SLO monitoring;
- full network calculus;
- detailed CPU microarchitectural simulation;
- cloud-billing integration;
- carbon-optimization infrastructure;
- automatic global utility optimization;
- one universal resource type system.

The purpose is to establish semantic distinctions and independently checkable bounded examples.

---

## 70. Final constitutional summary

RFC 0032 establishes quantity as a semantic dimension of machine-native programs without turning optimization preference into hidden language meaning.

The core rules are:

> **Resources are typed quantities, not generic numbers.**

> **Each metric carries its own composition and observation semantics.**

> **Abstract work, physical time, capacity, usage, statistical observation, and proven bounds remain distinct.**

> **Workload, target, environment, window, and evidence are part of quantitative claim identity whenever they affect meaning.**

> **Hard quantitative requirements filter realizations before preference optimization.**

> **Forge may infer, measure, profile, simulate, and search aggressively, but narrow verifiers should check dimensions, bounds, budgets, arithmetic, scope, and certificate obligations independently whenever feasible.**

The machine-native objective is therefore not:

```text
make program faster
```

It is:

```text
preserve semantic contract R
under workload W
on target context T
while establishing explicit quantitative relations Q
and satisfying every hard quantitative requirement B
```

Only after that does RFC 0033 ask which admissible point in the multidimensional quantitative space should actually be chosen.

---

## References and theory anchors

The following theory and systems are informative anchors rather than normative dependencies:

- Brent-style work/span parallel complexity and work-depth models;
- Automatic Amortized Resource Analysis and RaML;
- sized-type resource analysis;
- Atkey-style Quantitative Type Theory;
- graded modal and substructural type systems;
- resource-aware session types;
- separation logic with time/resource credits;
- abstract interpretation for resource bounds and WCET;
- Network Calculus;
- Real-Time Calculus / Modular Performance Analysis;
- queueing theory and Little's Law;
- probabilistic expected-resource analysis;
- PRISM-style quantitative reward model checking;
- weighted automata and weighted transition systems;
- Tail-at-Scale-style distributed tail analysis;
- SLI/SLO/error-budget operational semantics;
- Roofline operational-intensity/bottleneck analysis;
- energy-credit and energy accounting models;
- contextual carbon-accounting methods;
- proof-producing/certificate-checked quantitative analysis.

These theories are intentionally plural. RFC 0032 defines the semantic interfaces through which they can provide evidence without making any one analysis framework the universal definition of resource meaning.
