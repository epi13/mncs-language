# MNCS Compiler Implementation Roadmap

This document turns RFC 0038 into an executable implementation sequence for the current `mncs-language` repository.

It is intentionally implementation-oriented. RFC 0038 owns the architectural semantics; this document describes how to grow the reference compiler without replacing the semantic work already present in `mncs-model`.

## Current starting point

The repository already has more of a compiler spine than the crate layout suggests:

- executable semantic bodies;
- canonical identities and serialization;
- semantic graphs and transformation provenance;
- HIR generation;
- CFG validation;
- block-parameter SSA;
- obligations and verifier request/result artifacts;
- bounded body reference execution;
- independent bounded SSA execution;
- body/HIR/SSA consistency studies;
- evidence freshness and candidate-evaluation workflows;
- a Forge provider interface.

The compiler should therefore be built by **connecting and hardening the existing semantic spine**, not by introducing a separate conventional AST/IR stack that becomes a competing definition of the language.

## Implementation status after the first compiler tranche

The repository now implements the first bounded compiler tranche for the existing executable semantic subset:

- **C0 implemented:** `mncs-model` owns versioned compiler request, result, evidence, pass, transformation-edge, host/build/target/run, target-plan, diagnostic, study, and backend-boundary artifacts. Their identities use deterministic serialization and distinct identity domains. Integrity checks reject laundered identities, wrong transformation endpoints, stale backend bindings, omitted target/backend assumptions, and unresolved-obligation references not retained by the bundle.
- **C1 implemented for the current subset:** `mncs-compiler` validates an exact semantic input and orchestrates the existing `Program::lower_to_ir` and `Program::lower_to_ssa` implementations. `mncs compile` emits deterministic semantic, HIR, SSA, target-plan, and evidence JSON without defining a second HIR or SSA. Invalid semantics do not reach HIR/SSA, and unresolved obligations remain visible with conservative selection.
- **C2 repository boundary implemented; native study pending:** Linux and Windows CI build/test and run the compiler workflows. The Linux-to-AArch64 job remains a compile-shape check only. `mncs compiler-study` and `examples/compiler-study/bounded-sum.mncs.json` provide the machine-readable Fabric-side interface for native Raspberry Pi execution, but no Raspberry Pi result is claimed in this repository state.
- **C3 boundary only:** backend identity/configuration/result/evidence and `TargetLoweringPlan` are executable schemas. A named target with missing layout/ABI/integer/trap evidence produces `UNKNOWN` and no backend artifact. No portable or native executable backend is implemented yet.

The target-independent compiler artifacts intentionally exclude timestamps, absolute paths, usernames, hostnames, process IDs, and host observations. Compiler request/result and study identities include their derivation context; semantic, HIR, and SSA fingerprints do not.

The stage audit in RFC 0039 now makes the complete logical pipeline machine-readable through `mncs compiler-architecture`. It explicitly reports lexical analysis, CST, AST, and executable generation as planned rather than treating the semantic JSON bootstrap path as their implementation. Compiler study results now expose a language-owned observation contract with per-stage fingerprints, pass execution records, and unresolved obligations for Forge comparison.

## Ownership boundary

```text
mncs-language
    owns semantic legality, compiler contracts, IR relations,
    validation protocols, target-lowering contracts, and evidence schemas

Forge
    owns candidate search, pass exploration, generated transformations,
    equality search, superoptimization, and realization tournaments

Fabric
    owns distributed execution of compiler studies across available machines,
    environment observation, measurement collection, and result transport
```

Short form:

> The language owns legality. Forge owns search. Fabric owns distributed study.

## Compiler artifact flow

```text
source + LanguageProfile
        |
        v
parse / bind / elaborate
        |
        v
ElaboratedSemanticArtifact
        |
        v
ExecutableSemanticArtifact
        |
        v
Canonical HIR
        |
        v
Lowered HIR
        |
        v
Canonical SSA
        |
        v
Selected SSA
        |
        v
TargetLoweringPlan
        |
        v
backend adapter
        |
        v
object / portable module
        |
        v
linked executable
        +
CompilationEvidenceBundle
```

Every semantically meaningful arrow is a transformation edge with identity, provenance, an RFC 0020 relation claim, assumptions, obligations, and validation state.

## Proposed implementation boundaries

Do not split crates merely to mirror the diagram. Add a boundary only when it creates a clean dependency direction.

### Existing crates

`mncs-model`
: Continue to own the semantic artifact vocabulary and current HIR/SSA research representations. Compiler orchestration must depend on these definitions rather than duplicate them.

`mncs-syntax`
: Continue source-representation/parsing experiments. Final parsing remains downstream of RFC 0035/0036 language-profile decisions.

`mncs-cli`
: Remain the initial entry point for compiler inspection and research commands.

### Proposed incremental crates

`mncs-compiler`
: First new compiler crate. Orchestrates the existing semantic -> HIR -> SSA chain and emits versioned compilation request/result/evidence artifacts.

`mncs-target`
: Added after the compiler driver exists. Adapts RFC 0017 target/environment facts into compiler target contracts while preserving host/build/target/run separation.

`mncs-codegen`
: Added when the first executable backend is introduced. Defines backend-neutral adapter contracts and keeps specific backend implementations replaceable.

`mncs-translation-check`
: Added only after at least one validator has enough independent logic to justify a separate trust/dependency boundary.

## Platform model

Platform support must not be represented by one `platform` string.

Each compiler study records:

```text
compiler_host
build_host
target_contract
run_environment
backend
linker/toolchain
```

### Current baseline host laboratory

| Host | Native study role | Required early behavior |
| --- | --- | --- |
| Linux x86-64 | primary development + native Linux | build, validate, HIR, SSA, evidence |
| Windows x86-64 | independent host + Windows ABI study | build, validate, HIR, SSA, evidence |
| Raspberry Pi Linux/ARM | architecture-diverse host + ARM study | build, validate, HIR, SSA, evidence through Fabric |

If the connected Raspberry Pi uses a 32-bit userspace, create an ARM32 profile. Do not silently treat it as the AArch64 profile.

### First target families

1. **Portable executable baseline** — candidate WebAssembly/WASI path for same-artifact execution across current Fabric hosts.
2. **Linux x86-64 native**.
3. **Windows x86-64 native**.
4. **Linux AArch64 native** for Raspberry Pi-class systems where the node profile supports it.

Native host/target compilation is the first requirement. Cross-compilation is a second experiment so missing external linkers/sysroots do not block the core compiler study.

## Implementation phases

### C0 — Compiler contract objects

Implement versioned research artifacts for:

- `CompilationRequest`;
- `CompilationResult`;
- `CompilationEvidenceBundle`;
- `CompilerPassIdentity`;
- `TransformationEdge`;
- `CompilerHostIdentity`;
- `BuildHostIdentity`;
- `TargetContractRef`;
- `RunEnvironmentRef`;
- `PassPipelineIdentity`;
- `TargetLoweringPlan`.

Acceptance:

- deterministic canonical serialization;
- distinct fingerprints for host/build/target/run changes;
- negative fixtures reject identity laundering;
- compiler request/result round trip through CLI.

### C1 — Reference compiler driver

Add `mncs-compiler` and make it orchestrate the already-existing lowering path.

Initial command direction:

```text
mncs compile PROGRAM \
    --emit semantic,hir,ssa,evidence
```

Do not add native code generation yet.

Acceptance:

- one request produces a deterministic artifact chain;
- HIR/SSA fingerprints match existing CLI commands;
- transformation provenance is preserved;
- unresolved obligations appear in the evidence bundle rather than disappearing.

### C2 — Host portability gate

Make compiler-host portability an architectural requirement before backend work deepens.

GitHub CI:

- Linux x86-64 workspace build/test;
- Windows x86-64 workspace build/test;
- Linux-to-AArch64 `cargo check` shape gate for accidental host-only dependencies.

Fabric:

- native Raspberry Pi build/test run;
- node profile captured with architecture/OS/toolchain identities;
- same semantic fixture fingerprinted on all three host classes.

Acceptance:

- same canonical semantic input has the same semantic identity across hosts;
- any host-dependent output before target resolution is treated as a bug or explicitly explained dependency.

### C3 — Portable executable backend

Prototype one portable backend behind the codegen adapter.

WebAssembly/WASI is the preferred first experiment, not a permanent requirement.

Acceptance:

- bounded scalar/control-flow subset emits an executable portable artifact;
- Linux, Windows, and Raspberry Pi execute the same frozen corpus through Fabric;
- body, SSA, and portable-backend results are compared as bounded agreement only;
- mismatches produce structured counterexamples.

### C4 — Native backend experiment

Implement one replaceable native adapter or conduct a bounded backend tournament before selection.

Candidate families include Cranelift and LLVM.

Required initial native targets:

- Linux x86-64;
- Windows x86-64;
- Linux AArch64.

Acceptance:

- backend identity/version included in evidence;
- target assumptions explicit;
- no aggressive target-dependent optimization required yet;
- one kernel runs successfully on all three native target families.

### C5 — Translation validation

Create the first concrete transformation validator.

Start narrow, for example:

- constant folding with explicit overflow semantics;
- no-overflow-elision already represented in the prototype;
- simple CFG simplification;
- one target lowering promise.

Acceptance:

- one valid candidate -> `PASS`;
- one invalid candidate -> `FAIL` with counterexample/diagnostic;
- one unsupported case -> `UNKNOWN`;
- `UNKNOWN` cannot promote the stronger candidate;
- validator identity and dependency freshness are recorded.

### C6 — Conservative analysis layer

Add abstract interpretation incrementally.

First domains:

- integer range/overflow;
- one memory/provenance/alias fact;
- one target capability fact.

Acceptance:

- joins preserve uncertainty;
- analysis facts have dependency identities;
- stale facts invalidate only dependent transforms where possible;
- a missing fact retains a check or conservative lowering.

### C7 — Forge candidate protocol

Extend the existing Forge provider boundary so Forge can propose bounded compiler candidates without gaining correctness authority.

Candidate classes:

- pass ordering;
- SSA rewrite;
- representation selection;
- backend selection;
- target specialization;
- later equality-saturation/superoptimization candidates.

Acceptance:

- candidate written outside trusted baseline;
- compiler derives obligations;
- independent validator/policy decides promotion;
- one faster invalid candidate is rejected;
- one valid candidate is accepted.

### C8 — Fabric cross-platform compiler study

Run a complete study across the current Fabric machines.

For one semantic program capture:

- semantic fingerprint;
- HIR fingerprint;
- SSA fingerprint;
- compiler host/build identities;
- target/backend identities;
- final artifact fingerprints;
- bounded execution traces/results;
- compile time;
- artifact/code size;
- runtime measurements;
- all unresolved assumptions.

The comparison should localize the earliest divergence in the compiler chain.

Acceptance:

- target-independent artifacts agree where required;
- target-specific artifacts diverge only after an explicit target boundary;
- all native results satisfy the declared protected relation or produce retained counterexamples;
- one intentionally host-sensitive fixture exposes a hidden assumption.

### C9 — Incremental compilation

Use content identities and dependency edges instead of timestamp-oriented invalidation.

Acceptance:

- local semantic edit reuses unaffected compiler artifacts;
- target-profile change preserves target-independent artifacts;
- backend upgrade invalidates backend-dependent evidence without invalidating semantic identity;
- validator change invalidates only evidence dependent on that validator identity.

### C10 — Machine-directed specialization

Expose legal unresolved realization choices to Forge.

Examples:

- representation;
- dispatch;
- vectorization;
- allocator/runtime provider;
- target feature realization;
- scheduling realization;
- backend strategy.

Every specialization remains an evidence-bearing refinement.

### C11 — High-assurance compiler spine

Select small, high-leverage edges for stronger verification.

Preferred candidates:

1. semantic core -> canonical HIR;
2. HIR -> canonical SSA;
3. certificate/translation checker;
4. one backend lowering subset.

The objective is to shrink the trusted core progressively, not freeze all compiler development inside one theorem prover.

### C12 — Bootstrap and self-hosting

Only after the compiler and language profile stabilize enough to justify it.

Acceptance must preserve:

- seed compiler identity;
- bootstrap binary identity;
- source/binary correspondence evidence;
- independent/diverse rebuild evidence where attempted;
- explicit trust roots.

Self-hosting never means "trust problem solved."

## Compiler pass policy

A pass is not just a Rust function.

For semantically relevant passes, the architecture should converge toward metadata equivalent to:

```text
id
version
input_schema
output_schema
claimed_relation
observation_model
required_invariants
required_target_facts
consumed_assumptions
generated_obligations
validator_profile
fallback_policy
```

A particular invocation produces a `TransformationEdge` connecting exact input/output identities.

## Backend policy

Do not let the first backend leak into language semantics.

The backend interface must force target-dependent assumptions into explicit data:

- data layout;
- ABI/calling convention;
- object format;
- integer/float lowering;
- atomics/memory order;
- traps/unwind;
- target features;
- linker requirements;
- backend optimization flags;
- promises consumed from SSA;
- assumptions introduced by backend lowering.

## Cross-platform study invariants

Before target resolution, these should normally remain host independent:

- language profile identity;
- canonical semantic identity;
- semantic graph identity;
- HIR identity;
- SSA identity;
- compiler-pass definitions and relation claims.

After target resolution, differences may be expected in:

- target lowering plan;
- layout realizations;
- ABI;
- native instruction selection;
- object format;
- linker artifacts;
- scheduling/runtime realization;
- target-specific quantitative results.

The compiler evidence graph should make the transition visible.

## First study kernel

Use a small but nontrivial kernel rather than `hello world`:

- bounded loop;
- checked and wrapping arithmetic;
- one branch;
- one memory/state region;
- one effect/capability edge;
- one explicit failure/runtime-check path.

Freeze a deterministic edge corpus.

Run it through:

```text
semantic body
-> HIR
-> SSA
-> portable backend
-> all Fabric hosts
-> native Linux x86-64
-> native Windows x86-64
-> native Linux AArch64
```

Then deliberately inject:

- one invalid SSA rewrite;
- one target-feature assumption that is valid on only one node;
- one stale backend-validation artifact.

The compiler study is successful when each is either accepted for an explicit reason, rejected, or preserved as unresolved. Silent assumptions are failures.

## Near-term repository tasks

The first four repository-side tasks below are implemented for the bounded subset. Native Fabric execution and backend work remain ordered follow-ons:

1. **Implemented:** add compiler contract structs and adversarial identity tests to `mncs-model`.
2. **Implemented:** add the `mncs-compiler` orchestration crate.
3. **Implemented:** add CLI `compile` artifact-bundle output.
4. **Implemented:** exercise Linux/Windows hosts and retain the AArch64 shape gate.
5. **Implemented:** add Fabric-facing node/study request/result schemas and a frozen fixture.
6. **Pending external execution:** run and compare the first Linux/Windows/Raspberry Pi semantic/HIR/SSA identity study. GitHub-hosted Windows is CI evidence; a native Raspberry Pi run must come from Fabric.
7. **Pending:** add a replaceable portable backend adapter for a deliberately bounded subset.
8. **Pending:** introduce the first native backend experiment only after the portable/backend-validation seam is exercised.

This ordering lets the project learn from cross-platform differences before native backend complexity can hide whether a mismatch belongs to semantics, compiler orchestration, target lowering, toolchain, or runtime execution.
