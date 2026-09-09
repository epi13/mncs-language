# RFC 0047: Provably Terminating Structural Recursion over Finite Values

- **Status:** Draft
- **Target:** a future language tranche (not this one)
- **Depends on:** RFC 0004, RFC 0007, RFC 0019, RFC 0022, RFC 0041
- **Pressure source:** CP-0011 (`mncs-compiler/pressure/0011-acyclic-calls-machines.md`)

## Summary

Source Profiles 0.3+ reject every call cycle (`MNE130`), including direct
self-recursion. A compiler frontend is naturally recursive, so the entire
declaration core is written as explicit state machines (shunting-yard
expression parsing, frame-stack block parsing, worklist tree traversals),
tripling traversal code and moving progress arguments into comments.

This RFC proposes a narrow, provably terminating exception: direct
self-recursion over an inductive finite value where every recursive call
is statically proven to consume a proper structural subvalue obtained by
matching the function's recursive parameter. General recursion, mutual
recursion, and numeric recursion stay rejected. `MNE130` stays fail-closed
until an implementation of this design lands with its proof evidence.

## Non-goals

- General recursion or Turing-completeness. Termination, boundedness, and
  the proof model (RFC 0022) outrank expressiveness.
- Mutual or indirect recursion (deferred; the cycle graph stays for it).
- Numeric recursion (`f(n - 1)`), even when a human sees termination.
- Recursion over sequences, integers, or any non-finite measure.
- New syntax for designating the recursive parameter (deferred; see R1).
- Hand-rolled-machine removal in `mncs-compiler` (that happens after, in
  the compiler repo, once the language lands this).

## Proposed rule

For a function `f` with first parameter `p: T` where `T` is a finite
(inductive enum) type, a direct self-call `f(a0, a1, ...)` is admitted iff
all of the following hold:

- **R1 (designated measure).** The recursive parameter is the first
  parameter. It must have finite type. (Generalizing to an annotated
  parameter is future work; positional is enough for the corpus and needs
  no grammar change.)
- **R2 (descendant provenance).** The argument in the recursive-parameter
  position is a *strict structural descendant* of `p`: a value bound by a
  `match`-arm payload binding whose scrutinee is `p` or is itself a
  strict structural descendant (transitive closure through `match` only).
  Arguments in other positions are unrestricted — they cannot affect the
  measure.
- **R3 (resolution, not names).** Provenance is computed over
  *resolved bindings* (binding identities), never source text. A `let`
  alias (`let t = tree`), a rebinding that shadows a descendant name, a
  reconstructed value (`T.Node { ... }`), a call result, arithmetic, a
  `select`, a cast, or iteration state never carries descendant
  provenance, even when the spelling coincides.
- **R4 (self only).** Only direct self-calls are admitted. Any other
  cycle — mutual, indirect, or through a helper — is still `MNE130`.
- **R5 (evidence).** Every admitted call site records a
  `structural-decrease` obligation (see below). There is no silent
  admission path.

Accepted shape (must accept someday):

```mncs
enum Tree { Leaf, Node { left: Tree, right: Tree } }
fn depth(tree: Tree) -> (result: u64) {
    return match tree {
        Leaf => 0,
        Node { left: l, right: r } => depth(l) + depth(r) + 1
    };
}
```

Permanently rejected shapes (must always reject):

```mncs
depth(tree)            // root, not a descendant
count(n - 1)           // numeric, not structural
even(n) / odd(n)       // mutual
bad(rebuild(l, r))     // reconstructed, not matched
```

## Why the current architecture cannot host this yet

1. **The cycle check is pre-elaboration and syntactic.**
   `reject_recursive_calls` (`crates/mncs-compiler/src/frontend.rs`)
   runs on the raw AST after signature collection, before any body is
   elaborated. It walks names (`calls_in_statement` / `calls_in_expr`),
   so it cannot see shadowing, rebinding, or which `l` a use resolves
   to. R3 requires the check to run during or after body elaboration
   with access to resolved binding identities (`BindingEnv`, which
   already implements innermost-wins shadowing and the `MNE110`
   rebinding guard).
2. **No persistent call-graph or termination evidence exists.**
   The cycle graph is a transient `BTreeMap` built per elaboration and
   discarded. `mncs-model` has no call-graph fact, no acyclicity
   obligation, and no `structural-decrease` evidence kind. The proof
   kernel (`proof_kernel.rs`) and verifier know nothing about calls.
   Admission without persistent, kernel-checkable evidence would be
   exactly the frontend-trusts-itself hole the kernel exists to close.
3. **The runtime has no call-depth story.** The reference interpreter
   executes `Call` by recursing in host Rust (`execute_inner`) with only
   the step budget as a bound — no call-depth limit. Finite values carry
   no declared depth bound, so native-stack recursion in C/LLVM/Cranelift
   lowering has no stack budget to check against, and PTX/eBPF targets
   cannot recurse at all. Termination by measure is a static fact; the
   runtime still needs deterministic fuel accounting (see below).

## Proof-kernel design (for the implementation tranche)

- New obligation kind `structural-decrease`, one record per admitted
  call site: subject (call-site operation identity), requirement
  (function identity + recursive-parameter binding identity), and
  dependencies (the chain of match-operation identities from the
  parameter binding to the argument binding).
- The kernel must **re-derive**, not trust: given the recorded match
  operations (which the kernel already observes as
  `FiniteIsVariant`/`FinitePayloadProject` operations over resolved
  value identities), it re-checks that each link binds its value from
  the previous link's scrutinee and that the chain starts at the
  recursive parameter. A frontend claim of "decreases" with no
  verifiable chain is rejected; corrupted or sabotaged evidence fails
  the same way.
- The kernel's trusted surface grows by one narrow re-derivation rule
  over already-trusted operation facts. No general measure theory, no
  solver, no ordinal machinery.
- Semantic fingerprinting must include the admitted-call evidence so a
  program whose recursion evidence differs is a different program.

## Runtime design (for the implementation tranche)

Static termination does not remove runtime accounting. Every admitted
recursive call consumes call-depth fuel:

- The interpreter gains an explicit call-depth counter on
  `execute_inner` (it has none today) with deterministic
  budget-exhaustion failure, exactly like the step budget.
- Native backends (C11, LLVM IR, Cranelift) thread the same counter or
  prove `max_value_depth * frame_size <= stack_budget` per backend —
  until such a proof exists, the counter is mandatory.
- WASM MVP likewise: an explicit depth global, not reliance on host
  traps.
- External targets that cannot recurse (PTX, eBPF, RISC-V as
  capability-declared) refuse admitted-recursive programs with an
  honest capability diagnostic instead of miscompiling.
- Exhaustion is a deterministic runtime failure, never silent
  wrong-answer and never unbounded host stacking.

## Conformance corpus

Positive fixtures (in `examples/source/recursion-rfc/`, must be accepted
by the implementation tranche, rejected with `MNE130` until then):

- `depth.mncs` — two recursive calls on direct descendants (`l`, `r`).
- `single-descendant.mncs` — one recursive call plus unrestricted extra
  arguments (a fold with an accumulator argument).
- `nested-descendant.mncs` — transitive provenance through two matches
  (grandchild `l2`).
- `arg-position.mncs` — the descendant in recursive-parameter position
  with other arguments arbitrary.

Negative fixtures (must be rejected in every tranche; `MNE130` today and
after, since none carries structural evidence):

- `root-call.mncs` — recursion on the unchanged parameter.
- `alias-call.mncs` — recursion on a `let` alias of the parameter.
- `numeric-countdown.mncs` — `f(n - 1)` on an integer.
- `mutual.mncs` — two functions calling each other.
- `reconstructed.mncs` — recursion on a rebuilt value from a helper.
- `shadow-trap.mncs` — recursion on a name rebound to a call result
  over a descendant spelling.

`crates/mncs-cli/tests/pressure_structural_recursion.rs` pins the
fail-closed present: every fixture above currently elaborates to
`MNE130`. When the implementation tranche lands, the four positives move
to execution tests and the six negatives keep their test.

## Acceptance criteria for the implementation tranche

1. All four positive fixtures elaborate, execute identically on all five
   executable backends, and record `structural-decrease` obligations
   that the kernel discharges from re-derived chains.
2. All six negative fixtures are rejected (`MNE130` or a narrower
   successor code with the same fail-closed meaning).
3. Evidence sabotage (a hand-mutated decrease chain) is rejected by the
   kernel where the test framework supports such tests.
4. `MNE130` still rejects every non-structural cycle, including mutual
   recursion and numeric countdowns in new positions.
5. Interpreter call-depth exhaustion is tested (deep value, small depth
   budget → deterministic failure, not host stack overflow).
6. External targets declare the capability honestly.
7. No change to non-recursive programs' fingerprints, obligations, or
   diagnostics.

## Open questions

- Should the recursive parameter be positional (first) or designated by
  a future annotation? Positional is proposed; call-site ergonomics in
  real compiler code (context-first signatures) may force the issue.
- One match level or transitive closure? Transitive is proposed (the
  compiler's traversals destructure two levels); each link is
  independently re-derivable, so transitivity adds no trust.
- Should admitted recursion be allowed inside `iterate` bodies? The
  fuel stories compose (depth fuel × step fuel) but the accounting needs
  care; propose forbidding initially.
- Value-depth bounds as first-class facts (RFC 0019 territory) would
  strengthen the backend stack story; out of scope here.
