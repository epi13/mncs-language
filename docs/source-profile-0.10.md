# Source Profile 0.10 — explicit bounded polymorphism

Status: **implemented but partial, experimental** (2026-08-28). Profile 0.10
is additive over Profile 0.9 and introduces only explicit function-level type
and natural-number parameters. It does not add inference, traits, implicit
evidence, higher-kinded parameters, reflection, or unbounded type-level
computation.

## Surface

Generic parameters are declared in source order:

```mncs
fn first<T, N: Nat>(xs: [T; N]) -> (result: T) {
    return xs[0];
}
```

Calls provide every argument explicitly. Type arguments may be scalars,
declared finite/record types, bounded sequences, views, vectors, or masks
where the active profile supports that type; `Nat` arguments are literals or
forwarded `Nat` parameters:

```mncs
return first<Point, 2>(points);
```

`N` is bounded by `MAX_SEQUENCE_BOUND` (currently 64). Exact sequences use
`[E; N]`; bounded views use `[E; up_to N]`. A view's runtime length may be
zero even when its capacity is nonzero, so empty-view behavior remains an
explicit source-level case.

## Elaboration and identity

The frontend resolves generic declarations and explicit arguments before
body validation. The model records type parameters as `GenericParam` and
value parameters as typed `Nat` arguments. Canonical instantiation arguments
preserve declared parameter order and retain nominal record/finite identities;
source-friendly names are not used as a substitute for nominal identity.

Specialization is deterministic and fixed-point: each concrete instantiation
is emitted once, nested concrete generic calls are queued, call sites are
rewritten to the specialized function, and provenance links the generic
declaration, instantiation, and specialization through HIR and SSA.

Recursive or expanding specialization is refused with a bounded diagnostic
(`MNE227`). Generic calls with missing, extra, mismatched, non-constant, or
out-of-range arguments are rejected (`MNE220`–`MNE225`); higher-kinded and
other unsupported constraints fail closed (`MNE229`).

## Lowering boundary

Concrete SSA is checked immediately before every executable backend adapter.
The shared boundary rejects generic function templates, unresolved
`GenericParam`/generic names, parameterized sequence bounds, non-concrete
specialization records, and specialization records that do not resolve to a
generic declaration plus a concrete function. Individual backend checks are
defensive and unreachable for a valid compiler pipeline; they remain in place
to protect direct adapter callers.

The same boundary is exercised by research bytecode, portable WASM, C11,
LLVM IR, and Cranelift. Artifact-only RISC-V/eBPF/PTX paths are outside the
Profile 0.10 executable-backend claim and continue to report their existing
capability/refusal states.

## Compatibility and non-claims

Profile 0.9 programs retain their prior meaning. Existing arity-specific
stdlib wrappers remain available, while Profile 0.10 library primitives are
the authoritative generic implementations. This tranche is not a proof of
parametricity, coherence, generic evidence passing, layout independence, or
universal backend equivalence. Runtime-checked bounds and exact instruction
cost continue to produce `UNKNOWN` where the evidence contract requires it.

See [RFC 0013](../rfcs/0013-machine-native-abstraction-polymorphism-interface-evidence-semantics.md)
and [the Profile 0.10 evidence record](development-evidence/source-profile-0.10-generics-2026-08.md).
