# MNCS Language Project

[![CI](https://github.com/epi13/mncs-language/actions/workflows/ci.yml/badge.svg)](https://github.com/epi13/mncs-language/actions/workflows/ci.yml)

Research and reference implementation for a general-purpose, verification-native programming language built on Machine-Native Complexity Standard (MNCS) principles.

> **Project status:** the 0.1 executable semantic model now has an experimental 0.2 foundation for canonical form, stable semantic identities, graph dependencies, evidence invalidation, recursive artifacts, and authority-aware deltas. The supported 0.3 subset now has CFG-correct executable bodies, typed integer comparison, deterministic HIR, and a small validated 0.4 SSA artifact with body→HIR→SSA provenance and evidence-aware transformation refusal. The first `mncs-compiler` driver connects that existing spine through versioned request/result/evidence contracts, explicit host/build/target/run identities, and deterministic CLI emissions. Versioned, bounded body and independent SSA reference evaluators support cross-layer conformance and finite refactoring studies. Forge development evidence is configured locally; no final grammar, compiler stability, executable backend, native Raspberry Pi result, independent evaluation, universal equivalence, or production suitability is claimed.

## Why this project exists

Conventional languages primarily describe operations, data, and control flow. Verification tools then reconstruct behavioral relationships after the program has been written. This project investigates the opposite arrangement: contracts, effects, capabilities, assumptions, failure semantics, resource bounds, machine intent, and evidence are part of the program's meaning from the beginning.

The intended language is **machine-native without being machine-exclusive**. Humans, agents, compilers, auditors, and independent verifiers should be able to inspect the same semantic structure.

## Core proposition

An MNCS-oriented program should make it possible to answer:

1. What does this component claim?
2. What state may it observe or modify?
3. What authority does it possess?
4. What assumptions does it depend upon?
5. What evidence supports each property?
6. Where does proof stop and trust begin?
7. What must be re-verified when the component changes?
8. How can the system diagnose and improve itself without laundering evidence or silently broadening authority?
9. Which machine-oriented transformations are permitted, and what obligations justify them?

## What is in this repository

- `docs/` — vision, architecture, syntax research, machine-intent expressions, recursive refinement, terminology, trust model, and explicit non-goals.
- `spec/` — early normative semantic and representation documents.
- `rfcs/` — design proposals that can evolve independently of the specification.
- `crates/mncs-model/` — the executable semantic model, canonical representation, identities, graph/CFG, evidence manifest, invalidation logic, recursive artifacts, high-level IR, validated SSA artifact, and narrow machine-intent/verifier pilots.
- `crates/mncs-compiler/` — the evidence-bearing reference compiler driver over the existing semantic, HIR, SSA, obligation, identity, and provenance APIs.
- `crates/mncs-syntax/` — deterministic, tokenizer-neutral source representation metrics.
- `crates/mncs-cli/` — validation, canonicalization, identity, graph/CFG, evidence, IR, SSA, obligation, verification, comparison, diagnostic, diff, and syntax-tournament commands.
- `examples/` — semantic manifests, competing source candidates, canonical semantic forms, machine-intent sketches, and semantic patches.

The JSON manifests and source candidates are experimental transport and research representations. They are not yet a selected production grammar.

The complete logical compiler stage contract, including explicitly missing frontend and executable-backend stages, is available as deterministic machine JSON:

```bash
cargo run -p mncs-cli -- compiler-architecture
```

See [the 2026-08 compiler architecture assessment](docs/compiler-architecture-assessment-2026-08.md) and [RFC 0039](rfcs/0039-machine-native-compiler-stage-and-experiment-contracts.md).

## Current semantic objects

The 0.1 model contains:

- modules and functions;
- typed inputs and outputs;
- contract clauses;
- declared effects;
- required capabilities;
- explicit assumptions;
- evidence claims and verifier identities;
- failure modes;
- machine-readable validation diagnostics.

A key initial rule is that every effect must identify an authorizing capability, and that capability must be declared by the function. Evidence must reference a declared contract property rather than floating as unbound metadata.

The experimental 0.2 layer adds deterministic SHA-256 content fingerprints, structural semantic identities, a typed semantic graph, dependency-aware evidence manifests, conservative invalidation, diagnostic obligations, backward causal slices, finite refinement budgets, typed isolated repair patches, explicit promotion records, semantic/authority/evidence deltas, and versioned verifier artifact import checks. The 0.3 layer accepts a small syntax-independent executable body model, computes deterministic CFG reachability/dominance, and lowers symbolic values, state regions, normal and failure paths, explicit effect capability uses, contract/evidence traceability, generated obligations, machine-intent links, lowering/portability envelopes, and transformation provenance into HIR. The experimental 0.4 layer projects that subset into validated block-parameter SSA while retaining semantic/HIR/SSA identities and refusing a stronger no-overflow transformation without exact current verifier evidence. Forge is a local development/evidence harness, not a sandbox or independent evaluator. This is not a production compiler or backend.

## Try the semantic prototype

```bash
cargo run -p mncs-cli -- validate examples/account-transfer.mncs.json
```

The command emits a JSON validation report. The valid example should exit successfully.

The following example intentionally performs a network effect without declaring the required capability and should be rejected:

```bash
cargo run -p mncs-cli -- validate examples/invalid-undeclared-effect.mncs.json
```

Explore the semantic foundation with the small before/after examples:

```bash
cargo run -p mncs-cli -- canonicalize examples/semantic-foundation/before.mncs.json
cargo run -p mncs-cli -- graph examples/semantic-foundation/before.mncs.json
cargo run -p mncs-cli -- diff \
  examples/semantic-foundation/before.mncs.json \
  examples/semantic-foundation/after.mncs.json
cargo run -p mncs-cli -- evidence-manifest examples/semantic-foundation/before.mncs.json > /tmp/mncs-evidence.json
cargo run -p mncs-cli -- evidence-check /tmp/mncs-evidence.json \
  examples/semantic-foundation/after.mncs.json
cargo run -p mncs-cli -- ir examples/account-transfer.mncs.json
cargo run -p mncs-cli -- obligations examples/account-transfer.mncs.json
cargo run -p mncs-cli -- verify examples/account-transfer.mncs.json
cargo run -p mncs-cli -- obligations examples/ir/unknown-contract-evidence.mncs.json
cargo run -p mncs-cli -- verify examples/ir/unknown-contract-evidence.mncs.json  # exits 1: UNKNOWN is unresolved
cargo run -p mncs-cli -- diagnose examples/invalid-undeclared-effect.mncs.json
cargo run -p mncs-cli -- compare \
  examples/semantic-foundation/before.mncs.json \
  examples/semantic-foundation/after.mncs.json
cargo run -p mncs-cli -- body examples/executable/checked-add.mncs.json
cargo run -p mncs-cli -- ssa examples/executable/checked-add.mncs.json
cargo run -p mncs-cli -- trace examples/executable/checked-add.mncs.json
cargo run -p mncs-cli -- verifier-request examples/executable/checked-add.mncs.json
cargo run -p mncs-cli -- execute \
  examples/executable/checked-add.mncs.json \
  examples/execution/checked-add-request.json
cargo run -p mncs-cli -- execute-ssa \
  examples/executable/checked-add.mncs.json \
  examples/execution/checked-add-request.json
cargo run -p mncs-cli -- check-lowering-execution \
  examples/execution/bounded-sum-baseline.mncs.json \
  examples/execution/bounded-sum-corpus.json
cargo run -p mncs-cli -- compare-execution \
  examples/execution/bounded-sum-baseline.mncs.json \
  examples/execution/bounded-sum-equivalent-refactor.mncs.json \
  examples/execution/bounded-sum-corpus.json
cargo run -p mncs-cli -- compare-execution \
  examples/execution/bounded-sum-baseline.mncs.json \
  examples/execution/bounded-sum-regression.mncs.json \
  examples/execution/bounded-sum-corpus.json  # exits 1: mismatch detected
cargo run -p mncs-cli -- compare-execution \
  examples/execution/bounded-range-baseline.mncs.json \
  examples/execution/bounded-range-equivalent-refactor.mncs.json \
  examples/execution/bounded-range-corpus.json
cargo run -p mncs-cli -- compile \
  examples/compiler-study/bounded-sum.mncs.json \
  --emit semantic,hir,ssa,evidence
cargo run -p mncs-cli -- compiler-study \
  examples/compiler-study/bounded-sum.mncs.json \
  --node-id local-linux
```

The local contract-expression change changes the contract and function fingerprints and marks the dependent evidence stale. Canonicalization sorts set-like declarations while preserving ordered inputs and outputs. `execute` is a research interpreter for validated executable bodies and `execute-ssa` independently interprets the lowered SSA artifact; both have finite step budgets, bounded traces, conservative effects, and explicit unsupported/runtime-failure/budget-exhausted outcomes. `check-lowering-execution` compares the two reference paths over a corpus. These reports are finite behavioral evidence, not universal equivalence, compiler correctness, or backend validation.

The IR command emits a deterministic, inspectable projection with state regions, capability uses, failure blocks, generated obligations, semantic-to-IR traceability, and transformation provenance. `body` includes a deterministic CFG projection; `ssa` emits the experimental validated block-parameter SSA artifact. `compile` reuses those exact implementations and emits a structured compiler result plus an evidence bundle whose transformation edges bind exact artifact and pass identities. `compiler-study` keeps node/environment observations separate from the semantic, HIR, and SSA fingerprints expected to agree across compatible hosts. Executable-body manifests lower symbolic operations directly; `verifier-request` and `verify-result` exchange identity- and dependency-bound JSON artifacts. `verify` exercises the pure local verifier; `diagnose` emits a conservative backward slice when graph construction is available and a dependency-only slice for invalid input. Candidate evaluation remains isolated and does not promote or rewrite the trusted baseline.

## Forge development harness

`mncs-forge.toml` configures the installed MNCS Forge Provider Protocol adapter in
`tools/mncs_language_forge_provider.py`. Its bounded development workflows exercise Rust quality,
semantic fixtures, HIR trace integrity, SSA integrity, evidence freshness, bounded body execution,
independent SSA execution, and the bounded-sum/bounded-range lowering-consistency studies. Forge records are
development evidence only: local provider execution is not sandboxed, process success alone is not
proof, and no evaluator or promotion authority is claimed. The portable execution-bundle fixture
under `compatibility/execution-bundle/` is checked with the sibling MNCS standard tool and is not a
Forge bundle or an execution-assurance receipt.

## Explore the source syntax laboratory

The current hypothesis uses three related representations:

```text
Zig-influenced human source
        ↕ deterministic semantic round trip
compact canonical semantic form
        ↓
high-level MNCS IR and verified SSA
```

Recursive tooling also exchanges semantic patches that target stable graph identities rather than unrestricted source-text replacement.

Run the account-transfer tournament:

```bash
cargo run -p mncs-cli -- syntax-tournament \
  examples/syntax/account-transfer.tournament.json
```

The tournament compares three human-source candidates and one canonical machine form against the same 23 declared semantic claims. It reports deterministic lexical units and non-whitespace characters per claim. These are tokenizer-neutral comparison metrics, not exact token counts for a particular model.

See [Source Syntax Laboratory](docs/source-syntax-lab.md), [Source Representations](spec/source-representations.md), and [RFC 0005](rfcs/0005-source-representations-and-semantic-density.md).

## Machine-intent expressions

The experimental pilot makes useful low-level behavior explicit rather than relying on accidental source patterns or backend folklore. The bounded scalar subset now models addition, subtraction, multiplication, bitwise AND/OR/XOR, typed comparisons, explicit arithmetic intents, and the identity/obligation boundary; shifts, rotates, memory, and broader expression families remain design work.

For example, a future language should distinguish:

```text
add.wrap<i32>(a,b)
add.checked<i32>(a,b)
add.saturate<i32>(a,b)
add.trap<i32>(a,b)
```

rather than leaving overflow behavior implicit.

It should also distinguish an optimization preference from a security requirement:

```text
select.branchless(condition,a,b)
select.constant_time(condition,a,b,model=target_timing_model)
```

A machine-intent expression combines:

```text
exact semantics
+ machine intent
+ required invariants and capabilities
+ permitted lowering envelope
+ evidence and target envelope
```

The compiler may select among jump tables, decision trees, vector realizations, data layouts, or portable fallbacks only after the relevant obligations are accounted for. Missing evidence produces conservative lowering, explicit fallback, or `UNKNOWN`; it does not authorize optimistic backend metadata.

See [Machine-intent expressions](docs/machine-intent-expressions.md), the
[0.1 specification direction](spec/machine-intent-expressions.md), the
[canonical semantic sketch](examples/canonical/machine-intent-expressions.mncs-sem), and
[RFC 0006](rfcs/0006-machine-intent-expressions.md).

## Intended compilation model

The current direction is:

```text
human source syntax (experimental candidates)
        ↕
canonical semantic graph/form
        ↓
obligation generation and evidence resolution
        ↓
high-level MNCS IR
        ↓
verified SSA IR
        ↓
bounded deterministic body reference execution
        ↕ cross-layer comparison
bounded deterministic SSA reference execution
        ↓
LLVM IR / Cranelift IR / another backend
        ↓
machine code + evidence manifest
```

The important design boundary is between the verification-native semantic layers and a conventional optimization backend. Backend claims such as non-aliasing, no-overflow, in-bounds access, alignment, or floating-point relaxation should ideally be emitted only when established by language rules or current evidence rather than inserted optimistically.

Semantically required machine intent must survive lowering as traceable operations and obligations, not only as discardable debugging metadata.

## Recursive debugging and self-improvement

Recursive refinement is a foundational language requirement, not only an orchestration feature supplied by the Forge or RAVEL. The language should expose its semantic graph, evidence graph, diagnostics, causal slices, transformation history, machine-intent operations, and lowering obligations in forms that can be consumed by later verification and repair cycles.

The intended loop is bounded and evidence-driven:

```text
observe semantic and evidence state
        ↓
localize a failed or weak obligation
        ↓
propose a repair in an isolated candidate state
        ↓
declare intended improvements and protected properties
        ↓
run independent targeted verifiers
        ↓
compare semantic, authority, complexity, target, lowering, and evidence deltas
        ↓
promote or reject under explicit policy
        ↺
```

A generator must not silently modify the trusted baseline or certify its own repair merely by repeating the cycle. Recursion depth, authority, mutation scope, candidate count, verifier calls, resource use, target envelope, and realization choices must be bounded. See [Recursive Debugging and Refinement](docs/recursive-refinement.md) and [RFC 0004](rfcs/0004-recursive-introspection-and-refinement.md).

A repeatedly successful non-orthodox technique may eventually graduate from an intentional deviation to an experimental semantic operation and later to a standardized expression. The system must retain the complete semantics, invariants, target limits, failure modes, and evidence—not merely copy the unusual syntax or instruction sequence.

## Relationship to the MNCS project family

- **MNCS** defines the broader standard, contracts, complexity concepts, and verification philosophy.
- **MNCDS** explores deterministic and structural representation where applicable.
- **MNCS Language** investigates how those relationships can be expressed directly in a general-purpose language, including semantic structures for machine intent, verified lowering, recursive introspection, and repair.
- **MNCS Forge** can analyze, verify, localize failures, discharge bounded lowering obligations, test candidate transformations, and produce evidence for MNCS-language components and conventional code.
- **RAVEL** can coordinate recursive, distributed, multi-agent, multi-target, and multi-verifier refinement across machines and trust boundaries.

MNCS must remain applicable to existing languages even if this project never becomes production-ready. The standard therefore does not depend on this language project.

## Design principles

1. **Semantics before syntax.** Surface choices must preserve the semantic model rather than define it accidentally.
2. **Explicit authority.** Ambient filesystem, network, process, clock, randomness, and credential access should not be assumed.
3. **Closed effects.** Undeclared effects should be rejected or isolated at a visible trust boundary.
4. **Named assumptions.** Verification results must state the assumptions on which they depend.
5. **Evidence is addressable.** Every claim should identify its property, verifier, status, and optional artifact.
6. **Proof gradients are honest.** Tested, analyzed, verified, and externally verified are different states.
7. **Unsafe behavior is bounded.** Low-level operations may exist, but their boundary and obligations must be explicit.
8. **Incremental verification.** A change should invalidate the smallest defensible evidence subgraph.
9. **Human inspectability.** Machine-native structure must remain understandable without an LLM.
10. **Backend conservatism.** Optimization promises should be generated from established facts.
11. **Machine intent is explicit.** Edge behavior, transformation freedom, target limits, and required invariants should not depend on source accidents.
12. **Facts, preferences, and requirements remain separate.** A performance preference cannot satisfy a semantic or security obligation.
13. **Recursive refinement is bounded and reviewable.** Promotion requires explicit policy, protected-property checks, and sufficient independent evidence.
14. **Semantic density beats character density.** Representation efficiency is measured against complete, equivalent claims rather than raw brevity.
15. **Role-specific representations are allowed.** Human source, canonical agent form, semantic patches, and verified IR need not use the same notation.

## Roadmap

The current implementation is progressing through **Milestone 0.2 — Semantic graph, identity, and recursive artifacts** and the first part of **Milestone 0.3 — High-level MNCS IR** while retaining the 0.1 model and syntax laboratory. See [ROADMAP.md](ROADMAP.md).

## Contributing

The project is early enough that a precise counterexample is often more valuable than a large implementation. Read [CONTRIBUTING.md](CONTRIBUTING.md), then use an RFC for changes that alter the semantic model or source representation requirements.

## License

Apache License 2.0. See [LICENSE](LICENSE).
