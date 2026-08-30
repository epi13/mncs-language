# MNEL one-step pressure evidence — 2026-08

## Scope

This record documents the MNCS-language changes exposed by the first MNCS-native
one-step specialist slice in the sibling MNEL repository. It covers bounded
semantic fixtures and backend execution, not general ML support, conformance, or
performance.

## Shared-layer closures

- `mncs.core.random.v1.split` now derives two domain-separated child roots from
  the same root. The contract guarantees reproducible, distinct roots under the
  defined wrapping arithmetic; it deliberately makes no statistical-independence
  claim. Consumers must bind finite draw budgets and partition policy in their
  own experiment identity when that matters.
- Unsigned i32/i64 constants preserve their declared bit patterns through source
  lowering, Cranelift constants, and LLVM constants. This matters for u64 lineage
  fingerprints and high-bit random-derived values.
- Composite decoding distinguishes a finite type whose variants have no payloads
  from a payload-bearing boxed composite. Bare finite values therefore use the
  discriminant representation even when the type-level payload map contains all
  declared variants.
- Cranelift declared host calls with more than six scalar parameters use a
  generated local wrapper with an argument buffer, status pointer, and value
  pointer. The wrapper removes an accidental direct-call arity ceiling while
  preserving the existing host-call result protocol.

## Regression fixtures

The focused fixtures are:

- `examples/source/profile06-random-split.mncs` with
  `examples/execution/profile06-random-split-corpus.json`, which checks
  reproducibility and distinct child streams;
- `examples/source/profile08-numeric-boundary.mncs` with
  `examples/execution/profile08-numeric-boundary-corpus.json`, which checks
  unsigned boundary preservation and an eight-scalar call through the wide-call
  path;
- the composite unit test
  `mncs-codegen::composite::tests::status_pair_record_encodes_to_the_canonical_layout`,
  which covers payload-free finite fields.

Both executable fixture tests agree across research bytecode, portable WASM MVP,
LLVM IR, C11, and Cranelift. The focused composite tests pass. These observations
are finite test evidence; they do not establish universal backend equivalence or
production readiness.

## Relationship to MNEL evidence

The sibling MNEL record
`docs/mncs-reconstruction/evidence/mnel-native-one-step-study.json` records the
stateful `train_artifact -> infer -> decision_code` corpus for the native
fixed-point specialist across the same five executable backends. Its compiler
status remains `UNKNOWN` where checked-arithmetic obligations are retained, while
the declared stateful executions are reported separately as `PASS`. The student
remains diagnostic-only and its bounded u64 lineage fields are not cryptographic
identities.
