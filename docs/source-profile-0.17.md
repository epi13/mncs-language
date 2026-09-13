# Source Profile 0.17 — first-class test declarations

Status: **implemented, experimental (current)**. Profile 0.17 is additive
over Profile 0.16 and introduces the minimal language-owned testing surface:
the `test` declaration and compiler-owned static test inventory.

## Declaration role

`test` uses the ordinary MNCS declaration signature and body grammar:

```mncs
mncs 0.17;
module example;

test arithmetic() -> (result: i64) {
    return 42;
}
```

The declaration role is preserved in the AST and semantic model. A test still
obeys the same type, effect, capability, authority, budget, and failure rules
as any other executable declaration. `test` does not grant ambient authority
and does not introduce assertion, fixture, snapshot, generator, or closure
semantics.

The compiler assigns a stable declaration identity from the declaring module
and name, and emits a deterministic inventory containing the declaration
identity, body-sensitive test-case identity, function target, source span,
signature, effects, capabilities, and production subject fingerprint.

## Identity and build policy

Test declarations are verification artifacts about a subject. Their body is
not included in the production subject fingerprint. Changing a test changes
the test-case/experiment identity while leaving the production subject
identity and fingerprint unchanged, unless production declarations also
change.

Normal production compilation excludes test bodies from the executable
artifact. A test runner requests `mncs compile --include-tests`; a production
consumer can state the opposite policy explicitly with
`mncs compile --exclude-tests`. The compiler inventory is the only authority
for selecting first-class tests. Imported-module tests are not implicitly
selected by a root module inventory. Each test-bearing module is an explicit
inventory target, which keeps package policy and subject identity visible.

## Compatibility

Profiles 0.1–0.16 remain sealed. In Profile 0.17, `test` is a contextual
declaration token: it is a declaration at module scope and remains accepted in
ordinary name positions such as `mncs.test.assertions.v1` module paths. An
older source profile receives the precise `MNP220` diagnostic for a top-level
`test` declaration rather than silently treating it as a function. Profile
0.17 does not alter the meaning of earlier declarations.

`mncs-test` consumes the compiler inventory. Runtime test registration is not
duplicated in a per-test manifest; external experiment fixtures such as
compile-fail and diagnostic inputs remain manifest-addressable because their
source is intentionally invalid or is not an executable test declaration.
