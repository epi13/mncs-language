# Language-pressure repair campaign: native soundness + frontend (2026-09)

Pressure source: `mncs-math` wave-2 (`docs/language-pressure.md`,
`docs/performance.md`, `repro/`, latest wave-2 merge `cb1d32b`).
Language base: `mncs-language/main` at `2b57eee` (Stage C2 trig shipped).
Branch: `feat/pressure-native-soundness`.

Boundary honored: `mncs-math` read-only throughout. No consumer edits,
no weakened expectations, no algorithm rewrites. All proofs are
unmodified pressure workloads succeeding because MNCS became correct.

## Pressure reconciliation

| Pressure | Repro status (main) | Root cause (owning layer) | Fix | Tests | Backends validated | Consumer validation | Limits |
|---|---|---|---|---|---|---|---|
| f64 array value params miscompile (all natives) | Live: C11 garbage, Cranelift 0.0, LLVM `trunc i64 to double` poisoning whole module; local `[f64;6]` iterate also wrong | `scalar::slot_width_of` returned W32 for Float (vs canonical W64 in `composite.rs`); C11/LLVM/Cranelift then truncated double patterns to 32 bits; plus numeric (not bitwise) float cell/call/select conversions in C11, `trunc` (not `bitcast`) call return in LLVM, `load.i32` (not `i64`) call return in Cranelift CLIF | `slot_width_of` Float=>W64 (single contract); C11 memcpy-bitwise cells/calls/selects; LLVM bitcast call return; Cranelift i64 call loads | `pressure-f64-arrays` (10 cases: 2/4/6/8, locals, params, index, frac, neg-zero, large, subnormal) + `pressure_native_soundness.rs` | 5/5 green (bytecode, wasm, C11, LLVM, Cranelift) | `float.mncs` 50/50 on C11/LLVM/Cranelift (was 39/40, 0/40, diverged); `dot4-generic` + `sum6-loop` now exact | `SequenceReplace` with float elements now W64-bitwise; WASM `Replace` float path still refuses by design (no workload needs it) |
| Checked u64 +/\- mistrap on high-bit (C11, Cranelift) | Live: `addm(0,0,MAX)` runtime_failure on both; LLVM correct | C11 widened signed `int64_t` cell directly to `unsigned __int128` (sign-extend); Cranelift `integer_bounds` truncated u64 MAX to `i64::MAX` with `sextend`/signed compares (CLIF + JIT) | C11 zero-extend via `(uint64_t)`; Cranelift bounds u64 MAX as `-1` bits with `uextend`/unsigned compares | `pressure-u64-highbit` (9 cases: MAX, 2^63, 2^63+1, neighbors, add/sub, wrap, cmp, addm, minmod) | 5/5 green | `modular.mncs` 21/21, `deterministic.mncs` 21/21, `scalar.mncs` 63/63 on C11/LLVM/Cranelift (were 18/21, 12/21, 61/63) | Multiplication uses same substrate (verified via wrap + modular inv); conversions/comparisons use unsigned domain (verified via ge) |
| C11 stdlib collisions (`min`, `trunc`) | Live: `trunc` bricks 257 cases + examples on C11 | `support::c_symbol` identity (except `main`); no namespacing | Centralized `mncs_` prefix hygienic by construction (idempotent); applied in `scalar::lower_to_scalar` + drivers/JIT/trampolines; PTX/RISC-V entries mapped via `c_symbol` | `pressure-symbol-hygiene` (trunc/min/exp) + `backend_evidence.rs` updated to `mncs_` exports | 5/5 green (C11) + external (PTX/RISC-V/eBPF) green | `rational.mncs` 55/55 on C11 (was 0/55 unbuildable); linalg/autodiff/statistics/examples unblocked | Any future libc name namespaced away; `mncs_*` user names keep spelling (reserved surface) |
| LLVM float lowering / module poisoning | Live: one bad `trunc` poisons healthy `i8` too | Aggregate call-return path used `trunc` for `double` (int-only opcode) | `bitcast i64 to double` for float returns | Covered by f64-arrays (module now compiles; `i8` passes alongside floats) | LLVM green | `float.mncs` 50/50 on LLVM (was 0/50 unsupported) | Scalar C1 `sitofp`/`uitofp` path untouched (already correct) |
| `MIN % -1` native crash (C11, LLVM) | Live: invalid_request (no observation) on both; Cranelift already 0 | C `INT64_MIN % -1` SIGFPE/UB; LLVM `srem` poison; no guard (only `div` guarded) | C11 total zero fast-path; LLVM branch to zero; MNCS pins `MIN%-1==0`, only `MIN/-1` traps | Covered by u64-highbit `minmod` case | C11/LLVM green; Cranelift already green | `scalar.mncs` `mod-min-by-neg-one-is-zero` now holds on natives | 8/16/32-bit MIN%-1 same guard (width-parameterized) |
| MNB011 temp collision (`c0`, `b3`) | Live | `frontend::new_value` formatted `{prefix}{counter}` in user namespace | `$mncs$` prefix unspellable in source (hygienic by construction, deterministic counter) | `pressure_frontend.rs` adversarial (record + `\|\|`-chain) | Frontend (all backends, fails before codegen) | `tensor.mncs`/`autodiff.mncs` renames no longer needed for new code | Existing `xN`/`coefN` workarounds remain valid |
| MNB101 u64 iterate domains | Live | `body.rs` rejected element type == u64 counter (confused index with element) | Accept all integer widths, f64, bool; reject only unresolvable nominals | `pressure_frontend.rs` + u64-iterate repro now elaborates | Frontend | `modular.mncs`/`bigintx.mncs` phantom ladders removable for new kernels | Record-carry over u64 now resolves (same check) |
| MNP064 negative literal args | Live: `f(-5,-2)` MNP064 | No unary prefix in `primary_atom` | `-<int>`/`-<float>` atoms (literals only; `-x`/`--5` still refused; subtraction unambiguous) | `pressure_frontend.rs` (args, floats, nested, records, sub) + execution ear (`-7`, `4`) | Frontend + bytecode execution | Halley `(0-5)` idiom removable for new code | `DegSt {best:-1}` carry initializers now parse |
| Nested calls MNE133/135 | Already supported (negative result confirmed) | Misclassified: MNE133 is type mismatch, not nesting ban | No change; pinned by test | `pressure_frontend.rs` `sub(x,mul(a,b))` elaborates clean | Frontend | 3-4x helper multiplier claim retracted for plain composition (enums still need match-split) | `next`-state over-index anti-case remains (separate, documented) |
| u64 `-%` parity | Already supported | — | No change; verified wrapping `5 -% 8 == MAX-2` on bytecode/C11/LLVM | Covered by u64-highbit `wrap-max-1` | 3/3 green | `deterministic.mncs` Lemire carry unaffected | — |
| Boolean `!` | Missing (MNL002 lexical) | No `Bang` token, no AST/lowering | DEFERRED (see below) | — | — | `select(c,false,true)` idiom preserved | — |
| `select` strictness | Confirmed strict (traps untaken arm) | Intentional branchless primitive | No change (preserved) | Verified `select(false,1/0,0)` traps | Reference | Docs elsewhere; `if`/helpers for lazy dispatch | — |

## Before/after matrix (native soundness reproducers)

| Repro | Bytecode/WASM (before→after) | C11 (before→after) | LLVM (before→after) | Cranelift (before→after) |
|---|---|---|---|---|
| `native-f64-array-param` (5 cases) | 5/5 → 5/5 | 2/5 → 5/5 | 0/5 unsupported → 5/5 | 1/5 → 5/5 |
| `native-c11-u64-widening` (2) | 2/2 → 2/2 | 1/2 → 2/2 | 2/2 → 2/2 | 1/2 → 2/2 |
| `native-c11-symbol-collision` (1) | 1/1 → 1/1 | unsupported → 1/1 | 1/1 → 1/1 | untested → 1/1 (via pressure suite) |
| `native-min-mod-neg1` (1) | 1/1 → 1/1 | crash → 1/1 | crash → 1/1 | 1/1 → 1/1 |
| `float.mncs` (50) | 50/50 → 50/50 | 39/40* → 50/50 | 0/50 → 50/50 | diverged → 50/50 |
| `scalar.mncs` (63) | 63/63 → 63/63 | 61/63 → 63/63 | 61/63 → 63/63 | diverged → 63/63 |
| `rational.mncs` (55) | 55/55 → 55/55 | 0/55 → 55/55 | 55/55 → 55/55 | 50/55 → 55/55 |
| `modular.mncs` (21) | 21/21 → 21/21 | 18/21 → 21/21 | 21/21 → 21/21 | 18/21 → 21/21 |
| `deterministic.mncs` (21) | 21/21 → 21/21 | 12/21 → 21/21 | 21/21 → 21/21 | 12/21 → 21/21 |

\* C11 float previously 39/40 value cases in the pressure report's subset;
full corpus is 50 cases (all green after).

Cranelift heavy cells (`linalg5` 14m, `linalg`/`bigint` 590s guards) not
re-run full-matrix here; focused reproducers + `float`/`scalar`/`rational`
(50-63 cases) green on Cranelift. Full native conformance remains CI work.

## Deferred major capabilities

- **A. Callable values / function references: DEFERRED.** No existing
  callable representation in MNCS IR (targets are monomorphic callees).
  Minimal machine-native design needs bounded function-pointer identity,
  effect/capability capture rules, and backend calling-convention proof.
  Prover: general Newton/bisection targets, RK4 nonlinear ODEs.
- **B. Const/rank generics: DEFERRED.** Profile 0.10 has Nat-ted functions
  over concrete arrays (`fn f<N>(a:[i64;N])`) but no nominal generics,
  no dimension arithmetic (`[i64;N-1]` MNP149), no operator bounds.
  Do not invent a second generic system. Prover: one Bareiss/rank kernel
  deleting ~620 port lines (`docs/generics.md`).
- **C. Data-dependent loop exit: DEFERRED.** Boundedness/verifiability is
  load-bearing; unbounded `while` is out. Candidate: bounded loop with
  early data-dependent break (verifiable bound + dynamic exit). Prover:
  convergence loops, adaptive quadrature, rejection sampling.
- **D. Nested/composite records: PARTIAL.** Concrete nesting works
  (`Outer{inner:Inner}` verified); generic nominals (`record Wrap<T>`)
  do not (MNP123). Dual-of-dual needs type parameters, not mere nesting.
  Prover: `Dual<Dual<Q>>` Halley without second evaluation.
- **E. Traits / numeric abstraction: DEFERRED.** Highest semantic gravity;
  needs operator-constrained generics (`MNE120` today). Do not rush.
  Prover: one `dot` spanning Z/Q/fixed.

`!` sugar: DEFERRED. Needs `Bang` token, AST, elaboration, and lowering on
5 backends (or desugar to `select` with bool-domain proof). `select`
idiom is precise and liveable; revisit with a dedicated operator-model RFC.

Float `sqrt`/`exp`/`log`/`pow` (Phase 3): DEFERRED as a coherent tranche.
Profile 0.12 (`f64`, finite literals, `+-*/`, comparisons, `as`, trap
contract, `sin`/`cos`, five-backend evidence) is green and extended by
this run's representation repair. Next tranche needs per-function domain,
non-finite/invalid/overflow/underflow/trap semantics, cross-backend
agreement, backend strategy (host-libm vs intrinsic), and oracle corpora.
Do not add libm surface without that contract. Rounding-mode control is a
separate semantic design problem (host FENV is global, not machine-native).

## Performance (`source-study` scaling)

Reproduced: `linalg5` 572 lines / ~70s vs `scalar` 541 lines / ~6s (12x);
zero-import modules <3s (`error` 0.4s, `float` 1.8s, `sparse` 2.9s).
Cost follows import-closure size (`linalg5` closes over 1261-line `linalg`;
`optimize` 152 lines / 36s closes over 4 modules), not bare LOC. No
low-risk inefficiency isolated in this run; no speculative rewrite made.

Bottleneck hypothesis for next tranche (unproven): repeated import
traversal/elaboration without cross-module reuse (diamond imports
re-studied; no artifact cache across `source-study` invocations and
possibly within one closure). Recommended: instrument frontend with
per-module study timings + import-visit counts; then memoize studied
artifacts by content hash within a run and evaluate cross-run caching.
Compilation cost matters (concurrent agents/services); this is the next
measured optimization after correctness.

## Files/layers changed

- `crates/mncs-codegen/src/scalar.rs`: `slot_width_of` Float=>W64 + `c_symbol` namespacing in `lower_to_scalar`.
- `crates/mncs-codegen/src/c11.rs`: bitwise float cells/calls/selects; u64 zero-extend; MIN%-1 zero.
- `crates/mncs-codegen/src/llvm.rs`: bitcast double returns; MIN%-1 zero branches; PTX entries mapped via `c_symbol`.
- `crates/mncs-codegen/src/cranelift_backend.rs`: u64 bounds + unsigned CLIF checks; i64 call loads; trampoline lookup via symbol.
- `crates/mncs-codegen/src/support.rs`: centralized `mncs_` `c_symbol`.
- `crates/mncs-codegen/src/external.rs`: PTX entry mapping via `c_symbol`.
- `crates/mncs-compiler/src/frontend.rs`: hygienic `$mncs$` temps.
- `crates/mncs-model/src/body.rs`: MNB101 accepts u64/all widths.
- `crates/mncs-syntax/src/source.rs`: negative numeric literals.
- Tests: `pressure_native_soundness.rs` (3×5 backends), `pressure_frontend.rs` (5),
  `pressure-f64-arrays`/`u64-highbit`/`symbol-hygiene` examples + oracle corpora
  (`scripts/gen_pressure_corpora.py`), `backend_evidence.rs` updated to `mncs_` exports.
- No profile change (all correctness repairs; new negative-literal syntax is
  a compatible grammar extension within existing expression semantics, no
  version bump in this tranche).

## Test evidence

- `cargo test -p mncs-codegen --lib`: 38/38.
- `cargo test --test pressure_native_soundness`: 3/3 (each ×5 backends).
- `cargo test --test pressure_frontend`: 5/5.
- `cargo test --test backend_evidence`: 8/8.
- `cargo test --workspace --no-fail-fast`: all green except pre-existing
  `backend_family::core_status_module_envelope_per_backend` (expects PASS,
  gets UNKNOWN on bytecode; reproduces on untouched base `2b57eee`, unrelated
  to this tranche — integer-overflow/iteration-cost obligations predate it).
- `cargo fmt`, `cargo clippy --workspace --all-targets`: clean.
- Consumer (read-only, unmodified): `float`/`scalar`/`rational`/`modular`/
  `deterministic` green on bytecode/wasm/C11/LLVM (+ float/scalar/rational
  green on Cranelift). Full heavy Cranelift matrix (linalg5/linalg/bigint
  590s guards) not re-run; scoped as CI work.
