# Ox Alpha tranche evidence — standard library + Profile 0.6 — 2026-08-24

Status: **development record**. Bounded local evidence only. Nothing here is
independently evaluated, frozen, promoted, production-ready, or claimed as
universal equivalence. Baseline verification is recorded in
`ox-alpha-baseline-2026-08.md`; this file records what the tranche added and
what remains open.

## Commits (this branch, `lineage/language-integration`, superset of `main`)

| Commit | Subject |
| --- | --- |
| `861e9c7` | docs: record Ox Alpha 2026-08 baseline; declare Profile 0.5 in backend matrix |
| `6f57913` | feat: start mncs.core standard library with status, logic, and ordering modules |
| `54886be` | fix: simplify upstream snapshot comparison in library core test |
| `7c97a65` | feat: Source Profile 0.6 payload-bearing finite variants (sums) |
| `10f033c` | feat: mncs.core.result with real reason payloads; division/modulo in source; wildcard payload patterns |
| `768b317` | feat: strict boolean operators && and \|\| in Source Profile 0.6 |

## What was implemented

### mncs.core (library/core)

Executable, tested MNCS source — not documentation:

- `mncs.core.status.v1`: three-valued status lattice with dominance join
  FAIL ⊒ UNKNOWN ⊒ PASS; pair combination over `StatusPair`; decidedness and
  dominance predicates. No operation can turn UNKNOWN into PASS or delete FAIL.
- `mncs.core.logic.v1`: total boolean algebra helpers (`bool_not/and/or/implies/xor`).
- `mncs.core.ordering.v1`: comparison-only min/max/clamp for i32/i64.
- `mncs.core.result.v1` (Profile 0.6): Result sums with real reason payloads
  (`Ok { value }`, `Err { reason }`), division-based operations with explicit
  error reasons, exhaustive payload matching.

Corpora: `examples/execution/library-core-{status,logic,ordering,result}-corpus.json`,
regenerated deterministically by `scripts/gen-library-core-corpora.py`.
Negative fixture: `examples/source/library-core-status-wrong.mncs` launders
UNKNOWN into PASS and fails on exactly that corpus cell.

Observed experiment statuses (2026-08-24, local):

| Module | Corpus | research bytecode | portable WASM |
| --- | --- | --- | --- |
| status | 30 cases | PASS | refused: record parameter `StatusPair` (CGN302) |
| logic | 18 cases | PASS | PASS |
| ordering | 36 cases | PASS | PASS |
| result | 8 cases | UNKNOWN (honest overflow obligations only) | refused (CGN302) |
| status-wrong mutant | 9 cases | **FAIL** on `dominate-unknown-unknown` | — |

### Source Profile 0.6

New gated profile (`docs/source-profile-0.6.md`):

1. Payload-bearing finite variants: declaration, qualified record-style
   construction, bare/qualified patterns, exact-once payload binders,
   `{ .. }` wildcard, mandatory variant exhaustiveness.
2. Strict boolean operators `&&` / `||` with documented precedence ladder.
3. Checked `/` and `%` with explicit runtime failure for division by zero and
   `MIN / -1`.

Identity compatibility: new model fields serialize as absent when empty; all
Profile ≤ 0.5 fixtures pass without migration (170+ tests, no identity
migration performed or needed).

Realization envelope: body/SSA reference executors realize all of 0.6;
research bytecode realizes payloads, booleans, and refuses div/mod explicitly;
portable WASM realizes booleans and refuses payload sums/div-mod explicitly;
C11/LLVM/Cranelift refuse the new shapes explicitly. Every refusal is a
structured CGx30x capability diagnostic; none silently drops semantics.

### Consumer binding (RAVEL)

`examples/consumers/ravel-core-snapshot.mncs` freezes RAVEL's shipped
`ravel.core.v1`. It canonicalizes to fingerprint
`93813943182726e085bb361bf6de48a238dcca73b407e100d6b1da085277ed2a`,
identical to the upstream file `RAVEL/mncs/workspace/ravel_core.mncs`
(verified against the sibling checkout via `RAVEL_UPSTREAM_CORE`). The test
`ravel_snapshot_agrees_with_core_status_lattice_on_bytecode` binds the
consumer's `dominate` to the library join over the full 3×3 domain: 9/9 case
agreement on research bytecode.

## Acceptance criteria status

1. `mncs.core` executable and tested — **done** (four modules, corpora, negative fixtures).
2. MNEL and RAVEL consume shared core — **partially done**: RAVEL bound by
   identity-bound differential agreement (snapshot + test). The first MNEL-side
   module is future work (MNEL has no MNCS sources yet; its reconstruction has
   not started). Symbol-level consumption by any consumer is blocked below.
3. Semantic identity/contracts/effects/obligations retained through the chain —
   **done** (operations flow through semantic graph → HIR → SSA → selected SSA
   → backend artifact → execution evidence like every other program; identities
   are content-derived per run).
4. Payload-bearing results with negative tests — **done** (positive fixture +
   6-case corpus + four negative fixtures rejected with MNE140/MNE174/MNE177/MNE175).
5. At least one bounded collection abstraction — **not implemented; blocker
   recorded**: element storage needs an indexable aggregate type, which needs
   RFC 0009 memory/storage semantics and RFC 0019 array/slice value models;
   bounded iteration (RFC 0043) exists but carries one scalar state value, not
   a sequence. Recorded as the next language vertical slice after module linking.
6. Records execute through bytecode + second backend — **unchanged from
   baseline, envelope documented**: WASM still refuses cross-function/carried
   records and record parameters/results (CGN302, re-verified); C11/LLVM refuse
   record values. Extending portable WASM via multi-value results or planned ABI
   flattening recorded in TargetLoweringPlan remains the next backend slice.
7. Backend neutrality preserved — **done**: no semantic workaround was added;
   every unsupported shape is refused with a structured diagnostic.
8. UNKNOWN never authorizes stronger lowering — **preserved** (payload programs
   report overall UNKNOWN from honest overflow obligations; refusals are
   capability-scoped; promise decisions unchanged).
9. Legacy-vs-native differential coverage — **partial**: native-vs-consumer
   binding for RAVEL's decision spine (dominance lattice) is covered; legacy
   Python/Rust execution comparison remains an open obligation.
10. Language service resolves library symbols — **blocked on publishing**: the
    service depends on `mncs-language` via git branch `main`; this tranche is
    local commits. After merge, library modules resolve as ordinary modules
    because they use only public compiler artifacts; no service change is
    required for symbol resolution, though contract/capability reporting
    surfaces may want dedicated rendering later.
11. Profiles 0.1–0.5 fixtures valid — **done** (full workspace tests green;
    serialization compatibility designed and tested).
12. Gates — **green at last run**: fmt, clippy `-D warnings`, full workspace
    tests (171 passed / 0 failed), CLI integration suites including new
    `library_core.rs` and `profile06_payload_sums.rs`.
13. Documentation distinguishes implemented/experimental/refused/blocked/unclaimed —
    **done** (this file, baseline file, profile doc, library README).
14. Final report — this document plus the session summary.

## Open obligations and blockers

- Module linking (RFC 0014): blocks true import/consumption of `mncs.core`;
  blocks service-side library resolution until merged and repointed.
- Bounded collections: blocked on RFC 0009 memory semantics + RFC 0019
  aggregate values (see criterion 5 above).
- Record values across WASM boundaries: next backend slice; current refusal
  re-verified as intentional.
- Exact-cost obligations remain UNKNOWN everywhere arithmetic exists.
- Cranelift host-JIT executable-memory limitation remains host-scoped.
- Native object/linker/executable lifecycle, Raspberry Pi claims: unclaimed,
  unchanged from baseline.
- Short-circuit boolean forms: deliberately not implemented; require explicit
  control/effect semantics first.

## Reproduction

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 scripts/gen-library-core-corpora.py   # deterministic regeneration
cargo run -p mncs-cli -- experiment run library/core/status.mncs \
    --backend mncs-research-bytecode \
    --corpus examples/execution/library-core-status-corpus.json
RAVEL_UPSTREAM_CORE=../RAVEL/mncs/workspace/ravel_core.mncs \
    cargo test -p mncs-cli --test library_core
```
