# Source Profile 0.4: Semantically Bounded Iteration

Profile 0.4 includes all Profile 0.3 programs unchanged and adds the bounded-transition statement
defined by RFC 0043.

An implementation accepts a bounded iteration only when:

1. the declared bound is a source literal in `1..=32`;
2. initial, header, next, and exhausted values have one identical declared type;
3. its only backedge targets the declared header with exact state/counter arguments;
4. every cycle belongs to exactly one valid bounded-iteration region;
5. every body call retains exact callee and capability/effect closure;
6. bound, resource-ceiling, authority, state, completion, and exact-cost obligations survive HIR
   and selected SSA; and
7. backend artifacts remain identity-bound to that selected SSA.

Profile 0.4 elaboration MUST set the additive `bounded_iteration_only` cycle policy on its semantic
body and carry it through HIR and selected SSA. A cycle governed by that policy without exact
iteration metadata is invalid. Older semantic transports that omit the policy remain governed by
their prior schema behavior; omission in a Profile 0.4 elaboration is not a valid downgrade.

The declared attempt bound and runtime execution budget are distinct. Exhausting the former is a
normal declared completion path; exhausting the latter is `budget_exhausted`. Exact runtime cost
may remain `UNKNOWN` without erasing the static attempt ceiling.

Profiles 0.1–0.3 must not accept the new statement or reinterpret its keywords as older semantics.
