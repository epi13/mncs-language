# Architecture

This document describes the initial conceptual layers. Their exact implementation remains experimental.

## Layer 1: source representation

The eventual surface language should expose contracts, effects, capabilities, assumptions, failure behavior, and refinement boundaries without requiring users to author low-level IR. No grammar has been selected.

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
- diagnostics, candidate transformations, and promotion history.

The graph should support canonical serialization and content-addressed identity.

## Layer 3: high-level MNCS IR

The high-level IR makes state transitions, ownership, failure paths, effect closure, and candidate mutation regions explicit while preserving source-level meaning. It is the preferred layer for most semantic verification and agent repair.

## Layer 4: verified SSA

A lower representation introduces basic blocks and explicit dataflow. Transformations should carry proof obligations or evidence explaining why they preserve relevant properties.

## Layer 5: conventional backend

LLVM, Cranelift, WebAssembly, C, or another backend performs target lowering, register allocation, scheduling, and emission. Backend-facing optimization claims should be conservative:

- emit `noalias` only when non-aliasing is established;
- emit no-overflow flags only when overflow impossibility is established;
- emit in-bounds pointer claims only when bounds are established;
- otherwise preserve conservative semantics.

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
- invalidation triggers.

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

### Diagnostic obligations

A failed or weak property should be represented as a stable object rather than only a text message. It should identify the semantic subject, property, evidence state, assumptions, reproducer where relevant, and the smallest defensible causal slice.

### Candidate transformations

A repair proposal is untrusted and receives a separate candidate identity. It declares intended improvements, protected properties, required authority, predicted invalidation, and a verification plan. Generation does not modify the trusted baseline.

### Comparison and promotion

Candidate evaluation must report more than whether the original failure disappeared. It should reveal changed contracts, effects, capabilities, assumptions, complexity, evidence, and unresolved consequences. Promotion is explicit and policy-controlled.

### Self-description

Compiler passes, lowering rules, verifier contracts, diagnostics, and promotion policies should progressively become visible through the same introspection surface. A future self-hosted compiler may use this surface to diagnose and propose changes to itself without pretending that self-hosting removes bootstrap trust.

## Forge integration

The MNCS Forge can host or coordinate micro-verifiers that answer bounded questions against semantic or IR nodes. Rather than repeatedly ingesting enormous whole-program reports, an agent should be able to request evidence such as:

```text
verify effect closure for component X
verify conservation property Y across transition Z
explain the minimal causal path for failed obligation Q
identify evidence invalidated by change set C
check whether candidate R broadens authority
compare protected properties between baseline B and candidate R
```

The Forge may execute causal localization and candidate checks, but the language owns the semantic structures those operations consume and produce.

## RAVEL integration

RAVEL may coordinate recursive and distributed refinement across agents, machines, verifier implementations, and trust boundaries. It should consume interoperable diagnostic, proposal, budget, evidence, and promotion artifacts rather than relying on one privileged internal representation.

## Bootstrap implementation

The current Rust crates implement only structural consistency checks against JSON manifests. They establish a runnable test bed while leaving syntax, recursive-artifact schemas, and backend selection unresolved.
