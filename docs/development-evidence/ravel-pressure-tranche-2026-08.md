# Development Evidence — RAVEL-driven arithmetic intents, library resolution, and cross-module types (2026-08)

## Provenance

This tranche was driven end to end by reconstructing RAVEL as a real linked
MNCS application (`epi13/RAVEL` `mncs/workspace/ravel/`). Every feature below
exists because the application required it; each carries executable evidence in
this repository and is consumed by RAVEL's Forge checks.

## Features landed (main at `04bbf12`)

### 1. Explicit wrapping/saturating arithmetic intents

- Surface: `+% -% *%` (wrapping), `+| -| *|` (saturating); additive and
  multiplicative precedence respectively; gated below Profile 0.6 with
  `MNP143`.
- Elaboration maps them to `ArithmeticIntent::Wrapping` / `.Saturating`; the
  obligation generator already discharged those intents by semantics, so
  source programs finally gain access: overflow obligations become
  `language-explicit-{intent}-semantics` PASS instead of symbolic UNKNOWN.
- Realization completed across backends:
  - C11: 128-bit wide intermediates for checked/saturating. **Fixes a latent
    bug**: the previous i64 checked path computed the guard on an `int64_t`
    wide value, so an actual i64 wrap could not trip it; also fixes decimal
    `-9223372036854775808` literals materializing as unsigned constants.
  - LLVM IR: saturating intrinsics for add/sub; exact with.overflow +
    boundary select for multiplication; unsigned narrow returns now
    zero-extend (was sign-extend).
  - Cranelift JIT/AOT: i128-widened computation for 64-bit checked guards
    (**same latent wrapped-guard bug fixed**) and saturating clamps; narrow
    wrapping results normalized into the shared i64 cell.
  - portable WASM: three new core instructions (`i64.eqz`, `i64.shr_s`,
    `select`) through encoder, decoder, and interpreter; exact 64-bit signed
    saturating lowering via bit-overflow formulas and guarded division.
    Unsigned 64-bit saturating multiplication still refuses honestly.
- Evidence: `examples/source/profile06-explicit-intents.mncs` +
  16-case boundary corpus agreeing PASS on research bytecode, WASM, C11,
  LLVM, Cranelift (`crates/mncs-cli/tests/profile06_explicit_intents.rs`);
  profile-gate negative test.

### 2. `MNCS_LIBRARY_PATH` standard-library resolution

The research CLI resolves `use` targets beyond the importing tree against
`:`-separated library roots, so external consumers bind to `mncs.core.*`
without vendoring. Resolution misses remain honest `MNE173`s. Documented in
`docs/source-profile-0.6.md`; regression-tested in
`crates/mncs-cli/tests/library_resolution.rs`.

### 3. Imported types nameable in field/payload positions

Elaboration checked record-field and variant-payload type names against local
declarations only, making cross-module composition of records over imported
types impossible (discovered immediately by RAVEL task contexts). The visible
name sets now union successfully merged imports; collisions still fail closed
during merging. Regression: `module_imports.rs ::
imported_types_are_nameable_in_field_and_payload_positions`.

## Consumption proof

RAVEL main runs nine modules — five upgraded, four new — as one linked program
importing `mncs.core.status.v1`, `mncs.core.logic.v1`, and `ravel.types.v1`
through these features. All nine report PASS with zero unresolved obligations
on both executable backends; previously-UNKNOWN modules resolved their
obligations through feature 1. An executable legacy differential harness
(`RAVEL tools/ravel_mncs_differential.py`) reports AGREE 28/28 over its three
executable scopes.

## Non-claims

Bounded corpora and differential scopes only; no universal equivalence,
conformance, or independent evaluation.
