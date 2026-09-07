# RFC 0007 Evidence — Proof-Carrying Dependent Core, Tranche 0.1

This document records what the tranche 0.1 implementation actually is, what it
proves, what it refuses, and what remains bootstrap debt. It is the narrative
companion to the machine-readable claims in `rfcs/conformance-ledger.json`
(RFC 0007, criteria `0007-C1`–`0007-C20`).

Design status stays **DRAFT**: universe cumulativity, `Prop` separation,
erasure, full substitution, and the exact dependent calculus remain research
decisions per the RFC. Implementation status is **BOUNDED_IMPLEMENTATION**:
a real, executed, differentially checked slice — not the full vision.

## Architecture

```text
MNCS source program
        ↓  (compiler generates obligation O, status UNKNOWN)
untrusted proof generation (developer, script, solver, agent)
        ↓  (candidate proof artifact P: flat cells + claim)
MNCS proof kernel  ←→  independent reference checker
   check_proof_code        reference_check
        ↓  (both must agree; agreement is consistency evidence, not proof)
PASS evidence bound to O, P, kernel version, assumptions, dependencies
        ↓
compiler consumes the binding (proof-gated SSA elision,
proof-bound lowering certificates)
```

Proof generation is untrusted; proof checking is the narrow trust boundary.
The two checkers share no code. They share only SHA-256 content identity,
canonical JSON serialization, and — for executed runs — the MNCS compiler
itself. That shared surface is documented below, not hidden.

## The calculus (tranche 0.1)

Terms are flat `[Cell; 16]` buffers with an active prefix. Every child index
is strictly smaller than its parent (topological order), so checking
terminates by construction — no fuel, timeout, or recursion participates in
meaning.

| Tag | Arguments | Type rule |
| --- | --------- | --------- |
| `Universe(l)` | level immediate, `0..=3` | `Universe(l+1)`; `l = 3` fails closed |
| `Nat` | — | `Universe(0)` |
| `Zero` | — | `Nat`; literal value `0` |
| `Succ(p)` | `p : Nat` | `Nat`; literal value where closed |
| `Plus(a, b)` | both `: Nat` | `Nat`; closed primitive computation |
| `Var(p)` | binder cell `p`, a `Pi`/`Lambda` below | domain denotation of `p` |
| `Pi(d, c)` | `d`, `c` both types | `Universe(max)`; fully dependent formation |
| `Lambda(d, b, q)` | `q` a `Pi` cell whose domain matches and whose codomain denotes the body type | the claimed `Pi` (verified, never computed) |
| `Apply(f, a)` | `f : Pi`, argument matches domain | codomain denotation, or `UNKNOWN` when dependent |
| `NatElim(m, z, s, n)` | motive `Pi` over `Nat`, matching branches, `n : Nat` | zero-branch type on closed-zero iota, else `UNKNOWN` |
| `Eq(T, l, r)` | `T` a type, both sides `: T` | `Universe(level of T)` |
| `Refl(w, q)` | `q` an `Eq` cell | claim-driven (see below) |
| `Unsupported` | — | `UNKNOWN` |

Variables use globally unique binder identities (the binder cell index): the
Barendregt convention made explicit. A variable can only name a `Pi`/`Lambda`
cell below it, and scope extrusion degrades to weakening, never capture.

Definitional equality is shared representatives plus closed-literal values:
two sides are equal when their canonical indices coincide or when both are
closed `Nat` literals with the same value. This is sound and deliberately
incomplete — structurally rebuilt but distinct terms compare unequal, so
generators share cells.

The claim check verifies `proof : prop`. `Refl` proofs are verified against
their `Eq` proposition (both sides definitionally equal to the witness, the
witness typed by the carrier, and the equation identical to the claimed
proposition cell). Every other proof term must synthesize exactly the denoted
descriptor. Open proof terms (a variable occurs in the proof) resolve to
`UNKNOWN`: assumption accounting is a future tranche. Out-of-range claims
fail closed.

## Explicit UNKNOWN boundaries (gaps 0007-G1–G3, G6–G7)

- **G1** — Dependent application needs substitution; the kernel reports
  `UNKNOWN` instead of computing it.
- **G2** — `NatElim` over `Succ`-literal or stuck scrutinees reports
  `UNKNOWN`; only closed-zero iota computes.
- **G3** — Open proof terms report `UNKNOWN`; no assumption smuggling.
- **G6** — Proof erasure semantics are unimplemented.
- **G7** — One-step beta reduction is deferred (application representatives
  stay stuck; non-dependent typing is still precise).

`UNKNOWN` never upgrades to `PASS`: every combination goes through the
`mncs.core.status.v1` dominance lattice (`FAIL` dominates `UNKNOWN` dominates
`PASS`).

## The two checkers

- **MNCS-native kernel** (`library/core/proof_term.mncs`,
  `library/core/proof_check.mncs`, ~644 lines of MNCS): a single forward pass
  over the buffer carrying a memo of representatives, descriptors, literal
  values, and mention flags. All dynamic lookups are clamped and gated, so
  adversarial buffers fail closed instead of trapping.
- **Independent reference checker** (`crates/mncs-model/src/proof_kernel.rs`):
  iterative tables in Rust implementing the same rules. No code is shared with
  the MNCS kernel.

Differential agreement on the curated corpus (8 cases: valid theorems,
wrong-side equality, forward references, universe breach, dependent-lambda
`UNKNOWN`, Pi formation, wrong proof shape) is exact, and both checkers agree
on all 120 adversarial fuzz cases (19 `PASS`, 88 `FAIL`, 13 `UNKNOWN` by the
reference count). The fuzz corpus doubles as a robustness proof: all 120
inputs return normally on every backend — the kernel never traps.

## Bootstrap inventory (Rust that must migrate toward MNCS)

| Item | Location | Why not MNCS yet | Migration path |
| ---- | -------- | ---------------- | -------------- |
| Canonical JSON + SHA-256 identity | `canonical.rs`, `proof_kernel.rs` sealing | No byte/string/hash vocabulary in MNCS profiles | profiles with byte-level crypto boundary; keep identity outside the kernel even then |
| Reference checker tables | `proof_kernel.rs` `advance_reference` | Intentionally a second implementation in a second language | never migrate: checker diversity is the point |
| Proof/obligation binding + `reusable_if` | `proof_kernel.rs` `ProofBinding` | Fingerprint maps need richer value semantics | `mncs.core.identity.v1` digests once maps land |
| Kernel-backed verifier results | `proof_kernel.rs` `kernel_backed_range_result` | Verifier plumbing is host-side | keep host-side; consume from MNCS policy later |
| Proof-gated SSA elision | `ssa.rs` | SSA transforms are compiler internals | keep; the consumed *fact* is kernel-checked |
| Corpus generators | `scripts/gen_proof_*.py` | Untrusted generation may live anywhere | keep host-side permanently (generation is untrusted by design) |
| Ledger docs generator | `scripts/gen_rfc_ledger_docs.py` | Projection, not semantics | keep; the *gates* already live in MNCS |

MNCS semantic logic is 632 lines; the non-migratable Rust checker plus
binding logic is ~730 lines including 11 unit tests. The MNCS source
is never a wrapper around the Rust checker: both checkers run independently
and must agree.

## Obligation integration

Compiler-generated `integer-overflow` obligations (status `UNKNOWN`) are
discharged by a kernel proof of the closed constant computation conjoined
with an exact machine-integer evaluation. The conjunction is explicit in
`kernel_backed_range_result` and in the `KernelProof` authority class: the
Nat proof covers computation identity, the exact evaluation covers the
machine range, and neither is presented as the other.

The flagship demonstration (`docs/rfc-0007-demo.md`) runs the full pipeline:
source program, real obligation, proof term, dual checking, evidence binding,
compiler consumption, and invalidation on mutation.

## Measurements (tranche 0.1)

- MNCS kernel: 74 lines (`proof_term`) + 558 lines (`proof_check`); gate
  module 104 lines (`rfc_status`).
- Curated corpus: 8 cases, ~3.1k–4.2k reference steps each; identical
  verdicts on research bytecode, portable WASM, C11, LLVM IR, Cranelift.
- Fuzz corpus: 120 seeded cases, exact MNCS/reference agreement on every
  verdict, zero traps on every backend.
- Proof artifacts: canonical JSON, `mncs:proof:<sha256>` identities.
- Reference unit tests: 11 in `proof_kernel.rs`, all passing.
- The kernel's own obligation account: 407 generated, 277 statically
  discharged (authority closure, wrapping intent, iteration ceilings), 130
  unresolved at compile time with runtime enforcement (75 machine-intent
  bookkeeping, 38 checked-arithmetic remnants dominated by the `0 - 1`
  negative-literal idiom, 13 runtime-checked indexing/updates, 4 cost
  evidence). All 128 corpus executions return normally, so every one of those
  fails closed at runtime. Negative literals (or constant-obligation
  discharge) would clean this account; recorded as follow-up pressure, not a
  soundness issue.

## Evidence honesty

`proof`, `certificate`, `verifier result`, `translation validation`, `bounded
agreement`, `test pass`, `empirical observation`, `assumption`, and `UNKNOWN`
remain distinct. Backend agreement is bounded observation, never a theorem.
The `KernelProof` authority class keeps kernel verdicts separate from
diagnostic observations: only `Verifier` and `KernelProof` classes can
satisfy obligations, and kernel proofs additionally require exact binding
reuse.

## Heterogeneity note

The differential suite (`curated` 8 cases plus `fuzz` 120 cases, each
executed on all five backends and compared against the reference checker)
passes identically on Linux x86-64 and Windows x86-64 CI runners, in addition
to local runs and the Fabric-mediated WASM validation above. That is genuine
execution diversity across operating systems — but it is not Fabric worker
diversity, so ledger criterion `0007-C16` stays partial until a remote worker
checks the frozen artifacts.

## Remaining blockers (highest pressure first)

1. Substitution machinery for dependent application (G1) — the next calculus
   tranche; needs an explicit-substitution or stack-machine design that fits
   bounded MNCS.
2. Assumption accounting for open terms (G3).
3. Proof transport through HIR/SSA as first-class artifacts (G4).
4. File-based proof ingestion into `mncs compile` (G5).
5. Erasure semantics (G6) and beta reduction (G7).

---

# Tranche 0.2 record — genuine dependency (appended; tranche 0.1 above is frozen history)

Tranche 0.2 closes gaps G1, G2, G3, and G7 with a second MNCS-native kernel,
`library/core/proof_dep.mncs` (`mncs.core.proof_dep.v2`, kernel identity
`mncs:proof-kernel:0.2`), specified by `docs/rfc-0007-tranche-02-calculus.md`.
The tranche 0.1 kernel is untouched: the two kernels coexist, one per
vocabulary, with independent corpora and independent reference checkers.

## What changed semantically

- Binder-edge graphs with explicit levels (Barendregt by construction,
  checked): `Pi`/`Lam`/`Hyp` declare a level, `Var` names one. Capture is
  impossible — there is no shifting, renaming, or freshening anywhere.
- Genuine substitution by environment extension (explicit-substitution
  machine), beta and weak-head evaluation with fuel, structural
  definitional equality over values with rigid levels for binders.
- Dependent `NatElim` with motive application through the motive `VLam`
  term value (never through the synthesized `Pi` type's value-form
  codomain — that confusion cost one debugging round; see the bring-up log
  in the calculus document).
- `Hyp` assumption declarations with buffer-global scope, closed carrier
  types, root-ness (declarations are not subterms), and a base-33
  assumption code over buffer order. Open proofs check and report the exact
  assumption set they consume.
- `FAIL > UNKNOWN > PASS` through `mncs.core.status.v1`, unchanged.
  Malformed structure is `FAIL`; undecided structure (stuck neutrals,
  opaque proofs, fuel/pool exhaustion) is `UNKNOWN`.

## Bring-up cost log (abridged; full log in the calculus document)

1. Assumption uses failed lookup: `Hyp` levels never enter lexical
   environments (roots have no body edge). One-gate fix in `adv_var`;
   verified by `assumption-open-refl` flipping 1 to 0.
2. The flagship saturated the 64-node value pool (measured high-water 64
   with a temporary probe, since removed). One wider array is
   inexpressible (`MAX_SEQUENCE_BOUND` is 64), so the pool is split-range:
   ids 6..127 across two 64-wide banks (122 nodes), with the substitution
   node map split the same way. Post-split the flagship passes, so its
   need fits in 122 (exact post-split high-water not re-measured).
3. A mechanical bank-migration transposed `vcn_at` id/slot arguments —
   type-invisible (all `i64`), caught immediately by the flagship flipping
   0 to 1 in the differential corpus. The corpus earned its keep.

## The two checkers (tranche 0.2)

- **MNCS-native kernel** (`library/core/proof_dep.mncs`, ~2700 lines):
  flat single-pass tables with closures.
- **Independent reference** (`crates/mncs-model/src/proof_dep.rs`, ~2000
  lines): direct recursive evaluation with de Bruijn levels and fuel. No
  code is shared with the MNCS kernel; the calculus document adjudicates
  disputes.
- **Differential agreement**: exact, on all 15 curated cases (closed and
  flagship theorems, beta/defeq probes, assumption accounting, dependent
  elim, four adversarial FAIL cases). The Rust suite additionally covers
  sealed artifacts (`DepArtifact`, `mncs:proof-dep:<sha256>`) and
  assumption-aware bindings (`DepBinding::bind` / `reusable_if`, including
  drift rejection).
- Common-mode surface (unchanged): SHA-256 identity, canonical JSON, and
  the MNCS compiler itself for executed runs.

## Measurements (tranche 0.2)

- MNCS kernel: `library/core/proof_dep.mncs`; reference plus binding
  logic in `crates/mncs-model/src/proof_dep.rs` with 15 unit tests.
- Curated corpus: 15 cases in `examples/execution/proof-dep-corpus.json`,
  generated by `scripts/gen_proof_dep_corpus.py` (generation is untrusted
  by design; expectations are theory-derived, never recorded from runs).
- Research-bytecode execution: 15/15 expectations met, zero traps; the
  flagship checks in ~754k steps, the dearest case (`flagship-assumptions`)
  in ~1056k steps against a 2000k step budget.
- Five-backend agreement: table below (filled when the sweep lands).

## Five-backend agreement (tranche 0.2 corpus)

Executed `examples/execution/proof-dep-corpus.json` (15 cases) on all five
evidence backends via `mncs experiment run`; every backend process exited
0 and every case output matched its pinned expectation byte-for-byte
(divergences counted by an exact-path comparator that fails loudly on any
unreadable shape — no fuzzy matching, no skipped cases):

| backend                   | exit | cases met | divergences | traps |
|---------------------------|------|-----------|-------------|-------|
| mncs-research-bytecode    | 0    | 15/15     | 0           | 0     |
| mncs-portable-wasm-mvp    | 0    | 15/15     | 0           | 0     |
| mncs-c11                  | 0    | 15/15     | 0           | 0     |
| mncs-llvm-ir              | 0    | 15/15     | 0           | 0     |
| mncs-cranelift            | 0    | 15/15     | 0           | 0     |

The independent Rust reference (`cargo test -p mncs-model proof_dep`,
16 tests including the flagship, the assumption accounting, the sealed
artifact round-trip, and a corpus-agreement test over the same 15-case
file) agrees with every pinned expectation, so MNCS-native and reference
verdicts coincide on all 15 cases on all five backends. The harness-level
report status reads `UNKNOWN` on every run because compiling the kernel
module itself retains 950 unresolved compiler obligations (654
integer-overflow bodies, 188 machine-intent, plus sequence-bounds and
iteration-cost kinds — the same standing phenomenon as tranche 0.1's
retained obligations, and unrelated to the per-case proof verdicts);
every per-case expectation is met, every backend process exited 0, and
every stderr capture is empty.

## Remaining gaps after tranche 0.2

G4 (proof transport through every compiler stage), G5 (file-based proof
ingestion), G6 (erasure) stay open. Transport exists only as sealed
artifacts with assumption-aware bindings in `mncs-model` (`DepArtifact` /
`DepBinding`); no compiler stage consumes a tranche 0.2 proof yet, so
criterion 0007-C14 stays partial. Heterogeneous worker evidence (C16)
stays partial: five local backends, no remote worker.
