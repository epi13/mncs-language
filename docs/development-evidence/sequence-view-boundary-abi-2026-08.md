# Sequence, view, vector, and mask process-boundary ABI — 2026-08

Status: **implemented / exercised, experimental**.

This tranche closes the composition gap left after canonical composite
cells and the Profile 0.7 bounded-data substrate: sequences, views,
vectors, and masks already lowered internally, but language-level
signatures were classified as scalars and refused at the process
boundary.

## What landed

`BackendValueContract` now includes `Sequence`, `View`, `Vector`, and
`Mask`. Function value contracts classify `[E; N]`, `[E; up_to M]`,
`vec<T, N>`, and `mask<N>` from the same semantic names used by body,
HIR, and SSA.

Process-boundary realization:

- exact sequences and vectors cross as canonical cell roots
- views cross as packed descriptors `(offset as u32) | (length << 32)`
  with element storage in the same arena
- masks cross as packed 64-bit predicate words (no arena)
- portable WASM marshals the same logical values into linear memory
- C11, LLVM, and Cranelift reuse the canonical call-file / JIT arena path

Reference executors accept vector and mask arguments with the same
type-matching rules already used for sequences.

## Library

Authoritative MNCS-written entry points now execute as exports:

- `mncs.core.sequences.v1` — including `contains4`, `min4`, `max4`,
  `find_index4`, `first_or`/`last_or` (views), and `put4` (sequence result)
- `mncs.std.encoding.v1` — `encode_u32` / `decode_u32`
- `mncs.core.vector.v1` — `reduce_vec4` / `doubled4`

`mncs.core.geometry.v1` now imports `mncs.core.ordering.v1` instead of
copying `min`/`max`/`clamp`. `mncs.core.numeric.v1` no longer copies
`dot4_wrap` / `positive_sum4`; those remain in `vector.v1`.

## Evidence

```bash
cargo test -p mncs-codegen --lib
cargo test -p mncs-cli --test library_core -- sequence_typed encoding_sequence vector_typed
cargo test -p mncs-cli --test profile07_bounded_data
```

Nine sequence ABI cases, four encoding ABI cases, and two vector ABI
cases met on research bytecode, portable WASM, C11, LLVM IR, and
Cranelift.

## Honest remaining limitations

- Nested composite sequence elements are still unsupported
  (`nested_composite_sequence_elements`).
- Native SIMD / WASM SIMD is not selected; vector realization remains
  conservative scalar lanes (WASM packed lane widths, native 8-byte slots).
- RISC-V / eBPF / PTX remain artifact-only; eBPF still refuses >5-register
  signatures (`CGX410`).
- Finite corpus agreement is not a proof of universal ABI completeness.
