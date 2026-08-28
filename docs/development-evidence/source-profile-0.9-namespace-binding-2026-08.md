# Source Profile 0.9 namespace/binding tranche evidence — 2026-08

Status: **implemented / exercised, experimental**.

This tranche follows the Profile 0.6–0.8 source work and addresses the next
stdlib/compiler pressure point: multiple modules may export the same final
name without flattening the linked program into an order-dependent map. The
compiler now preserves declaring-module identity, supports aliases and
qualified nominal references, and publishes the binding decisions used by
the rest of the semantic pipeline.

## Scope

Implemented:

- Profile 0.9 parser gating for aliases, qualified expressions, and qualified
  nominal type annotations;
- deterministic namespace routes for requested module names, declared module
  names, aliases, and transitive imports;
- fail-closed duplicate handling for unqualified imported functions and
  nominal declarations;
- stable namespace, scope, binding, and reference identities;
- `SemanticBindingTable` in `Program` and `source-study` output;
- exact imported function/type/variant/record identities through body, HIR,
  SSA, and reference execution;
- stdlib namespace-pressure fixture and bounded research-bytecode/portable-WASM
  corpus.

Explicitly not implemented: general generic type parameters, package/version
resolution, arbitrary re-export policy, memory/layout semantics, or a claim
that every backend supports the new consumer shape.

## Verification commands

The focused checks for this tranche are:

```bash
cargo test -p mncs-compiler --test module_imports -- --nocapture
cargo test -p mncs-syntax profile_09_adds -- --nocapture
cargo test -p mncs-cli --test library_resolution profile09_consumer -- --nocapture
cargo test --workspace --no-run
cargo fmt --all -- --check
```

The module-import suite covers executable qualified aliases, duplicate
unqualified exports, alias collisions and missing members, duplicate nominal
types and finite variants, import-order-independent canonical fingerprints,
local unqualified shadowing, and transitive declaring identity. The CLI test
checks `binding_table.schema_version == "0.1"`, alias provenance/path, and
the bounded research-bytecode and portable-WASM consumer results.

## Backend and evidence boundary

The committed scalar stdlib consumer passes on both `mncs-research-bytecode`
and `mncs-portable-wasm-mvp`. This is a bounded cross-backend witness for
namespace binding and comparison-only execution, not a claim about every
backend or every source shape. The richer geometry consumer was also probed
during development, but existing arithmetic/intent obligations kept its
overall study `UNKNOWN`; it was not relabeled `PASS` and is not part of the
committed positive corpus.

## Joern graph-sensitive check

The repository's required Joern bridge was invoked before and after the
source edits with the focused repository snapshot request. The installed
bridge supports C, C++, Java, JavaScript, Kotlin, PHP, and Python, but this
repository is Rust-only. The Rust request returns `unsupported_language`, and
the C fallback returns `no_source_files`; consequently no valid CPG or graph
finding can be reported for this tranche. This is a tooling limitation, not
evidence that control-flow/reachability properties were proven. The exact
post-edit probes and their failures are recorded in the task handoff.

## Remaining uncertainty

Finite corpus agreement does not prove namespace completeness, generic
soundness, package compatibility, or target equivalence. The next high-value
language tranche is explicit generics only after the type-parameter and
instantiation identity can be carried through all semantic and realization
layers. Until then, duplicated nominal helpers remain an honest pressure test,
not a substitute for generic semantics.
