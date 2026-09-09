# Stage C1: binary64 float arithmetic and conversions (P-001 core)

## Contract

- `f64` is the only admitted float (Profile 0.12): literals are finite
  `digits.digits` spellings (MNP198 refuses non-finite spellings);
  `f32` parses so validation reports the single-width rule, but every
  backend realizes exactly one float domain.
- `+ - * /` and the six comparisons are total over finite values under
  the non-finite trap rule: any non-finite input or result traps,
  including division by zero, `0.0 / 0.0`, and overflow to infinity.
  Every backend guards both inputs and (for arithmetic) the result.
- Conversions: int/byte/bool to float round per IEEE-754 (always
  finite); float to int/byte truncates toward zero and traps on
  non-finite or out-of-range inputs, checked against the half-open
  domain exactly (including the fractional sliver below a negative
  bound and `-0.0` into unsigned domains).
- Mixed int/float arithmetic, float sequences/vectors, and checked
  float arithmetic stay refused; boundary values are computed, never
  spelled.

## Evidence

- `examples/source/stage-c1-float-arithmetic.mncs` (24 functions) and
  `examples/execution/stage-c1-float-corpus.json` (69 cases, generated
  by `scripts/gen_stage_c1_float_corpus.py` from an independent Python
  binary64 oracle): 69/69 value agreement on all five executable
  backends (reference, portable WASM, C11, LLVM IR, Cranelift), with
  trap cases asserting `runtime_failure` and valued cases asserting
  bit-exact agreement (including `-0.0`, subnormal `1e-320`,
  underflow-to-zero, and `u64::MAX` conversions).
- Committed test `crates/mncs-cli/tests/stage_c1_float.rs` runs the
  corpus per backend and requires status plus logical value agreement
  (PASS/UNKNOWN).
- Full gate `cargo test --workspace`: EXIT=0 (50 ok suites);
  `cargo fmt` clean; `cargo clippy --workspace --all-targets` clean.

## Latent defects the matrix exposed and fixed

1. Float HIR operations fell into the SSA catch-all `_ => Effect`
   (SSA010): `ssa_kind` now maps `FloatConstant`/`Float`/`FloatCompare`
   explicitly (`crates/mncs-model/src/ssa.rs`).
2. Float arguments died at four layers with four errors: no Float arm
   in `values_agree`, `normalize_value` (both model executors),
   `value_matches_type` (both model executors), or
   `backend_input_matches` (`crates/mncs-model/src/ssa_execution.rs`,
   `crates/mncs-model/src/execution.rs`,
   `crates/mncs-codegen/src/lib.rs`). Finiteness stays a runtime trap,
   never a request rejection.
3. Native float results violated the value contract: added the
   `(Float, Float)` boundary arm with a finiteness recheck
   (`crates/mncs-codegen/src/lib.rs`).
4. LLVM emitted `trunc double to i64` (invalid IR): conversions now use
   `sitofp`/`uitofp` and guarded `fptosi`/`fptoui` with sliver-safe
   bounds (`crates/mncs-codegen/src/llvm.rs`).
5. C11 converted u64 through signed `(double)` (the ABI carries u64 in
   `int64_t` cells) and trapped nothing: u64 reinterprets first, and
   float-to-int guards with libm `trunc` behind exact power-of-two
   bounds (`crates/mncs-codegen/src/c11.rs`, `-lm` linked in
   `crates/mncs-codegen/src/native.rs`).
6. WASM had no float conversions: 8 new opcodes
   (`F64ConvertI32S/U`, `F64ConvertI64S/U`, `I32TruncF64S/U`,
   `I64TruncF64S/U`) with interpreter, binary encode, and decode, plus
   `F64Load`/`F64Store` and float edges in `emit_convert`
   (`crates/mncs-codegen/src/wasm.rs`,
   `crates/mncs-codegen/src/lower.rs`).
7. Cranelift parsed float argv as integers, missed the mapped `main`
   export, and mis-converted: argv parses per-contract (`strtod`
   decimals stay decimal into AOT), export lookup maps through
   `c_symbol`, and conversions use `fcvt` with the same guard shapes
   (`crates/mncs-codegen/src/cranelift_backend.rs`,
   `crates/mncs-codegen/src/support.rs`).
8. Pre-existing, reproduced-first: integer `fn main` never linked on
   C11/LLVM (C `main` collision). Module and driver now agree on
   `mncs_main` through one native-symbol rule
   (`crates/mncs-codegen/src/support.rs`).

## Pressure resolution

- P-001 float semantics: PARTIALLY RESOLVED. Scalar type, arithmetic,
  comparisons, conversions, trap rule, backend facts, and bounded
  corpora land here. Trig (sin/cos workloads) belongs to Stage C2.
- P-008 trig: STILL OPEN. C1 makes no trig claim; C2 lowers `sin`/`cos`
  intrinsics to same-process libm on every backend.
