# Roadmap

This roadmap deliberately separates semantic research from surface-language design.

## 0.1 — Executable semantic model

**Goal:** demonstrate that MNCS relationships can be represented and mechanically checked before a source grammar exists.

Acceptance criteria:

- represent modules, functions, contracts, effects, capabilities, assumptions, evidence, and failure modes;
- reject an effect that lacks a declared authorizing capability;
- reject evidence that references an unknown property;
- report contracts that have no evidence;
- emit deterministic, machine-readable diagnostics;
- include accepted and intentionally rejected examples;
- document trust boundaries and non-goals;
- run formatting, linting, tests, and example validation in CI.

## 0.2 — Semantic graph and identity

- define stable identities for components, properties, assumptions, verifiers, and artifacts;
- model dependency and invalidation edges;
- define canonical serialization and content hashing;
- distinguish source identity, semantic identity, and implementation identity;
- produce a first evidence-manifest schema.

## 0.3 — High-level MNCS IR

- define typed state, transitions, ownership relationships, and regions;
- represent normal, error, and compensation paths;
- make effects and capability flow explicit;
- define deterministic lowering from the semantic graph;
- preserve source-to-semantic traceability.

## 0.4 — Verified SSA

- introduce basic blocks, SSA values, phi-like joins, and explicit memory regions;
- attach proof obligations to transformations;
- define conservative aliasing and overflow semantics;
- integrate the first micro-verifier interfaces;
- demonstrate incremental invalidation after a local change.

## 0.5 — Backend experiment

- lower a constrained subset to LLVM IR, Cranelift IR, or WebAssembly;
- emit optimization attributes only from verified facts;
- produce an executable and evidence manifest together;
- compare generated output with equivalent Rust or Zig implementations;
- document all backend-introduced assumptions.

## 0.6 — Surface-language experiments

- develop at least two competing syntax proposals;
- test human readability, agent repairability, canonical formatting, and parser stability;
- avoid syntax that cannot round-trip to the semantic graph without hidden meaning;
- select a grammar only after semantic coverage is demonstrated.

## 1.0 research threshold

A 1.0 designation would indicate a coherent research language and toolchain, not necessarily production maturity. It requires:

- a versioned specification;
- a stable semantic core;
- deterministic parsing and canonical formatting;
- at least one executable backend;
- an evidence manifest with verifiable artifacts;
- bounded unsafe and foreign-function interfaces;
- documented soundness limits;
- conformance tests and independent implementation guidance.
