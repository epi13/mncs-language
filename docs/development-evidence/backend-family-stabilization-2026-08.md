# Backend family stabilization evidence — 2026-08-25

Status: **development record**. Local observations on one host plus GitHub
Actions results are named separately throughout. Nothing here is
production-readiness or universal equivalence.

## Starting condition

`main` at `29f41e8` (PR #76 merge). Both GitHub workflows failed:

- `Compiler platform portability`: `checked_division_agrees_across_executable_backends`
  panicked with `experiment JSON for mncs-cranelift: EOF while parsing a value
  at line 1 column 0` (the CLI process died before emitting JSON), and
  `external_targets_produce_genuine_artifacts_and_refuse_execution_honestly`
  failed its top-level-status assumption.
- `CI`: formatting check failure.

Local runs of the same suite were green, which is exactly why they were not
evidence of correctness; the two failures were environment-dependent.

## Defect 1: Cranelift checked division could kill the compiler process

The Cranelift lowering emitted native `sdiv`/`srem` (CLIF text) and
`sdiv`/`srem` builder instructions (JIT/AOT path), relying on hardware traps
to represent invalid checked arithmetic.

Why it passed locally but failed in CI: this development host enforces an
SELinux policy that denies JIT executable memory, so execution silently took
the AOT child-process route where a SIGFPE-killed child classifies as a
runtime failure. On CI hosts JIT works; the compiled function ran inside the
CLI process and divide-by-zero / `MIN / -1` raised SIGFPE against the compiler
itself.

Fix (semantics, not test avoidance):

- Division never relies on traps. A zero divisor — always — and signed
  `MIN / -1` (division only) branch to the shared structured-failure block
  (`status=1`, zero value), identical to the reference executors.
- Unsigned operands use `udiv`/`urem`. i64 value slots are normalized to the
  declared width first (sign-extension for narrow signed types, mask for
  unsigned) so slot history cannot change division semantics.
- CLIF text emission mirrors the guarded form so the artifact honestly
  describes the realized lowering.
- JIT and AOT realize the identical builder path, so both classifications are
  the same by construction.
- New availability-gated test
  (`jit_guarded_division_never_traps_the_host_process`) exercises the guarded
  paths under JIT where host policy permits (CI Linux/Windows) and skips
  honestly where executable memory is denied.

## Defect 2: unsigned division was miscompiled on C11 and portable WASM

A new conformance case (`udiv-high-bit`: `4294967295u32 / 2`) exposed that
C11 divided through the signed C variable type and WASM selected `div_s`
unconditionally; both returned `0` where the reference executor returns
`2147483647`. LLVM IR already selected `udiv`; Cranelift happened to be right
only because i64 slots hold unsigned values zero-extended (now explicit and
guarded).

Fixes: C11 casts operands into the unsigned modular domain before `/`/`%`;
WASM selects `div_u`/`rem_u` for unsigned operand types (new opcodes in
encoder/decoder/executor) and applies the `MIN / -1` guard only for signed
division, where the corner exists.

Unsigned division is now agreed across all five executable backends by the
`unsigned_division_agrees_across_executable_backends` corpus.

## Defect 3: external-target conformance conflated semantic state with artifact success

The removed test assumed top-level `completed`, but compilation status folds
in program-obligation state; a successful backend lowering of a module with
unresolved obligations is `completed_with_unresolved_obligations`. The tests
now treat these as independent axes and assert each precisely:

- lowering success: coded backend diagnostics and/or artifact presence;
- artifact production: digest plus decodable bytes;
- artifact validity: structural checks (below);
- execution availability: honest per-case `unsupported` refusals.

Independent per-target tests exist for `mncs-riscv32`, `mncs-ebpf`, and
`mncs-ptx64` so one failure cannot suppress another target's coverage.
Where the LLVM toolchain is absent (e.g. Windows runners without `llc`),
artifact tests accept the structured CGX401 toolchain refusal instead of
fabricating coverage; Linux CI installs LLVM so the positive artifact paths
are authoritative there.

## Defect 4: external artifact validation did not validate the artifact

The previous validator spawned `llvm-readobj` with nulled stdin, discarded
its output, ignored its exit code, and counted "process spawned" as
validation. Validation now parses the produced bytes directly:

- ELF objects: magic, ELF class (ELF32 for riscv32, ELF64 for bpfel),
  little-endian encoding, ET_REL, `e_machine` (243 RISC-V / 247 BPF),
  `.text` presence, and symbol-table definitions for every declared export.
- PTX modules: `.version`, `.target sm_60` (realized explicitly via
  `-mcpu=sm_60` so the artifact matches the declared contract instead of the
  toolchain-default sm_30), `.address_size 64`, and a visible function
  definition per export.

Refusals gained precise identities: `CGX401` toolchain unavailable,
`CGX400`/`CGX402` tool failures, `CGX410` upstream target restriction
(e.g. eBPF's refusal to lower signed division, asserted by its own test),
`CGX411` artifact failing structural validation.

## Local observations (this host)

- Fedora 44, LLVM 22.1.8, SELinux enforcing (JIT execmem denied).
- `cargo test --workspace`: 191 passed, 0 failed (including the ten
  backend-family cases).
- `cargo fmt --all -- --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean.
- RISC-V/eBPF/PTX artifacts all produce and validate internally.
- eBPF signed-division module: `completed_with_unresolved_obligations` with
  CGX410 and no artifact — the restriction is represented, not hidden.
- The JIT-path test skips here (host policy); it is exercised by CI.

## Remaining genuine limitations

- Composite values across the C11/LLVM/Cranelift process boundary remain
  refused by design until the canonical value/ABI tranche lands.
- eBPF signed division is an upstream ISA/toolchain restriction; MNCS
  represents it as CGX410 rather than mis-lowering.
- No GPU/emulator runtimes here or in generic CI: external targets remain
  artifact-realization targets with honest execution refusals.
