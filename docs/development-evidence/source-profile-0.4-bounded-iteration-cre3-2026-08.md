# Source Profile 0.4 bounded-iteration and CRE-3 evidence — 2026-08

Status: local bounded implementation evidence; not conformance, universal equivalence, independent
evaluation, or production assurance

## Implemented slice

Profile 0.4 adds a structured `iterate … up_to … carrying … next …` transition with a versioned
static bound of 1 through 32. The declared bound, carried state, region blocks, callees,
capabilities, completion modes, cycle policy, and six identity-bound obligations survive source,
CST, AST, semantic body and graph, HIR, selected SSA, target plans, both backend artifacts, and the
language-owned experiment result. The body is lowered to a real block-parameter CFG backedge, not
source-unrolled.

The bound-validity, resource-ceiling, authority-closure, state-preservation, and completion-mode
obligations are `PASS`. Exact instruction cost remains `UNKNOWN`; that unresolved claim is retained
in the compiler study and Family Record reference. Runtime step budgets remain independent of the
language-semantic attempt ceiling.

## Structural analysis

The baseline was commit `941a087` (`origin/main`). The same Joern version, Rust frontend, repository
scope, and focused scripts were applied before and after:

```bash
joern-parse --language rust -o /tmp/mncs-language-profile04-before.cpg .
joern --script scripts/joern/source-vertical-slice.sc \
  --param cpgFile=/tmp/mncs-language-profile04-before.cpg
joern --script scripts/joern/compiler-stage-contracts.sc \
  --param cpgFile=/tmp/mncs-language-profile04-before.cpg
joern --script scripts/joern/experiment-bootstrap-profile.sc \
  --param cpgFile=/tmp/mncs-language-profile04-before.cpg
```

The final graph was `/tmp/mncs-language-profile04-after-final.cpg`. The updated experiment script
was also rerun against the baseline CPG so the comparison used identical queries.

The final graph recovered one `elaborate_bounded_iteration` method with 8 `if` and 2 `while`
control structures and one `validate_bounded_iterations` method called from body validation with 9
`if`, 2 `while`, and 1 `continue`. SSA `validate_function` grew from 17 `if`/14 `while` controls to
25 `if`/18 `while` plus one `continue`, consistent with explicit bounded-cycle, region, authority,
state, and obligation checks. The query retained all compiler-stage boundaries and recovered the
three existing body/SSA/backend comparison calls. No backend-specific branch appeared in the
generic compiler driver.

Joern's Rust frontend again emitted CFG order-fallback warnings for existing `break` and `continue`
statements. It did not recover useful internal call edges for several match-heavy Rust methods,
including the caller of `elaborate_bounded_iteration`; the focused boundary query, source review,
and executable tests provide complementary evidence. JVM native-access/deprecation warnings were
tool warnings, not parse failures.

## Compatibility evidence

A detached build of `origin/main` and the final branch processed the same unchanged sources with
the same logical paths. Semantic, HIR, and SSA fingerprints were identical in every comparison:

| Profile / fixture | Semantic | HIR | SSA |
|---|---|---|---|
| 0.1 `identity.mncs` | `b0741f252715c1d1318d238b7c239d04d7c0bfe73e650253dc677631ee7ed6c9` | `8eaf38d8ce23b7d4cdee7941f967d4efc7304c36b961255291101e4eaaae9e2f` | `edd9a8bbf00270feea206ee299f485e9ea579bf84802ad7deb103f795c078da6` |
| 0.2 `flagship.mncs` | `72af9a551d2b6fc08d9a9516206b5993f717da78d100f627fea8033e1b350b77` | `6d39e3ced8b783630681b98be3e95643fc92783b9095586c80e476f60682e770` | `cc4983e5d35e7bcd92638b9439cb52b9a6932295666f184bb6f93f3fb412b343` |
| 0.3 CRE-1 | `275a1ff09e2287731e12762e17a10c6929440be6203f193df93cd96d4a2afcc6` | `7ac63b1730595928cfa4edf05af4b60cdf7b58046e0f062b9a07a297e2a45999` | `cacc7c628234987f097c68baaa0c4b869174093202b3850d37e8434bbd551e61` |
| 0.3 CRE-2 | `ed98ec768f05bec47f98e395a4e6210c3ffc14c060a72114a9b7103b4c005a95` | `a58d67f9835dbcb2ec51a65b953b0b7d2159231d876d2995344b013f01db27c9` | `15e65eaa57d76791eb6e3bfbdb950cc7376d6e75f3c294d2b430f55abd987328` |

The compatibility implementation treats Profile 0.4 words as ordinary identifiers in older
profiles and uses an additive, omitted-by-default cycle policy so older serialized artifacts do not
churn.

## CRE observations

The final correct CRE-3 runs returned language-owned `PASS` through
`mncs-portable-wasm-mvp` and `mncs-research-bytecode`. Both had source fingerprint
`8e270615516ddf49ae3fd5c2c922bbc3a3b3fac8cb544182835f0450e8169917`, semantic fingerprint
`3565ecb936dd686456d23a0619f8141dd42b7e1218851fabfd15c34d6db97cd5`, HIR fingerprint
`48536dc240323cff7463b6888f0b78a3ba03e7d78f5524fec6c58e2964375214`, and selected-SSA
fingerprint `55caa4042de8a133a6745707910b48670c73eff3a098c475413e3d53920ed15b`.
Backend and artifact identities differed. All six typed cases met returned-value, returned-status,
maximum-step, expected-effect, and unexpected-effect-prohibition checks. Direct body-versus-SSA and
body-versus-SSA-versus-each-backend comparisons matched all six cases.

The wrong-UNKNOWN mutant returned `FAIL` with counterexamples for success at the exact bound,
uncertainty then success, and uncertainty then terminal failure. The wrong-bound mutant returned
`FAIL` for uncertainty through exhaustion. Seven source negatives retained specific diagnostics:
`MNP106`, `MNP094`, `MNE142`, `MNE144`, `MNE134`, and `MNE130` as applicable. Minimum bound 1 and
maximum bound 32 both executed to the same returned observation in independent body and SSA paths.

CRE-1 (nine cases) and CRE-2 (two cases) remained `PASS` through both adapters.

## Forge and Family Record boundary

The actual two final CRE-3 JSON records were parsed by Forge's existing
`CompilerExperimentObservation.from_language_record` and compared. Forge preserved both language
`PASS` statuses, the exact unresolved resource-cost obligation, distinct backend/artifact
identities, and equal source/semantic/HIR/SSA stages. Its result kept assurance and conformance
`null`. No Forge source or schema change was required because the language result contract remains
additive `mncs:language:experiment-result:0.1`.

Each language experiment output also writes `family-reference.json` with exact source/profile,
compiler/pipeline, semantic/HIR/SSA/selected-SSA, realization, backend/artifact, corpus, validator,
result/status, and unresolved-obligation identities. Its authority statement is observational only.

## Validation commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
target/debug/mncs experiment run examples/source/cre1-evidence-combine.mncs ...
target/debug/mncs experiment run examples/source/cre2-authorized-call.mncs ...
target/debug/mncs experiment run examples/source/cre3-retry-authority.mncs ...
target/debug/mncs experiment compare /tmp/mncs-cre3-wasm-final.json \
  /tmp/mncs-cre3-bytecode-final.json
```

The workspace test suite exercises the complete source-to-both-backends path, mutants, negative
diagnostics, exact accepted-bound boundaries, arbitrary-cycle rejection under the Profile 0.4
policy, typed corpus 0.2 body/SSA comparison, and Family Record generation.

## Remaining limits

This work does not establish exact instruction cost, universal equivalence, independent
evaluation, a final grammar or type theory, general loops or recursion, nested iteration,
unrestricted effects, memory semantics, native code generation, or production suitability. The
next high-leverage language candidate remains first-class product/record values or another typed
structured-state model; it was deliberately not started in this pass so the completed CRE-3 slice
and its compatibility evidence remained cohesive.
