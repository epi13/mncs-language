# Roadmap

This roadmap separates semantic research from final surface-language selection while treating recursive introspection, representation efficiency, machine-intent semantics, and refinement as cross-cutting requirements.

## Active cross-cutting track — source representations

The project now maintains an experimental syntax laboratory before committing to a grammar.

Current work:

- compare Zig-influenced, fully structured, and minimal human-source candidates;
- maintain a distinct compact canonical semantic form for agents and tooling;
- represent recursive changes as semantic patches rather than unrestricted text replacement;
- declare equivalent semantic claims before comparing candidate density;
- collect deterministic tokenizer-neutral metrics in CI;
- extend later studies with multiple model tokenizers, parser behavior, repair success, human comprehension, and verifier cost.

A final grammar must not be selected from character count alone.

## Active cross-cutting track — machine-intent expressions

[RFC 0006](rfcs/0006-machine-intent-expressions.md) proposes explicit semantic operations for
machine-oriented behavior that would otherwise depend on source idioms, undefined behavior,
unchecked compiler promises, or target folklore.

Current design work:

- separate exact semantics, intent, preferences, facts, requirements, and obligations;
- define explicit arithmetic edge behavior;
- model evidence-bearing memory, authority, and confinement capabilities;
- preserve closed dispatch, constant-time, layout, and target-specialization intent through IR;
- define lowering and portability envelopes;
- bind backend promises to current evidence;
- retain portable references and conservative fallbacks; and
- allow repeatedly verified non-orthodox techniques to graduate through explicit semantic review.

The first implementation pilot should remain narrow: integer arithmetic, one memory capability,
canonical serialization, IR preservation, and one conservative backend-promise experiment.

## 0.1 — Executable semantic model

**Goal:** demonstrate that MNCS relationships can be represented and mechanically checked before a source grammar exists.

Acceptance criteria:

- represent modules, functions, contracts, effects, capabilities, assumptions, evidence, and failure modes;
- reject an effect that lacks a declared authorizing capability;
- reject evidence that references an unknown property;
- report contracts that have no evidence;
- emit deterministic, machine-readable diagnostics;
- include accepted and intentionally rejected examples;
- document trust boundaries, recursive-refinement requirements, machine-intent direction, and non-goals;
- run formatting, linting, tests, example validation, and syntax measurements in CI.

## 0.2 — Semantic graph, identity, and recursive artifacts

Initial executable foundation delivered in the Rust model and CLI:

- deterministic 0.2 canonical JSON and SHA-256 fingerprints;
- structural identities for programs, functions, contracts, assumptions, effects, capabilities, and evidence claims;
- typed semantic graph nodes and relationship edges;
- conservative dependency-aware evidence invalidation;
- executable evidence manifests with dependency fingerprints and stale assessment;
- CLI commands for canonicalization, identities, graph emission, evidence manifests, diffs, and freshness checks; and
- a narrow machine-intent pilot preserving explicit operation and obligation identities;
- executable diagnostic obligations and conservative graph-based causal slices;
- finite refinement budgets, untrusted isolated repair proposals, and explicit promotion decisions;
- semantic, authority, and evidence delta records with explicit authority-broadening flags; and
- distinct observation, verification, mutation, generation, and promotion capabilities;
- typed semantic patches, protected-property evaluations, realization/portability envelopes, and
  transformation provenance; and
- versioned verifier request/result artifacts with exact subject, scope, dependency, freshness, and
  verifier-independence checks.

The implemented recursive artifacts are intentionally partial: causal slices are conservative
backward graph slices rather than formal minimal slices, repair application supports a small typed
patch vocabulary, realization selection is envelope-only, and promotion remains an explicit record
rather than an automatic mutation. Control/data-flow-complete refinement and target-specific
realizations remain future work.

- define stable identities for components, properties, assumptions, verifiers, artifacts, diagnostics, transformations, machine-intent operations, obligations, references, and realizations;
- model dependency and invalidation edges;
- define canonical serialization and content hashing;
- distinguish source identity, semantic identity, implementation identity, candidate identity, realization identity, and evidence identity;
- produce a first evidence-manifest schema;
- define schemas for diagnostic obligations, causal slices, repair proposals, refinement budgets, semantic deltas, evidence deltas, and promotion decisions;
- define facts, preferences, requirements, lowering envelopes, and portability envelopes as distinct semantic objects;
- ensure observation and mutation require distinct capabilities;
- version the canonical semantic representation independently from human syntax.

The first executable subset of these artifacts is in `mncs-model::refinement` and
`mncs-model::delta`; the JSON shapes are versioned `0.2` research artifacts, not an accepted RFC.

## 0.3 — High-level MNCS IR

Early executable foundation now present for the supported 0.1 manifest subset:

- deterministic `0.3` high-level IR serialization and SHA-256 fingerprints;
- typed values, explicit state regions, normal/error/compensation/fatal blocks, and transitions;
- effect operations with explicit capability uses;
- contract, assumption, evidence, and semantic graph traceability;
- generated obligations carried by the IR;
- executable semantic bodies with symbolic constants/integer/effect/runtime-check operations;
- direct body-to-HIR lowering with machine-intent links and transformation provenance;
- versioned lowering and portability envelopes with conservative realization selection; and
- a pure verifier request/result protocol with a deterministic capability-consistency verifier and
  import validation for external artifacts;
- conservative diagnostic output connected to generated obligations.

This remains an experimental subset and does not claim complete lowering correctness. CFG reachability,
predecessor/successor relationships, dominance, block-argument checking, and unreachable-block
rejection are now executable. A small block-parameter SSA artifact is available as the first 0.4
foundation, including deterministic identities, validation, and body→HIR→SSA traceability.

- define typed state, transitions, ownership relationships, and regions;
- represent normal, error, compensation, and diagnostic paths;
- make effects and capability flow explicit;
- define deterministic lowering from the semantic graph;
- preserve source-to-semantic traceability;
- preserve machine-intent operation and obligation identities;
- represent explicit integer edge behavior without backend undefined behavior;
- preserve enough structure to identify bounded repair regions and protected properties;
- represent candidate transformations without modifying the trusted baseline;
- demonstrate lossless projection from human source to canonical semantic form for the supported subset.

## 0.4 — Verified SSA and micro-debugging

Initial bounded foundation now present:

- versioned `0.4` SSA values, instructions, block parameters, terminators, and failure targets;
- deterministic SSA serialization/fingerprints and structural validation;
- semantic-body → HIR → SSA trace entries and transformation provenance;
- verifier/evidence authority classes that keep diagnostic observations from satisfying proof obligations; and
- one evidence-gated no-overflow-elision experiment that refuses missing, stale, wrong-scope, or non-verifier PASS evidence.

This is not verified compiler correctness, SSA optimization, memory SSA, or a backend.

The Luna bounded reference-execution increment is now also present:

- versioned JSON execution requests/results with explicit returned, runtime-failure, unsupported,
  budget-exhausted, and invalid-request outcomes;
- deterministic body-level interpretation with a mandatory finite step budget and capped identity
  traces;
- typed integer comparison (`eq`, `ne`, `lt`, `le`, `gt`, `ge`) through body validation, HIR, and
  SSA, with signed/unsigned interpretation;
- conservative effect handling with explicit deterministic recording policy and no ambient authority;
- reusable corpus comparison reports with mismatch localization and finite-equivalence language;
- a bounded-sum baseline, structural equivalent refactor, intentional regression, and frozen edge corpus;
- CLI commands `execute` and `compare-execution`; and
- Forge development verifier `mncs-language.execution-equivalence`.

This enables experimental MNCS-Language refactoring/reference studies before backend completion. It
does not establish universal equivalence, production runtime behavior, or compiler correctness.

- introduce basic blocks, SSA values, phi-like joins, and explicit memory regions;
- attach proof obligations to transformations;
- define conservative aliasing and overflow semantics;
- implement the first evidence-bearing bounds or alignment capability;
- preserve machine-intent traceability through SSA lowering;
- ensure `UNKNOWN` obligations select only declared conservative fallbacks;
- integrate the first micro-verifier interfaces;
- demonstrate incremental invalidation after a local change;
- derive a machine-readable diagnostic obligation from a failed verifier;
- produce a minimal defensible causal slice;
- generate and evaluate at least one isolated repair proposal;
- compare the baseline and candidate across contracts, effects, capabilities, assumptions, complexity, target support, backend promises, and evidence;
- enforce recursion, authority, candidate-count, and verifier-call budgets.

## 0.5 — Backend and bounded recursive-refinement experiment

- lower a constrained subset to LLVM IR, Cranelift IR, or WebAssembly;
- emit optimization attributes only from verified facts;
- demonstrate wrapping, checked, saturating, trapping, and widening integer operations;
- record why backend no-overflow, in-bounds, alignment, non-aliasing, or relaxation promises are emitted or withheld;
- produce an executable and evidence manifest together;
- compare generated output with equivalent Rust or Zig implementations;
- bind target-specific realizations to compiler, backend, feature, ABI, and pass-pipeline identities;
- document all backend-introduced assumptions;
- complete one bounded observe–localize–propose–verify–compare–promote cycle;
- require independent evidence before automatic candidate promotion;
- retain an auditable record of accepted and rejected candidate transformations.

The next Sol High pass should decide whether the body reference evaluator is sufficient as a study
oracle or whether a separately validated SSA evaluator is needed, without conflating either with a
production backend.

## 0.6 — Surface-language and self-description experiments

- expand the syntax tournament to a representative corpus;
- implement parsers and canonical formatters for at least two competing source candidates;
- include machine-intent expressions in syntax and semantic-density comparisons;
- measure human readability, agent generation, localized repair, model-token counts, parser recovery, canonical formatting, graph round trips, omission rates, and verifier workload;
- ensure every compared candidate represents the same declared semantic claims;
- avoid syntax that cannot round-trip to the semantic graph without hidden meaning;
- select or reject the Zig-influenced direction using collected evidence;
- expose compiler and verifier contracts, operation vocabularies, obligation templates, and lowering rules through the same introspection model used for ordinary programs;
- begin a limited self-hosting experiment without obscuring bootstrap trust;
- select a grammar only after semantic coverage and repairability are demonstrated.

## 0.7 — Target realizations and optimization regions

- bind portable reference semantics to at least two target-specific realizations;
- implement explicit target dispatch and conservative fallback;
- compare scalar and SIMD realizations under one semantic operation identity;
- add a bounded optimization-region model with mutation, target, authority, resource, candidate, and verifier limits;
- reject a faster candidate that violates a protected requirement;
- measure performance, code size, evidence cost, portability, and invalidation behavior;
- preserve accepted and rejected realization histories; and
- demonstrate stale evidence after target, compiler, backend, or pass-pipeline changes.

## 1.0 research threshold

A 1.0 designation would indicate a coherent research language and toolchain, not necessarily production maturity. It requires:

- a versioned specification;
- a stable semantic core;
- deterministic parsing and canonical formatting;
- a versioned canonical semantic representation;
- explicit machine-intent, obligation, lowering-envelope, and portability-envelope semantics;
- at least one executable backend;
- an evidence manifest with verifiable artifacts;
- conservative and traceable backend-promise emission;
- bounded unsafe and foreign-function interfaces;
- a versioned recursive diagnostic and refinement protocol;
- isolated candidate transformations and explicit promotion policy;
- documented recursion, authority, resource, target, and representation limits;
- documented soundness and bootstrap limits;
- conformance tests and independent implementation guidance.
