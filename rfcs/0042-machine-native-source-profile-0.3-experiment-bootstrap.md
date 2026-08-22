# RFC 0042 — Machine-Native Source Profile 0.3 Experiment Bootstrap

Status: Proposed and implemented as an experimental profile

## Summary

Source Profile 0.3 is the smallest coherent increment needed for the first MNCS Concept
Reconstruction Experiments. It adds nominal finite types, exhaustive typed `match`, expression
returns and nesting, boolean literals, and acyclic intra-module calls with exact authority/effect
closure. The profile remains bounded and preserves the backend-plural architecture of RFC 0041.

Profiles 0.1 and 0.2 remain immutable language worlds. A `mncs 0.3;` header selects this profile;
older headers neither acquire nor silently reinterpret the new constructs.

## Decisions

1. A finite type and each variant have stable nominal identities derived from module/type/variant
   names. Declaration order defines canonical ordinal discriminants.
2. Source code constructs variants by qualified name. Numeric discriminants are a private backend
   representation and invalid type/variant/discriminant triples are rejected at execution ingress.
3. A match is expression-valued and must name every variant exactly once. There is no backend
   default arm and no implicit UNKNOWN/failure behavior.
4. Calls resolve only inside the current module. Direct or mutual recursion is rejected. A call
   records the exact callee identity, required capabilities, effects, and caller capability uses.
5. A caller cannot acquire authority from a callee. The caller must independently declare every
   required capability and a matching effect-kind/capability relationship.
6. Both current adapters consume the same selected SSA. Portable WASM may encode a finite value as
   an integer only behind an artifact-bound language value contract; research bytecode retains the
   selected SSA representation. Neither encoding changes source semantics.
7. Experiment corpora may bind exact expected returns and finite-domain algebraic laws. These are
   bounded observations: PASS is not proof or conformance, FAIL retains a counterexample, and
   UNKNOWN cannot authorize a stronger transformation.

## Explicit exclusions

Profile 0.3 does not add payload-bearing sums, general records, heap allocation, cross-module or
higher-order calls, polymorphism, general recursion, or iteration. Named contracts remain the narrow
structural model inherited from Profile 0.2. CRE-3 retry semantics remain blocked on a semantically
visible bounded-iteration construct rather than being approximated with unrestricted loops.

## Required compiler path

Every accepted construct is represented through lexer/CST/AST, elaborated semantic bodies,
canonical semantic artifacts and graphs, obligations, HIR, canonical/selected SSA, both backend
adapters, bounded execution, traces, and language-owned experiment results. If a future adapter
cannot realize one of these constructs, it must return an explicit unavailable/UNKNOWN realization
result; it may not rewrite the construct.

## Diagnostics

The implementation assigns structured errors for unknown constructors, non-exhaustive and duplicate
arms, unresolved/ill-typed calls, recursion, undeclared effect authority, and call authority
laundering. See `docs/source-profile-0.3.md` for the current inventory and exact codes.
