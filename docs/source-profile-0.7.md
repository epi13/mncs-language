# Source Profile 0.7 — bounded data: bytes, fixed sequences, bounded views, traversal

Status: **experimental**, introduced 2026-08 by the Ox Alpha bounded-data
tranche (RFC 0044). Profiles 0.1–0.6 are unchanged; every earlier fixture
still parses, elaborates, and executes identically, including payload sums
and imports.

## Surface additions

### `byte` — a byte-oriented logical value

```mncs
fn mix(a: byte, b: byte) -> (result: byte) {
    return ((a ^ b) | (a & b)) << 1;
}
```

`byte` is an 8-bit unsigned byte-oriented value, deliberately distinct from
`u8`. Supported: bitwise `& | ^`, shifts `<< >>`, comparisons, equality,
explicit conversions. Not supported: arithmetic — use `as` to cross into
integers so the direction of every numeric step is visible at the call site.
Shift counts (`u64`) are taken modulo 8; both shifts are logical. All byte
operations are total by definition and leave no obligations.

### Fixed sequences and bounded views

```mncs
let xs: [i32; 4] = [10, 20, 30, 40];
let view: [i32; up_to 4] = xs[2..5];
```

- `[E; N]` is an exact-length sequence; `[E; up_to M]` is a bounded view
  with a runtime length observation. Both carry their bounds as part of
  type identity; `0 ≤ N, M ≤ 64`.
- Element types: scalars, `byte`, declared nominal types, nested sequence
  types (`[[u8; 2]; 3]`).
- Construction `[e0, ..., eN-1]` must supply exactly the declared count;
  underfill/overfill is an elaboration error (`MNE184`). A bare literal
  without an expected exact type is rejected (`MNE183`).
- Sequences have pure value semantics: no mutation, aliasing, or ownership
  transfer exists in this profile, so ownership/lifetime theory stays
  explicitly unresolved (RFC 0009).

### Element observation with three-state bounds evidence

```mncs
let head: i32 = xs[0];        // literal inside exact bound: StaticExact
return xs[7];                 // elaboration error MNE192 (provably invalid)
iterate i over xs ... { f(xs[i]); }   // TraversalDomain
let x: i32 = view[k];         // RuntimeChecked (obligation UNKNOWN)
```

Bounds facts are machine knowledge carried through body, HIR, SSA,
obligations, and evidence:

| Evidence | Meaning | Realization |
| --- | --- | --- |
| `StaticExact` | literal index inside an exact bound | plain access, no check |
| `TraversalDomain` | index bound of enclosing `over` traversal | plain access, no check |
| `RuntimeChecked` | validity unknown statically | explicit check; violation is a runtime failure |

A literal index at or beyond an exact bound fails at elaboration (`MNE192`)
instead of deferring a guaranteed runtime failure.

### Views, lengths, ranges

- `xs[start..end]` derives a view under checked semantics: `start ≤ end`,
  `end ≤ len(source)`, `end - start ≤ cap`; violations are deterministic
  runtime failures and the range-validity obligation stays `UNKNOWN` until
  discharged.
- `xs.len` observes the length as `u64` (constant for exact bounds, the
  packed runtime length for views).

### Bounded traversal over sequences

```mncs
iterate i over data carrying acc: u64 = 0 {
    next acc = acc + (data[i] as u64);
}
```

Traverses exactly `len(data)` steps (≤ the static bound). The index binding
is visible in the body only; element accesses through it need no bounds
check because traversal semantics discharge them. Bound ceiling is the
profile maximum (64); empty exact sequences traverse zero times. Nested
`over` iterations remain rejected (`MNE147`).

### Explicit total conversions

```mncs
let b: byte = (x / 16777216) as byte;
let w: u64 = b as u64;
let flag_count: i32 = (flag as i32);
```

`(e as T)` is a total conversion between scalar widths and `byte`
(booleans convert outward as 0/1 only): narrowing truncates high bits;
widening extends by the source signedness (`byte` zero-extends). Total by
definition; no obligation survives.

### Integer shifts

`<<`/`>>` also shift integers within their declared width: counts are taken
modulo the width, signed `>>` is arithmetic, unsigned `>>` logical, `<<`
wraps into the width. Shifts carry wrapping intent, so they discharge their
own totality obligation.

## Diagnostics

| Condition | Diagnostic |
|---|---|
| bracketed type below 0.7 | `MNP145` |
| malformed sequence type | `MNP146`–`MNP149` |
| `as` below 0.7 / missing type | `MNP150`, `MNP151` |
| sequence literal below 0.7 | `MNP152` |
| indexing/slicing below 0.7 | `MNP153` |
| `iterate over` below 0.7 | `MNP144` |
| bare sequence literal without expected type | `MNE183` |
| underfilled/overfilled literal | `MNE184` |
| element type mismatch | `MNE185` |
| non-sequence indexing / slicing | `MNE186`, `MNE187` |
| view type mismatch | `MNE188` |
| non-scalar conversion | `MNE189` |
| shift on non-byte/non-integer | `MNE190` |
| arithmetic on bytes | `MNE191` |
| literal index outside exact domain | `MNE192` |

## Realization envelope (observed 2026-08)

| Stage | byte ops | `[E; N]` | `[E; up_to M]` | traversal | casts/shifts |
| --- | --- | --- | --- | --- | --- |
| body/SSA reference executors | realizes | realizes | realizes | realizes | realizes |
| research bytecode | realizes | realizes (incl. arguments/results) | realizes | realizes | realizes |
| portable WASM MVP | realizes | cells, including language-level args/results | packed descriptor, including args/results | realizes | realizes |
| C11 | realizes | cells, including language-level args/results | packed descriptor, including args/results | realizes | realizes |
| LLVM IR | realizes | cells, including language-level args/results | packed descriptor, including args/results | realizes | realizes |
| Cranelift | realizes | cells, including language-level args/results | packed descriptor, including args/results | realizes | realizes |
| RISC-V/eBPF/PTX | via shared LLVM lowering; artifact production + structural validation only |

Sequence-typed and view-typed function parameters/results now cross the
process boundary through the canonical cell / packed-descriptor contract
used by records. Nested composite sequence elements remain an explicit
later step (`nested_composite_sequence_elements` is still unsupported).
eBPF additionally refuses signatures beyond its five-register calling
convention (`CGX410`). Backend refusal remains an evidence-backed
envelope, not semantic disagreement.

Differential agreement corpora:
`examples/execution/profile07-bounded-data-corpus.json`,
`examples/execution/profile07-serialization-corpus.json`,
`examples/execution/library-core-sequences-corpus.json`,
`examples/execution/library-std-encoding-corpus.json`.

## Identity compatibility

All new fields serialize as absent when default (`IterationDomain`,
`over_source`), so canonical identities of Profile ≤ 0.6 programs are
unchanged. Verified by the full fixture corpus passing without migration.
