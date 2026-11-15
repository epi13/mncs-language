# Testing MNCS source

`mncs-test` is the canonical provider for executable MNCS source tests. The
language owns the declaration and its semantic identity; the compiler emits a
deterministic test inventory; `mncs-test` executes that inventory and projects
the observations into the RFC 0034 experiment model.

## First-class declarations

Profile 0.17 introduces `test` as a declaration role while retaining the
ordinary MNCS function signature and body grammar:

```mncs
mncs 0.17;
module example;

use mncs.test.assertions;
use mncs.test.suite;

test arithmetic() -> (result: TestResult) {
    return from_assertion(equals_i64(42, 19 +% 23, 1001));
}
```

The declaration is subject to the same type, effect, capability, authority,
and budget rules as any other MNCS computation. `test` does not grant ambient
authority or add assertion/fixture/closure keywords.

The compiler's `mncs test-inventory source.mncs` command is authoritative for
test names, source spans, declaration identities, body-sensitive test-case
identities, and production-subject identity. A normal runtime manifest names
the source/module and policy only; it does not repeat each test entry or build
a handwritten suite.

The `mncs impact` command is the bounded semantic-impact authority for the
selective development loop. Given changed semantic roots, it emits
`mncs.semantic-impact/1`: graph identity, exact bounded nodes/edges, direct
dependents, compiler-owned test-case identities, typed risk flags, and explicit
completeness/limitations. Unknown roots or truncation remain visible so Ravel
can escalate instead of claiming a narrow proof. Ravel joins this artifact
with `mncs test-inventory` to produce the digest-bound verification plan
consumed by mncs-test.

## Production and verification artifacts

First-class tests are verification artifacts about a subject. Normal
`mncs compile` excludes test declarations from the production executable.
`mncs-test` requests an explicit test artifact with `--include-tests`. A test
body edit changes the test-case/experiment identity while leaving the
production subject fingerprint unchanged when production declarations are
unchanged.

Compile-pass, compile-fail, diagnostic, malformed-source, and profile-refusal
cases remain external experiment fixtures because their compiler input is
invalid or is not an executable first-class test. They are still evaluated by
`mncs-test` using structured diagnostic code, stage, severity, and span fields.

## Running the dogfood suite

From a checkout with `mncs-test` available, run the representative language
tests with the compiler and both MNCS library roots:

```text
mncs-test run --manifest mncs-test.toml \
  --library /path/to/mncs-test/native \
  --mncs /path/to/mncs-language/target/debug/mncs \
  --embed-library /path/to/mncs-language/target/debug/libmncs_embed.so
```

The normal path compiles once, opens one retained `mncs-embed` session, and
batch-invokes the discovered tests, then folds the returned native
`TestResult` values through `mncs.test.suite::empty` and `observe` in the
same session. Importing the suite module makes that native aggregation
dependency explicit; it is not a registration table. The subprocess path is
retained only as a transport fallback when the embed library is unavailable.

Results remain epistemic: `PASS` is bounded evidence for the recorded cases,
`FAIL` is an observed counterexample or execution failure, and `UNKNOWN` is
preserved for unsupported capabilities, budgets, timeouts, or infrastructure
uncertainty. A finite passing run is not a universal proof of the subject.

## Retaining the reference oracle

Host integration tests can retain research-bytecode compilation while replaying
the existing body/SSA/backend comparison on each new stateless corpus.
First admit the independently emitted `CompilationEvidenceBundle` with
`Session::bind_reference` or `mncs_session_bind_reference(handle, evidence_json)`.
The payload's semantic and SSA fingerprints must match the compiler's selected
input and intermediate references, and its backend bytes must match the emitted
backend reference. A self-consistent backend payload cannot serve as its own
reference authority. The C admission response is the evidence identity; NULL
reports invalid evidence. `mncs compile SOURCE --emit backend,evidence --quiet
--omit-result --output-dir DIR` supplies the existing artifacts without
redundant result output. Keep their source/compiler admission binding together.

`mncs_embed::Session::compare_reference(corpus, grants)` and the stable C ABI
`mncs_session_compare_reference(handle, corpus_json, grants_json)` return the
ordinary `LayeredExecutionComparison` document. The C response is released with
`mncs_response_free`; malformed inputs and unavailable reference preparation
return NULL with `mncs_last_error`. An unsupported backend or stateful corpus
is refused explicitly. Artifact admission still verifies identity and digest.

The session retains immutable body and SSA preparation with independent
runtime contexts for each validation layer and for real execution. Targets, arguments,
generic selections, budgets and host grants remain request-specific. Comparison
observes effects; the separate call/call-batch operation realizes them. No
execution result is cached. Properties, expected values and full experiment
status remain responsibilities of their existing experiment/test consumers;
this operation reports bounded layer agreement only. Store's frozen filesystem
oracle uses this surface without substituting a compiled backend for research
verification or rewriting its lifecycle and fault-injection tests.
