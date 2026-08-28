# Source Profile 0.9 — namespaces, qualified references, and binding evidence

Status: **implemented, experimental** (2026-08-27). Profile 0.9 is additive
over Profile 0.8. It makes the existing elaboration-time module linker
namespace-aware and publishes its binding decisions as semantic data. It does
not add general generics, package version solving, dynamic loading, or a new
authority mechanism.

## Imports and routes

The Profile 0.6 import form remains valid:

```mncs
use mncs.core.ordering.v1;
```

Profile 0.9 additionally accepts an alias:

```mncs
use mncs.core.ordering.v1 as order;

fn clamp(value: i64) -> (result: i64) {
    return order.clamp_i64(value, 0, 10);
}
```

An imported declaration can be named through its declared module path, its
requested path, or its alias when one exists. A qualified route is only a
name-resolution path; importing a module or assigning an alias never grants
capability or effect authority. Transitive imports retain the original
declaring module identity and are available through the consuming module's
routes.

Unqualified lookup is deliberately conservative. A local declaration wins
over an imported declaration. One imported identity may be used unqualified;
two different imported identities with the same final spelling are
qualification-only. Qualified functions, finite types/variants, records, and
record fields remain available. Duplicate aliases are `MNE181`; an alias that
collides with a local declaration is `MNE182`; missing qualified members retain
the existing unresolved-binding diagnostics.

Aliases and dotted references are gated to Profile 0.9. A Profile 0.8 source
file using `as` receives `MNP181`, and earlier programs do not acquire the new
qualified syntax through inference. Existing Profile 0.6–0.8 import and
canonical-identity behavior remains unchanged.

## Authoritative binding substrate

Successful Profile 0.9 source elaboration attaches an optional
`SemanticBindingTable` to `Program` and includes it in `source-study` output.
The table has schema version `0.1` and four sorted collections:

| Collection | Meaning |
| --- | --- |
| `namespaces` | module identities and their source names |
| `scopes` | module/body/lexical owners and parent relationships |
| `bindings` | declaration identity, spelling, namespace, scope, and binding kind |
| `references` | each resolved occurrence, route, provenance, and target binding |

Binding kinds include functions, parameters, locals, iteration state, finite
types/variants, record types, and record fields. Resolution provenance is
explicit: `local`, `direct_import`, `qualified`, `aliased`, or
`transitive_import`.

Binding and reference identities are derived from semantic namespace/function
owners and stable structural slots. Source offsets are retained on
`SemanticReference` for diagnostics and navigation only; they are erased from
the canonical fingerprint. Import discovery order therefore cannot rename a
binding or change a semantic fingerprint. The same resolved declaration
identity is consumed by body validation, HIR, SSA, and the reference execution
target lookup.

## Deliberate boundary: generics

This profile does not pretend that repeated nominal code is generic code. A
sound generic tranche still needs a type-parameter representation, scoped
substitution/instantiation identity, constraints, recursion and termination
rules, and backend obligations for every realization. Those pieces are not
implemented across `BodyType`, semantic identity, HIR/SSA, and execution, so
the current profile stops at namespace-aware nominal binding and records the
generic gap as the next design boundary.

## Fixtures and evidence

- `examples/source/profile09-stdlib-namespace-consumer.mncs` aliases two core
  modules with overlapping `clamp_i64` exports.
- `examples/execution/profile09-stdlib-namespace-consumer-corpus.json` is its
  bounded research-bytecode and portable-WASM experiment corpus.
- `library/core/bounds.mncs` intentionally adds namespace pressure without
  introducing arithmetic or unresolved contract obligations.
- `crates/mncs-compiler/tests/module_imports.rs` covers qualified calls,
  duplicate fail-closed behavior, aliases, nominal type/variant identity,
  import-order stability, local shadowing, and transitive visibility.
- `crates/mncs-cli/tests/library_resolution.rs` checks the source-study JSON
  binding table and the stdlib consumer experiment.

Agreement over these finite cases is empirical evidence, not a proof of the
language or of all backend realizations. The current scalar consumer passes on
research bytecode and the portable WASM MVP; unsupported or unresolved backend
envelopes remain explicit rather than being promoted to `PASS`.
