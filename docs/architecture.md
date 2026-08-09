# Architecture

This document describes the initial conceptual layers. Their exact implementation remains experimental.

## Layer 1: source representation

The eventual surface language should expose contracts, effects, capabilities, assumptions, failure behavior, refinement boundaries, and machine-intent operations without requiring users to author low-level IR. No grammar has been selected.

Source syntax should distinguish:

- exact semantic requirements;
- established facts;
- optimization preferences;
- target and portability constraints;
- evidence-bearing capabilities; and
- unsafe or unresolved assumptions.

A familiar source idiom must not acquire stronger semantics merely because a compiler recognizes its
shape.

## Layer 2: semantic graph

Parsing and name resolution produce a graph of stable semantic entities:

- components and functions;
- values and types;
- state regions;
- contract properties;
- effect and capability edges;
- assumptions;
- verifier and evidence edges;
- dependencies and invalidation relationships;
- machine-intent operations;
- facts, preferences, and requirements;
- obligation templates and instantiated obligations;
- reference semantics and target realizations;
- lowering and portability envelopes;
- diagnostics, candidate transformations, and promotion history.

The graph should support canonical serialization and content-addressed identity.

A machine-intent expression should be understandable as:

```text
exact semantics
+ machine intent
+ required invariants and capabilities
+ permitted lowering envelope
+ evidence and target envelope
```

The graph must keep these parts separately addressable so that a changed preference does not look
like a changed requirement and a stale fact cannot continue authorizing backend promises.

## Layer 3: high-level MNCS IR

The high-level IR makes state transitions, ownership, failure paths, effect closure, candidate mutation regions, machine-intent operations, and generated obligations explicit while preserving source-level meaning. It is the preferred layer for most semantic verification and agent repair.

Illustrative operations include:

```text
mncs.add.wrap.i32
mncs.add.checked.i32
mncs.dispatch.closed
mncs.select.constant_time
mncs.load.aligned.u32
mncs.layout.realize
```

The high-level IR should preserve why an operation exists and which facts, requirements, target
constraints, and verifier results govern its lowering.

## Layer 4: verified SSA

A lower representation introduces basic blocks and explicit dataflow. Transformations should carry proof obligations or evidence explaining why they preserve relevant properties.

Machine-intent operations may lower to ordinary SSA instructions only while retaining traceability
to:

- the source and semantic expression;
- consumed facts and evidence-bearing capabilities;
- generated runtime checks;
- transformation rules;
- target realization;
- discharged or unresolved obligations; and
- fallback behavior.

Semantically required intent must not survive only as discardable debugging metadata.

## Layer 5: conventional backend

LLVM, Cranelift, WebAssembly, C, or another backend performs target lowering, register allocation, scheduling, and emission. Backend-facing optimization claims should be conservative:

- emit `noalias` only when non-aliasing is established;
- emit no-overflow flags only when overflow impossibility is established;
- emit in-bounds pointer claims only when bounds are established;
- emit alignment claims only when the exact subject is proven aligned;
- emit floating-point relaxations only when the corresponding policy permits them;
- reduce atomic ordering only when a concurrency argument is established;
- otherwise preserve conservative semantics.

Backend intrinsics may implement one realization of a stable semantic operation, but should not
normally define the operation's language-level meaning.

## Machine-intent lowering plane

Machine-oriented lowering should proceed through explicit obligation generation rather than
source-pattern recognition alone.

```text
source expression
      ↓
canonical semantic operation
      ↓
obligation instantiation
      ↓
static discharge / runtime check / Forge verifier request
      ↓
valid realization set
      ↓
policy and preference selection
      ↓
high-level MNCS IR
      ↓
verified SSA
      ↓
backend artifact + evidence manifest
```

An obligation has `PASS`, `FAIL`, or `UNKNOWN` status under a declared method and scope.

- `PASS` permits only the dependent lowering choices it actually establishes.
- `FAIL` rejects those choices.
- `UNKNOWN` prevents optimistic promises but may permit a declared conservative fallback.

The compiler must never convert `UNKNOWN` into `PASS` because a target ordinarily behaves in a
particular way.

## Reference semantics and realizations

A semantic operation may bind a clear reference meaning to multiple target realizations:

```text
operation: widened i16 dot product
reference: portable scalar
realizations: AVX-512, AVX2, SVE, NEON, scalar
requirements: equal output, bounded access, explicit overflow
```

The reference may be executable, relational, mathematical, or contractual. Its own trust boundary
remains explicit. Performance evidence selects only among semantically valid realizations.

## Evidence sidecar

Evidence is not merely a compiler log. It is a first-class artifact graph linked to properties, implementations, assumptions, verifier versions, and inputs.

An evidence node should eventually identify:

- property identity;
- subject identity;
- verifier identity and version;
- evidence status;
- artifact hash and location;
- assumptions consumed;
- dependencies consumed;
- time and environment when relevant;
- invalidation triggers;
- machine-intent expression and operation identity;
- selected realization and portability envelope;
- compiler, backend, and pass-pipeline identity;
- obligations and evidence-bearing capabilities consumed; and
- backend promises emitted or deliberately withheld.

## Recursive refinement plane

Recursive debugging and improvement operate across the semantic, IR, and evidence layers rather than as an unstructured source-rewrite loop.

```text
trusted baseline
      ↓ observe
semantic graph + evidence graph
      ↓ localize
diagnostic obligation + causal slice
      ↓ propose
isolated candidate transformation
      ↓ verify
targeted verifier results
      ↓ compare
semantic, authority, complexity, and evidence delta
      ↓ policy
promote or reject
      ↺
```

The output of one cycle can become input to a later cycle, which gives the architecture its recursive character. Every cycle must remain bounded by mutation scope, capabilities, recursion depth, resource budgets, candidate count, verifier-call limits, stopping rules, and promotion policy.

Machine-intent optimization regions add target, realization, protected-property, and selection-metric
bounds. A candidate that is faster but violates a requirement is rejected rather than reclassified
as an acceptable tradeoff.

### Diagnostic obligations

A failed or weak property should be represented as a stable object rather than only a text message. It should identify the semantic subject, property, evidence state, assumptions, reproducer where relevant, and the smallest defensible causal slice.

For machine-intent operations, a diagnostic should also identify the dependent realization,
backend promise, target envelope, available conservative fallback, and evidence invalidated by a
repair.

### Candidate transformations

A repair proposal is untrusted and receives a separate candidate identity. It declares intended improvements, protected properties, required authority, predicted invalidation, and a verification plan. Generation does not modify the trusted baseline.

A machine-oriented candidate should preserve the semantic operation and modify only a declared
realization or optimization region unless the proposal explicitly requests a semantic change.

### Comparison and promotion

Candidate evaluation must report more than whether the original failure disappeared. It should reveal changed contracts, effects, capabilities, assumptions, complexity, evidence, obligations, target support, backend promises, and unresolved consequences. Promotion is explicit and policy-controlled.

### Self-description

Compiler passes, lowering rules, verifier contracts, diagnostics, operation vocabularies, obligation
templates, and promotion policies should progressively become visible through the same introspection surface. A future self-hosted compiler may use this surface to diagnose and propose changes to itself without pretending that self-hosting removes bootstrap trust.

## Forge integration

The MNCS Forge can host or coordinate micro-verifiers that answer bounded questions against semantic or IR nodes. Rather than repeatedly ingesting enormous whole-program reports, an agent should be able to request evidence such as:

```text
verify effect closure for component X
verify conservation property Y across transition Z
explain the minimal causal path for failed obligation Q
identify evidence invalidated by change set C
check whether candidate R broadens authority
compare protected properties between baseline B and candidate R
prove these regions disjoint for this loop
check constant-time lowering for this target artifact
verify this computed-dispatch target set is closed
identify which backend promises depend on capability C
```

The Forge may execute causal localization and candidate checks, but the language owns the semantic structures and obligation vocabulary those operations consume and produce.

## RAVEL integration

RAVEL may coordinate recursive and distributed refinement across agents, machines, verifier implementations, targets, and trust boundaries. It should consume interoperable diagnostic, proposal, budget, evidence, and promotion artifacts rather than relying on one privileged internal representation.

Target-specific results must remain bound to their portability envelopes. Cross-machine repetition is
not automatically equivalent to independent evaluation or universal portability.

## Bootstrap implementation

The current Rust crates implement structural consistency checks plus an experimental 0.2 semantic
foundation: deterministic canonical JSON, content fingerprints, stable identities, typed graph and
invalidation edges, evidence manifests, diagnostic obligations, conservative causal slices, finite
refinement budgets, isolated untrusted repair proposals, explicit promotion records, and separate
semantic/authority/evidence deltas. An early 0.3 high-level IR is executable for the supported
manifest subset, including executable symbolic bodies, typed values, state regions, failure paths,
capability uses, generated obligations, semantic-to-IR traceability, transformation provenance,
lowering/portability envelopes, and a pure local verifier protocol with importable result artifacts.
The surface syntax, verified SSA, complete recursive analysis, and backend selection remain
unresolved.

See [Machine-intent expressions](machine-intent-expressions.md),
[Machine-Intent Expressions 0.1 Direction](../spec/machine-intent-expressions.md), and
[RFC 0006](../rfcs/0006-machine-intent-expressions.md).
