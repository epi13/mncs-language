# Backend family expansion evidence — 2026-08-25 (Ox Alpha Max campaign)

Status: **development record**. Bounded local evidence only. Every claim below
was exercised on this host during this run; host-dependent limits are named as
such. Nothing here is production-readiness, universal equivalence, or a claim
that external tool success constitutes semantic proof.

> **Correction (2026-08-25, stabilization tranche):** two claims in this
> record did not survive CI and were fixed in
> `docs/development-evidence/backend-family-stabilization-2026-08.md`.
> First, Cranelift checked division relied on native `sdiv`/`srem` hardware
> traps; that only appeared safe on hosts where JIT executable memory was
> denied (AOT child-process route) and killed the CLI under working JIT.
> Second, the external-target "validation" spawned `llvm-readobj` without
> inspecting the artifact; validation now parses the emitted bytes directly,
> and unsigned division was additionally found miscompiled on C11 and WASM.
> See the stabilization record for the corrected behavior and evidence.

## Starting condition

`main` at `7fc73d9` (Ox Alpha Profile 0.6 + core library tranche, squashed)
plus module imports from a parallel tranche; local branch reconciled at
`947896a`; 178 tests green.

## What was implemented, backend by backend

### portable WASM (`mncs-portable-wasm-mvp`) — full envelope

- Linear memory + mutable arena global added to the encoder/interpreter
  (memory/global sections, i32/i64 loads/stores in 8-byte slots, global
  get/set); the decoder accepts the new sections.
- **Logical records** now materialize as immutable arena cells passed by i32
  pointer: parameters, results, and bounded-iteration block state all cross
  function/control boundaries. The previous intra-function-only forwarding is
  gone (stale refusal tests updated to assert agreement instead).
- **Payload-bearing finite variants** materialize as tag cell + payload slots;
  boxed types carry layouts for every variant (payload-free = empty).
- Checked division/remainder with explicit zero and `MIN / -1` guards trapping
  exactly like the reference executors.
- Typed execution marshals composites through language-owned value contracts;
  artifacts carry `composite_value_contracts` so execution needs no program.
- Observed: all four `mncs.core` modules PASS/UNKNOWN-as-honest on WASM,
  including `StatusPair` records and Result payloads.

### research bytecode — parity by construction

Its runtime interprets selected SSA directly; div/mod, booleans, records, and
payload sums execute. Used as the differential companion throughout.

### C11 and LLVM IR

- Checked div/mod: explicit zero guards (C UB protection) and `MIN / -1`
  guards branching to the shared fail path; LLVM picks sdiv/srem/udiv/urem by
  signedness.
- Strict booleans via `&&`/`||` → bitwise ops over normalized 0/1 slots.
- Generated driver prototypes now follow declared parameter types; the old
  int32-cast driver was C undefined behavior for i64 exports (found because
  the conformance corpus disagreed — fixed rather than normalized).
- Envelope honestly refused: composite signatures across the process boundary
  (structured struct-ABI drivers designed, not yet implemented).

### Cranelift (`mncs-cranelift`)

- `jit_scalar` split into `host_isa` + generic `declare_and_build<M: Module>`;
  identical per-instruction lowering feeds JIT and object emission.
- **AOT object fallback**: `aot_object_bytes` emits a host-ISA ELF object via
  `cranelift-object`; when the host denies JIT executable memory (this host's
  policy), execution falls back to object + shared driver linked by clang/gcc.
  Hardware-trap exits classify as runtime failures, matching reference
  rejection semantics.
- Observed: div probe 7/7 met, boolean probe PASS 9/9 through the AOT route
  on this previously-execution-less host.

### External targets (new): RISC-V, eBPF, PTX

New `external.rs` adapter family parameterized by target spec; lowering reuses
the shared LLVM scalar lowering verbatim, then invokes system `llc`:

| Adapter | Triple | Artifact | Observed on this host |
| --- | --- | --- | --- |
| `mncs-riscv32` | riscv32 `-mattr=+m` | ELF32 relocatable, RV32IM | genuine objects produced; disassembly shows real RV32IM code incl. zero/MIN guards |
| `mncs-ebpf` | bpfel | ELF64 eBPF object | produced; upstream llc rejects signed division — surfaced as a precise target restriction |
| `mncs-ptx64` | nvptx64 | PTX text module | produced with version/target headers |

Execution refuses honestly: no RISC-V emulator, no privileged kernel-verifier
run, no CUDA driver/ptxas here. Refusals name the environment capability.

## Toolchain identity (observations)

- clang/LLVM 22.1.8 (Fedora 22.1.8-4.fc44); `llc` with riscv32/riscv64,
  bpfel, nvptx64 targets registered.
- gcc available as fallback linker; llvm-readobj/llvm-objdump present.
- No sudo this session: no qemu-user, no spirv-tools CLI, no ptxas installed.
  SPIR-V codegen therefore remains planned (recorded in the matrix), not
  silently claimed.

## Conformance corpus and differential results

`crates/mncs-cli/tests/backend_family.rs` plus
`examples/execution/backend-conformance/`:

| Corpus | bytecode | wasm | c11 | llvm | cranelift(AOT) |
| --- | --- | --- | --- | --- | --- |
| checked div/mod (7 cases incl. failures) | 7/7 | 7/7 | 7/7 | 7/7 | 7/7 |
| strict booleans (9) | PASS | PASS | PASS | PASS | PASS |
| core status lattice+records (30) | PASS | PASS | refusal | refusal | refusal |
| Result payload sums (8) | 8/8 | 8/8 | refusal | refusal | refusal |

Refusals are structured CG* diagnostics naming exact unsupported semantic
requirements — part of the contract, not noise.

## Remaining genuine limitations (classified)

- Language/compiler limitation: composite signatures through the
  C11/LLVM/Cranelift process ABI (struct-ABI drivers) — next slice; design in
  this record. Saturating/widening intents remain scalar-refused there too.
- Backend implementation limitation: eBPF signed division (upstream ISA);
  PTX lacks ptxas validation here.
- Target limitation: no emulators/runtimes installed → external families are
  artifact-realization-only on this host.
- Host/toolchain limitation: no sudo; qemu-user/spirv-tools/ptxas could not
  be installed.

## Reproduction

```text
cargo test --workspace
cargo test -p mncs-cli --test backend_family
cargo run -p mncs-cli -- experiment matrix
cargo run -p mncs-cli -- compile examples/source/profile06-boolean-operators.mncs \
    --emit backend --target mncs-riscv32
llvm-objdump -d <emitted .o>
```
