# Compiler Pipeline Contracts 0.1

## 1. Scope

This document specifies the current research contract for compiler architecture disclosure and compiler experiment observations. It does not select a surface grammar, backend, executable format, assurance profile, or conformance profile.

## 2. Logical stage architecture

An MNCS compiler architecture MUST disclose these logical stages in order:

1. source / intent representation;
2. lexical analysis;
3. parsing;
4. concrete syntax tree;
5. abstract syntax tree;
6. semantic graph construction;
7. identity resolution;
8. type and contract analysis;
9. high-level intermediate representation;
10. verification passes;
11. lowering / backend generation; and
12. executable artifact.

An implementation MAY fuse stages, but fusion MUST NOT remove their contract records. An unavailable stage MUST be reported as unavailable, planned, partial, experimental, or bootstrap-only rather than inferred from a later artifact.

The current architecture contract ID is:

```text
mncs:language:compiler-stage-architecture:0.1
```

Each stage record MUST identify its availability, integration boundary, input and output contracts, determinism requirement, owner, material decisions, and blockers. The architecture record MUST have deterministic content identity.

## 3. Artifact independence from files

Compiler contracts MUST identify artifacts by semantic/content identity and role. A path or source filename MAY locate bytes but MUST NOT define semantic identity.

Source contract development MUST allow artifacts representing programs, models, workflows, verification systems, infrastructure definitions, autonomous components, and machine processes. Their relationships and provenance MUST remain explicit.

## 4. Compiler study result

The current compiler experiment observation contract ID is:

```text
mncs:language:compilation-study-result:0.1
```

A result MUST bind:

- study, request, run, compiler, pipeline, compiler-host, and build-host identities;
- target identity when one is resolved;
- compiler completion status;
- emitted stage fingerprints;
- exact pass-edge, pass, input-artifact, and output-artifact identities;
- pass-local `PASS`, `FAIL`, or `UNKNOWN` state;
- unresolved obligations and assumptions;
- diagnostics and referenced execution results; and
- cross-host comparison invariants.

The result MUST declare `observation_only_not_assurance_or_conformance`. Its deterministic identity MUST cover that declaration and all other serialized fields.

## 5. Interpretation

Compilation completion means the compiler driver completed its declared bounded path. It does not imply:

- all obligations are resolved;
- an executable artifact exists;
- every transformation is correct;
- independent assurance exists;
- an MNCS conformance profile is satisfied; or
- a candidate is authorized for promotion.

Pass status belongs to the exact transformation edge that emitted it. Consumers MUST NOT promote pass-local status into assurance or conformance without separately identified policy and evidence.

## 6. Current implementation envelope

The semantic JSON transport decode, lexical metrics, semantic graph, identity inventory, structural validator, HIR, SSA, target plan, and reference evaluators are research components with different maturity.

In particular:

- lexical analysis does not yet emit a compiler token stream;
- no lossless CST or representation-neutral AST exists;
- scope/binding resolution and a complete type system do not exist;
- semantic graph and identity inventory are library capabilities not yet emitted by the compiler driver;
- verification is structural and bounded, not complete translation validation;
- target lowering emits an unresolved plan when facts are absent; and
- no portable/native backend or executable artifact is implemented.

The machine-readable architecture record is authoritative over informal maturity summaries.
