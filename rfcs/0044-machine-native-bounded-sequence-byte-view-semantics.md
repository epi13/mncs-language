# RFC 0044: Machine-Native Bounded Sequence, Byte, and View Semantics

- **Status:** Accepted for Source Profile 0.7 (experimental)
- **Target:** mncs-language 0.7 tranche
- **Depends on:** RFC 0009, RFC 0019, RFC 0021, RFC 0022, RFC 0027, RFC 0041, RFC 0043

## Summary

MNCS gains its first native bounded data substrate: a byte-oriented logical
value (`byte`), fixed-length sequences (`[E; N]`), bounded views
(`[E; up_to M]`), explicit total conversions (`as`), and bounded traversal
over a sequence domain (`iterate i over xs ...`).

The substrate is deliberately **not** arrays/slices/pointers from conventional
languages. It follows RFC 0019: a sequence value is semantic information —
its element values and length facts. Contiguous storage, one ABI, packing,
boxing, or pointer shape are *realization choices* made by backends (and
eventually Forge), never part of logical meaning.

## Constitutional principles

1. **Bounds are semantic facts, not runtime afterthoughts.** An exact length
   `N`, a maximum length `M`, an index domain, and a range domain are carried
   by types and preserved into bodies, HIR, SSA, obligations, and evidence.
2. **Index safety has three epistemic states.**
   - `static_exact`: the index is a literal within the declared exact bound.
     No runtime check exists; the fact is recorded as evidence.
   - `traversal_domain`: the index is the binding of an enclosing
     `iterate ... over` domain; the traversal semantics discharge the check.
   - `runtime_checked`: validity is unknown statically; an explicit bounds
     obligation remains `UNKNOWN` until discharged and realization performs a
     deterministic bounds check whose failure is an explicit runtime failure.
3. **Value semantics.** Sequences and views are immutable logical values.
   There is no mutation, no aliasing, no ownership transfer in this tranche.
   A view observes its source's elements through its declared range; whether a
   realization copies or shares storage is unobservable through permitted
   observations, so both remain valid realizations (RFC 0019 §representation).
   Ownership/lifetime theory (RFC 0009) stays explicitly unresolved.
4. **Views are bounded.** A view type `[E; up_to M]` carries its maximum
   length `M` as machine knowledge. Its runtime length `ℓ ≤ M` is a first-class
   value observation. Slicing validates `start ≤ end ≤ len(source)` and
   `end - start ≤ M` under checked semantics.
5. **Byte is not a number.** `byte` is an 8-bit unsigned byte-oriented value.
   Bitwise operations, shifts, equality, and ordering are defined; arithmetic
   is not. Crossing between bytes and integers requires an explicit `as`
   conversion whose direction (truncate/extend) is total and visible.
6. **No hidden allocation, no unbounded growth.** Every sequence type declares
   a finite bound (0–64 in this profile). Construction supplies exactly the
   declared element count. Traversal executes at most `bound` steps.
7. **Canonical cells are one realization, not the definition.** Backends may
   realize fixed sequences as canonical cell slots, packed view descriptors,
   interpreter vectors, or future layouts. The logical sequence object is the
   same language object across all of them.

## Surface syntax (Profile 0.7)

```mncs
mncs 0.7;

fn checksum(data: [byte; 8]) -> (result: u64) {
    iterate i over data carrying acc: u64 = 0 {
        next acc = acc + (data[i] as u64) * 31;
    }
    return acc;
}

fn head_tail(xs: [i32; 4]) -> (result: i32) {
    let head: [i32; up_to 2] = xs[0..2];
    return head[0] + xs[3];
}
```

- Type forms: `[E; N]` (exact), `[E; up_to M]` (bounded view), with
  `0 ≤ N, M ≤ 64`; `E` is any scalar, byte, declared nominal type, or nested
  sequence type. Canonical spelling is exactly these forms.
- `[e0, e1, ...]` constructs a sequence; the element count must equal the
  expected exact length (underfilled/overfilled are elaboration errors).
- `xs[i]` projects element `i`. `xs[start..end]` derives a view.
- `xs.len` observes the length as `u64` (constant for exact sequences).
- `(e as T)` converts between scalar widths and `byte`; total, documented.
- `b & c`, `b | c`, `b ^ c`, `b << k`, `b >> k` operate on `byte`
  (`k: u64`, shift count taken mod 8; shifts are total).
- `iterate i over xs carrying s: T = init { ... next s = n; }` traverses
  exactly `len(xs)` steps (≤ the static bound); inside the body `i: u64` is
  the traversal index and `xs[i]` needs no bounds check.

All new syntax requires Profile 0.7; earlier profiles fail closed with coded
diagnostics. Existing profiles 0.1–0.6 are unchanged bit-for-bit.

## Realization envelope (initial)

| Stage | byte | [E; N] | [E; up_to M] | traversal | casts |
| --- | --- | --- | --- | --- | --- |
| body / SSA reference executors | realizes | realizes | realizes | realizes | realizes |
| research bytecode | realizes | realizes (incl. arguments/results) | realizes | realizes | realizes |
| portable WASM MVP | realizes | internal cells | packed descriptor, internal | realizes | realizes |
| C11 | realizes | internal cells | packed descriptor, internal | realizes | realizes |
| LLVM IR | realizes | internal cells | packed descriptor, internal | realizes | realizes |
| Cranelift | realizes | internal cells | packed descriptor, internal | realizes | realizes |
| RISC-V / eBPF / PTX artifacts | via shared LLVM lowering; artifact production only | | | | |

Sequence- or view-typed function parameters/results refused with coded
diagnostics on the four scalar backends in the original 0.7 landing
(the process-boundary marshal contract did not yet carry them); research
bytecode accepted them at language-level signatures. Those refusals were
capability-envelope facts, not silent approximations.

**Subsequent realization (2026-08):** exact sequences, views, vectors, and
masks now cross the process boundary through canonical cells and packed
descriptors on portable WASM, C11, LLVM IR, and Cranelift. Nested composite
sequence elements remain unsupported. See
`docs/development-evidence/sequence-view-boundary-abi-2026-08.md`.

## Non-goals

Unbounded/dynamic sequences, mutation, ownership/borrowing, general loops,
nested `over` iteration, string/text types, maps/sets, generalized generics,
and wire-format freezing (RFC 0027 canonical encoding remains future work;
this tranche only provides its byte substrate).
