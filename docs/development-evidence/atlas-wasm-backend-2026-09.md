# Atlas WASM backend evidence — 2026-09

This record documents the two generic backend pressures exposed by the
stateful Atlas JSON model. Commons is the coordination source of truth for
their lifecycle and append-only evidence:

- [`MNCS-LANG-4F3798658F55`](https://github.com/epi13/MNCS-Commons/blob/main/pressures/records/MNCS-LANG-4F3798658F55.json)
  — nested cell address normalization.
- [`MNCS-LANG-4219A56741DB`](https://github.com/epi13/MNCS-Commons/blob/main/pressures/records/MNCS-LANG-4219A56741DB.json)
  — packed bounded view lifetime across region reclamation.

The records are resolved, but the original reproductions remain preserved so
future compiler changes can distinguish a real regression from a change in
the consumer workload.

## Nested cell address normalization

At the pre-fix language revision `a0255f8`, recursive cell flattening used an
`i64` scratch local directly as the address for a WASM `i32.load`. The Atlas
model artifact was rejected at the WASM validation boundary.

The generic fix landed in commit `f527b49` in
`crates/mncs-codegen/src/lower.rs`: recursive lowering wraps an `i64` scratch
address to `i32` before a memory load. The regression test is
`nested_flatten_wraps_i64_scratch_addresses_before_memory_access`.

## Packed bounded view lifetime

After the address fix, the full Atlas render plan exposed a separate failure
at language revision `f527b49`. Packed bounded byte views are `i64`
descriptors and do not allocate, but region planning treated them as
heap-backed views and suppressed reclamation. Repeated chunks exhausted the
32 MiB arena.

The generic fix landed in commit `f9d790b`: packed bounded views are treated
as nonallocating words for region planning, preserving safe loop-region
reclamation. The regression test is
`packed_bounded_views_are_preserved_as_nonallocating_words`.

## Verification

Both fixes are available on language `main` at `e39f370` and were verified
against profile `0.16`:

- `cargo test -p mncs-codegen` passed, including both focused regressions.
- Atlas `tests.test_experimental_wasm` passed, including native WASM module
  instantiation and the full stateful model render plan.

No host-language workaround is part of the language implementation. The
fixes are generic code-generation and region-planning behavior used by Atlas
as a real consumer.
