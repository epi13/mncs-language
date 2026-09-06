# Executable semantic contracts

Status: **implemented** (Profile 0.9 addition, 2026-09). This spec defines how
an MNCS operation's intended observable semantics becomes machine-consumable
evidence instead of documentation prose.

## 1. Problem

`requires`/`ensures` clause names are opaque strings: nothing checks them,
nothing executes them, and no tool can derive what to test from them. The
semantic core (`spec/semantic-core.md` section 4) anticipated executable
kinds (`invariant`, `preserves`, `budget`) but source could not express them,
and no clause kind was bound to anything a backend could run.

## 2. Executable contract clauses

Three clause kinds name an MNCS predicate function in the same program:

| Kind | Model | Meaning |
| --- | --- | --- |
| `property` | `ContractKind::Property` | boolean predicate the operation's cases must satisfy |
| `invariant` | `ContractKind::Invariant` | boolean predicate maintained across the operation's region |
| `metamorphic` | `ContractKind::Metamorphic` | boolean predicate relating operations (round-trip, involution, commutativity) |

```mncs
fn add(a: i64, b: i64) -> (result: i64)
    property prop_add_sub_roundtrip
    metamorphic prop_add_commutes
{
    return a +% b;
}

fn prop_add_sub_roundtrip(x: i64, y: i64) -> (result: bool) {
    return sub(add(x, y), y) == x;
}
```

Elaboration (`MNE230`/`MNE231`/`MNE233`) guarantees:

1. the clause appears under source profile 0.9 or later (predicates compose
   library functions through namespace-qualified imports, which need the 0.9
   binding substrate);
2. the named predicate resolves to a function in the same program;
3. that function returns exactly one `bool`.

Legacy `requires`/`ensures`/`assumes` names stay unchecked and keep their
prior meaning. A predicate's parameter list is its own: it executes
standalone with generated inputs, so preconditions, tolerances, structured
input construction (images, scenes), and multi-operation relations all live
in ordinary MNCS code — the clause is only the machine-readable binding.

## 3. Predicate authoring rules

Predicates are total boolean functions subject to the active profile, plus:

- integer literals coerce from their declared context: bind a typed `let`
  before mixing literals with typed values (`let v: u64 = ...; (255 as u64) - v`);
- comparisons are integer-only: decide boolean equivalence by control flow
  (`mncs.core.contracts.v1/iff`), not `==` on bools;
- there is no boolean negation operator: write `if push { ... }` shapes;
- call arguments and `let` values accept chained record projections
  (`edges.samples[i]`, `frame.nodes[0].id`) since the projection-chain fix;
- structured inputs come from MNCS case constructors inside the predicate
  (e.g. `render_rect_outline(...)` builds the frame the observer checks);
- tolerances are explicit predicate parameters via
  `mncs.core.contracts.v1/within_tolerance_u64` (see section 5).

## 4. Conformance pipeline

`mncs conformance <program.mncs|program.json>` implements:

```text
contract clause + predicate
  -> deterministic case generation (boundary classes, then seeded MMIX-LCG stream)
  -> reference execution (twice; mismatch is FAIL, not flake)
  -> backend execution (each requested backend lowers the same SSA)
  -> PASS only when the predicate returns true on every executed backend
  -> mncs.conformance-report/1 evidence (no wall-clock; content-addressed)
```

Verdict vocabulary is honest by construction:

- `pass`: observed `true` where required;
- `fail`: observed `false`, a differential mismatch, or an execution failure
  where the reference succeeded;
- `unknown`: lowering failed, the toolchain errored, or the reference itself
  did not return (nothing was proven either way);
- `unsupported`: the case cannot be generated (non-scalar parameters,
  generic predicates) or the backend cannot execute (PTX/RISC-V/eBPF
  artifact-only targets report their existing capability states).

`--emit-corpus` re-emits the generated predicate cases as a standard
execution corpus (expected `true`), so `mncs experiment run/compare` and
`mncs check-backend-execution` re-execute exactly what the report observed.

## 4a. Evidence closes the loop (obligations)

`--attach-evidence <evidenced.json>` records one `EvidenceClaim` per clean
predicate on its operation (property = clause id, verifier = generator,
artifact = report sha256). Each claim discharges the compilation
`contract-evidence-bound` obligation for that clause — which is why the
experiment must run on the *evidenced* program:

```text
mncs conformance prog.mncs --emit-corpus corpus.json --attach-evidence evidenced.json
mncs experiment run evidenced.json --corpus corpus.json   # PASS
mncs experiment run prog.mncs --corpus corpus.json        # UNKNOWN (obligations retained)
```

The UNKNOWN on bare source is honest, not a gap: nothing has bound evidence
to those clauses yet. Identity stays stable across attachment on purpose:
evidence claims are verification annotations *about* a function, not semantic
content *of* it, so the canonical form erases them (like binding-table
coordinates) and the report's `subject_fingerprint` does not move when claims
are attached. Re-running conformance on an evidenced program therefore yields
the same report sha256, and re-attaching adds nothing.

## 5. Tolerances are measured, not guessed

Graphical/observer laws carry explicit tolerances because perception is
approximate even when deterministic. The campaign's reference measurement:
the 3x3 Sobel kernel responds one pixel beyond a rendered outline on every
side, so the round-trip law brackets position by 1 and each dimension by 2
(one flank pixel per side). If kernel support changes, the law fails loudly
and the tolerance is re-measured — never silently widened.

## 6. What this is not

- Not formal verification: predicates observe finite generated cases, they
  do not prove universal properties.
- Not a trust root: a contract is a claim; only generated independent
  observations plus provenance constitute evidence. An implementation that
  ships a favorable predicate still has to return `true` under generation.
- Not exhaustive search: bounded generators, equivalence-class sampling,
  and contract-specific domains keep the case space finite and seeded.
