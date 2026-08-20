# Compiler stage contracts validation — 2026-08

## Scope

This evidence covers the compiler stage architecture contract, compiler study observation fields, and CLI exposure added to `mncs-language`.

It does not claim frontend implementation, translation validation, backend correctness, executable generation, native Windows execution, native Raspberry Pi execution, independent evaluation, or conformance.

## Validation commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
cargo run -q -p mncs-cli -- compiler-architecture
cargo run -q -p mncs-cli -- compiler-study \
  examples/compiler-study/bounded-sum.mncs.json --node-id local-audit
```

Results:

- Rust formatting and clippy passed;
- 89 workspace tests passed: 16 CLI integration, 8 compiler, 62 model, and 3 syntax tests;
- documentation/diff whitespace validation passed;
- architecture output contained all 12 logical stages with a valid content identity; and
- compiler study output retained semantic/HIR/SSA/selected-SSA fingerprints, exact pass records, diagnostics, and unresolved obligations under the language-owned contract ID.

## Joern method

The same Joern version, Rust frontend, source scope, script, and query bounds were used before and after source changes.

```bash
joern-parse crates \
  -o workspace/compiler-stage-baseline.cpg.bin --language rust
joern --script scripts/joern/compiler-stage-contracts.sc \
  --param cpgFile=workspace/compiler-stage-baseline.cpg.bin --nocolors

joern-parse crates \
  -o workspace/compiler-stage-post.cpg.bin --language rust
joern --script scripts/joern/compiler-stage-contracts.sc \
  --param cpgFile=workspace/compiler-stage-post.cpg.bin --nocolors
```

The query was executed from `/tmp` with absolute paths because Joern project creation uses a local `workspace/` directory and conflicts with a CPG file stored under a repository directory named `workspace`.

## Comparative graph findings

- `compile` remained two methods with the same `IF=10`, `WHILE=2` control-structure count and the same validation/integrity calls.
- `request_for_program`, `reference_pipeline`, and `validate_integrity` retained their prior call and control boundaries.
- `run_study` remained one method with `IF=1`; new calls are bounded projection operations (`as_ref`, `get`, `cloned`, `unwrap_or_default`) over the exact compilation result.
- compiler study projection does not call semantic lowering again and does not create a second HIR or SSA path.
- the additional `identity_is_valid` implementation increased the focused method count from 14 to 15 and checks the study contract ID, schema version, observation-only interpretation, and content identity.
- stage architecture construction is data-only and does not add authorization, target, backend, or executable control paths.

## Failures and limitations

- Joern emitted JVM deprecation and native-access warnings; parsing and queries completed successfully.
- Rust CPG control/data-flow recovery is an analyzer model, not proof of semantic correctness.
- The focused script does not prove absence of every possible dependency outside the named compiler/model files.
- No cross-host CPG comparison was performed.
- Generated CPGs are retained locally under ignored `workspace/` paths and are not committed as portable evidence.

## Residual uncertainty

The compiler study result is identity-bound but producer-local. Forge comparison and Rust tests establish structural behavior only. Translation claims remain at their recorded pass status, unresolved obligations remain unresolved, and no independent assurance or conformance decision follows from this evidence.
