# RFC 0046: machine-native cost, reuse, and evidence-efficiency tranche

- **Status:** Implemented experimentally (2026-08-29)
- **Scope:** compiler stages, bounded stateful execution, and evidence receipts

## Problem

Repeated compiler and experiment work was paying for the same immutable
validation, artifact decoding, backend setup, and large logical-value copies.
The resulting wall time obscured which work was reused, while an unbound cache
could accidentally make stale observations look current.

## Design

The prototype records a machine-readable `CostReport` beside every `experiment
run` result. It reports stage counters, cumulative checkpoint timing,
independent adjacent-checkpoint timing, backend lowering/compile/validation/
session counts, artifact decode/hash counts, stateful calls, and reused versus
new execution counts. `backend_compile` means a successful external compiler
invocation; a native cache lookup is not a compile. The report is observability
only; it is never consulted for semantic validity.

`SsaExecutionSession` validates the exact semantic program and SSA artifact
once, builds immutable block indexes once, owns shared immutable copies of that
exact pair, and retains an `EvidenceReceipt`. `execute` accepts only a request;
it cannot be given a second program or SSA module. The receipt binds a fact to
its subject, validator, scope, dependency fingerprints, assumptions, and
invalidation triggers. Reuse is allowed only when the receipt identity and
every requested dependency match. This is the central invariant: cached
validation is reusable only for the exact immutable subject represented by the
validation evidence.

`BackendStatefulSession` keeps one decoded WASM module, research-bytecode
payload/session, Cranelift JIT session, or prepared C11/LLVM executable set
alive for a corpus. Each session borrows one identity-validated artifact and
uses that artifact identity as the checkpoint scope. C11 and LLVM still launch
one isolated native child per transition, preserving the external toolchain
boundary; their session reuses artifact decoding, driver preparation, and
compile/link results. Stateful prefixes may be captured only at maximal exact
shared step sequences, are capped in count, are scoped to the artifact-bound
session, and retain enough logical prior results for the suffix to be
revalidated. An invalid or incomplete checkpoint is an `InvalidRequest`, never
a cache hit.

Native source/IR and object/link caches use full framed source, compiler path
and version, flags, and host identity keys. Successful compiler output is
published through a same-directory temporary-file rename and a completion
marker; missing, truncated, or failed entries are not cache hits. Same-size
binary corruption remains an external runtime observation and fails through
the native protocol rather than becoming semantic evidence.

Large logical aggregate fields use reference-counted immutable storage. This
changes only physical copying; canonical serialization and the logical JSON
shape remain unchanged. Mutable vector/sequence updates copy the aggregate
before modification. The bounded evaluator still owns step, call, and trace
limits.

## Evidence and status semantics

Reuse is an execution optimization, not additional semantic evidence. A
successful stateful trace remains bounded empirical observation. `FAIL` retains
precedence over `UNKNOWN`, and `UNKNOWN` retains precedence over `PASS`; a
deferred validator, timeout, unsupported backend, invalid identity, or budget
exhaustion is not promoted by a timing result. The Atlas stress corpus uses an
explicit eight-million-step per-call bound because its finalizer exceeds one
million SSA instructions; the bound remains finite and is reported when
exceeded.

## Current envelope and limits

The tranche covers the reference compiler's semantic/HIR/SSA stages, portable
WASM, research bytecode, C11, LLVM, Cranelift, and the stateful experiment
runner. It does not claim a general incremental compiler, durable checkpoint
format, process sandbox, formal compiler correctness, or exact asymptotic cost.
Checkpoint storage is process-local and bounded; no cache is trusted across a
new artifact, validator, contract, schema, or execution environment. The
completion marker is a bounded corruption/partial-publication guard, not a
cryptographic attestation of an external executable's semantics.
