# Issue #108: per-case experiment cost — measured cause and fix (2026-09-05)

## Cost structure (debug toolchain, Commons mesh kernels, this host)

Slope measured over `experiment run` with 1-vs-3 (lifecycle) and 1-vs-2
(interest_named) case slices, same binary, both backends:

| kernel | cases | research-bytecode (before) | portable-wasm (before) |
|---|---|---|---|
| `commons.mesh.lifecycle` (pure i64 law) | slope | 0.86 s/case | 0.21 s/case |
| `commons.mesh.interest_named` (3x `text_map.lookup16`) | slope | 26.7 s/case | 14.9 s/case |

`MNCS_TIMINGS=1` stage breakdown showed two per-case cost centers:

1. `cli-case-observations`: `execute_backend` re-decoded the artifact on
   **every case** — research-bytecode re-deserialized the full
   Program+SsaModule JSON payload and re-ran `program.validate()`,
   `module.validate()`, fingerprinting, and block-index construction per
   case. WASM re-decoded its binary module per case (cheap). This is the
   entire ~4x backend gap on trivial law.
2. `cli-translation-validation` (`compare_body_ssa_and_backend`): three
   executions per case (body + SSA + backend), each repeating the same
   per-case decode/validate work. On textmap kernels the body and SSA
   reference interpreters additionally step every MNCS-level `iterate`
   iteration (`text.matches16` is a 16-step loop; `lookup16` scans the
   table), which dominates both backends equally.

## Fix (owning layer: `mncs-codegen`, consumed by `mncs-cli`)

New `BackendExecutionSession` (`crates/mncs-codegen/src/lib.rs`): decode
the artifact and build the `SsaExecutionSession` (validation, fingerprints,
block indexes) **once per corpus**; every case still executes through the
same bounded runner with all request-specific checks on every call.
Preparation failure falls back to the one-shot path, so validation
semantics never weaken. Wired into `experiment run`, `experiment execute`,
and both legs of `compare_body_ssa_and_backend`.

Pinned by `execution_session_reuse_matches_one_shot_on_both_backends`:
session observations equal one-shot observations (status, returned, steps,
artifact digest) on both backends, and a byte-tampered artifact declines
reuse and fails closed.

## After (same probes, same host)

| kernel | research-bytecode | portable-wasm |
|---|---|---|
| lifecycle slope | **0.10 s/case** (8.6x) | 0.09 s/case |
| interest_named slope | **10.4 s/case** (2.6x) | 10.3 s/case |

Backend-specific overhead is eliminated: both backends now cost exactly the
shared reference-interpretation price. Consequences:

- 484-case lifecycle corpus: ~52 s single invocation (was >600 s timeout).
- 25-case interest_named corpus: ~290 s (was >600 s timeout).
- MNCS-Commons adopted the fix by raising `KERNEL_CHUNK_CASES`
  (lifecycle 150 -> 500, interest_named 10 -> 15); chunking remains as a
  safety batching, not the fix.

## Remaining pressure (still open, not weakened)

The residual ~10.3 s/case on textmap kernels is the iteration-exact
reference interpretation of MNCS-level loops, identical on both backends.
If corpus scale needs more, the honest next step is the issue's option 2:
an exact-cost-but-cheap builtin exact-match primitive with declared
iteration cost — a language-design change for the owners, not a harness
shortcut. This fix deliberately does not touch that contract.
