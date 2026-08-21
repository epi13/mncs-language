# RFC 0041 — Machine-Native Backend Plurality and Realization Contracts

Status: Proposed

## Summary

MNCS must not allow any one code generator, object format, virtual machine, native toolchain, or execution engine to become the implicit definition of the language.

This RFC makes backend plurality an architectural invariant of the compiler. WebAssembly remains the first executable backend experiment, but becomes one implementation of a language-owned backend adapter contract rather than a privileged compiler route.

The language owns semantics, legality, obligations, intermediate representations, target contracts, lowering requirements, relation claims, and evidence requirements. Backends own bounded realizations of already-selected compiler artifacts under explicit target facts. Forge may search among backend realizations. Fabric may execute and measure them. Neither search nor execution grants semantic authority.

The central rule is:

> No backend is the semantic reference implementation of MNCS.

## Motivation

The current compiler architecture already separates semantic input, HIR, SSA, selected SSA, target-lowering plans, backend artifacts, and evidence. The current implementation, however, still contains a direct portable-WASM route in `mncs-compiler`: target recognition, backend configuration, target-plan construction, lowering dispatch, and transformation naming are aware of WASM specifically.

That is acceptable for a first vertical slice, but dangerous as a long-term shape. If more language features are implemented while only one backend path exists, constraints of that backend can accidentally become language assumptions. Examples include stack-machine preferences, integer representation choices, memory model expectations, ABI shape, exception/trap behavior, object/linking conventions, and runtime availability.

MNCS should instead use heterogeneous backends as a source of architectural pressure. A language concept that is only convenient because WASM, LLVM, x86-64, or one host toolchain happens to support it should be forced to declare that dependency explicitly.

## Architectural invariant

The compiler pipeline is conceptually:

```text
source + LanguageProfile
        |
        v
parse / bind / elaborate
        |
        v
semantic artifact
        |
        v
HIR
        |
        v
SSA
        |
        v
Selected SSA
        |
        v
TargetLoweringPlan
        |
        v
backend adapter
   |      |      |
   v      v      v
 WASM  bytecode native ...
        |
        v
BackendArtifact + BackendEvidence
```

Everything through `TargetLoweringPlan` is language-owned compiler meaning. A backend adapter consumes that meaning and attempts a realization. It may refuse unsupported input. It may introduce assumptions only when those assumptions are explicit in the returned evidence. It may never invent missing target facts or silently weaken obligations.

## Ownership boundary

### `mncs-language`

Owns:

- semantic legality;
- source/profile meaning;
- HIR and SSA contracts;
- selected-SSA identity;
- target contracts and target-fact requirements;
- `TargetLoweringPlan`;
- backend adapter protocol;
- backend identity and capability vocabulary;
- lowering obligations;
- transformation relation claims;
- backend evidence schema;
- translation-validation requirements;
- conservative fallback behavior.

### Backend adapters

Own:

- capability declaration;
- target compatibility declaration;
- lowering implementation;
- backend-specific artifact encoding;
- backend-specific execution integration when applicable;
- explicit backend assumptions;
- backend-specific diagnostics and unavailable-capability explanations.

Backends do not own language semantics, source legality, or promotion authority.

### Forge

May own:

- backend candidate search;
- backend configuration search;
- pass/backend combinations;
- representation/backend combinations;
- multi-objective realization tournaments;
- generated backend experiments.

Forge proposes. It does not make a candidate correct.

### Fabric

May own:

- distributed execution;
- host/target environment observations;
- performance/resource measurement;
- cross-machine experiment transport;
- repeated backend corpus runs.

Fabric observes. It does not define semantics.

## Backend adapter contract

The concrete Rust API may evolve, but the logical contract must support the following operations.

```rust
trait BackendAdapter {
    fn identity(&self) -> BackendIdentity;
    fn capabilities(&self) -> BackendCapabilities;

    fn support(
        &self,
        target: &TargetContractRef,
        selected_ssa: &CompilerArtifactRef,
    ) -> BackendSupport;

    fn configuration(
        &self,
        target: &TargetContractRef,
    ) -> BackendConfiguration;

    fn plan(
        &self,
        selected_ssa: CompilerArtifactRef,
        target: TargetContractRef,
        requested: Option<BackendConfiguration>,
    ) -> TargetLoweringPlan;

    fn lower(
        &self,
        program: &Program,
        ssa: &SsaModule,
        selected_ssa: CompilerArtifactRef,
        plan: &TargetLoweringPlan,
    ) -> BackendResult;
}
```

This is illustrative, not a commitment to an exact trait signature. The important boundary is that compiler orchestration dispatches through backend-neutral operations and does not branch on WASM, LLVM, Cranelift, C, or other backend names.

## Backend discovery and selection

The reference compiler must be able to distinguish:

1. no backend requested;
2. a specific backend requested;
3. one or more compatible backends available;
4. no compatible backend available;
5. target facts insufficient to determine compatibility;
6. multiple compatible backends intentionally retained as experiment candidates.

Selection must not be encoded as target-name string matching in the compiler driver.

A minimal registry can initially be static and in-process. Dynamic plugin loading is not required by this RFC. What matters is that adding backend #2 does not require rewriting compiler sequencing or embedding another target-specific branch in `mncs-compiler`.

## Backend support states

Backend support should distinguish at least:

- `SUPPORTED` — required facts are present and the backend can attempt lowering;
- `UNSUPPORTED` — the target or selected SSA is outside the backend's declared capability envelope;
- `UNKNOWN` — required target facts or evidence are absent or unresolved.

`UNKNOWN` must remain different from `UNSUPPORTED`.

A backend must never convert missing target evidence into an assumed fact merely because a target name is familiar.

## Target-lowering plans

`TargetLoweringPlan` remains language-owned. Backend-specific information can contribute to a plan, but the plan is not private backend state.

Each plan must preserve, as applicable:

- selected-SSA identity;
- target identity;
- backend identity/configuration;
- layout facts and assumptions;
- ABI/calling-convention facts and assumptions;
- integer/numeric mappings;
- trap/failure mappings;
- memory/provenance mappings;
- runtime requirements;
- linker/toolchain requirements;
- promises consumed;
- obligations generated;
- unresolved facts;
- transformation status.

## Evidence and identity

Every emitted backend artifact must retain enough identity to distinguish at least:

```text
selected SSA
backend implementation + version
backend configuration
build host where relevant
target contract
linker/toolchain where relevant
runtime/execution environment where relevant
artifact bytes/content identity
assumptions
obligations
evidence dependencies
```

Two artifacts produced from identical SSA by different backends are different realizations, even when their observed behavior agrees over a corpus.

Agreement among backends is evidence. It is not semantic definition and it is not automatically universal equivalence.

## Reference execution remains independent

MNCS semantic/body execution and SSA reference execution remain above backend execution in the trust structure.

A backend execution engine may be used for experiments, but it must not become the oracle that defines the meaning of the source language. The current embedded WASM interpreter is therefore a research executor for the WASM subset, not the reference semantics of MNCS.

## Initial backend family

Backend diversity should be chosen to stress different assumptions, not merely to maximize the number of code generators.

### 1. Portable WASM MVP

Keep the existing bounded WASM backend as backend #1.

Purpose:

- portable executable baseline;
- tightly constrained lowering target;
- same-artifact experiments across current Fabric hosts;
- existing bounded translation-validation work.

### 2. Minimal MNCS bytecode/reference backend

Add a deliberately small backend controlled entirely by the project.

Purpose:

- provide a second lowering route with very little external compiler behavior;
- expose accidental WASM assumptions;
- support deterministic compiler tests;
- create a conservative fallback realization for bounded language subsets.

This bytecode backend must not replace semantic or SSA reference execution. It is still a backend realization.

### 3. Cranelift native backend

Preferred first serious native experiment.

Initial target families:

- Linux x86-64;
- Windows x86-64;
- Linux AArch64 where the Raspberry Pi node profile supports it.

### 4. Conservative C11 backend

Emit deliberately conservative C and compile it with more than one downstream compiler where practical, initially GCC and Clang.

Purpose:

- add a structurally different lowering route;
- obtain independent mature downstream compiler families;
- provide useful differential experiments;
- expose ABI, undefined-behavior, integer, layout, and toolchain assumptions explicitly.

C emission must not rely on C undefined behavior to represent MNCS semantics.

### 5. LLVM backend

Add after the adapter boundary and at least one simpler native route are exercised.

Purpose:

- mature optimization infrastructure;
- broad target support;
- target-specific realization studies;
- comparison against Cranelift and other routes.

LLVM is a candidate backend, not the compiler architecture.

## Later stress backends

Some backends may primarily exist to test assumptions rather than serve as general deployment targets.

Candidate families include:

- eBPF — bounded/verifier-constrained execution;
- SPIR-V or another GPU route — SIMT and heterogeneous execution pressure;
- RISC-V — clean native ISA studies;
- constrained embedded targets — runtime, allocation, resource, and ABI pressure.

Support for these targets should be evidence-driven and bounded by explicit language profiles.

## Backend tournaments

Once multiple adapters exist, backend choice becomes a legal Forge realization-search dimension.

A tournament may compare candidates over declared objectives such as:

- functional observations;
- artifact size;
- compile latency;
- execution latency;
- memory use;
- energy/resource cost;
- portability;
- proof/validation cost;
- available evidence strength;
- target-specific constraints.

A faster candidate with failed or unresolved required obligations cannot be promoted solely because it performs better.

The result may legitimately differ by program and target. MNCS does not require one globally preferred backend.

## Compiler-driver rule

`mncs-compiler` must become backend-neutral.

After migration, the compiler driver must not:

- import WASM-specific target recognition helpers;
- import WASM-specific plan constructors;
- import WASM-specific lowering functions;
- branch on WASM target strings;
- use transformation identities whose generic compiler stage is named for WASM;
- assume that every `BackendArtifact` contains WASM bytes;
- treat backend execution as synonymous with WASM execution.

Backend-specific code belongs behind the adapter boundary.

## Execution adapter rule

Lowering and execution are related but distinct capabilities.

A backend may support:

- lowering only;
- lowering plus embedded execution;
- lowering plus external runtime execution;
- object generation requiring a linker;
- linked executable generation;
- remote execution through Fabric.

The generic backend API must therefore not assume a WASM-style in-process interpreter.

## Migration plan

### B0 — establish the contract

This RFC and a current-state coupling assessment are merged before additional backend-sensitive language features are added.

### B1 — extract backend-neutral dispatch

Refactor `mncs-codegen` to expose a backend adapter/registry boundary.

Keep compatibility wrappers for the current WASM public helpers while migrating internal orchestration.

### B2 — place WASM behind the adapter

The current WASM code becomes `PortableWasmBackend` or equivalent.

`mncs-compiler` discovers/selects it through the generic boundary.

The compiler's transformation edge becomes backend-neutral while retaining the concrete backend identity in evidence.

### B3 — separate generic backend execution from WASM execution

Move WASM decoding/interpreter assumptions behind the WASM execution capability.

Generic backend execution dispatches by adapter/capability rather than calling `artifact.wasm_bytes()` unconditionally.

### B4 — implement the minimal MNCS bytecode backend

Compile at least one existing scalar/control-flow corpus through both WASM and bytecode routes.

Require structured mismatch reporting and preserve bounded-agreement semantics.

### B5 — native backend experiment

Add Cranelift or another bounded native adapter and run identical kernels through Fabric on Linux x86-64, Windows x86-64, and Linux ARM where available.

### B6 — conservative C backend

Emit C for the same bounded corpus and compare GCC/Clang downstream results where available.

### B7 — backend tournament protocol

Allow Forge to propose backend/configuration candidates without granting Forge semantic authority.

## Acceptance criteria for backend plurality

The backend-plurality migration is considered established when:

1. `mncs-compiler` contains no WASM-specific dispatch logic;
2. adding a second backend requires implementing/registering an adapter, not editing compiler sequencing;
3. target facts remain explicit and `UNKNOWN` is preserved when required evidence is missing;
4. backend identity/version/configuration are present in emitted evidence;
5. transformation edges remain language-owned and backend-neutral in stage meaning;
6. generic backend execution does not assume WASM artifact encoding;
7. at least two independent backend realizations compile the same frozen corpus;
8. cross-backend comparison is reported as bounded agreement or a scoped relation claim, never as automatic equivalence;
9. unsupported backend features produce structured capability diagnostics rather than semantic reinterpretation;
10. Forge can eventually treat backend selection as a bounded realization dimension without changing language legality.

## Architectural regression guard

Future compiler changes should be reviewed against the following question:

> Would implementing the same feature in a second structurally different backend require changing the meaning of the language-owned compiler stages?

If yes, the feature is probably crossing the backend boundary incorrectly or depends on target facts that need to become explicit.

The strongest practical regression signal is any new backend-specific branch added directly to `mncs-compiler`. Such a change requires an explicit architectural justification and should normally be rejected in favor of extending the adapter contract.

## Non-goals

This RFC does not:

- require dynamic shared-library backend plugins;
- choose one permanent native backend;
- require every backend to implement every language feature;
- require all backends to produce equivalent machine layouts;
- treat cross-backend testing as proof;
- require immediate implementation of LLVM, C, GPU, eBPF, or embedded targets;
- permit backends to weaken semantic contracts for convenience.

## Consequences

Backend plurality costs some interface design and evidence bookkeeping early. That cost is intentional. It prevents a much larger future cost: discovering that the language, IR, runtime model, or optimization semantics silently inherited assumptions from whichever code generator happened to be implemented first.

WASM remains valuable precisely because it becomes the first member of a backend family rather than the foundation beneath the family.
