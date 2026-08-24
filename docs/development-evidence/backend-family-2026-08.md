# Backend family realization evidence — 2026-08

This record is bounded observation, not universal equivalence or compiler correctness. Roadmap
milestone 0.5 was completed later in the same month; see `roadmap-0.5-2026-08.md` for that distinct
acceptance record.

## Adapters actually present

| Backend | Artifact | Lowering | Execution | CRE-1 | CRE-2 | CRE-3 |
| --- | --- | --- | --- | --- | --- | --- |
| `mncs-portable-wasm-mvp` | `wasm_module` | PASS | embedded interpreter | PASS | PASS | PASS |
| `mncs-research-bytecode` | `research_bytecode` | PASS | SSA interpreter | PASS | PASS | PASS |
| `mncs-llvm-ir` | `llvm_ir` | PASS | external clang | PASS | PASS | PASS |
| `mncs-c11` | `c11_translation_unit` | PASS | external clang/gcc | PASS | PASS | PASS |
| `mncs-cranelift` | `cranelift_clif` | PASS | host JIT | FAIL | FAIL | execution ERROR/withheld |

Cranelift lowering emits inspectable CLIF and a real Cranelift IR/JIT path. On hosts that permit the
mapping, checked-add JIT execution returns 42 and takes the failure path on overflow in unit tests.
On the current development host, CRE-1/CRE-2/CRE-3 execution returns `unsupported` with host mapping
reason `unable to make memory readable+executable`. This is an execution-host policy limitation,
not a compiler/CLIF or language-semantic failure. The identity-bound CLIF artifact remains.

## Identities retained

Each experiment result retains source, profile, compiler, semantic, HIR, SSA,
selected SSA, realization request, backend, artifact, corpus, validator, and
unresolved-obligation identities. Cross-backend agreement on CRE-1/2/3 for
WASM, bytecode, LLVM, and C11 is bounded empirical agreement.

## Promise/evidence

LLVM checked integer lowering emits `llvm.*.with.overflow` when no current independently checked
certificate exists. Exact constant-range certificates now permit signed `nsw` and unsigned `nuw`;
the artifact records the original overflow obligation, selected SSA, constant producers, verifier,
method, evidence status/freshness, attribute, and emitted/withheld decision. Conservative lowering
remains available. Missing target facts still produce `UNKNOWN`, not guessed defaults.

## Not implemented

SPIR-V, PTX, eBPF, extra VMs, and embedded bare-metal adapters are planned
slots only. They are listed in `experiment matrix` as unimplemented.

## Source profiles vs roadmap milestones

Source Profiles 0.1–0.5 are language envelopes. Roadmap milestone 0.5 is a project acceptance gate.
They are not the same numbering.
