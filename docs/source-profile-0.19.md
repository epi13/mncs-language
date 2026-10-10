# Source Profile 0.19 — bounded sequence capacity 4096

Status: **implemented, experimental (current)**. Profile 0.19 is additive
over Profile 0.18. It raises the maximum exact sequence length and bounded
view capacity from 1024 to 4096 elements. All earlier profiles retain their
existing limits.

## Motivation

The typed process and application-entry contracts represent environment
values as bounded byte views. A real Forge invocation carried a 1,447-byte
`PATH`, while the process runtime already admitted up to 64 KiB of aggregate
environment data. Profile 0.18 could express only 1,024 bytes in one value,
so the typed boundary rejected this otherwise-valid request before starting
the child.

The 4096-byte per-value bound accommodates that measured request with a
finite, explicit limit. Process and application launchers continue to enforce
the 64 KiB aggregate environment limit and 256-entry count. Values are
preserved exactly; they are not truncated.

## Semantics

- `[E; N]` and `[E; up_to M]` admit `0 <= N, M <= 4096`.
- Sequence values remain immutable. Capacity and runtime length remain
  semantic facts, and all existing bounds, identity, indexing, conversion,
  and view rules are unchanged.
- The literal bounded-iteration ceiling remains 1024, with the existing
  nesting and static work-product limits. Traversal of a sequence remains
  bounded by its declared capacity and the caller's execution budget.
- The model's absolute sequence ceiling is 4096. This does not raise backend
  arenas, VM stacks, execution-step budgets, or process limits.
- A 4096-element `i64` axis represents at most 32 KiB of element data. A
  backend that cannot realize a particular valid shape must report that
  limitation under its existing capability contract; this profile does not
  claim universal backend execution.

## Evidence obligations

Implementations should include:

- a Profile 0.19 source case accepting a 4096-capacity type;
- a Profile 0.18 case rejecting a capacity above 1024;
- process-boundary coverage showing a 1,447-byte environment value arrives
  intact in the child;
- preservation of the 64 KiB aggregate process-environment limit;
- backend-specific tests for any target that claims execution of larger
  sequence values.

The profile itself does not establish Stage-1 compiler execution or universal
backend conformance.
