# MNCS Language Project

[![CI](https://github.com/epi13/mncs-language/actions/workflows/ci.yml/badge.svg)](https://github.com/epi13/mncs-language/actions/workflows/ci.yml)

Research and reference implementation for a general-purpose, verification-native programming language built on Machine-Native Complexity Standard (MNCS) principles.

> **Project status:** the 0.1 executable semantic model now has an experimental 0.2 foundation for canonical form, stable semantic identities, graph dependencies, evidence invalidation, recursive artifacts, and authority-aware deltas. An early 0.3 high-level IR, deterministic lowering, obligation generation, and local verifier protocol are executable for the supported manifest subset. No final grammar, compiler stability, or production suitability is claimed yet.

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
- `crates/mncs-model/` — the executable semantic model, canonical representation, identities, graph, evidence manifest, invalidation logic, recursive artifacts, high-level IR, and narrow machine-intent/verifier pilots.
- `crates/mncs-syntax/` — deterministic, tokenizer-neutral source representation metrics.
- `crates/mncs-cli/` — validation, canonicalization, identity, graph, evidence, IR, obligation, verification, comparison, diagnostic, diff, and syntax-tournament commands.
- `examples/` — semantic manifests, competing source candidates, canonical semantic forms, machine-intent sketches, and semantic patches.

The JSON manifests and source candidates are experimental transport and research representations. They are not yet a selected production grammar.

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

The experimental 0.2 layer adds deterministic SHA-256 content fingerprints, structural semantic identities, a typed semantic graph, dependency-aware evidence manifests, conservative invalidation, diagnostic obligations, graph-based causal slices, finite refinement budgets, untrusted isolated repair proposals, explicit promotion records, and semantic/authority/evidence deltas. The early 0.3 layer lowers the supported manifest subset into deterministic high-level IR with typed values, state regions, normal and failure paths, explicit effect capability uses, contract/evidence traceability, generated obligations, a verifier request/result protocol, and a deterministic local verifier. It also contains a deliberately small machine-intent pilot: explicit integer addition intents, PASS/FAIL/UNKNOWN obligations, alignment capabilities bound to subject and scope, intent-preserving IR records, and a no-overflow backend-promise decision. This is not a production compiler or backend.

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
```

The local contract-expression change changes the contract and function fingerprints and marks the dependent evidence stale. Canonicalization sorts set-like declarations while preserving ordered inputs and outputs.

The IR command emits a deterministic, inspectable projection with state regions, capability uses, failure blocks, generated obligations, and a semantic-to-IR trace map. `verify` exercises the pure local capability/effect verifier; `diagnose` retains a conservative dependency-only slice when an invalid manifest prevents graph construction. The isolated repair proposal and promotion types are currently library-level artifacts, not an automatic source-rewrite command.

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

The experimental pilot makes useful low-level behavior explicit rather than relying on accidental source patterns or backend folklore. It currently models integer addition and the identity/obligation boundary; the broader expression families remain design work.

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
