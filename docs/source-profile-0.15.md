# Source Profile 0.15 — bounded view composition

Status: **implemented, sealed**. Profile 0.15 is
additive over Profiles 0.1–0.14 and was the explicit evolution home
(RFC 0036) for safe static view-capacity widening; the current profile
is 0.16 (`docs/source-profile-0.16.md`). Older profiles
retain their historical syntax, semantics, and canonical fingerprints;
the `crates/mncs-cli/tests/profile_compat.rs` suite pins both directions
(old-profile refusals and 0.15 admissions).

Predecessor: Profile 0.14 (`docs/source-profile-0.14.md`).
Machine-readable policy: `mncs-syntax` registry
(`crates/mncs-syntax/src/profile.rs`,
`spec/source-profile-registry.json`).

## Static view widening (STORE-P-0012 / P2-002)

A view already in hand satisfies a wider same-element expectation
(`[E; up_to M]` to `[E; up_to N]`, `M <= N`, including `M == N`) with no
copy and no runtime check: every value valid under capacity `M` is valid
under capacity `N` by construction, and the live span is untouched — only
the static capacity fact widens. The descriptor is identical on every
backend, so lowering is unchanged; widening is an elaboration-level
re-annotation at the use site.

Widening applies at `let` bindings, call arguments (including
imported-module calls and generic specializations after substitution),
returns (both statement and bodyless-tail forms), and call results used
at a wider expectation. Checked narrowing (0.14, `ViewNarrow` with its
retained runtime span check) stays distinct: narrowing still verifies,
widening never does.

Refusals stay closed and are pinned by the corpus:

- element type mismatch (`[byte; up_to 8]` into `[u64; up_to 64]`);
- exact/view confusion in either direction (`[byte; 8]` into
  `[byte; up_to 4]`, `[byte; up_to 8]` into `[byte; 8]`);
- capacity relation in the wrong direction (`[byte; up_to 64]` into
  `[byte; up_to 8]` — that is narrowing, not widening);
- symbolic generic capacities that cannot decide `M <= N` statically;
- below 0.15 the historical `MNE117`/`MNE133`/`MNE115`/`MNE103`/`MNE135`
  refusals fire exactly as before.

Evidence: `examples/source/pressure-view-widen.mncs`,
`examples/execution/pressure-view-widen-corpus.json` (value cases on all
five executable backends plus trap-free refusal pins),
`crates/mncs-cli/tests/pressure_view_widen.rs`.

## Explicit non-claims

- Widening is a static capacity relation, not a runtime span operation:
  it never checks, copies, or re-slices the live value.
- Unrestricted `while` loops, heap allocation, traits, unrestricted
  callable values, and a conventional runtime model are all out of
  scope; boundedness, proof/evidence discipline, semantic identity, and
  the explicit backend capability model still govern.
