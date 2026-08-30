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

## Reuse integrity matrix

Every reusable object has an explicit key, immutable subject, validator
version, dependency set, scope, and invalidation rule:

| reusable object | key and immutable subject | validator / dependencies / scope | invalidators |
| --- | --- | --- | --- |
| `SsaExecutionSession` | Owned `Arc<Program>` plus `Arc<SsaModule>`; program content fingerprint and exact SSA fingerprint are retained in the receipt. | SSA execution-session validator `0.1`; execution contract `0.1`; dependencies are the program identity → program fingerprint and SSA identity → SSA fingerprint; scope is the session instance. | Program or SSA content/identity/schema changes, or validator/execution-contract changes. The execution API accepts only the session-owned pair, so A's receipt cannot be applied to B. |
| Research bytecode payload/session | Artifact identity and bytes hash; decoded payload shares the immutable program/SSA allocations with its `SsaExecutionSession`. | Payload schema `0.1`, artifact identity validation, and the SSA session receipt; scope is the backend artifact identity. | Artifact bytes, backend identity, payload schema, program/SSA identity or content, or session validator changes. |
| WASM, Cranelift, C11, and LLVM backend sessions | Borrowed `BackendArtifact` identity plus the backend-specific prepared representation. | Artifact identity/hash and backend preparation validation; native sessions additionally retain toolchain name, path, version, source/IR, and flags; scope is the artifact identity. | Any artifact mutation (also prevented by the borrow), identity/hash mismatch, backend or target contract change, toolchain change, or preparation failure. |
| Stateful prefix checkpoint | Exact corpus/bounds/step-prefix identity plus sealed logical state and optional backend-session scope. | Stateful checkpoint schema `0.1`; seal covers retained values, observations, counters, program identity/fingerprint, and scope; scope is the owning artifact identity for backend reuse. | Prefix step/policy/bound changes, retained-state or seal tampering, out-of-range index, cross-artifact scope, or invalid retained references. Divergent suffixes may share only an exact verified prefix. |
| Native executable cache | Full framed source material, compiler name/path/version, flags, host OS, and architecture hashed to a complete cache directory identity. | External compiler observation; publication is same-directory temp-file compile followed by atomic rename and a completion marker containing the executable size; `backend_compile_count` increments only after an external compiler invocation succeeds. | Source, compiler, flags, host identity, missing/invalid completion marker, size-truncated output, compile failure, or failed publication. The child-process protocol remains the final check for same-size binary corruption. |
| Evidence receipt | Receipt identity seals fact, subject, validator, scope, dependencies, outcome, and invalidation triggers. | Evidence receipt schema `0.1`; dependency map contains exact content fingerprints. | Any sealed field, subject, validator, scope, or dependency fingerprint change. |

`CostReport` keeps the legacy cumulative checkpoint timing fields and adds an
explicit adjacent-checkpoint exclusive timing field. `backend_compile_count`
now counts only an actual external native compiler invocation that published a
new executable; native cache hits do not masquerade as compilation. Backend
validation and lowering counters are recorded at their shared boundaries so
portable and external adapters use the same counter names.

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

- Receipt tests cover exact reuse, A/B program and SSA mutations, caller-side
  mutation after session creation, schema/identity/block-order tampering, and
  dependency/identity tampering.
- Stateful checkpoint tests prove that a verified prefix skips only the
  captured calls, can be reused by a divergent suffix, and rejects state,
  policy, range, seal, and cross-artifact-scope mutations.
- `cargo fmt --all -- --check`, `cargo check --workspace`, and the model suite
  pass. The complete Atlas five-backend differential also passes, with its
  machine-readable report at `/tmp/atlas-cost-reuse-full.json` for this run.
- Timing samples retain the existing cumulative compiler trace points under
  their legacy field and record independent adjacent CLI checkpoints under the
  explicit exclusive-duration field; they remain diagnostic measurements.
- Local execution is not a sandbox. RSS and wall-time results are host-local;
  no cross-host performance or independent evaluator claim follows.
