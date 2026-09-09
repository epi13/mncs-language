# Compiler-Pressure Reconciliation and Repair (2026-09)

## Contract

Reconcile every `mncs-compiler/pressure` finding (CP-0001 through CP-0013,
recorded against an older pinned Stage-0 revision) against the current
language, land well-tested repairs for pressures that still reproduce, and
make a proof-aware attack on the structural-recursion pressure without
weakening termination or soundness. `mncs-compiler` was read-only
throughout: workload, evidence source, and reproduction corpus only.

Reconciliation method: every compiler `pressure/repro/*.mncs` program was
re-run against the current implementation before any change. Findings
already fixed by prior language work became regression evidence, not new
projects.

## Reconciliation ledger

| Pressure | Before this tranche | After | Evidence | Status |
|---|---|---|---|---|
| CP-0001 (source storage) | 65-byte repro already passes (1024 ceiling) | unchanged | `pressure/repro/source-65.mncs` clean | old repro resolved; whole-module storage architecture requires follow-up, not a ceiling raise |
| CP-0002 (unicode) | ASCII-only whole-source APIs | unchanged | finding text | not language-owned this tranche (stdlib/runtime + compiler arch) |
| CP-0003 (scan cost) | bounded no-op iterations | unchanged | finding text | future pressure; no `while` added, boundedness preserved |
| CP-0004 (bool ops) | `bool == bool` → MNE121, no `!` (MNL002) | `BooleanCompare` / `BooleanNot`, all backends | `examples/source/pressure-bool-ops.mncs`, `pressure-bool-ops-corpus.json` (17 cases × 5 backends), `crates/mncs-cli/tests/pressure_bool_ops.rs` | fixed in this tranche |
| CP-0005 (test transport) | host test transport | unchanged | finding text | tooling-owned, not a language defect |
| CP-0006 (envelope) | leading-comment inference | unchanged | finding text | tooling-owned, not new syntax |
| CP-0007 (evidence cost) | IR/evidence sizes | unchanged | finding text | motivates improvements, not verification shortcuts |
| CP-0008 (finite payload identity) | already repaired (canonical payload resolution + qualified construction) | regression evidence only | `crates/mncs-cli/tests/pressure_enum_payloads.rs` (pre-existing, re-verified green) | fixed by prior language work |
| CP-0009 (bounds) | `up_to 256` already passes (1024 ceiling) | regression evidence only | `crates/mncs-cli/tests/pressure_raised_bounds.rs` (pre-existing, re-verified green) | that portion resolved by prior work |
| CP-0009 (identities) | sequential `iterate i` → MNE146 | live-scope identities + hygienic `i#2` records, nested still MNE146 | `examples/source/pressure-iteration-reuse.mncs` + corpus (4 cases × 5 backends), `crates/mncs-cli/tests/pressure_iteration_identity.rs`, `crates/mncs-compiler/tests/iteration_identity.rs` | fixed in this tranche |
| CP-0010 (scalar match) | integer patterns → MNP084 | total scalar dispatch, elaboration-side exhaustiveness, all backends | `examples/source/pressure-scalar-match.mncs` + corpus (20 cases × 5 backends), `crates/mncs-cli/tests/pressure_scalar_match.rs` | fixed in this tranche |
| CP-0011 (recursion) | every cycle → MNE130 | RFC 0047 design + 10-fixture corpus + fail-closed pins; MNE130 intact | `rfcs/0047-*.md`, `examples/source/recursion-rfc/*.mncs`, `crates/mncs-cli/tests/pressure_structural_recursion.rs`, ledger entry 0047 | requires architectural follow-up (checker + kernel evidence + runtime fuel) |
| CP-0012 (sequence payloads) | already repaired (bounded sequences in payloads) | regression evidence only | `pressure_enum_payloads.rs` over-bound rejection (`MNE171` stays closed) | fixed by prior language work |
| CP-0013 (`next` field) | `record R { next: u64 }` → MNP127 | contextual `next` in all field positions, step grammar unchanged, all backends | `examples/source/pressure-next-field.mncs` + corpus (6 cases × 5 backends), `crates/mncs-cli/tests/pressure_next_field.rs` | fixed in this tranche |

## What changed semantically (all purely additive)

- `bool == bool -> bool`, `bool != bool -> bool` (`BooleanCompare`);
  `!bool -> bool` (`BooleanNot`). Mixed-type and ordering rejections
  unchanged (`MNE119`/`MNE121`); `!` on non-bool is `MNE181`. New
  operation kinds flow through body validation (`MNB138`–`MNB141`),
  IR/SSA, both interpreters, scalar lowering, and all five executable
  backends. No new obligations (total ops, like `BooleanOp`).
- `next` accepted as a field/member name in record declarations and
  literals, finite payload declarations and constructions, `.`
  projections, and match payload bindings. Iteration-step grammar,
  carried-state naming, and `let` bindings are unchanged.
- Integer `match`: literal arms (including `-N` on signed types) plus
  exactly one required `_` default. Duplicates, missing/duplicated
  defaults, post-default arms (`MNE139`/`MNE140`), out-of-range literals
  (`MNE145`), and cross-domain arms (`MNE138`) fail closed at
  elaboration. Lowering reuses the finite-match branch chain
  (`IntegerCompare eq` + default); no new ops, no backend
  exhaustiveness logic. A bare `_` stays a variant pattern at parse
  time (finite types may declare a `_` variant — pinned by test).
- Iteration identities are live-scope unique with hygienic recorded ids
  (`i`, `i#2`, …). Nested overlap stays `MNE146`; sequential
  carried-state reuse stays `MNE110` (deliberate, untouched).

## What was deliberately not changed

- No general recursion, no `while`, no ceiling raises, no nominal
  identity loosening, no obligation suppression, no compiler-specific
  casing anywhere.
- `MNE130` is byte-for-byte intact. The ten RFC 0047 fixtures all
  produce exactly `MNE130` today (pinned).
- Single-loop iteration records keep bare source names, serialized
  variant-pattern arms are unchanged, and no existing diagnostic message
  was altered except `MNE136` (widened subject list) and `MNE146`
  (overlapping-scope wording) — both documented in the profile docs.

## Validation

- New tests: `pressure_bool_ops`, `pressure_next_field`,
  `pressure_scalar_match`, `pressure_iteration_identity` (cli, all five
  executable backends + negatives + determinism),
  `mncs-compiler/tests/iteration_identity.rs` (recorded-id unit pins +
  `MNB061` validation), `pressure_structural_recursion.rs` (fail-closed
  pins). Ledger count test updated 46 → 47.
- Re-verified green: `pressure_enum_payloads`,
  `pressure_raised_bounds`, `mncs-syntax` unit tests, `rfc_ledger`.
- Full workspace suites run before delivery (see final report).
