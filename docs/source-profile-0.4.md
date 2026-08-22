# MNCS Source Profile 0.4

Status: experimental bounded-iteration and CRE-3 profile

Profile 0.4 is additive over Profiles 0.1–0.3. Older profiles retain their syntax, semantics, and
canonical fingerprints; Profile 0.3 still rejects iteration.

## Bounded transition form

`iterate ID up_to N carrying NAME: TYPE = INITIAL { BODY next NAME = NEXT; }` executes `BODY` at
most `N` times. `N` is a semantic source literal from 1 through 32. `NAME` is a typed loop-carried
value visible in the body and after exhaustion. A body `return` completes the function early;
`fail` preserves the declared failure path; otherwise `next` supplies the next attempt's state.

The compiler retains the iteration identity, bound, state relationships, region blocks, calls,
capabilities, completion modes, and six obligations through semantic bodies, graph, HIR, selected
SSA, both artifacts, and execution evidence. The exact static instruction cost obligation remains
`UNKNOWN`; the attempt ceiling is known independently of the runtime step budget.

Profile 0.4 elaboration marks its body, HIR, and SSA with the additive
`bounded_iteration_only` cycle policy. That policy makes undeclared cycles invalid while preserving
the serialized form, fingerprints, and previously supported cyclic semantic fixtures from the
older schema. It is a compatibility discriminator, not permission for Profile 0.4 to accept a
metadata-free cycle.

## Diagnostics

| Condition | Diagnostic |
|---|---|
| `while` or another unbounded form | `MNP106` |
| non-literal bound | `MNP094` |
| bound outside 1–32 | `MNE142` |
| wrong state name at `next` | `MNE143` |
| initial/next state type mismatch | `MNE144` |
| duplicate iteration identity | `MNE146` |
| nested iteration | `MNE147` |
| unreachable continuation | `MNE148` |
| authority laundering in body call | `MNE134` |
| recursive workaround | `MNE130` |
| semantic cyclic CFG without metadata | `MNB060` |
| SSA cyclic CFG without metadata | `SSA018` |

See `examples/source/cre3-retry-authority.mncs`, its typed 0.2 execution corpus, mutants, and
negative fixtures. Profile 0.4 does not add a general loop or recursion.
