# Backend Plurality Assessment — 2026-08

## Outcome

The compiler architecture is conceptually backend-replaceable, but the implementation currently has one privileged executable route: portable WASM MVP.

This is not yet a semantic failure. The language still owns legality, selected SSA, target contracts, target-lowering plans, obligations, and evidence. However, `mncs-compiler` directly imports and invokes WASM-specific helpers, and `mncs-codegen` exposes generic-sounding execution APIs whose implementation assumes WASM artifact encoding.

That coupling should be removed before backend-sensitive language work expands.

RFC 0041 establishes backend plurality as an architectural invariant and defines the migration target.

## Current strengths

The current compiler already has the right major semantic boundaries:

```text
semantic
  -> HIR
  -> SSA
  -> Selected SSA
  -> TargetLoweringPlan
  -> BackendArtifact
  -> CompilationEvidenceBundle
```

Useful existing properties include:

- selected SSA has a distinct identity;
- target facts are explicit rather than inferred solely from host names;
- unevidenced target lowering can remain `UNKNOWN`;
- backend identity/configuration already exist in compiler artifacts;
- backend artifacts retain selected-SSA and target bindings;
- backend assumptions are represented explicitly;
- bounded body/SSA/backend comparison is already described as empirical agreement rather than proof;
- the roadmap already treats WebAssembly/WASI as the first experiment rather than a permanent requirement;
- the roadmap already anticipates Cranelift and LLVM as native backend candidates;
- Forge already has a conceptual role in backend selection/search.

These foundations mean the backend-plurality migration can be evolutionary rather than a compiler redesign.

## Current coupling inventory

### 1. Compiler imports WASM-specific codegen helpers

`crates/mncs-compiler/src/lib.rs` imports:

```text
lower_selected_ssa
portable_wasm_backend_configuration
portable_wasm_plan
target_is_portable_wasm
```

The first helper has a generic name, but the current implementation lowers only the portable WASM subset.

Desired state:

```text
mncs-compiler
  -> backend-neutral adapter/registry API
  -> selected adapter
  -> backend result
```

The compiler driver should not need to know whether the selected adapter is WASM, bytecode, Cranelift, C, LLVM, or another route.

### 2. Compiler request construction recognizes WASM directly

`ReferenceCompiler::request_for_program` currently inspects a target through `target_is_portable_wasm` and installs the portable-WASM backend configuration when it matches.

Risk:

- every new backend encourages another target-specific compiler branch;
- backend selection becomes conflated with target-name recognition;
- backend configuration policy becomes embedded in compiler sequencing.

Desired state:

- backend discovery/selection resolves compatible adapters;
- explicit requested backend remains possible;
- missing target facts produce `UNKNOWN` rather than guessed support;
- multiple compatible backends can later remain experiment candidates.

### 3. Target-lowering plan construction has a WASM special case

The compiler currently creates a portable-WASM template when the target is recognized, then copies its target-layout, ABI, integer, trap, linker, and promise fields into the language-owned `TargetLoweringPlan`.

The data model is useful. The special-case construction is the problem.

Desired state:

- the selected backend contributes its declared lowering requirements/realization mapping through the adapter contract;
- the resulting plan remains a language-owned artifact;
- the compiler orchestration remains generic.

### 4. Lowering dispatch is hard-wired to one backend

After a `PASS` target plan, `mncs-compiler` calls `lower_selected_ssa`, which currently invokes the portable-WASM lowering implementation.

Desired state:

```text
adapter.lower(program, ssa, selected_ssa, plan)
```

or an equivalent backend-neutral operation.

### 5. Compiler transformation naming mentions WASM

The backend transformation edge is currently named:

```text
lower-selected-ssa-to-portable-wasm
```

That is accurate for the implementation but encodes the concrete backend into the generic compiler stage.

Desired state:

- compiler stage identity describes the generic transformation boundary;
- concrete backend identity/version remain attached as evidence/derivation context;
- backend-specific subpass identities may exist inside backend evidence where useful.

A direction such as `lower-selected-ssa-to-backend-artifact` is preferable for the language-owned edge.

### 6. Generic backend execution assumes WASM bytes

`mncs-codegen::execute_backend` currently:

- calls `artifact.wasm_bytes()`;
- checks WASM magic/version;
- decodes the module with the embedded WASM decoder;
- invokes the embedded WASM interpreter.

This function is therefore a WASM executor despite its generic name.

Desired state:

- WASM execution is a capability of the WASM adapter/executor;
- generic backend execution dispatches to a backend-specific execution capability or reports that execution is unavailable;
- object-only/native backends can require linking or remote Fabric execution without pretending to be in-process interpreters.

### 7. `mncs-codegen` crate documentation describes one portable backend

The current crate-level documentation says it produces an identity-bound WASM MVP artifact.

Desired state:

- `mncs-codegen` becomes the backend adapter boundary;
- portable WASM becomes one backend module/implementation inside that boundary;
- crate-level documentation describes multiple replaceable realization routes.

## Immediate migration sequence

### Step 1 — define the adapter surface

Add a backend-neutral protocol in `mncs-codegen` or a narrowly justified adjacent module.

Minimum concepts:

```text
BackendAdapter
BackendCapabilities
BackendSupport
backend identity
configuration
target support evaluation
plan contribution
lowering
optional execution capability
```

Do not over-design dynamic loading yet. A static registry is sufficient.

### Step 2 — wrap existing WASM implementation

Create `PortableWasmBackend` or equivalent and route the existing functions through it.

Compatibility wrappers may temporarily retain:

```text
portable_wasm_backend_configuration()
portable_wasm_plan(...)
lower_selected_ssa(...)
```

External callers need not break during the first extraction.

### Step 3 — migrate compiler orchestration

Replace direct WASM imports in `mncs-compiler` with generic backend discovery/dispatch.

The compiler should be able to create a valid request and target plan even when no executable backend is available.

### Step 4 — separate execution dispatch

Rename or wrap the current WASM executor so generic backend code no longer assumes artifact format.

### Step 5 — add backend #2 early

Implement the minimal MNCS bytecode/reference backend before broadening backend-sensitive semantics.

The purpose is not performance. The purpose is to force the adapter boundary to prove that it is real.

## Recommended first backend matrix

| Backend | Primary role | External toolchain | Architectural pressure |
| --- | --- | --- | --- |
| Portable WASM MVP | portable baseline | none for embedded research execution | stack/VM constraints, portable traps |
| MNCS bytecode | project-controlled second route | none | exposes accidental WASM assumptions |
| Cranelift | first native route | Cranelift | native ABI/object/codegen pressure |
| C11 + GCC/Clang | structurally different route | C compilers | UB, ABI, layout, downstream compiler diversity |
| LLVM | mature optimizing route | LLVM | optimization and broad-target pressure |

Later stress routes can include eBPF, GPU/SPIR-V, RISC-V, and constrained embedded targets.

## Regression indicators

The following changes should be treated as warnings during review:

- a new `target_is_<backend>` branch added to `mncs-compiler`;
- a new backend-specific plan constructor imported directly by the compiler driver;
- a compiler stage renamed for one concrete backend when the semantic stage is generic;
- generic `BackendArtifact` logic that assumes one byte encoding;
- semantic validation that depends on a backend being present;
- source or SSA semantics changed merely because one backend cannot express them conveniently;
- target facts inferred from a backend name without evidence;
- Forge performance results used as correctness authority;
- cross-backend test agreement described as universal equivalence.

## Implementation acceptance checks

The first code migration should add tests that establish at least:

1. the compiler driver has no WASM-specific lowering branch;
2. the WASM backend still compiles the existing checked-add corpus;
3. a mystery/unevidenced target remains `UNKNOWN`;
4. registering a dummy or minimal second backend does not require changes to compiler sequencing;
5. backend identity is preserved in evidence and artifact derivation;
6. generic execution can report an unavailable execution capability without interpreting arbitrary bytes as WASM;
7. the current body/SSA/WASM bounded comparison remains intact after extraction.

## Practical conclusion

The project is at the right moment to make this change. The existing WASM implementation is large enough to prove the executable path, but backend-sensitive language semantics have not yet spread so far that the route is difficult to isolate.

The goal is not to abandon WASM. The goal is to demote it from "the executable path" to "backend #1" before architectural inertia turns that temporary fact into the language design.
