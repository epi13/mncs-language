# Stage B3: nested bounded iteration and the counted index (P-012)

## Contract (Source Profile 0.11, additive)

- A bounded iteration body may contain one further bounded iteration
  (counted or traversal) with a distinct identity each level; a third
  level is refused (`MNE147`) on every profile, as is any nesting below
  0.11. Duplicate identities stay refused (`MNE146`).
- The counted-loop identity names the 0-based position (`bound -
  remaining`), typed u64. It binds as a counted index: readable like
  the traversal index but never discharging element projections.
- Each level keeps its bound (1..=32), region, obligations, and call
  closure; early `return`/`fail` keep function-wide meaning.

## Evidence

- `examples/source/stage-b3-nested-iteration.mncs` and
  `examples/execution/stage-b3-nested-corpus.json` (14 cases,
  independent Python oracle): 14/14 value agreement on all five
  executable backends with zero backend changes — nested regions lower
  correctly everywhere already.
- Workload: CRC-shaped two-level checksum (2 outer byte-steps each
  running 8 inner bit-steps of shift-xor update), composing B1
  operators with B3 nesting; single-level index observation
  (`index_sum_n`); 3x4 position grid.
- Committed test `crates/mncs-cli/tests/stage_b3_nested.rs`: per-backend
  agreement plus negative fixtures `examples/source/profile11/`
  (depth-three and 0.10-nesting → `MNE147`; 0.10 counted index →
  `MNE102`).
- Pre-existing 0.9/0.10 probes re-verified frozen: nested refusal and
  unbound index diagnostics unchanged below 0.11.
- Profile doc `docs/source-profile-0.11.md`.

## Pressure resolution

- P-012: RESOLVED for two-level counted nests and the bound counted
  index (the bit-steps-within-byte-steps CRC shape). Depth-three+ stays
  refused by design; offset-chasing dynamic iteration over runtime
  positions remains a separate, still-open shape.
