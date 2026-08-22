# MNCS Source Profile 0.3

Status: experimental CRE bootstrap profile

Profile 0.3 is a coherent additive world over Profiles 0.1 and 0.2. It exists to make the first
Concept Reconstruction Experiments executable; it is not the final MNCS grammar or type system.

| Capability | Profile 0.3 behavior |
|---|---|
| finite types | nominal `enum Name { A, B }` with stable type/variant identities and canonical ordinal discriminants |
| constructors | qualified `Name.A`; hidden integers are not source-level constructors |
| branching | expression-valued `match`; every declared variant must appear exactly once |
| expressions | boolean/integer literals, names, nested expressions, comparisons, arithmetic, calls, and match results |
| returns | any well-typed expression may be returned |
| calls | resolved within one module; exact arity/types; general recursion and mutual recursion rejected |
| authority | caller must declare every callee capability and matching effect kind/capability pair |
| lowering | finite values and calls remain explicit through semantic bodies, HIR, SSA, graph edges, obligations, and traces |
| adapters | portable WASM MVP uses a private integer representation behind a typed value contract; research bytecode retains the selected SSA form |
| experiments | exact expected returns and bounded finite-domain property laws contribute to PASS/FAIL |

## Rejections

| Condition | Diagnostic or result |
|---|---|
| unknown type/variant constructor | `MNE123` / `MNE124` |
| non-exhaustive match | `MNE140` |
| duplicate or unreachable match arm | `MNE139` |
| unknown call, wrong arity, or wrong argument type | `MNE131` / `MNE132` / `MNE133` |
| recursive call graph | `MNE130` |
| caller lacks callee authority/effect closure | `MNE134` |
| effect lacks a declared capability | `MNE111` |
| invalid runtime variant/discriminant | `invalid_request` before candidate execution |

The profile intentionally rejects general sum payloads, heap objects, records, polymorphism,
cross-module calls, higher-order functions, general recursion, and source-level iteration. Named
contracts remain structural/name-bearing; Profile 0.3 does not pretend they form a theorem language.

## Example

```mncs
mncs 0.3;
module example.evidence;

enum Evidence { PASS, FAIL, UNKNOWN }

fn combine(left: Evidence, right: Evidence) -> (result: Evidence) {
    return match left {
        PASS => right,
        FAIL => Evidence.FAIL,
        UNKNOWN => Evidence.UNKNOWN,
    };
}
```

See `examples/source/cre1-evidence-combine.mncs` for the complete nested-match and call fixture.
