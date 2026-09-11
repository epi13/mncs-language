# Source Profile 0.14 — buffer pipelines

Status: **implemented, experimental (current)**. Profile 0.14 is
additive over Profiles 0.1–0.13 and is the explicit evolution home
(RFC 0036) for the buffer-pipeline tranche: bulk span copy, checked
view narrowing, and the checked-index discharge form. Older profiles
retain their historical syntax, semantics, and canonical fingerprints;
the `crates/mncs-cli/tests/profile_compat.rs` suite pins both directions
(old-profile refusals and 0.14 admissions).

Predecessor: Profile 0.13 (`docs/source-profile-0.13.md`).
Machine-readable policy: `mncs-syntax` registry
(`crates/mncs-syntax/src/profile.rs`,
`spec/source-profile-registry.json`).

## Bulk span copy (WEB-P-003)

`copy_span(dst, dst_at, src, src_at, len)` is a total functional
bounded span copy: it produces a new sequence equal to `dst` except the
`len` elements starting at `dst_at`, which become the `len` elements of
`src` starting at `src_at`. Neither input is mutated. The destination
must be an exact-bound sequence (views refuse, mirroring `replace`,
`MNE265`); the source may be exact or a view so staged and parsed spans
copy without remarshal. All three positions are `u64`.

Literal windows inside both static bounds establish `StaticExact`
evidence; a provably out-of-range literal window fails closed at
elaboration (`MNE271`); anything else keeps an explicit runtime-checked
failure obligation. The operation lowers through a dedicated scalar
operation (`SequenceCopy`) realized on all five executable backends with
identical trap semantics. Below 0.14 the intrinsic never parses
(`MNP207`); wrong arity is `MNP208`.

Evidence: `examples/source/pressure-span-copy.mncs`,
`examples/execution/pressure-span-copy-corpus.json` (15 cases, all five
executable backends),
`crates/mncs-cli/tests/pressure_span_copy.rs`.

## Checked view narrowing (WEB-P-006)

A view already in hand re-satisfies a narrower same-element expectation
(`[E; up_to A]` to `[E; up_to B]`, `B < A`) through an explicit runtime
span check: the narrow succeeds exactly when the live span fits the new
capacity and fails closed otherwise. No copy is materialized and element
identity is untouched; only the static capacity fact changes. The check
is retained on every backend through a dedicated scalar operation
(`ViewNarrow`) that aliases the source descriptor after verifying the
span.

Widening, element changes, and narrowing below profile 0.14 keep
refusing exactly as before (`MNE103`, `MNE115`, `MNE188`).

Evidence: `examples/source/pressure-view-narrow.mncs`,
`examples/execution/pressure-view-narrow-corpus.json` (10 cases, all
five executable backends),
`crates/mncs-cli/tests/pressure_view_narrow.rs`.

## Checked-index discharge (WEB-P-009)

`checked_index(sequence, index)` is the retained bounds check behind
`CheckedBound` discharge. It traps unless the candidate sits below the
sequence's runtime length and carries the candidate unchanged as a
`u64`. Binding the result and projecting through exactly that value
(`sequence[checked]`, including through `let` aliases of the same
value) discharges the projection's bounds obligation with
`CheckedBound` evidence; the dominating check is identified by value
identity, so merges, rebindings, and cross-sequence uses miss back to an
explicit runtime check and never discharge silently. The check itself is
retained on every backend through a dedicated scalar operation
(`BoundCheck`): exact bounds fold to constants, views read the packed
descriptor length (discarding the lowering-internal cell marker exactly
like checked projections do on the WASM backend).

A literal candidate at or beyond an exact declared bound fails closed at
elaboration (`MNE275`); non-sequence subjects are `MNE273`, non-`u64`
candidates are `MNE274`, and a checked index used where another type is
required is `MNE276`. Below 0.14 the intrinsic never parses (`MNP209`);
wrong arity is `MNP210`. Checked indices do not discharge functional
updates (`replace`) or vector lane operations: those keep their
historical obligations and fail closed on `CheckedBound` claims the
elaboration never establishes.

Evidence: `examples/source/pressure-checked-index.mncs`,
`examples/execution/pressure-checked-index-corpus.json` (12 cases, all
five executable backends),
`crates/mncs-cli/tests/pressure_checked_index.rs` (parity, observable
discharge via `unresolved_obligations`, and refusal coverage).

## Explicit non-claims

- Prefix-cost budgets, traversal fusion, and native loop-region
  reclamation stay future work; 0.14 adds no iteration-shape changes
  (see the prefix-traversal cost corpus,
  `examples/source/pressure-prefix-traverse.mncs`, for the blessed
  idiom under the current cost model).
- `CheckedBound` is a single-dominating-check fact, not a range
  analysis: arithmetic on a checked index, or a check against a
  different value of the same sequence, discharges nothing.
- Unrestricted `while` loops, heap allocation, traits, unrestricted
  callable values, and a conventional runtime model are all out of
  scope; boundedness, proof/evidence discipline, semantic identity, and
  the explicit backend capability model still govern.
