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
                ordering/selection; bounded sequences; byte, mask, vector, and
                weighted-partition primitives; namespace-pressure fixtures)
  std/          portable abstractions over core — encoding.v1 and ansi.v1
                provide canonical serialization and generic terminal events
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

Realization envelope: reference executors, research bytecode, portable WASM,
C11, LLVM, and Cranelift realize sequence-typed and view-typed language-level
signatures through canonical cells and packed view descriptors. Nested
composite sequence elements remain unsupported.

## Branchless vectors and masks (Profile 0.8)

`core/vector.v1` and `core/mask.v1` expose reusable bounded integer-vector
and logical-mask kernels without naming a physical SIMD width. `std/simd.v1`
adds an affine/ReLU/reduction kernel whose wrapping arithmetic intent and
fixed reduction order remain visible through HIR and SSA. The current five
executable backends agree through reference or conservative scalarized
realizations; native SIMD selection remains a later target-specific step.

## Namespace pressure (Profile 0.9)

`core/ordering.mncs` and `core/bounds.mncs` intentionally export overlapping
`clamp_i64` names. `examples/source/profile09-stdlib-namespace-consumer.mncs`
imports both with aliases and calls `order.clamp_i64`, proving that the
consumer retains two declaring-module identities without relying on import
discovery order. The same source-study report exposes its binding table and
resolution provenance.

## Modules

| File | Module | Contents |
| --- | --- | --- |
| `core/status.mncs` | `mncs.core.status.v1` | `Status { PASS, FAIL, UNKNOWN }`, dominance join (`dominate`), pair combination over `StatusPair`, decidedness predicate, dominance test |
| `core/logic.mncs` | `mncs.core.logic.v1` | total boolean algebra: `bool_not/and/or/implies/xor` |
| `core/ordering.mncs` | `mncs.core.ordering.v1` | comparison-only `min/max/clamp` for i32/i64 (no arithmetic, hence no overflow obligations) |
| `core/bounds.mncs` | `mncs.core.bounds.v1` (Profile 0.5) | comparison-only `clamp_i64`, intentionally overlapping `ordering` to pressure namespace qualification |
| `core/result.mncs` | `mncs.core.result.v1` (Profile 0.6) | the standard Result shape with real reason payloads: `Ok { value }`, `Err { reason }`, `divide`, `bounded_divide`, `value_or`, `reason_of`; exhaustive matching with payload binders |
| `core/sequences.mncs` | `mncs.core.sequences.v1` (Profile 0.8) | bounded folds, membership, counting, prefix/suffix views, reverse, extrema, find-index, equality, functional put, unsigned `u64` identity; sequence- and view-typed signatures |
| `core/bytes.mncs` | `mncs.core.bytes.v1` (Profile 0.7) | byte bitwise/shift/order, folding fingerprint, nibble split, ASCII classifiers, wrapping checksum |
| `core/numeric.mncs` | `mncs.core.numeric.v1` (Profile 0.8) | wrapping 4-lane sum/mean/centroid and L2-squared; vector kernels live in `vector.v1` |
| `core/random.mncs` | `mncs.core.random.v1` (Profile 0.6) | deterministic MMIX LCG streams, bounded draws, split/derive |
| `core/version.mncs` | `mncs.core.version.v1` (Profile 0.6) | version triples, envelopes, pre-1.0 breaking rule |
| `core/vector.mncs` | `mncs.core.vector.v1` (Profile 0.8) | wrapping dot product, masked positive sum, functional lane replacement, and vector-typed reduce/double exports |
| `core/mask.mncs` | `mncs.core.mask.v1` (Profile 0.8) | bounded any/all/none kernels plus mask-typed `any_of`/`all_of`/`none_of`/`and4`/`or4`/`xor4`/`not4`/`identity4` exports |
| `core/geometry.mncs` | `mncs.core.geometry.v1` (Profile 0.8) | typed Point/Size/Rect/Insets with containment, intersection, union, clipping, translation, alignment/place, split, disjoint/adjacent, and branchless selection helpers |
| `core/partition.mncs` | `mncs.core.partition.v1` | overflow-safe weighted four-lane partitioning with deterministic remainder allocation, zero/negative-weight normalization, cap-friendly leftovers, and explicit validity |
| `std/encoding.mncs` | `mncs.std.encoding.v1` (Profile 0.7) | canonical big-endian `encode_u16`/`u32`/`u64` and version encodings with executable round-trip laws |
| `std/simd.mncs` | `mncs.std.simd.v1` (Profile 0.8) | explicit-intent affine/ReLU/reduction kernel |
| `std/ansi.mncs` | `mncs.std.ansi.v1` | bounded ANSI/VT parsing into generic Character, Arrow, Focus, Paste, Mouse, Resize, and Unknown events |

## Current module boundary

Each library file declares one module, listed above. Source Profile 0.6 now
supports elaboration-time linking through a host-provided `ModuleResolver`:
imports are resolved transitively, dependency identity is preserved, and the
linked program is lowered and executable as one authority-checked semantic
closure. Source Profile 0.9 adds qualified and aliased routes plus an
inspectable `SemanticBindingTable`; see `docs/source-profile-0.9.md`.
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
- `partition.v1` owns weighted allocation arithmetic used by consumers such as
  `mncs-tui`; consumers do not copy its quotient/remainder logic. Its
  large-product corpus keeps the unsafe `request * weight` shortcut out of the
  authority implementation.
- `ansi.v1` owns terminal sequence meaning but not terminal I/O. A TUI or host
  adapter maps its generic events into application-specific events and retains
  malformed or unsupported input as `Unknown`.

## Backend envelope (observed 2026-08)

| Module | research bytecode | portable WASM | LLVM / C11 / Cranelift |
| --- | --- | --- | --- |
| status | PASS (30 cases) | PASS (30 cases) | PASS (30 cases) |
| logic | PASS (18 cases) | PASS (18 cases) | not re-run this tranche |
| ordering | PASS (36 cases) | PASS (36 cases) | not re-run this tranche |
| sequences | ABI corpora (signed, unsigned, extra helpers) met on all five executable backends | same | same |
| encoding | u32 and u64 sequence-typed encode/decode met on all five executable backends | same | same |
| vector | signed and unsigned vector/mask ABI corpora met on all five executable backends | same | same |
| mask | 13-case mask-only ABI corpus met on all five executable backends | same | same |
| partition | 2-case corpus returned; exact-cost status remains `UNKNOWN` | not exercised | not exercised |
| ansi consumer probe | 7-case corpus returned; exact-cost status remains `UNKNOWN` | not exercised | not exercised |

The linked `status` consumer now executes on both reference bytecode and
portable WASM. Backend rows still describe observed envelopes, not a promise
that every library module is supported by every realization.

## Consumer binding

Consumers bind through elaboration-time linking with a host resolver. The
minimal external-consumer witness is
`examples/source/library-consumer.mncs`, with its bounded execution corpus in
`examples/execution/library-consumer-corpus.json`; the CLI test runs it through
both reference bytecode and portable WASM and checks resolver provenance.
The Profile 0.9 namespace-pressure witness is
`examples/source/profile09-stdlib-namespace-consumer.mncs`, with its corpus in
`examples/execution/profile09-stdlib-namespace-consumer-corpus.json`.
`examples/consumers/ravel-core-snapshot.mncs` remains useful as an
identity-bound differential witness for RAVEL's shipped `ravel.core.v1`, and
`crates/mncs-cli/tests/library_core.rs` retains its case-by-case agreement
checks.

## Corpora

Bounded corpora live under `examples/execution/library-core-*.json` and are
regenerated by `python3 scripts/gen-library-core-corpora.py`. Regeneration is
deterministic; committed files must match generator output.
