# RFC 0007 Tranche 0.2 — Genuine Dependency: calculus specification

Status: **implementing**. Companion to `library/core/proof_dep.mncs`
(`mncs.core.proof_dep.v2`, kernel `mncs:proof-kernel:0.2`) and
`crates/mncs-model/src/proof_dep.rs` (independent reference checker).
Tranche 0.1 (`proof_term.v1` / `proof_check.v1`, kernel `0.1`) is frozen,
not modified: compatibility is explicit and versioned.

## Why tranche 0.1 was not genuinely dependent

In `proof_check.v1`, `Var` names a binder **cell index below it** while
buffers are topologically ordered (children strictly below parents). A
`Lambda` body therefore can never mention its **own** binder: self-reference
would be a forward reference, which fails closed. Lambda bodies that check
are necessarily non-dependent, and dependent application reports `UNKNOWN`
because no substitution exists. `Pi` formation over closed types is real but
is not dependent reasoning. This document specifies the representation that
removes that ceiling.

## Representation: flat binder-edge graph with explicit levels

Terms are flat `[Cell; 32]` buffers with an active prefix `count`, exactly
as in tranche 0.1. Every child **reference** is a buffer index strictly
smaller than its parent (topological order, termination by construction).
Binding is separate from buffering:

- `Pi`, `Lam`, and `Hyp` cells carry an explicit binder **level** immediate
  (`arg2` for `Pi`/`Lam`, `arg0` for `Hyp`).
- `Var` carries a level immediate (`arg0`) and may name a binder **above**
  it. Binding is therefore forward; self-reference is ordinary.
- Binder levels in the active prefix are pairwise **distinct** (Barendregt by
  construction, checked: duplicates are `FAIL`). Shadowing is impossible, so
  capture is impossible: substitution never needs renaming, shifting, or
  freshening. There is no de Bruijn index arithmetic anywhere.
- Resolution: level `l` resolves to the `Hyp` cell declaring `l` if one
  exists (assumption use, buffer-global scope, except inside assumption
  types themselves, which must be closed), else to the nearest ancestor
  `Pi`/`Lam` cell with level `l` (walk up single-parent edges, skipping
  domain edges since domains sit outside their binder's scope; must exist,
  else `FAIL`). Cross-tree references cannot resolve and fail closed, so
  assumptions cannot appear implicitly.
- Sharing rule (derived, not imposed): a cell transitively containing a
  `Var` (`mentions > 0`) must have at most one distinct parent, else `FAIL`
  (resolution would be ambiguous: one cell cannot stand for two occurrences
  under different binders). Open cones are therefore trees: generators
  duplicate one `Var` cell per use site. Closed cells (`mentions == 0`)
  share freely; they mean the same thing in every context.

## Substitution without rebuilding: closures

Substitution is realized as **environment extension**, never as buffer
rewriting (an explicit-substitution machine). A closure is a
`(term, env)` pair; environments are linked frames in a bounded pool mapping
levels to values (terms with environments, or neutral variables). `App` of a
`Lam` extends the environment; nothing is copied, so sharing is always
sound and there is no blowup to bound. Capture-safety holds by
construction (unique levels) and is additionally guarded: resolution failure
is `FAIL`, never silent capture.

Types elaborate to level-scoped **values** in a bounded pool: free variables
are absolute levels, so values are environment-free and compare
structurally. `VPi`/`VLam` nodes keep their binder's `(term, env)` closure
for the codomain/body; a `Lam`-synthesized `Pi` additionally records the
body's **type** value (marked `-2`) for comparing `Pi` types and checking
codomain well-formedness — the family-as-term lives only on the `VLam`
body closure, and every family instance applies that closure, never the
`-2` value. A standalone value-substitution pass (`vsubst`,
DAG-memoized, aliasing shared nodes) instantiates value-form codomains; it
is capture-safe for the same reason (unique levels make shadowing
unrepresentable).

Substitution consequences exercised by the corpus:

- identity, nested binders, substitution under `Pi`/`Lam`, into `Eq`;
- dependent application (`App` result = codomain closure extended);
- binder-depth/shadowing-shaped cases (duplicate levels `FAIL`);
- malformed references (forward, self, out-of-range: `FAIL`);
- stuck-on-neutral cases that must be `UNKNOWN`, never `PASS`.

## Reduction: evaluation to weak-head values with fuel

Evaluation is kernel semantics in MNCS: an explicit-operation-stack machine
over closures with bounded fuel (32 micro-steps per nested evaluation).
Supported reductions:

- beta: `App(Lam(d, b, l), a)` evaluates `b` with `l ↦ a`;
- `Plus` by recursion on the first argument: `Plus(Zero, m)` is `m`,
  `Plus(Succ(a), b)` rebuilds `Succ` over the unfolded tail (symbolic, so
  open terms reduce under constructors; rebuilds beyond 24 fail closed);
- closed-literal computation (`Succ`/`Plus` over literals, compared by
  integer reading);
- `NatElim` iota: target `Zero` selects the zero branch; target `Succ(n)`
  unfolds (bounded spine of 16) over the predecessor;
- stuck terms (neutral target, neutral function, `Unsupported`) stay stuck.

Fuel exhaustion, stack overflow, pool exhaustion, and structurally
unsupported shapes resolve to `UNKNOWN`, never `PASS`.

## Definitional equality

Structural comparison of weak-head values with an explicit pair stack
(8 entries, 32 visits). `Pi ≡ Pi` (and `Lam ≡ Lam`) compares domains, then
codomains under a shared rigid variable: the binders' own level when they
agree, else a fresh level `32 + stack depth` (disjoint from every buffer
level, hence capture-free). Neutral `Var` equals itself by level. Closed
literals compare by integer reading. Proof objects are opaque (`UNKNOWN`).
Everything undecided is `UNKNOWN`.

## Typing: infer plus defeq, no separate check

Every active cell synthesizes a term value and a type value. `Lam`
synthesizes its `Pi` (no claim cell, unlike v1), but the two values split
the work and must not be confused: the `VLam` term value keeps the body
**closure** (term-form, for beta and family application); the `VPi` type
value keeps the body's **type** value (value-form, marked `-2`, for
comparing `Pi` types and checking codomain well-formedness — never for
instantiation). `Refl(w)` synthesizes `Eq(T, w, w)` once `T` classifies;
`Cong(p, sA, sB)` turns `Eq(Nat, a, b)` into
`Eq(Nat, Succ(a), Succ(b))` against generator-provided `Succ` cells;
`App` requires a `Pi` and instantiates the codomain, with the term coming
from the function's `VLam` closure and the type from the `VPi` codomain;
`NatElim` accepts **dependent motives** (`motive : Pi(Nat, U(k))`) with the
zero branch and a structurally-checked step type
(`Pi(k:Nat, Pi(ih:P(k), P(Succ(k))))` verified by motive application,
never by constructing step types). Every family instance `P(v)` is the
motive **term** applied to `v` (`VLam` bodies evaluate, stuck motives stay
stuck, ill-shaped motives abstain); the motive **type** contributes only
the domain check and the codomain classifier check. All premise checks are
definitional-equality queries over already-normal values, so there is one
trust-relevant comparison, not a parallel set of rules.

Universes: `Universe(l) : Universe(l+1)` for `l < 3`, `Nat : Universe(0)`,
`Eq(T, _, _) : Universe(level of T)`, `Pi` at the join level. The ceiling
`3` is unchanged from tranche 0.1 and remains an explicit semantic bound of
this kernel version, not a language limitation.

## Assumptions

`Hyp(l, T)` cells declare assumptions with canonical identity
`(level, carrier type)`. A `Hyp` must be a **root** (no parents): it is a
declaration, not a subterm, so embedding one inside a term is `FAIL` (this
also fixes the declaration order — a `Hyp` at index 0 could never reference
its own type below it; roots can sit anywhere). A proof whose `Var`
resolves to a `Hyp` consumes that assumption; the artifact's assumption set
is part of its sealed identity, so adding, removing, or changing an
assumption invalidates reuse. Undeclared free variables (resolution
failure) are `FAIL`, never `PASS`. Naming a `Hyp` as the proof or the
proposition is a malformed query (`FAIL`). Closed proofs use no `Hyp`
cells and identify the empty assumption set (`assumptions_of == 0`). `Hyp`
types must be closed (dependent assumptions are a recorded future
boundary): a `Var` physically inside an assumption's type that resolves to
a `Hyp` is `FAIL`.

## Status discipline

`FAIL > UNKNOWN > PASS` through `mncs.core.status.v1`, exactly as in
tranche 0.1. Malformed structure is `FAIL`; undecided structure
(unsupported tags, stuck neutrals, fuel/pool exhaustion, level-mismatched
binders in defeq) is `UNKNOWN`. No path maps `UNKNOWN` to `PASS`.

## Bounds (explicit and versioned)

`[Cell; 32]`, split-range value pool of 122 nodes (ids 6..127 across two
64-wide banks), environment pools of 64, nested-evaluation fuel 32,
evaluator operation stack 6, definitional-equality pair stack 8 over 32
visits, value-substitution stack 32 with a split 128-entry node map,
eliminator spine 16, literal rebuild cap 24 — all recorded in
`proof_dep.mncs` and bound to kernel identity `mncs:proof-kernel:0.2`.
Exhaustion is `UNKNOWN`, never a trap and never `PASS`. Capacity doubled
from 16 to 32 because the flagship dependent proof needs 29 cells. The
value pool is split because the flagship saturates a single 58-slot bank
(high-water exactly 64, measured with a temporary probe, since removed)
and passes in the split pool; one wider array is inexpressible in
MNCS 0.10 (`MAX_SEQUENCE_BOUND` is 64).
Readers dispatch on the id (`id < 64` selects the bank); the substitution
node map splits the same way, since it is keyed by pool id. Bounds are
part of the versioned contract, and overflow stays fail-closed.

## Flagship theorem

`Π (n : Nat). Eq Nat (Plus n Zero) n` proved by `Lam(n, NatElim(...))` with
motive `Lam(n, Eq(Nat, Plus(Var n, Zero), Var n))`, base `Refl(Zero)`, and
step `Lam(k, Lam(ih, Cong(Var ih)))`. It exercises a bound variable, a
dependent proposition over that variable, substitution during application
(environment extension), symbolic `Plus` reduction, and a kernel-checked
proof term. Tranche 0.1 returns `UNKNOWN`/`FAIL` on every encoding of this
statement (open terms, dependent application, dependent motive); tranche 0.2
returns `PASS`.

## Bring-up log (abridged honest record)

- Assumptions failed first: `Hyp` levels never enter lexical environments
  (roots have no body edge to extend), so every assumption use failed the
  `adv_var` lookup gate. Fix: uses resolving to a `Hyp` skip the gate and
  mint a rigid with the carrier type. Flipped `assumption-open-refl` from
  1 to 0; undeclared variables still `FAIL` through the same gate.
- The flagship saturated the 64-node value pool (high-water exactly 64,
  measured with a temporary `pool_use_code` entrypoint, since removed).
  The infer/claim split was 60/4-and-stuck: inference fits, the claim does
  not. One wider array is inexpressible (`MAX_SEQUENCE_BOUND` is 64), so
  the pool is split-range (ids 6..127, 122 nodes) with the substitution
  node map split the same way. Post-split the flagship passes, so its need
  fits in 122; the exact post-split high-water was not re-measured (the
  probe was removed before the split landed).
- The bank migration transposed `vcn_at` id/slot arguments at 55 call
  sites — type-invisible (all `i64`), zero diagnostics. The differential
  corpus caught it at once (flagship 0 to 1). Mechanical refactors of this
  kernel must re-run the corpus; the typechecker cannot see the bug class.
- The motive term/type split bit once: family instances must apply the
  motive `VLam` term value (real body closure), never the synthesized
  `VPi` type's `-2` value-form codomain (which holds the body's *type*).
  The independent reference checker reproduced the confusion first, which
  is precisely what it is for; the calculus text now states the split
  explicitly.
