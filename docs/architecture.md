# Architecture

This document describes the initial conceptual layers. Their exact implementation remains experimental.

## Layer 1: source representation

The eventual surface language should expose contracts, effects, capabilities, assumptions, and failure behavior without requiring users to author low-level IR. No grammar has been selected.

## Layer 2: semantic graph

Parsing and name resolution produce a graph of stable semantic entities:

- components and functions;
- values and types;
- state regions;
- contract properties;
- effect and capability edges;
- assumptions;
- verifier and evidence edges;
- dependencies and invalidation relationships.

The graph should support canonical serialization and content-addressed identity.

## Layer 3: high-level MNCS IR

The high-level IR makes state transitions, ownership, failure paths, and effect closure explicit while preserving source-level meaning. It is the preferred layer for most semantic verification and agent repair.

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

## Forge integration

The MNCS Forge can host or coordinate micro-verifiers that answer bounded questions against semantic or IR nodes. Rather than repeatedly ingesting enormous whole-program reports, an agent should be able to request evidence such as:

```text
verify effect closure for component X
verify conservation property Y across transition Z
explain the minimal causal path for failed obligation Q
identify evidence invalidated by change set C
```

## Bootstrap implementation

The current Rust crates implement only structural consistency checks against JSON manifests. They establish a runnable test bed while leaving syntax and backend selection unresolved.
