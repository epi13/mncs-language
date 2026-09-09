# MNCS Host ABI — version 1

Status: normative for the portable-WASM function boundary. This document is
the stable host contract (P-011): a host MUST be able to call an MNCS WASM
artifact using only this document plus the per-module `mncs abi` report,
without reading compiler source. Anything not stated here is a replaceable
implementation detail and MUST NOT be relied upon.

Version history:

- `1` (current): initial versioned contract. Covers scalar, boolean, byte,
  integer, exact-sequence, bounded-view, record, and finite value transfer;
  the host-buffer staging protocol; allocator/reset discipline; memory and
  export envelope; failure taxonomy. The bit-63 cell selector on returned
  byte views is explicitly PROVISIONAL (section 6).

## 1. Envelope

- Artifacts are standard WASM MVP modules with ZERO host imports. A host
  instantiates with an empty import object.
- Every exported MNCS function is a plain WASM function export named with
  the source function name (the trailing segment of its semantic identity).
- Helper exports `mncs_alloc`, `mncs_host_buffer`, and
  `mncs_host_buffer_reset` exist exactly when the module materializes
  composites or bounded sequences; scalar-only modules MAY omit them and
  MAY omit linear memory.
- Modules that materialize composites export linear memory under the
  standard name `memory` with at least 512 initial pages (32 MiB arena
  budget). Hosts MUST NOT assume a maximum: memory MAY grow.
- `mncs abi <module.mncs>` reports the language-owned contract per
  function (parameter/result shapes, capacities, composite layouts) and
  carries `host_abi_version`. Hosts MUST check `host_abi_version == "1"`
  before applying this document; a higher version means this text no
  longer describes the artifact.

## 2. Scalar transfer

| MNCS type | WASM parameter/result | Notes |
|---|---|---|
| `bool` | `i32`, `0` or `1` | Any other value on input is `invalid_request`. |
| `byte` | `i32`, `0..=255` | Out-of-range input is `invalid_request`. |
| `i8/i16/i32`, `u8/u16/u32` | `i32` | Two's-complement low bits; signedness follows the declared MNCS type (unsigned comparisons stay unsigned through cells, views, and record fields). |
| `i64/u64` | `i64` | Full 64-bit patterns cross exactly, including `u64::MAX` and `2^63`. |
| finite (bare) | `i32` discriminant | Discriminants are variant declaration order starting at 0. |

## 3. Exact sequences

`[E; N]` crosses as an `i32` POINTER to a canonical cell root: one
eight-byte slot per element in order. Integer slots hold the full 64-bit
pattern for 64-bit elements and the low 32 bits (`i32` pattern) otherwise;
`byte` elements occupy the low bytes of their slot; `bool` elements are
`0/1` words. A host stages an exact input with `mncs_alloc(N*8)` and one
slot store per element; an exact result is read the same way.

## 4. Bounded views (inputs)

`[E; up_to M]` crosses as an `i64` PACKED DESCRIPTOR:
`offset = descriptor & 0xFFFF_FFFF` (byte address),
`length = (descriptor >> 32) & 0x7FFF_FFFF` (element count).
Hosts stage view inputs through the section-5 protocol and pass the packed
descriptor. Staged byte views are packed contiguous bytes. The declared
capacity `M` is a hard bound: a staged view longer than `M` is refused
with `invalid_request`; the 64-element profile maximum bounds every view.
The empty view is the zero descriptor (`offset 0, length 0`) and MUST be
accepted. Bit 63 of a host-staged descriptor MUST be clear.

## 5. Host-buffer staging protocol

`mncs_host_buffer(bytes: i32) -> i64` reserves `bytes` arena bytes for one
host-staged input and returns `(bytes << 32) | offset`. The canonical call
sequence per staged input is:

1. `mncs_host_buffer_reset()` — rewind to the end of the last staged
   input region (discards dead result cells, preserves staged inputs).
2. `offset = mncs_host_buffer(n) & 0xFFFF_FFFF`.
3. Refresh any cached `memory.buffer` view (growth detaches old views),
   write the `n` bytes at `offset`.
4. Pass `(n << 32) | offset` as the view argument.

`mncs_alloc(bytes: i32) -> i32` hands out 8-byte-aligned bump regions for
exact-sequence and record cells. There is no free: allocation is monotonic
within a reset cycle. Hosts MUST call `mncs_host_buffer_reset()` between
logical calls (or after reading results) or the arena grows without bound;
repeated calls WITHOUT the reset discipline grow memory linearly. Result
cells and record pointers are valid only until the next reset or memory
growth — hosts MUST copy values out before either.

## 6. Results

- Scalars, booleans, bytes, and bare finites return as section-2 values.
- Records return an `i32` pointer to canonical cells in
  NAME-SORTED field order (eight-byte slots, same slot widths as section
  3). Nested records cross as cell references.
- Exact sequences return an `i32` canonical cell root (section 3).
- Views return an `i64` packed descriptor (section 4) with one
  PROVISIONAL exception: a byte view derived inside the callee from an
  exact sequence addresses canonical cells (eight-byte stride) and carries
  bit 63 set. Hosts MUST branch on bit 63 for returned byte views: set
  means one byte per eight-byte slot starting at `offset`, clear means
  packed bytes. Bit 63 is NOT part of the length and MUST be masked before
  `>> 32`. This selector is an internal representation detail leaking at
  the boundary; a future ABI version will return packed-only descriptors.
  Hosts MUST NOT set bit 63 on staged inputs.
- A returned view longer than its declared capacity is a backend
  `runtime_failure`, never a truncated value.

## 7. Failure taxonomy

- `invalid_request`: the host violated the language-owned value contract
  (wrong arity, over-capacity view, mistyped argument, bad record
  identity, out-of-range scalar). No callee code ran.
- `runtime_failure`: a valid request trapped (out-of-bounds index or
  slice, failed bounds check, over-long returned view). Deterministic per
  input across backends.
- `budget_exhausted`: the step budget ran out. Increase the budget; the
  program is still bounded.
- Statuses are honest: `returned` carries checked logical values,
  `runtime_failure`/`invalid_request` carry reasons, and compilation
  obligations the compiler cannot discharge surface as `UNKNOWN` overall
  status with verified per-case values — never as silent PASS.

## 8. Conformance vectors

The contract is executable: each row names the corpus that proves it on
all five executable backends (`mncs-research-bytecode`,
`mncs-portable-wasm-mvp`, `mncs-c11`, `mncs-llvm-ir`, `mncs-cranelift`).

| Vector | Corpus |
|---|---|
| scalar in/out (signed/unsigned incl. `u64::MAX`, `2^63`) | `library-core-unsigned-sequences-corpus.json`, arithmetic corpora |
| bool in/out | `profile06-bool-match-corpus.json`, `profile06-boolean-operators-corpus.json` |
| byte in/out | `abi-view-returns-corpus.json` (`pick`, `exact1`) |
| byte view input (packed, staged) | `abi-view-returns-corpus.json` (`param_*`) |
| exact sequence input | `abi-nested-composites-corpus.json`, `abi-view-returns-corpus.json` (`exact_second`) |
| record input | `abi-nested-composites-corpus.json` |
| record output | `abi-unsigned-records-corpus.json` |
| sequence output (exact) | `abi-view-returns-corpus.json` (`exact1/3/64`) |
| view output (cell-backed and packed) | `abi-view-returns-corpus.json` (`s02/s13`, `odd_cell_slice`, `param_*`) |
| zero-length view | `abi-view-returns-corpus.json` (`param_empty`) |
| maximum-bound view (64) | `abi-view-returns-corpus.json` (`exact64`, `full64`) |
| repeated composite calls | `abi-view-returns-corpus.json` (`s02_repeat`) |
| invalid descriptor (over capacity) | `library-core-view-over-capacity-corpus.json` (`invalid_request`) |
| deterministic traps (OOB index, reversed slice) | `abi-view-returns-traps-corpus.json` (`runtime_failure` everywhere) |
| allocator/reset discipline | unit evidence (`mncs-codegen` allocator tests) + host-side reset protocol (section 5); cross-call growth is a host-test obligation, not covered by per-case experiment runs |

## 9. What is NOT contracted

- Arena addresses, bump offsets, and cell placement (observe, never assume).
- Step counts and artifact byte sizes (evidence, not semantics).
- Anything about `mask<N>`/vector lane internals beyond the value contract.
- The bit-63 selector's future (see section 6): code against both
  representations today.
