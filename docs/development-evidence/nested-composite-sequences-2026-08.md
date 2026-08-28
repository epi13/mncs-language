# Nested composite sequences and boolean sequence elements — 2026-08

Status: **implemented / exercised, experimental**.

This tranche closes the cell-of-cells gap left after the sequence/view
process-boundary ABI: `[Point; N]`, `[[i64; N]; M]`, records that store
sequences, views over composite elements, and logical `[bool; N]`
windows now flow through source, semantics, identities, HIR, SSA,
canonical cells, backend ABI, execution, and translation validation.

## Language

- `BodyType::from_semantic_name` parses `[E; N]` / `[E; up_to M]` even
  when `E` is a nominal spelling (`bool`, a declared record).
- `BodyType::from_program` resolves those spellings against the
  program's records and finites so SSA argument admission sees
  `Sequence { Record Point }`, not `Sequence { Named("Point") }`.
- Traversal over `[bool; N]` is legal (`MNB101` still refuses other
  unresolved named element types).
- `[bool; N]` is a sequence of boolean values. It is not `mask<N>`.
  `[mask<N>; K]` is refused (`MNE105`).
- Qualified Profile 0.9 sequence elements (`[alias.Point; N]`) canonicalize
  to the declaring-type identity in the semantic function signature so
  body validation agrees with elaboration.

## ABI / cells

Nested composite elements occupy referenced cells: a sequence of
records stores one 8-byte cell reference per element; nested exact
sequences do the same; views over composite elements pack
`(offset as u32) | (length << 32)` as before. Boolean sequence
elements occupy 32-bit 0/1 slots inside the 8-byte cell stride.

Portable WASM continues to store boxed pointers as i32 in those
slots; native backends store 64-bit cell offsets. Each backend
encodes and decodes with its own realization width. Logical
agreement is required; matching status is not success.

## Library

`mncs.core.sequences.v1` adds `count_true4`, `any_true4`, and
`all_true4`. `mncs.core.geometry.v1` adds sequence-typed
`first_point4`, `contains_any4`, and `union4`.

## Evidence

Five executable backends (research bytecode, portable WASM, C11,
LLVM IR, Cranelift) agree on:

- `examples/execution/abi-nested-composites-corpus.json`
- `examples/execution/abi-bool-sequences-corpus.json`
- geometry extra `first_point4` / `contains_any4` / `union4`
- sequences extra boolean helpers

A mutant corpus with a wrong nested-record expectation is an
explicit FAIL. Overall experiment status remains UNKNOWN where
runtime-checked indexes retain unresolved obligations.

C11 no longer duplicates `module_uses_cells`; every scalar backend
uses `scalar_module_uses_cells`, which also treats `SequenceReplace`
as a cell user.

## Still unsupported

- sequences of masks or vectors
- unbounded / dynamic collections
- generic type parameters (RFC 0013)
- native SIMD selection
- executable RISC-V / eBPF / PTX (artifact-only; eBPF `CGX410`)
