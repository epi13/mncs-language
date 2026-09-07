# Stage A — correctness, ABI, and module composability (2026-09)

Campaign branch: `feat/stage-a-sequence-abi`. Baseline: `c4e302c` (suite
green: `cargo test --workspace` exit 0, 47 binaries, 0 failures).
Reproducer classification against baseline: P-001/P-003/P-008/P-009/P-010/
P-012/P-013 STILL PRESENT; P-011 OPEN; P-014 STILL PRESENT with a stale
reproducer (names `PlatformEvent`, absent from current `platform.v1`);
P-007 PRODUCT-LOCAL; P-002/P-004 DEFERRED. No ALREADY-FIXED or STALE items
beyond P-014's vocabulary drift.

## A1 — sequence-return WASM correctness (P-013)

Reproduced exactly: `[48,55]` observed as `[48,0]` on
`mncs-portable-wasm-mvp`; other backends logically correct.

Root causes found (three, all fixed):

1. **Cell-marker ambiguity (semantic).** The portable-WASM lowering marks
   byte views over exact canonical-cell sequences with bit 0 of the packed
   descriptor. Bit 0 also carries address parity, so an odd-offset packed
   slice (`window[1..3]`) was indistinguishable from a cell-backed view:
   internal reads, bound checks, lengths, and the return readback all
   misbehaved (second byte zeroed; bounds traps defeated by a huge
   marker-polluted length). Fix: the marker moved to bit 63
   (`VIEW_CELL_MARKER` in `mncs-codegen/src/composite.rs`), outside the
   address and length halves. Length/bound reads mask it
   (`VIEW_LENGTH_MASK`); the return readback branches stride on it.
   Zero-copy views preserved; no new copies, no new ops.
2. **Native-backend prelude gap (codegen).** `scalar_module_uses_cells`
   ignored `SequenceProject`, so pure index-into-view modules omitted the
   canonical slot helpers: undeclared-function C failures and an
   unresolved-symbol Cranelift JIT panic. Fix: projection counts as cell
   use (`mncs-codegen/src/support.rs`).
3. Latent length/bound-check masking (same tranche, same cause).

Proof: `examples/source/abi-view-returns.mncs` (19 value cases: P-013
vectors, empty, len-1, exact-64, full-64, odd/even starts, param slices,
non-byte views, repeated calls, scalar consumption) all-met on all five
executable backends; `abi-view-returns-traps-corpus.json` pins uniform
`runtime_failure` for OOB index and reversed slice. Gap artifact
`scratchtrack/p013-view-returns` (PASS).

## A2 — versioned host ABI contract (P-011)

New normative `spec/host-abi.md`: HOST-ABI version 1 (envelope, scalars,
exact sequences, packed view descriptors, host-buffer staging protocol,
results, failure taxonomy, executable vector table, non-contracted items).
`mncs abi` carries `host_abi_version` (`HOST_ABI_VERSION` constant) so
hosts check the contract instead of reading compiler source. Honest
provisional status for the bit-63 cell selector on returned byte views
(documented, branched on, masked; packed-only normalization is declared
future work) and for cross-call memory-growth discipline (host-test
obligation; experiment runs reset per case). New unit tests: descriptor
codec marker isolation, arena allocator alignment/monotonicity/bounded
exhaustion. Gap artifact `scratchtrack/p011-host-abi` (PASS).

## A3 — imported nominal types (P-014)

The stale reproducer named a removed vocabulary; with current vocabulary
(`Os`, `Version`) imported nominals in signatures, match, projection, and
cross-module calls already elaborate. Delivered the missing proofs:

- `examples/source/test/nominal/machine.mncs` (declares `Phase`/`Evt`/
  `Transport` + transition/query fns) and `consumer.mncs` (unqualified AND
  qualified use in params, returns, construction, projection, matching,
  nested calls; 0.6 machine imported from a 0.10 consumer).
- 10-case corpus, all-met with overall PASS on all five backends;
  `mncs abi` shows declaring-module identities flowing into the consumer.
- Decisions recorded: dotted construction (`Os.Linux`) is canonical;
  `::` value paths are not term syntax (INVALID); bare variants resolve as
  match patterns only (values need the dotted form); dotted QUALIFIED type
  paths require Profile 0.9+ (grammar), unqualified unique imports work
  from 0.6.
- Elaborator tests: identity retention, `MNE161` for projection from a
  non-record; existing `MNE174/175/234` negatives unchanged.
- Gap artifact `scratchtrack/p014-nominal-imports` (PASS).

## A4 — exact-to-bounded-view borrow (P-010)

Rule (`docs/source-profile-0.7.md`): `[E; N]` satisfies `[E; up_to M]`
iff `N <= M` with equal elements. The compiler synthesizes the
full-range slice at the expectation site (annotated `let`, bare names,
call results incl. nested calls, call-argument backstop, both return
paths), so the borrow is explicit in the body IR and lowers through the
proven view machinery: no copy (aliases immutable cells), bound preserved
from the static length, element identity untouched. Refusals preserved:
`N > M`, element mismatch (`MNE133`), view-to-view capacity relaxation
(future work, honestly refused). Tail-`return name;` fast path routes to
general elaboration only when the borrow applies (side-effect-free probe
on a cloned env). Stdlib unlock: `mncs.std.encoding.v1::read_u16_le` /
`read_u32_le` over `[byte; up_to 64]`; `examples/source/subtype-windows.mncs`
proves one reader serving 44/46/22-byte windows plus nested/let/return
borrows and direct stdlib calls — 10 cases all-met on all five backends.
Unit tests for accept/refuse/nested/tail shapes. Gap artifact
`scratchtrack/p010-view-borrow` (PASS).

## Backend matrix (Stage A)

| Capability | research-bytecode | portable-wasm | c11 | llvm-ir | cranelift |
|---|---|---|---|---|---|
| view returns (19 cases) | met | met | met | met | met |
| view traps (2 cases) | runtime_failure | runtime_failure | runtime_failure | runtime_failure | runtime_failure |
| nominal imports (10 cases) | PASS | PASS | PASS | PASS | PASS |
| view borrow (10 cases) | met | met | met | met | met |

Overall corpus statuses remain `UNKNOWN` where integer/index obligations
are retained (P-003, unchanged); per-case values are verified, never
assumed. External/compile-only targets untouched and still labeled.
