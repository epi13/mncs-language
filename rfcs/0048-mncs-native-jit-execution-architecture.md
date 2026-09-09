# RFC 0048: MNCS-native JIT / execution orchestration architecture

Status: **Implemented** (experimental).
Scope: `library/jit/`, JIT corpora, `docs/jit-architecture.md`.
Related: RFC 0016 (staging/specialization), RFC 0017 (runtime/
execution-context), RFC 0038 (compiler refinement), RFC 0041
(backend plurality).

## Summary

Establish the reusable MNCS-language JIT/execution orchestration
layer inside `mncs-language`, with Cranelift as the first native
execution provider. The JIT layer owns session state, execution
planning, definition generations, logical bindings, redefinition,
dependency/invalidation state, artifact identity and metadata,
provider selection, code lifetime, profiling hooks, specialization
hooks, and proof-aware execution metadata. Providers own lowering,
compilation, publication, invocation, and retirement of executable
artifacts. The Rust bootstrap compiler is used only as the compiler:
it compiles the MNCS implementation and executes definitions
through existing backend paths. No JIT architecture lives in Rust.

## Motivation

Today `MNCS -> Cranelift` is a compile-and-run step with no
persistent execution model: no sessions, no redefinition, no
generations, no invalidation, no proof linkage, no lifetime story.
A self-hosted `mncs-compiler` and a future extracted `mncs-jit`
both need that orchestration layer, and it must be owned by MNCS
itself (so it survives the bootstrap) rather than by the temporary
Rust compiler.

## Design

### Layering

```text
MNCS source → frontend → semantic analysis → proof kernel
    → canonical MNCS IR → MNCS JIT subsystem
        ├── JitSession (mncs.jit.session.v1)
        ├── ExecutionPlan (mncs.jit.plan.v1)
        ├── DefinitionGeneration (session Definition rows)
        ├── DefinitionBinding (mncs.jit.binding.v1)
        ├── DependencySet (mncs.jit.depends.v1 edges)
        ├── InvalidationSet (mncs.jit.depends.v1 verdicts)
        ├── ExecutableArtifact (mncs.jit.types.v1 Executable)
        ├── ExecutionProvider (mncs.jit.types.v1 Provider table)
        ├── CodeLifetime (mncs.jit.lifecycle.v1)
        ├── ProfileState (mncs.jit.profile.v1)
        └── SpecializationContext (mncs.jit.proof.v1)
               ├── Cranelift provider (implemented)
               ├── interpreter provider (Reference standing member)
               ├── LLVM JIT provider (named, deferred)
               └── native MNCS provider (named, deferred)
```

### Generational model

Logical identity is stable; source generations append (global
counter); each generation compiles to zero or more executables from
possibly different providers; bindings select at most one; old
executables persist under explicit lifecycle states. No emitted
code is ever overwritten: publication is generational with logical
indirection.

### Invalidation

Explicit policy matrix over (change kind, edge kind) with
transitive escalation, bounded closure, and fail-closed unknowns
(see `docs/jit-architecture.md` section 5). The data model covers
body/signature/contract changes; layout/type/capability/proof
changes route through the Contract kind and era mechanisms today.

### Proof awareness

Artifact and definition triples
`(semantic_generation, proof_generation, assumption_bits)` are
checked at publication and reported at resolve. Specialization
admits only dual-key (micro-verifier + kernel) evidence. No
speculative unsafe optimization exists.

### What this RFC does not do

No VM build-out, no adaptive optimizer, no machine-code generator,
no LLVM parity, no GPU tiering, no REPL, no `mncs-compiler`
changes, no self-hosting. Those are explicitly future; the hooks
they need (provider table, tier parameters, advisory flags,
observation records) are present.

## Consequences

- `library/jit/` (8 MNCS modules, ~2.7k lines with tests-in-source
  candidates) plus 8 corpora (32 cases) is the extraction unit for
  a future `mncs-jit`.
- The bootstrap compiler required **zero** production changes for
  this subsystem: the 0.10 surface sufficed. Pressure discovered
  along the way is recorded as JIT-PRESSURE entries, not as
  drive-by compiler edits.
- Known limits: table capacities (8/8/16/16), whole-program
  artifacts per generation, exact-match assumption bits, host-
  assigned definition codes and content fingerprints, threaded
  wall-clock observations. Each is documented with its future
  owner (section 10 of the architecture document).

## Evidence

- 32/32 corpus candidates agree between `mncs-research-bytecode`
  and `mncs-cranelift` (plus full five-backend agreement for the
  six small modules and layered agreement for all eight).
- `crates/mncs-embed/tests/jit_execution.rs`: five chapters —
  basic execution, persistent session, safe redefinition (old code
  stays invocable), failed-generation rollback, dependency policy,
  proof-era refusal, retirement, close, Cranelift-native session
  threading, determinism replay.
