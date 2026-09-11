# Store-pressure Tranche A: durable filesystem mutation (2026-09)

Forcing workload: `mncs-store` pressure index P1-001 (no file-write
effect), P1-002 (no durable-sync barrier), P1-003 (no atomic rename /
dir ops), P2-005 (no atomic publication). Prior state: Profile 0.12
landed the observation half (`fs_list`, `fs_read`); every mutation
lived in the host driver. This tranche lands the mutation half as
Source Profile 0.16 (`docs/source-profile-0.16.md`).

## 1. Pressure reconciliation

| pressure | required | landed | evidence |
|---|---|---|---|
| P1-001 file-write effect | bounded, capability-authorized mutation with explicit errors | `fs_create_file`, `fs_write_bytes_at`, `fs_append_bytes_at` under `effect fs_write` + `--grant-fs` | `pressure_fs_mutation.rs` (9-case corpus, byte-exact read-back) |
| P1-002 sync barrier | explicit capability-gated barrier, failure as value/fault | `fs_sync_at`: file + parent-dir + root edges, `RuntimeFailure` on fault, POSIX-only dir edge recorded in provenance | corpus `seal-probe` + unit `staged_lifecycle_lands_byte_exact` |
| P1-003 namespace ops | mkdir, readdir (had), rename, stat-ish kinds (had), exclusive create | `fs_mkdir`, `fs_rename_at` (same-dir, atomic file replace), `fs_delete_at` (files + empty dirs), exclusive `create_new`/`create_dir` | corpus `open-temp`, `publish-probe`, `retire-*`; unit `rename_replace_*`, `refusal_matrix_*` |
| P2-005 atomic publication | stage-then-rename, sync file+dir, exclusive create, deletes | composes from the above; same-dir rename returns the new (observed, never predicted) index | `threaded_lifecycle_agrees_across_layers` threads the rename index host-side |
| P1-014 nominal boundary | stable host-constructible identity | ABI-name alias (`name`/`variant_names`) pinned to the expected declaration | `support::contract_tests` (7 tests) + `MNE261` family unchanged |
| P2-004 append-only limits | staging beyond single-file append | positioned + append writes against the granted tree (multi-chunk staging composes create + appends) | `grow-probe` / `patch-probe` cases |

Explicitly NOT claimed: path type (P1-017 — names stay bare
single-component views), identity issuance (P1-015 — needs persistent
state), generic boundary invocation (P1-013 — needs a request-schema
type-argument channel), typed error sums (P1-020 full — failures stay
`InvalidRequest`/`RuntimeFailure`; the `name`/`variant_names`
aliasing is the nominal half only), cross-directory moves, recursive
delete, quotas, snapshot isolation.

## 2. Design record (decisions with reasons)

- **One new effect, no new grant.** `fs_write` reuses `--grant-fs`
  (`Grant::fs_root` in `mncs-embed`, zero code change). Authority
  stays "which tree", never "which path".
- **Indices are data.** Mutations address listing indices resolved
  against a fresh snapshot per call, exactly like the read half;
  stale/wild indices are `InvalidRequest` with a re-list hint.
- **Post-state is observed.** Create/mkdir/rename return indices read
  from a post-mutation re-snapshot; a vanished root there is
  `RuntimeFailure`, never a fabricated index. Intent-only (`Record`
  policy) computes the exact rank from pre-state instead.
- **No implicit fsync.** Durability is the separate `fs_sync_at`
  barrier (file + containing dir + root). A crash between mutation
  and barrier is observable drift, never a false durability claim.
- **Trap-on-error, not error sums.** Failures are realization-time
  `InvalidRequest`/`RuntimeFailure`, matching the read half; typed
  sums await Tranche C. The elaborator documents this at
  `emit_fs_mutation`.
- **Validator-compatible corpora.** The translation validator replays
  each case independently under observed policy on the pristine tree,
  and any non-returning case is an automatic mismatch — so experiment
  corpora must be fixture-anchored (every case returns alone on
  pristine fixtures AND sequentially). Sequential-index lifecycles
  are proven instead by twin-root direct replay on both layers.
- **Compiled backends refuse by construction.** No backend code was
  touched: the pre-existing scalar-envelope `HostCall` refusal arm
  covers all seven operations (whole-program refusal with CGC301/302,
  destination untouched).

## 3. Verification (commands run, exact results)

- `cargo test -p mncs-model --lib fs_resource`: 5/5 pass
  (`staged_lifecycle_lands_byte_exact`,
  `intent_only_validates_without_touching_the_tree`,
  `refusal_matrix_leaves_no_trace`, `symlinks_are_never_touched`,
  `rename_replace_covers_files_never_directories`).
- `cargo test -p mncs-cli --test pressure_fs_mutation`: 6/6 pass
  (fixture-anchored 9-case lifecycle with overall status PASS,
  twin-layer corpus agreement, host-threaded lifecycle agreement,
  refusal matrix, ungranted fail-closed, 4-backend whole-program
  refusal).
- `cargo test -p mncs-cli --test profile_compat`: 26/26 pass
  (incl. new `older_profiles_refuse_fs_mutations` MNP211 and
  `profile_016_admits_fs_mutations`).
- `cargo test -p mncs-codegen --lib contract_tests`: 7/7 pass
  (P1-014/P1-020 name-alias pins).
- `cargo test -p mncs-cli --test profile_registry`: registry
  snapshot, linearity-to-0.16, and doc-existence pins pass after
  regenerating `spec/source-profile-registry.json` from
  `print_registry_snapshot`.
- Manual experiment runs: bytecode lifecycle `exit=0`, status PASS,
  designed end-state `[aa-stage, w-final]`; C11 whole-program refusal
  carries CGC301/302 + the scalar-envelope host-call message,
  destination untouched.
- `cargo fmt --all` applied; `cargo clippy -p mncs-model --lib`
  reports zero warnings (one over-broad lint autofix that collapsed
  interpolated `format!` literals was caught by test + review and
  repaired with byte-level verification against HEAD: pre-existing
  regions identical, diff purely additive).
- Full workspace suite: see §5.

## 4. Cross-backend parity

| backend | mutation corpus | expectation |
|---|---|---|
| `mncs-research-bytecode` | 9/9 returned, expectations met, status PASS | realizes with grant |
| `mncs-portable-wasm-mvp`, `mncs-c11`, `mncs-llvm-ir`, `mncs-cranelift` | whole-program refusal, explicit host-call diagnostics, destination untouched | `HostCall` refusal arm (no per-op code) |
| body vs SSA executors | twin-root replays agree case-by-case incl. byte-exact read-back | shared `fs_mutate` + mirrored arms |

## 5. Full-suite results

`cargo test --workspace --no-fail-fast`: **710 passed, 1 failed** —
the single failure was `roadmap_05_..._evidence_gated`
(`semantic_commands`), a stale whole-program-refusal pin left behind
by the earlier session's saturating-arithmetic lowering change (LLVM
now correctly executes the envelope's saturating entrypoint and
refuses only the widening entrypoint per-entrypoint). Re-pinned to
the admission-report shape with strictly stronger assertions
(exports, refusal reason, per-case outcomes); fresh target run
38/38 green. One additional transient: `pressure_record_field_identity`
failed once under full-workspace parallel load ("native program
produced no JSON observation" from the C11 runner) and passes 5/5
isolated plus in the no-fail-fast run — the failure reason
exonerates the contract-checking change (a contract refusal would
carry the `MNCS_VALUE_CONTRACT` diagnostic, and the C11 program
produced no output at all). `cargo fmt --all` applied; `cargo clippy` reports zero
warnings on new code (two pre-existing warnings in `codegen/lib.rs`
and `proof_demo.rs` left untouched).
