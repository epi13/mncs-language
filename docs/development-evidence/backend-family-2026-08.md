# Backend family realization evidence — 2026-08

This record is bounded observation, not universal equivalence, compiler
correctness, or roadmap milestone 0.5 completion.

## Adapters actually present

| Backend | Artifact | Lowering | Execution | CRE-1 | CRE-2 | CRE-3 |
| --- | --- | --- | --- | --- | --- | --- |
| `mncs-portable-wasm-mvp` | `wasm_module` | PASS | embedded interpreter | PASS | PASS | PASS |
| `mncs-research-bytecode` | `research_bytecode` | PASS | SSA interpreter | PASS | PASS | PASS |
| `mncs-llvm-ir` | `llvm_ir` | PASS | external clang | PASS | PASS | PASS |
| `mncs-c11` | `c11_translation_unit` | PASS | external clang/gcc | PASS | PASS | PASS |
| `mncs-cranelift` | `cranelift_clif` | PASS | host JIT | FAIL | FAIL | execution ERROR/withheld |

Cranelift lowering emits inspectable CLIF and a real Cranelift IR/JIT path.
Checked-add JIT execution returned 42 and trapped on overflow in unit tests.
CRE-1/CRE-2 JIT execution currently fails with host mapping
`unable to make memory readable+executable`. That is an execution-mode
limitation, not a language-semantic failure. The CLIF artifact remains.

## Identities retained

Each experiment result retains source, profile, compiler, semantic, HIR, SSA,
selected SSA, realization request, backend, artifact, corpus, validator, and
unresolved-obligation identities. Cross-backend agreement on CRE-1/2/3 for
WASM, bytecode, LLVM, and C11 is bounded empirical agreement.

## Promise/evidence

LLVM checked integer lowering emits `llvm.*.with.overflow` when no current
PASS no-overflow obligation exists. `nsw` is withheld. Conservative lowering
remains available. Missing target facts still produce `UNKNOWN`, not guessed
defaults.

## Not implemented

SPIR-V, PTX, eBPF, extra VMs, and embedded bare-metal adapters are planned
slots only. They are listed in `experiment matrix` as unimplemented.

## Source profiles vs roadmap milestones

Source Profiles 0.1–0.4 are language envelopes. Roadmap milestone 0.5 is a
project acceptance gate. They are not the same numbering. This work advances
roadmap 0.5 infrastructure and does **not** claim the milestone is complete.
