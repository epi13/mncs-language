# NEXT RUN — MNCS-LANGUAGE + SCRATCHTRACK (Stage A handoff)

Stage-A merge: the `feat/stage-a-sequence-abi` head, fast-forwarded to `main` (single Stage-A commit; see `git log`).
Language baseline before Stage A: `c4e302c`.

For every item: what ScratchTrack should replace, simplify, or execute.
Nothing below was applied to ScratchTrack in this run (read-only).

## P-013 — sequence-return WASM correctness → RESOLVED

- Semantic capability: bounded byte views derived from locally-built exact
  sequences return exact logical bytes on every executable backend; odd
  packed slice offsets read correctly; OOB/reversed slices trap uniformly.
- Original reproducer outcome: `textprod_slice_attempt` s02/s13 FAIL on
  portable-wasm (`[48,55]`→`[48,0]`, `[55,46]`→`[55,0]`).
- New outcome: both PASS (values exact; overall UNKNOWN only from retained
  obligations, same as every module).
- ScratchTrack unlock: `formatPosition` and other variable-width text
  production can return `[byte; up_to N]` slices through production WASM
  instead of fixed-width exact workarounds — after regenerating the
  browser loader for the bit-63 note below.
- Follow-up: browser `mncsWasm.ts` must branch returned byte-view reads on
  bit 63 of the descriptor (set = one byte per eight-byte slot from
  `offset`; clear = packed bytes; mask bit 63 before `>> 32`). Publish the
  branch behind the `host_abi_version == "1"` check from `mncs abi`. The
  next campaign repins the compiler revision and re-runs the text E2E.

## P-011 — unpublished WASM host ABI → RESOLVED (contract v1)

- Semantic capability: normative `spec/host-abi.md` HOST-ABI version 1;
  `mncs abi` reports carry `host_abi_version: "1"`.
- Original outcome: hosts reverse-mapped `lower.rs`/`wasm.rs`.
- New outcome: envelope, scalars, exact/views, staging protocol, results,
  failures, and executable vectors are contracted; `mncs abi` binds each
  module report to the contract version.
- ScratchTrack unlock: regenerate `mncsWasm.ts` projections from
  `spec/host-abi.md` + `mncs abi` instead of compiler source; add the
  version check; implement the section-6 bit-63 branch and the section-5
  reset discipline as specified.
- Follow-up: two-repo pass replaces tribal loader knowledge with generated
  marshalling; packed-only return normalization stays a language-side
  future item (hosts must code both representations per the contract).

## P-014 — imported nominal types → RESOLVED (reproducer was stale)

- Semantic capability: imported finite/record identities elaborate and
  execute end to end (params, returns, construction, projection, match,
  nested calls, ABI identities) — proven by
  `examples/source/test/nominal/{machine,consumer}.mncs`, 10/10 PASS on
  all five backends.
- Original reproducer outcome: `events_import_attempt` refused (MNE105/
  MNE161) — but it names `PlatformEvent`, which no longer exists in
  `platform.v1`. The refusal was correct for a missing type.
- ScratchTrack unlock: shared event/state vocabularies can move into one
  MNCS module and be imported (unqualified names work when unique;
  dotted construction `Os.Linux`; qualified type paths need Profile 0.9+;
  bare variants are match-patterns only, never values; `::` value paths
  are not MNCS syntax).
- Follow-up: port `events_import_attempt` to the living vocabulary (or the
  new nominal pair as a template), delete the local re-declarations, and
  execute the shared machine through production WASM.

## P-010 — exact→bounded view compatibility → RESOLVED

- Semantic capability: `[E; N]` auto-borrows into `[E; up_to M]` (`N ≤ M`,
  same element, zero-copy) at let/call/return positions; `N > M` and
  element mismatch still refuse (`MNE133`).
- Original reproducer outcome: `subtype_attempt` MNE133.
- New outcome: elaborates clean; shared-reader pattern proven
  (`subtype-windows`, 10/10 on all backends).
- ScratchTrack unlock: collapse per-width LE-reader duplication —
  one `read_u16_le`/`read_u32_le` (now in `mncs.std.encoding.v1`) serves
  the 44/46/22-byte `le.mncs` windows; remove workarounds that copy or
  re-declare per width. (View-to-view capacity relaxation is NOT included:
  keep passing exact windows, not sliced views, into narrower bounds —
  actually narrower-to-wider views also refuse; pass exacts.)
- Follow-up: rewrite `le.mncs` against the stdlib readers (passing exact
  windows; view-to-view capacity changes refuse in both directions) and
  re-run the WAV/pack E2E through production WASM.

## Still open (intentionally future)

- P-002 bounded text values, P-004 effects/async, P-001/P-008 floats+trig,
  P-009 bitwise, P-012 nested iteration: untouched; separate campaigns.
- P-003 verification: obligations still UNKNOWN where retained; no
  weakening performed.
- P-005 packed host buffers: bytes proven; float views and published
  buffer contracts ride with the float tranche.
- P-007 sub-tick representation: product-local, unchanged.
