# MNCS agent execution contract (language repository)

Any capable agent entering this repository works under this contract. It is
the language-side mirror of the ecosystem contract bound in
mncs-actions (`AGENTS.md` there, enforced by its `test_agent_contract.py`):
MNCS first, pressure routed upstream, evidence honest.

## 1. MNCS-language is the implementation default

New implementation code is written in MNCS source (`.mncs`) wherever
technically possible: `library/` for reusable capability, `examples/` for
probes and conformance programs, `pressure/`-style experiment corpora where
the workflow calls for them. Do not escape to a host language (Rust included)
merely because MNCS lacks a capability; that lack is a language-pressure
event (section 3). Host code in `crates/` and `scripts/` is compiler,
runtime, tooling, and transport, not a substitute for expressible MNCS.

## 2. Prefer stdlib over repository-local substitutes

Before adding a helper, check `library/std/` and `library/core/`. If the
primitive belongs in the standard library, add it there with backend and
conformance coverage rather than duplicating it at a call site. Coherent,
reusable, machine-native abstractions only; do not inflate stdlib with
one-off conveniences.

## 3. Missing capability becomes a capability-gap artifact, never a workaround

When understood MNCS semantics fall outside the active profile, a backend
cannot realize valid semantics, or a required library/runtime operation is
absent, record the limitation as a content-addressed capability-gap artifact
(`crates/mncs-model/src/capability_gap.rs`,
`docs/capability-gap-artifacts.md`) instead of prose or a silent host-side
shim. Classify the obstruction honestly (parser, typing, semantic
validation, lowering, runtime, backend, library, verification) and route the
repair to the owning layer. Fix upstream, re-run the owning layer's suite,
then return to the original task and prove the new capability through
actual use.

## 4. Backends must not overclaim execution

`crates/mncs-codegen/src/lib.rs` distinguishes executable adapters from
external ones (`crates/mncs-codegen/src/external.rs`). The distinction is
load-bearing:

- `code emitted` is not `code executed`; `executed locally` is not
  `executed remotely`; `emulated` is not `physical`.
- Every external target carries an `execution_note` stating exactly what was
  proven (artifact generation plus which external validation, and which
  execution step is still missing). Keep those notes current: when new
  execution evidence arrives, update the note; until then, compile-only
  stays compile-only.
- Never claim execution from `file` output or successful emission alone.
  Execution evidence means the bytes ran and produced a checked result.

## 5. Respect source-profile scoping

`spec/` profiles define what MNCS means at each stage. Do not smuggle
semantics past the active profile, and do not harden an experiment into the
profile without going through the profile's own evolution path.

## 6. Validate like the ecosystem watches

Run `cargo test` for every touched crate and the `mncs` CLI conformance
examples that cover the change. Backend work must exercise
`examples/source/backend-conformance/` or extend it. Cross-backend
disagreement is pressure, not noise: classify it (ambiguous semantics,
compiler bug, stdlib bug, backend bug, runtime bug, unsupported capability,
intentional backend-specific behavior) and make intentional differences
explicit in semantics or capability declarations.
