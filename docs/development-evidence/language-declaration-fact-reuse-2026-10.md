# Language declaration fact reuse — 2026-10

This slice adds independently reusable Language evidence for per-function
declaration validation. It complements
`mncs.language-verification-fact/1`, which covers function bodies.

## Contract

- `mncs.language-declaration-verification-fact/1` identifies the function
  declaration, its exact declaration fingerprint, verifier and policy
  versions, Language profile, referenced assumption fingerprints, successful
  result, witness hash, invalidators, cached warnings, and refreshed source
  location identity.
- `mncs.language-declaration-fact-reuse/1` reports declaration checks actually
  reused or recomputed, including a reason for each recomputation.
- The fingerprint covers the complete function declaration with its body
  removed. This includes evidence annotations, generic parameters, failure
  policy, callable inputs and outputs, contracts, effects, capabilities, and
  assumption references.
- Referenced assumptions are separate dependencies. A changed assumption
  invalidates the declaration fact even when the function declaration is
  unchanged.
- Whole-program schema, module, finite-type, assumption uniqueness, and
  function identity checks still run on every snapshot. A cached declaration
  cannot hide a duplicate function or a new invalid local declaration.
- Warnings keep function-relative paths and are rebased to the current
  declaration slot. Source-location identity is refreshed separately from
  the semantic witness.
- Only successful declaration checks produce reusable facts. Invalid checks
  and interrupted validation do not produce a declaration fact.

## Validation

The focused fact tests cover JSON round trips, fresh source-location and
diagnostic-path updates, assumption invalidation, declaration and evidence
changes, tampered witnesses, profile isolation, and fresh duplicate-function
diagnostics. The linked frontend test confirms declaration facts are reused
for both the root and an imported module using each module's own source
profile.

Validated in the selected Language checkout:

- `cargo test -p mncs-model`: 235 passed.
- `cargo test -p mncs-compiler`: 124 passed across unit and integration tests.

This is correctness and avoided-work evidence. Total application-build
performance is measured separately in the Compiler integration, which also
accounts for parsing, resolution, identity construction, artifact generation,
and VM execution.

## Nominal record declaration slice — 2026-10-20

Record declarations now use the same versioned declaration-fact shape with
their own verifier and policy identities. A record fact binds its canonical
field layout and the transitive closure of referenced nominal record and
finite-type fingerprints. The closure prevents a nested type edit from leaving
an outer record fact falsely reusable; cycles are bounded by semantic identity.
Source locations remain outside the witness and are refreshed for the current
snapshot.

The reused check is the model validator's record-shape validation: a non-empty,
unique record identity and non-empty, unique field names with non-empty field
types. Source parsing, import resolution, and field-type resolution still run
for each changed source snapshot. Unknown or ambiguous nominal dependencies do
not receive a reusable record fact.

The focused record tests cover serialized fact reuse, location refresh,
verifier identity, changed profile, duplicate-field diagnostics, and direct
plus transitive nominal dependency invalidation. The linked frontend test
reuses three record facts after a comment-only imported-source edit and
recomputes all three after the leaf field type changes.

Validated in the selected Language checkout:

- `cargo test --locked -p mncs-model`: 244 passed.
- `cargo test --locked -p mncs-compiler`: 135 passed across unit and
  integration tests.
- `cargo test --locked -p mncs-cli --test conformance`: 6 passed.
- Changed Rust files passed `rustfmt --check`; `git diff --check` passed.

This establishes exact avoided model-validation work and dependency
invalidation. It does not measure an end-to-end application-build speedup;
Compiler cache transport and workload-level timing remain follow-on work.
