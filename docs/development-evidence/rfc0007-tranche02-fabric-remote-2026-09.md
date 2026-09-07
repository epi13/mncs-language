# RFC 0007 tranche-0.2 remote proof-corpus check (2026-09)

Second worker axis for the tranche-0.2 corpora: the frozen main corpus
(`examples/execution/proof-dep-corpus.json`, 20 cases) and the frozen
admission corpus (`examples/execution/proof-dep-admission-corpus.json`,
21 cases) were executed on a genuine remote Fabric worker through an
embedded mTLS client (`FabricClient` + `load_registry`, same shape as the
harness CUDA probe), not the same-host `run local` mediation used in
tranche 0.1.

## Outcome

- Job `rfc0007-tranche02-proof-corpus`, plan identity
  `sha256:9c60a2aa2db3e3b7b4e1550fbb3fd8b2dde0858f1e301476fe9015f3ab3f129d`.
- Candidate identity
  `sha256:0a6dc909a8ff10fb4c90e36eb137beb5dee28c108cfa1a7ec9dc02a6c716be3c`
  (kernel `mncs:proof-kernel:0.2`, main 20 + admission 21 cases).
- Artifact manifest identity
  `sha256:ea4f16585f122325bda53d5d3154a58609e944670a103b7ff8a7d96e50721831`
  (verified byte-identical against the submitted bundle root before dispatch).
- Disposition `EXECUTED`, outcome `PASS`, termination `COMPLETED`, exit `0`,
  duration ~386s.
- Worker `fabric-worker-01`: Fedora 43, x86_64, 8 CPUs, glibc 2.42,
  Python 3.14.6, `DECLARED_OFFLINE` network policy.
- Task stdout verdict:
  `{"verdict": "PASS", "cases": 82, "runs": [["main-bytecode", true],
  ["main-wasm", true], ["admission-bytecode", true],
  ["admission-wasm", true]]}` — every case output matched its pinned
  expectation byte-for-byte under an exact comparator that fails loudly on
  any unreadable shape (no fuzzy matching, no skipped cases).
- Result receipt `result.json`:
  `sha256:96dc3e855c5c61d10b38cfb5b79419fec5c3587a5`
  (384 bytes, recorded in the execution record).

## Transport notes (all inside the content-addressed bundle)

- The pinned binary is the exact local
  `cargo build --release -p mncs-cli` bytes,
  sha256 `6a335eaadf03832147fd7e5fec8aa142d53f279ab726c09290bcf7ec6ecb939f`
  (max required glibc 2.39, worker provides 2.42).
- The Fabric execution bundle caps members at 8 MiB, so the ~18 MiB binary
  travels as `bin/mncs.part-*` chunks. The worker task reassembles them and
  refuses to execute unless the bytes match the pinned identity above — the
  chunking is transport only, never a second build.
- Backends on the worker are the two embedded ones (research bytecode and
  portable WASM MVP): no toolchain is required or assumed on the worker.

## Honest boundaries

- One remote worker, not two: `collamore02-windows` (Windows 10, Python
  3.11.0) answered a platform probe (`EXECUTED`/`PASS`) but there is no
  Windows `mncs` build in scope, so it checked no corpus case. No verdict
  is claimed from it. Ledger criterion `0007-C16` stays partial for exactly
  this reason, plus single-worker scope.
- The persistent-controller service path refused this job twice
  (`CAPABILITY_UNAVAILABLE: ['python']`, then `ADMISSION_EXHAUSTED`) because
  its cached capability inventory for the worker is stale (a Fabric-side
  observation issue, untouched by this pass). Dispatch used the embedded
  mTLS client with a fresh authenticated description instead; the refusal
  is recorded here, not hidden.
- A Fabric PASS is an execution observation with receipts, never a proof.
  Checker agreement (MNCS kernel versus the independent reference checker)
  remains the consistency evidence.
- The full execution record is not checked in: it embeds worker-local
  network identifiers with no evidentiary value. Everything needed to
  reproduce it is above: the corpora, the kernel sources, the pinned
  binary identity, the plan/manifest identities, and the
  `plan -> manifest -> embedded execute(worker_id=fabric-worker-01)`
  command shape with the task sources beside this note's bundle root
  (`task.py` reduction logic: exact `expectation_met`/`status_met` per case).
