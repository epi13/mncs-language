# Source Profile 0.1 vertical-slice validation — 2026-08

## Scope and claim boundary

This evidence covers the experimental Source Profile 0.1 envelope, lossless lexer/CST, spanned AST, bounded elaboration, semantic graph and identity artifacts, type/contract validation stage, and reuse of the existing HIR/SSA compiler path.

It does not establish a final grammar, complete binding or type system, translation validation, backend correctness, executable generation, native Raspberry Pi execution, independent assurance, or MNCS conformance.

## Validation commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
cargo run -q -p mncs-cli -- source-study \
  examples/source/identity.mncs --node-id local-source-audit
```

Results:

- formatting, clippy at the repository MSRV, and diff validation passed;
- all 96 tests passed: 17 CLI, 10 compiler, 62 model, and 7 syntax tests;
- the source envelope identity bound two non-file artifact relationships plus generator/derivation provenance and rejected post-seal substitution;
- malformed input emitted a structured `MNP016` span diagnostic and no downstream AST/semantic/HIR; and
- the source study completed with fingerprints for source, tokens, CST, AST, semantic, semantic graph, identity map, validation, HIR, SSA, selected SSA, and evidence bundle.

## Joern method

The same Joern version, Rust frontend, crate scope, and focused script were applied to the pre-edit and post-edit graphs. The earlier compiler-spine query was also repeated against the post graph.

```bash
joern --script scripts/joern/source-vertical-slice.sc \
  --param cpgFile=workspace/source-vertical-slice-baseline.cpg.bin --nocolors
joern-parse crates -o workspace/source-vertical-slice-post.cpg.bin --language rust
joern --script scripts/joern/source-vertical-slice.sc \
  --param cpgFile=workspace/source-vertical-slice-post.cpg.bin --nocolors
joern --script scripts/joern/compiler-stage-contracts.sc \
  --param cpgFile=workspace/source-vertical-slice-post.cpg.bin --nocolors
```

The queries ran from `/tmp` with absolute paths because Joern creates its own local `workspace` project directory.

## Comparative graph findings

- the baseline had no `lex`, `parse`, `document`, `function`, `front_end`, `elaborate_program`, `run_source_study`, or `front_end_pass_executions` method; the post graph has exactly one of each;
- lexical control is bounded (`IF=9`, `WHILE=6`), parser document construction is bounded (`IF=3`, `WHILE=1`), and elaboration has one function iteration with three validation branches and two explicit rejection continuations;
- `run_source_study` has one stop branch, calls the existing `run_study`, then seals the combined observation; it does not call HIR or SSA lowering directly;
- the existing `compile` graph remained `IF=10`, `WHILE=2`, and `run_study` remained `IF=1`; the compiler now resolves pipeline passes by stable ID rather than positional index, without adding a second lowering path;
- the source envelope adds one `identity_is_valid` method and parser call edge; relationship and provenance builders reseal after mutation; and
- the focused boundary inventory found no source/frontend call to backend, linker, execution, assurance, conformance, or promotion behavior.

## Analyzer failures and uncertainty

- Joern emitted JVM native-access/deprecation warnings plus Rust CFG fallback warnings for `continue` and `break`; parsing and both focused queries completed.
- Rust call recovery did not surface several calls nested in `front_end` match expressions, even though direct source inspection and executed tests cover parse, validate, graph, and identity construction. The graph result is therefore conservative rather than proof of call absence.
- CPG control/data-flow recovery does not prove identity soundness, parser completeness, semantic equivalence, or compiler correctness.
- Generated CPGs remain in ignored local `workspace/` paths and are not portable evidence artifacts.

## Residual uncertainty

Profile 0.1 intentionally supports only pure functions that explicitly return one input with the declared output type. General scopes, recursive artifact resolution, contract syntax, effects, calls, branching, partial-program identity, profile migration, translation validation, and executable backends remain undefined or unimplemented.

## Hosted-clippy follow-up

The first hosted CI run used a newer current Clippy lint set than the local repository baseline and rejected iterator `format!` collection plus a test-only owned-string assignment. The implementation now uses the repository-standard bounded hex-write loop and `clone_into`. Formatting, clippy, all 96 tests, and diff validation passed again. Repeating the same source vertical-slice Joern query after the repair preserved all stage method/control counts and boundaries; the change adds no compiler-stage, validation, backend, or authority branch.
