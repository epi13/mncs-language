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
- masks cross as packed 64-bit predicate words (no arena). Masks do
  not require arena storage, but they still use the canonical call-file
  protocol rather than decimal argv: argv is signed `strtoll` and
  cannot carry a packed predicate. A mask-only signature therefore
  emits a call file with kind-0 bit words even when the arena image is
  empty.
- unsigned integer cells reconstruct from the declared MNCS type.
  Slot bits are never routed through `i64` for unsigned `u64`;
  `2^63` and `u64::MAX` remain in the unsigned domain through
  sequence, view, vector, and record fields.
- portable WASM marshals the same logical values into linear memory
- C11, LLVM, and Cranelift reuse the canonical call-file / JIT arena path

Reference executors accept vector and mask arguments with the same
type-matching rules already used for sequences.

## Library

Authoritative MNCS-written entry points now execute as exports:

- `mncs.core.sequences.v1` — including `contains4`, `min4`, `max4`,
  `find_index4`, `first_or`/`last_or` (views), `put4` (sequence result),
  prefix/suffix views, unsigned `u64` identity/first access, and
  byte/bool window helpers
- `mncs.std.encoding.v1` — `encode_u16`/`u32`/`u64` and matching decodes
- `mncs.core.vector.v1` — `reduce_vec4` / `doubled4` plus unsigned
  `identity_u64` / `reduce_u64` / `positives_u64`
- `mncs.core.mask.v1` — mask-typed `any_of` / `all_of` / `none_of` /
  `and4` / `or4` / `xor4` / `not4` / `identity4` / `from_bools`

`mncs.core.geometry.v1` now imports `mncs.core.ordering.v1` instead of
copying `min`/`max`/`clamp`. `mncs.core.numeric.v1` no longer copies
`dot4_wrap` / `positive_sum4`; those remain in `vector.v1`.

## Evidence

```bash
cargo test -p mncs-codegen --lib
cargo test -p mncs-cli --test library_core -- sequence_typed encoding_sequence vector_typed
cargo test -p mncs-cli --test abi_boundary
cargo test -p mncs-cli --test profile07_bounded_data
```

Agreement is logical-value agreement: a backend that returns the
expected status with the wrong value fails. A mutant corpus with a
wrong `u64` expectation is an explicit FAIL.

Covered on research bytecode, portable WASM, C11, LLVM IR, and
Cranelift:

- signed sequence/view ABI cases
- mask-only input, mask-only result, and mask-to-mask composition
- unsigned `u64` sequence, view, vector, record-field, and encoding
  round-trips including `u64::MAX`, `2^63`, and `(2^63)+1`

## Honest remaining limitations

- Nested composite sequence elements are still unsupported
  (`nested_composite_sequence_elements`).
- Native SIMD / WASM SIMD is not selected; vector realization remains
  conservative scalar lanes (WASM packed lane widths, native 8-byte slots).
- RISC-V / eBPF / PTX remain artifact-only; eBPF still refuses >5-register
  signatures (`CGX410`).
- Finite corpus agreement is not a proof of universal ABI completeness.
- C11 stores 64-bit integers in `int64_t` slots. Unsigned compares now
  cast to `uint64_t`/`uint32_t` so `vec_gt` over `u64` does not treat
  `2^63` as negative. LLVM/Cranelift/WASM already used unsigned
  predicates from the declared integer type.
- `[bool; N]` is not a sequence element type in the current envelope;
  boolean windows remain `mask<N>` or integer 0/1 slots.
