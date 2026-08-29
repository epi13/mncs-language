# RFC 0045: machine-native bounded stateful execution traces

- **Status:** Accepted for the experimental execution corpus (2026-08-29)
- **Scope:** language-owned experiment and backend execution contracts

## Problem

The single-call `ExecutionRequest` contract cannot express an incremental
operation such as `init → chunk* → finish`. Encoding a stream into one large
argument loses the transition boundary that is the computation under test and
does not exercise persistent state across calls.

## Design

`ExecutionCorpus` schema `0.3` may contain `stateful_cases`. Each case is a
deterministically ordered, bounded list of `StatefulExecutionStep` values. A
step names one executable function and supplies either a literal logical value
or a `previous_result` reference to an earlier step's returned value. The
case declares `maximum_calls`; each call retains the existing positive
`step_budget`, and an optional aggregate step bound may be declared.

The state is value-threaded. A backend does not expose a session handle: the
language runner resolves a previous logical result into the next ordinary
`ExecutionRequest`. Backends may use pointers, cells, files, JIT values, or an
interpreter internally, but those physical representations are outside the
stateful contract.

Every successful transition receives a canonical logical return digest. The
case may opt into retaining complete returned values for selected intermediate
steps; the final returned value is retained. `StatefulExecutionResult` records
each call's target, status, step count, effects, failure, and logical return
digest, plus a trace identity covering the complete ordered trace, corpus
identity, program identity, backend identity, and artifact identity.

## Evidence semantics

Stateful execution is bounded observation, not a general runtime, persistence,
or conformance proof. `returned` means all declared calls returned; backend
runtime failures, budget exhaustion, invalid references, and unsupported
realizations remain explicit. Cross-backend comparison uses logical returned
values/digests, status, ordered step identity, and effects. It does not assume
identical physical state layouts. Missing or unusable trace evidence is
refused rather than promoted to `PASS`.

Artifact-only targets remain artifact-only. The current executable envelope is
research bytecode, portable WASM, C11, LLVM, and Cranelift; the RISC-V, eBPF,
and PTX adapters retain their existing refusal/capability declarations.

## Initial consumer

Atlas uses this contract for `atlas_model_init`, repeated
`atlas_model_chunk`, and `atlas_model_finish`. Its differential corpus keeps
malformed input, truncation, Unicode, ordering, unknown-field, capacity, and
empty-collection cases as real bounded traces. Agreement is reported as
bounded logical evidence and remains distinct from Atlas's production or
conformance decisions.
