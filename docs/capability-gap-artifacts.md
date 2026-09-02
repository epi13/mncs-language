# Capability-Gap Artifacts

Status: experimental compiler integration design.

When MNCS Language cannot express, lower, execute, or verify a requested operation, the compiler should be able to emit a structured capability-gap artifact. This turns language pressure into inspectable evidence without making the compiler depend on Commons or Forge.

## Artifact contract

A capability-gap artifact has a canonical, content-addressed identity and contains:

- `artifact_kind: mncs.capability_gap` and `contract_revision`;
- `gap_id), producer/compiler identity, source revision, and source artifact digest;
- source location when available;
- `requested_semantics`;
- `obstruction`: parser, typing, semantic validation, lowering, runtime, backend, library, or verification;
- active profile and backend set;
- closest supported semantics, if any;
- bounded reproducer/input reference;
- protected properties and `approximation_prohibited`;
- evidence requirements;
- scoped PASS/FAIL/UNKNOWN status and unresolved fields.

Identity includes every field that changes semantics, scope, or reproduction inputs. Human wording, line numbers, and local paths cannot be the sole identity inputs.

## Emission rules

Emit the artifact when understood semantics are outside the active profile, a backend cannot realize valid semantics, a required library/runtime operation is absent, verification cannot establish an obligation, or approximation would change a protected property. Ordinary diagnostics may accompany it.

Do not claim a feature is impossible in general; record the current bounded limitation and scope. Malformed input for an already supported operation remains ordinary negative evidence, not a capability gap.

## Consumption and evidence

The language repository remains artifact-neutral. Forge may ingest the artifact through a public adapter and Commons may publish a corresponding CapabilityGap record, but neither dependency is required to compile or execute language code.

A resolving compiler change links the original gap, candidate/source revision, profile/backend, reproducer/corpus, observed checks, and remaining unknowns. Older artifacts are never overwritten; amendments or new linked artifacts preserve history.

Candidate evidence distinguishes parser/semantic acceptance, reference execution, each backend, negative/boundary behavior, library integration, ABI compatibility, and independent replication where required. Unsupported backends remain UNKNOWN unless the declared obligation makes them FAIL. Finite tests support only their declared scope.

The first implementation can be schema, fixtures, and integration tests for one known limitation, including canonical identity, deterministic serialization, local-path redaction, and honest tri-state status.
