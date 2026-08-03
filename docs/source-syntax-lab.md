# Source Syntax Laboratory

MNCS Language does not yet have a selected surface grammar. The repository now contains an evidence-producing syntax laboratory rather than a single speculative syntax declaration.

## Current hypothesis

The strongest human-source candidate is a restrained systems language influenced by Zig:

- ordinary declarations, braces, and semicolons;
- explicit mutation and error propagation;
- short contract and effect keywords;
- capabilities represented as typed authority;
- specification information adjacent to implementation;
- deterministic formatting and lowering.

The canonical agent representation is intentionally separate and more compact.

## Why three forms

### Human source

Optimized for comprehension, review, and intentional authoring. It must remain complete enough that authority and behavior are not hidden behind tooling.

### Canonical semantic form

Optimized for retrieval, comparison, micro-verification, and agent exchange. It contains no irrelevant whitespace or optional sugar and can be projected as a bounded semantic slice.

### Verified IR

Optimized for explicit control flow, state transitions, proof obligations, and backend lowering. It is not intended for ordinary source authoring.

## Semantic density

The laboratory compares equivalent representations against an explicit claim list. This prevents a shorter candidate from appearing efficient by dropping meaning.

Run the current tournament:

```bash
cargo run -p mncs-cli -- syntax-tournament \
  examples/syntax/account-transfer.tournament.json
```

Inspect raw measurements:

```bash
cargo run -p mncs-cli -- syntax-metrics \
  examples/syntax/zig-like/account-transfer.mncs \
  examples/syntax/structured/account-transfer.mncs \
  examples/syntax/minimal/account-transfer.mncs \
  examples/canonical/account-transfer.mncs-sem
```

The initial lexical metric is deterministic and tokenizer-neutral. It is a comparison baseline, not a claim about exact inference cost on any model.

## Candidate roles

The tournament manifest records a role for every candidate. A canonical semantic form should not automatically replace the human source merely because it is smaller. Candidate roles should be compared both within-role and across the whole pipeline.

## Evaluation dimensions

Later syntax studies should record:

- semantic claims represented;
- lexical and model-token counts;
- parse reliability;
- formatting stability;
- graph round-trip fidelity;
- human comprehension;
- agent generation and repair success;
- effect or capability omission rate;
- semantic patch size;
- evidence invalidation precision;
- verifier cost.

## Recursive improvement

Syntax itself can participate in bounded MNCS refinement:

1. compile a shared semantic corpus into each candidate form;
2. collect density, parser, repair, and comprehension evidence;
3. localize recurrent ambiguity or unnecessary repetition;
4. propose a syntax transformation;
5. regenerate the corpus in isolation;
6. verify semantic equivalence and compare evidence;
7. promote or reject the proposal under explicit policy.

This is recursive language improvement without allowing the language generator to silently redefine correctness.

## Current artifacts

- `rfcs/0005-source-representations-and-semantic-density.md`
- `spec/source-representations.md`
- `examples/syntax/account-transfer.tournament.json`
- `examples/syntax/zig-like/`
- `examples/syntax/structured/`
- `examples/syntax/minimal/`
- `examples/canonical/`
- `examples/patches/`
- `crates/mncs-syntax/`

## Status warning

All example syntax is experimental. It may be changed or removed without compatibility guarantees. The examples test representation choices; they are not yet accepted MNCS programs.
