# Type architecture campaign — 2026-09-12

## Scope

Incremental pass over the MNCS type pipeline toward the
`TypeSyntax → SemanticType → TypeIdentity` direction, without redesigning the
language. Preserves all valid semantics and profile compatibility; every
change is fail-closed.

Branch: `feat/type-architecture-2026-09-12` (from `origin/main` post-ingest-merge).

## What changed

### 1. Explicit semantic `Bool` (`BodyType::Bool`)

`BodyType::Named("bool")` served as the boolean type across parsing,
elaboration, validation, both interpreters, and all four backends. It is now
an explicit variant:

- `from_semantic_name("bool")` returns `Bool`; nothing new mints the legacy spelling.
- `semantic_name()` / `canonical_identity()` yield `"bool"` for both `Bool`
  and legacy `Named("bool")`, so old and new fingerprints agree.
- `is_boolean()` admits `Bool`, legacy `Named("bool")`, and the historical
  `u1` value-boundary admission; `is_strict_boolean()` is `Bool` only.
- `normalize_legacy_bool()` / `normalized_legacy_bool()` migrate old artifacts recursively.
- Executable-body schema `0.2 → 0.3`; validation accepts both (`MNB001`
  only fires otherwise). Emitters produce `0.3`.

Touched: `mncs-model` (body, cfg, graph, ssa, value_contract, execution,
ssa_execution), `mncs-compiler` (frontend), `mncs-codegen` (lib, lower,
support, composite, scalar).

### 2. Resolved-type invariant (`MNB122`)

New validation: after elaboration, executable IR must contain only
fully-known semantic types or deliberate `GenericParam` variables — never
arbitrary unresolved names (including the `"invalid"` error poison).
`BodyType::has_unresolved_named()` is the single predicate; `execution`'s
`type_has_unresolved_name` and the frontend sequence gates delegate to it.
`SSA045` at the lowering boundary now flags any non-bool `Named`, not just
generic-name leaks.

### 3. Type relations made explicit

- `BodyType::same_type()` — structural equality with legacy-bool tolerance
  (nominals compare by `SemanticId`, never layout or short name).
- `BodyType::classify_compatibility()` / `is_compatible_with()` /
  `TypeRelation::{Equal, ViewWidening, Incompatible}` — equality vs static
  view-capacity widening (`[T; up_to M]` satisfies `[T; up_to N]` iff
  `M <= N`, same element; exact bounds, element mismatches, and symbolic
  generic capacities refuse). No implicit coercion added.
- `body_view_widening` delegates to the classifier; parameter, return, and
  call-argument/result checks use `same_type` so pre-0.3 artifacts validate.

### 4. Nominal-element duplication removed

`body_type_for_function_value` had two near-identical sequence-element
rehydration blocks (concrete vs generic-bound paths). Both now share
`resolve_sequence_element` (generic params → nominal identity by identity or
short name → spelling as-is). No behavior change; one place for nominal
identity to diverge removed.

### 5. Syntax vs semantics named (`TypeSyntax`)

New `TypeSyntax` wrapper documents the layer split: `Program`
`value_type`/`field_type` strings are syntax, not meaning. Resolve once via
`resolve_against(program)` (program-aware) or `resolve_abi_only()`
(no nominals), then carry `BodyType`. No schema change; adoption is
incremental.

### 6. ABI boundary choke point

- `BackendValueContract::scalar_from_body_type()` — authoritative scalar
  constructor from a resolved type.
- `BackendValueContract::logical_type(program)` — single resolution point
  for every contract variant (nominals by identity; sequence/view/vector
  elements via `from_program` so nested nominals cannot go opaque).
- `support::value_contract_for` now resolves sequences via `from_program`
  instead of `from_semantic_name`. Wire spellings unchanged (short names),
  so artifacts stay compatible.

## Pressures discovered

### TYPE-P-001: sequence-of-nominal ABI element naming (language deficiency, open)

`BackendValueContract::Sequence/View.element` carries the short element
spelling (`"Point"`), not the nominal identity. Two modules defining `Point`
produce the same element spelling with different identities; the composites
map is keyed by both name and identity, but the sequence contract itself does
not say which one it means. `logical_type(program)` resolves against the
linked program, so in-practice linked artifacts are unambiguous — but an
artifact fragment without its program is. Status: documented, not changed
(wire change would break ABI compat). Evidence: this file;
`crates/mncs-codegen/src/support.rs:value_contract_for`.

### TYPE-P-002: `IrType::Named` still stringly (known weakness, open)

HIR/SSA `IrType::Named(String)` carries scalar spellings (`"bool"`, `"i32"`,
`"byte"`, sequence spellings) as strings between Body and lowering. `Bool`
fixes the semantic layer; the IR transport still reparses via
`from_semantic_name` at `scalar.rs`, `lower.rs:wasm_type`, and
`ssa_execution::body_type`. Status: documented; a typed `IrScalar` is future
work, deliberately not done here to bound schema churn.

### TYPE-P-003: view-result `Named` tolerance (implementation defect, mitigated)

`MNB094/MNB095` paths excused result mismatches when `result.ty` was any
`Named`. With `MNB122` every such case is now also reported as unresolved,
so the tolerance no longer hides a soundness hole — but the tolerance code
itself remains. Status: mitigated; full removal needs a corpus run proving
no valid program depends on it.

## Tests

Committed (durable):

- `crates/mncs-model/src/body.rs` unit tests: `bool_is_semantic_not_named`,
  `legacy_bool_normalizes_recursively`,
  `type_relation_distinguishes_equality_from_widening`,
  `nominal_identities_do_not_collapse_by_structure`,
  `unresolved_named_is_an_invariant_violation`,
  `bool_body_validates_without_unresolved_names`
  (plus pre-existing `sequence_spellings_keep_bool_and_nominal_elements`,
  `from_program_resolves_nested_record_sequence_elements`).
- `crates/mncs-compiler/tests/type_architecture.rs` (7 tests): bool
  elaboration identity, nested imported nominal sequences, cross-module
  structurally-identical records, generic bool specialization, records of
  bounded sequences, byte-vs-u8 refusal, unknown-name rejection.

Observed: `cargo test -p mncs-model` 184 passed; `cargo test -p mncs-compiler`
9 passed (pre-existing suite); new `type_architecture` 7 passed. Broader
workspace/conformance and backend-specific suites were not run in this pass
(time-boxed); they are the next gate before merge.

## Remaining weaknesses (explicit)

- `Value.value_type` / `RecordField.field_type` remain `String` by schema
  design (syntax layer); `TypeSyntax` names the discipline but call sites
  migrate incrementally.
- `IrType::Named` stringly transport (TYPE-P-002).
- Sequence-of-nominal ABI element short names (TYPE-P-001).
- `MNB094/MNB095` Named tolerance code retained (TYPE-P-003).
- `u1` integer still admitted as boolean at value boundaries (historical;
  preserved intentionally, not collapsed into `Bool`).

## Recommended next campaign

Finish the IR transport: introduce a typed scalar/sequence-element
representation for `IrType` (or collapse the `BodyType → IrType::Named →
BodyType` round-trip), remove the `MNB094/MNB095` tolerance after a full
corpus run, and resolve TYPE-P-001 by carrying nominal identity in
sequence/view ABI contracts (versioned wire change with compat path).

---

# Type-transport campaign (same date, branch `feat/type-transport-2026-09-12`)

## Scope

Close TYPE-P-001/002/003 by finishing semantic type transport through
HIR/SSA/ABI: `BodyType` is now the single resolved type in persisted IR,
ABI contracts carry `AbiTypeRef(BodyType)` instead of bare strings, and
every nominal reader resolves by identity, failing closed on ambiguity.
No traits, subtyping, coercion, or dependent types; valid semantics and
compat preserved.

## What changed

- **TYPE-P-002 closed**: `IrType::Named` stringly transport removed.
  HIR (`HighLevelIr`) and SSA (`SsaModule`) carry `BodyType` directly;
  both schemas versioned `0.4 → 0.5` with deterministic
  `normalize_legacy_*` upgrades for pre-0.5 documents
  (`mncs-model` `ir.rs`, `ssa.rs`, `ssa_execution.rs`, `lib.rs`).
- **Backend artifact `0.4 → 0.5`**: contract element/field types are
  `AbiTypeRef(BodyType)`, which serializes transparently as the resolved
  type and deserializes from the typed shape or a legacy bare spelling.
  `BackendArtifact::normalize_legacy_contracts` upgrades pre-0.5
  artifacts deterministically and reseals the identity; `mncs-embed`
  normalizes after the identity check passes and re-validates before
  admission, so tampering is still refused on the original bytes.
- **TYPE-P-003 closed**: the `MNB094/MNB095` broad-`Named` tolerance no
  longer excuses anything — the committed test
  `view_result_with_unresolved_name_reports_mismatch_and_invariant`
  requires both the mismatch and `MNB122` to fire.
- **TYPE-P-001 readers hardened (this pass)**: every nominal
  contract/map lookup that fell back to the short-name key now resolves
  by identity only, via the single authority
  `BackendValueContract::find_nominal_contract` (identity key, then an
  identity scan for hand-built maps) and
  `resolve_nominal_spelling` (unique short names resolve, shared names
  refuse). Touched: `mncs-codegen` `composite.rs` (`resolve_contract`,
  field decode), `support.rs` (`check_declared_type`, contract-value
  check), `lib.rs` (`named_marshal`, including the legacy-`Named` arm
  which previously inherited whichever entry won the name slot).
- **Single-authority bare-spelling parse (this pass, incident-driven)**:
  `AbiTypeRef::visit_str` now resolves through
  `BodyType::from_semantic_name` instead of wrapping every bare string
  in `Named`. See incident below.

## Incident: scalar elements broke frozen execution

Staged testing caught two failures in
`mncs-cli/tests/host_generic_entrypoints.rs`
(`frozen_artifacts_serve_generic_entrypoints`,
`repeated_instantiation_calls_agree_without_cross_talk`): frozen
`experiment execute` refused fresh artifacts with
`backend artifact identity is stale or laundered`. Bisect (`git stash`
→ test passes at committed `9002027`) proved the cause was uncommitted
campaign work, and a round-trip probe pinned it: a `View` element
`Byte` serializes as the bare string `"byte"` (derived `Serialize` on
the `BodyType` unit variant) but deserialized as `Named("byte")`, so
the reloaded `function_value_contracts` differed from the sealed
material and the artifact identity no longer validated. Fix: parse bare
spellings with the one ABI spelling authority (`from_semantic_name` —
scalars denote scalars, anything else stays a `Named` carrier for
composite rehydration). Both CLI tests pass after the fix.

## Tests (all observed this pass)

- `cargo test -p mncs-model --lib`: 186 passed, including new
  `compiler::tests::scalar_element_contracts_survive_a_serde_round_trip_with_identity_intact`
  (Byte view element → bare `"byte"` wire form → identical reload,
  identity valid).
- `cargo test -p mncs-codegen --lib`: 61 passed, including new
  `typed_decode_prefers_identity_over_same_short_name` (same-named
  impostor under the short-name key: typed decode fails closed with a
  no-contract error instead of decoding the impostor; true contract
  under the identity key decodes, impostor ignored).
- `cargo test -p mncs-cli --test host_generic_entrypoints`: 10/10
  (both incident tests green).
- `cargo test --workspace` (plus per-package reruns past the one
  failure cargo stops at): 87 `mncs-cli` targets ok, 24 targets ok
  across `mncs-model`/`mncs-compiler`/`mncs-codegen`/`mncs-embed`/
  `mncs-conformance`/`mncs-syntax`/`mncs-translation-check`, zero
  failures — except one pre-existing, out-of-scope failure —
  `library_core::ravel_snapshot_is_canonically_identical_to_upstream`
  expects `1cc17f37…` but canonicalization yields `346e6342…`. Proven
  unrelated: it fails identically at committed `9002027` with this
  branch's work stashed, and the `origin/main` (`e72d916`) binary
  produces the same `346e6342…` output for the untouched snapshot file
  (last touched pre-campaign). The drift predates this campaign; the
  hash was recorded by an earlier tranche and never rotated. Left
  untouched deliberately — rotating a witness hash belongs with the
  delta verification, not this branch.
- `cargo fmt --all -- --check` clean;
  `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Remaining weaknesses (explicit)

- `composite_value_contracts` is NOT covered by the backend-artifact
  identity material (`BackendArtifactMaterial` omits it), so two
  artifacts differing only in composite contracts share an identity.
  Mitigated in practice today because the compiled bytes (which embody
  the contracts) ARE covered via `bytes_sha256`; still, any future
  post-hoc contract attachment should reseal explicitly. Adding the map
  to the material would churn every artifact fingerprint — a deliberate
  versioning decision, not taken here.
- `TypeSyntax` strings remain at the persisted `Program` schema (syntax
  layer by design); `Value.value_type` / `RecordField.field_type`
  likewise.
- Legacy `0.4` compat paths (`normalize_legacy_*`,
  `AbiTypeRef` string arm, `Named("bool")` aliases) are load-bearing
  for old artifacts; removal needs a full-corpus migration window.
- A nominal literally named like a scalar spelling (`"byte"`) resolves
  as the scalar in ABI-only positions — consistent with every other ABI
  classifier (`from_semantic_name` is used exactly once per position),
  but worth a reserved-name rule if nominal namespaces ever widen.

## Performance / artifact impact

No meaningful change observed: contract payloads carry the same type
facts in typed rather than string form (comparable serialized size);
identity recomputation is unchanged work; compile cost and runtime
unmeasured but no new passes were added (normalization runs only on
legacy load).

## Recommended next campaign

Attach `composite_value_contracts` to the artifact identity material as
a deliberate versioned change (with the fingerprint-churn migration it
implies); then pursue the consumer work named in the campaign brief
(`mncs-compiler`, `mncs-numerics`, `mncs-ingest`, `mncs-store`) on the
now-stable typed transport.
