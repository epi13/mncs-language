# Source Profile 0.3 experiment-readiness evidence — 2026-08

Status: local bounded implementation evidence; not conformance or independent evaluation

## Audit result

Current `main` already had a real Profile 0.2 source-to-SSA path, a backend-neutral adapter
contract with portable-WASM and research-bytecode implementations, language-owned experiment
records, bounded translation validation, and Forge persistence/compare primitives. The missing CRE
language concepts were nominal finite values, exhaustive branching, general expression returns,
source calls, and call authority closure. Forge did not require a schema or authority change.

## Structural analysis

The required before/after Joern workflow used:

```bash
joern-parse --language rust -o /tmp/mncs-language-profile03-before.cpg .
joern --script scripts/joern/source-vertical-slice.sc \
  --param cpgFile=/tmp/mncs-language-profile03-before.cpg
joern --script scripts/joern/compiler-stage-contracts.sc \
  --param cpgFile=/tmp/mncs-language-profile03-before.cpg
joern --script scripts/joern/experiment-bootstrap-profile.sc \
  --param cpgFile=/tmp/mncs-language-profile03-before.cpg
```

The same queries were repeated against `/tmp/mncs-language-profile03-after.cpg`. The post graph
retains the existing source/elaboration/validation/HIR/SSA/backend experiment boundaries and adds
the expected finite-declaration/recursion analysis in `elaborate_program`, property evaluation from
`run_experiment`, and stronger portable-backend value-contract checks. Joern's Rust frontend did not
recover useful internal call edges for the match-heavy `elaborate_expr` and `execute_instruction`
methods; this is an analysis limitation, not evidence that those source calls are absent. It also
reported CFG order fallback warnings for existing `break`/`continue` constructs. Runtime and Rust
tests provide the corresponding executable evidence.

Forge was parsed with Joern's `pythonsrc` frontend because the `python` alias attempted a missing
legacy `py2cpg.sh`. Its baseline persistence queries showed the existing compiler-experiment record
and immutable ledger boundaries. No Forge source changed, so no post-edit Forge graph was required.

## Local CRE observations

Both correct CRE-1 runs returned language-owned `PASS`: all nine exact cases met expectations and
all six bounded properties passed. The target-independent semantic and selected-SSA fingerprints
agreed; backend, plan, and artifact identities differed. The wrong candidate returned `FAIL` for
`unknown-unknown` and retained counterexamples for idempotence and UNKNOWN preservation. Both
adapters rejected the forged `UNKNOWN` discriminant `99` as `invalid_request`.

CRE-2's authorized caller returned both finite variants through both adapters. The negative fixtures
stopped in elaboration with `MNE111` (undeclared capability) and `MNE134` (authority laundering).
The acyclic-call policy is exercised by `MNE130`.

The exact language result records were submitted through Forge's `compiler record`, `list`, and
`compare` operations. Forge retained the language status and exact record, left assurance and
conformance `null`, and built `mncs-forge.concept-evaluation.v0.1` with distinct generator/verifier
identities. This is local Forge evaluation, not independent evaluation.

## Remaining blocker

CRE-3 requires semantically visible bounded iteration and per-attempt state. Profile 0.3 deliberately
does not fake this with recursion or an unrestricted loop. The machine-readable blocker is
`examples/experiments/cre3-retry-authority-blocker.json`.
