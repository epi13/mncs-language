# MNCS-native JIT / execution architecture

Status: **implemented, experimental** (branch `feat/mncs-native-jit`).
Implementation: `library/jit/` (MNCS language). Tests:
`examples/execution/jit-*-corpus.json`,
`crates/mncs-cli/tests/jit_orchestration.rs`,
`crates/mncs-embed/tests/jit_execution.rs`.
Pressure registry: `docs/development-evidence/jit-pressure-2026-09.md`.
Decision record: `rfcs/0048-mncs-native-jit-execution-architecture.md`.

## 1. Purpose

The MNCS JIT subsystem is the reusable execution **orchestration**
layer between validated compiler output and native execution. It owns
everything that is provider-independent:

```text
session state            (mncs.jit.session.v1)
execution planning       (mncs.jit.plan.v1)
definition generations   (mncs.jit.session.v1)
logical bindings         (mncs.jit.binding.v1)
redefinition             (mncs.jit.session.v1 + mncs.jit.binding.v1)
dependency state         (mncs.jit.depends.v1)
invalidation             (mncs.jit.depends.v1)
compiled artifact identity + metadata  (mncs.jit.types.v1)
provider selection       (mncs.jit.plan.v1)
code lifetime            (mncs.jit.lifecycle.v1 + mncs.jit.binding.v1)
profiling hooks          (mncs.jit.profile.v1)
specialization hooks     (mncs.jit.proof.v1)
proof-aware execution metadata         (mncs.jit.proof.v1)
```

It is deliberately **not** `MNCS -> Cranelift`. Cranelift is one
execution provider behind a provider-neutral interface. The same MNCS
orchestration code runs unchanged on the reference provider, the
Cranelift provider, and (by construction, verified in corpus) every
other executable backend.

Ownership boundaries used throughout this document:

```text
BOOTSTRAP COMPILER RESPONSIBILITY — temporary Rust implementation
JIT ARCHITECTURE — must remain MNCS-native
FUTURE SELF-HOSTED COMPILER RESPONSIBILITY
```

## 2. Provider architecture

Providers own only provider mechanics:

```text
lower representation        (existing backend lowering, untouched)
compile/execute representation
publish executable artifact (provider memory keyed by MNCS artifact code)
invoke executable artifact
retire provider-specific artifact
```

The MNCS layer exchanges provider-neutral metadata (u64 codes from
the `mncs.jit.types.v1` tables) plus opaque artifact identities. The
provider table:

| code | provider  | status      |
|------|-----------|-------------|
| 0    | Reference | implemented (direct execution) |
| 1    | Cranelift | implemented (first native provider) |
| 2    | LLVM      | named future class (selection refused today) |
| 3    | Native    | named future class (selection refused today) |

LLVM and Native exist as first-class names so policy code can reason
about them, but selecting them is an explicit deferred/unsupported
verdict — never a silent substitution and never faked execution.
`provider_is_implemented` is the single gate; `note_compiled`
refuses unimplemented providers (reason 6).

The first execution policy (`select_provider`):

```text
OneShot      -> Reference (no native compile for one-shots)
Interactive  -> Cranelift when ready, else Reference + deferred flag
HotStable    -> Cranelift when ready + llvm_advisory flag
Accelerator  -> refused (no provider faked)
```

No HotSpot-style thresholds are hard-coded: tier boundaries and
promotion candidacy take thresholds as parameters
(`recommend_tier`, `hot_candidate`).

## 3. Session architecture

A session is a persistent **value**, not an opaque handle. The host
threads it between calls (the `task` pattern); the language keeps no
ambient store. One `Session` record holds separable state:

```text
semantic_generation / proof_generation   era (proof kernel linkage)
next_logical / next_generation /         allocator counters
  next_artifact / publish_seq
closed                                   freeze flag
defs[8]                                  source-generation history
bindings[8]                              logical -> executable selection
artifacts[16]                            executable metadata registry
edges[16]                                dependency edges
```

Creation and destruction are deterministic total functions
(`create`, `close`). Close retires every live artifact, retires every
binding, sets `closed`, and is idempotent; every mutating entry point
rejects a closed session with the value unchanged.

Capacities are explicit (8 definitions, 8 bindings, 16 artifacts, 16
edges, 4 deps per define call). Full tables are explicit rejection
verdicts, never silent loss. These bounds fit the backend step
budgets with margin (heaviest corpus flow: ~50k reference steps).

## 4. Generation / binding model

```text
foo (logical 1, stable)
 ├── source generation 1 (content hash A, era (1,1))
 │     └── executable 1 (cranelift, Active)      <- binding (gen 1)
 ├── source generation 2 (content hash B, era (1,1))
 │     └── executable 3 (cranelift, Active)      <- binding (gen 2)
 └── source generation 3 (content hash C, era (1,2))
       └── (publish refused: stale proof)
```

- Logical identity (`logical`) is allocator-owned and stable across
  redefinition. Definition names are host-assigned u64 codes (the
  `token_set` pattern: MNCS has no string type).
- Source generations form one global counter (a total order across
  the session, free determinism). History rows are never mutated.
- Resubmitting identical content is refused as duplicate (reason 4):
  a new generation must mean new bytes.
- Redefinition snapshots the current selection into
  `previous_generation` / `previous_artifact`, parks the binding
  Stale with no artifact, and never touches old executables.
- `note_compiled` records a provider compilation (Compiled state).
- `publish` validates owner, generation currency, lifecycle, and
  proof era, then swings the binding atomically: new artifact
  Active, previous Active demoted to Published (alive, serving no new
  calls), binding Valid with a new `publish_seq`.
- `abandon` rolls a Stale binding back to its previous selection.
  A failed generation therefore cannot replace the active binding.
- `resolve` reports found/generation/artifact/state plus
  `proof_current`; the driver invokes only Valid + proof-current
  selections.

## 5. Invalidation model

Edges `user -> used` carry a kind (Call, Signature, Layout).
Policy matrix (`classify_dependent`):

| change \ edge | Call | Signature | Layout |
|---------------|------|-----------|--------|
| BodyOnly      | Unaffected | Unaffected | Unaffected |
| Signature     | RequiresRecompile | RequiresRecompile | ValidRoutesNew |
| Contract      | RequiresRecompile | RequiresRecompile | RequiresRecompile |

Transitive dependents (depth >= 2) escalate review-forcing changes to
RequiresRecompile. Unknown change/kind codes fail closed to
RequiresRecompile. RequiresRecompile marks the dependent binding
Stale (reason 2 dep-signature / 3 dep-contract); ValidRoutesNew
leaves the binding routing. The changed definition's own binding is
owned by redefinition, not by the stale pass. Closure is bounded
(8 rounds, 8 nodes) with explicit truncation verdicts.

## 6. Proof integration

Every definition row and every artifact carries
`(semantic_generation, proof_generation, assumption_bits)`.
Publication requires the artifact triple and its definition triple
to match the session era exactly; assumption bits compare for
equality (subset semantics are recorded future work, not
approximated). Era advance (`advance_proof` / `advance_semantic`)
makes older bindings report proof-stale at resolve time, so the
driver falls back / recompiles / retires through the ordinary
transitions. Specialization follows the dual-key flow
(micro-verifier AND kernel must accept; `admit_specialization`
takes both verdicts as explicit evidence). There is no speculative
unsafe optimization anywhere in this subsystem.

## 7. Lifecycle model

`Empty -> Compiled -> Published -> Active`, with `Published` and
`Active` both able to `-> Retired`, `Compiled -> Retired` (never
activated), and `Active -> Published` (superseded but alive).
`Retired` is terminal; `Empty` is unreachable backwards;
self-transitions are refused; unknown codes fail closed. The table
is enforced in one place (`transition_allowed`) and exercised both
by the single-artifact paths and by `retire_window` (session close).
Logical definition lifetime, compiled generation lifetime, active
binding lifetime, and provider artifact lifetime are distinct
concepts with distinct states; nothing here implies provider memory
can be reclaimed, and nothing leaks silently: retirement is always
an explicit transition.

## 8. Current Cranelift implementation

Cranelift is consumed through the **existing** lowering and JIT
paths (`crates/mncs-codegen/src/cranelift_backend.rs`); no second
Cranelift pipeline was created:

```text
canonical compiler IR
      ↓
existing Cranelift lowering (selected-SSA -> scalar -> CLIF)
      ↓
JIT publication path (one-shot execute / stateful session)
```

- The 32 JIT corpus candidates execute as Cranelift machine code
  with byte-identical results to the reference layers.
- The e2e driver compiles each definition source to a fresh
  Cranelift artifact per source generation (whole-program artifact;
  old artifacts stay alive in retained provider sessions keyed by
  MNCS artifact code) and invokes through the logical binding.
- Measured (see pressure registry for full table): tiny definition
  sources compile to Cranelift in ~0.1 s; the full orchestration
  program compiles in ~55-60 s; stateful open amortizes to ~7 s;
  per-call marginal cost is milliseconds.

Today's generational publication is whole-program artifacts per
generation. True incremental Cranelift publication (one JIT module,
many definitions) is future provider work; the MNCS layer already
speaks only in generations and codes, so it will not change.

## 9. Planned interpreter / VM and LLVM providers

- Interpreter / direct execution: the Reference provider (0) is the
  standing member of this class. A future bytecode VM provider joins
  the same table with no orchestration changes.
- LLVM JIT: named (code 2) with an advisory flag
  (`llvm_advisory`) marking hot stable definitions as tiering
  candidates. Wiring execution is future provider work; the plan
  layer already routes honestly until then.
- Native MNCS machine-code backend: named (code 3); same story.
- Accelerator: workload class 3 exists; selection is refused until
  a provider exists.

## 10. Relationship to future `mncs-compiler`

`mncs-compiler` is untouched in this run and remains the parity
target. When it reaches parity, this subsystem moves without
redesign: it depends only on settled language surface (records,
enums, match, bounded iteration, generics, u64 arithmetic) plus two
host conventions (u64 codes in, wall-clock millis in). Boundaries
that must become compiler-owned over time:

```text
FUTURE SELF-HOSTED COMPILER RESPONSIBILITY:
- content hashing of definition sources (host fingerprint today)
- signature/layout change detection (host-declared change kinds today)
- proof-era advancement hooks (explicit advance_* calls today)
- incremental native publication (whole-program artifacts today)
```

## 11. Future extraction into `mncs-jit`

Extraction unit: `library/jit/` (8 modules) + its 8 corpora + the
two test files' JIT portions + this document. The modules import
only `mncs.core.logic.v1` outside the family, use no backend-
specific surface, and exchange only u64/bool/record values across
their boundaries — the planned `mncs-jit` package boundary is the
current directory boundary. Known extraction friction (tooling
pressure, not redesign): embedders need a public file/library
module resolver (the CLI's is private; the e2e test reimplements
~20 lines).

What belongs where:

```text
mncs-jit        session/planning/generations/bindings/invalidation/
                profiling/tiering-hooks/specialization/proof-metadata/
                lifecycle  (this subsystem, as-is)
mncs-compiler   parsing/resolution/typing/proofs/IR/optimization/
                lowering (canonical semantics; JIT consumes results)
runtime         effect realization, clocks, grants (host observations in)
providers       Cranelift/LLVM/VM lowers + artifact memory + invoke
mncs-lab / REPL interaction policy only (which buttons call define/
                publish/abandon; no orchestration logic)
```

## 12. Pressure findings

Full registry:
`docs/development-evidence/jit-pressure-2026-09.md`. Headlines:

- Zero production Rust changes were needed: the entire
  orchestration layer compiled under the existing 0.10 surface.
  (Two test-only Rust files were added; no `crates/` production
  code was touched.)
- Elaboration does not unify `select` literal arms with a
  let-annotation (`as u64` casts required); oversized literals
  default to i64.
- No chained `a.b.c[i]` projections (single-level only).
- `next` and `over` are reserved words (naming friction).
- No strings/maps/heap/time in pure MNCS: host-assigned codes,
  bounded tables, threaded millis (all established patterns).
- Cranelift one-shot execution recompiles per call; the stateful
  session amortizes it (measured ~9x on open). Incremental
  publication is the next provider pressure.
- Test embedders need a public library resolver.
