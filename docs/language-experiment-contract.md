# Language-owned compiler experiments

`mncs:language:experiment-definition:0.1` and
`mncs:language:experiment-result:0.1` make a bounded compiler/backend study a first-class language
artifact. They do not claim universal equivalence, conformance, or production fitness.

The definition binds the exact source identity and profile, compiler configuration, selected-SSA
realization request, execution corpus, required validators, requested result artifacts, invariants,
claimed bounded relation, parent experiment, and provenance. The realization request binds acceptable
backend identities, required intents, artifact kinds, validator capabilities, and fallback policy.

The result retains the definition, compiler study, backend capability manifest, target-lowering plan,
typed backend artifact, translation-validation results, per-case execution observations, unresolved
obligations, and one conservative status:

- `FAIL` if compilation, validation, or an execution observation fails;
- `UNKNOWN` if required evidence or execution remains unresolved;
- `PASS` only when every requested bounded observation passes.

`PASS` still means only the explicitly named finite experiment passed. It is not a general semantic
equivalence or MNCS conformance verdict.

## CLI lifecycle

```text
mncs experiment plan SOURCE --backend BACKEND --corpus CORPUS
mncs experiment run SOURCE --backend BACKEND --corpus CORPUS --output-dir DIR
mncs experiment inspect DIR/result.json
mncs experiment compare LEFT/result.json RIGHT/result.json
mncs experiment execute DIR/backend-artifact.json CORPUS
```

`plan` freezes the source-to-selected-SSA identity and realization requirements. `run` performs the
language-owned compilation, realization, validation, and bounded observation. `execute` accepts an
already frozen backend artifact and corpus and does not recompile; this is the intended narrow Fabric
runtime entry point. `inspect` rechecks content identities. `compare` localizes the earliest known
semantic/representation/realization divergence and reports bounded behavior agreement separately.

Forge may persist, list, and compare the exact result, but it may not replace these language-owned
status rules. Fabric may transport and execute the frozen artifact/corpus, but it may not select a
different source, SSA, backend, or validation policy silently.
