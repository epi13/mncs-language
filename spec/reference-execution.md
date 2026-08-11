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

The current evaluator supports integer parameters, integer constants, integer `add` with the
represented arithmetic intent, typed integer comparison predicates `eq`, `ne`, `lt`, `le`, `gt`, and
`ge`, direct/conditional branches, block arguments, returns, and cyclic CFG execution under the step
budget. Signed and unsigned comparisons use the declared integer type. Checked/trapping/widening
overflow is an explicit runtime failure; wrapping and saturating behavior is preserved.

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
