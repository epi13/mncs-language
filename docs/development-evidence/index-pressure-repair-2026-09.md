# Index pressure-repair campaign (2026-09): reconciliation and vertical slices

Run scope: `mncs-language` is the write repository; `mncs-index` is a
read-only consumer, reproducer source, and acceptance oracle. No
`mncs-index` file was modified. Branch
`feat/pressure-native-soundness`, on top of the pressure tranche at
`081ab12`.

## 1. Reconciliation matrix (PRESS-001..019 vs current main)

`old` = registry text at `mncs-index` slice-7 (2026-09-08, validated
against an older language pin). `now` = reproduced against this
run's base before implementing. Evidence is observed output, never
status text.

| PRESS | old claim | now (observed) | root cause | owning layer | action | acceptance evidence |
|---|---|---|---|---|---|---|
| 001 tasks | no spawn/join; RFC 0010 NONE | STILL LIVE (confirmed: no task syntax; `task.v1` vocabulary-only) | missing semantics | language | Slice A+B: stdlib scope contracts + embed TaskScope | `task_scope.rs` (CLI), `task_scope.rs` (embed) |
| 002 queues | no channels | STILL LIVE | missing semantics | language | Slice B: stdlib channel contracts | `channel_contract.rs` |
| 003 fs traversal | `host_read` only | PARTIAL (host_read green; no traversal) | missing effects | language | Slice C: `fs_*` intrinsics | `fs_effects.rs` (11 tests) |
| 004 int bitwise | RESOLVED | RESOLVED (xor 12^10=6 observed) | — | — | none (verify only) | reproducer run |
| 005 traversal hist | MNB101 on u64 + unbounded gaps | PARTIALLY RESOLVED: u64 domain fixed (reproducer elaborates, CMP301 only); unbounded strings/sort/split still absent | traversal domains + bounded substrate | language | u64 verified; sort/relation/chunk stdlib narrow the unbounded gap | `pressure_u64_domain.rs`, `collections.rs` |
| 006 digest | grant-gated ≤64B experiment path | STILL LIVE (sha/abc + clock probes green but unchanged shape) | missing pure primitive | language | Slice E: pure stdlib SHA-256 | `sha256_pure.rs` vs hashlib |
| 007 watch/time | relational clock only | PARTIAL (clock green) | missing event half | language | generation-hint slice | `fs_generation_moves_on_mutation` |
| 008 typed failure | no aggregation | STILL LIVE | missing semantics | language | failure taxonomy in scope contracts + embed aggregation | failure tests both layers |
| 009 nested iterate | 2-level resolved, 3rd refused | RESOLVED (bounded; 3rd-level refusal is the documented bound) | — | — | none (verify only) | reproducer + `relation.mncs` uses 2-level shapes |
| 010 embed sessions | subprocess cliff | LANGUAGE-RESOLVED (0.024 ms/call pilot green; reusable sessions + C ABI exist) | consumer adoption | consumer | Slice A: batch/concurrent scope API + index-shaped proof | embed `task_scope.rs` benchmark |
| 011 MNB101 span | whole-file span on u64 reproducer | SUPERSEDED: reproducer now elaborates (no MNB101 at all); enum domains elaborate too | fixed traversal domains | — | none (no manufactured work) | source-study codes `[CMP301]` |
| 012 cancellation | none | STILL LIVE | missing semantics | language | cooperative cancel in scope contracts + embed | cancel tests both layers |
| 013 CAS | RFC 0026 NONE | STILL LIVE | missing semantics | language | Slice F: store CAS contracts | `store_contracts.rs` |
| 014 text plumbing | token windows only | STILL LIVE (text_scan/token_set green, unchanged) | missing chunked/types substrate | language | chunk cursors + sort/dedup stdlib | `collections.rs`, `sha256_pure.rs` |
| 015 relations | no tables | STILL LIVE (`graph.mncs` is a host workaround per registry) | missing values | language | bounded relation stdlib | `collections.rs` (5 closure shapes) |
| 016 durable commit | RFC 0026 NONE | STILL LIVE | missing semantics | language | Slice F: receipts/levels | `store_contracts.rs` |
| 017 snapshots | RFC 0026 NONE | STILL LIVE | missing semantics | language | Slice F: handle contracts | snapshot isolation case |
| 018 compaction | RFC 0026 NONE | STILL LIVE | missing semantics | language | Slice F: retention-as-data | reclaim cases |
| 019 distribution | BLOCKED (gated) | BLOCKED (prereqs 2,3 unmet at slice-7; this run advances them) | gating | — | none (no fake distribution) | gate re-evaluation below |

Gate re-evaluation for PRESS-019: prerequisite 1 (economical
invocation) now holds (0.018 ms warm mean, batch C ABI); prerequisites
2–3 hold as bounded substrate (contracts + embed realization, not a
full model). Distribution remains gated: no transport, wire contract,
or retry semantics were added, and no "distributed" labels were put
around the local executor.

## 2. Implemented capabilities (semantics, not files)

**Concurrency (RFC 0010 → SUBSTRATE).** Source semantics and runtime
mechanism are separated on purpose. `mncs.std.scope.v1` owns
spawn/join ownership (ids are slot indices, so id order is insertion
order), deterministic collection (folds walk slots, never completion
order), cooperative cancellation (pending→cancelled with per-task
cleanup counts; success after cancel is rejected by construction),
failure as inspectable data (codes survive sibling cancellation), and
close-rejects-escape. `mncs.std.channel.v1` owns the bounded-FIFO
contract: capacity, backpressure as `blocked` data (never a block),
`producer_done` accounting, close accepted exactly once and only when
all producers are done, drained-close observed as `kind == 2` with no
sentinel convention. `mncs-embed::TaskScope` is the runtime
realization: one verified artifact, worker threads bounded by scope
capacity, one exclusive session per worker from the same verified
bytes, `std::thread::scope` joins on all paths (no leaks), merge by
work-item index, `first_failure` by lowest failed index (never
completion time), pre-start cancellation with cleanup accounting, and
per-task authority (grants + budgets travel per item). In-flight
bounded calls run to their step budget — the same bound the index
subprocess driver enforces with its timeout — so no wakeup mechanism
is owed and none is claimed. No fairness, deadlock-freedom,
lock-freedom, or scheduler-independence property is claimed anywhere.

**Filesystem (RFC 0008 C3 → partial).** Five intrinsics under two
effect kinds (`fs_list`, `fs_read`) with `--grant-fs` authority (§1–3
of `docs/fs-resource-effects.md`). Canonical byte-order enumeration
with snapshot identity + generation provenance; bounded chunked reads
with short/empty EOF; fail-closed denies, escapes, stale indexes,
directory/symlink reads, overlong-name skips, and the 1024-entry
bound. Watch is an experimental generation-poll hint, not events.

**Chunked text and deterministic collections.** `mncs.std.chunk.v1`
cursors/spans/newline scans with cross-chunk join measurement;
`mncs.std.sort.v1` insertion sort + sorted-dedup over u64 codes;
`mncs.std.relation.v1` edge sets with canonical normalize and
level-synchronous bounded transitive closure (cycles terminate,
duplicates rejected, missing targets are leaves, budgets report
`truncated`). All pure, all total for hostile counts, all on five
backends.

**Digest.** `mncs.std.sha256.v1`: compression, streaming
init/update/finalize over ≤64-byte views, no grant, no capability, no
new trusted primitive. NIST "abc" plus a 156-byte input in two
chunkings converge to the hashlib oracle.

**Persistence (RFC 0026 → SUBSTRATE).** `mncs.std.store.v1`:
generations, atomic compare-and-transition with typed stale results,
`VOLATILE/PROCESS/POWER` receipts with power-loss explicitly UNKNOWN,
snapshot handles with total release, retention policy as data. No
mechanism, no crash evidence — that is the defined next slice.

**Invocation (PRESS-010).** `Session::call_batch` equivalent at the C
ABI (`mncs_session_call_batch`: many entrypoints, one crossing, order
preserved) plus the Rust `TaskScope::{run_batch, run_concurrent}`.
Index-shaped proof: 600 mixed-entrypoint calls, warm mean 0.018 ms,
p50 0.020 ms, p99 0.025 ms (debug build), artifact digest on every
result, granted host-effect call in the same workload. Recorded as
language-resolved / consumer-integration pending.

## 3. Architecture fit

- Source: five `fs_*` intrinsics (0.12-gated, MNP204/205) plus six
  pure stdlib modules. No syntax change for concurrency (contracts
  are values) or digest/relations (bitwise + iteration substrate).
- Elaboration: `elaborate_fs_nullary/index/entry_at/read_bytes_at`
  with MNE257–262 authority/type diagnostics; record-valued `select`
  proven portable on all backends before use.
- Semantic model: `host_call_effect_kind`/`host_call_arity` extended
  (`fs_list`/`fs_read`); validation unchanged in shape (arity +
  closure checking reuse).
- Proof/evidence: no kernel change (Section 4). New stdlib proves
  through existing obligations; concurrency properties rest on
  convergence + join-guarantee evidence, not proof.
- HIR/SSA: `HostCall` operation ids flow through; no new IR forms,
  so PTX/RISC-V/eBPF artifact contracts are untouched.
- Runtime: one shared realization (`fs_resource.rs`) for body and
  SSA executors (layer agreement tested); embed `scope.rs` for
  threads; pure stdlib everywhere else.
- Backends: pure slices run on all five; `fs_*` executes on research
  with explicit lowering refusal elsewhere (pre-existing `HostCall`
  arm, no new refusal code needed).
- Stdlib: scope, channel, chunk, sort, relation, sha256, store.

## 4. Proof/soundness impact

- New obligations: none. All new constructs reuse the existing
  obligation framework (iteration-cost CMP301s appear and fall back
  conservatively, as with prior stdlib).
- Proof-kernel changes: none. The kernel stays small by design;
  SHA-256 deliberately avoids a new trusted primitive (pure MNCS on
  the bitwise substrate), and concurrency properties exceeding the
  kernel use the RFC 0010 external-evidence architecture (stress
  convergence, join guarantees, step budgets).
- PASS/FAIL/UNKNOWN discipline preserved: no missing evidence was
  promoted. Power-loss survival is carried as UNKNOWN in receipts;
  scheduler properties are explicitly unclaimed; the watch hint is
  marked experimental.
- Trusted base did not grow: one shared, audited realization file
  for fs effects; everything else is stdlib + a host-side runtime
  (threads are a realization detail behind a deterministic API).
- Two genuine defects were found and fixed by the campaign's own
  testing (not by weakening tests): `channel.close` mutated on
  rejection (close-smuggling), and `relation.normalize` had an
  inverted swap plus a compaction that clobbered slot 0 on skips.
  Both are pinned by regression corpora.
- Three pre-existing language constraints were documented as new
  pressure (Section 8) rather than worked around: `up_to` counters
  are unbound, param-rooted index-then-field projections do not
  parse, and iteration bodies must stay straight-line.

## 5. Backend matrix

| capability | research | wasm | C11 | LLVM | Cranelift |
|---|---|---|---|---|---|
| scope/channel contracts | execute | execute | execute | execute | execute |
| chunk/sort/relation/store contracts | execute | execute | execute | execute | execute |
| pure SHA-256 | execute | execute | execute | execute | execute |
| `fs_*` effects | execute (grant) | refuse | refuse | refuse | refuse |
| embed TaskScope/batch | n/a (host runtime; per-backend sessions) | — | — | — | — |

"Refuse" means explicit lowering failure (`host calls are
unsupported …`), proven by `unrealizing_backends_refuse_fs_explicitly`.
No backend silently no-ops, emulates, or reports support by mere
compilation. PTX/RISC-V/eBPF contracts untouched (no new IR forms);
live probes on the scope corpus confirm the honesty shape is
preserved there too: `mncs-ebpf` exits 1 with
`completed_with_unresolved_obligations` and CMP301 diagnostics
naming the retained integer-overflow obligations (no fabricated
cases); `mncs-riscv32`/`mncs-ptx64` mark every case `unsupported`
with an explicit missing-toolchain reason (no emulator / no
ptxas-CUDA), exit 1; misspelled names (`mncs-ptx`, `mncs-riscv`)
are refused as `unknown backend` with the adapter list.

## 6. Performance evidence (debug build)

- Reusable session: open (compile+verify) ~207 ms once; frozen
  artifacts skip compile via `from_json`.
- Warm call: mean 0.018 ms, p50 0.020 ms, p99 0.025 ms over 200
  calls (subprocess is ~10 ms: ~500x).
- 600-call concurrent scope wall: ~64 ms (~0.1 ms/call amortized
  incl. 8 thread spawns + 8 session opens).
- SHA-256: 28k steps ("abc"), 66k steps (156 bytes); identical step
  counts across chunkings.
- Relation closure and SHA-256 dominate native-backend test time
  (collections suite ~221 s, sha256 suite ~133 s); step budgets hold
  with margin.

## 7. Tests (commands and results)

New suites (all green in this run):

- `cargo test -p mncs-cli --test task_scope` — 3 tests, 5 backends.
- `cargo test -p mncs-cli --test channel_contract` — 3 tests, 5 backends.
- `cargo test -p mncs-embed --test task_scope` — 8 tests incl. the
  600-call benchmark, failure/cancel/ownership shapes, the fs-grant
  path, and the C batch boundary.
- `cargo test -p mncs-cli --test fs_effects` — 12 tests (granted
  corpus, fail-closed, refusal, determinism, order-independence,
  empty roots, generation movement, stale/nonfile/symlink refusals,
  bounded large-file reads, body/SSA agreement).
- `cargo test -p mncs-cli --test collections` — 3 tests, 5 backends
  (chunk/sort/relation corpora).
- `cargo test -p mncs-cli --test sha256_pure` — 3 tests, 5 backends
  (hashlib oracle, chunking convergence).
- `cargo test -p mncs-cli --test store_contracts` — 3 tests, 5 backends.

Corpora (regenerable): `scripts/gen_scope_channel_corpora.py`,
`scripts/gen_fs_scan_corpus.py`, `scripts/gen_collection_corpora.py`,
`scripts/gen_sha256_corpus.py`, `scripts/gen_store_corpus.py`.
Fixtures: `examples/source/fs-scan.mncs`,
`examples/source/fs-invalid-*.mncs` (6), `examples/fs-fixture/`.

Full workspace verification (final tree, this run): `cargo test
--workspace` exits 0 with zero FAILED/panicked/error lines across
all targets (failure-filtered re-run after a stale pre-fix run was
terminated; the stale run's verdict is discarded); `cargo fmt
--check` clean; `cargo clippy --workspace --all-targets -- -D
warnings` clean.

## 8. Consumer deletion map (for the next `mncs-index` run)

Do not delete these yet — this run proves the language side only.
Each row names the host workaround a later integration run should now
be able to remove, with the replacement:

- `pipeline.py` `ThreadPoolExecutor` fan-out → `TaskScope` (or the C
  batch API) with per-task budgets; deterministic merge by index
  replaces completion-order handling.
- `queue.Queue` + `threading.Event` drain protocol → `channel`
  contracts (capacity, backpressure verdicts, producer accounting,
  close-once) with the runtime looping on verdicts.
- `cancelled`/`failed`/`timeout` bookkeeping in the index driver →
  `scope` outcomes + `ScopeRun.first_failure`/cancelled distinction.
- `discover.py` fast-path `os.scandir` recursion → `fs_*`
  enumeration (canonical order + snapshot identity + generation);
  keep `os.walk` only as the differential oracle during migration.
- `rich_model.py` split/slice/sort control plane →
  `chunk`/`sort`/`sha256` stdlib over capability-fed views.
- `graph.py` adjacency/visited/ordering ownership → `relation`
  edge sets + bounded closure (kernel keeps admission decisions).
- `dual-hash` (host SHA-256 cross-check) → `sha256` stdlib through
  reusable sessions; delete only after the migration measures
  per-window cost on real workloads.
- `durable.py` compare-and-swap/publication strings → `store`
  contracts once a mechanism lands (NOT yet — substrate only).
- Subprocess-per-call invocation → reusable sessions / batch C ABI
  (PRESS-010 consumer adoption; the Python binding shape is the
  integration run's decision, not this run's).

## 9. Remaining pressure (dependency-ordered)

1. Source-threaded execution + blocking channel transport (needs
   first-class work descriptors; blocked by deferred function
   values). The contracts and the embed realization bound it.
2. Filesystem write/mutation effects and true event delivery
   (PRESS-007 watch half proper). Related bound to consider: walk
   work scales with tree size (skipped overlong names feed the
   generation but not the 1024-entry bound); a walked-path cap is
   future hardening, acceptable now because grant roots are
   operator-chosen and IO failure is fail-closed. Truncation races
   in `fs_read_bytes_at` fail closed (RuntimeFailure) rather than
   delivering torn prefixes.
3. Durable mechanism + crash evidence behind the store contracts
   (PRESS-013/016/017/018 realization; process-death tests first,
   power-loss honesty preserved).
4. Unbounded-string/split/sort remainder of PRESS-005 (narrowed by
   chunk/sort/relation, not closed).
5. Grammar asymmetries found this run: `up_to` counters unbound;
   param-rooted index-then-field projections do not parse;
   conditional `next` inside iterate bodies rejected (MNP061).
   Each is a small, well-isolated language change with a specified
   backward-compatible shape.
6. PRESS-019 distribution: still gated (no transport, wire
   contract, or retry semantics added).

## 10. Recommended next run

A focused `mncs-index` integration run (not another language run):
adopt reusable sessions for the fixture build, replace one system
(discovery enumeration first — smallest blast radius) with `fs_*`
calls keeping `os.walk` as the differential oracle, and measure the
dual-hash removal on real windows. Do not start language-level
distribution or the durable mechanism until that integration lands;
both depend on its measurements.
