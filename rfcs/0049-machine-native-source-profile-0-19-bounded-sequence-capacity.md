# RFC 0049: Machine-Native Source Profile 0.19 — bounded sequence capacity

- **Status:** Proposed
- **Target:** mncs-language Source Profile 0.19 (experimental)
- **Depends on:** RFC 0036, RFC 0044

## Summary

Add Source Profile 0.19 with a maximum exact sequence length and bounded-view
capacity of 4096 elements. Keep all earlier profile limits, iteration
ceilings, nesting limits, static work-product limits, and execution budgets
unchanged. Use the new capacity for process and application environment
values, which retain their existing 64 KiB aggregate bound.

## Motivation

Forge's declared process workflow forwards an allowlisted host `PATH` through
the typed MNCS process request. The observed path is 1,447 bytes. Profile
0.18's `EnvironmentEntry.value: [byte; up_to 1024]` rejects that request at
typed-value validation, before the external process starts. The host process
runtime already enforces at most 64 KiB for all environment keys and values
together; it has no 1,024-byte per-value restriction.

This is a language-owned bounded-sequence capability gap exposed by a real
execution contract. It is not a compiler semantic rejection of the child's
behavior, a Store failure, or evidence about the real compiler's execution
cost.

## Proposed semantics

1. Profile 0.19 admits exact and bounded sequences up to 4096 elements.
2. Profiles through 0.18 remain unchanged at a 1024-element ceiling.
3. Bounded iteration keeps the 1024 literal limit, two-level nesting cap,
   and existing static work-product limit. Runtime step budgets remain
   independent.
4. `mncs.std.process.v1` and `mncs.std.application.v1` move to Profile 0.19
   and allow each environment value up to 4096 bytes.
5. Process and application launch boundaries enforce at most 256 entries,
   keys of at most 64 bytes, values of at most 4096 bytes, and at most 64 KiB
   total key/value bytes. No value may be truncated to fit.
6. The 4096 bound is representational and finite. It does not promise that
   every backend realizes every such shape. Existing backend capability and
   verification reporting remains authoritative.

## Validation obligations

- Profile registry tests preserve historical ceilings and admit 0.19.
- Stage-0 rejects a 4096-bound type in Profile 0.18 and accepts it in 0.19.
- A retained process-effect execution passes a 1,447-byte value to a child;
  the child verifies the full value length and exits successfully.
- Existing process tests continue to enforce vector capacity and aggregate
  environment bounds.
- Forge's formerly failing receipt and declared-workflow tests pass with the
  candidate language provider selected.
- No claim is made about native Cranelift success, the real `flow.mncs`
  closure, Stage-1 executable creation, or Stage-2 succession.

## Trust and resource consequences

A 4096-element `i64` axis accounts for 32 KiB of element data, four times the
Profile 0.18 maximum. This remains below the unchanged 16 MiB composite-cell
arena, but each backend must continue to enforce its own memory and execution
envelope. The profile does not enlarge backend arenas, stack limits, process
limits, or execution budgets. The process environment as a whole remains
bounded to 64 KiB.

The Forge boundary still depends on the host-provided environment and the
explicit allowlist. This RFC does not grant ambient environment inheritance
or alter process authority.

## Alternatives

- Keeping 1024 requires splitting one logical environment value into multiple
  transport fields. That leaks chunk layout into the stable process record
  and requires every producer and consumer to reconstruct it.
- Raising the 0.18 limit retroactively would change a sealed profile's
  declared ceiling and invalidate profile-scoped evidence.
- Removing the 64 KiB aggregate limit would weaken a bounded resource
  contract without need.

## Compatibility and migration

Existing source profiles and their artifacts keep their prior semantics.
Modules that need values larger than 1024 bytes opt into Profile 0.19. The
stdlib process and application modules use 0.19; consumers may continue to
use older profiles when they only import and pass the nominal request types.
Consumers that construct such bounded values directly must use a profile that
admits the declared bound.
