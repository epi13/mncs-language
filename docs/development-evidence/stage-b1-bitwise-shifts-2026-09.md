# Stage B1: total integer bitwise operators and shifts (P-009 core)

## Contract

- `^ & |` elaborate over all eight integer widths under wrapping intent
  (pure: no overflow obligations). Both operands share one integer type;
  result keeps the operand width.
- Shift counts are uniformly u64 and reduce modulo the value width at
  realization (oversized counts are well-defined, never poison/UB).
- `shr` on signed values is arithmetic; `shr` on unsigned is logical.
- Result literal expectations thread into bitwise operands, as with
  `Add/Sub/Mul` (`3988292384 ^ shifted` adapts under a u64 context).

## Evidence

- `examples/source/stage-b1-bitwise-shifts.mncs` (41 functions) and
  `examples/execution/stage-b1-bitwise-corpus.json` (341 cases, Python
  oracle): 341/341 value agreement on all five executable backends
  (reference, portable WASM, C11, LLVM IR, Cranelift).
- Committed test `crates/mncs-cli/tests/stage_b1_bitwise.rs` runs the
  corpus per backend and requires logical value agreement (PASS/UNKNOWN).
- Full gate `cargo test --workspace`: CARGO_EXIT=0 (48 ok suites);
  `cargo fmt` clean; `cargo clippy --workspace --all-targets` clean.
- Former pressure probes `crc32_u32_attempt`, `crc_u64_attempt`,
  `bitwise_u64_attempt` now elaborate with zero errors; the CRC32
  single-bit step ships as a workload function in the matrix.

## Latent defects the matrix exposed and fixed

1. Reference `shl` on signed values shifted the magnitude
   (`unsigned_domain` reduced the wrapped form mod 2^bits):
   `crates/mncs-model/src/machine_intent.rs` now masks the wrapped
   value, keeping the two's-complement bit pattern.
2. WASM scalar `shr` arms were swapped against the `unsigned` flag
   (unsigned shifted arithmetically): `crates/mncs-codegen/src/lower.rs`.
3. C11 unsigned `shr` shifted the sign-extended 64-bit variable and
   narrowed afterwards: now masks to the declared width before shifting
   (`crates/mncs-codegen/src/c11.rs`).
4. Full-range u64 never crossed native process boundaries: argv words
   parsed with `strtoll` (C11/LLVM drivers, saturating to i64::MAX) and
   Cranelift parsed argv as `i64` (rejecting u64::MAX as invalid
   request). Drivers now parse `strtoull`; Cranelift round-trips through
   i128 (`crates/mncs-codegen/src/support.rs`,
   `crates/mncs-codegen/src/cranelift_backend.rs`).
5. LLVM narrow shifts reduced the u64 count in-lane (poison for
   widths < 64, UB for C11): counts now reduce in the i64 lane and
   truncate to the value width (`crates/mncs-codegen/src/llvm.rs`).

## Pressure resolution

- P-009 integer core: RESOLVED for `^ & |`, shifts, and the CRC32
  single-bit workload. Remaining P-009 surface (256-entry table vs the
  64-element sequence bound, full byte-table CRC) belongs to Stage B2/B3.
