# Roadmap

This roadmap deliberately separates semantic research from surface-language design while treating recursive introspection and refinement as a cross-cutting requirement.

## 0.1 — Executable semantic model

**Goal:** demonstrate that MNCS relationships can be represented and mechanically checked before a source grammar exists.

Acceptance criteria:

- represent modules, functions, contracts, effects, capabilities, assumptions, evidence, and failure modes;
- reject an effect that lacks a declared authorizing capability;
- reject evidence that references an unknown property;
- report contracts that have no evidence;
- emit deterministic, machine-readable diagnostics;
- include accepted and intentionally rejected examples;
- document trust boundaries, recursive-refinement requirements, and non-goals;
- run formatting, linting, tests, and example validation in CI.

## 0.2 — Semantic graph, identity, and recursive artifacts

- define stable identities for components, properties, assumptions, verifiers, artifacts, diagnostics, and transformations;
- model dependency and invalidation edges;
- define canonical serialization and content hashing;
- distinguish source identity, semantic identity, implementation identity, candidate identity, and evidence identity;
- produce a first evidence-manifest schema;
- define schemas for diagnostic obligations, causal slices, repair proposals, refinement budgets, semantic deltas, evidence deltas, and promotion decisions;
- ensure observation and mutation require distinct capabilities.

## 0.3 — High-level MNCS IR

- define typed state, transitions, ownership relationships, and regions;
- represent normal, error, compensation, and diagnostic paths;
- make effects and capability flow explicit;
- define deterministic lowering from the semantic graph;
- preserve source-to-semantic traceability;
- preserve enough structure to identify bounded repair regions and protected properties;
- represent candidate transformations without modifying the trusted baseline.

## 0.4 — Verified SSA and micro-debugging

- introduce basic blocks, SSA values, phi-like joins, and explicit memory regions;
- attach proof obligations to transformations;
- define conservative aliasing and overflow semantics;
- integrate the first micro-verifier interfaces;
- demonstrate incremental invalidation after a local change;
- derive a machine-readable diagnostic obligation from a failed verifier;
- produce a minimal defensible causal slice;
- generate and evaluate at least one isolated repair proposal;
- compare the baseline and candidate across contracts, effects, capabilities, assumptions, complexity, and evidence;
- enforce recursion, authority, candidate-count, and verifier-call budgets.

## 0.5 — Backend and bounded recursive-refinement experiment

- lower a constrained subset to LLVM IR, Cranelift IR, or WebAssembly;
- emit optimization attributes only from verified facts;
- produce an executable and evidence manifest together;
- compare generated output with equivalent Rust or Zig implementations;
- document all backend-introduced assumptions;
- complete one bounded observe–localize–propose–verify–compare–promote cycle;
- require independent evidence before automatic candidate promotion;
- retain an auditable record of accepted and rejected candidate transformations.

## 0.6 — Surface-language and self-description experiments

- develop at least two competing syntax proposals;
- test human readability, agent repairability, canonical formatting, and parser stability;
- avoid syntax that cannot round-trip to the semantic graph without hidden meaning;
- expose compiler and verifier contracts through the same introspection model used for ordinary programs;
- begin a limited self-hosting experiment without obscuring bootstrap trust;
- select a grammar only after semantic coverage is demonstrated.

## 1.0 research threshold

A 1.0 designation would indicate a coherent research language and toolchain, not necessarily production maturity. It requires:

- a versioned specification;
- a stable semantic core;
- deterministic parsing and canonical formatting;
- at least one executable backend;
- an evidence manifest with verifiable artifacts;
- bounded unsafe and foreign-function interfaces;
- a versioned recursive diagnostic and refinement protocol;
- isolated candidate transformations and explicit promotion policy;
- documented recursion, authority, and resource limits;
- documented soundness and bootstrap limits;
- conformance tests and independent implementation guidance.
