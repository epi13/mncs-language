# Compiler Architecture Assessment — 2026-08

## Outcome

The repository has a credible evidence-bearing compiler spine for a bounded semantic subset, not yet a complete language compiler.

C0–C2 are implemented at the repository boundary: deterministic compiler identities and requests, semantic validation, semantic→HIR→SSA orchestration, transformation edges, unresolved-obligation retention, target-plan uncertainty, Linux/Windows CI, and an AArch64 compile-shape gate. No selected lexer, parser, lossless CST, representation-neutral AST, complete elaborator, portable backend, native backend, linked executable, native Raspberry Pi run, or translation validator exists.

The maturity description is therefore:

> semantic compiler spine with evidence contracts; frontend and executable backend not yet implemented.

## Existing compiler stages

| Logical stage | Existing implementation | Maturity |
| --- | --- | --- |
| source / intent representation | semantic JSON manifests and experimental source candidates | bootstrap only |
| lexical analysis | syntax metrics count lexical units but do not emit a compiler token stream | missing |
| parsing | `serde_json` creates `Program` before compiler invocation | bootstrap external path |
| CST | none | missing |
| AST | none distinct from semantic `Program` | missing |
| semantic graph | `Program::semantic_graph` with typed nodes/edges | experimental library capability |
| identity resolution | deterministic semantic identity inventory | experimental library capability; not scope/binding resolution |
| type and contract analysis | structural validation, effect/capability closure, obligation generation | partial |
| HIR | deterministic 0.3 HIR with trace and provenance | experimental driver stage |
| verification passes | structural validators, evidence checks, bounded body/SSA comparison | partial; not translation validation |
| lowering / backend | canonical SSA selection and `TargetLoweringPlan` | boundary only |
| executable artifact | reference interpreters only | missing |

Run `mncs compiler-architecture` for the versioned machine-readable statement.

## Established semantic concepts

The repository already establishes useful foundations:

- semantic identity distinct from paths and host identity;
- canonical serialization and content fingerprints;
- recursive semantic graph relationships and invalidation edges;
- explicit contracts, effects, capabilities, assumptions, and failure modes;
- evidence states with `PASS`, `FAIL`, and `UNKNOWN` separation;
- diagnostic obligations and causal slices;
- HIR/SSA provenance and exact transformation-edge identities;
- compiler/build/target/run identity separation;
- deterministic target-independent artifacts;
- bounded reference execution with explicit limitations;
- target facts and backend assumptions that cannot be inferred from a target name;
- Forge as search/evidence coordination rather than semantic authority.

## Missing concepts before implementation accelerates

The next acceleration threshold requires contracts, not syntax volume:

1. A source artifact envelope independent of files, including artifact kind, relationships, provenance, language profile, and execution intent.
2. Lossless token, source-span, trivia, CST, recovery diagnostic, and source-map identity rules.
3. A representation-neutral AST and explicit AST→semantic-graph elaboration relation.
4. Scope, binding, namespace, import, generic, constraint, defaulting, and coherence rules from RFC 0035.
5. A type/contract calculus beyond the current structural validator.
6. Semantic-graph and identity-resolution outputs integrated into compilation evidence.
7. Translation-validation request/result and fallback policy implementations.
8. Target-contract resolution, portable backend adapter, linker derivation, executable identity, and runtime envelope.
9. Feature/profile compatibility and migration fixtures across compiler versions.
10. Forge-native experiment persistence linked to separate assurance and conformance results.

## Undefined language contracts

The following boundaries remain intentionally undefined:

- final surface grammar and lexical policy;
- whether whitespace/trivia contributes to any source identity class;
- CST and AST schema/versioning;
- error recovery and partial-program identity;
- artifact-kind taxonomy and recursive containment semantics;
- name lookup, shadowing, hygiene, module imports, and cyclic dependencies;
- general type formation, subtyping/refinement, inference, and trait/interface coherence;
- semantic graph completion rules for non-program artifacts;
- universal behavioral relation for each lowering class;
- target fact authority and expiration;
- backend ABI, layout, trap, unwind, object, and linker contracts;
- executable packaging and lifecycle semantics;
- conformance profiles and minimum required evidence by language version.

## Architectural-debt risks

The highest-risk assumptions are:

- treating `Program::from_json` as the future parser;
- calling the semantic `Program` an AST and later coupling syntax to semantic identity;
- creating a second HIR/SSA in the compiler crate;
- equating structural validation with a complete type system;
- letting compiler completion or Forge selection imply correctness;
- copying compiler schemas into Forge and allowing them to drift;
- making paths, timestamps, hostnames, or process details part of portable semantic identity;
- selecting a backend before target/translation-validation boundaries are executable;
- reporting finite corpus agreement as equivalence;
- grouping Linux, Windows, ARM64, and ARM32 observations under one host or target identity.

## Implemented assessment response

This development slice adds:

- a versioned, content-addressed 12-stage architecture contract;
- explicit availability and integration state for every stage;
- a CLI architecture inspection command;
- a language-owned compiler study contract with stage fingerprints, pass execution observations, completion status, diagnostics, and unresolved obligations;
- tests for stage completeness, gap disclosure, deterministic identity, and pass/obligation retention;
- RFC 0039 defining the language/Forge boundary; and
- a Forge consumer that compares language-owned observations without emitting assurance or conformance verdicts.

## Practical roadmap

### Phase 1 — specification and minimal path

- stabilize source artifact and language profile contracts;
- keep the complete architecture record current;
- freeze token/CST/AST/diagnostic interfaces;
- integrate semantic graph and identity outputs into compilation evidence;
- expand deterministic positive and negative fixtures;
- retain semantic→HIR→SSA as the minimal end-to-end path.

### Phase 2 — frontend and semantic analysis

- implement lexer and parser for one deliberately small profile;
- emit lossless CST and representation-neutral AST;
- implement elaboration, scope, binding, identity resolution, and type/contract analysis;
- prove round-trip and cross-representation identity properties for fixtures;
- keep invalid or unresolved artifacts from HIR.

### Phase 3 — verified lowering and executable targets

- implement translation-validation contracts and conservative fallbacks;
- add a replaceable portable backend;
- run identical corpora on Linux, Windows, and Raspberry Pi environments;
- add a native backend experiment only after the portable boundary is exercised;
- retain all backend assumptions and derivation identities.

### Phase 4 — Forge-native compiler evolution

- persist compiler experiment observations through existing Forge evidence records;
- compare compiler/pass/IR versions and detect earliest divergence;
- track feature/profile compatibility and regression fixtures;
- run pass/backend tournaments with language-owned legality contracts;
- add distributed Fabric experiments and independent assurance workflows;
- keep conformance evaluation separate from observation and assurance.

## Recommended next cycle

Implement the source artifact envelope and lossless frontend contracts before adding grammar breadth. The acceptance fixture should represent at least two non-file artifact relationships, parse one bounded profile through tokens/CST/AST, elaborate to the existing semantic graph, preserve provenance, and demonstrate deterministic identity plus one recovery diagnostic and one rejected identity-laundering case.
