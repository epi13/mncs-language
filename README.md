# MNCS Language Project

[![CI](https://github.com/epi13/mncs-language/actions/workflows/ci.yml/badge.svg)](https://github.com/epi13/mncs-language/actions/workflows/ci.yml)

Research and reference implementation for a general-purpose, verification-native programming language built on Machine-Native Complexity Standard (MNCS) principles.

> **Project status:** semantic-model research and executable prototype. No surface-language syntax, compiler stability, or production suitability is claimed yet.

## Why this project exists

Conventional languages primarily describe operations, data, and control flow. Verification tools then reconstruct behavioral relationships after the program has been written. This project investigates the opposite arrangement: contracts, effects, capabilities, assumptions, failure semantics, resource bounds, and evidence are part of the program's meaning from the beginning.

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

## What is in this repository

- `docs/` — vision, architecture, recursive refinement, terminology, trust model, and explicit non-goals.
- `spec/` — early normative semantic documents.
- `rfcs/` — design proposals that can evolve independently of the specification.
- `crates/mncs-model/` — an executable Rust model of the initial semantic objects and validation rules.
- `crates/mncs-cli/` — a small validator for experimental JSON semantic manifests.
- `examples/` — one accepted manifest and one intentionally rejected manifest.

The JSON manifests are not proposed source syntax. They are a temporary, inspectable transport representation used to test the semantic model before committing to a parser or grammar.

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

## Try the prototype

```bash
cargo run -p mncs-cli -- validate examples/account-transfer.mncs.json
```

The command emits a JSON validation report. The valid example should exit successfully.

The following example intentionally performs a network effect without declaring the required capability and should be rejected:

```bash
cargo run -p mncs-cli -- validate examples/invalid-undeclared-effect.mncs.json
```

## Intended compilation model

The current direction is:

```text
source syntax (undecided)
        ↓
MNCS semantic graph
        ↓
high-level MNCS IR
        ↓
verified SSA IR
        ↓
LLVM IR / Cranelift IR / another backend
        ↓
machine code + evidence manifest
```

The important design boundary is between the verification-native semantic layers and a conventional optimization backend. Backend claims such as non-aliasing, no-overflow, or in-bounds access should ideally be emitted only when established by evidence rather than inserted optimistically.

## Recursive debugging and self-improvement

Recursive refinement is a foundational language requirement, not only an orchestration feature supplied by the Forge or RAVEL. The language should expose its semantic graph, evidence graph, diagnostics, causal slices, and transformation history in forms that can be consumed by later verification and repair cycles.

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
compare semantic, authority, complexity, and evidence deltas
        ↓
promote or reject under explicit policy
        ↺
```

A generator must not silently modify the trusted baseline or certify its own repair merely by repeating the cycle. Recursion depth, authority, mutation scope, candidate count, verifier calls, and resource use must be bounded. See [Recursive Debugging and Refinement](docs/recursive-refinement.md) and [RFC 0004](rfcs/0004-recursive-introspection-and-refinement.md).

## Relationship to the MNCS project family

- **MNCS** defines the broader standard, contracts, complexity concepts, and verification philosophy.
- **MNCDS** explores deterministic and structural representation where applicable.
- **MNCS Language** investigates how those relationships can be expressed directly in a general-purpose language, including the semantic structures needed for recursive introspection and repair.
- **MNCS Forge** can analyze, verify, localize failures, test candidate transformations, and produce evidence for MNCS-language components and conventional code.
- **RAVEL** can coordinate recursive, distributed, multi-agent, and multi-verifier refinement across machines and trust boundaries.

MNCS must remain applicable to existing languages even if this project never becomes production-ready. The standard therefore does not depend on this language project.

## Design principles

1. **Semantics before syntax.** The project will not optimize punctuation before establishing what must be represented.
2. **Explicit authority.** Ambient filesystem, network, process, clock, randomness, and credential access should not be assumed.
3. **Closed effects.** Undeclared effects should be rejected or isolated at a visible trust boundary.
4. **Named assumptions.** Verification results must state the assumptions on which they depend.
5. **Evidence is addressable.** Every claim should identify its property, verifier, status, and optional artifact.
6. **Proof gradients are honest.** Tested, analyzed, verified, and externally verified are different states.
7. **Unsafe behavior is bounded.** Low-level operations may exist, but their boundary and obligations must be explicit.
8. **Incremental verification.** A change should invalidate the smallest defensible evidence subgraph.
9. **Human inspectability.** Machine-native structure must remain understandable without an LLM.
10. **Backend conservatism.** Optimization promises should be generated from established facts.
11. **Recursive refinement is bounded and reviewable.** Diagnostics and repair proposals may feed later cycles, but promotion requires explicit policy, protected-property checks, and sufficient independent evidence.

## Roadmap

The immediate target is **Milestone 0.1 — Executable Semantic Model**. See [ROADMAP.md](ROADMAP.md) for acceptance criteria and later phases.

## Contributing

The project is early enough that a precise counterexample is often more valuable than a large implementation. Read [CONTRIBUTING.md](CONTRIBUTING.md), then use an RFC for changes that alter the semantic model.

## License

Apache License 2.0. See [LICENSE](LICENSE).
