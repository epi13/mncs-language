# Bounded reference execution 0.1

This is an experimental transport and research schema for executing the currently supported
syntax-independent executable semantic body subset. It is not a production runtime specification.

## Request

`ExecutionRequest` has schema version `0.1`, a module/function target, typed arguments, a positive
`step_budget` no greater than the implementation's declared maximum, and an optional policy. Effects
default to `unsupported`; `record` is an explicit deterministic mock/reference policy.

## Result

`ExecutionResult` has schema version `0.1` and one of:

- `returned` with typed values;
- `runtime_failure` with a bounded reason and semantic operation identity where available;
- `unsupported` when the representation does not define deterministic reference semantics;
- `budget_exhausted` when the finite model budget is consumed; or
- `invalid_request` for an invalid schema, target, argument, or program.

Results contain program/function identities where available, a semantic program fingerprint, step
count, deterministic effect events, and a trace capped at 256 entries. Canonical output contains no
timestamps or ambient environment values.

## Supported body operations

The current body evaluator supports integer parameters, integer constants, integer `add`, `sub`, and
`mul` with represented arithmetic intent, wrapping bitwise `and`, `or`, and `xor`, typed integer
comparison predicates `eq`, `ne`, `lt`, `le`, `gt`, and `ge`, direct/conditional branches, block
arguments, returns, and cyclic CFG execution under the step budget. Signed and unsigned comparisons
use the declared integer type. Checked/trapping/widening overflow is an explicit runtime failure;
wrapping and saturating behavior is preserved. Bitwise operations currently require wrapping intent.

Effect operations require explicit recording policy and never perform external side effects. Runtime
checks are unsupported until their body representation carries an executable condition. Named types
other than `bool` and integer widths outside the bounded host-independent reference range are
unsupported.

## Corpus comparison

`ExecutionCorpus` and `ExecutionComparison` are versioned `0.1` JSON artifacts. The CLI command

```text
mncs compare-execution BASELINE CANDIDATE CORPUS
```

reports corpus size, matching/mismatching cases, subject identities/fingerprints, and the first
bounded mismatch with traces. `equivalent_over_corpus` means only that the declared finite corpus
matched. It is not universal equivalence, a compiler-correctness result, independent evaluation,
or production backend evidence.

The repository's bounded-sum fixture demonstrates a baseline, a structurally different equivalent
refactor, and a plausible regression whose comparison fails on ordinary and overflow-edge cases.

## SSA reference execution and cross-layer consistency

`SsaModule` schema `0.4` is independently interpreted by the SSA reference evaluator. Its result
schema is `0.1` and retains the target, semantic program/function identities, SSA module and HIR
fingerprints, values, bounded steps, effects, and a trace containing block, semantic operation, HIR,
and SSA identities. The evaluator supports the currently validated scalar SSA instructions,
block-parameter edges, returns, failure terminators, and bounded loops. It does not reconstruct the
body and invoke the body evaluator.

`LoweringExecutionComparison` schema `0.1` is emitted by:

```text
mncs check-lowering-execution PROGRAM CORPUS
```

Its statuses distinguish `consistent_over_corpus`, `mismatch_detected`, `unsupported`, and
`invalid_input`. `consistent_over_corpus` means only that body and SSA reference outcomes matched
over the declared finite corpus. It is not universal equivalence, formal compiler correctness,
backend validation, or independent evaluation. Shared low-level arithmetic helpers mean the two paths
are not fully independent implementations.

## Stateful execution traces

ExecutionCorpus schema `0.3` adds an optional `stateful_cases` collection. A
`StatefulExecutionCase` is a bounded ordered sequence of named calls. Each
`StatefulExecutionStep` has a target, ordinary typed literal arguments or
`previous_result` references to an earlier step, a per-call budget, and
optional expected status/return observations. The runner resolves references
to logical values and invokes the existing backend call ABI, so state is
retained across the sequence without exposing a backend-specific session
handle.

`StatefulExecutionResult` records every transition's status, effects, step
count, and canonical returned-value digest. Complete values are retained only
for selected observations and the final result, which keeps large streaming
states bounded while preserving a digest for every transition. Its trace
identity covers the ordered calls, corpus identity, program identity, backend,
and artifact. `returned` is reported only when every declared transition
returns; invalid references, failures, budget exhaustion, and unsupported
backends remain explicit.

`compare_stateful_results` compares status and logical transition observations
over the finite case set. This is bounded cross-backend evidence, not universal
equivalence, compiler correctness, persistence, or conformance.
