# RFC 0022: Machine-Native Termination, Productivity, Progress, Liveness, and Bounded-Computation Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.2–1.0
- **Depends on:** RFC 0001, RFC 0002, RFC 0003, RFC 0004, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0013, RFC 0014, RFC 0015, RFC 0016, RFC 0017, RFC 0018, RFC 0019, RFC 0020, RFC 0021

## Summary

MNCS Language should treat **termination, totality, divergence, productivity, local progress, concurrent progress, temporal liveness, fairness, quiescence, stabilization, and bounded execution as distinct semantic properties** rather than collapsing them into a single notion of “finishes” or inferring them from one successful or unsuccessful execution.

The central rule is:

> **Termination, totality, productivity, progress, liveness, fairness, bounded completion, and divergence are distinct claims. None may be silently inferred from another.**

A second rule is equally important:

> **Failure to prove termination is not evidence of divergence, and exhaustion of a finite execution budget is not evidence of nontermination.**

RFC 0022 establishes a machine-native vocabulary and evidence model capable of representing questions such as:

- Does a computation terminate for all inputs in a declared domain?
- Does it merely have at least one terminating execution?
- Can it diverge under another valid nondeterministic schedule or branch?
- Is termination proved by structural descent, a ranking function, a well-founded relation, a size-change certificate, or another witness?
- Is a nonterminating stream producer nevertheless productive?
- Does every requested finite observation arrive after finite work?
- Is a concurrent operation wait-free, lock-free, obstruction-free, deadlock-free, or only experimentally responsive?
- Does a service satisfy a request-response liveness property only under an explicit scheduler-fairness assumption?
- Has a computation proved to terminate, proved to diverge, or merely remained unresolved?
- Did an experimental run exhaust its fuel, semantic-step, transition, recursion, observation, or candidate budget?
- Which termination/progress/liveness relation from RFC 0020 is being claimed?
- Which evidence is sufficient under RFC 0018 to authorize experimentation, lowering, deployment, or promotion?

The RFC deliberately does **not** make universal totality a requirement for ordinary MNCS execution. General recursive programs, long-running services, event loops, streams, reactive systems, concurrent workers, search procedures, solvers, and intentionally nonterminating processes remain legitimate computations. What changes is that their completion and progress contracts become explicit.

The intended design is especially important for machine-generated software. Forge must be able to explore unfamiliar algorithms without either:

1. granting them unjustified termination authority because they happened to finish in testing; or
2. rejecting them merely because a particular static termination checker cannot prove them total.

MNCS therefore separates **semantic behavior**, **proof obligations**, **bounded operational safety**, **evidence**, and **authorization**.

---

## Motivation

The existing RFC set already depends on termination-like concepts in several places:

- RFC 0004 requires bounded recursive refinement and finite mutation/search authority;
- RFC 0007 needs a terminating or otherwise controlled proof-checking/normalization boundary;
- RFC 0010 introduces concurrent executions where completion, starvation, and deadlock are independent questions;
- RFC 0011 distinguishes failure, nondeterminism, external observation, and unresolved world knowledge;
- RFC 0012 defines executable computations and transitions but does not yet classify all completion/progress behavior;
- RFC 0016 permits staged and self-transforming computation that must remain bounded where authority requires it;
- RFC 0017 defines runtime/environment context that can make liveness or fairness claims applicable or inapplicable;
- RFC 0018 requires evidence and assumptions to remain scoped to the exact property and action being authorized;
- RFC 0020 provides the relation vocabulary needed to distinguish bounded agreement, refinement, and observational claims;
- RFC 0021 explicitly defers exact-real computability and termination questions here.

Without one completion/progress semantics, different subsystems could incorrectly treat these statements as interchangeable:

```text
completed in one run
terminates for this input
terminates for every finite input
must terminate under every schedule
has at least one terminating branch
is productive forever
never deadlocks
some thread always progresses
every thread eventually progresses
will eventually respond under fair scheduling
exhausted a 100000-step budget
provably diverges
```

They are not interchangeable.

A machine-native system needs a precise model of **what kind of progress is required, over which input/schedule/environment domain, under which assumptions, with which witness, and with what operational fallback if the property remains unresolved**.

---

# Constitutional principles

## 1. Unknown termination is not divergence

> **A computation for which MNCS lacks sufficient termination evidence remains termination-unresolved unless there is independent evidence of divergence.**

The language must preserve at least the distinction among:

```text
TERMINATES
DIVERGES
UNRESOLVED
```

for a particular scoped termination claim.

No timeout, fuel exhaustion, verifier timeout, proof-search failure, or missing ranking function may be silently converted into `DIVERGES`.

## 2. Budget exhaustion is an observation, not a theorem

> **`BUDGET_EXHAUSTED` means only that a particular bounded execution did not reach its requested terminal/observation condition within the declared budget metric and bound.**

It does not prove nontermination.

Likewise, successful execution under a finite corpus does not prove universal termination.

## 3. Termination is not correctness

A computation may terminate by:

- returning a valid result;
- returning a declared failure;
- entering a declared terminal cancellation state;
- hitting an authorized trap; or
- reaching another terminal state.

Termination says that a terminal state is reached. Separate contracts determine whether that terminal outcome is acceptable.

## 4. Nontermination is not inherently incorrect

Long-running services, streams, event loops, monitors, reactors, protocol engines, and other open computations may intentionally never terminate.

Their contracts may instead require:

- productivity;
- responsiveness;
- deadlock freedom;
- request-response liveness;
- quiescence of a workload;
- stabilization of an invariant;
- bounded per-operation progress.

## 5. Productivity is not termination

> **A productive computation may run forever while guaranteeing that each requested finite observation becomes available after finite work.**

MNCS must not reject a productive stream merely because global termination is false.

## 6. May-termination is not must-termination

Under nondeterminism:

```text
MayTerminate(P)
```

means that at least one permitted execution terminates.

```text
MustTerminate(P)
```

means that every permitted execution in the declared scope terminates.

These collapse only when the execution model justifies that collapse.

## 7. Fairness is an assumption or evidenced runtime property

> **A liveness proof that depends on scheduler/network/environment fairness must name that fairness assumption explicitly.**

No runtime is silently “fair enough.”

## 8. Local progress is not temporal liveness

A well-typed machine state may be able to take another legal transition forever.

Therefore:

```text
CanReduce
```

or

```text
NotStuck
```

does not imply eventual termination, response, quiescence, or fairness.

## 9. Concurrent progress is multidimensional

Deadlock-free, starvation-free, obstruction-free, lock-free, and wait-free are separate progress claims.

No universal `concurrent_progress = true` property is sufficient.

## 10. Proof index is not execution fuel

Step indices used in semantic/logical proofs may be meta-level reasoning devices.

Runtime fuel is operational authority.

They must remain semantically distinguishable unless a specific formal relation connects them.

## 11. Termination claims are domain-scoped

`Terminates(f)` is incomplete unless the relevant input, state, context, environment, schedule, and nondeterminism quantification are known.

## 12. Progress/liveness claims preserve assumptions

A liveness result derived under weak fairness, a reliable clock, a bounded queue, or a specific failure model becomes inapplicable if those dependencies no longer hold.

## 13. Stronger evidence never changes the property being evidenced

A proof of deadlock freedom does not become a proof of wait freedom.

A proof of productivity does not become a proof of termination.

An empirical latency distribution does not become a termination theorem.

RFC 0018 may authorize an action using weaker evidence where policy permits, but policy may not relabel evidence.

---

# Semantic separation

RFC 0022 proposes at least seven semantically distinct planes.

## Completion plane

Describes whether an execution may, must, or does not reach a terminal state.

Candidate properties include:

```text
MayTerminate
MustTerminate
BoundedTerminate
AlmostSureTerminate
Diverges
```

## Normalization plane

Describes reduction-to-normal-form properties used especially by the proof/logical stratum.

```text
WeakNormalization
StrongNormalization
```

## Productivity plane

Describes finite observability of potentially infinite computations.

```text
Productive
ProductiveWithinBudget
```

## Local progress plane

Describes whether a nonterminal well-formed state can take a legal next step.

```text
CanReduce
NotStuck
```

## Concurrent progress plane

Describes completion guarantees of concurrent operations or systems.

```text
DeadlockFree
StarvationFree
ObstructionFree
LockFree
WaitFree
```

## Temporal liveness plane

Describes eventual temporal properties.

```text
Eventually(P)
AlwaysEventually(P)
EventuallyAlways(P)
LeadsTo(P,Q)
```

## Bounded-operation plane

Describes finite authority assigned to evaluation, search, recursion, transition, observation, verification, or candidate generation.

```text
ComputationBudget
BoundedExecutionResult
```

These planes may interact, but they must not be flattened into one ordered status.

---

# Candidate semantic objects

A future canonical model may include objects conceptually similar to:

```text
TerminationRequirement
TerminationClaim
TerminationMode
TerminationWitness
StructuralDescentWitness
WellFoundedRelation
AccessibilityWitness
RankingFunction
LexicographicRanking
MultisetRanking
SizeChangeGraph
SizeChangeCertificate
DivergenceClaim
PartialComputationContract
ProductivityRequirement
ProductivityClaim
GuardWitness
SizeIndexWitness
ProgressRequirement
ProgressClaim
ProgressCondition
LivenessRequirement
LivenessClaim
TemporalProperty
FairnessAssumption
FairnessModel
QuiescenceClaim
StabilizationClaim
ComputationBudget
BudgetMetric
BoundedExecutionRequest
BoundedExecutionResult
BudgetExhaustionObservation
CompletionCounterexample
TerminationDelta
ProgressDelta
LivenessDelta
```

Exact names and serialization are not frozen by this Draft.

---

# Part I — totality and explicit partiality

## Total computation

A total computation establishes that, for every input/state in its declared domain and under its declared execution assumptions, evaluation reaches an allowed terminal result.

Conceptually:

```text
TotalComputation<A>
```

is stronger than merely having return type `A`.

The totality claim depends on:

- input domain;
- failure/terminal-state model;
- nondeterminism quantification;
- environment assumptions;
- recursive callees;
- foreign/unsafe dependencies;
- termination evidence.

## Partial computation

MNCS should permit explicit partial computation.

Conceptually:

```text
PartialComputation<A>
```

may:

```text
Return(A)
Fail(declared_failure)
Continue(...)
Diverge
```

The exact operational representation need not be a literal datatype. The important semantic rule is that general recursion and potential divergence remain properties of computation, not hidden inhabitants of every ordinary value type.

## Why MNCS should not require universal totality

Universal totality would exclude or distort legitimate workloads such as:

- operating-system loops;
- actors and servers;
- event processing;
- reactive controls;
- search with unknown stopping conditions;
- numerical procedures whose exact-real decisions are semidecidable/partial;
- protocol participants;
- user-driven interactive systems;
- self-improving/iterative experimental loops;
- intentionally persistent agents.

The machine-native requirement is therefore **explicitness**, not totality everywhere.

---

# Part II — structural recursion

## Structural descent

Structural recursion should be the simplest termination witness.

Given an inductive value, a recursive call is structurally decreasing when it consumes a strict subcomponent under the datatype's inductive structure.

Example:

```text
length([]) = 0
length(x :: xs) = 1 + length(xs)
```

The recursive call uses `xs`, a strict structural subvalue of `x :: xs`.

A canonical witness may resemble:

```text
StructuralDescentWitness {
    subject
    recursive_call_sites
    decreasing_parameter
    constructor_relation
}
```

## Advantages

Structural descent is attractive because it is:

- local;
- deterministic;
- inexpensive to verify;
- suitable for independent implementations;
- easy for agents to synthesize;
- stable under many refactorings;
- naturally connected to RFC 0019 inductive data.

## Limitation

MNCS must not make structural recursion the only legal proof method. Many valid algorithms decrease semantic measures not directly visible as syntactic subterms.

---

# Part III — well-founded recursion

## General model

The general logical foundation should be well-founded descent.

Conceptually:

```text
WellFoundedRelation<R>
```

establishes that there is no infinite chain:

```text
x0 >R x1 >R x2 >R ...
```

A recursive transition from `current` to `next` then generates an obligation:

```text
R(next, current)
```

## Candidate relation domains

Useful well-founded domains include:

```text
Natural-number <
Lexicographic tuples
Finite-tree height
Finite-container cardinality
Multiset extensions
Domain-specific ordinal-like measures
User-/machine-defined relations with well-foundedness evidence
```

## Termination measure

A machine-native object may resemble:

```text
TerminationMeasure {
    subject
    state_domain
    measure
    codomain
    well_founded_order
    assumptions
}
```

Each recursive/cyclic transition carries a `DecreaseEvidence` obligation.

## Accessibility

Where the proof core benefits from it, accessibility-style evidence may encode that a state admits no infinite descending sequence under the selected relation.

RFC 0022 does not require one proof-theoretic encoding; RFC 0007 owns the trusted proof representation.

---

# Part IV — ranking functions

## Imperative loops

For loops and state machines, the well-founded measure often appears as a ranking function:

```text
rank : ProgramState -> W
```

with obligations:

```text
loop_guard(state)
=> rank(next(state)) <W rank(state)
```

and `W` well founded.

## Ranking witnesses

Candidate artifacts:

```text
RankingFunction {
    state_projection
    codomain
    order
}

RankingWitness {
    loop_or_scc
    ranking_function
    decrease_obligations
    lower_bound_or_well_founded_evidence
}
```

## Lexicographic rankings

Many algorithms require multicomponent rankings:

```text
rank(state) = (outer, inner)
```

under lexicographic order.

The first component may stay constant while the second decreases; when the first decreases, the second may reset.

MNCS should represent this directly instead of forcing artificial scalar encodings.

## Multiset/structured rankings

Future analyses may require multisets, trees, ordinals, or other structured measures.

The semantic model should therefore define ranking by relation/well-founded evidence rather than hard-code integer decrement.

---

# Part V — size-change termination

## Motivation

Size-change termination is particularly attractive for machine-native verification because a verifier may infer termination from local call information without requiring a human-authored global ranking function.

A call edge may record relations such as:

```text
f(a,b) -> g(a',b')

a' < a
b' <= b
```

Composition through recursive call cycles can reveal whether every infinite path would require impossible infinite descent.

## Size-change graph

A candidate artifact may resemble:

```text
SizeChangeGraph {
    caller
    callee
    argument_relations
}
```

Relations might initially include:

```text
STRICT_DECREASE
NON_INCREASE
UNKNOWN
```

## Certificate

Rather than trusting the inference engine, Forge may emit a checkable certificate:

```text
SizeChangeCertificate {
    subject_scc
    graph_set
    closure_summary
    descending_cycle_argument
}
```

A small independent checker can validate it.

This is a strong candidate for a dedicated Haskell or other proof-oriented micro-verifier, while the canonical language remains implementation-neutral.

---

# Part VI — sized types and machine-only size indices

## Size indices

RFC 0019 indexed data can be extended with size information useful for termination/productivity reasoning.

Conceptually:

```text
Data<T, Size<i>>
```

where `i` need not be a runtime value or human-visible annotation.

Recursive calls may require:

```text
j < i
```

## Machine-native use

Size indices are especially appealing because they can exist only in canonical semantic/proof representations.

Agents and verifiers may infer them even if the human surface syntax remains uncluttered.

## Erasure

Where size evidence has no runtime role, it may be erasable after obligations are discharged, subject to RFC 0007 proof-erasure policy.

---

# Part VII — normalization

## Weak normalization

A term is weakly normalizing when at least one valid reduction sequence reaches normal form.

## Strong normalization

A term is strongly normalizing when every valid reduction sequence reaches normal form.

## Proof-kernel relevance

The RFC 0007 kernel may require strong normalization or another sufficiently controlled normalization discipline for definitional equality and proof checking.

This requirement should not automatically apply to ordinary executable MNCS programs.

## Separate identities

```text
KernelNormalizationClaim
```

and

```text
ProgramTerminationClaim
```

must remain distinct even where they share well-founded proof techniques.

---

# Part VIII — divergence

## Divergence as a positive claim

MNCS should permit an evidenced claim:

```text
Diverges(subject, scope)
```

where every execution in the claim's quantified model continues indefinitely without reaching a terminal state.

## Evidence

Possible divergence evidence may include:

- a coinductive proof;
- a cycle/invariant proof showing perpetual continuation;
- a proof that no terminal state is reachable while progress remains possible;
- another verifier-specific certificate.

## What does not prove divergence

The following do not establish universal divergence by themselves:

```text
timeout
budget exhaustion
large finite trace
failed termination prover
unknown ranking function
solver timeout
human suspicion
```

They may become evidence of difficulty or bounded noncompletion, but not a universal divergence theorem.

---

# Part IX — may-, must-, and almost-sure termination

## May-termination

```text
MayTerminate(P)
```

means that at least one execution permitted by the declared nondeterminism/schedule/environment model reaches a terminal state.

## Must-termination

```text
MustTerminate(P)
```

means that every permitted execution in the declared model reaches a terminal state.

## Example

```text
choose {
    return 1
    loop_forever()
}
```

may satisfy `MayTerminate` while failing `MustTerminate`.

## Almost-sure termination

Probabilistic computation introduces another relation:

```text
AlmostSureTerminate(P, probability_model)
```

meaning termination occurs with probability 1 under the declared probability space/model.

This is not identical to must-termination because measure-zero nonterminating executions may remain.

## Ranking supermartingales

Future probabilistic verifiers may use ranking-supermartingale certificates.

RFC 0022 reserves the claim/witness vocabulary but does not require a probabilistic termination engine in the first implementation.

## Expected completion cost

Claims about expected number of steps, expected latency, or asymptotic cost belong primarily to RFC 0032 quantitative semantics, though they may reference RFC 0022 termination properties.

---

# Part X — productivity and coinductive computation

## Productivity

For a potentially infinite producer, the appropriate property is often:

```text
Productive(P)
```

meaning each demanded finite observation can be produced after finite computation.

## Stream example

```text
ones = 1 :: ones
```

is not terminating as a complete computation.

It can nevertheless be productive because each next element becomes available after finite work.

## Observation unit

A productivity claim must identify what counts as an observation:

```text
ProductivityClaim {
    subject
    observation_model
    demand_model
    assumptions
    witness
}
```

Examples:

- one stream element;
- one protocol response;
- one event batch;
- one incremental compiler result;
- one generated candidate;
- one reactive-state update.

## Productivity within a bound

A stronger claim may establish:

```text
ProductiveWithin(subject, observation, budget)
```

using abstract semantic steps.

Physical deadlines remain RFC 0023.

---

# Part XI — guarded recursion

## Later modality

Guarded recursion provides a useful conceptual device:

```text
Later<T>
```

meaning that `T` becomes available after a semantic step/guard boundary.

A stream may be modeled conceptually as:

```text
Stream<T> = T × Later<Stream<T>>
```

## Guard condition

Recursive self-reference is permitted only underneath a guard/delay that ensures an observation is exposed before recursion resumes.

## Guard witness

```text
GuardWitness {
    recursive_reference
    guarding_observation
    delay_relation
}
```

## Canonical rather than surface syntax

MNCS need not expose a literal `Later` operator to human programmers. The canonical core may represent guardedness even if source syntax uses generators, streams, async producers, or reactive declarations.

---

# Part XII — local progress and preservation boundary

## Reduction progress

Local progress means a well-formed nonterminal state can take a legal next step.

Conceptually:

```text
LocalProgress(state)
```

or:

```text
NotStuck(state)
```

## Preservation

RFC 0007/0012 may establish that a legal step preserves relevant typing/semantic invariants.

RFC 0022 uses that foundation but does not redefine type preservation.

## Infinite progress

A computation may satisfy local progress indefinitely and still never terminate.

Therefore local progress is not a liveness theorem.

---

# Part XIII — concurrent progress

Concurrent algorithms require a family of progress properties.

## Deadlock freedom

```text
DeadlockFree
```

means the system cannot enter a nonterminal state where all required progress is permanently blocked by cyclic/resource waiting under the claim's model.

It does not imply fairness or termination.

## Starvation freedom

```text
StarvationFree
```

means an eligible operation/thread is not postponed forever under the declared scheduler assumptions.

This normally depends on fairness assumptions.

## Obstruction freedom

```text
ObstructionFree
```

means an operation completes if it eventually executes for sufficiently long without interference.

## Lock freedom

```text
LockFree
```

means the system as a whole continually makes operation-level progress: some operation completes in a finite number of steps, though a particular operation may starve.

## Wait freedom

```text
WaitFree
```

means every operation completes within a finite number of its own steps under the declared model, independent of delays in other participants.

## Partial ordering

Some progress conditions admit useful implication relationships under specific formal definitions, but RFC 0022 should not encode a universal ladder without fixing those exact definitions and scopes.

Each claim should instead identify its `ProgressCondition` and verifier semantics.

## Interaction with RFC 0010

RFC 0010 owns:

- concurrency semantics;
- causality;
- atomicity;
- memory consistency;
- synchronization behavior.

RFC 0022 owns the generic progress-property vocabulary applied to those executions.

---

# Part XIV — temporal liveness

## Eventuality

Liveness properties describe something desirable eventually occurring.

Candidate temporal forms include:

```text
Eventually(P)
AlwaysEventually(P)
EventuallyAlways(P)
LeadsTo(P,Q)
```

## Examples

```text
request_accepted
LeadsTo
response_or_declared_terminal_failure
```

```text
task_queued
LeadsTo
task_completed
```

```text
AlwaysEventually(heartbeat)
```

## Canonical representation

RFC 0022 need not select a full temporal-logic surface language.

A canonical temporal property may be structured as:

```text
TemporalProperty {
    trigger
    response
    quantification
    allowed_terminal_outcomes
    observation_model
}
```

with a later richer temporal calculus if required.

---

# Part XV — fairness

## Fairness as explicit dependency

Many liveness claims are false without scheduling/environment assumptions.

Therefore:

```text
LivenessClaim {
    property
    fairness_assumptions
    environment_assumptions
    evidence
}
```

must preserve those assumptions.

## Weak fairness

Conceptually, weak fairness prevents an action that remains continuously enabled from being postponed forever.

## Strong fairness

Conceptually, strong fairness covers stronger cases where an action may become enabled repeatedly rather than continuously.

## No ambient fairness

MNCS must not assume scheduler fairness merely because a runtime is described as cooperative, preemptive, work-stealing, or concurrent.

The runtime/environment must provide evidence or an explicit assumption recognized by the assurance policy.

## Applicability

Changing scheduler, runtime version, worker topology, priority model, or failure model may invalidate a liveness claim even if program code is unchanged.

This is a direct RFC 0017 + RFC 0018 freshness/applicability interaction.

---

# Part XVI — quiescence and stabilization

## Quiescence

A long-running system may remain alive while its current workload drains.

```text
EventuallyQuiescent
```

can mean that the system eventually reaches a state with no pending relevant work under a declared workload/input model.

This differs from process termination.

## Stabilization

A system may converge to an invariant regime:

```text
EventuallyAlways(P)
```

or a named:

```text
Stabilizes(P)
```

claim.

Examples:

- an adaptive optimizer eventually stops changing a selected realization;
- a cache eventually reaches a stable coherence state under fixed input;
- a distributed configuration eventually satisfies an invariant after transient faults.

Full distributed self-stabilization semantics remain RFC 0028 territory; RFC 0022 establishes the generic temporal concept.

---

# Part XVII — bounded computation

## Why bounded computation is constitutional

Machine-generated and experimental code frequently has unresolved termination behavior.

MNCS must still be able to execute it safely.

Therefore bounded execution is not merely a debugger feature; it is a semantic/authority mechanism.

## Budget metrics

A computation budget may count:

```text
SemanticSteps
ReductionSteps
StateTransitions
RecursiveCalls
LoopIterations
Observations
VerifierCalls
SolverCalls
CandidateExpansions
SearchNodes
RefinementDepth
```

Each metric must be explicit.

## Budget object

```text
ComputationBudget {
    metric
    limit
    scope
    owner
    reset_policy
    escalation_policy
}
```

## Result outcomes

A bounded execution may return:

```text
RETURNED
DECLARED_FAILURE
FATAL_TERMINAL
BUDGET_EXHAUSTED
UNSUPPORTED
INVALID_REQUEST
```

subject to RFC 0011 outcome vocabulary.

## Budget exhaustion

A result such as:

```text
BoundedExecutionResult {
    status: BUDGET_EXHAUSTED
    metric: SemanticSteps
    consumed: 100000
}
```

supports only a bounded noncompletion observation.

It must not produce:

```text
Diverges(subject)
```

without separate evidence.

---

# Part XVIII — fuel semantics

## Fuel

A reference evaluator may consume explicit fuel:

```text
execute(subject, fuel = N)
```

Fuel guarantees the evaluator itself has finite authority.

## Result

```text
FuelResult {
    terminal_outcome?
    fuel_consumed
    fuel_remaining
    trace_prefix
}
```

## Semantic caution

Fuel is an operational guard.

It does not need to correspond exactly to:

- source-level complexity;
- machine instructions;
- physical time;
- proof-theoretic reduction count;
- asymptotic resource cost.

The metric must remain named.

## Existing Luna integration

The existing bounded body/SSA evaluators already provide a useful experimental foundation. RFC 0022 should refine their semantics so `budget_exhausted` becomes explicitly non-theorem evidence.

---

# Part XIX — step-indexed reasoning

## Meta-level step indices

Step-indexed logical relations may assign a finite natural index to recursive semantic claims.

The index bounds how many semantic steps a proof relation promises to observe, helping define otherwise circular recursive relations.

## Separation from execution fuel

A step index may be:

- proof-internal;
- erased;
- unrelated to actual runtime evaluation strategy.

Therefore:

```text
ProofStepIndex != RuntimeFuel
```

unless an explicit relation establishes equivalence.

## RFC 0020 interaction

Step-indexing may be a proof technique for recursive contextual/refinement relations without changing those relation identities.

---

# Part XX — termination-sensitive observations

## Termination as an observable

RFC 0020 equivalence/refinement claims must be able to include or exclude termination/divergence as an observation dimension.

Candidate distinction:

```text
TerminationSensitive
TerminationInsensitive
```

## Example

Two programs with the same returned value whenever both terminate may satisfy a termination-insensitive relation while failing a termination-sensitive relation if one can diverge.

## Security significance

RFC 0024 information-flow work may care whether secret-dependent termination leaks information.

RFC 0022 therefore exposes termination observation explicitly rather than hiding it inside the evaluator.

---

# Part XXI — proof-relevant termination claims

## Relation claim structure

Following RFC 0020, completion/progress claims should be proof-relevant artifacts rather than booleans.

Conceptually:

```text
TerminationClaim {
    id
    subject
    mode
    quantified_input_domain
    execution_model
    observation_model
    environment_scope
    target_scope
    assumptions
    witness
    obligations
    evidence
}
```

## Witness kinds

Candidate witness kinds include:

```text
StructuralDescent
WellFoundedMeasure
AccessibilityProof
RankingFunction
LexicographicRanking
MultisetRanking
SizeChangeCertificate
FiniteStateExhaustiveProof
GuardedProductivity
CoinductiveDivergenceProof
RankingSupermartingale
ExternalKernelProof
ExternalVerifierCertificate
```

## Untrusted producers

Forge, LLMs, SMT solvers, abstract interpreters, termination analyzers, and specialized micro-verifiers may generate witnesses.

Trusted authority depends on the checker/evidence path, not the producer.

---

# Part XXII — dependency and invalidation

## Dependency graph

Termination evidence should record dependencies on:

- callees;
- recursive SCC members;
- measures;
- datatype definitions;
- proof lemmas;
- target/runtime facts where relevant;
- scheduler/fairness assumptions;
- foreign functions;
- unsafe boundaries;
- verifier version;
- canonical semantic identity.

## Local invalidation

If `A` depends on `B` termination and `B` changes, only support paths depending on that version of `B` should become stale.

RFC 0018's alternative justification paths remain applicable.

## Mutually recursive SCCs

Mutually recursive functions may share one joint termination witness.

A candidate object:

```text
TerminationSccWitness {
    members
    joint_measure
    call_relations
    proof_or_certificate
}
```

---

# Part XXIII — termination requirements as contracts

A function/component may declare:

```text
TerminationRequirement {
    mode: MustTerminate
    input_domain: ...
}
```

A stream may instead declare:

```text
ProductivityRequirement {
    observation: NextElement
}
```

A concurrent operation may declare:

```text
ProgressRequirement {
    condition: LockFree
}
```

A service may declare:

```text
LivenessRequirement {
    property: request LeadsTo terminal_response
    assumptions: WeakFairness(worker_scheduler)
}
```

These requirements constrain realization search.

---

# Part XXIV — quantified domains and preconditions

## Domain scope

Termination claims must preserve the exact input domain.

Examples:

```text
MustTerminate(parser, all finite ByteString)
```

```text
MustTerminate(search, inputs satisfying finite_graph && reachable_goal)
```

```text
BoundedTerminate(f, corpus = C, budget = B)
```

These are different claims.

## Preconditions

A termination proof may rely on preconditions:

```text
n >= 0
acyclic(graph)
finite(input)
well_formed(tree)
```

Those preconditions become evidence dependencies, not comments.

## Dynamic checking

If a precondition is not statically established, a runtime check may create a narrower path where the termination theorem becomes applicable.

This follows RFC 0006 machine-intent and RFC 0018 assurance patterns.

---

# Part XXV — terminal outcome classes

## Generic termination

`Terminates` should mean that execution reaches a terminal state under the declared semantics.

## Stronger claims

Separate properties may include:

```text
TerminatesSuccessfully
TerminatesWithDeclaredOutcome
TerminatesWithoutFatalOutcome
TerminatesWithoutCancellation
```

## Why separate them

A program that always traps after one instruction unquestionably terminates, but likely fails its functional contract.

Conflating termination with success would make proofs unsound and diagnostics misleading.

---

# Part XXVI — transformation and optimization semantics

## Termination preservation

A transformation may claim:

```text
PreservesTerminationSensitiveBehavior
```

or a directional relation such as:

```text
candidate contextually refines baseline
under termination-sensitive observations
```

## Removing divergence

A candidate may remove an allowed divergent branch and thereby refine a nondeterministic specification without being equivalent to it.

RFC 0020's directional refinement is the correct relation.

## Introducing divergence

A transformation that introduces possible divergence where the protected contract requires must-termination violates the protected relation even if all finite tests pass.

## Inlining and specialization

Inlining, specialization, unrolling, and defunctionalization may change the syntactic form of a termination witness.

Forge should preserve/reconstruct the semantic claim rather than depend on source syntax surviving unchanged.

---

# Part XXVII — Forge realization/search semantics

RFC 0022 substantially expands what Forge can ask of a candidate.

Example requirement:

```text
operation SearchKernel {
    required:
        MustTerminate(all finite valid inputs)
        no fatal terminal outcome

    preference:
        minimize latency

    experiment_budget:
        100000 semantic steps per candidate
}
```

Candidate A:

```text
termination witness:
    StructuralDescent
performance:
    acceptable
```

Candidate B:

```text
termination:
    UNRESOLVED
bounded tests:
    1000000 successful cases
performance:
    fastest
```

Candidate C:

```text
termination witness:
    LexicographicRanking
performance:
    fastest among proven candidates
```

RFC 0018 policy may permit B in a sandbox but require A/C for promotion.

Performance never upgrades bounded testing into must-termination.

---

# Part XXVIII — RAVEL and recursive repair

A failed completion/progress authorization should produce a structured diagnosis rather than one generic “verification failed.”

Possible causes include:

```text
missing ranking witness
ranking measure not well founded
decrease obligation unresolved
callee termination stale
fairness assumption unavailable
productivity guard broken
progress class weaker than required
bounded execution exhausted
relation counterexample found
```

RAVEL may then repair:

- the algorithm;
- the termination witness;
- a missing precondition;
- an overly strong progress requirement;
- a fairness/environment mismatch;
- a recursion structure;
- a proof/certificate;
- an operational budget for experimentation.

It must not treat “insufficient evidence for promotion” as equivalent to “program definitely diverges.”

---

# Part XXIX — Fabric and runtime orchestration

Fabric can contribute evidence by:

- running bounded execution across workers;
- varying schedules/interleavings;
- invoking heterogeneous termination/progress verifiers;
- reproducing liveness scenarios;
- checking runtime fairness/environment facts;
- collecting budget-exhaustion traces;
- isolating nonterminating experimental candidates.

Fabric cannot infer universal termination merely from extensive execution.

Independent worker repetition remains empirical evidence unless a formal finite-state/exhaustive argument changes the claim class.

---

# Part XXX — assurance integration

RFC 0018 should appraise completion/progress evidence without changing its semantics.

Possible evidence classes include:

```text
KernelCheckedTerminationProof
CheckableTerminationCertificate
IndependentStaticVerifierResult
FiniteStateExhaustiveResult
BoundedExecutionObservation
StatisticalTerminationObservation
RuntimeFairnessAttestation
LivenessModelCheckResult
```

An assurance profile might allow:

```text
sandbox_execution:
    bounded execution authority even if termination UNRESOLVED
```

while requiring for promotion:

```text
candidate_promotion:
    MustTerminate claim
    AND acceptable proof/certificate evidence
    AND all dependencies fresh
```

for a component whose contract requires universal termination.

A service component with a productivity/liveness contract would use different evidence requirements.

---

# Part XXXI — counterexamples

A termination/progress counterexample should be scoped to the exact claim.

Examples:

```text
MustTerminate counterexample:
    one valid execution/schedule that diverges
```

```text
WaitFree counterexample:
    one operation with unbounded own-step delay under allowed interference
```

```text
Liveness counterexample:
    one fair execution violating the declared leads-to property
```

A deadlock trace does not automatically refute unrelated totality claims about a different execution model.

Counterexamples must retain:

- input/state;
- schedule/interleaving where relevant;
- environment;
- fairness classification;
- trace prefix/cycle;
- subject/version identities.

---

# Part XXXII — diagnostics

Machine-readable diagnostics should distinguish at least:

```text
termination.proven
termination.refuted
termination.unresolved
termination.measure_missing
termination.measure_not_well_founded
termination.decrease_obligation_failed
termination.callee_stale
termination.certificate_invalid

divergence.proven

action.budget_exhausted
bounded_run.completed
bounded_run.noncompletion_observed

productivity.proven
productivity.guard_missing
productivity.counterexample

progress.deadlock_counterexample
progress.starvation_counterexample
progress.requirement_unsatisfied

liveness.counterexample
liveness.fairness_assumption_missing
liveness.environment_inapplicable
```

Diagnostics should state what would change the result when possible.

---

# Part XXXIII — proof-core boundary

RFC 0022 must keep the trusted proof boundary narrow.

A termination analyzer may be complex and untrusted.

It should ideally emit:

- a kernel proof term;
- a small certificate;
- a ranking witness checked by a small verifier;
- a size-change certificate;
- another independently checkable artifact.

The proof kernel should not embed every termination-search algorithm.

This follows RFC 0007's producer/checker separation.

---

# Part XXXIV — machine-native micro-verifiers

RFC 0022 creates a rich family of specialized verifier opportunities.

Possible micro-verifiers include:

```text
StructuralRecursionVerifier
RankingFunctionVerifier
LexicographicRankingVerifier
SizeChangeTerminationVerifier
GuardedProductivityVerifier
FiniteStateLivenessVerifier
ConcurrentProgressVerifier
FairnessApplicabilityVerifier
BudgetTraceVerifier
TerminationSensitiveRelationVerifier
```

Different implementation languages may be appropriate for different verifier families.

The semantic protocol matters more than implementation uniformity.

Each verifier should consume exact subject/scope/dependency identities and emit versioned evidence compatible with RFC 0018.

---

# Part XXXV — interaction with exact-real numeric semantics

RFC 0021 permits mathematical `Real` semantics without claiming all exact-real operations are ordinary terminating Boolean computations.

RFC 0022 therefore provides the completion vocabulary needed for operations such as exact-real comparison or equality where computation may be partial or semidecidable.

A numeric operation may expose:

```text
PartialComputation<Boolean>
```

or a richer result rather than pretending every exact-real predicate must terminate.

This prevents the numeric semantics from making impossible computability promises.

---

# Part XXXVI — interaction with future time semantics

RFC 0022 owns abstract bounded computation:

```text
MustTerminate within N semantic steps
Productive within K transitions
WaitFree within finite own-step bound
```

RFC 0023 will own clock/time semantics:

```text
must complete before deadline D
respond within 5 ms
lease expires at timestamp T
```

A semantic-step bound may support a deadline proof under separately evidenced target/runtime cost bounds, but the two are not identical.

---

# Part XXXVII — interaction with future quantitative semantics

RFC 0032 should own:

```text
O(n log n)
exact step-count functions
memory cost
energy cost
throughput
latency distributions
expected cost
QoS budgets
```

RFC 0022 owns whether the computation/progress obligation is finite/eventual at all.

A termination witness may carry a quantitative measure that RFC 0032 later enriches.

---

# Part XXXVIII — interaction with future distributed semantics

RFC 0028 will own distribution-specific semantics such as:

- partial failure;
- replication;
- consistency;
- messaging;
- consensus;
- failure detectors;
- distributed liveness.

RFC 0022 provides reusable generic concepts:

```text
Eventually
LeadsTo
FairnessAssumption
Quiescence
Stabilization
ProgressCondition
```

Distributed RFCs should refine these rather than invent incompatible terminology.

---

# Part XXXIX — interaction with information-flow/security semantics

RFC 0024 may distinguish termination-sensitive from termination-insensitive noninterference.

RFC 0022 must therefore expose:

- whether termination/divergence is observable;
- whether liveness timing is observable;
- whether secret-dependent nontermination exists;
- which fairness assumptions affect the observation.

The security RFC owns information-flow policy; RFC 0022 owns the completion/progress facts it consumes.

---

# Part XL — transformation deltas

Forge should be able to compare baseline and candidate completion/progress properties explicitly.

A candidate artifact may contain:

```text
TerminationDelta {
    preserved
    strengthened
    weakened
    added
    removed
    unresolved
    invalidated
}
```

Examples:

```text
baseline: MayTerminate
candidate: MustTerminate
=> strengthened under same scope
```

```text
baseline: WaitFree
candidate: LockFree
=> potential weakening; not substitutable if WaitFree protected
```

```text
baseline: Productive
candidate: termination UNRESOLVED but productivity preserved
=> acceptable if termination was not required
```

RFC 0020 defines the relation semantics; RFC 0022 provides the property vocabulary.

---

# Part XLI — no universal completion ladder

The following must not be treated as one scalar assurance ladder:

```text
Bounded execution success
Structural termination proof
Wait-free progress proof
Productivity proof
Liveness model-check result
Almost-sure termination proof
Divergence proof
```

They establish different propositions.

A weaker-risk action may accept bounded evidence where a stronger-risk action demands proof, but the evidence classes remain distinct.

---

# Part XLII — first bounded implementation experiment

The first RFC 0022 implementation should be deliberately small and executable.

## Required artifacts

Introduce versioned research artifacts for at least:

```text
TerminationClaim
TerminationMode
WellFoundedMeasure
RankingWitness
ComputationBudget
BoundedExecutionResult
ProductivityClaim
FairnessAssumption
```

Optional but strongly useful:

```text
SizeChangeGraph
ProgressClaim
LivenessClaim
CompletionCounterexample
```

## Required fixture 1 — structural recursion

Implement a structurally recursive function over a finite inductive value.

Expected result:

```text
MustTerminate = SUPPORTED
witness = StructuralDescent
```

## Required fixture 2 — ranking loop

Implement a simple loop whose integer/lexicographic measure provably decreases.

Expected result:

```text
MustTerminate = SUPPORTED
witness = RankingFunction
```

## Required fixture 3 — intentional divergence under fuel

Implement an intentional infinite computation.

Run it with finite fuel.

Expected result:

```text
BoundedExecutionResult.status = BUDGET_EXHAUSTED
TerminationClaim = UNRESOLVED
```

unless a separate divergence verifier proves divergence.

The fixture must explicitly reject the inference:

```text
BUDGET_EXHAUSTED => DIVERGES
```

## Required fixture 4 — budget sensitivity

Use a finite computation that needs more than a small budget but less than a larger budget.

Example:

```text
budget 100  -> BUDGET_EXHAUSTED
budget 1000 -> RETURNED
```

This fixture demonstrates why budget exhaustion is not nontermination.

## Required fixture 5 — productive nontermination

Implement a simple guarded stream/event producer.

Expected result:

```text
MustTerminate = not required
Productive = SUPPORTED
```

The system must not report failed termination as the primary defect.

## Required fixture 6 — may versus must termination

Implement a bounded nondeterministic model with one terminating branch and one nonterminating/continuing branch.

Expected relation:

```text
MayTerminate = SUPPORTED
MustTerminate = REFUTED or UNRESOLVED depending on finite model/witness
```

For the first pilot, use a model where the distinction is mechanically demonstrable.

## Required fixture 7 — fairness-dependent liveness

Model a small worker/request system where request completion depends on an explicit fairness assumption.

Expected results:

```text
without fairness fact:
    LivenessClaim = UNSUPPORTED/UNRESOLVED

with applicable fairness assumption:
    LivenessClaim = eligible for proof/model-check evidence
```

## Required fixture 8 — progress-class distinction

Where practical, model one concurrent operation that satisfies a weaker progress property but not a stronger one.

This may initially be a finite abstract transition model rather than production lock-free code.

## RFC 0018 assurance profiles

Define at least two action profiles:

```text
experimental_sandbox
candidate_promotion
```

Sandbox policy may authorize an unresolved-termination candidate only under finite execution authority.

Promotion policy for a component declaring `MustTerminate` should require stronger current evidence.

## RFC 0020 relation integration

At least one fixture should produce a relation claim showing that two candidates are equivalent/refining under one observation model but differ under termination-sensitive observation.

## Non-goals of the pilot

The first implementation does not need:

- a complete automated termination prover;
- full temporal logic;
- production lock-free algorithm verification;
- arbitrary probabilistic termination;
- complete sized-type inference;
- distributed liveness;
- asymptotic complexity proofs;
- physical real-time deadlines;
- proof of the entire compiler's termination.

---

# Part XLIII — candidate serialization sketch

A research `TerminationClaim` might conceptually resemble:

```json
{
  "schema": "mncs.termination-claim/0.1",
  "subject": "semantic:function:...",
  "mode": "must_terminate",
  "input_domain": "property:finite-valid-input",
  "execution_model": "deterministic",
  "assumptions": [],
  "witness": {
    "kind": "ranking_function",
    "artifact": "artifact:ranking:..."
  },
  "evidence": ["evidence:..."]
}
```

A bounded run might resemble:

```json
{
  "schema": "mncs.bounded-execution/0.1",
  "subject": "candidate:...",
  "budget": {
    "metric": "semantic_steps",
    "limit": 100000
  },
  "status": "budget_exhausted",
  "consumed": 100000,
  "trace_prefix": "artifact:trace:...",
  "termination_conclusion": "unresolved"
}
```

These are conceptual shapes only; implementation may use existing model conventions.

---

# Part XLIV — open research questions

RFC 0022 intentionally leaves several questions open for experimentation.

## How much termination belongs in the canonical type system?

Possible options include:

- separate contract metadata;
- effect-like total/partial strata;
- indexed computation types;
- proof obligations attached to recursive constructs;
- combinations of the above.

The first implementation should avoid prematurely forcing all termination semantics into surface type syntax.

## Which well-founded relations belong in the kernel?

The kernel should likely support a small general well-founded proof principle rather than many specialized termination algorithms.

## Which certificates provide the best independent-checker economics?

Candidates include:

- ranking certificates;
- size-change certificates;
- transition invariants;
- proof terms;
- finite-state model-check certificates.

Forge experiments should measure proof/certificate size, checker complexity, synthesis success, invalidation locality, and model performance.

## How should productivity interact with effects and asynchronous computation?

A producer may make progress through external effects, backpressure, cancellation, or demand.

RFC 0008/0011 semantics must constrain what counts as a valid productivity observation.

## How should fairness be represented?

The final system may use:

- explicit named assumptions;
- runtime capability/fact objects;
- temporal formulas;
- scheduler contracts;
- target/environment attestations.

The key requirement is that fairness never remain ambient.

## How should partial exact-real computations surface?

RFC 0021 + RFC 0022 should permit exact semantics without pretending undecidable predicates are total Booleans.

The best human-facing design remains open.

## How should termination proofs survive aggressive transformations?

Possible directions include:

- proof transport;
- relation-based preservation;
- witness regeneration;
- small pass-specific simulation proofs;
- independent post-lowering verification.

RFC 0020 provides the relation framework; experiments should determine the practical mix.

---

# Part XLV — non-goals

RFC 0022 does not attempt to define:

- one universal termination checker;
- one universal fairness model;
- one universal scheduler;
- a total-only programming language;
- a full temporal logic surface syntax;
- asymptotic complexity semantics;
- physical time/deadline semantics;
- distributed consensus liveness;
- probabilistic-program semantics in full;
- a proof that arbitrary general recursive code terminates;
- a universal progress hierarchy across all concurrency models;
- a replacement for RFC 0010 concurrency semantics;
- a replacement for RFC 0011 failure/nondeterminism semantics;
- a replacement for RFC 0018 assurance;
- a replacement for RFC 0020 relation semantics.

---

# Part XLVI — relationship to other RFCs

## RFC 0004 — recursive refinement

RFC 0022 gives recursive refinement explicit finite authority and completion/progress claims. Refinement search budgets are operational guards, not proofs that candidate-generation procedures terminate universally.

## RFC 0007 — proof-carrying dependent core

RFC 0007 owns trusted proof terms and normalization rules. RFC 0022 supplies the termination/productivity propositions and witness families that proof terms/certificates may establish.

## RFC 0008 — effects/resources/events

Productivity and liveness observations involving I/O/events must respect explicit effects and capabilities.

## RFC 0010 — concurrency

RFC 0010 defines the concurrent execution model; RFC 0022 classifies progress properties over it.

## RFC 0011 — failure/nondeterminism

May/must/almost-sure termination quantify over RFC 0011 nondeterminism/failure models. A terminal failure remains distinct from divergence.

## RFC 0012 — executable semantic core

RFC 0012 supplies state-transition execution. RFC 0022 classifies terminality, continuing behavior, bounded execution, and productivity over those transitions.

## RFC 0017 — runtime/environment

Fairness and liveness applicability may depend on scheduler/runtime/target facts from RFC 0017.

## RFC 0018 — assurance

RFC 0018 decides whether available completion/progress evidence authorizes a requested action. It does not change the claim type.

## RFC 0019 — value/data semantics

Inductive data supports structural/well-founded termination; coinductive observation supports productivity.

## RFC 0020 — relation semantics

Termination-sensitive/insensitive equivalence, refinement, and progress-preservation claims use RFC 0020 relation objects.

## RFC 0021 — numeric semantics

Exact-real and numerical-search computations may be partial, bounded, or progressively refined. RFC 0022 supplies their completion vocabulary.

## RFC 0023 — time

RFC 0023 will distinguish physical/logical clocks and deadlines from abstract computation budgets.

## RFC 0024 — information flow

Security may observe or abstract termination differences. RFC 0022 exposes the relevant completion facts.

## RFC 0028 — distribution

Distributed liveness, failure detectors, and consensus will refine the generic liveness/fairness vocabulary.

## RFC 0032 — quantitative cost

Complexity/resource bounds enrich RFC 0022 finite-progress claims without replacing them.

---

# Part XLVII — recommended theory synthesis

RFC 0022 should explicitly draw from, without mechanically copying, the following theoretical families:

1. **Total functional programming** — separating total computation, partiality, finite data, and productive codata.
2. **Structural recursion** — local machine-checkable descent on inductive structure.
3. **Well-founded recursion and accessibility** — the general logical foundation for total recursive definitions.
4. **Ranking functions** — imperative/state-machine termination via well-founded state measures.
5. **Lexicographic and multicomponent ranking** — structured descent where no single scalar decreases every step.
6. **Size-change termination** — compositional termination inference/certificates from local parameter-change graphs.
7. **Sized types** — machine-visible size/depth indices that can support recursion/productivity checking.
8. **Weak and strong normalization** — especially for proof-kernel reduction semantics.
9. **Coinductive partiality** — explicit modeling of general recursive computation and divergence.
10. **Guarded recursion / later modalities** — productivity of infinite/coinductive structures.
11. **Progress/preservation theory** — local non-stuck execution, kept distinct from liveness.
12. **Concurrent progress theory** — obstruction-free, lock-free, wait-free, deadlock/starvation distinctions.
13. **Temporal logic** — eventuality, leads-to, recurring progress, stabilization.
14. **Fairness theory** — explicit weak/strong fairness dependencies for liveness.
15. **Ranking supermartingales** — future almost-sure termination for probabilistic computation.
16. **Step-indexed logical reasoning** — finite semantic approximations for recursive relations.
17. **Fuel/bounded operational semantics** — safe finite execution authority for unresolved/untrusted computation.
18. **Quiescence and stabilization** — completion-like properties for persistent/open systems.

No single theory is sufficient. The machine-native contribution is the **typed coexistence and explicit scoping** of their different claims.

---

# Part XLVIII — architectural consequence

Once RFC 0022 exists, Forge should no longer ask only:

```text
Does this program terminate?
```

It can instead carry a structured answer such as:

```text
subject: candidate X

MustTerminate:
    SUPPORTED
    scope: all finite valid inputs
    witness: lexicographic ranking R

Productive:
    not applicable

ConcurrentProgress:
    LockFree supported
    WaitFree unresolved

Liveness:
    request LeadsTo response
    supported only under WeakFairness(worker_scheduler)

BoundedExecution:
    100000-step experimental runs completed on corpus C
    empirical evidence only

Assurance:
    candidate promotion authorized for action A
    because action A requires MustTerminate + LockFree,
    not WaitFree
```

Another subject may validly say:

```text
MustTerminate:
    not required

Productive:
    SUPPORTED
    observation: next stream item

Liveness:
    AlwaysEventually(item)

Shutdown:
    MayTerminate on cancellation
```

And an unresolved candidate may say:

```text
MustTerminate:
    UNRESOLVED

BoundedExecution:
    BUDGET_EXHAUSTED at 100000 semantic steps

Diverges:
    UNSUPPORTED

Authorization:
    sandbox only
```

That is the intended machine-native direction:

> **Completion behavior becomes explicit semantic information, with proof-relevant witnesses, scoped assumptions, bounded operational authority, and action-relative assurance, rather than an accidental inference from whatever execution happened to do.**
