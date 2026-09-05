# Exotic-backend execution evidence — 2026-09

## Scope

First genuine execution results for three MNCS backends that were previously
compile-only, plus a Windows cross-compilation proof and a strengthened
differential gate. Program under test throughout: `bounded_min` from
`examples/source/bounded-min.mncs` (six cases: ordered, reversed, equal,
negative, zero, negative pair). All artifact bytes were independently
re-verified outside the compiler (`file`, `llvm-objdump`, driver API).

## Results

- `mncs-riscv32` RV32-IMC object (`fda53eaa…`), statically linked with a
  freestanding harness and executed under user-mode `qemu-riscv32-static`
  10.2.2 (Fedora `qemu-user-static-riscv`, host x86_64): **6/6 PASS**.
  Labeled `arch.riscv32 + execution.emulated + emulator.qemu-user-10.2.2`.
- `mncs-ebpf` object executed in the userspace ubpf VM (safe profile:
  interpreter-only, provenance-tracked, descriptor-checked regions):
  `ubpf_load` validation **ACCEPT**, **6/6 PASS**. This is *not* kernel
  verifier evidence (`unprivileged_bpf_disabled=2`, no `CAP_BPF` on the
  probe host); the backend `execution_note` still truthfully says no kernel
  verifier ran.
- `mncs-c11` translation unit compiled freestanding for AArch64 and executed
  under user-mode `qemu-aarch64-static` 10.2.2: **6/6 PASS**. Same unit
  compiled natively for x86_64: **6/6 PASS** with per-case output agreement.
- `mncs-c11` unit cross-compiled with llvm-mingw 20260826 (ucrt) to a
  Windows PE32+ console executable; `bounded_min` disassembled inside the PE
  implements the correct min-select logic. Compile-conformant;
  execution-unverified (no Windows worker, no Wine on the probe host).
- `mncs-ptx64` (prior session): MNCS-generated PTX JIT-loaded via the CUDA
  driver API and executed on a real Quadro P620 (compute 6.1, driver
  580.173.02): 4/4 correct.

## Differential agreement

Native x86_64, RV32/QEMU, AArch64/QEMU, PTX/GPU, and eBPF/ubpf all agree on
all cases. Pinned mechanically by
`crates/mncs-cli/tests/backend_evidence.rs`: execution-boundary honesty per
external backend, byte determinism across repeated compilation, and
identical `function_value_contracts` across riscv32/ebpf/ptx64.

## Open pressure (owners)

- PTX backend emits `.func`, not a launchable `.entry`; direct
  `cuModuleGetFunction` fails with CUDA error 500. Owner: codegen PTX path
  (optional entry emission).
- Kernel-verifier evidence for `mncs-ebpf` needs a privileged eBPF-capable
  worker. Owner: Fabric capability provisioning.
- Physical RISC-V/ARM and any Apple runtime evidence need real workers.
  Owner: Fabric capability provisioning.
- Windows execution evidence needs a Windows worker. Owner: Fabric.
