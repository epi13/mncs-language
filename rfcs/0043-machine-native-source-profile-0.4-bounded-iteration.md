# RFC 0043: Machine-Native Source Profile 0.4 Bounded Iteration

Status: Implemented experimentally

## Problem

Profile 0.3 cannot represent retry, polling, or another finite state transition without hand
unrolling calls. A general loop plus a runtime budget would hide the language-level ceiling and
would make termination an execution accident rather than inspectable semantics.

## Decision

Profile 0.4 adds one structured statement:

```mncs
iterate retries up_to 3 carrying state: State = initial {
    // calls, matches, lets, conditionals, early return, or failure
    next state = transition(state);
}
```

The bound is a decimal source literal in `1..=32`. The maximum is a versioned profile rule chosen
to keep experiment domains and generated evidence finite; it is not a host integer limit. The
initial and `next` values must have the declared nominal or scalar type. `return` completes the
enclosing function early. Reaching `next` performs one state transition. After the declared number
of attempts, control falls through and the carried binding names the last state.

`while`, computed bounds, zero, bounds above 32, nested iteration, recursion, missing continuation,
and state-type changes are rejected. A runtime step budget remains independent and may still end a
valid bounded computation with `budget_exhausted`.

## Semantic representation

The executable body retains a `BodyBoundedIteration` region with stable identity, declared bound,
carried type/value identities, preheader/header/body/backedge/exit blocks, callees, capabilities,
and completion modes. HIR and SSA retain peer region records with transformed block identities and
exact obligation links. Canonicalization sorts set-like dependencies; changing the bound or state
transition changes semantic, HIR, and SSA fingerprints.

Profile 0.4 bodies carry an additive `bounded_iteration_only` cycle policy. Under that policy only
a backedge named by a valid iteration region is legal; body and SSA validation reject every other
cycle, malformed block arguments, lost metadata, missing obligations, and mismatched call/authority
closure. Older serialized semantic fixtures omit the policy and retain their established behavior
and fingerprints. This makes block-parameter SSA loop-carried values available without silently
redefining an earlier language world or making arbitrary Profile 0.4 cyclic SSA legal.

## Obligations

Each iteration emits identity-bound obligations for bound validity, static resource ceiling,
exact resource cost, authority closure, state preservation, and completion modes. All except exact
instruction cost are decided directly by the bounded profile checks. Exact cost remains `UNKNOWN`;
an experiment that claims only the attempt ceiling may still PASS its requested observations while
retaining that unresolved obligation in the compiler study.

Repeating a region never grants authority. Every body call must already satisfy the caller/callee
capability and effect closure before entry, and the same dependencies survive HIR and SSA.

## Realization

Both adapters consume the same selected SSA. Portable WASM uses its existing private dispatcher
loop; research bytecode preserves the cyclic SSA for its bounded interpreter. Neither backend
defines iteration legality. Unsupported lowering remains structured `UNKNOWN`.

## Evidence boundary

CRE-3 reconstructs three-attempt retry over `PASS`, `FAIL`, and `UNKNOWN`, including early success,
terminal failure, state advancement, exact exhaustion, and authority-bearing calls. Agreement over
the frozen corpus is bounded observation, not universal equivalence, conformance, independent
evaluation, or production assurance.

## Non-goals

This RFC does not add general loops, general recursion, nested iteration, exact cost analysis,
parallel attempts, unrestricted effects, records, memory, or a final grammar/type theory.
