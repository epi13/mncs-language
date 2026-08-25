# Ox Alpha tranche baseline — 2026-08-24

Status: **baseline record**. Everything below was re-verified against the working tree on
2026-08-24 before any Ox Alpha change. Historical claims are treated as historical; each
claimed gap was checked against the current implementation and marked accordingly.

## Environment identities

| Subject | Value |
| --- | --- |
| repository | `epi13/mncs-language` |
| branch | `lineage/language-integration` (superset of `main`; `main` is an ancestor) |
| head at baseline | `0bd88b572791508ca5153e47e2ef14fc56b7fba4` |
| toolchain | rustc 1.97.1 (Fedora); MSRV declared 1.85 |
| stage architecture | `mncs:compiler:stage-architecture:180ae9724a7c34ae5b45854edf692605b2c57f08cc3659d7622fc7d00bcccdf5` |
| source profiles in parser | `0.1`, `0.2`, `0.3`, `0.4`, `0.5` (`crates/mncs-syntax/src/source.rs`) |
| host | local Linux x86-64 development machine |

## Baseline check results (all green)

```text
cargo fmt --all -- --check                              # clean
cargo clippy --workspace --all-targets -- -D warnings   # clean
cargo test --workspace                                  # all suites pass
```

## Verified gap assessment (claims vs current tree)

| # | Claimed gap | Verification on 2026-08-24 |
| --- | --- | --- |
| 1 | No payload-bearing enum variants / Option-Result values | **Confirmed.** `AstFiniteType` variants are bare names; `FiniteConstruct` carries only a discriminant (`crates/mncs-model/src/body.rs`). Result-with-reason shapes must use wide records today. |
| 2 | No bounded collections / typed lists / bytes / arrays / slices | **Confirmed.** No indexable aggregate exists in any profile; iteration is the Profile 0.4 scalar-carrying bounded loop only. |
| 3 | No string/text type, hashing primitive, or canonical serialization in source | **Confirmed.** Source scalars are bool + i8-i64 plus declared finite/record types (`profile_scalar_supported`, `crates/mncs-compiler/src/frontend.rs`). Content identities stay outside MNCS source. |
| 4 | No wildcard/qualified/binding patterns | **Confirmed.** Match arms are bare variant names only. Exhaustiveness itself works. |
| 5 | No boolean operators `&&` / `\|\|` | **Confirmed.** The tokenizer lexes both operators but the grammar has no boolean binary operator. |
| 6 | No constants; no division/modulo in source subset | **Confirmed.** No const declaration form; `AstBinaryOp` has Add/Sub/Mul/comparisons only. |
| 7 | Module imports "work experimentally" | **Stale claim — corrected.** There is no import syntax anywhere in this tree. Every MNCS module is a single self-contained file (RAVEL's five modules confirm the shape). Module linking is future RFC 0014 work. |
| 8 | RFC 0035 elaboration model largely unspecified | **Confirmed.** Elaboration records an authoritative name-resolution index (`crates/mncs-compiler/src/resolution.rs`) but has no explicit scope objects, constraint generation/solving split, defaulting policy, or coherence checks. |
| 9 | Records: bytecode executes; WASM refuses params/results/carried state; LLVM/C11/Cranelift refuse record ABI crossings | **Confirmed by execution today** (probe module: record param, call, record result). WASM CGN302 "record value cannot be a function result" and "cross-function and block-carried records are unsupported"; C11 CGC302 and LLVM CGL302 scalar-envelope refusals. Research bytecode executes record modules. Matrix declares WASM `immutable_record_values_intra_function_only`, bytecode full `immutable_record_values`. |
| 10 | Exact arithmetic-cost / iteration-cost obligations remain UNKNOWN | **Confirmed.** Matrix rows carry "exact instruction cost remains UNKNOWN"; a minimal record+add probe returns overall UNKNOWN solely from unresolved integer-overflow exact-cost obligations while every corpus case passes. |
| 11 | Cranelift host-JIT execution UNSUPPORTED (executable-memory policy) | **Confirmed as documented** in `roadmap-0.5-2026-08.md`: host-scoped capability result, not a language/lowering failure. Not re-executed beyond matrix inspection in this baseline. |
| 12 | Native object/linker/executable lifecycle incomplete | **Confirmed.** External clang/gcc route only; no identity-bound native artifact lifecycle in this tree. |
| 13 | No independent differential harness vs legacy implementations | **Confirmed.** RAVEL `mncs-native-evidence.md` lists it as an open obligation with a manual mapping table only. |
| 14 | Translation validation shares semantic model/reference executor with implementation | **Confirmed** per handoff doc ("shares some model/execution code with generators"). |
| 15 | No Raspberry Pi native claim; AArch64 compile-shape study only | **Confirmed.** No Pi result claimed anywhere in evidence records. |

## Stale documentation corrections

1. **"Module imports work experimentally"** (task brief): not true of this tree. Recorded
   here; no historical file asserted imports, so no rewrite was needed.
2. **Backend matrix `supported_source_profiles`** hard-codes `0.1..0.4`
   (`crates/mncs-codegen/src/matrix.rs`) while Source Profile 0.5 exists, elaborates, and
   executes on research bytecode. Stale adapter declaration; corrected in this tranche.
3. **No Source Profile 0.6 exists.** Roadmap section 0.6 ("Surface-language and
   self-description experiments") is planned research-only and is distinct from
   source-profile numbering. This tranche introduces Source Profile 0.6 as the gated
   extension profile; Profiles 0.1–0.5 remain frozen.

## Probe artifacts from baseline verification

Probe module (transient; recorded for reproduction): record passed to a function, record
result returned through a call. Run identities are content-derived per invocation;
statuses were backend-refusal diagnostics as tabulated above, not semantic disagreement:
compilation refused realization; no PASS/FAIL execution claim exists for those backends.

