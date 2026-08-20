# RFC 0034: Machine-Native Test, Fuzz, Coverage, Experiment, Benchmark, Oracle, and Empirical-Evidence Semantics

- **Status:** Draft
- **Track:** Foundational semantics
- **Depends on:** RFCs 0001–0033
- **Primary owner:** empirical verification, testing, experimentation, benchmarking, coverage, oracles, replay, and empirical-evidence applicability

## 1. Summary

MNCS needs a first-class semantic model for empirical knowledge acquisition.

Testing, fuzzing, benchmarks, differential execution, metamorphic testing, mutation testing, symbolic/concolic test generation, schedule exploration, fault injection, statistical experiments, and replay are all useful because they execute concrete or sampled behaviors and produce observations. They are not interchangeable with formal proof, and they are not interchangeable with one another.

This RFC therefore treats experimentation as **evidence-producing computation**.

The central object is not merely a `test`. It is an `Experiment`: an explicit relation among a subject, hypothesis/property, input model, generator or corpus, environment, target, execution strategy, observer, oracle, coverage model, repetition/stopping policy, and inference rule. Individual runs produce observations. Oracles interpret observations. Empirical claims remain scoped to the exact experimental applicability envelope that made those observations meaningful. RFC 0018 decides what those claims authorize.

The constitutional rule is:

> **A test is not evidence merely because it executed. Empirical evidence is a scoped relation among a subject, experiment, inputs, environment, oracle, observations, coverage model, and inference rule.**

Four immediate consequences follow:

1. **Passing observations do not constitute universal proof. A valid counterexample can, however, refute a universal claim.**
2. **Coverage measures where an experiment looked; it does not by itself establish that what was observed was correct.**
3. **An oracle can be wrong independently of the implementation under test.**
4. **Experiment generation, execution, observation, oracle evaluation, inference, and assurance appraisal are separate stages.**

This RFC is intentionally broader than any one test framework. QuickCheck, libFuzzer/AFL-style coverage-guided fuzzing, KLEE, DART/concolic execution, mutation testing, MC/DC, combinatorial interaction testing, CHESS/DPOR, statistical model checking, sequential testing, and benchmark-design methods are realization and evidence families, not the language ontology.

---

## 2. Motivation

MNCS already distinguishes proof from observation, evidence from authorization, semantic relations from finite agreement, bounded execution from universal behavior, and quantitative metrics from selection policy. Without RFC 0034, however, empirical work still risks collapsing into conventional CI vocabulary:

```text
28,430 tests passed
coverage: 94%
benchmark: 12 ms
```

Those statements omit the facts needed to know what the observations mean:

- Which exact artifact was exercised?
- Which target and environment?
- Which input domain?
- Which generator/distribution/corpus?
- Which schedules or failures?
- Which observer and instrumentation?
- Which oracle?
- Which coverage criterion?
- Which experiment budget?
- Which statistical method?
- Which assumptions?
- Which results were flaky or unresolved?
- Which portions of the claim remain unexplored?

MNCS should instead be able to express:

```text
Claim:
    Body/SSA execution relation R

Experiment campaign:
    generator = grammar-aware edge-biased generator G7
    seed = S
    generated subjects = 41,230
    semantic operation coverage = 97%
    SSA branch coverage = 91%
    modeled 3-way interaction coverage = 100%
    oracle = body evaluator vs SSA evaluator under R
    symbolic path classes = 612
    fuzz budget = 19.4 worker-hours
    mutation results = 182 killed / 4 equivalent / 4 unresolved
    counterexamples = 3 relation-distinct failures
    reduced counterexamples = 3 minimal-under-shrink-relation artifacts
    target = T
    compiler = C
    backend = B

Conclusion:
    universal relation R REFUTED
```

Or, when no counterexample is found:

```text
Conclusion:
    bounded empirical support under applicability envelope E

NOT:
    universal proof
```

This distinction is foundational for a machine-native system that will automatically perform large volumes of experiments.

---

## 3. Scope

RFC 0034 owns:

- experiment, run, campaign, and experimental-unit identity;
- test-case, generator, corpus, mutation, and stimulus identity;
- oracle identity, applicability, strength, dependencies, and evaluation;
- property-based, differential, metamorphic, mutation, model-conformance, benchmark, schedule, and fault experiments;
- coverage criteria and reports;
- fuzzing/search state and empirical exploration feedback;
- symbolic/concolic generation as test-generation realizations;
- counterexample capture, shrinking, delta reduction, clustering, and replay artifacts;
- sampling, repetition, blocking, randomization, statistical hypothesis/inference, and sequential experiment metadata;
- benchmark methodology, warmup/steady-state classification, and measurement hierarchy;
- empirical applicability envelopes, empirical claim scope, and narrow invalidation;
- experiment manifests, provenance hooks, and empirical deltas;
- deterministic checking of experiment identities, coverage arithmetic, oracle applicability, replay manifests, and statistical-result contracts.

RFC 0034 does **not** own:

- the underlying semantic relation being tested — RFC 0020 and the domain RFCs own that;
- formal proof — RFC 0007;
- evidence composition, trust, and authorization — RFC 0018;
- concurrency/failure/distribution semantics — RFCs 0010, 0011, 0028;
- target/environment meaning — RFC 0017;
- quantitative metric semantics — RFC 0032;
- whether more experimentation is worth its cost or which candidate should win — RFC 0033;
- harness/build provenance itself — RFC 0031.

---

## 4. Constitutional distinctions

The following distinctions are normative design constraints.

### 4.1 Test identity

- A `TestCase` is not a `TestExecution`.
- A `TestExecution` is not an `Observation`.
- An `Observation` is not an `OracleEvaluation`.
- An `OracleEvaluation` is not an `EmpiricalClaim`.
- Repeating one test case creates new execution identities even when stimulus identity is unchanged.

### 4.2 Empirical support and proof

- A passing finite sample does not establish a universal property.
- Property-based testing samples a quantified property; it does not prove the quantifier.
- A bounded exhaustive campaign may be complete only for its explicit finite domain.
- Budget exhaustion does not establish bug absence.
- Solver timeout or `UNKNOWN` does not establish unreachability.
- Statistical support does not become formal proof.

### 4.3 Counterexample asymmetry

For a claim of the form:

```text
forall x in D. P(x)
```

one valid `x` in `D` such that `not P(x)` may refute the universal claim. Conversely, finitely many successful samples support only the sampled/bounded claim unless an explicit statistical inference model authorizes a different scoped conclusion.

A counterexample is valid only when:

- it belongs to the quantified domain;
- the subject identity is correct;
- the observation is valid under the declared observer;
- the oracle/relation evaluation is applicable;
- the failure is not solely a harness/oracle defect.

### 4.4 Oracle semantics

- Oracle correctness is independent of subject correctness.
- Crash freedom is not semantic correctness.
- A differential disagreement establishes inconsistency under a relation, not which implementation is guilty.
- A metamorphic relation needs its own semantic/evidential justification.
- A weak oracle cannot silently establish a stronger property.
- Oracle dependencies and common-mode failure paths must remain visible.

### 4.5 Coverage

- Coverage is not correctness.
- Coverage is not adequacy.
- Adequacy is purpose-, claim-, fault-model-, and assurance-policy-relative.
- There is no universal coverage percentage.
- Statement, branch, condition, MC/DC, path, dataflow, input-domain, t-way, sequence, state, transition, schedule, fault, mutation, requirement, and semantic coverage are distinct.
- `100%` under one coverage model does not imply `100%` under another.
- `Uncovered` does not imply `ReachableButUnexplored`.

### 4.6 Generation and fuzzing

- Generator identity and distribution are part of empirical-evidence identity.
- `random` is incomplete without PRNG/seed/generator/distribution/sampling method.
- Fuzzing strategy chooses what to explore; the oracle determines what constitutes a violation.
- Coverage-guided novelty is not correctness.
- Corpus retention/minimization is relative to a declared feature model.
- Shrunk does not automatically mean causal or globally minimal.

### 4.7 Mutation testing

- Mutation adequacy is relative to a mutation/fault model.
- A surviving mutant may be undetected, equivalent, invalid, or unresolved.
- Mutation score does not directly measure program correctness.
- Equivalent-mutant classification is an RFC 0020 relation problem, not a scoring convenience.

### 4.8 Concurrency and failure experiments

- Input identity and schedule identity are distinct.
- Finite schedule completeness is always domain- and scheduler-model-scoped.
- Fault injection demonstrates observed behavior under injected traces; it does not automatically prove universal fault tolerance.
- Crash-point, partition-pair, failure-combination, and recovery-path coverage are different.

### 4.9 Benchmark/statistical semantics

- Benchmark values without workload, target, environment, subject, and method are incomplete evidence.
- Cold, warm, and steady-state observations are distinct.
- Repeated observations are not automatically independent observations.
- Failure to reject a null hypothesis does not prove it true.
- Statistical significance is not practical significance.
- Observed maximum is not a universal worst-case bound.
- Confidence-interval semantics must name the statistical interpretation/model.
- Multiple-comparison families must be explicit when inference depends on them.

### 4.10 Replay and flakiness

- Reproduction is not independent rediscovery.
- Replayability does not imply uncontrolled execution is deterministic.
- A flaky test is evidence of outcome variability or uncontrolled dependence, not permission to retry until success.
- Aggregate retry policy must preserve constituent observations.

### 4.11 Provenance and scope

- Harness, oracle, generator, instrumentation, benchmark, and subject provenance are part of empirical provenance.
- Empirical claims remain scoped to an applicability envelope.
- Evidence transfer to another subject/target/environment requires an explicit relation.
- Machine-generated experiment plans remain untrusted proposals.

---

## 5. Core semantic pipeline

The canonical pipeline is:

```text
ExperimentSpecification
        |
        v
ExperimentPlan
        |
        v
ExperimentRun(s)
        |
        v
ObservationSet
        |
        v
OracleEvaluation(s)
        |
        v
EmpiricalInference
        |
        v
EmpiricalClaim / Counterexample
        |
        v
RFC 0018 Appraisal
```

Each arrow must preserve subject, scope, dependencies, provenance, and evidence identity.

The generation/search machinery may be untrusted. The interpretation/checking path should be narrow and independently checkable wherever feasible.

---

## 6. Experiment identity

### 6.1 `Experiment`

```text
Experiment {
    id
    subject
    property_or_hypothesis
    input_model
    environment_model
    target_scope
    generation_strategy
    execution_strategy
    observation_model
    oracle
    coverage_models[]
    repetition_policy?
    sampling_plan?
    stopping_policy
    inference_rule
    resource_budget?
    dependencies[]
}
```

Changing any field that affects meaning changes experiment identity.

### 6.2 `ExperimentRun`

```text
ExperimentRun {
    experiment
    run_id
    subject_instance
    concrete_input
    generator_state?
    seed?
    schedule?
    fault_trace?
    environment_snapshot
    target_snapshot
    instrumentation
    start/end observations
    outcome
    raw_observations
}
```

### 6.3 `ExperimentCampaign`

A campaign groups related experiments/runs while preserving per-run identities.

```text
ExperimentCampaign {
    campaign_id
    experiments[]
    campaign_goal
    shared_budget
    shared_dependencies
    coverage_aggregation_rules
    stopping_policy
}
```

Campaign aggregation must never erase contradictory/flaky constituent results.

---

## 7. Stimulus semantics

### 7.1 `TestCase`

```text
TestCase {
    input
    preconditions
    environment_requirements
    schedule?
    fault_scenario?
    expected_observation_scope
}
```

### 7.2 `InputModel`

`InputModel` identifies the modeled domain. It may describe:

- algebraic values;
- structural sizes;
- grammars;
- protocol actions;
- distributed histories;
- schedule choices;
- failure traces;
- parameter combinations;
- workloads.

Input-model completeness must never be assumed merely because generation is exhaustive relative to the model.

### 7.3 `TestGenerator`

```text
TestGenerator {
    input_model
    generation_algorithm
    distribution
    size_policy
    boundary_policy
    constraint_policy
    seed_model
    version
}
```

A generator may be random, exhaustive, grammar-based, property-directed, search-based, model-based, combinatorial, symbolic, concolic, evolutionary, LLM-generated, or hybrid.

### 7.4 Corpus semantics

```text
TestCorpus {
    entries[]
    feature_model
    provenance
    retention_policy
}

CorpusEntry {
    input
    reason
    covered_features[]
    source_campaign?
    prior_failure?
    lineage
}
```

A corpus is not merely a directory of blobs. Retention reason and feature coverage are first-class.

---

## 8. Property-based testing

A property experiment separates universal property syntax from sampled execution.

```text
PropertyExperiment {
    property
    generator
    classification?
    shrinker?
    sample_policy
}
```

Possible outcomes include:

```text
COUNTEREXAMPLE_FOUND
NO_COUNTEREXAMPLE_OBSERVED
GENERATOR_EXHAUSTED_FINITE_DOMAIN
BUDGET_EXHAUSTED
ORACLE_UNRESOLVED
INVALID_EXPERIMENT
```

`NO_COUNTEREXAMPLE_OBSERVED` is bounded support. It is not `PROVED`.

For a genuinely finite domain that was exhaustively enumerated and checked, the claim may be complete **for that exact finite domain**.

---

## 9. Shrinking and counterexample reduction

### 9.1 Shrink relation

```text
ShrinkRelation<T> {
    smaller(a,b)
    preserves_domain
}
```

### 9.2 `CounterexampleReduction`

```text
CounterexampleReduction {
    original
    predicate
    reduction_relation
    algorithm
    reduced
    minimality_kind
    search_coverage
}
```

Minimality kinds include:

```text
ONE_MINIMAL
LOCALLY_MINIMAL
GLOBALLY_MINIMAL
MINIMAL_UNDER_RELATION
UNKNOWN
```

### 9.3 Delta debugging

Delta debugging is a realization family for reducing failure-inducing configurations or event sequences while repeatedly evaluating a failure predicate.

It must preserve:

- original failure identity;
- decomposition relation;
- tested subsets/deltas where retained;
- final minimality scope;
- oracle identity.

A reduced counterexample is not automatically a root-cause proof. RFC 0004 causal slicing remains separate.

---

## 10. Oracle model

### 10.1 `Oracle`

```text
Oracle {
    id
    subject_class
    property/relation
    observation_requirements
    applicability
    method
    dependencies
    evidence
    strength
}
```

### 10.2 Oracle strengths

Candidate strengths include:

```text
EXACT_REFERENCE
RELATION_CHECKER
NECESSARY_CONDITION
PARTIAL
METAMORPHIC
HEURISTIC
CRASH_ONLY
SANITIZER
STATISTICAL
```

The list is extensible. Strength is not a single total order.

### 10.3 Oracle applicability

An oracle may be valid only for:

- a subset of inputs;
- a target family;
- a protocol version;
- an observer model;
- a numeric profile;
- an execution mode;
- a workload class.

Out-of-scope use produces `UNRESOLVED` or experiment invalidity rather than fabricated PASS/FAIL.

### 10.4 Oracle disagreement

If multiple oracles disagree:

```text
OracleConflict {
    observation
    oracle_results[]
    shared_dependencies
}
```

RFC 0018 appraises the conflict. Majority vote is not a default semantic rule.

---

## 11. Differential testing

```text
DifferentialExperiment {
    subjects[]
    relation
    inputs
    observer
    oracle_policy
}
```

If subjects disagree under the required RFC 0020 relation, the relation claim may be refuted by a concrete counterexample.

However:

```text
A output != B output
```

establishes only disagreement until an independent specification/oracle determines whether A, B, both, or the relation assumption is wrong.

Differential testing is especially valuable for:

- semantic body vs HIR vs SSA interpreters;
- multiple backends;
- compiler differential testing;
- codec implementations;
- replicated-state implementations;
- reference numerical implementations.

---

## 12. Metamorphic testing

```text
MetamorphicRelation {
    input_relation
    output_relation
    applicability
    semantic_justification
}

MetamorphicExperiment {
    base_input
    transformed_input
    base_observation
    transformed_observation
    relation
}
```

Metamorphic testing is useful where no exact expected output is available but a relation between executions is known.

The metamorphic relation itself is a semantic claim and cannot grant itself oracle authority.

---

## 13. Mutation testing

```text
MutationModel {
    mutation_operators[]
    applicability
    fault_family
}

MutationExperiment {
    baseline_subject
    mutant
    test_suite
    oracle
}
```

Mutant dispositions:

```text
KILLED
SURVIVED
EQUIVALENT
INVALID_MUTANT
UNRESOLVED
```

### 13.1 Mutation adequacy

```text
MutationAdequacy {
    mutation_model
    considered_mutants
    killed
    survived
    equivalent
    invalid
    unresolved
    scoring_policy
}
```

A score is meaningful only relative to the mutation model and scoring policy.

### 13.2 Equivalent mutants

Equivalent-mutant claims should use RFC 0020 relation evidence. `survived` does not imply `equivalent`; `not killed` is not a semantic classification.

---

## 14. Coverage semantics

### 14.1 `CoverageModel`

```text
CoverageModel {
    id
    feature_domain
    covered_when
    aggregation
    denominator_definition
    applicability
}
```

### 14.2 Coverage families

MNCS should allow at least:

```text
StatementCoverage
BranchCoverage
ConditionCoverage
MCDCCoverage
PathCoverage
DataflowCoverage
InputPartitionCoverage
TWayCombinationCoverage
SequenceCoverage
StateCoverage
TransitionCoverage
ScheduleCoverage
FaultCoverage
MutationCoverage
RequirementCoverage
SemanticFeatureCoverage
EmpiricalObligationCoverage
```

### 14.3 `CoverageReport`

```text
CoverageReport {
    model
    subject
    campaign
    covered_features
    uncovered_features
    unreachable_features?
    unresolved_features?
    ratio?
    evidence
}
```

`uncovered`, `unreachable`, and `unresolved` are separate states.

### 14.4 Coverage arithmetic

Percentage is optional. When used, denominator identity is mandatory.

```text
CoverageRatio = covered_features / modeled_features
```

Changing the feature model changes the meaning even if the numeric ratio remains identical.

---

## 15. Semantic coverage

MNCS has semantic structure unavailable to conventional coverage tools.

`SemanticCoverageModel` may expose features such as:

- contracts exercised;
- effects exercised;
- capability paths exercised;
- failure modes exercised;
- numeric edge semantics exercised;
- serialization presence/unknown-field/migration cases exercised;
- persistence recovery cases exercised;
- distributed consistency histories exercised;
- authority boundaries exercised;
- lifecycle transitions exercised;
- placement/migration constraints exercised;
- selection policies exercised.

Semantic coverage should be distinct from HIR, SSA, backend-block, or machine-code coverage.

Mappings may allow evidence such as:

```text
SemanticFeature S
    lowered_to -> SSA blocks B1..B7
```

but seven backend blocks must not inflate one semantic feature into seven semantic obligations.

---

## 16. Combinatorial interaction testing

```text
CombinatorialModel {
    parameters
    value_domains
    constraints
    interaction_strength t
}
```

```text
CoveringArrayCampaign {
    model
    generated_cases
    covered_tuples
}
```

A claim of `100% t-way coverage` is relative to the modeled parameter/value domain and constraints.

Unmodeled values or parameters remain outside the evidence envelope.

Sequence/ordered covering models may be used for stateful workflows where event order matters.

---

## 17. Fuzzing as empirical search

### 17.1 Fuzz campaign

```text
FuzzCampaign {
    seed_corpus
    mutation_strategy
    seed_selection
    feedback_model
    novelty_relation
    oracle
    budget
    stopping_policy
}
```

### 17.2 Feedback

Feedback can include:

```text
NativeEdgeNovelty
PathNovelty
ValueProfileNovelty
StateNovelty
TransitionNovelty
SemanticCoverageNovelty
FailureBehaviorNovelty
ProtocolStateNovelty
ObligationNovelty
```

Coverage-guided, Markov-chain-guided, search-based, evolutionary, and learned fuzzing remain realization families.

### 17.3 Fuzz search state

```text
FuzzSearchState {
    corpus
    executions
    features_seen
    failures_seen
    budget_consumed
    pending_inputs
    search_strategy_state
}
```

Budget exhaustion yields a bounded search conclusion, not bug absence.

---

## 18. Symbolic and concolic test generation

### 18.1 Symbolic generation

```text
SymbolicTestGeneration {
    symbolic_inputs
    path_model
    solver
    exploration_policy
    bounds
}
```

Outputs may be:

- concrete test cases satisfying path constraints;
- path-reachability witnesses;
- `UNSAT` path claims when independently justified;
- `UNKNOWN`/timeout results.

Solver `UNKNOWN` or budget exhaustion must not become `UNREACHABLE`.

### 18.2 Concolic generation

Concolic realization families alternate concrete execution and symbolic path inversion:

```text
execute
observe path
construct path predicate
select branch alternative
solve
execute generated input
```

Generated test cases remain ordinary experiment stimuli after generation.

---

## 19. Model-based and trace-conformance testing

```text
ModelConformanceExperiment {
    model
    implementation
    generated_trace
    observation_trace
    conformance_relation
}
```

Finite trace conformance supports only the explored trace/domain unless exhaustive finite-state/model evidence establishes stronger bounded completeness.

Model correctness remains a separate assumption/evidence problem.

---

## 20. Concurrent schedule exploration

### 20.1 Schedule identity

```text
ConcurrentTestCase {
    input
    operations
    schedule
    scheduler_model
}
```

Same input under different schedules is not the same experiment execution.

### 20.2 Systematic concurrency testing

CHESS-like systematic scheduling is a realization family for bounded exploration.

### 20.3 Dynamic partial-order reduction

DPOR-like strategies may collapse schedules that differ only by independent transition order.

```text
ScheduleCoverage {
    raw_schedules?
    dependency_relation
    equivalence_classes
    explored_classes
    bounds
}
```

A complete bounded result must name:

- thread/process count;
- operation/event bound;
- dependency model;
- scheduler assumptions;
- input model.

### 20.4 Concurrent history oracles

RFC 0020/RFC 0028 relation checkers may evaluate finite histories for:

```text
Linearizable
SequentiallyConsistent
CausallyConsistent
EventuallyConvergent
```

A history can pass one relation and fail another.

---

## 21. Fault injection and resilience experiments

```text
FaultScenario {
    fault_model
    injection_points
    schedule
    duration
    recovery_conditions
}
```

Potential coverage models include:

```text
CrashPointCoverage
PartitionPairCoverage
MessageFaultCoverage
FailureCombinationCoverage
RecoveryPathCoverage
```

Fault experiments reuse domain semantics from RFCs 0011, 0026, 0028, 0029, and 0030. They do not redefine failure correctness.

---

## 22. Observation and instrumentation

### 22.1 Observation model

```text
ObservationModel {
    observable_events
    sampling_policy
    precision
    perturbation_model?
    authority
}
```

### 22.2 Instrumentation identity

Sanitizers, profilers, tracing, deterministic schedulers, fault injectors, and measurement hooks may alter behavior.

```text
InstrumentedSubjectIdentity != ProductionSubjectIdentity
```

unless an explicit RFC 0020 relation establishes the relevant equivalence/refinement.

### 22.3 Instrumentation authority

Observation may itself require RFC 0008 authority and may expose RFC 0024-protected information. More observation is not automatically permitted.

---

## 23. Flakiness and outcome variability

```text
FlakyObservation {
    test_case
    requested_conditions
    outcomes
    observed_variance
    unresolved_dependencies
}
```

Possible interpretations include:

- intentional nondeterminism;
- uncontrolled scheduler behavior;
- environment drift;
- race;
- external service variability;
- measurement noise;
- oracle instability.

A retry policy is represented explicitly:

```text
RetryAggregationPolicy {
    attempts
    success_rule
}
```

It must preserve each attempt's observations and may not erase failures.

---

## 24. Replay and rediscovery

### 24.1 Replay bundle

```text
ReplayBundle {
    subject_identity
    harness_identity
    oracle_identity
    concrete_input
    generator_seed?
    schedule_trace?
    clock_trace?
    network_trace?
    fault_trace?
    environment_snapshot
    target_snapshot
    instrumentation
}
```

### 24.2 Replay outcomes

```text
REPRODUCED
NOT_REPRODUCED
PARTIALLY_REPRODUCED
ENVIRONMENT_UNAVAILABLE
UNRESOLVED
```

### 24.3 Independent rediscovery

Independent rediscovery by another campaign is distinct evidence from replaying the original trace.

---

## 25. Failure fingerprinting and clustering

Raw failure count is usually not defect count.

```text
FailureFingerprint {
    relation
    violated_obligation?
    stack_signature?
    semantic_counterexample_shape?
    causal_slice?
}
```

Different clustering relations may group by:

- native stack;
- violated obligation;
- semantic counterexample;
- causal slice;
- oracle result.

Clustering criterion is part of result identity.

---

## 26. Regression artifacts

A discovered counterexample should be able to produce a lineage chain:

```text
Failure
   -> ReducedCounterexample
   -> Repair
   -> RegressionTest
```

```text
RegressionArtifact {
    input
    violated_relation
    original_failure
    repair?
    expected_behavior
    oracle
    provenance
}
```

A regression suite is therefore a provenance-bearing corpus, not merely a list of examples.

---

## 27. Statistical experiment semantics

### 27.1 Sampling plan

```text
SamplingPlan {
    population_or_process
    sampling_method
    sample_size_plan
    independence_assumptions
    randomization_plan?
    blocking_plan?
    pairing_plan?
}
```

### 27.2 Measurement hierarchy

Systems experiments may vary at several nested levels:

```text
iteration
process execution
VM/container/deployment
build
machine
cluster
```

`100` inner-loop samples are not equivalent to `100` independent builds or machines.

### 27.3 Statistical hypothesis

```text
StatisticalHypothesisExperiment {
    null_hypothesis
    alternative_hypothesis
    statistic
    sampling_model
    alpha
    beta_or_power?
    effect_threshold?
}
```

Possible conclusions:

```text
REJECT_NULL
FAIL_TO_REJECT_NULL
INCONCLUSIVE
ASSUMPTIONS_VIOLATED
```

`FAIL_TO_REJECT_NULL` must not be renamed `NULL_TRUE`.

### 27.4 Effect size

```text
EffectEstimate {
    quantity
    uncertainty
    practical_threshold?
}
```

Statistical detectability and practical significance remain distinct.

### 27.5 Confidence intervals

A confidence interval must name:

- parameter/estimand;
- procedure;
- confidence level;
- sampling assumptions;
- interpretation family.

MNCS should prevent reinterpreting one statistical interval as a different inferential object.

### 27.6 Multiple comparisons

```text
ComparisonFamily {
    hypotheses[]
    family_scope
    correction_policy?
}
```

No implicit reuse of single-comparison error rates across large search families.

---

## 28. Sequential experimentation

Sequential methods may inspect evidence after each observation and stop according to a declared rule.

```text
SequentialExperiment {
    hypothesis_or_estimand
    observation_process
    decision_boundaries
    type_I_error_bound?
    type_II_error_bound?
    indifference_region?
    max_budget?
}
```

Stopping because an experiment's statistical criterion is satisfied is distinct from RFC 0033 deciding that the overall realization-selection problem has enough information.

---

## 29. Statistical model checking

```text
StatisticalPropertyExperiment {
    stochastic_subject_or_model
    temporal_property
    sampling_policy
    inference_method
    error_parameters
}
```

Statistical model checking produces probabilistic/statistical evidence. It must remain distinct from exhaustive model checking or proof.

---

## 30. Benchmark semantics

### 30.1 `BenchmarkExperiment`

```text
BenchmarkExperiment {
    subject
    workload
    target
    environment
    measurement_metric
    instrumentation
    warmup_policy
    execution_protocol
    repetition_plan
    statistical_analysis
}
```

RFC 0032 owns the metric itself. RFC 0034 owns how the empirical measurement was produced.

### 30.2 Warmup/state classification

Potential classes:

```text
COLD
WARMING
STEADY_STATE
PHASED
UNKNOWN
```

JIT compilation, lazy initialization, cache state, GC, model loading, and adaptive runtime behavior may all make phase identity relevant.

### 30.3 Benchmark comparison

```text
BenchmarkComparison {
    baseline
    candidate
    metric
    workload
    pairing/randomization
    effect_estimate
    uncertainty
    practical_threshold?
}
```

Possible conclusions include:

```text
BASELINE_BETTER
CANDIDATE_BETTER
NO_DETECTABLE_DIFFERENCE
PRACTICALLY_EQUIVALENT
INCONCLUSIVE
```

`NO_DETECTABLE_DIFFERENCE` is not proof of equality.

### 30.4 Performance regression

```text
PerformanceRegressionClaim {
    baseline
    candidate
    metric
    workload
    allowed_tolerance
    statistical_model
}
```

Regression evidence is scoped to benchmark identity and applicability.

---

## 31. Empirical applicability

```text
EmpiricalApplicability {
    subject_version
    relation_or_claim
    input_domain
    generator_or_corpus
    environment
    target
    workload?
    schedule_model?
    failure_model?
    observation_model
    oracle
    instrumentation
    time_validity?
}
```

Changing a dependency may invalidate only the relevant empirical claims.

Examples:

- documentation-only change need not invalidate performance evidence;
- allocator change may invalidate memory/latency evidence;
- target microarchitecture change may stale performance or side-channel observations while leaving semantic proof intact;
- oracle update may invalidate result interpretation without invalidating raw observations.

---

## 32. Evidence transport

Empirical evidence does not automatically transfer from one subject to another.

```text
EvidenceTransportClaim {
    source_subject
    target_subject
    relation
    evidence_class
    observer_scope
}
```

Example: if two realizations are proven equivalent under a value-only observer, value-regression evidence may be reusable while timing evidence is not.

---

## 33. Experiment provenance

RFC 0031 lineage applies to:

- test harness;
- generator;
- corpus;
- oracle;
- instrumentation;
- benchmark harness;
- statistical analysis code;
- subject artifact.

```text
ExperimentManifest {
    experiment
    subject
    harness
    generator_or_corpus
    oracle
    environment
    target
    seed/schedule/fault metadata
    budget
    observations
    coverage
    analysis
    provenance_refs
}
```

Canonical representation should be deterministic under RFC 0027 where applicable.

---

## 34. Evidence diversity and common-mode dependencies

A campaign may combine:

- handwritten examples;
- property generation;
- differential testing;
- metamorphic testing;
- fuzzing;
- symbolic execution;
- mutation testing;
- combinatorial testing;
- concurrency/fault exploration.

Different methods can increase diversity of failure detection, but independence must not be assumed from tool names alone.

Shared dependencies might include:

- same parser;
- same reference evaluator;
- same test-data generator;
- same compiler;
- same environment;
- same specification bug.

RFC 0018 evaluates evidence independence.

---

## 35. Empirical claim dispositions

RFC 0034 should emit method-scoped empirical artifacts, not overwrite RFC 0018 disposition semantics.

Useful run/campaign outcomes include:

```text
COUNTEREXAMPLE_FOUND
NO_COUNTEREXAMPLE_OBSERVED
BOUNDED_DOMAIN_EXHAUSTED
COVERAGE_TARGET_MET
COVERAGE_TARGET_UNMET
BUDGET_EXHAUSTED
ORACLE_CONFLICT
FLAKY
INCONCLUSIVE
INVALID_EXPERIMENT
```

These are experiment results. RFC 0018 maps their evidence into claim-level support/refutation/conflict and action authorization.

---

## 36. Micro-verifier opportunities

RFC 0034 should expose narrow deterministic verifiers wherever possible.

Candidate micro-verifiers:

### 36.1 Experiment identity verifier

Checks that a result references the exact subject, generator/corpus, environment, target, oracle, and observation model claimed.

### 36.2 Counterexample verifier

Replays or directly evaluates a concrete counterexample against the declared RFC 0020 relation where possible.

### 36.3 Coverage arithmetic verifier

Checks feature denominator, covered/uncovered sets, aggregation, and ratio.

### 36.4 Combinatorial coverage verifier

Checks that a test set covers all modeled valid t-way tuples.

### 36.5 Mutation accounting verifier

Checks mutant classifications and score arithmetic without deciding semantic equivalence itself.

### 36.6 Replay-manifest verifier

Checks replay bundle completeness and subject/harness identity.

### 36.7 Statistical-result verifier

Checks arithmetic/model contracts for declared statistical procedures where feasible; it does not prove assumptions such as independence unless separately evidenced.

### 36.8 Differential relation verifier

Checks a supplied concrete mismatch under the declared observer/relation.

### 36.9 Schedule/fault-domain verifier

Checks bounded schedule or fault coverage over the explicitly modeled finite domain.

The expensive generation/search side remains outside the TCB.

---

## 37. Forge experiment search

Forge should treat experiment generation as a search problem whose candidates are **information-producing actions**.

Potential generators/searchers:

- LLM-generated tests;
- property generators;
- grammar fuzzers;
- coverage-guided fuzzers;
- evolutionary fuzzers;
- symbolic/concolic engines;
- combinatorial test generators;
- schedule explorers;
- fault injectors;
- statistical experiment planners;
- benchmark planners.

Forge may optimize for:

- new semantic coverage;
- new schedule/fault classes;
- counterexample likelihood;
- mutation-kill novelty;
- uncertainty reduction;
- reproduction value;
- coverage of unresolved obligations.

But RFC 0034 does not decide whether another experiment is worth its cost; RFC 0033 value-of-information/selection policy owns that decision.

---

## 38. Forge study A — compiler transformation from multiple epistemic angles

Subject:

```text
Baseline transformation A
Candidate transformation B
Required relation R
```

Campaign:

1. property-based generation;
2. boundary-biased generation;
3. semantic coverage-guided fuzzing;
4. body/reference vs HIR/SSA differential execution;
5. symbolic/concolic path generation;
6. controlled mutation testing of the test suite.

Output is an `EmpiricalEvidenceGraph`, not `six tools passed`.

The graph must show:

- sampled input regions;
- coverage criteria;
- oracle dependencies;
- unresolved regions;
- counterexamples;
- method overlap/common dependencies;
- applicability envelope.

---

## 39. Forge study B — discover, reduce, and preserve a lowering bug

Fabric generates bounded valid MNCS programs and compares:

```text
semantic/body evaluator
vs
SSA evaluator
```

On mismatch:

1. create relation-scoped counterexample;
2. cluster/fingerprint the failure;
3. shrink input;
4. delta-debug structure/operations;
5. derive RFC 0004 causal slice separately;
6. create regression artifact;
7. rerun targeted campaign after repair.

Desired final structure:

```text
OriginalProgram: large
ReducedCounterexample: small
ViolatedRelation: BodySSAExecutionAgreement
RegressionArtifact: persistent
```

---

## 40. Forge study C — empirical adequacy of test strategies

Compare:

```text
A = handwritten examples
B = coverage-guided fuzzing
C = property + combinatorial + fuzzing
```

against a controlled mutation/fault model.

RFC 0034 reports:

- structural coverage;
- semantic coverage;
- t-way coverage;
- mutation classes killed/survived/equivalent/unresolved;
- counterexample quality/reduction;
- empirical cost measurements via RFC 0032.

RFC 0033 later decides which strategy is preferred for a particular assurance/cost policy.

---

## 41. Forge study D — distributed schedule/fault experimentation

Use a bounded RFC 0028 service.

Generate:

- operations;
- schedules;
- delays;
- drops;
- duplication;
- partitions;
- crashes;
- recoveries.

Check resulting histories separately against:

```text
Linearizable
Causal
EventuallyConvergent
```

A history may pass one relation and fail another without contradiction.

Bounded schedule/fault completeness remains scoped to the finite modeled domain.

---

## 42. Forge study E — benchmark methodology can change the conclusion

Compare two semantically valid realizations.

Naive benchmark initially claims:

```text
A is 15% faster
```

A better randomized/multi-level experiment may reveal:

- the effect is uncertain;
- the difference is below practical significance;
- result depends on cold/warm state;
- build-to-build variance dominates;
- machine variation reverses ordering.

The study must show that confident-looking numbers can be invalid empirical claims when methodology/scope is wrong.

---

## 43. Interaction with RFC 0032 and RFC 0033

The intended chain is:

```text
RFC 0032:
    define metric and quantitative claim

RFC 0034:
    design and execute experiment
    produce scoped observations/evidence

RFC 0018:
    appraise empirical evidence

RFC 0033:
    decide whether more information is valuable
    and/or select among admissible realizations
```

Example:

```text
RFC0032 metric:
    P99 latency

RFC0034 benchmark:
    estimate P99 under workload W/target T

RFC0018:
    evidence adequate for experimental use

RFC0033:
    candidate choice remains unresolved;
    another benchmark has positive value of information
```

---

## 44. Lowering and preservation obligations

RFC 0034 semantic artifacts do not all survive into executable code, but lowering must preserve references where empirical meaning depends on them.

Examples:

- semantic coverage features map to HIR/SSA features;
- relation identities remain traceable through differential execution;
- instrumentation points record their semantic origin;
- test-case subject identity remains attached to target artifact identity;
- regression artifacts preserve violated obligation/relation identities;
- generated benchmark instrumentation cannot silently change the metric/observer contract.

Compiler transformations may change backend coverage shape without changing the higher-level semantic coverage denominator.

---

## 45. Security and authority

Experiments can be dangerous.

A generator may propose:

- destructive I/O;
- malformed foreign input;
- resource exhaustion;
- network faults;
- credential misuse;
- data disclosure;
- distributed partitions;
- deployment/lifecycle changes.

Experiment authority must therefore be separate from generation authority.

```text
GenerateExperimentCapability
ExecuteSandboxExperimentCapability
InjectFaultCapability
ObserveProtectedMetricCapability
ExecuteProductionExperimentCapability
```

must remain distinguishable under RFC 0008/RFC 0025.

A test generator may be allowed to propose an experiment that it is not allowed to execute.

---

## 46. Information-flow considerations

Instrumentation and experimental outputs can create RFC 0024 channels.

Examples:

- timing traces;
- cache/profiler data;
- private test inputs;
- production workloads;
- credentials in crash dumps;
- internal topology;
- proprietary benchmark data.

Coverage and replay artifacts must obey information-flow policy. A richer observer is not automatically authorized simply because it would improve testing.

---

## 47. Failure semantics

Experiment harnesses can fail independently of subjects.

Potential outcomes include:

```text
SUBJECT_FAILURE
HARNESS_FAILURE
ORACLE_FAILURE
INSTRUMENTATION_FAILURE
ENVIRONMENT_FAILURE
BUDGET_EXHAUSTED
UNKNOWN_REMOTE_OUTCOME
```

These must not collapse into one test `FAIL`.

Remote/distributed experiments reuse RFC 0028 outcome-uncertainty semantics.

---

## 48. Quantitative experiment cost

RFC 0032 may quantify:

- CPU/GPU time;
- wall time;
- model calls/tokens;
- generated cases;
- executions;
- solver calls;
- verifier calls;
- network bytes;
- storage;
- energy;
- reproduction cost.

RFC 0034 records consumption against experiment/campaign budgets.

Search budget exhaustion is an experiment outcome, not correctness evidence.

---

## 49. Theory synthesis

RFC 0034 selectively adopts ideas from the following families.

### 49.1 Property-based testing

QuickCheck-style quantified properties, generated samples, classification, and shrinking motivate explicit separation of universal property identity from sampled support.

### 49.2 Oracle theory

The testing-oracle literature motivates treating expected-behavior judgment as an independent problem rather than assuming every test has an infallible expected output.

### 49.3 Differential testing

Compiler differential testing and Csmith-style randomized testing motivate relation-scoped inconsistency detection without assuming disagreement identifies the guilty implementation.

### 49.4 Metamorphic testing

Metamorphic relations provide relational oracles when exact expected outputs are unavailable.

### 49.5 Mutation testing

Mutation testing provides fault-model-relative evidence about test-suite sensitivity, while equivalent mutants motivate explicit relation checking.

### 49.6 Coverage-guided fuzzing

libFuzzer/AFL-like corpus mutation and coverage feedback motivate explicit fuzz search state, novelty models, corpus provenance, and budget-scoped exploration.

### 49.7 Greybox fuzzing/search scheduling

AFLFast-style power scheduling and other search heuristics motivate treating fuzz energy/seed scheduling as realization strategy rather than evidence semantics.

### 49.8 Symbolic execution

KLEE-like path-condition exploration motivates solver-backed generation with explicit path scope and `UNKNOWN` preservation.

### 49.9 Concolic execution

DART-like concrete/symbolic alternation motivates recursive generation from observed execution paths.

### 49.10 Delta debugging

Delta debugging motivates automated counterexample reduction with explicit minimality scope.

### 49.11 Combinatorial interaction testing

Covering arrays and NIST ACTS-style t-way testing motivate explicit modeled input-interaction coverage.

### 49.12 Structural coverage / MC/DC

MC/DC motivates treating coverage criteria as semantically distinct and refusing to infer correctness from structural coverage.

### 49.13 Model-based testing

State/transition models motivate trace generation and finite conformance experiments.

### 49.14 Systematic concurrency testing

CHESS-like systematic scheduling motivates deterministic bounded interleaving exploration and replay.

### 49.15 Dynamic partial-order reduction

DPOR motivates independence-aware schedule equivalence classes and bounded schedule completeness.

### 49.16 Concurrent-history checking

Linearizability/consistency history checkers motivate relation-scoped empirical oracles over finite histories.

### 49.17 Fault injection and resilience experimentation

Fault campaigns motivate explicit failure-trace models and fault-coverage criteria while preserving bounded-evidence semantics.

### 49.18 Design of experiments

Randomization, blocking, pairing, replication, and sample-size planning motivate first-class experiment-design artifacts.

### 49.19 Rigorous systems benchmarking

Multi-level repetition and warmup/steady-state methodology motivate explicit measurement hierarchy and benchmark phase identity.

### 49.20 Sequential testing

Sequential probability testing motivates bounded error-controlled stopping policies that remain distinct from global decision stopping.

### 49.21 Statistical model checking

Sampled stochastic property checking motivates empirical probabilistic evidence without being confused with exhaustive model checking.

### 49.22 Proof-/evidence-producing experiment checking

MNCS extends these theories by keeping arbitrary search/generation outside the TCB while narrow checkers validate concrete counterexamples, coverage arithmetic, experiment manifests, replay identities, and inference contracts.

---

## 50. Candidate semantic planes

RFC 0034 uses ten conceptual planes.

### Experiment plane

```text
Experiment
ExperimentRun
ExperimentCampaign
ExperimentalUnit
```

### Stimulus plane

```text
TestCase
InputModel
TestGenerator
TestCorpus
Mutation
FaultScenario
Schedule
```

### Oracle plane

```text
Oracle
OracleEvaluation
DifferentialRelation
MetamorphicRelation
```

### Observation plane

```text
Observation
ObservationModel
Instrumentation
ReplayTrace
```

### Coverage plane

```text
CoverageModel
CoverageFeature
CoverageReport
AdequacyClaim
```

### Search plane

```text
FuzzCampaign
FuzzSearchState
SymbolicTestGeneration
ConcolicGeneration
ScheduleSearch
```

### Counterexample plane

```text
Counterexample
FailureFingerprint
CounterexampleReduction
RegressionArtifact
```

### Statistical plane

```text
SamplingPlan
RepetitionPlan
StatisticalHypothesis
EffectEstimate
ConfidenceInterval
SequentialExperiment
```

### Benchmark plane

```text
BenchmarkExperiment
MeasurementHierarchy
WarmupPolicy
BenchmarkComparison
```

### Assurance plane

```text
EmpiricalClaim
EmpiricalApplicability
ExperimentWitness
ExperimentCounterexample
ExperimentDelta
```

---

## 51. Candidate first-class artifacts

The canonical schema should reserve at least:

```text
Experiment
ExperimentRun
ExperimentCampaign

TestCase
InputModel
TestGenerator
TestCorpus
CorpusEntry

Oracle
OracleEvaluation

PropertyExperiment
DifferentialExperiment
MetamorphicExperiment
MutationModel
MutationExperiment
MutationAdequacy

CoverageModel
CoverageReport
SemanticCoverageModel
CombinatorialModel

FuzzCampaign
FuzzSearchState
SymbolicTestGeneration
ConcolicTestGeneration
ModelConformanceExperiment
ScheduleExperiment
FaultExperiment

Observation
ObservationModel
ReplayBundle

Counterexample
CounterexampleReduction
FailureFingerprint
RegressionArtifact

SamplingPlan
RepetitionPlan
StatisticalHypothesisExperiment
EffectEstimate
ConfidenceIntervalClaim
SequentialExperiment
StatisticalPropertyExperiment

BenchmarkExperiment
BenchmarkComparison
PerformanceRegressionClaim

EmpiricalClaim
EmpiricalApplicability
EvidenceTransportClaim
ExperimentManifest

ExperimentWitness
ExperimentCounterexample
ExperimentDelta
```

---

## 52. Diagnostics

Diagnostics should distinguish causes precisely.

Examples:

```text
E3401 UNIVERSAL_CLAIM_FROM_FINITE_SAMPLE
E3402 ORACLE_OUT_OF_SCOPE
E3403 ORACLE_CONFLICT
E3404 COVERAGE_MODEL_MISMATCH
E3405 COVERAGE_DENOMINATOR_UNKNOWN
E3406 GENERATOR_DOMAIN_MISMATCH
E3407 SHRINK_MINIMALITY_OVERCLAIM
E3408 DIFFERENTIAL_GUILT_UNRESOLVED
E3409 MUTANT_EQUIVALENCE_UNRESOLVED
E3410 SYMBOLIC_REACHABILITY_UNKNOWN
E3411 SCHEDULE_COMPLETENESS_OVERCLAIM
E3412 FAULT_COVERAGE_SCOPE_MISMATCH
E3413 INSTRUMENTATION_SUBJECT_MISMATCH
E3414 FLAKY_RESULT_HIDDEN_BY_RETRY
E3415 REPLAY_ENVIRONMENT_MISMATCH
E3416 STATISTICAL_ASSUMPTION_UNSUPPORTED
E3417 FAIL_TO_REJECT_MISREAD_AS_TRUE
E3418 PRACTICAL_SIGNIFICANCE_UNSPECIFIED
E3419 BENCHMARK_WORKLOAD_MISSING
E3420 EMPIRICAL_APPLICABILITY_STALE
E3421 HARNESS_PROVENANCE_MISSING
E3422 EXPERIMENT_BUDGET_EXHAUSTED
E3423 UNAUTHORIZED_EXPERIMENT_EFFECT
```

Diagnostics should include explanation slices and dependency references where possible.

---

## 53. Bounded first implementation pilot

The first implementation should remain intentionally small while demonstrating the core epistemic distinctions.

Implement:

```text
Experiment
ExperimentRun
TestCase
TestGenerator
Oracle
OracleEvaluation
CoverageModel
CoverageReport
PropertyExperiment
DifferentialExperiment
FuzzCampaign
Counterexample
CounterexampleReduction
ReplayBundle
BenchmarkExperiment
EmpiricalApplicability
ExperimentWitness
ExperimentCounterexample
ExperimentDelta
```

Optional early additions:

```text
MutationExperiment
CombinatorialCoverage
ScheduleExperiment
SamplingPlan
```

---

## 54. Required pilot fixtures

### Fixture 1 — finite passing sample != proof

Property:

```text
forall x:u8. f(x) = x
```

Run samples `0..99`, all pass.

Required result:

```text
bounded empirical support
```

Forbidden result:

```text
universal property proved
```

Then execute `x=255` where the property fails. The concrete valid counterexample refutes the universal claim.

### Fixture 2 — generator distribution identity

Two 1,000-run campaigns use different distributions. They must not acquire identical empirical identities merely because run count matches.

### Fixture 3 — oracle defect

Correct subject + intentionally buggy reference oracle yields disagreement. Required result identifies disagreement/oracle uncertainty rather than automatically refuting the subject.

### Fixture 4 — metamorphic oracle

Use an exact-output-unavailable case with a separately justified metamorphic relation. Violation produces a relation-scoped counterexample.

### Fixture 5 — coverage != correctness

Achieve 100% statement/branch coverage while retaining a value-dependent bug discovered by another method.

### Fixture 6 — branch coverage != MC/DC

Construct a compound Boolean decision where branches are both exercised but independent condition influence is incomplete.

### Fixture 7 — t-way coverage

Generate a pairwise covering set over a finite parameter model. Establish complete 2-way modeled coverage while explicitly leaving 3-way coverage incomplete.

### Fixture 8 — mutation classification

Include killed, survived, equivalent, and unresolved mutants. Equivalent mutants must not be treated as missed defects.

### Fixture 9 — coverage-guided fuzzing

Retain inputs for new semantic coverage features, preserve mutation/seed lineage, and find at least one concrete failure.

### Fixture 10 — shrinking

Reduce a large failing input and label the result only as minimal under the declared shrink relation/search.

### Fixture 11 — delta reduction

Reduce a failing event sequence while preserving failure and minimality scope.

### Fixture 12 — differential relation refutation

Two expected-equivalent realizations disagree. Refute their relation while leaving faulty-party identity unresolved absent stronger oracle evidence.

### Fixture 13 — schedule-sensitive failure

Same input under schedule S1 passes and schedule S2 fails. Preserve input identity and schedule identity separately.

### Fixture 14 — bounded DPOR completeness

Cover all dependency-distinct schedules for an explicitly finite domain and refuse universal thread-safety language.

### Fixture 15 — fault-model scope

One distributed candidate survives a single crash but fails partition+crash. Fault coverage remains explicit.

### Fixture 16 — flakiness

Repeated controlled requests produce PASS/PASS/FAIL/PASS/FAIL. Preserve all observations and return variability/unresolved dependence rather than majority pass.

### Fixture 17 — benchmark hierarchy

Measure across iterations, processes, builds, and machines and show that variance attribution changes with repetition level.

### Fixture 18 — statistical vs practical significance

Produce a statistically detectable but operationally negligible effect. RFC 0034 reports both; RFC 0033 policy decides relevance.

### Fixture 19 — insufficient power/inconclusive result

A too-small experiment returns `INCONCLUSIVE`, not `NO_DIFFERENCE`.

### Fixture 20 — sequential experiment

Stop according to explicit sequential decision boundaries and preserve error assumptions/parameters.

### Fixture 21 — stale empirical evidence

Change target/environment identity so performance evidence becomes stale while unrelated semantic evidence remains valid.

### Fixture 22 — instrumentation perturbation

Demonstrate an execution whose behavior differs under instrumentation and refuse automatic transfer to uninstrumented production behavior.

---

## 55. 1.0 research threshold additions

RFC 0034 is considered represented at the 1.0 research threshold only if MNCS demonstrates all of the following:

- a versioned experiment/run/campaign identity model;
- explicit separation of test case, execution, observation, oracle evaluation, and empirical claim;
- explicit empirical applicability envelopes;
- machine-enforced refusal to upgrade finite passing samples into universal proof;
- at least one concrete counterexample that refutes a universal claim while preserving domain/oracle validity;
- a versioned generator/input-model schema with distribution/seed/size/boundary identity;
- at least one property-based campaign with shrinking and correctly scoped minimality language;
- a versioned oracle model with applicability/dependency/strength distinctions;
- at least one oracle-conflict or faulty-oracle fixture where subject guilt remains unresolved;
- at least one differential experiment where relation disagreement is established without fabricating faulty-party identity;
- at least one metamorphic experiment whose relation carries separate semantic justification;
- a versioned mutation model with killed/survived/equivalent/invalid/unresolved statuses;
- at least one mutation-adequacy study whose result remains fault-model-relative;
- a versioned coverage framework that distinguishes at minimum statement/branch, combinatorial input, semantic, schedule/fault, and mutation coverage families;
- at least one `100%` coverage study demonstrating that coverage under one model does not establish correctness or another coverage model;
- at least one combinatorial interaction campaign with mechanically checked t-way coverage;
- a versioned fuzz-campaign/search-state model with explicit feedback/novelty and budget semantics;
- at least one semantic-coverage-guided fuzzing experiment;
- at least one fuzz budget exhaustion result that remains bounded non-discovery evidence rather than bug absence;
- symbolic or concolic generation with `SAT`/`UNSAT`/`UNKNOWN`-style path results that do not turn timeout into unreachability;
- at least one replay bundle that deterministically reproduces a discovered failure under controlled conditions;
- at least one independent rediscovery case kept distinct from replay;
- at least one failure-clustering/fingerprinting study where raw failure count differs from distinct counterexample classes;
- at least one counterexample-reduction pipeline producing a persistent regression artifact;
- a versioned schedule-experiment model and one bounded schedule-completeness result with explicit operation/thread/dependency bounds;
- at least one finite concurrent-history relation check where the same subject can pass one relation and fail another;
- a versioned fault-experiment model with explicit fault-scenario and fault-coverage identity;
- at least one bounded distributed fault campaign whose evidence remains bounded rather than becoming universal fault-tolerance proof;
- explicit instrumentation identity and one case where instrumented and uninstrumented behavior differ;
- explicit flakiness/outcome-variability semantics preserving failed attempts despite retry policy;
- a versioned benchmark experiment model tied to RFC 0032 metric/workload/target identities;
- explicit cold/warm/steady-state or equivalent phase semantics;
- a multi-level repetition/measurement-hierarchy study showing inner-loop repetition is not independent environment replication;
- at least one statistical experiment preserving `FAIL_TO_REJECT` versus `TRUE` distinction;
- at least one effect-size/practical-significance study distinct from statistical significance;
- at least one inconclusive/insufficient-information statistical result that does not fabricate equality;
- at least one sequential experiment with explicit stopping/error parameters;
- at least one stale empirical result invalidated narrowly after target/environment/harness/oracle dependency change;
- experiment-harness/oracle/generator provenance linked through RFC 0031;
- at least one empirical evidence set appraised by RFC 0018 without being upgraded to stronger evidence than its method permits;
- at least one RFC 0033 decision where the correct action is to request an additional RFC 0034 experiment rather than immediately select;
- untrusted experiment generation followed by narrower deterministic checking of subject identity, oracle applicability, counterexample validity, coverage arithmetic, replay identity, or statistical-result contracts;
- a Forge capstone combining multiple empirical strategies into one evidence graph without collapsing them into a scalar `tests_passed` result.

---

## 56. Non-goals

RFC 0034 does not attempt to:

- define one universal testing framework;
- define one universal oracle;
- require random testing;
- require fuzzing;
- require statistical inference for deterministic properties;
- turn coverage into assurance level;
- treat every test failure as a subject defect;
- prove universal correctness from empirical execution;
- replace the RFC 0007 proof core;
- define all experiment-authority policy;
- select the best experiment strategy — RFC 0033 owns selection/value-of-information;
- define metric semantics — RFC 0032;
- define provenance formats — RFC 0031.

---

## 57. Open questions

1. Which coverage families should enter the first canonical schema versus remain foreign extensions?
2. Should `SemanticCoverageFeature` identifiers be generated directly from semantic graph node kinds, obligations, or a dedicated coverage ontology?
3. How should coverage denominators behave when semantic lowering creates or removes implementation-only structure?
4. Which finite-domain exhaustive campaigns are strong enough to produce proof-like certificates for the finite domain, and should those certificates be RFC 0007-compatible?
5. How should oracle dependence/common-mode risk be summarized without inventing a scalar oracle confidence?
6. What minimum provenance should every experiment manifest require?
7. Which statistical procedures are simple enough for independent micro-verifier implementations in 1.0?
8. Should experiment plans themselves be typed programs in a later MNCS surface syntax?
9. How should Fabric coordinate distributed experiments while preserving independent-run identity and common-dependency structure?
10. How should MNEL training/evaluation corpus lineage interact with experiment/campaign identity?

---

## 58. References and theoretical influences

The RFC should track at least the following literature/systems as theory and realization references:

- Claessen and Hughes, **QuickCheck: A Lightweight Tool for Random Testing of Haskell Programs**.
- Barr et al., **The Oracle Problem in Software Testing: A Survey**.
- Yang et al., **Finding and Understanding Bugs in C Compilers (Csmith)**.
- Chen et al., foundational **Metamorphic Testing** work and subsequent surveys.
- Jia and Harman, **An Analysis and Survey of the Development of Mutation Testing**.
- LLVM **libFuzzer** documentation and coverage-guided fuzzing practice.
- Böhme et al., **Coverage-based Greybox Fuzzing as Markov Chain / AFLFast**.
- Cadar et al., **KLEE: Unassisted and Automatic Generation of High-Coverage Tests for Complex Systems Programs**.
- Godefroid et al., **DART: Directed Automated Random Testing**.
- Zeller and Hildebrandt, **Simplifying and Isolating Failure-Inducing Input (Delta Debugging)**.
- NIST **Automated Combinatorial Testing for Software / ACTS**.
- Hayhurst et al., **A Practical Tutorial on Modified Condition/Decision Coverage**.
- Musuvathi et al., **CHESS: systematic testing of concurrent software**.
- Flanagan and Godefroid, **Dynamic Partial-Order Reduction for Model Checking Software**.
- concurrent-history / linearizability checking literature.
- fault-injection and resilience/chaos-experiment literature.
- NIST/SEMATECH **Engineering Statistics Handbook**.
- Kalibera and Jones, **Rigorous Benchmarking in Reasonable Time** and related benchmarking methodology.
- Wald-style sequential testing / SPRT theory.
- statistical model checking literature.

These references guide semantics. MNCS does not require any one tool or algorithm.

---

## 59. Defining principle

The defining sentence for RFC 0034 is:

> **Testing does not certify what a program is; it records what explicitly constructed experiments were able to observe about it, under named generators, environments, oracles, coverage models, and inference rules.**

The machine-native formulation is:

> **MNCS treats experimentation as evidence-producing computation: machines may generate tests, fuzz inputs, schedules, failures, benchmarks, and hypotheses aggressively, but every resulting claim retains the exact experimental scope that made the observation meaningful.**
