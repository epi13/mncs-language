# Development evidence — store-pressure follow-up 2026-09-12

Branch: `feat/store-pressure-followup-2026-09-12` from `origin/main@2ad7707`.
Mission: post-merge audit + next implementation campaign after the
2026-09-11 Profile 0.15/0.16 store-pressure run.

## Phase 0 — branch repair (done)

Observed before: checkout `feat/proof-transport-exhaustion-hardening`
at `091a184`; `git log origin/main..HEAD` empty (0 unique commits);
`merge-base --is-ancestor HEAD origin/main` true (merged via `85aed1e`);
tree clean; local `main` at `9ce205f` (4 behind `origin/main@2ad7707`,
held by a stale `/tmp/lang-main` worktree from a prior session).

Repaired without touching shared history: fetched origin, fast-forwarded
`main` to `2ad7707` inside the holding worktree (it was clean), created
`feat/store-pressure-followup-2026-09-12` from `origin/main`, deleted the
local stale branch (`git branch -d`, merged-status proven). The remote
`origin/feat/proof-transport-exhaustion-hardening` was left intact.
No force-reset, no force-push.

## Baseline findings (current main, verified not inherited)

- Focused suites around the merge all green: model `fs_resource`
  (5 passed at the time), `pressure_fs_mutation` (6), `profile_compat`
  (4), `profile_registry` (44), `contract` (4), effect partition (3),
  generation scan (4), view widen (2), checked-arith-u32 (3).
- Warning debt: the reported `BackendPromise` unused import in
  `proof_demo.rs` reproduces, plus two needless-borrow and one
  unneeded-pattern clippy lints. Fixed as Tranche 0 (commit `68492c0`,
  semantics-neutral, clippy warning-free afterwards).
- `cargo fmt --check` under rustfmt 1.9.0 flags TWO PRE-EXISTING
  drifts (`semantic_commands.rs`, `matrix.rs`) untouched by any recent
  commit. Left in place to keep tranches minimal;toolchain-version
  drift, not new debt.

## Pressure re-baselining (every item reproduced or re-read on main)

- P1-013 (host cannot call generics): STILL REPRODUCED. Direct corpus
  targeting `grow_fill_local` fails `invalid_request / execution
  target SSA function does not exist` on current main. → Tranche 1.
- P2-003 (no generic boundary functions): same root cause as P1-013,
  closed by the same channel (no wrapper functions generated).
- P1-016 (checked-u32 C11): RESOLVED on main. All four checked-arith
  suites pass; the one transient C11-runner failure cited previously
  did not reproduce across the reruns in this campaign (focused suites
  plus full workspace run); no change made, no flakiness observed.
- P2-021 (generation scan): RESOLVED on main (4/4 pass).
- P1-020 (untyped errors): still open as designed — see deferral.
- P1-017 (path/name): still open as designed — see deferral.
- P1-015 (durable identity): still open as designed — see deferral.
- Profile 0.16 TOCTOU limitation: reproduced by reading
  `fs_resource.rs` (final-component swap window between
  re-canonicalization and open on read/write/append/sync, plus the
  `create_new`-follows-dangling-links escape). → Tranche 2.

## Tranche 1 — P1-013/P2-003 host-called generics (CLOSED)

No new source syntax, no new profile: a boundary/schema evolution with
explicit canonical forms, additive and backward compatible.

Request channel (`crates/mncs-model/src/execution.rs`):

- `ExecutionTypeArgument`: `{"kind": "nat", "value": 8}` /
  `{"kind": "type", "type": "i64"}`. Type spellings are canonical
  semantic names resolved by `BodyType::from_program`, exactly like an
  in-language explicit `<...>` argument.
- `ExecutionRequest.type_arguments` (serde default + skip-if-empty):
  additive like `host_grants`/`call_depth_budget` before it; request
  schema stays `0.1`; every existing corpus parses unchanged (full
  suite proves it).
- `StatefulExecutionStep.type_arguments`, carried into the step's
  backend request; nested (callee) requests pass empty and fail closed
  on generic templates.
- `resolve_generic_entry` + `parse_host_generic_args` +
  `generic_entry_failure_reason`: one resolution shared by the body
  executor, the SSA executor, and corpus lint. A bare generic target
  now refuses uniformly (`requires explicit type_arguments`) on every
  layer instead of the previous per-layer accidents; execution NEVER
  invents an instantiation.

Compile-time seeding (`generics.rs`, `core.rs`, `frontend.rs`):

- `specialize_program_with_seeds` (existing `specialize_program`
  delegates with `&[]`): host seeds join the SAME fixed-point queue
  ahead of the call-site scan, merged by instantiation key with
  spellings unioned. One function, one record, one identity however
  the instantiation was requested. Seeds consume the shared
  `MAX_SPECIALIZATIONS` budget and report the shared diagnostics.
- `GenericSpecializationRecord.host_spellings` (skip-if-empty: no
  fingerprint churn for existing records).
- `specialization_entry_name` centralizes the deterministic
  `{base}__spec_{hash8}` naming as a stability contract.
- `resolve_host_seeds` parses seeds with the in-language taxonomy
  (MNE131 unknown, MNE221 arity, MNE222 kind, MNE105 unknown type) and
  checks `Nat` values against the DEFINING module's admitted ceiling
  (MNE225 — there is no call-site module on the boundary); the
  post-specialization sweep re-checks every substituted traversal
  bound (MNE182) including seeded records.
- `specialize_program_with_host_seeds` (also used for semantic-JSON
  programs, where pre-existing records keep their admissions and only
  new seeds face the envelope-ceiling sweep).

Artifact map (`compiler.rs`, codegen `support.rs` + all backends):

- `BackendArtifact.generic_entrypoints` (schema `0.3` → `0.4`,
  identity-joined, serde-defaulted for old JSON). One row per
  host-seeded instantiation: (module, generic, normalized spellings)
  → concrete entry. Populated by all six constructors through one
  shared builder.
- `resolve_request_entry`: artifact-only resolution for WASM/C11/LLVM/
  Cranelift/research. Empty args pass through untouched; misses fail
  closed (unknown spellings list the compiled ones; pre-0.4 artifacts
  name the recompile). Resolved names flow into the existing
  contract/export lookups and session cache keys, so instantiations
  get distinct cache slots with zero cache-logic changes.

Command surface (`mncs-cli`):

- `experiment run`/`plan` read the corpus first and seed elaboration
  (`front_end_with_resolver_and_seeds`).
- `execute`/`execute-ssa` seed from the single request (direct host
  invocation, no corpus needed).
- `compare-execution`, `check-lowering-execution`, `corpus lint` seed
  from their corpus.
- `compile --corpus <path>` seeds frozen-artifact emission, so frozen
  `experiment execute` serves the same entrypoints.
- `mncs abi` reports `generic_params` (declaration order),
  `requires_type_arguments`, and `compiled_instantiations`
  (canonical args, entry, spellings) per function.

Evidence (new unless noted):

- `examples/source/pressure-host-generics.mncs`,
  `examples/source/pressure/host_generics/lib.mncs`,
  `examples/execution/pressure-host-generics-corpus.json` (10 cases:
  Nat ×2 instantiations, type, view-bound, cross-module ×2, nominal
  record, plus 3 in-language identity anchors).
- `crates/mncs-cli/tests/host_generic_entrypoints.rs` (10 tests):
  5-backend PASS with validation PASS; one-row-per-instantiation
  artifact map; frozen artifacts on 4 backends; ABI discoverability;
  bare-target refusal on both reference executors; 8-case MNE seed
  table; malformed-shape corpus refusal; map-stripping tamper
  evidence; repeat-call agreement without cross-talk.
- Model unit tests: seed collection incl. stateful steps, step→request
  carrying. Codegen unit tests: entry resolution incl. legacy-schema
  recompile diagnostic and spelling normalization.
- Cross-backend result: 10/10 cases PASS on all five backends, each
  with `backend-lowering-bounded-agreement` PASS (body/SSA/backend
  triple agreement on generic entrypoints).

Identity proof: `grow_fill_local<8>` requested both by host seed and
by `probe_same_fill` yields ONE artifact row with spellings `["8"]`;
host-only instantiations (`vlen`, `id_value`, lib `first`) yield rows
because seeds compile them in.

Deliberately NOT done in this tranche: inferred (omitted) type
arguments (the host always spells every parameter — inference stays a
source concept); `Nat` spellings beyond decimal `u32`; seeding
frozen `compare-execution` baselines across schema versions (same-
version only, by construction).

Follow-up (numerics-pressure campaign, 2026-09-12): the "spellings
unioned" merge above treated the positional spelling list as a set
(`sort`+`dedup` across positions), so `(2, 2)` collapsed to `(2)`
and `(3, 2)` reordered to `(2, 3)` on every native backend
(mncs-numerics P-002). Repaired on branch
`feat/numerics-pressure-2026-09-12`: spelling addresses stay
positional whole vectors (ordered set per instantiation,
first-seen); `GenericSpecializationRecord.host_spellings` is now
`Vec<Vec<String>>`, and the artifact map plus `mncs abi` emit one
row per address sharing one specialization entry. Pinned by
`pick2_sum<2, 2>`/`<3, 2>` plus a nominal short-name/identity union
pair in `examples/source/pressure-host-generics.mncs` (14 cases,
all five backends) with artifact-row assertions in
`crates/mncs-cli/tests/host_generic_entrypoints.rs`.

## Tranche 2 — filesystem TOCTOU (PARTIAL CLOSE + explicit remainder)

Verdict: the final-component swap closes cleanly on Unix without
lying about portability; the remainder stays explicitly non-claimed.

Mechanism (`crates/mncs-model/src/fs_resource.rs`, +`libc` unix dep):

- `open_nofollow` (`O_NOFOLLOW` on Unix, plain open elsewhere) gates
  the read, positioned-write, append, create-content, and barrier
  opens. A post-validation symlink swap fails the open atomically;
  all subsequent IO uses the descriptor, immune to later swaps.
  Refusals map to explicit `InvalidRequest` (`became a symlink
  between validation and open`).
- `verify_inside_root`: creates re-resolve under the canonical root
  after the syscall, catching the `create_new`-follows-dangling-links
  escape deterministically. Escape fails closed as `RuntimeFailure`
  with the file deliberately left in place (unlinking a re-swapped
  path could delete an unverified file; created content is bounded
  program bytes under `O_EXCL`, never an overwrite).
- Documented-by-construction safety (comments + tests, no code
  change): `mkdir(2)` refuses links with `EEXIST` (incl. dangling);
  unlink-family never follows a final-component link; `rename(2)`
  replaces destination links.
- Explicit remainder (profile doc note): intermediate-component
  swaps on nested paths, final-component swaps on non-Unix builds,
  directory-handle stability across the snapshot walk, hostile
  co-writers inside the granted root.

Evidence: 5 new model tests (`outside_pointing_symlinks_*`,
`dangling_symlinks_*`, `post_create_verification_*`,
`gated_open_*`, `final_component_swap_stress_*` — the stress test
asserts exact properties: outside sentinel byte-identical,
convergence to exact content, no timing assumption). Pre-placed-link
refusals were previously claimed but untested at this layer; the
existing `symlinks_are_never_touched` (delete/rename/inside-link)
still passes. Profile 0.16 keeps its seal: one dated additive
hardening note, no semantic rewrite.

## Deferred with architectural reason (NOT stale, NOT forgotten)

- P1-020 typed errors: the store needs ≥8 distinguishable failure
  classes while the implementation has `FsFail::{InvalidRequest,
  RuntimeFailure}`. A real fix routes typed failures through the
  finite/sum machinery end-to-end (source type → specialization →
  execution values → boundary → backends), i.e. a second
  flagship-sized boundary change of the same magnitude as Tranche 1.
  Doing both in one run risks two half-finished experiments; the
  mission names either/or as a good run. Faking it with string
  wrapping was explicitly rejected. Next run owns it; Tranche 1's
  additive-field and version-bump patterns are the template.
- P1-017 path/name: Profile 0.16 deliberately scoped to bare
  components; a true bounded root-confined path abstraction needs its
  own tranche (leased views / path capabilities). No half-step taken.
- P1-015 durable identity: purity cannot mint freshness; the honest
  design is a capability-gated effectful issuance (uniqueness scope,
  authority, concurrency, crash/durability ordering specified), not a
  wall-clock hack. Needs its own tranche.
- Proof/JIT adjacents (`proof_bindings` execution revalidation,
  `assumption_bits` linkage, JIT publication, host-stack behavior,
  indexed projection chains): read but untouched — coherent with
  neither tranche; exploding the run would violate scope discipline.

## Conformance ledger

No update: 0013-C3 (interfaces/higher-kinds) is not host-called
generics; 0015-C2 (ABI inspection) stays `partial` (generics are now
advertised, but typed errors and the remaining boundary gaps keep it
partial — upgrading it on half the story would be the vague
"mostly supported" this repo forbids).

## Compatibility

- All existing corpora parse and pass unchanged (additive request
  field, additive record/ABI fields, skip-if-empty everywhere).
- Backend artifact `0.3 → 0.4`: old JSON parses (serde defaults) but
  old identities fail closed at the identity gate first
  (`stale or laundered`), same precedent as the `0.2 → 0.3` bump;
  genuine pre-0.4 artifacts additionally get the explicit recompile
  diagnostic on generic requests.
- No source profile change (0.16 current); no syntax change; ceiling
  semantics unchanged; specialization identity scheme unchanged
  (same hash, factored into one function).

## Costs observed

- `mncs-compiler` lib suite: ~337 s in this environment (proof
  admission SoS revalidation dominates); unchanged by this run.
- Host-generic seeding adds one corpus shape-pass plus at most one
  specialization per distinct (declaration, arguments); native
  retained sessions key per resolved entry (no extra recompiles for
  repeated calls — pinned by the repeat-agreement test).
- New dependency: `libc 0.2` (unix-only use, already in lockfile).

## Session closure 2026-09-12 (post-draft verification)

- Embed refusal wording (Tranche 1 remainder): the seeded
  `from_source_with_seeds` path always worked — the seeded call
  returned `returned` on the first reproduction. The failure was the
  UNSEEDED leg of
  `seeded_artifacts_serve_generic_entrypoints_in_process`: the
  research-session path (program-record resolution via the shared
  `generic_entry_failure_reason`) said `no compiled specialization
  ... no instantiation ... was compiled`, while the artifact-map path
  (`support.rs`) and the test contract say `no compiled generic
  instantiation`. Fix: empty-`available` branch of
  `generic_entry_failure_reason` (`crates/mncs-model/src/execution.rs`)
  now reads `no compiled generic instantiation of {function:?} for
  arguments ({canonical_args}) matches this program: name the
  instantiation in the corpus so elaboration compiles it in` —
  unifying program-record and artifact-map refusals per branch. No
  test asserted the old wording; `mncs-embed/tests/embedding.rs`
  6/6 green.
- TOCTOU adversarial verification: `fs_resource` unit suite 10/10
  green, including all five adversarial tests
  (`symlinks_are_never_touched`,
  `outside_pointing_symlinks_refuse_on_every_indexed_entrypoint`,
  `dangling_symlinks_refuse_create_and_mkdir`,
  `gated_open_passes_files_and_refuses_links`,
  `final_component_swap_stress_never_escapes`). The stress test runs
  a live 400-iteration flipper thread against positioned writes with
  exact sentinel/convergence assertions (no timing assumption).
- Gates: `cargo fmt --check` clean for all campaign-owned files
  (`embedding.rs`, `fs_resource.rs` reformatted; the two remaining
  drifts are pre-existing in untouched files, toolchain-version
  drift per above); `cargo clippy --workspace --all-targets --
  -D warnings` clean; `cargo test --workspace --no-fail-fast`
  FULLY GREEN: 105/105 suites `ok`, 735 tests passed, 0 failed,
  0 ignored, cargo exit 0 (file-logged run
  `/tmp/mncs_ws_test.log`, `CARGO_EXIT:0`; an earlier piped run was
  discarded after `head -n 60` was found to truncate 98
  executables' results — the file-logged re-run is the verdict of
  record). Cross-backend parity: `host_generic_entrypoints`
  (10 tests: 10/10 cases PASS on all five backends with translation
  validation PASS), frozen-artifact `experiment execute` on four
  native backends, and the `embedding` suite (6/6, incl. seeded
  generics in-process) all inside the green workspace run.
