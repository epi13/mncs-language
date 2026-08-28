# Source Profile 0.8 performance-substrate evidence — 2026-08

## Scope

This record covers the rescued branchless work and the bounded integer
vector/mask vertical slice. It does not claim native SIMD, general
autovectorization, floating-point semantics, or general alias analysis.

## Interrupted work preservation

The branch began with 13 modified tracked files implementing incomplete
Profile 0.8 scalar `select` and functional sequence `replace`. Generic backend
matches were non-exhaustive and the workspace did not compile. The exact state
was preserved as commit `8a5898b` before reconciliation. The completed path now
uses genuine WASM/LLVM/Cranelift selection and a branch-free C bit blend;
sequence replacement allocates a fresh canonical cell, copies every slot,
checks dynamic bounds, and writes only the fresh cell.

## Semantic and compiler path

`vec<T, N>` and `mask<N>` retain element/lane identity across source, body,
HIR, SSA, identity graphs, obligation generation, and both reference
executors. Masks remain logical lane predicates in semantics and become packed
bits only in the applicable backend realization. Arithmetic intent is carried
per vector operation. Lane access records static or runtime-checked bounds
evidence. Masked selection is eager and does not become source-level `if`.

The representation pilot separately models logical identity and `AoS`, `SoA`,
or `AoSoA<N>` realization identity. Exact current alignment can authorize a
preferred candidate; missing evidence produces UNKNOWN and retains the
fallback. Exact half-open ranges in one allocation epoch can establish the
bounded disjointness pilot.

## Differential execution

Commands used the standard experiment runner with
`examples/source/profile08-vectors.mncs` and its 12-case corpus. Every case met
its expected result and layered translation validation returned PASS:

| backend | experiment status | translation validation | artifact bytes |
| --- | --- | --- | ---: |
| research bytecode | UNKNOWN | PASS | 178102 |
| portable WASM MVP | UNKNOWN | PASS | 3081 |
| C11 (external clang) | UNKNOWN | PASS | 32022 |
| LLVM IR (external clang) | UNKNOWN | PASS | 87319 |
| Cranelift | UNKNOWN | PASS | 197920 |

The UNKNOWN experiment status is intentional: unresolved proof obligations
are retained, while bounded execution agreement is PASS. Artifact sizes are
workload/build observations only and are not cross-format performance claims.

The 4-case scalar branchless corpus also agrees across the same five backends.
`crates/mncs-cli/tests/profile08_branchless_vectors.rs` makes both matrices and
four negative fixtures regression tests.

## Backend realizations

- Research bytecode: complete semantic integer-vector/mask reference path,
  including vector/mask-typed language-level signatures.
- WASM MVP: packed `i64` masks and packed i32/i64 lane cells, scalarized fixed
  lanes; language-level vector/mask signatures marshal through linear memory.
  No `v128` claim.
- C11: portable canonical-cell scalar fallback and call-file packed-mask
  words; no compiler vector extension.
- LLVM: conservative scalarized LLVM IR fallback; scalar select is native
  LLVM `select`; vector/mask signatures use canonical cells / packed bits.
  No native vector-IR claim.
- Cranelift: conservative scalarized JIT/CLIF fallback with native scalar
  select; vector/mask signatures use the same cell/call-file boundary.
  No SIMD claim.
- PTX/eBPF/RISC-V: unsupported for semantic vectors/masks; no fabricated
  lowering or execution result.

## Bugs exposed and fixed

1. The interrupted generic lowerers omitted new `select`/`replace` matches.
2. `select` elaborated its true candidate twice when no expected type existed.
3. WASM scalar vector binary lowering advanced source offsets but not the
   destination offset, repeatedly overwriting lane zero.
4. C11 total-shift emission accidentally emitted the Rust identifier `bits`
   into generated C for 64-bit mask shifts.
5. LLVM scalar selection hard-coded an `i32` condition even for packed masks.
6. Cranelift canonical 32-bit slot loads were zero-extended and lost signed
   integer semantics before comparisons.

Each general defect is covered by the Profile 0.8 differential corpus.

## Joern graph analysis

Joern 4.0.583 was invoked before source edits. Its wrapper rejected `rust`,
auto-detection searched for C sources, and the direct frontend initially failed
inside `AstGenRunner`. The exact pre-edit state remained available as safety
commit `8a5898b`, so the baseline was reconstructed from that commit in a
detached worktree. Redirecting JVM/native temporary extraction away from the
quota-full system `/tmp` allowed the real Rust frontend to complete for both
snapshots.

Both CPGs received CFG, dominator, control-dependence, and reaching-definition
overlays. Focused query results were:

| query | `8a5898b` baseline | post-edit |
| --- | ---: | ---: |
| methods | 1719 | 1776 |
| focused vector/layout methods | 0 | 12 |
| calls whose name contains `select` | 9 | 9 |
| calls whose code contains `reduce` | 0 | 35 |

The 12 post-edit focused methods are `elaborate_vector_intrinsic`, six
`lower_vector_*` helpers, three WASM `emit_vector_*` helpers,
`select_representation`, and `DisjointCapability::derive_static`. Joern warned
that its Rust frontend does not yet handle several Rust AST node families
(`use`, enums, matches, closures, and others) and reported unresolved member
links. The graph therefore confirms focused method/control-flow presence and
overlay construction, but it is not complete enough for a universal
reachability or data-flow equivalence claim.

## Process-boundary follow-up

Logical vector and mask values now also cross language-level signatures
on the five executable backends. That is a canonical-cell / packed-bit
ABI claim, not a native SIMD claim:

- masks use the call-file protocol as packed 64-bit words even when they
  do not allocate arena cells; they are not decimal argv scalars
- unsigned `u64` cells reconstruct from declared signedness, so values
  `>= 2^63` stay in the unsigned domain through WASM and native arena
  decoding
- ABI regression tests require the returned logical value, not only the
  execution status

See `docs/development-evidence/sequence-view-boundary-abi-2026-08.md`.

## Deferred work

Native WASM SIMD, LLVM/Cranelift vectors, floats, sequence/vector conversion,
alignment-consuming loads, selected-SSA autovectorization, and a materializing
fusion rewrite remain future work. `mncs.std.simd.v1` supplies a language-owned
affine/ReLU pressure kernel, but this tranche does not mislabel its ordinary
vector chain as an evidence-bearing fusion transformation.
