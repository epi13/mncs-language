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
batch-invokes the discovered tests. The subprocess path is retained only as a
transport fallback when the embed library is unavailable.

Results remain epistemic: `PASS` is bounded evidence for the recorded cases,
`FAIL` is an observed counterexample or execution failure, and `UNKNOWN` is
preserved for unsupported capabilities, budgets, timeouts, or infrastructure
uncertainty. A finite passing run is not a universal proof of the subject.
