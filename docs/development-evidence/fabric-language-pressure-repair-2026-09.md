# Fabric language-pressure repair — 2026-09

Branch: `feat/pressure-native-soundness` (from `f934588`; see completion
report for the final commit). Baseline reconciled: Fabric ledger at
`mncs-fabric` `252cf9d` (language pin `0216d64`).

This campaign reconciled every Fabric pressure (P-001–P-014) against the
current tree, then implemented vertical slices where the language was
genuinely missing capability. Nothing was worked around: Fabric semantics
were not weakened, no Python-side shims were added, and every behavior
below is proven through executable backends, not unit-test passage alone.

## Reconciliation table

| ID | Fabric status | This campaign | Evidence |
|----|---------------|---------------|----------|
| P-001 mixed-width diagnostics | open (awkward) | **RESOLVED** | MNE119 names both widths + suggests `as`; `crates/mncs-cli/tests/fabric_pressure.rs` |
| P-002 ABI mismatch diagnostics | open (awkward) | **RESOLVED** | failure reason carries function, index, expected + received identities; same test file |
| P-003 corpus tooling | open (awkward) | **RESOLVED** | `mncs corpus lint` reusing execution's validator; same test file |
| P-004 imports | resolved | **RESOLVED (confirmed, untouched)** | pre-existing; not re-proven here |
| P-005 text surface | narrowed→P-014 | **RESOLVED via P-014** | remainder was views/folding/feeding; all three proven below |
| P-006 IO effects | blocked | **PARTIALLY_RESOLVED** | read/clock/crypto pre-existing; `host_write` append slice added; network/processes still open |
| P-007 bounded collections | blocked | **PARTIALLY_RESOLVED** | `mncs.std.token_set.v1` + fleet verdicts on 5 backends; full sort/ledger folds still open |
| P-008 services/concurrency | blocked | **OPEN** | untouched by design; P-006 is still the prerequisite |
| P-009 performance | awkward | **OPEN (mitigated)** | no observation-cost work; embedding removes the per-decision subprocess cliff (see P-010) |
| P-010 embedding | pilot-proven | **PARTIALLY_RESOLVED** | `mncs-embed` Rust + C ABI, 0.024 ms/call; Python binding + default flip are consumer-side |
| P-011 backends | partially evidenced | **PARTIALLY_RESOLVED** | all new corpora 5-backend green + weekly matrix workflow; RV32/eBPF/PTX still artifact-only |
| P-012 Option/None | awkward | **OPEN** | no sum/Option work; documented below |
| P-013 caller-judged cases | open | **RESOLVED** | empty `expected` == omitted; same test file |
| P-014 variable-length text | blocked | **RESOLVED** | folding + `equals`/`equals_folded` + 7-alias classifier + host feeding, 5 backends |

## P-001 — mixed-width diagnostics (RESOLVED)

Old: `i32 <= i64` failed with bare MNE119 `binary operands must have the
same type` (plus MNE163 on the projection), naming neither width.
New: MNE119 reports `binary operands must have the same type (left: i32,
right: i64); convert explicitly with `as` (e.g. `left as i64)`.
Semantics unchanged: no implicit promotion; narrowing/widening stay
explicit. General: applies to every binary operator at the shared
MNE119 site (`crates/mncs-compiler/src/frontend.rs`), not a
Fabric-specific comparison. Tests: positive (message names widths and
`as`) and negative (same-width elaborates; the suggested conversion
resolves the refusal) in `fabric_pressure.rs`.

## P-002 — ABI mismatch diagnostics (RESOLVED)

Old: `failure_reason: "argument does not match SSA input type"` with no
identities; Fabric root-caused a `::` typo by dumping artifact bytes.
New: `argument does not match SSA input type: function probe.p002::f
(ssa function mncs:0.4:ssa:function:…), argument index 0, expected R
(record:mncs:0.2:record-type:probe.p002::R::x%3Ai64%3B) but received
record(name="R",
type_identity="mncs:0.2:record-type:probe.p002:R:x%3Ai64%3B")` — the typo
is visible in the message. Covers records, finites, sequences/views,
scalars via one shared `execution_value_summary` helper
(`crates/mncs-model/src/execution.rs`), wired into both SSA
(`ssa_execution.rs`) and body (`execution.rs::validate_arguments`)
paths plus argument-count mismatches. Historical message prefix kept so
existing consumers keep matching. Tests: `fabric_pressure.rs`
reproduces the exact Fabric typo shape.

## P-003 — corpus tooling (RESOLVED)

New toolchain-owned command: `mncs corpus lint
<program.mncs|program.json> <corpus.json>`. Validates every case against
the actual program ABI — target resolution, step budget, and the exact
`validate_arguments` function execution uses (same canonical
type/identity logic, not a parallel implementation) — plus
expected-arity structure and stateful-case `validate()`. Prints a
machine-readable report (`CorpusLintReport`), exits 1 on errors, 0 when
clean. The P-002 typo corpus fails lint pre-execution with the pinpoint
error. Minimum slice only: no separate value-builder DSL; linting is the
supported authoring-time gate. Tests: `fabric_pressure.rs`.

## P-013 — caller-judged execution cases (RESOLVED)

Contract, now documented at the single decision point
(`caller_judged` in `crates/mncs-cli/src/main.rs`): missing `expected`
and explicitly empty `expected: []` both mean caller-judged (no
judgment recorded). Rationale: every MNCS function returns exactly one
value, so `[]` can never match a real return; failing it was an opaque
exit-1 trap for authority-style batches. Applies uniformly to plain
cases, stateful final expectations, and per-step expectations across
`experiment run` and `experiment execute` (shared observation
builders). A genuinely wrong expectation still fails. If a future
zero-return function shape ever exists, this contract must be revisited.
Tests: `fabric_pressure.rs` (both spellings pass; wrong expectation
fails).

## P-014 — bounded text (RESOLVED)

The Fabric claim (variable-length strings blocked) no longer holds, and
this campaign closed the confirmed remainder rather than rebuilding the
substrate:

- Pre-existing and reused: `[byte; up_to 64]` views crossing executable
  ABIs, `text_scan` literal scans, `host_read` bounded byte feeding.
- Added small total primitives: `to_ascii_lower`/`to_ascii_upper`,
  `is_ascii_upper`/`is_ascii_lower`, `eq_ascii_fold`
  (`library/core/bytes.mncs`); generic `equals<N>`/`equals_folded<N>`
  over explicit lengths plus `candidate_equals(_folded)`
  (`library/std/text_scan.mncs`). No heap String; folding is the 26
  ASCII pairs only, locale-free by construction.
- Language-owned reproducer `examples/source/arch/classify.mncs`
  (pure decision module; host-fed composition in `arch/host.mncs`, which
  imports it — backend lowering is whole-program, so the effectful
  wrapper lives separately and the classifier stays portable):
  `classify_arch(word: [byte; up_to 64], word_length: u64)` answers all
  seven Fabric aliases (`x86_64`, `amd64`, `x86`, `i386`, `aarch64`,
  `arm64`, `riscv64`) case-insensitively through one entrypoint taking
  differing runtime lengths (family codes 1/2/3/4, 0 unknown).
- Host feeding composed: `classify_blob()` reads bounded bytes through
  an explicit `host_read` grant and classifies by runtime length;
  mixed-case `AArch64` from a granted file → family 2 with the effect
  recorded (`examples/execution/arch-classify-host-corpus.json`).
- Backend evidence: `arch-classify-corpus.json` (35 cases: every alias ×
  lower/upper/mixed + 14 unknowns) 35/35 on all five executable
  backends; extended `text-scan-corpus.json` 32/32 on all five.
  Tests: `crates/mncs-cli/tests/arch_classify.rs`.

Remaining limitation (honest, narrow): delimiter scanning over unbounded
input stays out of envelope by design — hosts bound inputs at grants.
Nothing in the Fabric classifier list needs more than this.

## P-006 — capability-scoped effects (PARTIALLY_RESOLVED)

Current-tree mapping: `host_read` (bounded grant bytes), `clock_read`
(wall-clock instants), `sha256_digest`/`ed25519_verify` (verify-only
crypto) all pre-existed with explicit grants, effect records, and
fail-closed execution. This campaign added the natural next slice:

- `host_write(view) -> u64` (Profile 0.12 intrinsic, operation
  `blob_append`, effect kind `host_write`): appends exactly the view's
  runtime bytes (≤64 per call) to the `--grant-write capability=path`
  destination, returns the appended count, records the realized effect
  with the appended bytes' SHA-256. Append-only: create-if-absent, no
  read-back, no truncation, no ambient path. New diagnostics
  MNE253/MNE254/MNE255 + MNP202 mirror the read-side authority gaps.
- Realized on the model/body and SSA paths (research backend executes);
  portable-WASM/C11/LLVM/Cranelift refuse explicitly at lowering via the
  pre-existing `HostCall` refusal arm — no new refusal code was needed.
- Provenance: `examples/source/host-write-append.mncs`,
  `host-write-append-corpus.json`, `crates/mncs-cli/tests/host_write.rs`
  (granted byte-exact order, fail-closed without grant, explicit
  cross-backend refusal). RFC 0008 ledger entry updated (stays PARTIAL;
  0008-C3 general I/O remains unsatisfied).
- Known limitation (new pressure N-001, below): `experiment run`
  replays realized writes once per validation layer.

Still open: transport/network, process execution, ledger persistence
(read-back), cumulative storage bounds (per-call 64 B only).

## P-007 — bounded collections (PARTIALLY_RESOLVED)

Solved as a stdlib abstraction — no new syntax. `library/std/token_set.mncs`
(`mncs.std.token_set.v1`): storage + logical count + explicit
overflow verdicts; `contains`, `first_free_or_duplicate` (insertion
decision without mutation; callers apply `replace`), `require_all`,
`forbid_any`, `prefer_hits`, `dedup_count`, and `top_index`
(maximum-score index, ties to lowest index, -1 when empty). Tokens are
host-assigned byte codes, mirroring the arch family-code pattern.
`examples/source/fleet-tokens.mncs` computes the Fabric-shaped verdict
(required/forbidden/preferred tokens, candidate worker tokens,
eligibility vs preference separation, best-worker ranking). Backend
evidence: `token-set-corpus.json` (23 cases) and
`fleet-tokens-corpus.json` (13 cases) green on all five backends;
layered agreement holds. Tests: `crates/mncs-cli/tests/token_sets.rs`.
Still open: full deterministic sort (comparator exists via
`text_scan.compare`; the sort itself waits), ledger folding over event
lists.

## P-010 — embedding (PARTIALLY_RESOLVED)

`crates/mncs-codegen::OwnedExecutionSession`: an owned reusable session
(decoded research payload + block indexes, decoded WASM module; one-shot
elsewhere, mirroring the borrowing session). `crates/mncs-embed`: minimal
host-callable surface — `Artifact::from_json` (identity-validated open;
tamper refused) / `from_source`, `Session::open/call/call_json` with
canonical ABI values, explicit per-call grants, structured
`CallOutput` carrying the executed artifact identity + digest + backend
on every call, plus a stable C ABI (`mncs_session_open/info/call`,
`mncs_response_text/free`, `mncs_last_error`) with no Rust layout for
Python/ctypes consumers. No Python package added to Fabric, per scope.

Evidence (`crates/mncs-embed/tests/embedding.rs`, 5 tests): repeated
calls agree verdict-for-verdict with digests attached; tampered bytes
refused at open; unknown entrypoint yields structured `invalid_request`.
Latency on an already-loaded artifact (debug build, 200 calls):
**mean 0.024 ms, p50 0.025 ms, p99 0.032 ms** — against the pilot's
~280 ms subprocess batch. Millisecond-scale authority is now a
measurement, not a claim. Remaining for resolution: a Python-side
binding in the consumer and the production default flip (Fabric-side).

## P-011 — backend evidence (PARTIALLY_RESOLVED)

Every new corpus in this campaign ran green on all five executable
backends (research-bytecode, portable-wasm-mvp, c11, llvm-ir,
cranelift): arch-classify 35/35, text-scan 32/32, token-set 23/23,
fleet-tokens 13/13. Host effects execute on research with explicit
lowering refusal elsewhere (proven for `host_write`; same arm covers
all host calls). Added `.github/workflows/backend-matrix-weekly.yml`:
weekly scheduled + dispatchable matrix over the campaign corpora plus
the machine-readable `experiment matrix` envelope. Still
artifact-only: RISC-V/eBPF/PTX (no runtime-execution claims made).

## P-008 / P-009 / P-012 (OPEN, mostly untouched)

- P-008: no service/timer work; P-006 remains the prerequisite. The
  append slice plus `mncs.std.task.v1`/`mncs.std.clock.v1` are the
  substrate a future tick/lease design builds on.
- P-009: no observation-cost work. Mitigating facts: artifact/session
  reuse already exists (`BodyExecutionSession`, `BackendExecutionSession`,
  `OwnedExecutionSession`); embedding removes the subprocess cliff.
- P-012: no Option/sum work. The three absence policies stay host-side
  folds by design until payload-bearing sums land; a magic nullable was
  correctly not bolted on.

## New pressure uncovered

- **N-001 (validation replay duplicates realized writes).**
  `experiment run` replays each case through body, SSA, and backend
  layers with the same grants, so one corpus case appends 3× (proven:
  ledger held `ab×3 cd×3 efgh×2`). Pure reads/hashes are unaffected.
  Frozen `experiment execute` runs each case exactly once (proven:
  ledger held exactly `abcdefgh`). Production embedding calls execute
  once per call. Options: skip layered validation for effect-realizing
  corpora, or record-and-dedupe realized writes per layer. Left open
  deliberately — changing validation replay needs its own RFC-level
  decision.

## Test and conformance notes

- New suites: `fabric_pressure` (5), `arch_classify` (3),
  `token_sets` (3), `host_write` (4), `mncs-embed/tests/embedding` (5).
- Extended: `text-scan-corpus.json` 21→32 cases.
- Existing suites were re-run (see completion report); no existing test
  was weakened. Overall experiment status UNKNOWN on new corpora comes
  only from honest checked-arithmetic obligations, matching the
  repository's existing convention.
