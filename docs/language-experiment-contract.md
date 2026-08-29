# Language-owned compiler experiments

`mncs:language:experiment-definition:0.1` and
`mncs:language:experiment-result:0.1` make a bounded compiler/backend study a first-class language
artifact. They do not claim universal equivalence, conformance, or production fitness.

Execution corpus 0.2 additively supports typed per-case expectations for execution status,
returned value, maximum backend step count, expected effect triples, and prohibition of unexpected
effects. Execution corpus 0.3 additionally supports bounded stateful cases: named calls can pass
logical returned values to later calls through `previous_result` references, with per-transition
budgets, intermediate observations, a final observation, and a whole-trace identity. Older 0.1
and 0.2 corpora retain their behavior. These are finite deterministic assertions, not arbitrary
property strings or a theorem language.

The definition binds the exact source identity and profile, compiler configuration, selected-SSA
realization request, execution corpus, required validators, requested result artifacts, invariants,
claimed bounded relation, parent experiment, and provenance. The realization request binds acceptable
backend identities, required intents, artifact kinds, validator capabilities, and fallback policy.

The result retains the definition, compiler study, backend capability manifest, target-lowering plan,
typed backend artifact, translation-validation results, per-case execution observations, bounded
property observations, stateful transition observations, unresolved obligations, and one conservative
status. An execution corpus may
bind an exact expected return for a case and finite-domain laws (`commutative`, `associative`,
`idempotent`, `neutral`, `absorbing`, and `preserved`). A property `FAIL` retains its first
deterministic counterexample:

- `FAIL` if compilation, validation, expected return, or property observation fails;
- `UNKNOWN` if required evidence or execution remains unresolved;
- `PASS` only when every requested bounded observation passes.

`PASS` still means only the explicitly named finite experiment passed. It is not a general semantic
equivalence or MNCS conformance verdict.

Profile 0.4 compilation may retain an `UNKNOWN` exact-resource-cost obligation while establishing
the static attempt ceiling. A bounded experiment that does not claim exact cost may PASS its
requested observations; that obligation remains visible in `compiler_study`. Other required
unresolved obligations keep the result `UNKNOWN`.

## CLI lifecycle

```text
mncs experiment plan SOURCE --backend BACKEND --corpus CORPUS
mncs experiment run SOURCE --backend BACKEND --corpus CORPUS --output-dir DIR
mncs experiment inspect DIR/result.json
mncs experiment compare LEFT/result.json RIGHT/result.json
mncs experiment execute DIR/backend-artifact.json CORPUS
mncs experiment execute DIR/backend-artifact.json CORPUS --baseline BASELINE/result.json --output-dir REPLICA
```

`plan` freezes the source-to-selected-SSA identity and realization requirements. `run` performs the
language-owned compilation, realization, validation, and bounded observation. `execute` accepts an
already frozen backend artifact and corpus and does not recompile; this is the intended narrow Fabric
runtime entry point. It executes stateful cases through the same adapter boundary while the
language-owned runner retains logical values, never backend session handles. `inspect` rechecks
content identities. `compare` localizes the earliest known
semantic/representation/realization divergence and reports bounded behavior agreement separately.

### Frozen replication (`execute --baseline`)

With `--baseline RESULT --output-dir DIR`, `execute` performs an identity-bound replication of a
recorded experiment realization instead of emitting anonymous case observations. Before executing
anything it fails closed unless all of the following hold:

- the baseline result's entire identity chain re-verifies (`experiment inspect` semantics);
- the supplied corpus is exactly equal to the corpus sealed inside the baseline definition;
- the supplied backend artifact is exactly equal to the frozen artifact recorded by the baseline.

Any mismatch aborts with exit code 2 and writes no outputs; a mutated or substituted realization is
never silently rebuilt or replaced. On success the command executes every corpus case and every
bounded property law against the frozen artifact, seals a new `LanguageExperimentResult` that
inherits the definition, compiler study, capability manifest, realization plan, artifact, and
translation validations verbatim from the baseline, and writes `replicated-result.json` plus
`replicated-family-reference.json`. Because language identities are content-addressed, a replication
that observes identical bounded behavior yields the identical result identity as the baseline;
behavioral divergence changes the identity while the Fabric layer separately records which worker
and execution attempt produced it. The printed summary carries both result identities, the
definition/backend/artifact identities, per-case/per-property agreement counts, a bounded behavior
agreement flag, and the full experiment comparison. Stateful replication includes the complete
bounded transition observations in the sealed result. Exit code 1 reports `FAIL` status or observed
baseline disagreement.

The compile-time evidence inherited into a replicated result (study fingerprints, translation
validations) describes the original realization of the frozen artifact; the fresh case and property
observations describe this specific execution. Runtime facts such as worker identity remain owned by
Fabric execution records, never by the language result.

When an output directory is requested, `family-reference.json` provides an observational
`LanguageExperimentResult` reference with exact source/profile, compiler/pipeline,
semantic/HIR/SSA, selected-SSA, realization, backend/artifact, corpus, validator, result/status,
and unresolved-obligation identities. Commons remains the owner of generic Family Record storage.

Forge may persist, list, and compare the exact result, but it may not replace these language-owned
status rules. Fabric may transport and execute the frozen artifact/corpus, but it may not select a
different source, SSA, backend, or validation policy silently.
