# MNCS cost, reuse, and evidence efficiency — 2026-08

## Scope

This tranche measures and reduces repeated work in `mncs-language` compiler and
bounded stateful execution paths. It preserves the language-owned evidence
boundary: backend success is execution evidence, not conformance or production
readiness.

## Implemented changes

- `CostReport` is emitted as `cost-report.json` beside `experiment run`
  outputs. Counters cover parse, semantic validation, HIR, SSA, backend
  compilation/validation, artifact decoding/hashing, serialization, session
  setup, stateful calls, prefix checkpoints, and reused/new execution.
- `SsaExecutionSession` performs immutable program/SSA validation, block-index
  construction, and fingerprints once per session. Its validation receipt is
  identity-bound to the program, SSA artifact, validator, schema, and
  execution contract.
- `BackendStatefulSession` reuses decoded portable-WASM modules, research
  bytecode payloads plus SSA sessions, Cranelift JIT sessions, and prepared
  C11/LLVM native executables across a corpus. C11/LLVM still keep one child
  process per transition because their external protocol is request-scoped;
  the session removes repeated preparation and compile/link work without
  moving native tools into the semantic trust boundary. Unsupported or
  invalid preparation falls back to ordinary per-call adapter execution.
- Stateful traces support capped, process-local, exact-prefix checkpoints.
  Prefix identity excludes case labels but includes corpus, bounds, and exact
  step definitions; the session additionally binds the checkpoint to the
  immutable artifact identity.
- Logical aggregate values use reference-counted immutable backing storage,
  avoiding repeated deep copies of Atlas records and bounded sequences while
  preserving canonical serialized values. Stateful previous-result arguments
  move a value only at its last in-trace use.
- The Atlas differential generator records an explicit 8,000,000-step request
  bound for its large finalizer. This remains a resource limit, not an
  unbounded escape hatch.

## Measurements

Commands used for the primary stress measurements:

```bash
cargo check --workspace
cargo build --release -p mncs-cli
MNCS_LIBRARY_PATH=../mncs-language/library \
  target/release/mncs experiment run mncs/atlas-model.mncs \
  --backend mncs-research-bytecode \
  --corpus /tmp/atlas-all-stateful-8m.json \
  --output-dir /tmp/atlas-research-all-8m-out \
  --validation-profile artifact-build
```

The generated Atlas corpus contains one bounded probe and 22 stateful traces.
The research-bytecode stress completed in **530.99s** with **688,392 KiB peak
RSS**. Every stateful case returned; the experiment status remained `UNKNOWN`
under `artifact-build` because the documented layered validation job is
deferred. The cost report recorded:

| measure | result |
| --- | ---: |
| artifact decodes | 1 |
| backend sessions | 1 |
| stateful traces / calls | 22 / 6,521 |
| prefix checkpoints captured | 14 |
| prefix reuses | 1 |
| new executions | 1 |

The complete single-trace research run completed in **53.73s** at **524,024
KiB peak RSS** after aggregate sharing, returning the previously exhausted
Atlas finalizer under the explicit 8M bound. Cranelift completed the same 22
stateful traces in **86.57s** at **662,872 KiB peak RSS**. The earlier
one-million-step cap classified the research finalizer as `BUDGET_EXHAUSTED`;
that result was retained rather than treated as a pass.

The complete Atlas five-backend differential then passed across the bounded
mutation fixture and all **22 stateful traces**. Each backend produced one
reusable session, **5,904** stateful calls, **9** captured checkpoints, and
**10** verified prefix reuses. The differential runner remains explicitly
empirical: its report status is `PASS` for agreement and expectation coverage,
not a conformance or production-readiness claim.

Before aggregate sharing, the single full research trace took about 127.5s and
hit the one-million-step cap. The copy-on-write change reduced wall time to
about 53.9s before the explicit stress budget was raised.

## Verification and limitations

- Receipt tests cover exact reuse and dependency/identity tampering.
- Stateful checkpoint tests prove that a verified prefix skips only the
  captured calls and retains the same final logical result.
- `cargo fmt --all -- --check`, `cargo check --workspace`, and the model suite
  pass. The complete Atlas five-backend differential also passes, with its
  machine-readable report at `/tmp/atlas-cost-reuse-full.json` for this run.
- Timing samples include existing cumulative compiler trace points; they are
  diagnostic measurements and should not be added as independent exclusive
  stage durations without accounting for nesting.
- Local execution is not a sandbox. RSS and wall-time results are host-local;
  no cross-host performance or independent evaluator claim follows.
