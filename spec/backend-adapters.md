# Backend Adapter Boundary

This specification defines the minimum architectural requirements for executable backend integrations in MNCS. It is derived from RFC 0041 and is intentionally narrower than any individual backend implementation.

## Core rule

A backend is a realization mechanism, not a definition of MNCS semantics.

Language-owned legality and meaning must be established before backend lowering. A backend may reject or defer unsupported realization requests, but it may not reinterpret the language to fit its own capabilities.

## Required separation

The compiler pipeline must preserve the following boundary:

```text
language-owned:
  semantic artifact
  HIR
  SSA
  Selected SSA
  TargetLoweringPlan

backend-owned realization:
  capability evaluation
  lowering implementation
  artifact encoding
  optional backend execution integration

language-owned evidence boundary:
  BackendArtifact identity
  BackendEvidence
  transformation relation/evidence
  CompilationEvidenceBundle
```

## Backend adapter requirements

Every backend integration MUST expose or provide equivalent behavior for:

- stable backend identity;
- backend version identity;
- declared capability envelope;
- target compatibility evaluation;
- backend configuration;
- explicit required target facts;
- lowering from selected SSA under a target-lowering plan;
- structured unavailable-capability diagnostics;
- explicit assumptions introduced by the backend;
- backend artifact identity and provenance;
- obligations and evidence dependencies produced or consumed by lowering.
- identity-bound decisions for every emitted or withheld backend promise that applies to the
  selected operation.

A backend MAY additionally expose:

- embedded execution;
- external-runtime execution;
- object generation;
- linker integration;
- executable packaging;
- remote execution through Fabric;
- backend-specific optimization controls;
- backend-specific measurement hooks.

These optional capabilities must not be assumed by generic compiler orchestration.

## Support evaluation

Backend compatibility evaluation MUST distinguish:

- supported with sufficient facts;
- unsupported by the backend capability envelope;
- unresolved because required target facts/evidence are missing.

Missing evidence MUST NOT be converted into an assumed fact solely because a backend recognizes a familiar target name.

## Compiler-driver requirements

`mncs-compiler` MUST dispatch backend work through a backend-neutral interface.

It MUST NOT require a concrete backend in order to validate language semantics or emit target-independent compiler artifacts.

It MUST NOT branch directly on backend names or backend-specific target-recognition helpers as the normal extension mechanism.

Adding a new backend SHOULD require implementing and registering an adapter rather than modifying compiler sequencing.

The generic compiler transformation edge from selected SSA to a backend artifact SHOULD use backend-neutral stage meaning. The concrete backend identity MUST remain available in derivation/evidence records.

## Target-lowering plan requirements

`TargetLoweringPlan` remains a language-owned artifact even when backend declarations contribute to its construction.

A plan MUST retain enough information to explain why lowering was legal, unresolved, or unavailable.

Relevant facts include, where applicable:

- data layout;
- ABI and calling convention;
- integer and numeric mapping;
- trap/failure behavior;
- memory/provenance mapping;
- runtime requirements;
- linker/toolchain requirements;
- target features;
- consumed promises;
- introduced assumptions;
- generated obligations;
- unresolved facts.

## Artifact requirements

A `BackendArtifact` MUST identify the backend realization that produced it.

Backend artifacts from two different backends MUST remain distinguishable even when they originate from the same selected SSA and agree over all currently tested observations.

Generic backend artifact handling MUST NOT assume one artifact encoding such as WASM.

The executable contract is `BackendAdapter`. Each registered adapter declares an identity-bound
`BackendCapabilityManifest`, owns its target contract and configuration, produces a
`TargetLoweringPlan`, lowers one exact selected-SSA identity, and optionally executes its own typed
artifact. A `RealizationRequest` binds acceptable backend identities, required intent support,
artifact kinds, validators, and fallback policy before realization is selected.

## Execution requirements

Backend lowering and backend execution are distinct capabilities.

A backend can validly support lowering without an in-process execution engine. Generic execution orchestration MUST therefore be able to report execution as unavailable or route execution through an external runtime/Fabric instead of assuming an embedded interpreter.

WASM magic/version checks, WASM decoding, and the embedded WASM interpreter belong to the WASM execution implementation, not generic backend execution.

## Evidence requirements

Backend evidence MUST preserve, as applicable:

- selected-SSA identity;
- backend identity/version;
- backend configuration;
- target identity;
- artifact identity/content hash;
- assumptions;
- obligations;
- evidence dependencies;
- linker/toolchain identity;
- run environment identity;
- validation status.

Cross-backend execution agreement MUST be described as bounded agreement or another explicitly scoped relation claim unless stronger evidence exists.

For Profile 0.4, both current capability manifests declare `semantic_bounded_iteration`. The
selected SSA retains each iteration identity, bound, state type, region blocks, callees,
capabilities, completion modes, and obligations. Portable WASM realizes the cyclic control flow
with its private dispatcher loop; research bytecode retains the block-parameter SSA. An adapter may
choose another private shape, but it MUST NOT erase or invent the selected-SSA bound. A cyclic SSA
graph without exact bounded-iteration metadata is invalid before adapter dispatch.

## Forge requirements

Forge MAY search:

- backend choice;
- backend configuration;
- pass/backend combinations;
- representation/backend combinations;
- target specialization/backend combinations.

Forge MUST NOT gain semantic authority through this interface.

A candidate that performs better but fails required obligations MUST NOT be promoted solely on performance.

## Fabric requirements

Fabric MAY execute and measure backend artifacts across heterogeneous hosts and targets.

Fabric observations MUST retain backend, build-host, target, run-environment, and toolchain identity where relevant.

Cross-machine agreement is evidence, not semantic definition.

## Initial conformance target

The backend adapter boundary is considered minimally exercised when:

1. portable WASM is routed through the generic adapter boundary;
2. `mncs-compiler` no longer contains WASM-specific dispatch logic;
3. generic backend execution no longer assumes WASM bytes;
4. a second backend compiles the same frozen corpus without changing compiler sequencing;
5. backend identity and assumptions remain present in evidence;
6. missing target facts continue to produce unresolved lowering rather than guessed defaults.

This target is now exercised by five registered adapters. Adding another backend still requires an
adapter, capability manifest, target facts, lowering, artifact representation, optional execution,
and tests — not generic compiler sequencing edits.

| Adapter | Artifact kind | Execution path | Status |
| --- | --- | --- | --- |
| `mncs-portable-wasm-mvp` | `wasm_module` | embedded decoder/interpreter for the declared MVP subset | implemented / exercised |
| `mncs-research-bytecode` | `research_bytecode` | bounded SSA interpreter over an identity-bound payload | implemented / exercised |
| `mncs-llvm-ir` | `llvm_ir` | external clang; optional llc object pathway | implemented / exercised on CRE-1/2/3 |
| `mncs-c11` | `c11_translation_unit` | external clang/gcc `-std=c11` | implemented / exercised on CRE-1/2/3 |
| `mncs-cranelift` | `cranelift_clif` | Cranelift IR/JIT on the host ISA | CLIF implemented; execution reports host-scoped `UNSUPPORTED` when executable-memory policy denies JIT mapping |

LLVM IR generation does not establish correctness. Clang/llc/gcc are outside the MNCS semantic
trust boundary. Native object/executable realization uses those tools as optional external steps.
x86-64, AArch64, and Raspberry Pi-class ARM are target realizations, not language variants.

Planned and **not** implemented: SPIR-V/GPU, PTX, eBPF, additional VMs, bare-metal embedded
toolchains. Query `mncs experiment matrix` for the generated capability envelope.

Agreement across adapters over a corpus is retained as bounded observation only.
