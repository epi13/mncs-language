# RFC 0033: Machine-Native Realization Search, Multi-Objective Optimization, Pareto, Risk, Preference, and Selection-Policy Semantics

- **Status:** Draft
- **Authors:** MNCS project
- **Target:** pre-1.0 research roadmap

## Summary

MNCS needs a semantic model for choosing among multiple realizations that are already admissible under the earlier semantic, assurance, target, lifecycle, derivation, and quantitative contracts.

RFC 0032 deliberately refuses to collapse latency, memory, energy, evidence cost, bandwidth, reliability, monetary valuation, and other quantitative dimensions into one generic `cost`. RFC 0033 completes that design by defining how explicit decision policy may reduce an admissible, multidimensional candidate space to one candidate, a candidate set, a conditional policy, a portfolio, a request for more information, or a justified unresolved result.

The primary constitutional rule is:

> **Feasibility is not preference. Pareto optimality is not selection. Measurement is not utility. A candidate becomes “best” only relative to an explicit, authorized decision policy.**

The second rule is:

> **Preference is semantic input. Search, optimization, learning, and heuristics may discover candidates and tradeoffs, but they may not invent weights, priorities, aspiration levels, risk appetite, constraint relaxation, or preference authority.**

The third rule is:

> **Uncertainty about a candidate is not dislike of the candidate. Missing or noisy objective evidence must remain epistemic uncertainty until an explicit decision model says how uncertainty affects selection.**

The fourth rule is:

> **The cost of learning which candidate is better is itself part of the decision problem. A machine-native selector may rationally request more evidence, explore safely, stop when satisfactory, or return `UNRESOLVED` rather than fabricate a globally optimal winner.**

The machine-native objective is not to standardize one optimizer, evolutionary algorithm, scalar score, Bayesian model, risk measure, or policy language. Weighted sums, epsilon-constraint methods, lexicographic optimization, reference-point and goal programming, satisficing, robust optimization, distributionally robust optimization, minimax regret, CVaR, evolutionary multi-objective search, exact mathematical programming, surrogate optimization, bandit methods, safe exploration, value-of-information analysis, and future mechanisms are realization and decision-policy families over a smaller semantic model.

MNCS programs, profiles, operators, higher-level agents, and Forge should be able to state which candidates may compete, which objectives matter, which preferences and priorities are authoritative, what uncertainty/risk model applies, what information may be gathered, how much search is permitted, and what kind of conclusion is required. Search mechanisms may then explore that decision problem only inside those declared boundaries.

---

## 1. Scope

RFC 0033 owns the semantics of:

- decision-problem and decision-instance identity;
- candidate sets after earlier semantic and quantitative admissibility checks;
- hard selection constraints versus soft constraints and preferences;
- objective identity, direction, transformation, normalization, and objective vectors;
- dominance relations and dominance evidence;
- Pareto candidate sets and Pareto objective fronts;
- exact versus approximate Pareto claims;
- preference profiles and preference provenance;
- strict priority and lexicographic policies;
- weighted utility/scalarization as one explicit policy family;
- epsilon-constraint policies;
- aspiration, reservation, goal, and reference-point policies;
- satisficing and bounded-rationality stopping policies;
- incomplete/partial preference and incomparability;
- tie handling and deterministic/randomized tie-breaking policy;
- uncertainty models used by decision policy;
- risk measures and risk-sensitive selection;
- robust, stochastic, distributionally robust, and regret-based selection families;
- sensitivity and robustness of selection to evidence/policy changes;
- search-state, search-coverage, incumbent, Pareto approximation, and optimality-gap artifacts;
- exact, bounded, heuristic, stochastic, learned, and agentic search strategies as realization families;
- safe exploration, baseline-safe sets, and experimentation authority;
- surrogate prediction as distinct from measured or proved objective evidence;
- value-of-information and experiment-selection claims;
- stopping rules and search-budget outcomes;
- static, conditional, adaptive, randomized, and portfolio selection results;
- switching/hysteresis policy when selection changes at runtime;
- selection witnesses, counterexamples, explanations, counterfactuals, and deltas;
- Forge realization-selection search and narrow independent decision checking.

This RFC does **not** own:

- generic semantic correctness of candidates: RFCs 0001-0031 as applicable;
- generic evidence strength, trust, independence, freshness, or action authorization: RFC 0018;
- generic equivalence/refinement/compatibility relation semantics: RFC 0020;
- numerical correctness of objective calculations: RFC 0021;
- search termination, bounded execution, or budget-exhaustion meaning: RFC 0022;
- temporal semantics: RFC 0023;
- information-flow/security requirements that determine whether a candidate is admissible: RFC 0024;
- principal identity and authority to define/modify preference or relax constraints: RFC 0025;
- persistence/distribution/placement/lifecycle/build correctness of candidate realizations: RFCs 0026-0031;
- metric, unit, bound, measurement, workload, resource, QoS, energy, or quantitative-feasibility semantics: RFC 0032.

The critical boundary is:

> **RFC 0032 determines quantitative facts and hard quantitative admissibility. RFC 0033 determines preference-relative selection among the remaining admissible candidates.**

---

## 2. Constitutional distinctions

The following distinctions are normative design rules.

1. **Feasibility is not preference.**
2. **A requirement is not an objective.**
3. **A hard constraint may not silently become a penalty.**
4. **A soft constraint is not a hard constraint with a smaller number.**
5. **Pareto optimality is not selection.**
6. **Pareto membership does not establish equal desirability.**
7. **Candidate identity is not objective-vector identity.**
8. **Metric magnitude is not utility.**
9. **Metric units do not establish commensurability.**
10. **Normalization is part of decision policy.**
11. **Weights encode tradeoffs; they are not objective facts.**
12. **Lexicographic priority is not equivalent to a sufficiently large finite weight.**
13. **Aspiration level is not reservation level.**
14. **Reference point is not a hard requirement unless declared as one.**
15. **Satisficing is not failed optimization.**
16. **Best-known is not globally optimal.**
17. **Search-budget exhaustion is not optimality evidence.**
18. **Approximate Pareto front is not exact Pareto front.**
19. **Search diversity is not decision preference.**
20. **Hypervolume/front-quality is not universal utility.**
21. **Preference may be partial, incomplete, conditional, or context-sensitive.**
22. **Incomparability is a valid result.**
23. **Tie and incomparability are distinct.**
24. **Tie-breaking must not depend on incidental iteration/hash/order behavior.**
25. **Preference source and preference authority are explicit.**
26. **Search authority does not imply preference-edit authority.**
27. **Preference-edit authority does not imply hard-constraint-relaxation authority.**
28. **Changing decision policy changes decision-instance identity even when candidate facts do not change.**
29. **Probability is not preference.**
30. **Uncertainty is not risk appetite.**
31. **Expected value is not worst-case value.**
32. **VaR is not CVaR.**
33. **Robust optimization is not stochastic optimization.**
34. **Distributional uncertainty is not ordinary outcome randomness.**
35. **Missing objective evidence is not zero, average, or favorable cost.**
36. **Regret is not loss.**
37. **Surrogate prediction is not measurement.**
38. **Measurement is not proof.**
39. **Exploration is not deployment authorization.**
40. **Safe-exploration constraints remain hard during search.**
41. **Search cost may be part of the decision problem.**
42. **More information is not automatically worth its acquisition cost.**
43. **A decision may legitimately request evidence instead of choosing a candidate.**
44. **A selection result may be one candidate, a candidate set, a portfolio, a conditional policy, a randomized policy, a tie, an incomparability, or `UNRESOLVED`.**
45. **Adaptive selection does not bypass migration/lifecycle semantics.**
46. **Switching cost is not steady-state cost.**
47. **Hysteresis is policy, not an optimizer bug.**
48. **Search strategy is not the trusted decision semantics.**
49. **Candidate generation and selection checking remain separate.**
50. **RFC 0033 may select only from candidates admissible under protected earlier-RFC contracts unless an explicitly authorized relaxation process first changes those contracts.**

---

## 3. Decision pipeline

The canonical decision pipeline is:

```text
CandidateGeneration
        |
        v
EarlierSemanticChecks
        |
        v
RFC0032QuantitativeFeasibility
        |
        v
AdmissibleCandidateSet
        |
        v
DominanceAnalysis
        |
        v
Preference / Risk / Uncertainty Policy
        |
        v
SelectionOrInformationAction
```

No later stage may retroactively reinterpret a failed earlier hard requirement as an objective penalty.

### 3.1 Candidate generation

Candidate generation may be performed by:

- Forge;
- compiler realization search;
- placement/lifecycle planners;
- exact solvers;
- heuristic search;
- evolutionary algorithms;
- Bayesian/surrogate optimization;
- LLM/agent proposal;
- human proposal;
- imported realization libraries.

Generation is untrusted proposal work.

### 3.2 Admissible candidate set

```text
AdmissibleCandidateSet {
    candidate_ids
    semantic_snapshot
    assurance_snapshot
    quantitative_snapshot
    environment_snapshot
}
```

An admissible set is snapshot-relative. If its underlying evidence changes, admissibility may become stale without changing candidate identity.

### 3.3 Selection stage

Selection evaluates only candidates currently admissible under the decision instance.

If no candidate is admissible, the result is:

```text
NO_FEASIBLE_CANDIDATE
```

not “least bad candidate,” unless an independent, authorized relaxation process changes the feasible set.

---

## 4. Decision-problem identity

A decision problem is not just a set of candidates.

```text
DecisionProblem {
    subject_scope
    candidate_generator_scope
    required_relations
    hard_quantitative_constraints
    objective_definitions
    preference_schema
    uncertainty_schema
    risk_schema
    allowed_information_actions
    search_authority
    selection_authority
}
```

A concrete evaluation becomes:

```text
DecisionInstance {
    problem_id
    candidate_set
    evidence_snapshot
    objective_snapshot
    preference_profile
    uncertainty_model
    risk_policy
    search_budget
    current_realization?
    policy_version
}
```

Changing any decision-relevant policy or snapshot may produce a different decision instance even when candidate implementations are unchanged.

---

## 5. Requirements, constraints, objectives, and preferences

### 5.1 Hard requirements

A hard requirement defines admissibility.

```text
HardSelectionConstraint {
    predicate
    subject_scope
    authority
}
```

Example:

```text
PeakMemory <= 4 GiB
```

A candidate violating it does not compete by receiving a bad score.

### 5.2 Soft constraints

A soft constraint has explicit violation semantics.

```text
SoftSelectionConstraint {
    predicate
    violation_measure
    policy_role
}
```

The conversion from hard to soft is itself an authority-sensitive policy change.

### 5.3 Objectives

```text
Objective {
    metric
    direction: MINIMIZE | MAXIMIZE | TARGET | CUSTOM_ORDER
    scope
    transformation?
}
```

Objectives consume RFC 0032 metric/claim identities; they do not redefine metric meaning.

### 5.4 Preferences

```text
Preference {
    relation
    objective_scope
    condition?
    provenance
    authority
}
```

Preference is semantic input to the decision problem.

---

## 6. Objective vectors and commensurability

For candidate `c` under objective list `O`:

```text
ObjectiveVector(c, O)
```

is a vector of typed quantities or objective values.

The vector does not imply the dimensions are addable.

### 6.1 No implicit scalarization

Given:

```text
latency = 10 ms
memory = 2 GiB
energy = 9 J
```

MNCS must not compute:

```text
10 + 2 + 9
```

or any weighted equivalent unless a decision policy explicitly defines:

- transformations;
- normalization;
- direction;
- weights/tradeoffs;
- units or dimensionless utility mapping.

### 6.2 Normalization profile

```text
NormalizationProfile {
    objective
    method
    reference_values
    validity_scope
}
```

Normalization changes decision semantics and therefore belongs to policy identity.

---

## 7. Dominance semantics

Dominance is a relation over a declared objective set and scenario/evidence snapshot.

### 7.1 Pareto dominance

For minimization objectives, `A` Pareto-dominates `B` when:

```text
forall objective o:
    A[o] <= B[o]

and exists objective p:
    A[p] < B[p]
```

The direction is transformed appropriately for maximize/custom objectives.

### 7.2 Dominance artifacts

```text
DominanceClaim {
    candidate_a
    candidate_b
    objective_set
    comparison_context
    conclusion:
        DOMINATES
        DOMINATED
        INCOMPARABLE
        UNRESOLVED
    witness
}
```

### 7.3 Uncertainty-aware dominance

If objective evidence is incomplete or overlapping intervals prevent a sound conclusion, dominance remains `UNRESOLVED`.

Unknown must never be replaced with zero, best-case, worst-case, average, or sampled value unless a declared uncertainty policy explicitly does so.

---

## 8. Pareto set and Pareto front

### 8.1 Candidate-space object

```text
ParetoSet {
    decision_instance
    candidate_ids
}
```

### 8.2 Objective-space object

```text
ParetoFront {
    decision_instance
    objective_vectors
}
```

Candidate identity and objective-vector identity remain distinct.

### 8.3 Pareto membership is not selection

If two candidates trade latency against energy and neither dominates, both may remain Pareto-optimal.

RFC 0033 must not invent a winner before a preference policy resolves the tradeoff.

---

## 9. Exact versus approximate Pareto knowledge

Search may be incomplete.

```text
ParetoKnowledge {
    kind:
        EXACT
        APPROXIMATE
        BOUNDED
        HEURISTIC
    search_coverage
    evidence
}
```

### 9.1 Pareto approximation

```text
ParetoApproximation {
    candidates
    search_method
    search_budget
    approximation_metric?
    unresolved_search_regions?
}
```

Evolutionary methods such as NSGA-II are valuable proposal mechanisms for discovering diverse nondominated candidates. Their output is not automatically proof that the exact Pareto set has been found.

### 9.2 Front-quality indicators

Hypervolume, epsilon indicators, coverage, diversity, or spacing may describe quality of a Pareto approximation.

They are search-quality measures, not universal utility functions.

---

## 10. Preference timing

MNCS should support three broad preference-timing profiles.

### 10.1 A priori

Preference is supplied before search.

Search may focus only on the policy-relevant region.

### 10.2 A posteriori

Search first generates a Pareto or candidate set.

Preference is applied afterward.

### 10.3 Interactive

Preference evolves during search.

Each preference update creates a new policy/evidence dependency and may narrow future search without rewriting historical decisions.

---

## 11. Weighted utility / scalarization

Weighted scalarization is supported as an explicit policy family.

```text
WeightedUtilityPolicy {
    normalized_objectives
    weights
    aggregation
    authority
}
```

Requirements:

- weights must be explicit;
- normalization must be explicit;
- objectives must be mapped into a commensurable utility representation;
- weight provenance/authority must be recorded;
- changing weights changes policy identity.

Weighted sums are not the canonical definition of preference.

---

## 12. Lexicographic policy

```text
LexicographicPolicy {
    ordered_objective_or_policy_levels
}
```

Candidate comparison first considers the highest-priority level. Lower levels apply only if higher levels are tied under the declared comparison relation.

Lexicographic priority is semantically stronger than assigning a large finite weight.

Example:

```text
1. assurance profile strength
2. latency
3. energy
```

No amount of lower-priority energy improvement may compensate for a higher-level preference loss unless the policy explicitly permits that tradeoff.

---

## 13. Epsilon-constraint policy

```text
EpsilonConstraintPolicy {
    primary_objective
    secondary_constraints
}
```

Example:

```text
minimize latency
subject to:
    energy <= 10 J
    memory <= 4 GiB
```

Changing epsilon values changes the decision instance.

This policy family is especially useful when one objective is genuinely primary and the others are bounded tradeoffs rather than commensurable utility components.

---

## 14. Goal, aspiration, reservation, and reference-point semantics

### 14.1 Aspiration level

An aspiration is a desired target.

```text
AspirationLevel {
    objective
    target
}
```

### 14.2 Reservation level

A reservation level is a worst acceptable threshold under the preference policy.

```text
ReservationLevel {
    objective
    limit
}
```

A candidate may be acceptable without satisfying the aspiration.

### 14.3 Reference-point policy

```text
ReferencePointPolicy {
    aspirations
    reservations?
    achievement_scalarization_or_order
}
```

Reference points guide search and selection without being silently upgraded to hard semantic requirements.

---

## 15. Satisficing and bounded rationality

A machine-native system must be able to say that a candidate is good enough under explicit aspirations without pretending that global optimization has been completed.

```text
SatisficingPolicy {
    aspiration_predicates
    stopping_rule
    tie_policy?
}
```

Possible outcome:

```text
SATISFICING_SELECTION
```

with no `GLOBAL_OPTIMUM` claim.

### 15.1 Why satisficing is first-class

Search is itself resource-consuming.

If further exploration has low expected value relative to search/verification/experiment cost, stopping can be the rational authorized decision.

### 15.2 Satisfactory is not vague

All aspiration predicates must be explicit and checkable.

---

## 16. Partial preference and incomparability

Preference need not form a total order.

```text
PreferenceRelation {
    relation:
        PREFERRED
        DISFAVORED
        TIED
        INCOMPARABLE
        UNRESOLVED
}
```

### 16.1 Tie

Candidates are equivalent under the declared preference relation.

### 16.2 Incomparable

Policy does not define a ranking between the candidates.

### 16.3 Unresolved

A ranking may exist under policy, but current evidence is insufficient to establish it.

These three states must not collapse.

---

## 17. Tie-breaking policy

Tie-breaking must be explicit.

Possible policies:

- retain current realization;
- stable semantic identity order;
- lower switching cost;
- lower invalidation cost;
- randomized choice;
- return all tied candidates.

Incidental iteration order, hash ordering, filesystem ordering, discovery order, or process scheduling may not silently decide a tie.

---

## 18. Preference provenance and authority

```text
PreferenceProfile {
    preferences
    priorities
    risk_policy
    source_principals
    authority_evidence
    version
}
```

Potential preference sources include:

- user/operator;
- project policy;
- security policy;
- energy policy;
- battery policy;
- organization policy;
- regulatory profile;
- higher-level agent authorized to choose within a bounded envelope.

RFC 0025 determines whether the source has authority for the relevant preference class.

An optimizer may be authorized to search but not authorized to alter objectives, weights, risk appetite, or hard constraints.

---

## 19. Multi-principal preference composition

Several principals may supply preference information.

RFC 0033 does not prescribe one social-choice or voting mechanism.

Instead:

```text
PreferenceAggregationPolicy {
    contributing_profiles
    authority_relation
    conflict_resolution_rule
}
```

must be explicit.

Averaging weights is not a default constitutional rule.

Preference conflict is a semantic result, not automatically optimizer failure.

---

## 20. Utility models

Utility is optional.

```text
UtilityModel {
    consequence_space
    utility_function
    validity_scope
    authority
}
```

A utility model is appropriate only when the relevant decision maker or policy has authorized that representation of tradeoffs.

MNCS does not assume every partial or qualitative preference admits one meaningful scalar utility function.

---

## 21. Uncertainty model

Decision uncertainty must be explicit.

```text
UncertaintyModel {
    uncertain_claims
    uncertainty_kind
    parameterization
    evidence
}
```

At minimum distinguish:

- outcome randomness;
- model uncertainty;
- measurement uncertainty;
- workload uncertainty;
- target/environment uncertainty;
- search incompleteness;
- missing evidence.

Search incompleteness is particularly important: even a deterministic candidate space can leave the machine uncertain which candidate is best because search is bounded.

---

## 22. Probability versus preference

Probabilistic objective claims belong to RFC 0032/evidence layers.

RFC 0033 policy decides how to value or constrain those probabilities.

Example:

```text
P(latency > 100 ms) = 0.01
```

does not state whether `0.01` is acceptable.

Risk appetite belongs to policy.

---

## 23. Risk measures

```text
RiskMeasure {
    subject_metric
    measure_kind
    parameters
    orientation
}
```

Potential families include:

- expected loss;
- worst-case loss;
- chance constraint;
- Value at Risk;
- Conditional Value at Risk;
- entropic/coherent risk measures;
- regret.

Each is a distinct decision semantics.

### 23.1 VaR versus CVaR

VaR is a quantile-style threshold.

CVaR is a tail-average/expected-tail measure under the declared orientation and confidence level.

They must not be interchangeable.

---

## 24. Tail-sensitive policy

A candidate with excellent mean but catastrophic upper-tail outcomes may be less preferred than a candidate with a slightly worse mean and safer tail.

Policy may specify:

```text
minimize CVaR_0.99(latency)
```

or a chance constraint such as:

```text
P(latency > 100ms) <= 0.001
```

These are explicit policies, not generic “risk awareness.”

---

## 25. Robust optimization

```text
RobustSelectionPolicy {
    uncertainty_set
    protected_constraints
    objective
    conservatism_profile?
}
```

A robust policy evaluates candidates across a declared uncertainty set rather than only at a nominal point.

Robustness is always relative to that uncertainty set.

No universal `robust=true` claim exists.

---

## 26. Stochastic optimization

```text
StochasticSelectionPolicy {
    probability_model
    objective_or_utility
    risk_measure?
}
```

Stochastic optimization depends on an explicit probability model.

It is not equivalent to robust optimization.

---

## 27. Distributionally robust optimization

When finite data are available but one precise probability distribution is not trusted, policy may operate over an ambiguity set.

```text
DistributionallyRobustPolicy {
    ambiguity_set
    base_evidence
    objective
    risk_measure
}
```

Wasserstein-distance ambiguity balls, moment sets, phi-divergence sets, and future mechanisms are realization families.

---

## 28. Regret semantics

Regret compares a chosen action against the best alternative that would have been chosen with hindsight under a realized state/scenario.

```text
Regret(candidate, state) =
    value(candidate, state)
    - value(best_candidate_for_state, state)
```

with sign/orientation adapted to policy.

### 28.1 Minimax regret

```text
MinimaxRegretPolicy {
    scenario_set
    regret_definition
}
```

selects the candidate minimizing maximum regret.

Regret is not absolute loss.

---

## 29. Robust and stochastic dominance directions

RFC 0033 may support specialized dominance relations such as:

- robust dominance over an uncertainty set;
- first/second-order stochastic dominance where justified;
- interval dominance;
- scenario dominance.

These are scoped RFC 0020 relation families.

They do not replace ordinary Pareto dominance.

---

## 30. Search-state semantics

Search itself is inspectable state.

```text
SearchState {
    decision_instance
    generated_candidates
    evaluated_candidates
    infeasible_candidates
    dominated_candidates
    incumbent?
    pareto_approximation
    unresolved_regions
    search_budget_remaining
    search_method
}
```

This prevents the optimizer from presenting its current incumbent as if no alternatives could exist.

---

## 31. Search coverage

```text
SearchCoverage {
    kind:
        COMPLETE
        BOUNDED_ENUMERATION
        HEURISTIC
        STOCHASTIC
        SURROGATE_GUIDED
        AGENTIC
    explored_domain
    unexplored_domain?
    evidence
}
```

A `COMPLETE` claim needs stronger evidence than “search stopped.”

---

## 32. Optimality claims

Selection outcomes may carry different strength:

```text
OptimalityDisposition {
    FEASIBLE
    SATISFICING
    PARETO_OPTIMAL
    LOCALLY_OPTIMAL
    APPROXIMATE
    GLOBAL_OPTIMUM
    UNRESOLVED
}
```

A search tool may not claim a stronger disposition than its certificate/evidence supports.

---

## 33. Optimality gap

Where lower/upper optimization bounds are available:

```text
OptimalityGap {
    incumbent_value
    proven_bound
    gap
    relation
}
```

is stronger evidence than elapsed search time.

Approximation guarantees must name the metric/objective/policy they approximate.

---

## 34. Search strategies are realizations

Candidate search may use:

- exhaustive enumeration;
- branch-and-bound;
- dynamic programming;
- MILP/MINLP;
- SMT/OMT;
- constraint programming;
- gradient-based methods;
- local search;
- simulated annealing;
- evolutionary algorithms;
- NSGA-II-like multiobjective evolution;
- MOEA/D-like decomposition;
- epsilon-constraint enumeration;
- random search;
- Bayesian optimization;
- surrogate-assisted optimization;
- bandit methods;
- LLM/agent proposal;
- human-guided search;
- hybrid combinations.

None become canonical selection semantics.

---

## 35. Untrusted search / trusted checking split

The architecture is:

```text
Untrusted Search / Planner / AI / Optimizer
                |
                v
CandidateDecision / SearchWitness
                |
                v
Narrow Decision Checker
```

The checker may verify:

- candidate membership in the admissible set;
- hard-constraint satisfaction;
- objective-evidence bindings;
- normalization/policy identity;
- dominance arithmetic;
- lexicographic ordering;
- epsilon constraints;
- aspiration/reservation semantics;
- utility calculation under a declared utility model;
- risk-measure calculation from supplied evidence;
- tie policy;
- selection explanation;
- claimed exact optimality certificate when available.

Search itself remains outside the trusted core.

---

## 36. Surrogate and predicted objectives

```text
PredictedObjective {
    candidate
    metric
    prediction
    uncertainty
    model_identity
    training_evidence
}
```

Prediction is not measurement and not proof.

A decision policy may use predictions only when its evidence/assurance profile authorizes that use.

---

## 37. Bayesian/surrogate optimization

Surrogate-guided optimization is useful when candidate evaluation is expensive.

Possible acquisition-policy goals include:

- expected improvement;
- probability of improvement;
- confidence-bound exploration;
- entropy reduction;
- information gain;
- Pareto-front coverage;
- safe expansion.

The acquisition function is search policy, not semantic truth.

---

## 38. Exploration versus exploitation

Sequential selection may trade immediate utility against information gained about uncertain alternatives.

```text
ExplorationPolicy {
    horizon
    comparator
    exploration_authority
    exploitation_objective
}
```

Bandit/regret algorithms are realization families.

### 38.1 Search regret

```text
SearchRegret {
    policy
    horizon
    comparator
    accumulated_regret
}
```

may quantify the cost of learning over repeated selections.

---

## 39. Safe exploration

Unknown candidates may not be evaluated arbitrarily if evaluation itself can violate protected semantics.

```text
SafeSearchPolicy {
    safe_baseline_set
    hard_experiment_constraints
    permitted_exposure
    experiment_authority
}
```

A candidate may be:

- semantically valid for isolated execution;
- authorized for sandbox experimentation;
- unauthorized for production exposure.

These states remain distinct.

---

## 40. Baseline-safe set

```text
SafeBaselineSet {
    candidates
    established_safety_relations
    evidence
}
```

Search may expand from the baseline only where the candidate's experiment remains within the declared protected envelope.

SafeOpt-style algorithms are search realizations, not language semantics.

---

## 41. Information actions

The decision system may choose not only a candidate, but an information-acquisition action.

```text
DecisionAction =
    SelectCandidate
  | SelectPolicy
  | RunBenchmark
  | RunVerifier
  | RunExperiment
  | CollectMeasurement
  | ExploreCandidate
  | RequestPreference
  | DeferDecision
```

Every action remains subject to authority and earlier semantic constraints.

---

## 42. Value of information

```text
ValueOfInformationClaim {
    proposed_information_action
    current_decision_state
    possible_observations
    decision-improvement_model
    acquisition_cost
    result
}
```

The central question is:

> Would acquiring this information improve expected/robust/regret-sensitive decision quality enough to justify the explicit cost and risk of acquisition?

Information cost consumes RFC 0032 quantities.

Evidence significance remains RFC 0018-scoped.

---

## 43. Expected value of perfect and sample information

The ontology should permit:

- perfect-information upper bounds;
- sample-information value;
- experiment-specific information value;
- preference-information value.

These are decision-analysis artifacts.

They do not create authority to perform the proposed experiment.

---

## 44. Information-directed search

Search may explicitly trade immediate regret against information gain.

Information-directed sampling and related algorithms are realization families.

The canonical semantic object remains the policy goal:

```text
SearchForInformationEfficiency
```

with explicit regret/information quantities and search horizon.

---

## 45. Search cost and stopping

Search consumes resources under RFC 0032.

A machine-native decision may stop because:

- a satisfactory candidate exists;
- global optimality is proven;
- optimality gap is sufficiently small;
- no further information action has positive authorized value;
- Pareto approximation is stable enough under policy;
- search budget is exhausted;
- exploration authority is exhausted;
- no feasible unexplored region remains.

Each reason has different semantics.

---

## 46. Stopping-rule artifact

```text
StoppingRule {
    condition
    conclusion_strength
    policy_authority
}
```

Examples:

```text
StopWhenSatisfactory
StopWhenGlobalOptimalityProven
StopWhenGapBelowEpsilon
StopWhenNoPositiveValueOfInformation
StopWhenSearchBudgetExhausted
StopWhenFrontStableUnderMetric
```

Budget exhaustion may authorize stopping, but not a global-optimum claim.

---

## 47. Anytime optimization

An anytime search should expose:

```text
AnytimeState {
    incumbent
    pareto_approximation
    known_bounds
    search_budget_spent
    search_budget_remaining
    decision_status
}
```

This permits Fabric/Forge to stop and resume search without lying about completeness.

---

## 48. Static versus adaptive selection

Selection output may be:

```text
StaticSelection {
    candidate
}
```

or:

```text
ConditionalSelectionPolicy {
    context_predicates
    candidate_branches
}
```

Example:

```text
if workload = small:
    choose CPU
if workload = large and GPU available:
    choose GPU
else:
    choose distributed fallback
```

Every branch must remain admissible under the context in which it is selected.

---

## 49. Sequential decision semantics

One-shot and sequential decisions differ.

```text
DecisionHorizon =
    ONE_SHOT
  | FINITE_HORIZON
  | INDEFINITE
```

Exploration, regret, switching cost, and information value are horizon-sensitive.

The RFC reserves policy/MDP-like sequential decision models without requiring one for the first implementation.

---

## 50. Switching cost

Switching between realizations may require:

- state migration;
- cache/model warmup;
- code loading;
- lifecycle transition;
- re-attestation;
- reconfiguration;
- traffic cutover.

RFCs 0029-0030 own whether the transition is valid.

RFC 0032 quantifies its cost.

RFC 0033 decides whether the expected gain justifies the switch.

---

## 51. Hysteresis

```text
HysteresisPolicy {
    switch_threshold
    minimum_hold_time?
    evidence_stability_requirement?
}
```

prevents frequent switching when objective estimates fluctuate near a boundary.

Hysteresis is explicit decision policy.

---

## 52. Portfolio and set selection

Selection may return multiple candidates when the realization itself requires a set.

```text
PortfolioSelection {
    candidate_members
    role_assignment
    selection_relation
}
```

Examples include:

- heterogeneous replicas;
- ensembles;
- workload-partitioned realizations;
- redundant verifier sets;
- diverse rebuilders.

Earlier RFCs still define correctness of the resulting composite realization.

---

## 53. Randomized selection

```text
RandomizedSelectionPolicy {
    candidate_distribution
    purpose
    horizon
}
```

may be used for:

- exploration;
- load distribution;
- diversity;
- robustness.

Randomization may not assign positive probability to candidates that are inadmissible in the relevant context.

---

## 54. Preference sensitivity

A selection may depend strongly on policy parameters.

```text
PreferenceSensitivity {
    selected_candidate
    varied_policy_parameters
    stability_region
}
```

A candidate that wins only within a tiny weight/reference region is less decision-robust than a candidate selected across a broad region.

Sensitivity is information, not an automatic preference rule.

---

## 55. Evidence sensitivity

```text
EvidenceSensitivity {
    selected_candidate
    varied_objective_evidence
    stability_region
}
```

separates instability caused by uncertain measurements from instability caused by preference choices.

---

## 56. Decision robustness

```text
DecisionRobustnessClaim {
    selected_candidate_or_policy
    policy_uncertainty_set
    evidence_uncertainty_set
    workload_uncertainty_set
    conclusion
}
```

A candidate may be robustly selected over one set and unresolved over another.

No universal `robustly_best` exists.

---

## 57. Selection explanation

Every nontrivial selection should be able to produce an explanation artifact.

```text
SelectionExplanation {
    decision_instance
    infeasible_rejections
    dominance_rejections
    surviving_candidates
    preference_policy
    uncertainty_policy
    decisive_relations
    tie_break?
    selected_result
}
```

The explanation is machine-readable and dependency-aware.

---

## 58. Selection counterfactual

```text
SelectionCounterfactual {
    current_result
    changed_fact_or_policy
    resulting_selection
    threshold_or_condition
}
```

Examples:

- candidate A would win if energy fell below 8 J;
- candidate B would win if latency became the lexicographic first priority;
- candidate C would become feasible if memory budget rose from 1.0 to 1.2 GiB.

Counterfactuals are useful Forge repair/search targets.

---

## 59. Typed candidate disposition

Candidates may leave the decision pipeline because of:

```text
CandidateDisposition =
    SEMANTICALLY_INVALID
  | ASSURANCE_INSUFFICIENT
  | QUANTITATIVELY_INFEASIBLE
  | POLICY_FORBIDDEN
  | DOMINATED
  | LOWER_PREFERENCE
  | SEARCH_PRUNED
  | UNEVALUATED
  | SELECTED
```

These dispositions must remain distinct.

A dominated candidate may remain archived for future policy/environment changes.

---

## 60. Constraint relaxation proposals

No feasible candidate may exist.

Forge may produce:

```text
ConstraintRelaxationProposal {
    constraint
    proposed_change
    newly_feasible_candidates
    semantic_consequence
    quantitative_consequence
    authority_required
}
```

Forge may not enact the relaxation without appropriate authority.

“No feasible candidate” remains a valid decision outcome.

---

## 61. Decision outcomes

The core outcome family should include:

```text
DecisionOutcome =
    SELECTED
  | SATISFICING_SELECTION
  | SELECTED_POLICY
  | SELECTED_PORTFOLIO
  | TIED
  | INCOMPARABLE
  | UNRESOLVED
  | MORE_INFORMATION_WORTHWHILE
  | NO_FEASIBLE_CANDIDATE
```

Optional optimality metadata may accompany a selection.

---

## 62. Selection witness

```text
SelectionWitness {
    decision_instance
    selected_result
    admissible_set_digest
    objective_evidence
    dominance_evidence
    preference_profile
    risk_policy
    uncertainty_model
    policy_application_trace
    optimality_certificate?
}
```

The witness should be narrow enough for deterministic re-checking where the policy itself is deterministic.

---

## 63. Optimality certificate families

Potential exact/strong witnesses include:

- dual bounds from mathematical programming;
- exhaustive-domain certificate;
- branch-and-bound closure;
- unsatisfiability proof for any better solution;
- Pareto-completeness certificate in a finite modeled domain;
- epsilon-optimality bound;
- policy-specific dominance certificate.

Approximate/heuristic search may legitimately have no such global certificate.

---

## 64. Selection counterexamples

```text
SelectionCounterexample {
    decision_instance
    violated_rule
    candidate_or_policy
    witness
}
```

Examples:

- weighted policy included a hard-infeasible candidate;
- lexicographic checker selected a lower-priority winner incorrectly;
- normalization used an undeclared baseline;
- tie broken by incidental iteration order;
- optimizer assigned missing energy evidence a value of zero;
- unsafe experimental candidate evaluated outside allowed exposure;
- `GLOBAL_OPTIMUM` claimed after heuristic budget exhaustion.

---

## 65. Decision delta

```text
DecisionDelta {
    old_instance
    new_instance
    candidate_changes
    evidence_changes
    objective_changes
    preference_changes
    uncertainty_changes
    risk_changes
    selected_result_change
}
```

This enables localized invalidation and recursive re-selection.

---

## 66. Incremental invalidation

Selection depends on:

- candidate admissibility;
- quantitative evidence;
- preference profile;
- risk model;
- uncertainty model;
- search-state assumptions;
- current realization/switch cost.

If only energy preference changes, earlier semantic proofs need not be re-run.

If only a latency benchmark becomes stale, semantic correctness may remain intact while the selection decision becomes `UNRESOLVED`.

RFC 0018 dependency graphs should carry these scoped invalidations.

---

## 67. Forge realization-search architecture

Forge should operate approximately as:

```text
Generate Candidates
      |
      v
Check Earlier RFC Contracts
      |
      v
Check RFC0032 Hard Quantitative Feasibility
      |
      v
Maintain Feasible Candidate Set
      |
      v
Compute Dominance / Pareto Approximation
      |
      v
Apply Authorized Preference/Risk Policy
      |
      +------------------+
      |                  |
      v                  v
Select            Request Information
      |                  |
      v                  v
Check Witness      Safe Experiment / Verify
      |                  |
      +--------<---------+
```

The search loop is bounded by RFC 0022 and quantified by RFC 0032.

---

## 68. Forge study A — policy-dependent winner

Start from several RFC 0032-admissible realizations:

```text
B:
    latency 12 ms
    memory  2.0 GiB
    energy  9 J
    evidence_generation 20 s

C:
    latency 8 ms
    memory  3.5 GiB
    energy  15 J
    evidence_generation 60 s

D:
    latency 14 ms
    memory  1.5 GiB
    energy  6 J
    evidence_generation 5 s
```

None dominates the others.

Apply three policies:

1. lexicographic latency -> energy -> memory;
2. energy <= 10 J, then minimize latency;
3. satisfy latency <= 15 ms, memory <= 2 GiB, energy <= 10 J, then minimize verification cost.

The policies may legitimately select different candidates with identical metric evidence.

This fixture establishes that preference, not measurement alone, creates the winner.

---

## 69. Forge study B — uncertainty and information value

Candidates:

```text
A:
    well measured, stable, acceptable

B:
    predicted faster, uncertainty high
```

Possible decision actions:

- select A;
- select B;
- benchmark B;
- safely experiment with B;
- request more preference information.

Forge should evaluate whether the expected/robust/regret-sensitive improvement from the benchmark exceeds its RFC 0032 cost and risk.

The correct output may be:

```text
MORE_INFORMATION_WORTHWHILE
```

instead of premature candidate selection.

---

## 70. Forge study C — safe recursive search

Start from known-safe baseline A.

Generate B, C, D.

- D violates an RFC 0021 numeric requirement and leaves before RFC 0033.
- C is semantically valid but lacks authority/evidence for safe experiment.
- B is safe for bounded sandbox exploration.

Benchmark B, update the Pareto approximation, generate B2, and repeat under a finite search/experiment budget.

At no point may exploration authority be upgraded to deployment authority.

---

## 71. Forge study D — Fabric plan selection

Candidate Fabric execution plans:

- one large model;
- several small specialist models;
- planner plus micro-verifiers;
- aggressive parallel ensemble.

RFC 0032 reports:

- wall latency;
- total model calls/tokens;
- CPU/GPU work;
- network messages/bytes;
- verifier cost;
- energy;
- tail behavior.

RFC 0018 reports assurance.

RFC 0033 applies an explicit policy, for example:

```text
Requirements:
    assurance profile P
    latency <= 10 min

Preference:
    minimize energy
    then token consumption
```

The harness architecture becomes an evidence-bearing policy selection rather than undocumented routing folklore.

---

## 72. First implementation pilot

The first bounded implementation should introduce versioned:

- `DecisionProblem`;
- `DecisionInstance`;
- `CandidateSet`;
- `FeasibleCandidateSet`;
- `Objective`;
- `ObjectiveVector`;
- `NormalizationProfile`;
- `HardSelectionConstraint`;
- `SoftSelectionConstraint`;
- `DominanceClaim`;
- `ParetoSet`;
- `ParetoFront`;
- `ParetoApproximation`;
- `Preference`;
- `PreferenceProfile`;
- `PriorityOrder`;
- `AspirationLevel`;
- `ReservationLevel`;
- `SelectionPolicy`;
- `WeightedUtilityPolicy`;
- `LexicographicPolicy`;
- `EpsilonConstraintPolicy`;
- `SatisficingPolicy`;
- `UncertaintyModel`;
- `RiskMeasure`;
- `RobustSelectionPolicy`;
- `RegretPolicy`;
- `SearchState`;
- `SearchCoverage`;
- `ExperimentProposal`;
- `ValueOfInformationClaim`;
- `StoppingRule`;
- `SelectionDecision`;
- `SelectionWitness`;
- `OptimalityGap`;
- `SelectionExplanation`;
- `SelectionCounterfactual`;
- `SelectionCounterexample`;
- `DecisionDelta`.

The first implementation should remain intentionally small enough that policy application is independently checkable.

---

## 73. Required pilot fixtures

### Fixture 1 — Pareto does not select

A has lower latency, B lower energy.

Both are Pareto optimal.

No selection is permitted without policy.

### Fixture 2 — dominance elimination

A is no worse than B on all objectives and better on at least one.

B is removed by dominance without any weighting policy.

### Fixture 3 — hard constraint before score

B has a better weighted score but violates memory <= 4 GiB.

B is rejected before scoring.

### Fixture 4 — weighted versus lexicographic

The same candidates produce different valid winners under explicit weighted and lexicographic policies.

### Fixture 5 — epsilon constraint

Changing only an epsilon bound changes the selected candidate while all candidate metrics remain identical.

### Fixture 6 — aspiration versus reservation

A candidate satisfies reservation but not aspiration and remains acceptable under policy.

### Fixture 7 — satisficing

Search returns the first candidate satisfying all aspirations under `StopWhenSatisfactory` without claiming global optimality.

### Fixture 8 — approximate Pareto front

A bounded evolutionary search returns `ParetoApproximation`, not exact `ParetoSet`.

### Fixture 9 — unknown objective

Energy(A) is unknown. The checker refuses to fabricate a dominance conclusion.

### Fixture 10 — mean versus tail risk

Mean policy selects A; CVaR99/tail-sensitive policy selects B.

### Fixture 11 — nominal versus robust

A wins at nominal workload; B wins over every scenario in declared uncertainty set.

### Fixture 12 — minimax regret

Regret-based policy selects a different candidate than expected-value policy over the same scenarios.

### Fixture 13 — safe exploration

Uncertain candidate C may be promising but cannot be evaluated outside its authorized sandbox envelope.

### Fixture 14 — positive value of information

One experiment has expected decision value greater than acquisition cost; output is `MORE_INFORMATION_WORTHWHILE`.

### Fixture 15 — negative value of information

Further search is more expensive than any plausible authorized improvement; policy stops with best-known/satisficing result and no global-optimality claim.

### Fixture 16 — tie policy

A/B are tied; explicit `PreferExistingDeployment` chooses A because transition cost is avoided.

### Fixture 17 — preference sensitivity

A wins only in a narrow weight/reference region; emit a high-sensitivity decision artifact.

### Fixture 18 — no feasible candidate

All candidates violate at least one hard requirement. Return `NO_FEASIBLE_CANDIDATE` rather than least-invalid.

### Fixture 19 — unauthorized relaxation

Optimizer proposes widening memory budget. Checker rejects the policy mutation because search principal lacks constraint-relaxation authority.

### Fixture 20 — incidental tie order

Two tied candidates discovered in different enumeration orders produce the same explicit tie outcome unless a declared tie-break policy applies.

### Fixture 21 — budget exhaustion

Search exhausts budget with incumbent A. Result is `BEST_KNOWN`/satisficing or approximate according to policy, never `GLOBAL_OPTIMUM` without certificate.

### Fixture 22 — stale evidence

Latency evidence for selected A becomes stale. Decision becomes `UNRESOLVED` while unrelated semantic proofs remain valid.

### Fixture 23 — switching hysteresis

Alternating tiny metric changes do not trigger repeated A/B migration when policy requires a minimum improvement threshold.

### Fixture 24 — prediction versus measurement

Surrogate predicts B is best; measured evidence later differs. Historical prediction remains intact but cannot be rewritten as a measurement.

---

## 74. Micro-verifier opportunities

Candidate narrow verifiers include:

### 74.1 Objective binding verifier

Checks that every objective value references a valid RFC 0032 claim with correct workload/target/scope.

### 74.2 Hard-constraint verifier

Checks the admissible set excludes candidates violating hard constraints.

### 74.3 Dominance verifier

Checks Pareto-dominance arithmetic over supplied objective vectors.

### 74.4 Pareto-membership verifier

Checks nondominance within a declared finite candidate set without claiming search completeness outside that set.

### 74.5 Lexicographic verifier

Checks selected result follows the declared priority order.

### 74.6 Epsilon-policy verifier

Checks secondary bounds and primary-objective selection.

### 74.7 Satisficing verifier

Checks all aspirations and stopping conditions are satisfied without adding an optimality claim.

### 74.8 Risk-calculation verifier

Checks a declared risk measure from supplied distributional evidence.

### 74.9 Selection-witness verifier

Checks decision snapshot, policy identity, candidate membership, evidence bindings, and deterministic policy application.

### 74.10 Relaxation-authority verifier

Checks that any change to hard constraints or preference authority was approved by an appropriate RFC 0025 principal under RFC 0018 policy.

---

## 75. Proof and assurance boundaries

RFC 0033 does not make search trustworthy merely because the final arithmetic is checkable.

Potential evidence includes:

- exact solver certificates;
- finite candidate-set dominance proofs;
- Pareto approximation reports;
- stochastic search histories;
- surrogate models;
- empirical objective evidence;
- robust/risk calculations;
- value-of-information models;
- human or policy preference attestations.

RFC 0018 determines what evidence suffices for the action.

A heuristic Pareto approximation may authorize experimentation while a stronger production promotion policy may require stable selection plus current measurements and explicit preference authority.

---

## 76. Information-flow considerations

Decision state can reveal:

- confidential workloads;
- proprietary cost structures;
- security priorities;
- internal model performance;
- energy pricing;
- failure-risk estimates;
- strategic preferences.

RFC 0024 applies to:

- objective evidence;
- preference profiles;
- policy weights;
- candidate sets;
- search history;
- selection explanations.

A public decision explanation may require an explicitly related redacted projection.

---

## 77. Authority model

RFC 0025 should distinguish at least:

- candidate-generation authority;
- experiment authority;
- objective-definition authority;
- preference-definition authority;
- risk-policy authority;
- constraint-relaxation authority;
- selection authority;
- deployment/promotion authority.

No one role implies the others.

An AI optimizer may generate and rank candidates without possessing authority to mutate the decision policy.

---

## 78. Lifecycle integration

A selected realization does not automatically become active.

RFC 0030 still governs:

- deployment;
- canary;
- cutover;
- finalization;
- rollback;
- retirement.

RFC 0033 may select a lifecycle plan or a target candidate, but activation remains a separately authorized lifecycle action.

---

## 79. Placement integration

Selection may choose among RFC 0029-valid placements.

If selected policy adapts placement at runtime, every movement must preserve migration/cutover/trust semantics.

Switching cost comes from RFC 0032.

---

## 80. Build and provenance integration

Selection may choose among RFC 0031-valid build/reproduction strategies.

Example objectives:

- build latency;
- independent-rebuilder cost;
- evidence-storage cost;
- artifact size.

Supply-chain assurance remains a hard earlier-RFC requirement unless explicitly made a negotiable decision dimension by an authorized policy, which should be rare and carefully scoped.

---

## 81. Quantitative integration

RFC 0032 provides:

- metrics;
- units;
- quantities;
- bounds;
- distributions;
- workloads;
- capacities;
- budgets;
- QoS constraints;
- energy/power/carbon;
- hard quantitative feasibility.

RFC 0033 consumes these artifacts without redefining them.

A preference transformation must reference exact RFC 0032 metric identities.

---

## 82. Lowering and traceability

Decision artifacts should preserve:

- candidate semantic identity;
- realization identity;
- objective-claim identity;
- policy identity;
- selection-decision identity;
- evidence snapshot identity.

If a backend, runtime, deployment, or adaptive selector implements a selected policy, the relevant selection trace must remain auditable.

Lowering must not silently replace:

```text
lexicographic policy
```

with:

```text
weighted sum
```

or change epsilon/aspiration/risk parameters.

---

## 83. Canonical diagnostics

Examples:

```text
SELECTION_NO_FEASIBLE_CANDIDATE
```

```text
SELECTION_PARETO_INCOMPARABLE
```

```text
SELECTION_PREFERENCE_MISSING
```

```text
SELECTION_OBJECTIVE_UNKNOWN
```

```text
SELECTION_HARD_CONSTRAINT_VIOLATION
```

```text
SELECTION_UNAUTHORIZED_RELAXATION
```

```text
SELECTION_APPROXIMATE_FRONT_ONLY
```

```text
SELECTION_GLOBAL_OPTIMUM_UNSUPPORTED
```

```text
SELECTION_MORE_INFORMATION_WORTHWHILE
```

```text
SELECTION_INFORMATION_NOT_WORTH_COST
```

```text
SELECTION_UNSAFE_EXPLORATION
```

```text
SELECTION_POLICY_STALE
```

```text
SELECTION_EVIDENCE_STALE
```

```text
SELECTION_TIE_REQUIRES_POLICY
```

```text
SELECTION_INCOMPARABLE
```

```text
SELECTION_UNRESOLVED
```

Diagnostics should identify the minimal defensible policy/evidence dependency slice where possible.

---

## 84. Decision-policy realization search

Forge may itself search among decision policies only when authorized.

For example, it may be asked:

> Compare lexicographic and epsilon-constraint policies and show their consequences.

This is not authority to choose one policy silently.

The output should be a policy-comparison artifact showing:

- candidate selected by each policy;
- sensitivity;
- required preference information;
- unresolved tradeoffs.

---

## 85. Policy learning

A future MNCS system may learn preferences from repeated decisions.

Any learned preference is an inferred claim with provenance/evidence.

It is not equivalent to an explicit authorized policy unless a higher-level authority permits learned preferences to govern the relevant decision class.

This keeps behavioral inference from silently becoming policy authority.

---

## 86. Preference elicitation

Interactive systems may request information such as:

```text
Which matters more here:
    20% lower latency
or
    30% lower energy?
```

The answer becomes explicit preference evidence.

The system should prefer targeted questions with high expected value of information rather than demanding an exhaustive utility function when one is unnecessary.

---

## 87. Search and model budgets

RFC 0033 consumes RFC 0032 resource budgets such as:

- candidate count;
- optimizer iterations;
- verifier calls;
- benchmark runs;
- model calls;
- input/output tokens;
- CPU/GPU time;
- energy;
- elapsed time.

A stopping rule must identify whether the search stopped because:

- policy was satisfied;
- evidence was sufficient;
- budget was exhausted;
- no further feasible search actions existed.

---

## 88. Policy evolution

Decision policies themselves may evolve.

RFC 0033 should preserve:

- policy version identity;
- policy provenance;
- migration from old to new policy where needed;
- historical decisions under their original policy.

A past choice should not be retroactively rewritten because the current preference profile differs.

---

## 89. Decision reproducibility

A deterministic decision policy with the same candidate/evidence/policy snapshot should be reproducible.

A stochastic policy should instead provide:

- random-source identity/seed where appropriate;
- distributional selection semantics;
- replay information when required.

Decision reproducibility is distinct from RFC 0031 build reproducibility.

---

## 90. Machine-native theory synthesis

RFC 0033 draws selectively from the following theory families.

### 90.1 Pareto multiobjective optimization

Use Pareto dominance/nondominance to remove objectively inferior candidates without inventing tradeoffs.

### 90.2 A priori, a posteriori, and interactive multiobjective decision making

Use preference timing as an explicit decision design choice.

### 90.3 Weighted scalarization

Use only when an authorized commensurable utility/tradeoff model exists.

### 90.4 Epsilon-constraint methods

Use one primary objective with explicit bounds on others.

### 90.5 Lexicographic optimization

Use strict priority semantics where lower levels cannot compensate for higher-level losses.

### 90.6 Reference-point and goal programming

Use aspiration/reservation semantics for target-oriented decisions.

### 90.7 Satisficing and bounded rationality

Use explicit aspirations and stopping rules when exhaustive global optimization is unnecessary or uneconomic.

### 90.8 Expected utility / Bayesian decision theory

Use only when both probability and utility models are explicit and authorized.

### 90.9 Risk-sensitive decision theory

Support expected loss, chance constraints, VaR/CVaR, and other explicit risk measures.

### 90.10 Robust optimization

Use declared uncertainty sets to protect against adverse but modeled conditions.

### 90.11 Distributionally robust optimization

Use ambiguity sets when finite evidence does not justify one exact distribution.

### 90.12 Minimax regret

Use hindsight-relative opportunity loss when probability models are unavailable or inappropriate.

### 90.13 Evolutionary multiobjective optimization

NSGA-II-like and related approaches may generate approximate diverse nondominated candidate sets.

### 90.14 Exact mathematical optimization

MILP/SMT/OMT/constraint solving/branch-and-bound may produce strong feasibility or optimality witnesses where the modeled domain permits.

### 90.15 Bayesian/surrogate optimization

Use expensive evaluations efficiently while keeping prediction distinct from measurement.

### 90.16 Safe optimization

Use baseline-safe regions and hard experiment constraints to explore without sacrificing protected safety properties.

### 90.17 Multi-armed bandit/regret theory

Use repeated-decision models to balance exploration against exploitation.

### 90.18 Value-of-information and information-directed sampling

Choose which experiment or observation is worth acquiring before final selection.

### 90.19 Sensitivity/robust-decision analysis

Quantify how decision outcome changes under evidence/policy perturbation.

### 90.20 Proof-producing optimization

Keep expensive search outside the TCB and verify policy application/optimality claims through smaller checkers where feasible.

---

## 91. Reference theory and implementation families

Non-normative references include:

- Kaisa Miettinen, *Nonlinear Multiobjective Optimization*;
- Kalyanmoy Deb et al., NSGA-II;
- Stephen Boyd and Lieven Vandenberghe, convex optimization and lexicographic/constraint formulations;
- Herbert A. Simon, bounded rationality and satisficing;
- John von Neumann/Oskar Morgenstern and Bayesian decision-theory traditions for explicit expected utility;
- Rockafellar and Uryasev, Value-at-Risk and Conditional Value-at-Risk optimization;
- Bertsimas and Sim, robust optimization and budgeted uncertainty;
- distributionally robust optimization including Wasserstein ambiguity-set formulations;
- minimax-regret decision theory;
- Auer et al., finite-time multi-armed bandit analysis;
- Sui et al., SafeOpt/safe Bayesian optimization;
- Russo and Van Roy, information-directed sampling;
- classical value-of-information analysis;
- exact mathematical programming and satisfiability/optimization certificate systems.

These references motivate semantics and realization families; none is adopted wholesale as the language ontology.

---

## 92. First Forge decision checker

The first deterministic checker should accept:

```text
DecisionInstance
FeasibleCandidateSet
ObjectiveVectors
SelectionPolicy
CandidateSelection
```

and verify a deliberately small policy subset:

- hard-constraint candidate membership;
- two-objective Pareto dominance;
- finite Pareto-set membership over supplied candidates;
- lexicographic selection;
- epsilon-constraint selection;
- satisficing predicate satisfaction;
- explicit tie policy;
- selection witness identity.

It should reject:

- hidden weights;
- implicit normalization;
- candidate not in feasible set;
- missing objective treated as zero;
- unsupported global-optimum claim;
- unauthorized hard-constraint relaxation.

---

## 93. First safe-search pilot

A bounded safe-search pilot should:

1. begin with one known-safe candidate;
2. permit an untrusted proposer to generate nearby candidates;
3. run earlier RFC checks;
4. allow only sandbox-authorized candidates to receive experiments;
5. consume an RFC 0032 experiment budget;
6. update a Pareto approximation;
7. stop under explicit satisficing, value-of-information, or budget policy;
8. return a selection witness or justified unresolved result.

The pilot must not claim universal safe optimization.

---

## 94. First value-of-information pilot

Given candidates A/B and one uncertain objective on B:

```text
Current policy outcome:
    A selected provisionally

Possible experiment:
    measure B latency

Experiment cost:
    explicit RFC0032 quantity
```

The decision engine should produce either:

```text
RUN_EXPERIMENT
```

or:

```text
INFORMATION_NOT_WORTH_COST
```

from an explicit bounded decision model.

This remains model-relative evidence, not universal rationality.

---

## 95. First decision-sensitivity pilot

Given a weighted/reference-point policy, vary one authorized preference parameter over an interval.

Emit:

```text
SelectionRegion {
    parameter_interval
    selected_candidate
}
```

The experiment should demonstrate:

- stable selection region;
- boundary/tie region;
- alternate winner region.

This prevents a fragile near-boundary selection from appearing universally preferred.

---

## 96. RFC 0033 and recursive refinement

Selection counterexamples should feed RFC 0004 refinement.

Examples:

```text
Candidate C rejected:
    energy too high
```

can produce a bounded repair/search objective:

```text
Find refinement of C
preserving protected semantics
with Energy <= threshold
```

or:

```text
Candidate A loses only on latency
```

can focus optimization there.

Forge should preserve the causal reason for each search branch.

---

## 97. RFC 0033 and machine-intent

Machine-intent expressions may state:

```text
prefer lower energy
```

or:

```text
require memory <= 1GiB
```

but the compiler must preserve the distinction between preference and requirement through the decision model.

A preference cannot be strengthened into a hard requirement or weakened from a requirement without explicit policy authority.

---

## 98. RFC 0033 and assurance profiles

Different actions may require different selection assurance.

Example:

```text
Experiment:
    approximate Pareto + recent benchmark sufficient

Production:
    current hard-feasibility evidence
    explicit preference authority
    stable selection witness
    lifecycle authorization
```

The same candidate can be selected for sandbox experimentation but not production promotion.

RFC 0018 remains the appraisal layer.

---

## 99. Non-goals

RFC 0033 does not attempt to:

- prove every search problem globally optimal;
- mandate a single optimizer;
- mandate scalar utility;
- make all preferences total orders;
- make every objective numerically commensurable;
- replace human/operator judgment with opaque learned weights;
- choose security/reliability tradeoffs without explicit authority;
- guarantee that finite Pareto approximations are exact;
- make Bayesian predictions equivalent to measurements;
- turn canary/sandbox evidence into production authority;
- redefine RFC 0032 quantitative semantics;
- redefine earlier semantic admissibility.

---

## 100. Open research questions

1. Which preference-policy subset should be in the first canonical semantic core?
2. Should Pareto dominance be encoded as a specialized RFC 0020 relation artifact or a generic relation instance with RFC 0033 metadata?
3. Which exact optimality-certificate families can be independently checked with a small TCB?
4. How should uncertain interval/distributional objective claims compose with dominance?
5. Which value-of-information models are useful enough for an initial Forge implementation without overstating rationality?
6. How should learned preferences be authorized and invalidated?
7. How should multi-principal preference conflicts be surfaced without adopting premature social-choice machinery?
8. Which sensitivity metrics best expose fragile decision boundaries?
9. How should adaptive selection policy identity relate to RFC 0030 lifecycle generations?
10. Which safe-search algorithms work best when candidate evaluation itself invokes multiple RFC verifiers?
11. How should selection policy be serialized canonically while preserving qualitative/incomplete preference?
12. Can Forge derive useful policy questions automatically from Pareto incomparabilities?
13. When should a Pareto approximation be treated as stable enough for satisficing selection?
14. How should search-history evidence age when target/workload distributions change?
15. Can proof-carrying optimality certificates be generated for useful finite realization subspaces?

---

## 101. 1.0 research threshold additions

RFC 0033 should be considered represented at the 1.0 research threshold only when all of the following are demonstrated.

### 101.1 Decision ontology

- a versioned decision model distinguishing decision problem, decision instance, candidate set, feasible set, objective, preference, uncertainty, risk, search state, policy, selection, and witness identities;
- explicit hard/soft constraint semantics;
- explicit objective direction and scope;
- preference provenance and RFC 0025 authority binding;
- explicit decision snapshot/version identity.

### 101.2 Feasibility before preference

- at least one candidate with an attractive objective score that is rejected before preference application because it violates a hard earlier-RFC requirement;
- at least one `NO_FEASIBLE_CANDIDATE` result with typed failure slices;
- at least one unauthorized relaxation proposal mechanically rejected.

### 101.3 Pareto semantics

- versioned dominance and Pareto-set/front artifacts;
- at least one dominance elimination requiring no scalar preference;
- at least one pair of Pareto-incomparable candidates where no unique selection occurs until policy is supplied;
- explicit exact-versus-approximate Pareto knowledge;
- one bounded/heuristic front that remains labeled approximate.

### 101.4 Policy families

- at least three independently checkable selection-policy families including lexicographic, epsilon-constraint, and satisficing;
- at least one weighted/scalar policy demonstrating explicit normalization and weights rather than implicit metric arithmetic;
- one study where weighted and lexicographic policies select different candidates from identical objective evidence;
- one epsilon-bound change that changes selection without changing candidate metrics;
- one aspiration/reservation study preserving acceptability without aspiration satisfaction.

### 101.5 Partial preference

- machine-readable `TIED`, `INCOMPARABLE`, and `UNRESOLVED` outcomes;
- a tie-breaking policy that produces deterministic behavior independent of candidate-discovery order;
- one incomplete preference case that refuses to fabricate a winner.

### 101.6 Satisficing and bounded search

- one `StopWhenSatisfactory` study that terminates search after an aspiration-satisfying candidate without claiming global optimality;
- one search-budget exhaustion case that returns best-known/approximate status without upgrading to `GLOBAL_OPTIMUM`;
- explicit search-coverage metadata;
- optional optimality-gap evidence where supported.

### 101.7 Uncertainty and risk

- a versioned uncertainty/risk model;
- one mean-versus-tail-risk study where a CVaR/tail-sensitive policy selects differently from mean optimization;
- one nominal-versus-robust study using an explicit uncertainty set;
- one expected/stochastic versus minimax-regret or robust comparison demonstrating policy-dependent outcome;
- no missing objective value silently replaced by a favorable scalar.

### 101.8 Safe exploration

- a safe-baseline and safe-search artifact;
- one promising candidate blocked from experimental execution because experiment safety/authority is unresolved;
- one bounded candidate safely explored under a stricter sandbox authority than deployment authority;
- explicit separation of exploration and promotion.

### 101.9 Value of information

- a versioned experiment/information-action artifact;
- one case where additional information is selected because its modeled decision value exceeds RFC 0032 acquisition cost;
- one case where search/experiment stops because additional information is not worth its cost;
- explicit model-relative wording preventing bounded value-of-information calculations from becoming universal rationality claims.

### 101.10 Sensitivity and robustness

- one decision-sensitivity study showing at least two policy regions with different selected candidates;
- one evidence-sensitivity study where stale/uncertain objective evidence invalidates only dependent selection conclusions;
- one robust-selection claim scoped to explicit preference/evidence/workload uncertainty sets.

### 101.11 Adaptive selection

- a bounded conditional-selection policy with at least two contexts and independently checked candidate admissibility in each context;
- one switching-cost/hysteresis example preventing repeated oscillatory migration;
- explicit RFC 0029/RFC 0030 validation of any runtime switch.

### 101.12 Trusted boundary

- at least one untrusted optimizer, evolutionary method, solver, learned proposer, or LLM producing a candidate selection or Pareto approximation;
- a narrower deterministic checker independently validating hard-constraint membership, objective bindings, dominance/policy application, and witness identity;
- unsupported exact/global-optimum claims mechanically rejected.

### 101.13 Forge capstone

- a Forge study beginning with multiple RFC 0032-admissible candidates where no candidate dominates all others;
- at least two explicit authorized decision policies selecting different legitimate winners from the same candidate facts;
- one decision state in which Forge requests more evidence instead of selecting;
- one decision state returning `INCOMPARABLE`, `UNRESOLVED`, or `NO_FEASIBLE_CANDIDATE` rather than inventing a winner;
- full decision history retained as machine-readable evidence for recursive refinement.

The 1.0 threshold therefore requires MNCS to demonstrate not merely optimization machinery, but a trustworthy boundary between **what is valid, what is measured, what is preferred, what is uncertain, what is worth learning, and what is actually authorized to be selected.**

---

## 102. Final constitutional statement

The defining principle of RFC 0033 is:

> **Optimization does not create preference. It searches the space permitted by semantics and exposes tradeoffs; selection occurs only when an explicit decision policy authorizes how those tradeoffs may be resolved.**

The corresponding machine-native rule is:

> **MNCS should not ask a machine to find “the best implementation.” It should give the machine an admissible realization space, explicit objectives, uncertainty, risk policy, preference authority, evidence state, and search budget—and require the machine either to produce a justified selection or to state precisely why no justified selection is yet possible.**
