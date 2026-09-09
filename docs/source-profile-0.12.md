# Source Profile 0.12 — binary64 floats, trigonometry, and granted-filesystem observation

Status: **implemented, experimental**. Profile 0.12 is additive over
Profiles 0.1–0.11. Older profiles retain their syntax, semantics, and
canonical fingerprints; in particular, float literals stay refused
(`MNP197`), float intrinsics stay refused (`MNP201`, "float intrinsics
require source profile 0.12 or later"), and filesystem intrinsics stay
refused (`MNP204`) below 0.12.

Resource ceilings are unchanged from Profile 0.11: iteration bounds
1..=32 per level (`MNE142`), at most two nested levels (`MNE147`), a
4096-step static work envelope for traversal compositions, and admitted
sequence/view lengths of at most 64 (`MNE182`/`MNE105`/`MNE225`).

## Binary64 arithmetic and conversions

`f64` is the only admitted float. Literals are finite `digits.digits`
spellings: the parser refuses non-finite spellings (`MNP198`), and `f32`
parses only so validation can report the single-width rule — every
backend realizes exactly one float domain.

`+ - * /` and the six comparisons are total over finite values under the
non-finite trap rule: any non-finite input or result is a deterministic
runtime failure (fail-closed, like checked division's singular inputs),
including division by zero, `0.0 / 0.0`, and overflow to infinity. Every
backend guards both inputs and (for arithmetic) the result, so NaN
payloads never cross backend boundaries.

Conversions (`as` between int/byte/bool and float): int/byte/bool to
float round per IEEE-754 (always finite); float to int/byte truncates
toward zero and traps on non-finite or out-of-range inputs, checked
against the half-open domain exactly (including the fractional sliver
below a negative bound and `-0.0` into unsigned domains).

Mixed int/float arithmetic, float sequences/vectors, and checked float
arithmetic stay refused; boundary values are computed, never spelled.

## Trigonometric intrinsics

`sin(x)` / `cos(x)` over finite binary64, via same-process libm on
every backend so layers agree bit-exactly. The operand must be finite;
results for finite inputs are finite, guarded like arithmetic.

## Granted-filesystem observation

`fs_list_count()`, `fs_entry_name_at(i)`, `fs_entry_kind_at(i)`,
`fs_generation()`, and `fs_read_bytes_at(entry, offset, length)` expose
bounded observation of an explicitly granted directory tree
(`--grant-fs capability=root-path`; chunk reads additionally require
`effect fs_read` plus its capability). No path is spelled in source:
indices name entries in canonical byte-sorted order, reads are capped at
64 bytes, and indices are generation-scoped (re-list when
`fs_generation` moves). Reads through directories or symlinks are
refused. Authority comes from the enclosing function's declarations;
without a grant there is no access.

## Explicit non-claims

- No `f16`/`f32` execution domain, no decimal floats, no float formatting.
- No float-indexed or float-element sequences, vectors, or views.
- No ambient filesystem access: every tree operation needs its grant.
- No filesystem mutation in 0.12 (observation only; mutation hints such
  as `host_write` storage appends are a separate capability).
- Cross-backend float agreement is bounded empirical agreement over the
  corpus, not proof; the trap rule (fail-closed, never a wrong value) is
  what the profile guarantees.

## Evidence

- `examples/source/stage-c1-float-arithmetic.mncs` (24 functions) and
  `examples/execution/stage-c1-float-corpus.json` (69 cases from an
  independent Python binary64 oracle): value agreement on all five
  executable backends, trap cases asserting `runtime_failure`.
- `crates/mncs-cli/tests/stage_c1_float.rs`.
- `docs/development-evidence/stage-c1-float-arithmetic-2026-09.md` and
  `docs/development-evidence/stage-c2-trig-2026-09.md`.
- Filesystem observation: `examples/source/fs-*.mncs`,
  `examples/execution/fs-scan-corpus.json`,
  `crates/mncs-cli/tests/fs_effects.rs`, `--grant-fs` CLI surface.
