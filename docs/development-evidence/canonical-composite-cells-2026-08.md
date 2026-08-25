# Canonical composite cells — 2026-08-25

Status: **development record**. Local observations on one host; CI results are
recorded separately in the pull request. No production-readiness or universal
equivalence is claimed.

## What landed

One language-owned contract now governs composite values (records and
payload-bearing finite variants) across every executable backend:

**MNCS canonical cell layout v0.1** — 8-byte aligned cells; record field `i`
at byte offset `i * 8` in canonical name-sorted order; boxed finite tag in
slot 0 with payload fields from byte 8; composite field values stored as cell
references into a per-execution arena.

Realizations:

| Backend | Arena | Cell access |
| --- | --- | --- |
| research bytecode | interpreter state | direct SSA interpretation |
| portable WASM | linear memory + bump global (pre-existing) | i32/i64 load/store |
| C11 | module-defined `mncs_arena` symbols | `memcpy` slot helpers |
| LLVM IR | `@mncs_arena` / `@mncs_bump` globals | GEP load/store |
| Cranelift (JIT + AOT) | host runtime / driver-defined arena | five uniform libcalls |

Process boundary: a canonical call file carries every argument (scalar bits
or cell roots) plus the preloaded arena image; the driver dumps the final
arena so composite results decode against the artifact's value contracts on
the host side. Pure scalar calls keep the historical argv-only protocol.

## Observed agreement

`mncs.core.status.v1` (records, payload-free sums, 30 cases): PASS on all of
research bytecode, WASM, C11, LLVM IR, and Cranelift.
`mncs.core.result.v1` (boxed payload sums, 8 cases): UNKNOWN-as-honest with
8/8 met on all five — identical bounded observations everywhere.

## Honest scope

- The wire format and layout are versioned (v0.1); changing them requires a
  new contract version, not silent drift.
- Cranelift executes through JIT where host policy allows and through the AOT
  object + driver elsewhere; both consume the same builder path.
- Bounded corpora remain finite evidence, not proof of equivalence.
