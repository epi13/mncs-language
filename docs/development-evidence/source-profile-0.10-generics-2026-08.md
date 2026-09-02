# Source Profile 0.10 generic integration — 2026-08

Status: **implemented but partial / experimental**.

This record covers recovery of the interrupted Profile 0.10 integration from
the `6636752` handoff. The repair was checked against the prior Profile 0.9
namespace path and the Profile 0.7/0.8 ABI suites.

## Repaired contracts

- Generic calls now parse inside qualified projection chains and nested type
  arguments, including the lexer’s `>>` token boundary.
- Canonical generic arguments retain declaration order and nominal record/
  finite identities; repeated calls deduplicate to one specialization.
- Type and `Nat` arguments are checked for arity, kind, scope, constancy, and
  the profile ceiling (`MAX_SEQUENCE_BOUND = 64`). Higher-kinded parameters
  are rejected as unsupported.
- Exact and view sequence bounds substitute through body, bounded-iteration
  metadata, HIR, SSA, and specialization records. Remaining generic names,
  `GenericParam` values, or parameterized bounds cannot reach lowering.
- A shared checked compiler boundary runs before every executable adapter;
  defensive guards remain in research bytecode, portable WASM, C11, LLVM IR,
  and Cranelift. Direct backend callers therefore fail closed too.
- `[mask<N>; K]` remains invalid as a sequence element (`MNE105`); logical
  `[bool; N]` is separate from mask lanes.
- Imported generic sequence signatures now retain the declaring module's
  nominal finite/record element identity through specialization. The backend
  contract builder also preserves a root-module wrapper contract when an
  imported generic declaration has the same short name, preventing strict
  process-boundary backends from rejecting a valid wrapper request.

## Tests and fixtures

The compiler tests cover zero, one, and 64-length instantiations; repeated and
different substitutions; qualified and aliased module calls; nominal records;
nested sequences; boolean and `u64` values; empty views; malformed generic
constraints; missing/count/kind/non-constant/out-of-range arguments; recursive
calls; and a 130-specialization expansion-limit refusal (`MNE227`).

Committed fixtures are:

- `examples/source/generic-polymorphism.mncs` with
  `examples/execution/generic-polymorphism-corpus.json`;
- `examples/source/generic-module-consumer.mncs` with
  `examples/execution/generic-module-consumer-corpus.json`;
- `examples/source/test/generic/lib.mncs` for file-resolved module linking.
- `examples/source/status-generic-consumer.mncs` with
  `examples/execution/status-generic-consumer-corpus.json`, which exercises
  imported `Status` sequences, generic exact/prefix summaries, and the
  process-boundary contract on all five executable backends.

## Backend evidence

The generic-polymorphism corpus was run with each executable adapter:

| Backend | Cases returned | Translation validation | Overall study |
| --- | ---: | --- | --- |
| research bytecode | 10/10 | PASS | UNKNOWN |
| portable WASM MVP | 10/10 | PASS | UNKNOWN |
| C11 | 10/10 | PASS | UNKNOWN |
| LLVM IR | 10/10 | PASS | UNKNOWN |
| Cranelift | 10/10 | PASS | UNKNOWN |

The three-case qualified/aliased module corpus returned 3/3 on all five
backends, with translation validation `PASS` and overall study `UNKNOWN`.
`UNKNOWN` is retained because compilation carries required unresolved
obligations; matching returned values and a bounded translation judgement are
not promoted to universal equivalence. Backend artifacts retain their normal
cell/view ABI contracts; no backend-specific generic representation is
invented.

## Standard library and documentation

`mncs.core.sequences.v1` and `mncs.core.geometry.v1` are Profile 0.10 source
consumers. Existing `*4` compatibility wrappers delegate to generic
authorities, including byte, nonzero, boolean, extrema, and empty-view
helpers. A small forward slice adds generic `first_of_generic<N>` and
`last_of_generic<N>` over `[i64; up_to N]`, preserving runtime-empty behavior.

RFC 0013 now distinguishes the implemented bounded Track 1 + Track 12 slice
from future constrained/evidence polymorphism, coherence, parametricity
verification, higher-kinded parameters, and realization search. The roadmap,
compiler roadmap, standard-library direction, and nested-sequence evidence
record use the same boundary.

## Verification commands

```text
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p mncs-syntax profile_010 -- --nocapture
cargo test -p mncs-compiler --test module_imports profile_010 -- --nocapture
```

The full workspace suite includes the existing 13-case five-backend ABI
matrix, backend-family and library corpora, and Profile 0.5–0.8 regressions.
The post-repair workspace run completed with every listed suite passing; the
13-case ABI matrix took approximately 861 seconds.

## Non-claims

This is not a proof of universal parametricity, coherence, layout
independence, generic evidence passing, or production backend correctness.
Inference is intentionally unavailable; higher-kinded and constrained
polymorphism remain future work; exact instruction cost and runtime-checked
range obligations remain capable of producing `UNKNOWN`.
