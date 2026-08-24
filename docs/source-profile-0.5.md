# Source Profile 0.5 — logical record values

Status: **implemented, experimental** (2026-08-23). Profile 0.5 is a language
envelope; it is not roadmap milestone 0.5.

## What 0.5 adds on top of 0.4

Profile 0.5 is cumulative with 0.4 (bounded iteration keeps working) and adds
nominal **record types**: labeled products with stable semantic identity.

```text
mncs 0.5;

module examples.demo;

record Reading { celsius: i32, milli: i32 }

fn main(celsius: i32) -> (result: i32) {
    let base: Reading = Reading { celsius: celsius, milli: 3 };
    let updated: Reading = Reading { ..base, milli: 21 };
    return updated.celsius + updated.milli * 2;
}
```

* Declaration — `record Name { field: Type, ... }`. Field types may be
  supported scalars, declared finite types, or other declared records
  (nesting), regardless of declaration order.
* Canonical field identity — `record_type_id(module, name, fields)` hashes the
  sorted `name:type;` pairs. Two declarations naming the same fields in a
  different source order denote one logical type. Physical layout is never
  part of record identity.
* Construction — `Name { field: expr, ... }` requires exact coverage of the
  declared fields (no missing, no invented, no duplicate assignment).
* Functional update — `Name { ..base, field: expr }` reuses `base` for every
  field not explicitly assigned. The base expression must have exactly the
  named record type.
* Projection — `expr.field` requires a value of a declared record type and
  names a declared field.
* Disambiguation — `x.selector` is syntactically shared with finite variant
  constructors (`Color.RED`). A name bound in the lexical environment always
  means projection; otherwise the selector resolves against module-level
  finite types.

## Explicit non-goals for 0.5

* Record equality/comparison operators are **not defined**. Comparing records
  with `==` is an elaboration error rather than a silently structural
  decision.
* Records are immutable; there is no field assignment.
* Records cannot cross function boundaries as parameters/results on backends
  whose ABI is scalar-only (see below); there is no record polymorphism.

## Compiler realization

Records flow through every stage as first-class operations:

| Stage | Representation |
| --- | --- |
| body validation | `RecordConstruct` / `RecordProject` with canonical operand order (diagnostics MNB048–MNB057) |
| HIR | `IrOperationKind::RecordConstruct` / `RecordProject`, `IrType::Record` |
| SSA | same instructions; `SsaModule.record_types` carries logical declarations so backends never re-read source |
| reference execution | `ExecutionValue::Record` keeps fields in canonical sorted order |
| identity | record type identities join program fingerprints, so any record change changes downstream artifact identities |

## Backend matrix for records

| Realization | Records | How |
| --- | --- | --- |
| research bytecode | SUPPORTED | executes logical SSA directly |
| portable WASM MVP | SUPPORTED (intra-function only) | forwarding realization: construction materializes nothing and projection forwards the field operand's local slot. Record-typed parameters, results, or block parameters fail closed (`CGN302`) instead of being silently flattened. |
| LLVM IR / C11 / Cranelift | UNSUPPORTED | `scalar_ty` rejects record values with an explicit envelope diagnostic; manifests declare `record_values` unsupported |

A backend limitation never redefines the language: `SUPPORTED`,
`UNSUPPORTED`, and `UNKNOWN` remain distinct per capability manifest.

## Diagnostics added

Parser: MNP120–MNP133 (profile gate, declaration/literal/projection syntax).
Elaboration: MNE150–MNE163 (duplicate/colliding record names, duplicate
fields, unknown field types, literal coverage/type errors, projection errors).
Body validation: MNB048–MNB057.

## Fixtures

* `examples/source/profile05-record-values.mncs` — positive fixture;
  executable form `examples/executable/record-values.mncs.json`; corpus
  `examples/execution/profile05-record-values-corpus.json` demonstrates
  cross-backend agreement including checked-multiply trap behavior.
* `examples/source/profile05-invalid-duplicate-field.mncs` (MNE152),
  `examples/source/profile05-invalid-literal-unknown-field.mncs` (MNE156),
  `examples/source/profile05-records-in-profile04.mncs` (MNP120).
