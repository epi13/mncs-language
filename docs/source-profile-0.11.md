# Source Profile 0.11 — nested bounded iteration and the counted index

Status: **implemented, experimental**. Profile 0.11 is additive over
Profiles 0.1–0.10. Older profiles retain their syntax, semantics, and
canonical fingerprints; in particular, nesting stays refused (`MNE147`)
and the counted-loop name stays unbound (`MNE102`) below 0.11.

## Two-level nesting

A bounded iteration body may contain one further bounded iteration
(counted or traversal), with a distinct iteration identity each level:

```mncs
iterate byte up_to 2 carrying crc: u32 = 4294967295 {
    next crc = checksum_word(crc, select(byte == 0, a, b));
}
```

where `checksum_word` itself iterates `bit up_to 8`. A third level is
refused (`MNE147`) on every profile, as is any nesting below 0.11.
Duplicate iteration identities stay refused (`MNE146`); shadowing an
enclosing identity by reuse is therefore impossible — nested loops name
their levels distinctly (`byte`/`bit`, not `step`/`step`).

Each level keeps its own bound (1..=32, `MNE142` otherwise), so two
levels compose to at most 1024 dynamic steps, inside the step budget.
Early `return` and `fail` inside either level keep their function-wide
meaning. Each level records its own region, obligations, and call
closure, exactly as a top-level iteration does.

## The counted index

The counted-loop identity names the 0-based position (`bound -
remaining`), typed u64 like the counter:

```mncs
iterate step up_to 8 carrying total: u64 = 0 {
    next total = total +% select(step < n, step, 0);
}
```

The counted position binds as a counted index: it reads like the
traversal index but never discharges element projections — only
traversal semantics prove positions in bounds, so indexing a sequence
by a counted position still requires its own bounds evidence.

## Evidence

`examples/source/stage-b3-nested-iteration.mncs`,
`examples/execution/stage-b3-nested-corpus.json` (14 cases, independent
Python oracle, 14/14 on all five executable backends), negative
fixtures `examples/source/profile11/` (`MNE147`/`MNE102`), and
`crates/mncs-cli/tests/stage_b3_nested.rs`.
