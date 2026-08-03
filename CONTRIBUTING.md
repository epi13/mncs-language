# Contributing

MNCS Language is currently a semantic-model research project. Contributions should make claims narrow enough to test and assumptions visible enough to challenge.

## Before opening a change

- Read `docs/vision.md`, `docs/non-goals.md`, and `spec/README.md`.
- Search existing RFCs for the concept.
- Separate semantic requirements from preferred syntax.
- Include a counterexample or rejected program when proposing a new rule.

## Change categories

### Documentation clarification

Small corrections and explanatory examples may be submitted directly.

### Semantic behavior

Changes to contracts, effects, capabilities, assumptions, evidence, failure semantics, identity, lowering, or trust boundaries require an RFC in `rfcs/`.

### Prototype implementation

Implementation changes must cite the specification or RFC rule they exercise. Tests should include both acceptance and rejection behavior where meaningful.

## Local checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p mncs-cli -- validate examples/account-transfer.mncs.json
```

The invalid example is expected to exit unsuccessfully:

```bash
cargo run -p mncs-cli -- validate examples/invalid-undeclared-effect.mncs.json
```

## RFC naming

Use the next available four-digit number:

```text
rfcs/0004-short-descriptive-name.md
```

An RFC should contain:

- status;
- summary;
- motivation;
- terminology;
- proposed semantics;
- validation or verification obligations;
- trust and security consequences;
- alternatives;
- unresolved questions;
- compatibility and migration notes.

## Design review questions

Every substantial proposal should answer:

1. What new fact becomes expressible?
2. Can the fact be checked independently?
3. What assumptions remain outside the proof boundary?
4. What evidence is produced?
5. What changes invalidate that evidence?
6. Can a human understand the result without reconstructing compiler internals?
7. Does the proposal constrain ordinary programs unnecessarily?

## Pull requests

Keep pull requests centered on one semantic change or one implementation milestone. Explain what is normative, what is experimental, and what remains deliberately unresolved.
