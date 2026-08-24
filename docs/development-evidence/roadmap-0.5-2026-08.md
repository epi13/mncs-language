# Roadmap 0.5 completion evidence — 2026-08-23

Status: **complete as a bounded experimental milestone**. This record does not claim universal
equivalence, production compiler correctness, a general proof kernel, or completion of Source
Profile 0.5 maturity. Source Profile numbering is separate from project roadmap numbering.

## Acceptance reconciliation

| Acceptance item | Implementation and exercised evidence | Residual limit |
| --- | --- | --- |
| plural constrained backend | `BackendAdapter`, selected SSA, plans, WASM/bytecode/LLVM/C11/Cranelift artifacts; existing CRE suites | SPIR-V/PTX/eBPF remain future adapters |
| verified optimization attributes | `promises.rs` release-path constant-range checker; LLVM `nsw` and `nuw` positive test; checked-add negative test retains overflow intrinsic | pilot proves constants only |
| five integer intents | `machine_intent.rs`, body/HIR/SSA and both reference evaluators; WASM bounded-width realization | LLVM/C11/Cranelift deliberately refuse saturating/widening |
| promise decision history | `BackendArtifact` schema 0.2 `promise_decisions` records subject, obligation, status/freshness, attribute, certificate, and reason for no-overflow/in-bounds/alignment/non-aliasing/relaxation | memory promises are withheld/non-applicable because the scalar envelope has no memory operations |
| executable + evidence | adapter artifacts and evidence bundles remain emitted together | native packaging still uses external tools |
| Rust/Zig control | `experiment rust-control` and `examples/controls/arithmetic-envelope.rs` | five-case bounded comparison, not proof |
| realization identities | existing compiler/backend/config/target/ABI/pass/artifact/corpus/validator chain, extended with promise certificates and refinement decisions | no independent custody claim |
| backend assumptions | target plans, backend configurations, artifacts, and evidence | host/tool assumptions remain explicit trust boundaries |
| bounded refinement cycle | `BoundedRefinementCycle` and `experiment refine` | realization choice only; no semantic self-modification |
| independent promotion evidence | current PASS translation validators, valid identities, no unresolved reasons, and protected bounded agreement are mandatory | Forge/search cannot promote |
| accepted/rejected history | identity-sealed per-candidate decisions retain evidence and reasons | append-only external witness is future higher-assurance work |
| proof-carrying lowering | selected-SSA-bound constant-range certificate authorizes LLVM no-overflow flag | not a general dependent proof core |
| search/check split | discovery proposes a certificate; a narrow checker recomputes typed range and validates every dependency before emission | one in-process checker, not diverse independent implementations |

## Arithmetic experiment

`examples/executable/arithmetic-envelope.mncs.json` defines signed i8 saturating add and exact i8→i16
widening multiply. `examples/execution/arithmetic-envelope-corpus.json` covers positive saturation,
negative saturation, in-range addition, maximum positive multiplication, and mixed-sign edge
multiplication.

Executed results:

- body↔SSA: 5 matching, 0 mismatching;
- research bytecode: PASS, 5/5;
- portable WASM: PASS, 5/5;
- compiled Rust control vs generated WASM observations: bounded agreement, 5/5;
- LLVM: structured `unavailable_backend_capability` diagnostics `CGL301`/`CGL302`, no artifact;
- the same scalar capability refusal applies to C11 and Cranelift.

Retained run identities:

- bytecode result `mncs:language:experiment:result:2854a9ac2e58e018ef8e1cb4bd1c6ab21da0f20079bdedf15859ce9578c0aaad`;
- bytecode artifact `mncs:compiler:backend-artifact:6f6cc3a72fc69713ccb1694c50aea3f086281da1e96961faca18f35ceb8feb2a`;
- WASM result `mncs:language:experiment:result:1eee8693aa15d516cc9b9566ea88f15aa2fb3a4c888b53aa579172289611e32c`;
- WASM artifact `mncs:compiler:backend-artifact:250689032c5fec2dd1cd344a70c0653b174a77c3a0f87d5d9abb7e155cfd2903`;
- Rust comparison `mncs:source:artifact:f4f0246ff0557fe0569909bff725eeac68930aeaf069261c432c0aae2fb14773`.

The WASM implementation also repairs a pre-existing signed sub-word masking bug: the old path left
an unused shift constant on the stack and retained only the unsigned bit pattern. The new lowering
uses the MVP-safe `(masked ^ sign) - sign` identity, so later signed arithmetic sees the declared
value rather than merely reconstructing it at the final output boundary.

## Promise certificate experiment

`examples/executable/verified-constant-add.mncs.json` supplies safe signed and unsigned constant adds.
Certificate discovery records the actual selected-SSA overflow obligation and exact constant
producers. The checker then validates the certificate identity, selected SSA, subject, obligation,
verifier/method, dependencies, typed operation, and exact range before the release lowering path can
emit `add nsw` or `add nuw`.

The negative checked-add fixture has no certificate, emits no `nsw`, and retains
`llvm.*.with.overflow`. A selected-SSA identity mutation makes the positive certificate fail
verification. Thus `UNKNOWN`, missing, stale, or wrong-scope evidence cannot authorize the flag.

## Bounded refinement cycle

The observed baseline is the PASS research-bytecode arithmetic result. The localized/proposed change
is only `backend:mncs-research-bytecode->mncs-portable-wasm-mvp`. Translation validators and bounded
behavior accept the WASM realization. A second proposed candidate is the intentionally wrong CRE-1
semantic mutant; its different source/stage identities, FAIL status, and counterexamples cause an
explicit retained rejection.

Cycle identity:

`mncs:language:experiment:bounded-refinement-cycle:791bbc8502ab2bbb72c2c15234e562c304c8c1ee3d32fdd4e9cc7b0c34af9bb0`

It records two candidates, one promoted experimental realization, and rejected candidate
`mncs:language:experiment:result:cebc745fc92c0167c25ed24e78862fefe1a53487843bed94f00681cb2a5b1960`.
The cycle's authority field states that Forge/search has no promotion authority. No performance
observation participates in authorization.

## Cranelift investigation

CRE-1, CRE-2, and CRE-3 still lower to identity-bound CLIF. On this Fedora x86-64 development host,
all three execution attempts return `unsupported` with `unable to make memory readable+executable`.
That is the Cranelift JIT executable-memory transition being denied by the host environment. It is
not a selected-SSA compiler failure, multi-function lowering failure, or MNCS semantic result. No
execution PASS was fabricated. A future object/external-link execution route can address hosts with
this policy without making WASM the specification.

## Joern graph-sensitive workflow

The Joern agent bridge was attempted first but its snapshot adapter rejected `--language rust`; the
installed bridge lists only C/C++, Java, JavaScript, Kotlin, PHP, and Python. Real Joern's installed
Rust frontend was therefore used directly for both snapshots:

```text
joern-parse crates --language rust -o target/roadmap-0.5-joern/baseline.cpg.bin
joern --script scripts/joern/roadmap-0.5.sc --param cpgFile=target/roadmap-0.5-joern/baseline.cpg.bin
joern-parse crates --language rust -o target/roadmap-0.5-joern/post.cpg.bin
joern --script scripts/joern/roadmap-0.5.sc --param cpgFile=target/roadmap-0.5-joern/post.cpg.bin
```

The same final focused query was rerun over both CPGs. The post graph adds explicit edges from
machine-intent evaluation to certificate search/checking, from certificate checking to
`identity_is_valid`, from WASM lowering to `emit_saturating`/`emit_widening`, and from the refinement
cycle's nine guarded decisions to result comparison, identity checks, and sealing. Existing
validation, body/SSA execution, LLVM/C11/Cranelift emission, and JIT boundaries remain reachable.

Joern reported Rust frontend CFG warnings that `continue` and `break` used order fallback. Therefore
the query supports call/control boundary comparison but is not claimed as complete Rust CFG/dataflow
proof. Query source is retained in `scripts/joern/roadmap-0.5.sc`.

## Reproduction commands

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p mncs-cli -- check-lowering-execution examples/executable/arithmetic-envelope.mncs.json examples/execution/arithmetic-envelope-corpus.json
cargo run -p mncs-cli -- experiment run examples/executable/arithmetic-envelope.mncs.json --backend mncs-research-bytecode --corpus examples/execution/arithmetic-envelope-corpus.json --output-dir /tmp/mncs-roadmap05-bytecode
cargo run -p mncs-cli -- experiment run examples/executable/arithmetic-envelope.mncs.json --backend mncs-portable-wasm-mvp --corpus examples/execution/arithmetic-envelope-corpus.json --output-dir /tmp/mncs-roadmap05-wasm
cargo run -p mncs-cli -- experiment rust-control /tmp/mncs-roadmap05-wasm/result.json examples/controls/arithmetic-envelope.rs
cargo run -p mncs-cli -- experiment refine BASELINE PASS_CANDIDATE REJECTED_CANDIDATE --budget 2
```

The CI workflow runs the arithmetic, Rust-control, accepted/rejected refinement, and unsupported LLVM
paths in addition to the full workspace format/lint/test gate.
