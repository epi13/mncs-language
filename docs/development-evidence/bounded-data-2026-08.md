# Development Evidence — bounded data substrate (bytes, sequences, views), 2026-08

Tranche: Source Profile 0.7 + RFC 0044. All experiments are local
development evidence over finite corpora; nothing here is independent
evaluation, formal verification of the compiler, or a production claim.

## Implemented and exercised

- **Language surface** (`docs/source-profile-0.7.md`): `byte`, `[E; N]`,
  `[E; up_to M]`, `xs[i]` with three-state bounds evidence, `xs[a..b]`
  checked views, `xs.len`, `iterate i over xs carrying ...` traversal,
  `(e as T)` total conversions, bool-outward conversion, integer shifts.
  Gated to Profile 0.7 with coded diagnostics (`MNP144`–`MNP153`);
  profiles 0.1–0.6 fixtures pass unchanged (identity compatibility via
  serde defaults).
- **Machine knowledge preserved end to end**: bounds evidence survives
  body → obligations → HIR → SSA → backend lowering; runtime-checked
  indexes mint `sequence-index-bounds` obligations (UNKNOWN) and view
  constructions mint `view-range-valid` obligations (UNKNOWN);
  traversal-domain and statically-exact accesses discharge by semantics.
  Static out-of-domain literal indexing is an elaboration error (`MNE192`)
  rather than a deferred runtime failure.
- **Backends**: research bytecode and portable WASM realize the full
  envelope; C11, LLVM IR, and Cranelift realize bytes, casts, shifts,
  internal fixed sequences/views through canonical cells plus packed
  `(offset | length << 32)` descriptors; external RISC-V/PTX artifact
  generation continues through shared LLVM lowering; eBPF refuses
  >5-register signatures precisely (`CGX410`).
  **Follow-up (2026-08 sequence ABI):** language-level sequence/view/vector/mask
  signatures now also cross the process boundary; see
  `sequence-view-boundary-abi-2026-08.md`. Nested composite sequence
  elements remain unsupported.
- **Differential agreement** (layered body/SSA/backend validation PASS on
  every executable backend):
  - `examples/execution/profile07-bounded-data-corpus.json` — 5 cases:
    sequence fold, byte checksum, view windowing with runtime length,
    count-over-threshold search fold, byte bitwise mix.
  - `examples/execution/profile07-serialization-corpus.json` — 2 cases:
    header encode/decode round trip and magic rejection, consuming
    `mncs.std.encoding.v1` and `mncs.core.bytes.v1` through imports.
- **Standard library** (written in MNCS): `mncs.core.sequences.v1`
  (folds, membership, counting, option-shaped first/last, sorted scan,
  equality, positional update), `mncs.core.bytes.v1` (bitwise, shifts,
  ordering, folding fingerprint), `mncs.std.encoding.v1` — first real
  `mncs.std` module: canonical big-endian u32/version encodings with
  executable round-trip laws.
- **Family pressure test (RAVEL)**: new `ravel.identity.v1` module
  replaces the "embedding-free integer identity" stand-in with bounded
  `[byte; 8]` content digests, bytewise agreement over traversal, dead
  digest round-trip law, and a deterministic SnapshotId migration path;
  agrees across research-bytecode and portable-wasm corpora.

## Real bugs found by pressure-testing (fixed)

1. **WASM checked narrow arithmetic underflow** — checked add/sub/mul on
   sub-32-bit result widths emitted an unbalanced operand-stack mask after
   the destination store; any such program failed with "operand stack
   underflow". Found by `u16 /` inside the serialization module.
2. **WASM i64 division guard used i32 zero test** — `% 4294967296` has all
   low 32 bits zero, so the I32Eqz guard falsely trapped. Guard now matches
   the divisor carrier width.
3. **LLVM cell payload width mismatch** — cell store/load forced i32
   payloads regardless of the value's realization type; byte-typed values
   crossing cells silently corrupted neighbors (i8 slot stored as i32).
4. **Layout width for nominal-typed fields** — record-typed variant payload
   fields took W32 layout slots although they carry W64 cell references,
   corrupting every projection of a record-in-sum payload on scalar
   backends.
5. **Record fields could not name bounded-sequence types** — elaboration
   rejected the spellings outright; now resolved through the provisional
   namespaces like other element types.

## Explicit non-claims

- No unbounded/dynamic allocation, mutation, ownership/lifetime theory,
  general recursion or loops, nested `over` iteration, text types,
  generalized generics, wire-format freezing, or ABI stabilization.
- Sequence/view values do not yet cross C11/LLVM/Cranelift/WASM process-
  boundary marshal contracts; refusals are coded and recorded in each
  capability manifest.
- Finite corpus agreement is behavioral evidence only; it is not proof of
  universal semantic equivalence or backend correctness (shared helpers
  can create common-mode defects).
- Unresolved obligations (checked arithmetic, runtime-checked indexes,
  view ranges) keep experiment status UNKNOWN honestly; no PASS was
  manufactured from incomplete evidence.
