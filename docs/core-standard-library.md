# Core Standard Library Direction

Status: **research-active / partially realized** (2026-08-27).

The current tranche is Profile 0.10 explicit polymorphism over the bounded
sequence/view substrate. Overlapping core exports can still be consumed
through aliases while the compiler retains declaring-module identity and
emits inspectable binding evidence. Generic library primitives are explicit,
fully specialized before lowering, and remain subject to the same backend
capability and unresolved-obligation rules as non-generic code.

MNCS should develop a standard library, but it should not begin as a monolithic host-runtime API patterned after an existing language. The library must preserve the same separation already enforced by the language architecture: logical semantics are authoritative, capabilities are explicit, physical representation is a realization choice, and target-specific implementations do not redefine meaning.

The immediate goal is to establish a small Core Standard Library track that can grow alongside the language and pressure-test missing language/compiler features through real use by MNCS programs, RAVEL/MNEL reconstruction experiments, Forge, and later self-hosting work.

## Architectural principle

The library should be layered around semantic authority rather than convenience alone:

```text
                    MNCS programs
                         │
                ┌────────┴────────┐
                │                 │
            mncs.core         mncs.std
                │                 │
                └────────┬────────┘
                         │
             capability-scoped libraries
          io / fs / net / time / random / etc.
                         │
                 semantic operations
                         │
          reference + candidate realizations
                         │
       WASM / LLVM / C / Cranelift / GPU / ...
```

The names above describe the intended conceptual split. They do **not** freeze final package/import syntax before the module/package system has earned that decision experimentally.

A library operation should be treated like any other MNCS semantic operation:

```text
logical meaning
+ required capabilities/effects
+ representation requirements
+ permitted realization envelope
+ evidence / obligations
+ portability and fallback policy
```

Optimized implementations may vary by backend, target, toolchain, host, or Forge-selected realization while the language-level contract remains stable.

## Layer 1 — `mncs.core`

`mncs.core` should be the smallest portable deterministic foundation that ordinary MNCS code can rely on without ambient host authority.

Candidate responsibilities include:

- fundamental logical value operations;
- canonical integer/numeric helpers built on explicit MNCS arithmetic semantics;
- comparison and ordering building blocks where their semantics are defined;
- product/sum/record/variant helpers as those value forms mature;
- eventual `Option`/`Result`-like types once the language has an accepted sum/variant mechanism;
- bounded iteration and basic sequence abstractions where they can be expressed without hidden allocation;
- canonical identity/hash/fingerprint helpers where appropriate and semantically specified;
- small conversion/validation operations with explicit failure behavior;
- primitives required by the rest of the portable library.

The first reusable allocation primitive is now `mncs.core.partition.v1`. It
implements a bounded four-lane weighted split with nonnegative normalized
weights, deterministic remainder assignment, explicit invalid-weight-sum
reporting, and an overflow-safe quotient/remainder path. Layout consumers use
this module as their arithmetic authority and add only consumer-specific cap
projection.

`mncs.core` should avoid:

- filesystem, network, clock, process, randomness, environment, or credential authority;
- implicit allocation before storage/allocation semantics exist;
- target-defined integer behavior;
- hidden global state;
- implementation-defined layout exposed as logical meaning;
- operations whose contract exists only because one backend happens to provide an intrinsic.

Where an operation is sufficiently fundamental, its semantics may effectively become part of the stable language ecosystem even if its source implementation lives in a library.

## Layer 2 — `mncs.std`

`mncs.std` should contain richer portable facilities written primarily in MNCS itself once the language supports them.

Candidate domains include:

- bytes and byte sequences;
- text/string abstractions;
- arrays, slices, sequences, and collection interfaces;
- canonical algorithms;
- numeric utilities;
- encoding/decoding and canonical serialization;
- iterators/streams or an MNCS-native alternative;
- maps/sets and other collections after memory/storage semantics mature;
- formatting and parsing;
- portable data transformation utilities;
- higher-level error/result composition;
- common proof/evidence-aware adapters where they belong at library rather than language level.

`mncs.std.ansi.v1` is the first terminal-protocol adapter in this layer. It
accepts bounded byte sequences and emits generic semantic events (including
`Unknown` for malformed or unsupported input). It deliberately has no TUI
dependency and does not perform host I/O; applications map the generic events
at their own boundary.

`mncs.std.text_view.v1` and `mncs.std.json_cursor.v1` now pressure-test the
text/serialization boundary. Text is a borrowed byte span with explicit
encoding and UTF-8 validity flags; the cursor exposes bounded JSON structure,
string/key events, absolute spans, and a twelve-level container stack. It does
not allocate a DOM or decide application schema meaning. Unknown keys outside
the fixed 32-byte matcher window remain structurally valid, while malformed
UTF-8 lead/continuation sequences are rejected. Schema consumers such as the
Atlas model own their field classification and render intent.

Portable library semantics should remain independent from their physical representation. A string, sequence, map, record collection, or encoded value must not be forced into one physical layout merely because the first implementation used it.

## Layer 3 — capability-scoped libraries

Host interaction should live in explicit capability-bearing libraries rather than ambient `std` authority.

Likely domains include conceptually:

- `mncs.io`;
- `mncs.fs`;
- `mncs.net`;
- `mncs.time`;
- `mncs.random`;
- process/execution facilities;
- device and accelerator interfaces;
- environment/configuration access;
- Fabric/distributed-system operations where appropriate.

Importing or naming one of these APIs must **not** grant authority. Operations remain subject to the language's declared effect/capability closure and whatever authority object/evidence the eventual capability model requires.

For example, a filesystem read should conceptually require explicit read authority for a declared scope; a clock query should require the relevant time capability; randomness should distinguish deterministic seeded computation from host entropy.

The current deterministic random stream module (`mncs.core.random.v1`) exposes
domain-separated child roots for `split`. It guarantees reproducibility and
distinct roots under its defined wrapping arithmetic, but makes no statistical
independence claim; a consumer that needs non-overlapping finite streams must
bind its draw budget and partition policy in its own experiment identity.

This preserves the existing MNCS rule that external effects never arise implicitly from host availability.

## Realization and intrinsic boundary

The standard library should be one of the strongest consumers of MNCS's reference-semantics / multiple-realizations model.

A single logical operation may have:

- a portable MNCS reference implementation;
- a research-bytecode realization;
- a WASM realization;
- LLVM or Cranelift intrinsics/lowerings;
- scalar and SIMD forms;
- CPU-family-specific forms;
- accelerator/GPU realizations;
- specialized implementations selected by Forge;
- a conservative fallback when stronger requirements cannot be established.

Examples may eventually include:

- memory copy/fill/search;
- hashing;
- parsing/encoding kernels;
- vector operations;
- matrix operations;
- sorting/searching;
- checksums;
- compression primitives;
- numeric kernels.

The optimized realization is not the semantic definition. A candidate may be selected only when its required obligations are satisfied for the active target/host/toolchain envelope.

Forge may search the realization space, but it must not invent stronger semantics or promote a faster implementation that violates protected library contracts.

## Identity and evidence

Library artifacts should participate in MNCS identity/evidence machinery rather than behaving as opaque source bundles.

A library operation or package should eventually expose identities for relevant layers such as:

- logical API/contract identity;
- source/module/package identity;
- semantic/HIR/SSA identity where compiled;
- reference implementation identity;
- selected realization identity;
- target/backend/toolchain envelope;
- evidence and obligation references;
- compatibility/refinement relation claims;
- Family Record references for durable experiments and promotions.

Changing a performance preference should not silently change logical API identity. Changing semantic behavior must not masquerade as a realization-only update.

Profile 0.9 now supplies the first executable substrate for this boundary:
`SemanticNamespace`, `SemanticScope`, `SemanticBinding`, and
`SemanticReference` records are published in the source-study result. Their
identities are derived from module/function ownership and structural slots;
source offsets are retained only for navigation. The binding table is built
from the authoritative elaboration decisions and carried into canonical
semantic identity, HIR, SSA, and execution checks.

## Language features the library should pressure-test

The Core Standard Library should be developed early enough to expose missing language/compiler requirements rather than waiting for a theoretically complete language.

Important pressure points include:

- module/import/package boundaries;
- cross-module type/value identity;
- sum/variant and option/result forms;
- arrays, slices, bytes, and text;
- general memory/storage semantics;
- allocation and ownership/lifetime models or MNCS-native alternatives;
- polymorphism/generics/parametricity or a different machine-native abstraction mechanism;
- traits/interfaces/protocols or an MNCS-native alternative where reusable algorithms need abstraction;
- richer iteration and recursion boundaries;
- record values across function/module/backend ABI boundaries;
- error/failure propagation;
- serialization and canonical encodings;
- FFI/host capability boundaries;
- cross-backend conformance;
- package/library compatibility and version identity;
- language-service indexing/resolution for library symbols.

A missing feature discovered while implementing a core library primitive should be treated as language-design evidence, not worked around indefinitely inside the library.

## Bootstrap and self-hosting path

A healthy progression is:

```text
language primitives
      ↓
mncs.core
      ↓
portable mncs.std
      ↓
MNCS ecosystem tools using the library
      ↓
reconstruction of larger components in MNCS
      ↓
selected compiler / Forge / tooling components in MNCS
      ↓
limited, evidence-visible self-hosting experiments
```

The goal is not to rush self-hosting or hide bootstrap trust. The library should create a practical substrate that makes self-description and later self-hosting experiments meaningful.

RAVEL and MNEL reconstruction efforts are particularly useful consumers: duplicated helper logic encountered during those conversions can reveal which operations belong in `mncs.core`, `mncs.std`, or a capability-scoped package.

## Repository strategy

### Initial phase

Keep the first Core Standard Library work close to `mncs-language` while language/package contracts are changing rapidly.

Possible repository-local shapes include:

```text
library/
  core/
  std/
  capability/
```

or a later equivalent chosen by the package/module design.

Benefits of initially colocating the work:

- language and library fixtures can evolve atomically;
- missing compiler support is immediately visible;
- conformance can run against every backend envelope;
- source-profile experiments can directly pressure library syntax;
- package boundaries are not frozen before MNCS has evidence for them.

### Later phase

Once the module/package contract, version identity, compatibility semantics, and release tooling stabilize, evaluate moving the portable library to an independent `mncs-stdlib` repository.

A repository split should follow a stable semantic/package boundary; it should not be used to invent one prematurely.

## Integration with `mncs-language-service`

Now that bounded Profile 0.6 library resolution is real, `mncs-language-service` should index the canonical library through the same authoritative semantic model as workspace source.

LSP/MCP/native clients should be able to:

- resolve library definitions;
- inspect library contracts/effects/capabilities;
- distinguish portable reference operations from target realizations;
- query identity and compatibility information;
- identify obligations introduced by a library call;
- receive bounded context for library symbols without duplicating stdlib semantics in each client.

The language service remains an adapter/index over authoritative compiler semantics, not a separate library implementation.

## Roadmap

### Stage A — establish the Core contract

Status: **implemented / exercised incrementally**.

- define the conceptual `core` / portable `std` / capability-library split;
- identify the smallest operations expressible by the current source profile;
- add conformance fixtures for those operations;
- define how library operation identity and contracts are represented;
- explicitly list blockers discovered in module/type/backend support;
- avoid committing to final package syntax.

The Profile 0.10 sequence module is the first authoritative generic-library
consumer: fixed-window wrappers delegate to `N: Nat` implementations, while
view helpers preserve the runtime-empty case explicitly. The generic
implementation is source-owned; wrappers do not define a second algorithm.

### Stage B — first executable `mncs.core`

Status: **blocked only by operation-specific language support; incremental implementation encouraged**.

- implement a minimal deterministic core using current scalar/enumeration/record facilities;
- exercise it through research bytecode and every backend that declares the required capability envelope;
- fail closed on unsupported backend features;
- add bounded differential corpora;
- make RAVEL/MNEL conversions consume core operations where useful instead of cloning helpers.

### Stage C — data and reusable algorithm substrate

Status: **planned / language-feature dependent**.

- add sums/option/result once supported;
- add bytes/sequence foundations;
- add arrays/slices/text after memory/storage semantics are sufficiently defined;
- establish reusable algorithm abstraction mechanisms from evidence gathered during implementation;
- define canonical serialization facilities consistent with logical-vs-physical representation separation.

### Stage D — capability libraries

Status: **planned / capability and host-interface dependent**.

- define explicit effect/capability-bearing APIs;
- build deterministic/mock/reference interfaces where useful for tests;
- preserve host authority boundaries;
- integrate target/host support manifests;
- record effect/evidence information through compiler and Family Record paths.

### Stage E — realization search and optimization

Status: **planned / incremental**.

- bind portable library semantics to multiple realizations;
- allow Forge to compare semantically valid implementations;
- retain conservative fallbacks;
- make optimized realization promotion evidence-bearing and invalidatable;
- exercise SIMD/accelerator/target-specific implementations without creating language variants.

### Stage F — package stabilization and possible repository split

Status: **future**.

- stabilize package/module identity and compatibility rules;
- define release/version/signing/provenance policy as required;
- make language-service resolution robust across library versions;
- evaluate `mncs-stdlib` as a separate MNCS-family repository;
- begin stronger self-hosting experiments using the stabilized library substrate.

## Initial implementation candidates

The first implementation should remain deliberately small and should favor operations that expose architectural weaknesses without requiring a general allocator.

Candidate experiments:

1. record-based `Pair`/small product helpers and canonical projections;
2. checked/wrapping/saturating numeric helpers tied directly to existing arithmetic intent;
3. comparison/min/max/clamp-style operations where exact semantics are already available;
4. finite-option/result experiments once a suitable sum/variant form exists;
5. small fixed/bounded sequence operations before general dynamic collections;
6. canonical byte/serialization experiments when byte values and storage semantics permit them;
7. one operation with a portable reference plus two different backend realizations and an evidence-gated Forge comparison.

Do not pad the library with wrappers around host facilities simply to make it look large. Each operation should earn its place by providing reusable semantics, a meaningful abstraction boundary, or useful pressure on the language/compiler architecture.

## Success criteria

The Core Standard Library direction is successful when:

1. ordinary MNCS programs can share common operations without recreating them per project;
2. library semantics remain stable across different physical/backend realizations;
3. host authority is explicit and capability-scoped;
4. RAVEL, MNEL, and later MNCS-native applications reveal and then reuse common library abstractions;
5. backend limitations remain explicit `SUPPORTED` / `UNSUPPORTED` / `UNKNOWN` envelope facts rather than changing language behavior;
6. library calls retain identity, obligation, evidence, and realization traceability;
7. Forge can optimize library realizations without becoming a semantic authority;
8. the library actively informs language design instead of being postponed until after it;
9. the same substrate can support progressively more MNCS-native tooling and bounded self-hosting experiments.

## Non-goals

This track does not:

- freeze final module/package/import syntax now;
- clone Rust, C++, Zig, Go, Python, or another language's standard library hierarchy mechanically;
- make every host service implicitly available;
- define logical values by one target ABI or memory layout;
- require one backend or runtime;
- treat WASM as the canonical target;
- let Forge or backend intrinsics define library semantics;
- claim a production-ready stdlib before memory, storage, allocation, package, and compatibility semantics are mature;
- move to a separate repository before the semantic/package boundary is stable;
- treat self-hosting as removal of bootstrap trust.

The standard library should become a proving ground for the same central MNCS idea as the compiler itself: **stable explicit semantics, multiple evidence-bearing realizations, and machine-oriented freedom without hidden meaning or authority.**
