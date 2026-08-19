# RFC 0012: Machine-Native Executable Semantic Core

- **Status:** Draft
- **Target:** Cross-cutting 0.3–1.0
- **Depends on:** RFC 0001, RFC 0002, RFC 0003, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011

## Summary

MNCS Language should define executable meaning through a **small, typed, modular transition system over an explicit semantic machine**. The language should not allow its reference interpreter, HIR, SSA, verifier interfaces, runtime, or backend to acquire separate informal meanings of execution.

The central rule is:

> **MNCS execution is a sequence or coinductive behavior of modular semantic transitions. Values and computations are distinct; effectful computations interact with semantic subsystems through typed events; observable behavior is projected from those transitions; and lowerings are valid only through evidence-bearing refinement of that behavior.**

RFC 0012 therefore establishes the direction for the executable semantic core that binds together the previously separated foundations for:

- contracts and evidence;
- machine intent and lowering envelopes;
- proof-carrying semantics;
- effects, authority, resources, and I/O;
- memory, provenance, and storage;
- concurrency, causality, and visibility;
- failure, recovery, nondeterminism, and external observations; and
- HIR/SSA/reference-execution artifacts.

The proposal synthesizes a deliberately small set of complementary theories:

- **modular structural operational semantics (MSOS)** and labeled small-step semantics for the normative transition relation;
- **CEK/CESK-style abstract machines** for explicit control, environments, continuations, and store-like state;
- **Call-by-Push-Value (CBPV)** as the primary inspiration for separating values from computations without selecting one accidental evaluation strategy as the language ontology;
- **algebraic effects and typed semantic requests** for the boundary between local computation and global semantic subsystems;
- **interaction-tree/coinductive execution models** for recursive, effectful, potentially nonterminating computations;
- **trace semantics** for observable behavior distinct from internal machine steps;
- **simulation and refinement** as the correctness relationship between semantic, HIR, SSA, target-IR, and machine realizations; and
- **abstract interpretation of the executable machine** as a preferred route for deriving conservative analyses and micro-verifiers.

Rewriting-logic/K-style semantics and denotational/monadic models should be investigated as independent validation and executable cross-checks, not selected as the sole normative definition.

The result should be executable enough to serve as a reference machine, formal enough to support proof, modular enough to absorb later language features without rewriting unrelated rules, and explicit enough that machines can inspect and transform the semantics directly.

## Motivation

MNCS already has a growing executable model and multiple representations:

```text
semantic graph
    -> semantic bodies
    -> HIR
    -> SSA
    -> bounded reference evaluators
    -> future backend realizations
```

Those artifacts are useful, but without one normative executable semantics they can drift.

A body evaluator can accidentally interpret failure differently from HIR. SSA can preserve integer arithmetic while losing an authority transition. A backend can emit a memory operation that appears locally equivalent but changes allowed observations under concurrency. A verifier can prove a property about an approximation that is subtly different from the reference interpreter's behavior.

A machine-native language should not rely on convention to keep these layers aligned.

The language needs one formal answer to questions such as:

- What is a value?
- What is a computation?
- What does function application mean?
- What is the state of an executing computation?
- What does recursion mean when execution may diverge?
- What constitutes a semantic step?
- Which state dimensions may a step observe or modify?
- When is evaluation order required, permitted, or irrelevant?
- How are memory operations connected to RFC 0009 provenance and authority?
- How are I/O operations connected to RFC 0008 typed effects and events?
- How are task creation and observation connected to RFC 0010 concurrency semantics?
- How do timeout, cancellation, ambiguous commit, and recovery alter execution state under RFC 0011?
- Which transitions are internal and which are externally observable?
- What behavior must HIR, SSA, and machine code preserve?
- When is a lowering equivalent, and when is refinement sufficient?
- How can static analyses be shown to conservatively approximate the same machine that executes programs?

The semantic core must answer these questions without making one operating system, scheduler, allocator, backend, physical stack, or source representation fundamental.

## Design goals

RFC 0012 should make the following possible.

1. **One executable meaning.** All executable representations should relate to one normative semantic machine rather than maintaining independent informal semantics.
2. **Modular rules.** Adding a semantic subsystem must not require rewriting unrelated evaluation rules.
3. **Explicit state.** Control, environments, continuations, memory, authority, resources, event context, external observations, and knowledge state must be representable as semantic objects when relevant.
4. **Value/computation separation.** Pure values and effectful computations should be distinguishable in the semantic model.
5. **Typed semantic interaction.** Memory, I/O, concurrency, failure, time, entropy, foreign behavior, and other effectful interactions should enter the machine through typed operations/events rather than hidden host-language effects.
6. **No accidental evaluation commitments.** Source sequence, host-language evaluation order, physical stack behavior, or one runtime strategy must not become semantics unless explicitly required.
7. **Finite and infinite behavior.** Termination, divergence, long-lived services, recursive event streams, and reactive computations must belong to the same execution model.
8. **Observable/internal separation.** Internal machine reductions should be distinguishable from externally observable behavior.
9. **Refinement-oriented lowering.** Compiler stages should prove or otherwise establish simulation/refinement relationships against prior stages.
10. **Explicit permitted nondeterminism.** Multiple legal transitions or observations should be represented as semantic possibility, not collapsed into implementation accident.
11. **Static-analysis derivability.** Where practical, analyzers and micro-verifiers should arise as conservative abstractions of the normative machine.
12. **Proof integration without kernel inflation.** The executable machine may generate obligations and consume evidence while keeping RFC 0007's trusted proof kernel small.
13. **Deterministic artifacts.** Semantic configurations, requests, traces, transition identities, and evidence bindings should have canonical, versioned representations where persisted.
14. **Independent implementation.** The semantics should be precise enough for multiple independent interpreters/checkers to agree on supported programs.
15. **Machine inspectability.** Machines should be able to query why a transition is legal, which semantic dimensions it touches, which obligations it generates, and which later transformations depend on it.

## Non-goals

This RFC does not:

- select final human-readable surface syntax;
- require one concrete runtime architecture;
- require the semantic machine to be the production execution engine;
- require all programs to terminate;
- make the physical call stack part of language semantics;
- require one universal scheduler, allocator, garbage collector, or I/O reactor;
- replace RFC 0007's proof core with an executable evaluator;
- place operating-system, allocator, hardware, or backend correctness inside the proof kernel;
- require every semantic transition to be represented at production runtime;
- define the complete module, abstraction, FFI, ABI, staging, or package system;
- require every static analysis to be automatically derived from the machine;
- claim universal compiler correctness from bounded differential testing; or
- make a host-language implementation of the semantics authoritative merely because it executes.

Unsupported or unresolved semantics may produce `UNKNOWN`, an explicit unsupported outcome in a research tool, or rejection from the language subset under test. They must not be silently assigned host-language behavior.

## Selected theoretical synthesis

### Modular structural operational semantics

The normative semantics should be based on a labeled small-step transition relation with MSOS-style modularity.

Conceptually:

```text
Configuration
    -- TransitionLabel -->
Configuration'
```

A transition rule should mention only the semantic dimensions it requires.

For example, pure integer addition should not need to mention network authority, task state, the memory store, or external knowledge:

```text
Add(Int(2), Int(3))
    -->
Int(5)
```

A memory load must mention the relevant memory/provenance/authority relationship:

```text
Load(ref)
    -- requires Read(ref.footprint)
       observes Memory(ref.subject, ref.epoch) -->
Value(v)
```

A timeout may update observation and knowledge state without claiming a remote operation failed:

```text
Await(Ack42)
    -- observes DeadlineElapsed(D42) -->
TimedOut(Ack42)
```

with the associated RFC 0011 knowledge state remaining `UNKNOWN` when commit status is unresolved.

The exact formal notation remains open, but rule modularity is a requirement.

### Explicit abstract machine

MNCS should define execution configurations explicitly rather than relying primarily on substitution semantics.

CEK/CESK-family machines provide useful inspiration by making control, environments, continuations, and store-like state explicit.

A conceptual configuration is:

```text
MachineConfiguration {
    control: Control,
    environment: Environment,
    continuation: Continuation,

    memory: MemoryState,
    authority: AuthorityState,
    resources: ResourceState,

    tasks: TaskState,
    event_context: EventContext,

    world: ExternalInteractionState,
    observations: ObservationState,
    knowledge: KnowledgeState,

    assumptions: AssumptionSet,
}
```

This is a semantic decomposition, not a required production struct.

Backends may erase, fuse, encode, or avoid these structures when refinement evidence permits.

### Call-by-Push-Value-inspired value/computation distinction

MNCS should make a fundamental semantic distinction between values and computations.

Conceptually:

```text
Gamma |- v : Value<T>
```

and:

```text
Gamma ; A ; R ; E ; P ; C ; D |- c : Computation<T>
```

where the additional dimensions follow the multidimensional execution/type direction established in prior RFCs.

A value is inert semantic data. A computation may:

- invoke effects;
- consume or produce authority;
- mutate memory through authorized operations;
- allocate resources;
- create or await events;
- spawn structured activity;
- fail, cancel, suspend, or recover;
- observe the external world; or
- diverge.

MNCS should not adopt CBPV wholesale without experiment. The key requirement is that values and effectful computations do not collapse into one category merely for source convenience.

Surface syntax may elaborate convenient expressions into this distinction.

### Algebraic effects and typed semantic requests

Local evaluation should not directly implement every semantic subsystem through hidden host-language operations.

Instead, effectful computation should be able to emit typed semantic requests/events:

```text
Request<ReadMemory<R>>
Request<WriteMemory<R>>
Request<Allocate<Region<R>>>
Request<Receive<Channel<C>>>
Request<Spawn<TaskClass<K>>>
Request<ObserveClock<C>>
Request<ObserveEntropy<S>>
Request<Cancel<Event<E>>>
```

A semantic subsystem interprets the request subject to authority, resource, protocol, state, and evidence constraints.

This separates local computation from world interaction.

It also permits multiple verified realizations of the same semantic operation without changing the operation's identity.

### Interaction-tree and coinductive execution

Recursive effectful programs may not terminate.

The semantic core should therefore support a coinductive view of behavior in addition to finite reduction sequences.

A useful conceptual computation shape is:

```text
Computation<T> =
    Return<T>
  | InternalStep<Computation<T>>
  | Invoke<Event<E>, Continuation<E.Result, T>>
```

The final representation need not literally be an interaction tree, but it must support:

- recursive interactions;
- infinite behavior;
- suspended computations;
- explicit semantic events;
- continuations after events; and
- equivalence/refinement reasoning over such behaviors.

Small-step transition semantics remains normative; coinductive structures provide the required account of infinite behavior.

### Trace semantics

The language should distinguish internal semantic reductions from observable events.

Conceptually:

```text
InternalTransition
ObservableTransition<Event>
```

A program behavior may project to a trace such as:

```text
Behavior =
    Returns(Value)
  | Fails(FailureOutcome)
  | Diverges(TracePrefix*)
  | Produces(ObservableTrace)
```

The exact behavior domain must account for RFC 0010 nondeterminism and RFC 0011 uncertainty rather than assuming one unique trace for every program.

Two implementations may perform radically different numbers of internal steps while remaining observationally equivalent or mutually refining under the declared contract.

### Simulation and refinement

The preferred correctness relation between compiler stages is simulation/refinement, not literal representation equality.

Conceptually:

```text
SemanticCore
    >=refines
HIR
    >=refines
SSA
    >=refines
TargetIR
    >=refines
MachineRealization
```

For each lowering, the implementation should identify:

```text
source_semantics
source_observations
lowered_semantics
target_observations
simulation_relation
allowed_relaxations
assumptions
obligations
evidence
```

Some transformations should require observational equivalence.

Others may legally narrow a permitted nondeterministic result set or choose one realization from a lowering envelope, making refinement the correct relation.

No transformation may produce observations outside the source contract merely because a backend can do so.

### Abstract interpretation of the semantic machine

Where practical, MNCS static analyses should be derived as conservative abstractions of the executable machine.

Examples include abstracting:

```text
exact integer       -> interval / congruence domain
exact provenance    -> provenance set / UNKNOWN
exact footprint     -> region/range abstraction
exact continuation  -> bounded continuation class
exact external fact -> Known / Unknown / Conflicted
exact event graph   -> conservative dependency summary
```

The resulting abstract machine may explore a finite over-approximation of concrete behavior.

This direction is valuable because analyzers, micro-verifiers, Forge diagnostics, and execution semantics then share transition structure instead of independently reconstructing language meaning.

Not every verifier must be derived this way. External theorem provers, model checkers, SMT solvers, domain-specific analyses, and empirical tools remain valid evidence producers under RFC 0002/RFC 0007.

### Independent rewriting-logic model

A K/rewrite-style executable semantics is a useful independent validation target.

The cell-oriented decomposition maps naturally to MNCS state dimensions:

```text
<control> ... </control>
<environment> ... </environment>
<memory> ... </memory>
<authority> ... </authority>
<resources> ... </resources>
<tasks> ... </tasks>
<events> ... </events>
<world> ... </world>
<knowledge> ... </knowledge>
```

MNCS should not depend on K as its normative definition.

Instead, a later experiment should implement an independent rewriting model and differentially compare it with the primary reference machine over a shared conformance corpus.

### Denotational and monadic validation

A secondary denotational projection should be investigated for the pure and effect-compositional subsets.

Its purpose is to provide:

- compositional laws;
- equational reasoning;
- effect laws;
- abstraction checks; and
- an independent way to detect inconsistencies in the operational model.

Operational and denotational models need not cover identical research subsets initially, but any overlapping construct should not receive contradictory meaning without an explicit reason.

## Semantic strata

MNCS should keep at least three semantic strata distinct.

### Logical/proof stratum

RFC 0007's total proof core remains a separate trusted calculus.

It may reason about executable semantics, encode propositions about transitions, or check proof objects generated from execution obligations.

The executable machine itself is not promoted into the trusted proof kernel merely because it is formalized.

### Executable semantic stratum

RFC 0012 defines this stratum.

It contains:

- values;
- computations;
- control state;
- environments;
- continuations;
- typed semantic requests;
- resource/authority interactions;
- memory transitions;
- events and tasks;
- observations and knowledge; and
- failure/recovery transitions.

### Realization stratum

HIR, SSA, backend IR, runtime mechanisms, and machine code are realizations/refinements.

They may contain additional target-specific state and may erase semantic structures whose obligations have been discharged.

Their correctness is relative to the executable semantics and declared lowering/portability envelopes.

## Local machine and world machine

RFC 0012 should investigate a compositional two-layer execution architecture.

### Local machine

The local evaluator is responsible for ordinary computation structure:

```text
values
closures
function application
bindings
pattern matching
algebraic data construction
recursion
continuations
sequencing
local control
```

When it reaches an operation requiring semantic interaction, it emits a typed request.

Conceptually:

```text
LocalMachine
    -> SemanticRequest<E>
```

### World machine

The world machine interprets semantic requests against global semantic state:

```text
memory
provenance
authority
resources
I/O state
protocol state
task/event state
external observations
knowledge
failure/recovery state
```

Conceptually:

```text
WorldMachine(
    SemanticRequest<E>,
    WorldState
)
    ->
WorldResponse<E.Result> + WorldState'
```

The response resumes the local continuation.

This decomposition is attractive because RFCs 0008–0011 can evolve as semantic subsystems without forcing ordinary function-evaluation rules to mention their entire state.

The final formalization may encode the two layers through MSOS labels rather than literally implementing two interpreter objects.

## Values

The initial executable core should support a deliberately small value language.

Candidate value forms include:

```text
Unit
Bool
Integer<Semantics>
Tuple<V...>
Record<Fields...>
Variant<Tag, Payload>
Vector<T, N>
Closure<Environment, FunctionIdentity>
CapabilityRef<C>
ResourceRef<R>
MemoryRef<P>
EventRef<E>
```

Value identity and runtime representation are separate.

RFC 0009 governs memory/reference identity and representation constraints. RFC 0006 governs machine-intent arithmetic/representation semantics.

The initial formal core should avoid adding nominal or object-oriented complexity until the abstraction/module RFCs define their semantics.

## Computations

Candidate computation forms include:

```text
return v
bind x <- c1; c2
apply f args
match v with ...
letrec ...
invoke semantic_operation(args)
handle effect_scope with handler
```

Surface forms may elaborate to these constructs.

The canonical executable calculus should favor a small number of orthogonal forms over convenience syntax.

## Environments and closures

Variable binding should use explicit semantic environments.

A closure conceptually binds:

```text
Closure {
    function_identity,
    captured_environment,
}
```

The environment carries semantic values/capabilities, not physical stack addresses.

Backends may realize closures through static dispatch, register passing, environment objects, specialization, inlining, or elimination.

Closure identity and environment capture must compose with RFC 0009 ownership/provenance rules when captured values include resources or references.

## Continuations

Continuations should be semantic control objects.

They may represent:

```text
return destination
pending bind
pending pattern branch
function return
handler boundary
suspension/resumption point
failure propagation boundary
structured task join
```

A continuation is not a requirement to allocate a heap object or retain a physical call stack.

This abstraction gives a uniform semantic foundation for:

- tail calls;
- suspension;
- async/await lowering;
- algebraic effect handlers;
- cancellation propagation;
- resumable computations; and
- stack-eliding optimization.

## Function application

Function application should transition through semantic closure/environment binding rather than host-language calls.

Conceptually:

```text
Apply(Closure<F, Env>, args, K)
    ->
Execute(
    body = F.body,
    environment = bind(Env, F.params, args),
    continuation = K
)
```

Capability/resource arguments must obey their usage discipline during binding.

Application may generate obligations when dependent preconditions, effect ceilings, authority requirements, or protocol states are not already established.

## Recursion, divergence, and partiality

The executable stratum is not required to be total.

Recursive definitions may:

- terminate;
- diverge internally;
- produce infinite observable traces;
- block awaiting an event;
- be cancelled;
- fail; or
- transition into recovery.

The proof stratum must not assume executable termination unless termination evidence is available.

A bounded reference evaluator may report `BUDGET_EXHAUSTED`, but budget exhaustion is an evaluator outcome, not semantic proof of program divergence.

This distinction must be explicit.

## Algebraic data and pattern matching

Algebraic data should initially have explicit constructor/tag identity.

Pattern matching should be defined over logical value structure, not physical layout.

A match transition should either:

- select a uniquely legal branch;
- reject an invalid/non-exhaustive program under the selected language rules; or
- emit an explicit failure/obligation if dynamic checking is intentionally permitted.

Backend tag representation, niche optimization, jump-table lowering, and branch masking remain realizations under RFC 0006 and later representation/ABI work.

## Evaluation dependencies and ordering

RFC 0012 should not define all textual sequencing as mandatory semantic ordering.

A computation form may impose data/control dependence, while RFC 0010 determines when independent computations may proceed concurrently or be reordered.

For pure subcomputations, evaluation strategy may be flexible if observable behavior and resource semantics are unchanged.

For effectful operations, order must follow explicit dependency, effect, authority, resource, protocol, visibility, atomicity, and failure semantics.

The language should distinguish:

```text
presentation/source order
local evaluation dependency
semantic event order
observable order
```

These are not automatically identical.

## Typed semantic requests

A semantic request should carry enough structure for a subsystem to validate and execute the interaction.

Conceptually:

```text
SemanticRequest<E> {
    identity,
    operation,
    arguments,

    required_authority,
    resource_uses,
    effects,

    preconditions,
    protocol_state,

    event_dependencies,
    context_requirements,

    obligations,
    evidence_refs,
}
```

A response may contain:

```text
SemanticResponse<E> {
    result,
    produced_resources,
    produced_authority,
    state_transitions,
    observations,
    emitted_events,
    generated_obligations,
}
```

Failure/cancellation/uncertainty responses must preserve RFC 0011 outcome semantics rather than collapsing into one generic exception.

## Transition labels

Transition labels should expose semantically relevant deltas without requiring every rule to carry the full machine state.

Candidate dimensions include:

```text
control_delta
environment_delta
memory_delta
authority_delta
resource_delta
effect_delta
event_delta
task_delta
observation_delta
knowledge_delta
failure_delta
assumption_delta
obligation_delta
```

The final calculus should avoid redundant duplication where one relation can be derived from another.

Transition labels should be canonically serializable for research tooling when persisted, but the semantics should not require every production execution to materialize them.

## Internal and observable transitions

Every transition should have an observation class.

At minimum:

```text
Internal
Observable<EventClass>
```

Examples of potentially observable events include:

```text
external I/O
public protocol messages
foreign calls where the boundary is observable
process exit
returned public result
persistent state commitment
explicit timing observation when timing is semantic
```

Memory implementation details, continuation manipulation, internal reductions, and optimization bookkeeping are ordinarily internal unless a contract explicitly exposes them.

RFC 0006 constant-time or side-channel requirements may make otherwise internal timing/cache behavior relevant to an observation model.

## Behavior domain

A program should denote a set or structured family of permitted behaviors rather than always one deterministic result.

A conceptual behavior domain includes:

```text
Return(value, trace)
Failure(outcome, trace)
Divergence(trace_prefixes)
Suspended(waiting_on, trace)
Cancelled(outcome, trace)
```

RFC 0010 may permit multiple schedules/observations.

RFC 0011 may permit multiple external outcomes or unresolved knowledge states.

The semantics must therefore preserve permitted nondeterminism explicitly.

`UNKNOWN` evidence does not enlarge program semantics. It restricts what implementations/verifiers may claim about those semantics.

## Memory integration

Memory requests delegate to RFC 0009 semantics.

A load must establish at least the relevant form of:

```text
live allocation epoch
valid provenance
in-bounds footprint
read authority
initialization/validity
legal visibility under RFC 0010
```

A write additionally requires compatible write authority and must produce an updated memory/resource state.

The executable core must not turn a numeric address into access authority.

Relocation, pinning, initialization, retyping, and deallocation remain explicit semantic transitions governed by RFC 0009.

## I/O and effect integration

RFC 0008 semantic I/O operations become typed requests/events in the executable machine.

The core evaluator does not define `read`, `write`, DMA, polling, `io_uring`, or system calls as primitive mechanisms.

It invokes semantic operations whose realizations may use those mechanisms.

In-flight resources, completion events, cancellation states, and protocol transitions must survive through the machine state.

## Concurrency integration

RFC 0010 defines event independence, causality, visibility, atomicity, interference, progress, and task lifetime.

RFC 0012 supplies the executable transition substrate in which those events occur.

A scheduler realization may choose any enabled event consistent with the concurrency graph and policy.

The normative semantics should represent enabled transition sets/partial orders rather than hide scheduler choice inside a single deterministic host-language loop.

A deterministic reference evaluator may choose a canonical schedule for reproducibility, but it must not imply that the language permits only that schedule.

## Failure and recovery integration

RFC 0011 outcomes become ordinary machine states/transitions.

A failure transition must preserve:

```text
completed external effects
partial effects
uncertain effects
resource state
authority state
observation state
knowledge state
retry/recovery authority
```

Timeout does not imply remote failure.

Compensation does not erase historical effects.

Checkpoint restoration does not rewind the external world unless the relevant external state is part of a proven rollback/transaction contract.

The executable machine should make these facts available to handlers and recovery logic.

## Nondeterminism

The transition system must distinguish at least:

```text
implementation choice
environment choice
scheduler choice
probabilistic choice with model
external uncertainty
```

Different kinds of choice have different proof obligations.

An implementation may select one legal refinement.

A safety property under environment choice generally must hold for all permitted environment outcomes.

A probabilistic claim requires an explicit probability model/evidence basis under RFC 0011.

The reference evaluator must not resolve semantic nondeterminism and then accidentally present its chosen path as the only legal behavior.

## Algebraic effect handlers

Handlers are a promising control abstraction for typed effects, but the RFC does not require a specific surface syntax.

A handler may:

- interpret an abstract effect;
- transform a request into another semantic operation;
- resume a continuation;
- terminate a computation;
- establish a recovery boundary; or
- delegate an effect outward.

Handlers cannot forge authority or turn uncertain external state into known state.

Handler installation and resumption must obey resource usage and continuation lifetime rules.

## Obligations and evidence

Transitions may generate obligations.

Examples include:

```text
prove index in bounds
prove capability authorizes operation
prove effect below declared ceiling
prove pattern exhaustiveness
prove refinement preserves observation set
prove selected memory lowering preserves provenance requirement
prove handler does not duplicate linear continuation/resource
```

The executable machine may carry obligation identities and evidence references.

It does not itself decide that empirical evidence is a formal proof.

RFC 0002 and RFC 0007 remain authoritative for evidence classes and proof trust.

## Stuck, invalid, unsupported, and unresolved states

The semantics must distinguish these categories.

### Invalid

The program/configuration violates a language rule.

Examples:

```text
use of consumed linear resource
invalid provenance
unmatched closed variant
unauthorized effect
```

### Stuck but semantically meaningful

The computation is awaiting a semantic event or resource.

Examples:

```text
awaiting completion
blocked on a protocol event
suspended child task
```

This is not invalidity.

### Unsupported by a research evaluator

The normative semantics may define a construct that a particular tool has not implemented.

The tool should report:

```text
UNSUPPORTED
```

rather than invent behavior.

### Unresolved evidence

A semantic operation may be representable while a required proof/verification obligation remains `UNKNOWN`.

Promotion or optimized lowering may be refused even if the reference semantics can still describe the operation.

These distinctions must remain machine-readable.

## Bounded reference evaluators

Existing body and SSA evaluators are useful research tools.

RFC 0012 should place them in a clear hierarchy:

```text
Normative transition semantics
        |
        +-- executable reference-machine implementation
        |
        +-- bounded body evaluator
        |
        +-- bounded HIR/SSA evaluator
        |
        +-- independent semantic model(s)
```

A finite step budget is an evaluator control, not language semantics.

`BUDGET_EXHAUSTED` must never be interpreted as proof of divergence.

Differential equality over a finite corpus is evidence, not universal semantic equivalence.

## Refinement relations

The project should define a versioned refinement artifact.

Conceptually:

```text
RefinementClaim {
    source_artifact,
    target_artifact,

    source_semantics_version,
    target_semantics_version,

    observation_model,
    relation,

    permitted_relaxations,
    assumptions,

    obligations,
    evidence,
}
```

Possible relation classes include:

```text
observational_equivalence
forward_simulation
backward_simulation
bisimulation
trace_inclusion
contract_refinement
```

The project should begin with the smallest relation sufficient for the first bounded lowering proofs rather than prematurely requiring one universal equivalence notion.

## HIR relationship

HIR should be a structured executable realization of the semantic core.

Lowering from semantic computation to HIR must preserve:

- value identity where relevant;
- effect identity;
- authority/resource uses;
- normal/error/compensation/fatal paths;
- machine-intent identity;
- obligation identity;
- observation requirements; and
- transformation provenance.

HIR may introduce explicit control-flow blocks or temporaries absent from the semantic source.

Those are implementation structure, not semantic differences.

## SSA relationship

SSA should refine HIR while making def-use/control dependencies explicit.

SSA lowering must preserve the semantic conditions under which reordering, elimination, common-subexpression reuse, alias-based optimization, and backend promises are valid.

Memory/effect/event/resource relationships should not be erased merely because ordinary scalar SSA lacks a place to express them.

RFC 0012 therefore reinforces the need for auxiliary semantic/evidence structure alongside scalar SSA where required.

## Backend relationship

A backend realization is valid only when its observable behavior refines the semantic behavior under the declared target and portability envelope.

Target facts such as:

```text
ABI
integer representation
memory model
atomic capabilities
address spaces
exception model
calling convention
vector features
```

belong to the realization proof/assumption set.

Backend undefined behavior must not silently become MNCS semantics.

If a backend optimization requires a fact, that fact must be established in the current evidence scope before the corresponding promise/attribute is emitted.

## Semantic versioning

The executable semantics itself must be versioned independently from source syntax and backend versions.

Persisted artifacts should identify at least:

```text
semantic_core_version
proof_core_version
HIR_version
SSA_version
observation_model_version
```

Changing a semantic rule may invalidate:

- conformance fixtures;
- proof objects tied to the old relation;
- refinement evidence;
- backend promise evidence;
- cached verifier results; and
- replay/execution identities.

The dependency graph should make that invalidation explicit.

## Determinism of semantic artifacts

Canonical serialization of semantic configurations and transitions should be deterministic when persisted.

However, deterministic serialization is different from deterministic execution.

For nondeterministic semantics, a canonical representation may encode the enabled transition set or transition identity ordering without claiming only one transition is legal.

## Execution identities

The project should investigate stable identities for:

```text
initial_configuration
transition
semantic_event
observable_event
execution_prefix
behavior_trace
refinement_claim
```

For deterministic replay under RFC 0011, an execution identity may additionally bind an observation ledger and realization identity.

These identities can support:

- differential testing;
- causal debugging;
- proof binding;
- replay;
- Forge experiments; and
- incremental invalidation.

## Machine-readable rule metadata

Each normative rule should eventually expose metadata such as:

```text
rule_identity
constructs_consumed
state_dimensions_read
state_dimensions_written
possible_events
possible_failures
obligations_generated
related_rfc_constraints
```

This is a machine-native feature, not just documentation.

Forge or other tooling could ask:

```text
Which rules can alter authority?
Which transitions can create external observations?
Which rules may suspend?
Which rules depend on memory provenance?
```

without parsing prose.

The metadata must be derived from or checked against the formal rules rather than becoming an unsynchronized second specification.

## Static analyses as semantic abstractions

A first abstract-machine analysis experiment should select one small domain.

Candidate:

```text
concrete integer -> interval
concrete branch -> possible branch set
concrete effect -> effect summary
```

The abstract transition relation must over-approximate concrete transitions.

If the analysis says an outcome is impossible, the project should be able to state why that exclusion is sound relative to the concrete machine.

A later memory analysis can abstract RFC 0009 provenance/footprints, and a later concurrency analysis can abstract RFC 0010 event dependencies.

## Diagnostics

A semantic failure should produce diagnostics grounded in transition/rule structure.

Conceptually:

```text
Diagnostic {
    configuration_identity,
    attempted_rule,
    failed_precondition,
    relevant_state_slice,
    obligation,
    evidence_status,
    candidate_repair_classes,
}
```

This should feed directly into RFC 0004 recursive refinement.

The goal is not merely `type mismatch` or `invalid operation`; the machine should identify the semantic relation that prevented the transition.

## Causal slicing

Because transitions explicitly state which semantic dimensions they consume and produce, execution histories should support causal slicing.

Given a failed obligation or unexpected observation, tooling may trace backward through:

```text
value dependencies
resource transitions
authority transitions
memory provenance
event dependencies
observations
knowledge updates
assumptions
```

This can improve Forge repair localization and reduce evidence invalidation radius.

The first implementation may remain conservative rather than formally minimal.

## Security consequences

An explicit semantic machine strengthens security reasoning.

Authority checks are semantic transitions, not optional runtime conventions.

A component without a capability cannot perform the corresponding semantic operation even if a backend/runtime object accidentally contains a broader handle.

Observable traces can support confinement claims such as:

```text
no filesystem observations
no network observations
no authority amplification
```

provided the relevant semantics/evidence are complete.

A foreign or unsafe boundary introduced later must explicitly state which parts of this model become assumptions.

## Performance consequences

The semantic machine is not intended to execute in production at one-transition-per-runtime-step fidelity.

Its purpose is to expose optimization freedom safely.

Examples include:

- pure reductions may collapse;
- continuations may inline or disappear;
- independent computations may parallelize;
- effect handlers may specialize;
- memory operations may choose different storage realizations;
- event operations may choose polling or interrupt-based mechanisms;
- task structures may map to threads, coroutines, GPU workgroups, or inline execution.

Each optimization remains bounded by observable behavior, authority, resource, failure, and target contracts.

## Reference-rule examples

The following examples are sketches only.

### Return

```text
<control> Return(v) </control>
<continuation> K </continuation>

-->

<control> resume(K, v) </control>
```

### Bind

```text
<control> Bind(x, c1, c2) </control>
<continuation> K </continuation>

-->

<control> c1 </control>
<continuation> BindFrame(x, c2, env) :: K </continuation>
```

### Function application

```text
<control> Apply(Closure(F, Env0), args) </control>
<continuation> K </continuation>

-->

<control> F.body </control>
<environment> bind(Env0, F.params, args) </environment>
<continuation> ReturnFrame(K) </continuation>
```

### Semantic request

```text
<control> Invoke(E, args) </control>

-- emit -->

SemanticRequest<E>(args, authority, resources, obligations)
```

The world subsystem then produces a response or explicit failure/suspension state.

### Await

```text
<control> Await(event) </control>
```

may either:

```text
observe completed(event) -> resume
```

or:

```text
suspend task on event
```

or, if a deadline observation is part of the computation:

```text
observe deadline_elapsed -> timeout outcome
```

without inventing a remote failure fact.

## Experimental tracks

RFC 0012 should begin with bounded experiments rather than attempt the whole language simultaneously.

### Track 1: minimal local abstract machine

Implement:

```text
values
closures
return
bind
function application
conditionals/matching
bounded recursion
continuations
```

No external effects are required initially.

Compare its results with the current semantic-body evaluator for overlapping programs.

### Track 2: typed semantic-event bridge

Add a tiny event set:

```text
record deterministic effect
read bounded memory cell
write bounded memory cell
```

Require explicit authority/resource checks.

Demonstrate that local computation does not directly mutate world state outside the request path.

### Track 3: RFC 0009 memory bridge

Implement one reference/provenance/epoch-aware memory subset.

Negative tests must reject:

```text
stale epoch
out-of-bounds footprint
missing read/write authority
uninitialized read
forged numeric-address access
```

### Track 4: RFC 0010 event scheduling

Represent two independent semantic computations as enabled events.

Explore multiple legal schedules and show that the semantic behavior set, not the reference evaluator's canonical schedule, is authoritative.

Add one conflicting case that must serialize or reject.

### Track 5: RFC 0011 failure/knowledge bridge

Implement an ambiguous external operation:

```text
submit
external commit
lost acknowledgement
```

The semantic machine must reach an uncertain knowledge state rather than ordinary failure.

A handler must be unable to retry unless retry authority is established.

### Track 6: coinductive/infinite behavior

Model at least one:

```text
infinite pure loop
infinite event-producing service
recursive wait/respond loop
```

Ensure finite evaluator budget exhaustion remains distinct from semantic divergence.

### Track 7: trace/refinement experiment

Define one semantic computation and two structurally different realizations.

Show trace equivalence or trace inclusion over a bounded but representative corpus.

Then attach one independently checkable refinement certificate or proof obligation to the transformation.

### Track 8: abstract-machine-derived analyzer

Abstract one concrete domain, initially integers/control flow.

Demonstrate that the analyzer over-approximates the concrete reference machine on a frozen corpus.

Produce `UNKNOWN` rather than unsound exclusion when abstraction loses precision.

### Track 9: independent rewriting model

Implement a small overlapping subset in a rewriting-logic/K-style model or another independently developed executable formalism.

Differentially compare transition/trace behavior against the primary model.

Disagreement is treated as a semantic defect requiring investigation, not as permission to choose whichever output is convenient.

## Mandatory negative tests

Before RFC 0012 advances beyond Draft/Proposed, the executable core should include adversarial fixtures for at least:

1. unauthorized semantic effect;
2. consumed linear resource reuse;
3. stale memory provenance/epoch;
4. invalid pattern/tag state;
5. host-language evaluation order accidentally changing semantics;
6. reference evaluator budget exhaustion mislabeled as divergence;
7. timeout mislabeled as remote failure;
8. semantic nondeterminism collapsed into one claimed legal trace;
9. backend/SSA behavior introducing an observation outside the semantic contract;
10. stale refinement evidence after semantic-rule version change;
11. effect handler duplicating a linear continuation/resource;
12. unsupported construct accidentally receiving host-language behavior;
13. `UNKNOWN` analysis result used as optimization proof;
14. physical stack/address identity leaking into logical closure/continuation identity; and
15. internal implementation event incorrectly appearing in the public observation model.

## Graduation criteria

RFC 0012 should advance only when evidence demonstrates that the model is useful rather than merely elegant.

Minimum evidence should include:

1. a versioned normative transition-schema representation;
2. an executable local reference machine;
3. at least one typed semantic subsystem bridge;
4. deterministic canonical identities/serialization for persisted semantic artifacts;
5. finite and infinite behavior represented without conflating evaluator limits with semantics;
6. at least one nondeterministic/concurrent behavior represented as multiple legal executions;
7. RFC 0009 memory integration for one bounded subset;
8. RFC 0011 uncertain-outcome integration for one bounded subset;
9. one source/semantic -> HIR refinement experiment;
10. one HIR -> SSA refinement experiment;
11. one abstract interpretation demonstrably conservative over the concrete machine;
12. one independent executable semantics cross-check;
13. accepted, rejected, unsupported, unresolved, and suspended fixtures;
14. machine-readable diagnostics grounded in failed transition preconditions; and
15. documentation of the trusted boundary and all places where evidence remains empirical or incomplete.

## Open questions

The following questions intentionally remain research topics.

### Exact calculus notation

Should the formal presentation use:

- explicit MSOS labels;
- a configuration-plus-delta notation;
- a rewriting relation with modular cells;
- an intrinsically typed transition representation; or
- multiple equivalent presentations generated from one machine-readable rule schema?

The machine-readable source-of-truth direction is attractive, but human-inspectable formal rules remain necessary.

### CBPV depth

Should CBPV merely inspire value/computation separation, or should the executable core adopt a recognizable CBPV calculus with explicit `thunk`/`force`-like distinctions?

The answer should be driven by effect clarity, proof burden, agent generation quality, and lowering simplicity rather than familiarity.

### Handler expressiveness

How much algebraic-effect handler power should be admitted before reasoning about linear resources, authority, and continuations becomes too complex?

A deliberately restricted first handler calculus may be preferable.

### Determinism of local pure evaluation

Should the pure local machine select a canonical deterministic reduction strategy while permitting verified reordering as refinement, or should the core semantics expose independent pure reductions directly?

A deterministic local core may simplify reference implementation, while RFC 0010 can still expose parallel semantics at the event level.

### Behavior equivalence

Which equivalence/refinement relations should be normative for which transformation classes?

No single relation is assumed sufficient for pure functions, I/O, concurrency, relaxed numerical semantics, and external uncertainty.

### Proof encoding

How much of the transition relation should be encoded inside RFC 0007's proof language versus checked through an independent semantics verifier/certificate?

Keeping the proof kernel small remains a hard requirement.

### Abstract interpretation framework

Should the project define a reusable lattice/fixpoint framework tied directly to the semantic machine, or keep early abstract interpreters specialized?

Specialization may produce faster experimental progress; a shared framework may reduce later drift.

### Rule-schema self-description

Can the semantic rule set itself be represented as a versioned MNCS artifact subject to introspection, diffing, proof obligations, and recursive refinement without creating circular trust?

This likely connects to the later staging/self-description RFC and must preserve bootstrap boundaries.

## Relationship to prior RFCs

### RFC 0001 — Semantic Foundation

RFC 0012 gives executable transition meaning to the semantic entities introduced by the foundational model.

### RFC 0002 — Contract and Evidence Model

Transition preconditions and refinement claims produce obligations/evidence relationships under RFC 0002.

### RFC 0003 — Verified IR

HIR/SSA are treated as refinement stages of the executable semantics rather than independent meanings.

### RFC 0004 — Recursive Introspection and Refinement

Machine-readable transition identities, diagnostics, causal slices, and refinement evidence become inputs to recursive repair and promotion.

### RFC 0005 — Source Representations

Source syntaxes elaborate into the executable semantic core. Surface order or syntax must not introduce hidden semantics that cannot be represented canonically.

### RFC 0006 — Machine Intent

Intent, requirements, preferences, and lowering envelopes constrain valid executable realizations without replacing exact semantics.

### RFC 0007 — Proof-Carrying Dependent Core

The proof core remains distinct and small. RFC 0012 generates and consumes obligations but does not replace proof checking.

### RFC 0008 — I/O, Resources, Effects, and Events

RFC 0008 operations become typed semantic requests interpreted by the world semantics.

### RFC 0009 — Memory, References, Provenance, and Storage

RFC 0009 memory transitions become a semantic subsystem of the world machine.

### RFC 0010 — Concurrency and Memory Consistency

RFC 0010 event graphs govern legal ordering, visibility, interference, and scheduling among enabled transitions.

### RFC 0011 — Failure, Recovery, Nondeterminism, and Observation

RFC 0011 outcome, knowledge, observation, retry, and recovery states become first-class execution states/transitions.

## Consequences for the roadmap

If this direction survives experiment, the project should treat the executable semantic core as the reference against which future foundational work is connected.

In particular, later RFCs for:

```text
abstraction/polymorphism
modules/components
unsafe/FFI/ABI
staging/metaprogramming/self-description
runtime/target contracts
```

should extend the same machine rather than define separate execution models.

The existing body/HIR/SSA evaluators should gradually become conformance projections of RFC 0012 instead of accumulating independent semantics.

## Proposed constitutional rules

RFC 0012 proposes the following rules for further experimentation.

1. **One executable meaning.** Executable representations must relate to one normative semantic transition system.
2. **Values are not computations.** The core must preserve an explicit semantic distinction between inert values and effectful/partial computation.
3. **Effects cross typed boundaries.** Global semantic state is accessed through typed semantic operations, not accidental host effects.
4. **Rules are modular.** Unrelated semantics must not be rewritten merely because a new state dimension is added.
5. **Internal steps are not observations.** Optimization may change internal execution while preserving the declared observation model.
6. **Infinite execution is first-class.** Divergence and long-lived interaction belong to the semantics, not evaluator failure modes.
7. **Nondeterminism remains visible.** A reference schedule or implementation choice does not erase other permitted behavior.
8. **Lowering requires refinement evidence.** HIR, SSA, backend IR, and machine code are valid only insofar as they refine the semantic contract.
9. **Analyses approximate the same machine.** Static analyses should conservatively abstract normative semantics where practical.
10. **Host behavior is never a default specification.** Unsupported constructs remain unsupported until semantics are defined.
11. **Unknown removes authority.** Missing semantic/evidence facts cannot justify stronger optimization or behavioral claims.
12. **Semantic rules are inspectable artifacts.** Machines should eventually be able to query rule dependencies, state dimensions, obligations, and observation effects directly.

## Closing position

MNCS has intentionally developed proof, effects, memory, concurrency, and failure semantics before committing to a conventional source grammar or runtime model.

RFC 0012 is the point where those foundations become one executable theory.

The language should not be defined by what its current Rust evaluator happens to do, what LLVM happens to permit, what a human-facing syntax implies by sequencing, or what one verifier happens to understand.

It should be defined by an explicit semantic machine whose transitions are modular, typed, inspectable, and independently checkable.

The machine may be slow. Production implementations should not be.

Its purpose is to define **what execution means**, expose **what implementations are free to change**, and provide one stable object against which autonomous generation, verification, optimization, debugging, replay, and target lowering can reason.
