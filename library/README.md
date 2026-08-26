# MNCS standard library (workspace-local)

This tree hosts the MNCS standard library, written in the MNCS source language
itself and verified by the same compiler/experiment pipeline as every other
program. It is semantically authoritative at the MNCS level: consumers bind to
these modules by identity and bounded agreement, never by re-implementing the
same helpers ad hoc.

## Layout

```text
library/
  core/         foundational total operations (status lattice, boolean algebra,
                ordering/selection; bounded sequences; byte, mask, and vector primitives)
  std/          portable abstractions over core — encoding.v1 begins the
                canonical serialization groundwork (RFC 0027 direction)
  capability/   reserved for capability/effect-shape declarations
```

`capability/` remains empty until its governing language features exist; no
placeholder syntax is invented to fill it.

## Bounded data (Profile 0.7)

`core/sequences.v1` and `core/bytes.v1` are the standard-library vocabulary
over the new substrate: folds, membership, counting, option-shaped
first/last access, sorted scans, byte bitwise/shift/ordering primitives,
and a reproducible folding fingerprint. `std/encoding.v1` is the first
portable `mncs.std` module: canonical big-endian encodings whose decodes
are total by type and whose round-trip laws are executable MNCS functions.

Realization envelope: reference executors and research bytecode realize
these modules fully. The scalar backends refuse sequence-typed boundary
crossings this tranche, so differential corpora for these modules use
scalar-entry wrappers where cross-backend agreement is required.

## Branchless vectors and masks (Profile 0.8)

`core/vector.v1` and `core/mask.v1` expose reusable bounded integer-vector
and logical-mask kernels without naming a physical SIMD width. `std/simd.v1`
adds an affine/ReLU/reduction kernel whose wrapping arithmetic intent and
fixed reduction order remain visible through HIR and SSA. The current five
executable backends agree through reference or conservative scalarized
realizations; native SIMD selection remains a later target-specific step.

## Modules

| File | Module | Contents |
| --- | --- | --- |
| `core/status.mncs` | `mncs.core.status.v1` | `Status { PASS, FAIL, UNKNOWN }`, dominance join (`dominate`), pair combination over `StatusPair`, decidedness predicate, dominance test |
| `core/logic.mncs` | `mncs.core.logic.v1` | total boolean algebra: `bool_not/and/or/implies/xor` |
| `core/ordering.mncs` | `mncs.core.ordering.v1` | comparison-only `min/max/clamp` for i32/i64 (no arithmetic, hence no overflow obligations) |
| `core/result.mncs` | `mncs.core.result.v1` (Profile 0.6) | the standard Result shape with real reason payloads: `Ok { value }`, `Err { reason }`, `divide`, `bounded_divide`, `value_or`, `reason_of`; exhaustive matching with payload binders |
| `core/vector.mncs` | `mncs.core.vector.v1` (Profile 0.8) | wrapping dot product, masked positive sum, and functional lane replacement |
| `core/mask.mncs` | `mncs.core.mask.v1` (Profile 0.8) | bounded any/all/none predicate kernels |
| `std/simd.mncs` | `mncs.std.simd.v1` (Profile 0.8) | explicit-intent affine/ReLU/reduction kernel |

## Current constraint: one module per file

Source Profiles 0.1–0.5 have **no import/linking form**. Every MNCS module is a
single self-contained file (this is also how RAVEL's reconstruction modules are
shipped). Library files therefore declare one module each, listed above.
`core/result.mncs` additionally requires the Profile 0.6 payload-sum surface;
see `docs/source-profile-0.6.md`.

## Contracts and honesty properties

- Every operation is total and effect-free; no capability is declared.
- The status lattice can only weaken claims: nothing here maps UNKNOWN to
  PASS or deletes FAIL. The negative fixture
  `examples/source/library-core-status-wrong.mncs` proves the corpus
  discriminates an UNKNOWN-laundering mutant on exactly that cell.
- `ordering` deliberately avoids contract clauses: current profiles cannot yet
  discharge contract-evidence obligations from source, so declaring one would
  leave the module permanently UNKNOWN for reasons no corpus can resolve.
  Preconditions are documented in comments instead.
- Exact-cost obligations remain UNKNOWN where arithmetic exists elsewhere;
  these modules use comparisons/matches only and decide PASS end to end.

## Backend envelope (observed 2026-08)

| Module | research bytecode | portable WASM | LLVM / C11 / Cranelift |
| --- | --- | --- | --- |
| status | PASS (30 cases) | refuses record parameter `StatusPair` (CGN302) | not exercised (scalar-envelope refusals expected) |
| logic | PASS (18 cases) | PASS (18 cases) | not exercised |
| ordering | PASS (36 cases) | PASS (36 cases) | not exercised |

WASM's refusal of the record parameter is an intentional realization envelope,
not semantic disagreement; see `docs/development-evidence/ox-alpha-tranche-2026-08.md`.

## Consumer binding

Until RFC 0014 module linking lands, consumers bind by **identity-bound
differential agreement**: `examples/consumers/ravel-core-snapshot.mncs` freezes
RAVEL's shipped `ravel.core.v1` (canonically identical to upstream), and
`crates/mncs-cli/tests/library_core.rs` asserts case-by-case agreement of its
`dominate` with the library join over the full 3×3 domain. Symbol-level import
and consumption is recorded as blocked on imports in that evidence file.

## Corpora

Bounded corpora live under `examples/execution/library-core-*.json` and are
regenerated by `python3 scripts/gen-library-core-corpora.py`. Regeneration is
deterministic; committed files must match generator output.
