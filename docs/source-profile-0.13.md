# Source Profile 0.13 — pressure-driven consolidation and progression

Status: **implemented, experimental (current)**. Profile 0.13 is
additive over Profiles 0.1–0.12 and is the explicit evolution home
(RFC 0036) for every post-0.12 grammar, elaboration, semantic, and
resource-envelope extension. Older profiles retain their historical
syntax, semantics, and canonical fingerprints; the
`crates/mncs-cli/tests/profile_compat.rs` suite pins both directions
(old-profile refusals and 0.13 admissions).

Predecessor: Profile 0.12 (`docs/source-profile-0.12.md`).
Machine-readable policy: `mncs-syntax` registry
(`crates/mncs-syntax/src/profile.rs`,
`spec/source-profile-registry.json`).

## Boolean negation and equality (CP-0004)

- Prefix `!bool -> bool` through the total `BooleanNot` operation.
  `!` binds tighter than any binary operator; `!!a`, `!a.b`, `!f(x)`,
  and `!(a == b)` all parse. Below 0.13 `!` never lexed (`MNL002`).
- `bool == bool -> bool` / `bool != bool -> bool` through the total
  `BooleanCompare` operation (`eq` | `ne`). Mixed-type operands keep the
  `MNE119` refusal; ordering comparisons on bools keep `MNE121`. Below
  0.13, `true == false` is `MNE121`.
- Neither is a frontend rewrite: every backend observes the same
  operation. Evidence: `examples/source/pressure-bool-ops.mncs`,
  `examples/execution/pressure-bool-ops-corpus.json` (17 cases, all five
  executable backends), `crates/mncs-cli/tests/pressure_bool_ops.rs`.

## Scalar integer match (CP-0010)

`match` over an integer subject accepts integer literal arms (including
negative patterns such as `-5` on signed types) plus exactly one
required `_` default arm. Totality is an elaboration property over the
open scalar domain. Duplicate literals, duplicated defaults, and arms
after the default are `MNE139`; a missing default is `MNE140`;
out-of-range/overflowing literals are `MNE145` (no silent truncation);
non-scalar arms on integer subjects are `MNE138`, as are scalar patterns
on finite/bool subjects. A bare `_` stays a variant pattern at parse
time, so finite/bool subjects keep their historical meaning (a type may
declare a variant literally named `_`).

Lowering reuses the finite-match branch-chain shape (one
`IntegerCompare eq` per literal arm, default as terminal branch), so no
new operation, obligation, or proof burden is introduced. Ranges,
guards, destructuring, and closed-domain matching without `_` are out of
scope. Below 0.13, integer arms are a parse error (`MNP084`) and integer
subjects keep the historical `MNE136` refusal.

Evidence: `examples/source/pressure-scalar-match.mncs`,
`examples/execution/pressure-scalar-match-corpus.json`,
`crates/mncs-cli/tests/pressure_scalar_match.rs`.

## Sequential iteration-name reuse (CP-0009)

Iteration identities are unique over their live lexical scope: two
sequential non-overlapping loops may reuse a source-level index name,
while a nested loop reusing a still-open enclosing identity stays
rejected (`MNE146`). The recorded identity is hygienic — the first loop
keeps its name verbatim (existing fingerprints are stable) and later
sequential reuses record `name#2`, `name#3`, … (`#` never lexes, so
machine identities cannot collide with user names). Sequential
carried-state reuse stays governed by the rebinding guard (`MNE110`).
Below 0.13, any reuse is `MNE146` under the historical function-wide
rule with verbatim recording.

Evidence: `examples/source/pressure-iteration-reuse.mncs`,
`examples/execution/pressure-iteration-reuse-corpus.json` (4 cases, all
five backends), `crates/mncs-cli/tests/pressure_iteration_identity.rs`,
`crates/mncs-compiler/tests/iteration_identity.rs`.

## Contextual `next` fields (CP-0013)

`next` opens the iteration-step clause and names carried state only
through that step grammar, so it stays reserved exactly there. In field
positions — record declarations and literals, finite payload
declarations and constructions, `.` projections, match payload bindings —
it is an ordinary member name (`record Link { next: u64, after: u64 }`,
`value.next`). A `let` binding or carried state still may not be named
`next`, and malformed step/field syntax keeps its precise diagnostics.
Below 0.13, `next` in field position keeps its historical parse refusal.

Evidence: `examples/source/pressure-next-field.mncs`,
`examples/execution/pressure-next-field-corpus.json`,
`crates/mncs-cli/tests/pressure_next_field.rs`.

## Repeat sequence literals (ENG-0020)

`[value; N]` constructs N copies of one element value (Nat literal
count; symbolic counts stay refused), elaborated by sharing the single
elaborated element across the constructed slots (`MNE184`/`MNE256` on
malformed shapes). Below 0.13, `;` in a sequence literal keeps the
historical `MNP157` refusal.

Evidence: `examples/source/pressure-repeat.mncs`,
`examples/execution/pressure-repeat-corpus.json`,
`crates/mncs-cli/tests/pressure_repeat_literals.rs`.

## Lexical shadowing (ENG-0021)

A plain `let` may rebind a name already bound to a plain value (a `let`
or a parameter) in the same scope: the initializer elaborates against
the previous binding, and the monotonic slot allocator keeps binding
identities distinct and fingerprint-stable. Rebinding over (or under)
index and iteration-state names stays `MNE110` on every profile, and
traversal-discharge reasoning keys off the innermost binding. Below
0.13, same-scope rebinding keeps the historical `MNE110` refusal.

Evidence: `examples/source/pressure-rebind.mncs`,
`examples/execution/pressure-rebind-corpus.json`,
`crates/mncs-cli/tests/pressure_rebinding.rs`.

## Negative literal atoms

Unary `-` directly before an integer or float literal (`-5`, `-2.0`) is
a single expression atom in operand positions (call arguments, record
fields, nested calls, match-arm magnitudes via the scalar path).
General negation (`-x`, `-(a+b)`, `--5`) stays refused — spell it
`(0 - x)` — and binary subtraction is unambiguous. Below 0.13, a
leading `-` before a literal keeps the historical `MNP064` refusal.

## u64 traversal domains (ENG-0010)

Any ordinary scalar element type traverses, including `u64`: the
traversal index is an abstract u64 counter, so a `u64` element type is a
legitimate domain, not a missing resolution. Only truly unresolvable
nominal types fail (`MNB101`). Below 0.13, `u64` domains keep the
historical refusal (enforced at elaboration as `MNE194` so model
validation stays profile-agnostic).

Evidence: `examples/source/pressure-u64-domain.mncs`,
`examples/execution/pressure-u64-domain-corpus.json`,
`crates/mncs-cli/tests/pressure_u64_domain.rs`.

## Deterministic generic-argument inference (ENG-0019)

Calls to generic functions may omit `<...>` when every parameter draws
exactly one answer from the value arguments (constraints flow through
sequence structure and direct generic positions; caller parameters
forward). Ambiguous calls keep `MNE220` with the culprits named
("cannot infer T", "conflicting arguments for N"); explicit `<...>`
always remains. Generic record *declarations* (`Box<N: Nat>`-style)
stay unimplemented. Below 0.13, omitted arguments keep the historical
`MNE220` refusal ("inference is not available in this tranche").

Evidence: `examples/source/pressure-generics.mncs`,
`examples/execution/pressure-generics-corpus.json` (7 cases, all five
backends), `crates/mncs-cli/tests/pressure_generic_inference.rs`.

## Qualified cross-module payload construction (ENG-0007)

`alias.Type.Variant { field: value, ... }` (three-segment qualifier)
constructs a payload-bearing variant across module aliases, mirroring
the two-segment Profile 0.6 shape. Elaboration retries a qualified
record spelling when no finite type matches. Below 0.13, the qualified
path followed by `{` keeps its historical refusal.

Evidence: `crates/mncs-cli/tests/pressure_enum_payloads.rs`,
`examples/source/pressure-payload.mncs`.

## Raised resource ceilings

- Admitted sequence/view lengths: at most 1024 (`MNE105`/`MNE182`/
  `MNE225` past it). Through 0.12 the ceiling was 64.
- Admitted counted-iteration bounds: 1..=1024 per level (`MNE142`).
  Through 0.12 the bound was 1..=32.
- Nesting stays capped at two levels (`MNE147`), and the static product
  of enclosing bounds with a new loop's bound must fit the profile's
  admitted work envelope (`MNE193`): 32 (0.4–0.6), 64 (0.7–0.10), 4096
  (0.11–0.12, covering traversal-64 compositions), 1048576 = 1024×1024
  (0.13, explicit). The model's absolute representational ceilings
  (`MODEL_MAX_ITERATION_BOUND`, `MODEL_MAX_SEQUENCE_BOUND`) are unchanged
  at 1024; calls inside a loop body can multiply work transitively, and
  that residual cost is an obligation on the callee's own bounds, not
  part of the static product.

Evidence: `examples/source/pressure-bounds.mncs`,
`examples/source/pressure-ceilings.mncs` (with execution corpora, all
five backends), `crates/mncs-cli/tests/pressure_raised_bounds.rs`,
`crates/mncs-cli/tests/pressure_ceilings.rs`.

## Structural recursion (RFC 0047, partial)

A direct self-call is admitted when its first argument is a match-bound
structural descendant of a finite first parameter (R1–R5 descendant
provenance). Each admitted call site records a `structural-decrease`
claim the kernel re-derives from body and binding-table facts — the
frontend's admission is never trusted — and every backend enforces the
same static call-depth fuel (incoming depth above
`MODEL_MAX_CALL_DEPTH` fails closed instead of overflowing the native
stack). Generic definitions defer the admitted-ceiling check to
instantiation, and each substituted traversal bound is checked against
the admitted ceiling of the module that defines the traversal: a narrow
root never un-admits library internals admitted under the library's own
profile, and a wide caller never smuggles an over-ceiling instantiation
past the definition site (`MNE182`). Fuel exhaustion is execution-tested
on two axes: explicit per-request budgets exhaust identically on all five
executable backends (uniform fuel seeding — a budgeted entry depth of
`MODEL_MAX_CALL_DEPTH - budget` fits exactly `budget + 1` activations in
both the reference interpreters and native code), and over-cap activation
(a 1025-deep runtime tree, no explicit budget) reports `BudgetExhausted`
on the C11, LLVM, and Cranelift backends via a dedicated status code that
callers propagate without collapsing into generic `RuntimeFailure`.
Malformed budgets (zero, above the cap) are `InvalidRequest` on all five
backends, exactly like the reference. The reference interpreters recurse
on the host stack, so the over-cap case is excluded from them in debug
builds by construction (1025 activations overflow an 8 MiB debug stack);
native frames are small enough that the same tree runs natively.

Evidence: `examples/source/recursion-rfc/` (ten study fixtures),
`examples/source/recursion-rfc/recursion-probe.mncs` with
`examples/execution/recursion-rfc-corpus.json` (eight cases, all five
backends), `crates/mncs-cli/tests/pressure_structural_recursion.rs`.

## Explicit non-claims

- General recursion, mutual/indirect recursion, numeric countdown
  recursion, cross-module recursion, and higher-order recursion stay
  rejected (`MNE130`); only the admitted structural subset above runs,
  see RFC 0047.
- Unrestricted `while` loops, heap allocation, traits, unrestricted
  callable values, and a conventional runtime model are all out of
  scope; boundedness, proof/evidence discipline, semantic identity, and
  the explicit backend capability model still govern.
- Generic nominal record declarations, dimension arithmetic, numeric
  traits, and higher-order effect capture remain future capability-gap
  work, not 0.13 features.
