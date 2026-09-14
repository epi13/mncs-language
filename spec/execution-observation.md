# Bounded execution observability

`mncs.execution-observation/1` is the language/runtime-owned stream used by
debuggers and other evidence consumers. It is composed with, rather than
folded into, the unchanged `execution-result/0.1` semantic result:

```text
semantic request + program
    -> execution-result/0.1       (the execution verdict)
    -> execution-observation/1    (bounded explanatory facts)
```

The observation policy is not part of the semantic execution request. A
bounded and a diagnostic observation of the same request therefore share the
same `execution_identity`, while their policy/completeness/content-derived
observation identities may differ. Observation cannot turn a runtime result
into a different program or verdict.

The stream is bounded by the runtime. It records typed values with explicit
full/truncated/digest-only/unavailable capture, execution-scoped value
identities and versions, execution-scoped frames, operation input/output
references, and effect invocation/result lineage. It does not claim scheduler
control, external-state replay, or deterministic replay merely because an
effect was recorded.

`mncs.execution-source-map/1` is the compiler-owned source join. It maps
semantic operation identities to exact source spans and marks synthetic
operations honestly. HIR and SSA artifacts retain their existing
`semantic_identity` correspondences, so consumers join source → semantic →
HIR/SSA without matching text or operation order. Runtime events carry the
semantic operation identity; they do not duplicate source coordinates.

The reference CLI exposes the contract with:

```text
mncs observe PROGRAM REQUEST --capture none|failure-only|selected|bounded|diagnostic
```

`none` and a passing `failure-only` run intentionally produce no retained
events or values. `max-events`, `max-values`, and `max-value-bytes` are
defensive bounds and truncation is reported as data, never silently treated as
complete evidence.
