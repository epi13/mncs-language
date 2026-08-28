# Source Profile 0.8 — branchless selection, masks, and semantic integer vectors

Profile 0.8 is additive over Profile 0.7. It introduces branchless selection
as a semantic operation, a machine-independent `vec<T, N>` family, and logical
`mask<N>` predicates. These are language values, not commitments to a register
width, ABI, or storage layout.

## Types and bounds

```mncs
let values: vec<i32, 4> = vector(a, b, c, d);
let active: mask<4> = vec_gt(values, splat(0));
let kept: vec<i32, 4> = select(active, values, splat(0));
```

`N` is a literal compile-time lane count in `1..=64`. Canonical semantic
spellings are `vec<T, N>` and `mask<N>`. The initial executable vector envelope
accepts integer lanes. A vector is distinct from `[T; N]`: the latter is a
bounded logical sequence; the former carries lane and predication semantics.
No implicit sequence/vector reinterpretation exists in this profile.

## Operations

Construction and lane access:

- `vector(...)`, `splat(value)`
- `extract_lane(vector, index)`
- `replace_lane(vector, index, value)` (functional; the input is unchanged)

Arithmetic and integer lane operations:

- `vec_add_wrap`, `vec_add_checked`, `vec_add_sat`
- `vec_sub_wrap`, `vec_sub_checked`, `vec_sub_sat`
- `vec_mul_wrap`, `vec_mul_checked`, `vec_mul_sat`
- `vec_and`, `vec_or`, `vec_xor`, `vec_shl`, `vec_shr`
- `vec_min`, `vec_max`

Comparisons `vec_eq/ne/lt/le/gt/ge` return `mask<N>`. Mask composition uses
`mask_and/or/xor/not`; `mask_any/all/none` reduce to `bool`. `select(mask, a,
b)` blends vectors lane by lane. `reduce_sum_wrap`, `reduce_sum_checked`,
`reduce_min`, and `reduce_max` use a fixed left-to-right lane order.

Every arithmetic operation retains its `ArithmeticIntent` through semantic
body, HIR, SSA, obligations, reference execution, and lowering. Checked and
saturating operations are lane-wise; a checked failure in any lane fails the
operation. SIMD never implies host-language overflow behavior.

## Branchless is not constant-time

Scalar `select(condition, true_value, false_value)` and mask selection survive
as explicit operations. Both candidates are evaluated eagerly. A branchless
realization request starts UNKNOWN and is only discharged by independent
artifact evidence. Branchlessness does not establish constant-time behavior,
which also depends on target timing, memory access, speculation, and the wider
execution context.

## Realization envelope

The research backend executes semantic lanes and masks directly. Portable WASM
uses immutable packed lane cells and a packed `i64` mask; C11, LLVM, and
Cranelift provide conservative scalarized fixed-lane fallbacks over canonical
8-byte arena slots. Scalar `select` lowers to WASM `select`, LLVM `select`,
Cranelift `select`, and a branch-free unsigned blend in portable C.

Logical `vec<T, N>` and `mask<N>` values now cross language-level signatures
on the five executable backends through the canonical cell / packed-bit
process boundary (see
`docs/development-evidence/sequence-view-boundary-abi-2026-08.md`). That is
logical ABI support, not a native vector calling convention.

This profile still does **not** claim:

- WASM SIMD `v128`
- LLVM native vector IR
- Cranelift SIMD
- PTX predication
- float vectors
- sequence/vector memory views as a second layout
- a target-specific SIMD register ABI

Current realizations remain conservative: research bytecode interprets
semantic lanes; portable WASM uses packed lane cells and a packed `i64`
mask; C11, LLVM, and Cranelift scalarize over canonical 8-byte slots.
Native SIMD selection is a later target-specific step. External
PTX/eBPF/RISC-V adapters remain precisely unsupported for these
operations.

## Spatial and representation pilot

The model now has executable late-bound `AoS`, `SoA`, and `AoSoA<N>` candidates
that retain one logical type identity. Selection consumes exact contiguity,
target, protected-observation, and current subject/scope-bound alignment facts;
UNKNOWN alignment keeps the explicit fallback. Physical field order, address,
size, alignment, ABI, and serialized-byte observations constrain eligible
changes. A bounded `DisjointCapability` can establish two non-overlapping
half-open ranges only within the same allocation epoch.

This pilot is compiler-model machinery, not frozen surface syntax and not yet
consumed to choose a native vector load/store.

## Diagnostics

Profile gating and elaboration fail closed with `MNP163`–`MNP166` and
`MNE201`–`MNE216`. Important cases include a missing construction context,
lane-count mismatch, statically out-of-range lane, mismatched vector or mask
lanes, non-integer arithmetic lanes, and invalid reduction operands.

## Evidence and limitations

The committed branchless and vector corpora compare body/reference,
SSA/reference, and five executable backends. Agreement is finite empirical
evidence, not a proof; overall experiments remain UNKNOWN while generated
machine-intent obligations are unresolved. See
`docs/development-evidence/source-profile-0.8-performance-substrate-2026-08.md`.
