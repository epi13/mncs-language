# RFC Conformance

> Generated from `rfcs/conformance-ledger.json` by
> `scripts/gen_rfc_ledger_docs.py`. Do not edit by hand.

Ledger revision `bfa455a5453ee36edfa3987de12b136bed95356c` covering 46 RFCs: 68 satisfied, 27 partially satisfied, 38 unsatisfied criteria.

## Reading this document

- **Design status** (`DRAFT`, `PROPOSED`, `ACCEPTED`, `STABLE`, `SUPERSEDED`)
  tracks the maturity of the RFC text.
- **Implementation status** (`NONE`, `SUBSTRATE`, `PARTIAL`,
  `BOUNDED_IMPLEMENTATION`, `IMPLEMENTED_EXPERIMENTALLY`, `IMPLEMENTED`)
  tracks what the repository can actually do, with evidence per criterion.
- The two axes never collapse: a `DRAFT` design can have a bounded
  implementation, and an `ACCEPTED` design can remain unimplemented.

## Status overview

| RFC | Title | Design | Implementation |
| --- | ----- | ------ | -------------- |
| 0001 | Semantic Foundation | STABLE | IMPLEMENTED_EXPERIMENTALLY |
| 0002 | Contract and Evidence Model | ACCEPTED | IMPLEMENTED_EXPERIMENTALLY |
| 0003 | Verified Intermediate Representation | DRAFT | PARTIAL |
| 0004 | Recursive Introspection and Refinement | DRAFT | PARTIAL |
| 0005 | Source Representations and Semantic Density | PROPOSED | SUBSTRATE |
| 0006 | Machine-Intent Expressions and Verified Lowering Envelopes | DRAFT | BOUNDED_IMPLEMENTATION |
| 0007 | Proof-Carrying Dependent Core | DRAFT | BOUNDED_IMPLEMENTATION |
| 0008 | Machine-Native I/O, Resource, Effect, and Event Semantics | DRAFT | PARTIAL |
| 0009 | Machine-Native Memory, Reference, Provenance, and Storage Semantics | DRAFT | SUBSTRATE |
| 0010 | Machine-Native Concurrency, Causality, Atomicity, and Memory Consistency Semantics | DRAFT | NONE |
| 0011 | Machine-Native Failure, Recovery, Nondeterminism, and External Observation Semantics | DRAFT | PARTIAL |
| 0012 | Machine-Native Executable Semantic Core | DRAFT | BOUNDED_IMPLEMENTATION |
| 0013 | Machine-Native Abstraction, Polymorphism, Interface, and Evidence Semantics | DRAFT | BOUNDED_IMPLEMENTATION |
| 0014 | Machine-Native Module, Component, Linking, Dependency, and Compatibility Semantics | DRAFT | PARTIAL |
| 0015 | Machine-Native Trust Boundary, Unsafe Operation, Foreign Interface, ABI, and Containment Semantics | DRAFT | SUBSTRATE |
| 0016 | Machine-Native Staging, Metaprogramming, Introspection, Specialization, and Self-Transformation Semantics | DRAFT | PARTIAL |
| 0017 | Machine-Native Runtime, Environment, Target, and Execution-Context Semantics | DRAFT | PARTIAL |
| 0018 | Machine-Native Assurance Profiles, Evidence Algebra, Trust Policy, and Witness Semantics | DRAFT | PARTIAL |
| 0019 | Machine-Native Value, Data, Type, and Representation Semantics | DRAFT | BOUNDED_IMPLEMENTATION |
| 0020 | Machine-Native Identity, Equality, Equivalence, Refinement, Substitutability, and Compatibility Semantics | DRAFT | PARTIAL |
| 0021 | Machine-Native Numeric, Arithmetic, Precision, Error, and Reproducibility Semantics | DRAFT | BOUNDED_IMPLEMENTATION |
| 0022 | Machine-Native Termination, Productivity, Progress, Liveness, and Bounded-Computation Semantics | DRAFT | PARTIAL |
| 0023 | Machine-Native Time, Clock, Deadline, Temporal Validity, and Real-Time Semantics | DRAFT | NONE |
| 0024 | Machine-Native Information Flow, Confidentiality, Integrity, Declassification, and Side-Channel Semantics | DRAFT | NONE |
| 0025 | Machine-Native Principal, Identity, Authentication, Credential, Attestation, and Cryptographic Trust Semantics | DRAFT | SUBSTRATE |
| 0026 | Machine-Native Persistent State, Durability, Transaction, Snapshot, Journal, and Crash-Consistency Semantics | DRAFT | NONE |
| 0027 | Machine-Native Serialization, Canonical Encoding, Schema, Wire Contract, and Data-Evolution Semantics | DRAFT | BOUNDED_IMPLEMENTATION |
| 0028 | Machine-Native Distribution, Messaging, Partial Failure, Consistency, Replication, Consensus, and Failure-Detector Semantics | DRAFT | NONE |
| 0029 | Machine-Native Placement, Topology, Locality, Mobility, Migration, and Heterogeneous-Execution Semantics | DRAFT | SUBSTRATE |
| 0030 | Machine-Native Deployment, Lifecycle, Live Upgrade, Version Coexistence, State Migration, Rollback, and Retirement Semantics | DRAFT | SUBSTRATE |
| 0031 | Machine-Native Build, Derivation, Reproducibility, Artifact Lineage, Supply-Chain, and Attestation Semantics | DRAFT | SUBSTRATE |
| 0032 | Machine-Native Complexity, Quantitative Cost, Resource Accounting, QoS, Energy, and Performance Semantics | DRAFT | PARTIAL |
| 0033 | Machine-Native Realization Search, Multi-Objective Optimization, Pareto, Risk, Preference, and Selection-Policy Semantics | DRAFT | SUBSTRATE |
| 0034 | Machine-Native Test, Fuzz, Coverage, Experiment, Benchmark, Oracle, and Empirical-Evidence Semantics | DRAFT | BOUNDED_IMPLEMENTATION |
| 0035 | Machine-Native Elaboration, Scope, Binding, Inference, Constraint, Defaulting, Coherence, and Resolution Semantics | DRAFT | PARTIAL |
| 0036 | Machine-Native Language, Specification, Feature, Compatibility, Migration, Deprecation, and Evolution Semantics | DRAFT | SUBSTRATE |
| 0037 | Machine-Native Observability, Audit, Telemetry, Trace Correlation, Operational Evidence, and Runtime-Verification Semantics | DRAFT | SUBSTRATE |
| 0038 | Machine-Native Compiler, Compilation, Refinement, Validation, and Cross-Target Semantics | DRAFT | PARTIAL |
| 0039 | Machine-Native Compiler Stage and Experiment Contracts | PROPOSED | PARTIAL |
| 0040 | Machine-Native Source Profile 0.1 | PROPOSED | IMPLEMENTED_EXPERIMENTALLY |
| 0041 | Machine-Native Backend Plurality and Realization Contracts | PROPOSED | BOUNDED_IMPLEMENTATION |
| 0042 | Machine-Native Source Profile 0.3 Experiment Bootstrap | PROPOSED | IMPLEMENTED_EXPERIMENTALLY |
| 0043 | Machine-Native Source Profile 0.4 Bounded Iteration | ACCEPTED | IMPLEMENTED_EXPERIMENTALLY |
| 0044 | Machine-Native Bounded Sequence, Byte, and View Semantics | ACCEPTED | IMPLEMENTED_EXPERIMENTALLY |
| 0045 | Machine-Native Bounded Stateful Execution Traces | ACCEPTED | IMPLEMENTED_EXPERIMENTALLY |
| 0046 | Machine-Native Cost, Reuse, and Evidence-Efficiency Tranche | ACCEPTED | IMPLEMENTED_EXPERIMENTALLY |

## Entries

### RFC 0001 — Semantic Foundation

- Design: **STABLE**; implementation: **IMPLEMENTED_EXPERIMENTALLY** (experimental; confidence high).
- Scope: Semantic program model, validation, canonical identities, and fingerprints across crates/mncs-model.
- Stages: semantic program, validation.
- Required by: RFC 0002, RFC 0003, RFC 0038.
- Tests: `crates/mncs-model unit tests (validation, identity, canonical)`.
- Acceptance criteria:
  - [x] 0001-C1: Versioned semantic program model with validation — evidence: `spec/semantic-core.md`, `crates/mncs-model/src/validation.rs`
  - [x] 0001-C2: Canonical representation with deterministic identities — evidence: `crates/mncs-model/src/canonical.rs`, `crates/mncs-model/src/identity.rs`
  - [ ] 0001-C3: Production hardening beyond the experimental envelope
- Known gaps:
  - Production hardening and soundness beyond the bounded experimental tranche remain open.
- Pressure sources: all downstream RFC tranches.
- Note: RFC header predates the split ledger and reads 'Implemented experimentally'; recorded here as design STABLE with experimental implementation.

### RFC 0002 — Contract and Evidence Model

- Design: **ACCEPTED**; implementation: **IMPLEMENTED_EXPERIMENTALLY** (experimental; confidence high).
- Scope: Obligations, evidence manifests and receipts, verifier results, PASS/FAIL/UNKNOWN discipline with UNKNOWN never promoted.
- Profiles: 0.2+.
- Stdlib: mncs.core.status.v1.
- Stages: semantic program, evidence manifest.
- Depends on: RFC 0001.
- Required by: RFC 0003, RFC 0006, RFC 0007, RFC 0018.
- Tests: `crates/mncs-model unit tests (evidence, verifier, obligations)`, `crates/mncs-cli/tests/semantic_commands.rs`.
- Acceptance criteria:
  - [x] 0002-C1: Obligations generated by language constructs with stable identities — evidence: `crates/mncs-model/src/obligations.rs`
  - [x] 0002-C2: Identity-bound evidence with invalidation on dependency change — evidence: `crates/mncs-model/src/evidence.rs`
  - [~] 0002-C3: Method-scoped outcomes distinguishing proof, certificate, verifier result, observation, assumption — evidence: `crates/mncs-model/src/verifier.rs`, `library/core/proof_check.mncs`
- Known gaps:
  - Full method taxonomy enforcement across every evidence producer; so far carried by verifier.rs authority classes plus the new proof kernel.
- Pressure sources: RFC 0007 tranche, RFC 0018 assurance work.
- Note: RFC header predates the split ledger and reads 'Implemented experimentally'.

### RFC 0003 — Verified Intermediate Representation

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence high).
- Scope: HIR, SSA, selected SSA, translation validators, proof-obligation survival into SSA; proof-preserving lowering is partial.
- Stages: HIR, SSA, selected SSA.
- Depends on: RFC 0001, RFC 0002.
- Required by: RFC 0007, RFC 0038.
- Tests: `crates/mncs-model unit tests (ssa, translation)`, `crates/mncs-cli/tests/semantic_commands.rs`.
- Acceptance criteria:
  - [x] 0003-C1: HIR and verified SSA with validation — evidence: `crates/mncs-model/src/ir.rs`, `crates/mncs-model/src/ssa.rs`
  - [x] 0003-C2: Translation validation over finite corpora — evidence: `crates/mncs-translation-check/src/lib.rs`
  - [~] 0003-C3: Proof obligations survive lowering without accidental loss — evidence: `crates/mncs-model/src/ssa.rs`, `crates/mncs-codegen/src/promises.rs`
  - [ ] 0003-C4: Proof-preserving lowering with explicit proof transport
- Known gaps:
  - Proof objects do not yet travel through HIR/SSA as first-class artifacts; obligations and certificates travel, kernel proofs attach at the SSA boundary (elision) and lowering boundary (certificate binding).
- Pressure sources: RFC 0007 tranche.

### RFC 0004 — Recursive Introspection and Refinement

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence medium).
- Scope: Refinement model, candidate evaluation pipeline, semantic patches; self-transformation remains experimental.
- Stages: semantic program.
- Depends on: RFC 0001, RFC 0002.
- Tests: `crates/mncs-model unit tests (refinement, experiment)`.
- Acceptance criteria:
  - [x] 0004-C1: Semantic patch representation for recursive change — evidence: `crates/mncs-model/src/refinement.rs`, `spec/recursive-refinement.md`
  - [~] 0004-C2: Candidate evaluation with bounded evidence — evidence: `crates/mncs-model/src/experiment.rs`
  - [ ] 0004-C3: Closed-loop self-transformation under proof-carrying policy
- Known gaps:
  - Closed-loop policy and proof-carrying promotion are future work.

### RFC 0005 — Source Representations and Semantic Density

- Design: **PROPOSED**; implementation: **SUBSTRATE** (absent; confidence medium).
- Scope: Experimental syntax laboratory and density metrics; no grammar selected.
- Depends on: RFC 0001.
- Required by: RFC 0040, RFC 0042, RFC 0043.
- Acceptance criteria:
  - [~] 0005-C1: Candidate surface comparison with tokenizer-neutral metrics — evidence: `docs/source-syntax-lab.md`, `docs/concept-reconstruction-experiments.md`
  - [ ] 0005-C2: Final grammar selection
- Known gaps:
  - Grammar selection explicitly deferred by project policy.

### RFC 0006 — Machine-Intent Expressions and Verified Lowering Envelopes

- Design: **DRAFT**; implementation: **BOUNDED_IMPLEMENTATION** (experimental; confidence high).
- Scope: Integer arithmetic intents, facts/requirements/obligations, lowering envelopes, evidence-gated nsw/nuw pilot, proof-bound certificates (RFC 0007 tranche).
- Profiles: 0.6, 0.8.
- Stdlib: mncs.core.numeric.v1, mncs.core.vector.v1.
- Stages: HIR, SSA, selected SSA, lowering.
- Depends on: RFC 0001, RFC 0002, RFC 0003.
- Required by: RFC 0007, RFC 0021.
- Tests: `crates/mncs-cli/tests/profile06_explicit_intents.rs`, `crates/mncs-cli/tests/backend_evidence.rs`.
- Acceptance criteria:
  - [x] 0006-C1: Explicit arithmetic intents with edge semantics — evidence: `examples/source/profile06-explicit-intents.mncs`, `crates/mncs-compiler/src/frontend.rs`
  - [x] 0006-C2: Facts, requirements, and obligations preserved through IR — evidence: `crates/mncs-model/src/machine_intent.rs`
  - [x] 0006-C3: Conservative backend promises gated by current evidence — evidence: `crates/mncs-codegen/src/promises.rs`
  - [ ] 0006-C4: Generalization beyond the constant-range nsw/nuw pilot
- Known gaps:
  - Memory, aliasing, alignment, and relaxation promises await their semantics (explicit roadmap item).
- Pressure sources: RFC 0007 tranche (proof-bound certificates).

### RFC 0007 — Proof-Carrying Dependent Core

- Design: **DRAFT**; implementation: **BOUNDED_IMPLEMENTATION** (experimental; confidence high).
- Scope: Tranche 0.1: flat topological proof buffers, bounded universes, dependent Pi formation, Nat family with closed computation, propositional equality, deterministic dual checkers (MNCS-native plus independent reference), obligation binding with invalidation, proof-gated SSA elision, and proof-bound lowering certificates. Dependent application/substitution, erasure, and full proof transport remain explicit gaps.
- Profiles: 0.10.
- Stdlib: mncs.core.proof_term.v1, mncs.core.proof_check.v1.
- Stages: semantic program, SSA, selected SSA, lowering, backend execution.
- Depends on: RFC 0001, RFC 0002, RFC 0003, RFC 0006, RFC 0012, RFC 0013, RFC 0019, RFC 0020, RFC 0021, RFC 0022, RFC 0027.
- Required by: RFC 0003, RFC 0006, RFC 0018, RFC 0020, RFC 0021, RFC 0022, RFC 0034, RFC 0038.
- Tests: `crates/mncs-cli/tests/proof_kernel.rs`, `crates/mncs-model proof_kernel unit tests`.
- Acceptance criteria:
  - [x] 0007-C1: Canonical proof-core representation with versioned terms — evidence: `library/core/proof_term.mncs`, `crates/mncs-model/src/proof_kernel.rs`
  - [x] 0007-C2: Explicit inspectable proof terms with canonical identities — evidence: `library/core/proof_term.mncs`, `crates/mncs-model/src/proof_kernel.rs`
  - [x] 0007-C3: Bounded explicit universe hierarchy without hidden impredicativity — evidence: `library/core/proof_check.mncs`, `examples/execution/proof-kernel-corpus.json`
  - [~] 0007-C4: Dependent function types beyond monomorphized generics — evidence: `library/core/proof_check.mncs`
  - [x] 0007-C5: At least one inductive family with dependent reasoning (Nat) — evidence: `library/core/proof_check.mncs`, `examples/execution/proof-kernel-corpus.json`
  - [x] 0007-C6: Explicit propositional equality kept separate from other equalities — evidence: `library/core/proof_check.mncs`, `docs/rfc-0007-evidence.md`
  - [x] 0007-C7: Deterministic checking to PASS/FAIL/UNKNOWN that never promotes UNKNOWN — evidence: `library/core/proof_check.mncs`, `crates/mncs-cli/tests/proof_kernel.rs`
  - [x] 0007-C8: Proof identities and kernel identity/version — evidence: `crates/mncs-model/src/proof_kernel.rs`
  - [x] 0007-C9: Obligation integration: compiler-generated obligation discharged by a proof term — evidence: `crates/mncs-cli/tests/proof_demo.rs`, `crates/mncs-model/src/proof_kernel.rs`, `crates/mncs-model/src/ssa.rs`
  - [x] 0007-C10: Invalidation: changed inputs invalidate proof reuse — evidence: `crates/mncs-model/src/proof_kernel.rs`
  - [x] 0007-C11: MNCS-native checker executing through real backend paths — evidence: `library/core/proof_check.mncs`, `crates/mncs-cli/tests/proof_kernel.rs`
  - [x] 0007-C12: Independent second checker with differential agreement — evidence: `crates/mncs-model/src/proof_kernel.rs`, `crates/mncs-cli/tests/proof_kernel.rs`
  - [x] 0007-C13: Valid, invalid, adversarial, and unresolved fixtures — evidence: `examples/execution/proof-kernel-corpus.json`, `examples/execution/proof-kernel-fuzz-corpus.json`
  - [~] 0007-C14: Real compiler integration with explicit proof relationship — evidence: `crates/mncs-model/src/ssa.rs`, `crates/mncs-model/src/machine_intent.rs`
  - [x] 0007-C15: Execution through research bytecode and portable WASM (plus C11/LLVM/Cranelift) — evidence: `crates/mncs-cli/tests/proof_kernel.rs`
  - [~] 0007-C16: Heterogeneous Fabric worker evidence — evidence: `docs/rfc-0007-evidence.md`
  - [x] 0007-C17: No proof claim implemented through test success or backend agreement alone — evidence: `docs/rfc-0007-evidence.md`, `crates/mncs-model/src/verifier.rs`
  - [x] 0007-C18: Explicit trust boundary with untrusted generation — evidence: `docs/rfc-0007-evidence.md`, `library/core/proof_check.mncs`
  - [x] 0007-C19: Proof-core stratification: total kernel versus effectful execution — evidence: `library/core/proof_term.mncs`, `docs/rfc-0007-evidence.md`
  - [ ] 0007-C20: Proof erasure semantics — evidence: `0007-G6`
- Known gaps:
  - 0007-G1 dependent application needs substitution (UNKNOWN boundary)
  - 0007-G2 NatElim over Succ-literals and stuck scrutinees (UNKNOWN boundary)
  - 0007-G3 open proof terms need assumption accounting (UNKNOWN boundary)
  - 0007-G4 proof transport through every compiler stage unresolved
  - 0007-G5 file-based proof ingestion into mncs compile is a follow-up
  - 0007-G6 proof erasure semantics unimplemented
  - 0007-G7 one-step beta reduction deferred
- Pressure sources: mncs-actions proof-kernel conformance, Fabric heterogeneous validation.
- Note: Design stays DRAFT (unresolved calculus choices per the RFC); implementation is a bounded experimental tranche, not the full RFC vision. C14/C16 flip to satisfied only with full proof transport plus file-based ingestion, and heterogeneous worker evidence, respectively.

### RFC 0008 — Machine-Native I/O, Resource, Effect, and Event Semantics

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence medium).
- Scope: Effects, capabilities, authority, and closure obligations; general I/O and resource semantics remain narrow.
- Profiles: 0.2+.
- Stages: semantic program.
- Depends on: RFC 0001, RFC 0002.
- Required by: RFC 0009, RFC 0010.
- Tests: `crates/mncs-cli/tests/semantic_commands.rs`, `examples/source/cre2-*.mncs`.
- Acceptance criteria:
  - [x] 0008-C1: Effect/capability declarations with closure checking — evidence: `spec/effects-and-capabilities.md`, `crates/mncs-model/src/authority.rs`
  - [~] 0008-C2: Authority preservation across calls — evidence: `crates/mncs-model/src/obligations.rs`
  - [ ] 0008-C3: General I/O, resource, and event semantics
- Known gaps:
  - I/O and resource vocabularies beyond capabilities are not yet modeled.

### RFC 0009 — Machine-Native Memory, Reference, Provenance, and Storage Semantics

- Design: **DRAFT**; implementation: **SUBSTRATE** (experimental; confidence medium).
- Scope: Narrow capability pilots (alignment, disjoint ranges); no general memory model.
- Depends on: RFC 0001, RFC 0002, RFC 0008.
- Tests: `crates/mncs-model unit tests (machine_intent)`.
- Acceptance criteria:
  - [~] 0009-C1: Evidence-bearing memory capability pilots — evidence: `crates/mncs-model/src/machine_intent.rs`
  - [ ] 0009-C2: General memory, reference, and provenance semantics
- Known gaps:
  - General memory model explicitly missing per roadmap.

### RFC 0010 — Machine-Native Concurrency, Causality, Atomicity, and Memory Consistency Semantics

- Design: **DRAFT**; implementation: **NONE** (absent; confidence low).
- Scope: No concurrency semantics implemented in mncs-language.
- Depends on: RFC 0008, RFC 0009.
- Acceptance criteria:
  - [ ] 0010-C1: Concurrency and memory-consistency model
- Known gaps:
  - Entire RFC vision unimplemented; blocked on memory semantics (0009).
- Note: Survey-level classification; deepen if concurrency work lands.

### RFC 0011 — Machine-Native Failure, Recovery, Nondeterminism, and External Observation Semantics

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence medium).
- Scope: Failure-mode vocabulary, isolated failure semantics, runtime-failure classification across backends.
- Profiles: 0.2+.
- Stages: semantic program, backend execution.
- Depends on: RFC 0001.
- Tests: `crates/mncs-cli/tests/backend_family.rs`.
- Acceptance criteria:
  - [x] 0011-C1: Failure-mode declarations with conservative preservation — evidence: `spec/semantic-core.md`, `examples/source/flagship.mncs`
  - [x] 0011-C2: Runtime-failure classification (division by zero, MIN/-1) on executables — evidence: `crates/mncs-cli/tests/backend_family.rs`
  - [ ] 0011-C3: Recovery, nondeterminism, and external-observation semantics
- Known gaps:
  - Recovery and nondeterminism models remain future work.

### RFC 0012 — Machine-Native Executable Semantic Core

- Design: **DRAFT**; implementation: **BOUNDED_IMPLEMENTATION** (experimental; confidence high).
- Scope: Reference executors, research bytecode, portable WASM, C11, LLVM, Cranelift over the declared envelope; artifact-only RISC-V/eBPF/PTX.
- Profiles: 0.1-0.10.
- Stages: backend execution.
- Depends on: RFC 0001, RFC 0003.
- Required by: RFC 0007, RFC 0017, RFC 0041.
- Tests: `crates/mncs-cli/tests/backend_family.rs`, `crates/mncs-cli/tests/backend_evidence.rs`.
- Acceptance criteria:
  - [x] 0012-C1: Reference execution with step budgets — evidence: `crates/mncs-model/src/execution.rs`, `crates/mncs-model/src/ssa_execution.rs`
  - [x] 0012-C2: Five executable backend adapters with bounded agreement — evidence: `crates/mncs-codegen/src/lib.rs`, `docs/development-evidence/backend-family-2026-08.md`
  - [x] 0012-C3: Honest capability/refusal reporting per backend — evidence: `crates/mncs-codegen/src/external.rs`
  - [ ] 0012-C4: General memory/layout and unrestricted loops
- Known gaps:
  - General memory/layout and unrestricted loops explicitly missing per roadmap.
- Pressure sources: RFC 0007 tranche (kernel execution on all five backends).

### RFC 0013 — Machine-Native Abstraction, Polymorphism, Interface, and Evidence Semantics

- Design: **DRAFT**; implementation: **BOUNDED_IMPLEMENTATION** (experimental; confidence high).
- Scope: Track 1 explicit universal abstraction: Profile 0.10 function-level type/Nat parameters with deterministic monomorphization. Remaining tracks are design.
- Profiles: 0.10.
- Stdlib: mncs.core.sequences.v1 (generic folds), mncs.core.status.v1 (generic summaries).
- Stages: elaboration, specialization, lowering boundary.
- Depends on: RFC 0001, RFC 0014.
- Required by: RFC 0007.
- Tests: `crates/mncs-cli/tests/library_core.rs`, `examples/source/generic-negative-missing-args.mncs`.
- Acceptance criteria:
  - [x] 0013-C1: Explicit type/Nat parameters with deterministic specialization — evidence: `docs/source-profile-0.10.md`, `crates/mncs-model/src/generics.rs`, `docs/development-evidence/source-profile-0.10-generics-2026-08.md`
  - [x] 0013-C2: Cross-module generic resolution with nominal identities — evidence: `examples/source/status-generic-consumer.mncs`, `examples/source/generic-polymorphism.mncs`
  - [ ] 0013-C3: Remaining abstraction tracks (interfaces, evidence passing, higher kinds)
- Known gaps:
  - Higher-kinded parameters refused (MNE229); implicit evidence and traits are future tracks. Explicitly not parametricity or coherence proofs.
- Note: RFC header notes Track 1 and Track 12 explicitly; other tracks remain design.

### RFC 0014 — Machine-Native Module, Component, Linking, Dependency, and Compatibility Semantics

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence high).
- Scope: Module namespaces, imports with aliases, nominal cross-module identities, dependency closure in canonical form; component model partial.
- Profiles: 0.9, 0.10.
- Stdlib: mncs.core.*.v1 module namespaces.
- Stages: elaboration, resolution.
- Depends on: RFC 0001.
- Required by: RFC 0007, RFC 0013.
- Tests: `crates/mncs-compiler/tests/module_imports.rs`, `crates/mncs-cli/tests/library_resolution.rs`.
- Acceptance criteria:
  - [x] 0014-C1: Namespace imports with aliasing and identity preservation — evidence: `examples/source/profile09-stdlib-namespace-consumer.mncs`, `crates/mncs-compiler/src/resolution.rs`
  - [x] 0014-C2: Dependency closure in program identity — evidence: `crates/mncs-model/src/canonical.rs`, `docs/development-evidence/module-linking-maturity-2026-08.md`
  - [ ] 0014-C3: Full component/version-compatibility model
- Known gaps:
  - Component-level versioning and compatibility negotiation remain future work.

### RFC 0015 — Machine-Native Trust Boundary, Unsafe Operation, Foreign Interface, ABI, and Containment Semantics

- Design: **DRAFT**; implementation: **SUBSTRATE** (experimental; confidence medium).
- Scope: Executable/external adapter distinction with execution notes, language-owned ABI inspection; explicit unsafe/foreign semantics narrow.
- Stages: lowering, ABI.
- Depends on: RFC 0001, RFC 0002, RFC 0012.
- Required by: RFC 0007.
- Tests: `crates/mncs-cli/tests/abi_boundary.rs`, `crates/mncs-cli/tests/forge_boundary.rs`.
- Acceptance criteria:
  - [x] 0015-C1: Executable versus external adapter distinction with honest notes — evidence: `crates/mncs-codegen/src/external.rs`
  - [~] 0015-C2: Language-owned ABI inspection — evidence: `crates/mncs-cli/tests/library_core.rs`
  - [ ] 0015-C3: Proof-gated unsafe operations
- Known gaps:
  - Unsafe operations do not yet generate proof obligations; RFC 0007 names this as future pressure.
- Pressure sources: RFC 0007 tranche (trust-boundary framing).

### RFC 0016 — Machine-Native Staging, Metaprogramming, Introspection, Specialization, and Self-Transformation Semantics

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence medium).
- Scope: Deterministic generic specialization/monomorphization with provenance; general staging and metaprogramming are design.
- Profiles: 0.10.
- Stages: specialization.
- Depends on: RFC 0013.
- Tests: `crates/mncs-cli/tests/library_core.rs`.
- Acceptance criteria:
  - [x] 0016-C1: Deterministic specialization with declaration provenance — evidence: `crates/mncs-model/src/generics.rs`, `docs/source-profile-0.10.md`
  - [ ] 0016-C2: General staging, metaprogramming, and introspection
- Known gaps:
  - Staging and metaprogramming beyond monomorphization are unimplemented.

### RFC 0017 — Machine-Native Runtime, Environment, Target, and Execution-Context Semantics

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence medium).
- Scope: Backend capability manifests, execution policies, step budgets, frozen-experiment replication on one worker.
- Stages: backend execution.
- Depends on: RFC 0012, RFC 0041.
- Required by: RFC 0029.
- Tests: `crates/mncs-cli/tests/backend_family.rs`.
- Acceptance criteria:
  - [x] 0017-C1: Backend capability manifests and execution policies — evidence: `crates/mncs-model/src/execution.rs`, `crates/mncs-codegen/src/lib.rs`
  - [~] 0017-C2: Frozen-experiment replication across workers — evidence: `ROADMAP.md`
  - [ ] 0017-C3: General runtime/environment semantics
- Known gaps:
  - Multi-worker replication evidence lives mostly outside mncs-language (Fabric/Forge); heterogeneous proof-checking evidence added by the RFC 0007 tranche where workers permit.
- Pressure sources: RFC 0007 tranche (heterogeneous validation).

### RFC 0018 — Machine-Native Assurance Profiles, Evidence Algebra, Trust Policy, and Witness Semantics

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence high).
- Scope: Status lattice, evidence combination, family coherence verdicts, promotion gates; assurance profiles are policy-level.
- Stdlib: mncs.core.status.v1, mncs.family.coherence.v01.
- Depends on: RFC 0002.
- Required by: RFC 0007.
- Tests: `examples/execution/family-coherence-transition-corpus.json`, `examples/execution/family-coherence-promotion-corpus.json`.
- Acceptance criteria:
  - [x] 0018-C1: Evidence combination algebra that cannot promote UNKNOWN — evidence: `library/core/status.mncs`, `library/family/coherence.mncs`
  - [x] 0018-C2: Promotion gates refusing on unresolved UNKNOWN — evidence: `library/family/coherence.mncs`
  - [ ] 0018-C3: Full assurance profiles with witness semantics
- Known gaps:
  - Assurance profiles remain policy outside the language; kernel-proof authority class added by the RFC 0007 tranche.
- Pressure sources: RFC 0007 tranche (KernelProof authority).

### RFC 0019 — Machine-Native Value, Data, Type, and Representation Semantics

- Design: **DRAFT**; implementation: **BOUNDED_IMPLEMENTATION** (experimental; confidence high).
- Scope: Records, payload-bearing finite types, bounded sequences, views, bytes, vectors, masks, canonical composite cells across five backends.
- Profiles: 0.5, 0.6, 0.7, 0.8.
- Stdlib: mncs.core.sequences.v1, mncs.core.bytes.v1, mncs.core.vector.v1, mncs.core.mask.v1.
- Stages: elaboration, backend execution.
- Depends on: RFC 0001.
- Required by: RFC 0007, RFC 0027, RFC 0044.
- Tests: `crates/mncs-cli/tests/profile07_bounded_data.rs`, `crates/mncs-cli/tests/profile08_branchless_vectors.rs`, `crates/mncs-cli/tests/profile05_records.rs`.
- Acceptance criteria:
  - [x] 0019-C1: Composite values with canonical cells on all executable backends — evidence: `docs/development-evidence/canonical-composite-cells-2026-08.md`, `docs/development-evidence/nested-composite-sequences-2026-08.md`
  - [x] 0019-C2: Bounded sequences, views, bytes, vectors, masks — evidence: `library/core/sequences.mncs`, `library/core/bytes.mncs`, `library/core/vector.mncs`, `library/core/mask.mncs`
  - [~] 0019-C3: Record equality semantics and nested-record CRE coverage — evidence: `ROADMAP.md`
  - [ ] 0019-C4: General memory/layout representation
- Known gaps:
  - Record equality semantics and general layout remain roadmap items; masks/vectors as sequence elements refused.
- Pressure sources: RFC 0007 tranche (flat proof buffers over records/sequences).

### RFC 0020 — Machine-Native Identity, Equality, Equivalence, Refinement, Substitutability, and Compatibility Semantics

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence high).
- Scope: Semantic identities and fingerprints, digest ordering, propositional equality in the proof core; refinement/substitutability partial via deltas.
- Stdlib: mncs.core.identity.v1, mncs.core.proof_term.v1.
- Stages: semantic program.
- Depends on: RFC 0001.
- Required by: RFC 0007.
- Tests: `crates/mncs-cli/tests/proof_kernel.rs`, `crates/mncs-cli/tests/abi_boundary.rs`.
- Acceptance criteria:
  - [x] 0020-C1: Deterministic semantic identities with invalidation — evidence: `crates/mncs-model/src/identity.rs`, `library/core/identity.mncs`
  - [~] 0020-C2: Explicit propositional equality in the proof core — evidence: `library/core/proof_check.mncs`, `docs/rfc-0007-evidence.md`
  - [~] 0020-C3: Equivalence, refinement, and substitutability relations kept distinct — evidence: `crates/mncs-model/src/delta.rs`, `crates/mncs-model/src/refinement.rs`
  - [ ] 0020-C4: Unified substitutability/compatibility semantics
- Known gaps:
  - Definitional/propositional/refinement equalities are now separated by construction in the kernel, but the general RFC 0020 relation lattice is future work.
- Pressure sources: RFC 0007 tranche (equality stratification).

### RFC 0021 — Machine-Native Numeric, Arithmetic, Precision, Error, and Reproducibility Semantics

- Design: **DRAFT**; implementation: **BOUNDED_IMPLEMENTATION** (experimental; confidence high).
- Scope: Explicit integer intents, exact evaluation, wrapping/saturating/total division, kernel-checked closed computation; floats and precision models are future.
- Profiles: 0.6, 0.8.
- Stdlib: mncs.core.numeric.v1.
- Stages: elaboration, backend execution.
- Depends on: RFC 0001, RFC 0019.
- Required by: RFC 0006, RFC 0007.
- Tests: `crates/mncs-cli/tests/profile06_explicit_intents.rs`, `crates/mncs-cli/tests/profile08_numeric_boundary.rs`, `crates/mncs-cli/tests/proof_kernel.rs`.
- Acceptance criteria:
  - [x] 0021-C1: Explicit arithmetic intents with exact evaluation — evidence: `crates/mncs-model/src/machine_intent.rs`, `examples/source/profile06-explicit-intents.mncs`
  - [x] 0021-C2: Kernel-checked closed Nat computation (Plus/literals) — evidence: `library/core/proof_check.mncs`, `examples/execution/proof-kernel-corpus.json`
  - [ ] 0021-C3: Floating-point, precision, and error semantics
- Known gaps:
  - Floating-point relaxation and precision tracking are future work; kernel Nat covers closed literal computation only.
- Pressure sources: RFC 0007 tranche (closed computation proofs).

### RFC 0022 — Machine-Native Termination, Productivity, Progress, Liveness, and Bounded-Computation Semantics

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence high).
- Scope: Bounded iteration with ceilings, step budgets, acyclic calls, topological proof buffers; general totality checking is future.
- Profiles: 0.4.
- Stages: elaboration, backend execution.
- Depends on: RFC 0001, RFC 0012.
- Required by: RFC 0007.
- Tests: `examples/source/profile04-unbounded-while.mncs`, `crates/mncs-cli/tests/proof_kernel.rs`.
- Acceptance criteria:
  - [x] 0022-C1: Bounded iteration with static ceilings — evidence: `docs/development-evidence/source-profile-0.4-bounded-iteration-cre3-2026-08.md`
  - [x] 0022-C2: Execution step budgets on every backend path — evidence: `crates/mncs-model/src/execution.rs`
  - [x] 0022-C3: Proof-kernel termination by topological construction — evidence: `library/core/proof_term.mncs`, `docs/rfc-0007-evidence.md`
  - [ ] 0022-C4: General totality, productivity, and liveness claims
- Known gaps:
  - Totality checking and liveness claims remain future work.
- Pressure sources: RFC 0007 tranche (termination-by-construction).

### RFC 0023 — Machine-Native Time, Clock, Deadline, Temporal Validity, and Real-Time Semantics

- Design: **DRAFT**; implementation: **NONE** (absent; confidence low).
- Scope: No clock, deadline, or temporal-validity semantics in mncs-language.
- Depends on: RFC 0022.
- Acceptance criteria:
  - [ ] 0023-C1: Time/clock/deadline model
- Known gaps:
  - Entire RFC vision unimplemented; step budgets are abstract computation bounds, not clocks.
- Note: Survey-level classification.

### RFC 0024 — Machine-Native Information Flow, Confidentiality, Integrity, Declassification, and Side-Channel Semantics

- Design: **DRAFT**; implementation: **NONE** (absent; confidence low).
- Scope: No information-flow semantics in mncs-language.
- Depends on: RFC 0008.
- Acceptance criteria:
  - [ ] 0024-C1: Information-flow and declassification model
- Known gaps:
  - Entire RFC vision unimplemented.
- Note: Survey-level classification.

### RFC 0025 — Machine-Native Principal, Identity, Authentication, Credential, Attestation, and Cryptographic Trust Semantics

- Design: **DRAFT**; implementation: **SUBSTRATE** (experimental; confidence low).
- Scope: Issuance/attestation-adjacent authority machinery; no principal or credential semantics.
- Depends on: RFC 0002.
- Tests: `crates/mncs-model unit tests (authority)`.
- Acceptance criteria:
  - [~] 0025-C1: Authority and issuance binding machinery — evidence: `crates/mncs-model/src/authority.rs`
  - [ ] 0025-C2: Principal, credential, and attestation semantics
- Known gaps:
  - Credential and attestation models live outside mncs-language (rights-provenance).
- Note: Survey-level classification; authority.rs relevance is partial.

### RFC 0026 — Machine-Native Persistent State, Durability, Transaction, Snapshot, Journal, and Crash-Consistency Semantics

- Design: **DRAFT**; implementation: **NONE** (absent; confidence low).
- Scope: No durability or crash-consistency semantics; stateful execution traces are observations, not persistence.
- Depends on: RFC 0009.
- Acceptance criteria:
  - [ ] 0026-C1: Durability and crash-consistency model
- Known gaps:
  - Entire RFC vision unimplemented.
- Note: Survey-level classification.

### RFC 0027 — Machine-Native Serialization, Canonical Encoding, Schema, Wire Contract, and Data-Evolution Semantics

- Design: **DRAFT**; implementation: **BOUNDED_IMPLEMENTATION** (experimental; confidence high).
- Scope: Canonical JSON and composite cells, portable encoding.v1 with executable round-trip laws, JSON cursor/projection/stream modules.
- Profiles: 0.7.
- Stdlib: mncs.std.encoding.v1, mncs.std.json_cursor.v1.
- Stages: canonical form.
- Depends on: RFC 0019.
- Required by: RFC 0007.
- Tests: `crates/mncs-cli/tests/library_core.rs`, `examples/execution/library-std-encoding-corpus.json`.
- Acceptance criteria:
  - [x] 0027-C1: Canonical serialization with stable identities — evidence: `crates/mncs-model/src/canonical.rs`
  - [x] 0027-C2: Portable encodings with executable round-trip laws — evidence: `library/std/encoding.mncs`, `library/core/bytes.mncs`
  - [ ] 0027-C3: Schema evolution and wire-contract negotiation
- Known gaps:
  - Evolution and negotiation are future work; proof artifacts use canonical JSON with pinned schema versions.
- Pressure sources: RFC 0007 tranche (canonical proof artifacts).

### RFC 0028 — Machine-Native Distribution, Messaging, Partial Failure, Consistency, Replication, Consensus, and Failure-Detector Semantics

- Design: **DRAFT**; implementation: **NONE** (absent; confidence low).
- Scope: No distribution semantics in mncs-language; execution lives in Fabric/Forge.
- Depends on: RFC 0010, RFC 0011.
- Acceptance criteria:
  - [ ] 0028-C1: Distribution and consistency model
- Known gaps:
  - Entire RFC vision unimplemented in this repository.
- Note: Survey-level classification.

### RFC 0029 — Machine-Native Placement, Topology, Locality, Mobility, Migration, and Heterogeneous-Execution Semantics

- Design: **DRAFT**; implementation: **SUBSTRATE** (experimental; confidence low).
- Scope: Target lowering plans and frozen-experiment worker routing; no topology or migration semantics.
- Stages: target lowering plan.
- Depends on: RFC 0012, RFC 0017.
- Acceptance criteria:
  - [~] 0029-C1: Target plans and worker-routed frozen execution — evidence: `crates/mncs-model/src/compiler.rs`, `ROADMAP.md`
  - [ ] 0029-C2: Topology, locality, and migration semantics
- Known gaps:
  - Heterogeneous proof-checking evidence from the RFC 0007 tranche exercises worker routing where workers permit.
- Pressure sources: RFC 0007 tranche (heterogeneous validation).
- Note: Survey-level classification.

### RFC 0030 — Machine-Native Deployment, Lifecycle, Live Upgrade, Version Coexistence, State Migration, Rollback, and Retirement Semantics

- Design: **DRAFT**; implementation: **SUBSTRATE** (experimental; confidence medium).
- Scope: Change-set transition legality and promotion gates in the family coherence module; no deployment runtime.
- Stdlib: mncs.family.coherence.v01.
- Depends on: RFC 0018.
- Tests: `examples/execution/family-coherence-promotion-corpus.json`, `examples/execution/family-coherence-transition-corpus.json`.
- Acceptance criteria:
  - [~] 0030-C1: Change-set transition and promotion decision vocabulary — evidence: `library/family/coherence.mncs`
  - [ ] 0030-C2: Deployment lifecycle and live-upgrade semantics
- Known gaps:
  - Runtime deployment semantics live outside mncs-language.

### RFC 0031 — Machine-Native Build, Derivation, Reproducibility, Artifact Lineage, Supply-Chain, and Attestation Semantics

- Design: **DRAFT**; implementation: **SUBSTRATE** (experimental; confidence low).
- Scope: Provenance and lineage records, requirement identities; no derivation or supply-chain runtime.
- Depends on: RFC 0002, RFC 0027.
- Acceptance criteria:
  - [~] 0031-C1: Provenance and lineage record vocabulary — evidence: `crates/mncs-model/src/provenance.rs`, `library/core/lineage.mncs`
  - [ ] 0031-C2: Derivation, reproducibility, and attestation semantics
- Known gaps:
  - Derivation tracking and attestation live largely outside mncs-language.
- Note: Survey-level classification.

### RFC 0032 — Machine-Native Complexity, Quantitative Cost, Resource Accounting, QoS, Energy, and Performance Semantics

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence medium).
- Scope: Stage cost reporting, step budgets, iteration ceilings; QoS/energy/performance models are future.
- Profiles: 0.4.
- Depends on: RFC 0022.
- Required by: RFC 0046.
- Acceptance criteria:
  - [x] 0032-C1: Compiler-stage cost reporting — evidence: `crates/mncs-model/src/cost.rs`
  - [~] 0032-C2: Bounded resource ceilings in source semantics — evidence: `crates/mncs-model/src/obligations.rs`
  - [ ] 0032-C3: QoS, energy, and performance semantics
- Known gaps:
  - Quantitative cost beyond stage accounting is future work; kernel cost measurement added by the RFC 0007 tranche.
- Pressure sources: RFC 0007 tranche (kernel cost measurement).

### RFC 0033 — Machine-Native Realization Search, Multi-Objective Optimization, Pareto, Risk, Preference, and Selection-Policy Semantics

- Design: **DRAFT**; implementation: **SUBSTRATE** (experimental; confidence low).
- Scope: Candidate evaluation scaffolding and machine preferences; no optimizer or Pareto machinery.
- Depends on: RFC 0004, RFC 0006.
- Acceptance criteria:
  - [~] 0033-C1: Candidate evaluation with preferences — evidence: `crates/mncs-model/src/machine_intent.rs`, `crates/mncs-model/src/experiment.rs`
  - [ ] 0033-C2: Multi-objective search and selection policy
- Known gaps:
  - Search and optimization live outside mncs-language (Forge).
- Note: Survey-level classification.

### RFC 0034 — Machine-Native Test, Fuzz, Coverage, Experiment, Benchmark, Oracle, and Empirical-Evidence Semantics

- Design: **DRAFT**; implementation: **BOUNDED_IMPLEMENTATION** (experimental; confidence high).
- Scope: Experiment corpora with expectations, bounded agreement checks, backend evidence suites, proof-kernel fuzz corpus; coverage/benchmark/oracle models partial.
- Stages: backend execution.
- Depends on: RFC 0012.
- Required by: RFC 0007.
- Tests: `crates/mncs-cli/tests/backend_family.rs`, `crates/mncs-cli/tests/proof_kernel.rs`.
- Acceptance criteria:
  - [x] 0034-C1: Experiment corpora with machine-checked expectations — evidence: `examples/execution/`, `crates/mncs-model/src/experiment.rs`
  - [x] 0034-C2: Bounded cross-backend agreement as explicit non-proof evidence — evidence: `crates/mncs-cli/tests/backend_family.rs`, `crates/mncs-cli/tests/proof_kernel.rs`
  - [~] 0034-C3: Fuzz corpora with differential oracles — evidence: `examples/execution/proof-kernel-fuzz-corpus.json`, `scripts/gen_proof_fuzz.py`
  - [ ] 0034-C4: Coverage, benchmark, and oracle semantics
- Known gaps:
  - Coverage and benchmark models remain future work; empirical evidence is never promoted to proof by construction.
- Pressure sources: RFC 0007 tranche (differential fuzz oracle).

### RFC 0035 — Machine-Native Elaboration, Scope, Binding, Inference, Constraint, Defaulting, Coherence, and Resolution Semantics

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence medium).
- Scope: Elaboration with binding tables and resolution provenance, exhaustiveness, explicit generics; inference and defaults are narrow by design.
- Profiles: 0.5, 0.9, 0.10.
- Stages: elaboration, resolution.
- Depends on: RFC 0001.
- Required by: RFC 0013.
- Tests: `crates/mncs-compiler/tests/module_imports.rs`, `examples/source/profile05-record-values.mncs`.
- Acceptance criteria:
  - [x] 0035-C1: Scope/binding elaboration with provenance — evidence: `crates/mncs-compiler/src/resolution.rs`, `crates/mncs-model/src/bindings.rs`
  - [x] 0035-C2: Exhaustiveness and finite-type elaboration — evidence: `examples/source/cre1-evidence-combine.mncs`
  - [ ] 0035-C3: General inference, defaulting, and coherence
- Known gaps:
  - Inference is intentionally explicit-only (Profile 0.10); defaults and coherence are future work.

### RFC 0036 — Machine-Native Language, Specification, Feature, Compatibility, Migration, Deprecation, and Evolution Semantics

- Design: **DRAFT**; implementation: **SUBSTRATE** (experimental; confidence medium).
- Scope: Schema versions, profile-gated features, additive profile evolution; migration and deprecation machinery narrow.
- Profiles: 0.1-0.10.
- Depends on: RFC 0005.
- Acceptance criteria:
  - [~] 0036-C1: Versioned schemas with additive profile evolution — evidence: `spec/source-profile-0.1.md`, `docs/source-profile-0.10.md`
  - [ ] 0036-C2: Migration, deprecation, and compatibility machinery
- Known gaps:
  - Formal migration/deprecation semantics are future work.

### RFC 0037 — Machine-Native Observability, Audit, Telemetry, Trace Correlation, Operational Evidence, and Runtime-Verification Semantics

- Design: **DRAFT**; implementation: **SUBSTRATE** (experimental; confidence low).
- Scope: Execution traces, diagnostics, transformation records; telemetry and runtime verification are future.
- Stages: backend execution.
- Depends on: RFC 0002, RFC 0012.
- Acceptance criteria:
  - [~] 0037-C1: Execution traces and stable diagnostics — evidence: `crates/mncs-model/src/execution.rs`, `crates/mncs-model/src/provenance.rs`
  - [ ] 0037-C2: Telemetry, audit, and runtime-verification semantics
- Known gaps:
  - Operational telemetry lives outside mncs-language.
- Note: Survey-level classification.

### RFC 0038 — Machine-Native Compiler, Compilation, Refinement, Validation, and Cross-Target Semantics

- Design: **DRAFT**; implementation: **PARTIAL** (experimental; confidence high).
- Scope: Full pipeline source-to-backend with HIR/SSA/selection/lowering, translation validators, cross-target agreement; proof transport partial.
- Stages: source, semantic program, HIR, SSA, selected SSA, lowering, backend.
- Depends on: RFC 0003, RFC 0012, RFC 0041.
- Required by: RFC 0007.
- Tests: `crates/mncs-cli/tests/semantic_commands.rs`, `crates/mncs-cli/tests/backend_family.rs`.
- Acceptance criteria:
  - [x] 0038-C1: Staged pipeline with validated transitions — evidence: `spec/compiler-pipeline.md`, `crates/mncs-model/src/compiler.rs`, `crates/mncs-compiler/src/lib.rs`
  - [x] 0038-C2: Cross-target agreement checks — evidence: `crates/mncs-cli/tests/backend_family.rs`
  - [~] 0038-C3: Proof-carrying compilation stages — evidence: `crates/mncs-model/src/ssa.rs`, `docs/rfc-0007-evidence.md`
  - [ ] 0038-C4: Verified end-to-end compilation
- Known gaps:
  - Proof transport through every stage is an explicit unresolved boundary (ledger 0007-G4).
- Pressure sources: RFC 0007 tranche (proof-gated elision, proof-bound certificates).

### RFC 0039 — Machine-Native Compiler Stage and Experiment Contracts

- Design: **PROPOSED**; implementation: **PARTIAL** (experimental; confidence medium).
- Scope: Stage contracts and experiment harnesses with bounded corpora; full contract enforcement is future.
- Depends on: RFC 0038.
- Acceptance criteria:
  - [~] 0039-C1: Compiler stage contracts with evidence — evidence: `docs/development-evidence/compiler-stage-contracts-2026-08.md`
  - [~] 0039-C2: Experiment contracts over bounded corpora — evidence: `crates/mncs-model/src/experiment.rs`
- Known gaps:
  - Contract enforcement depth is future work.

### RFC 0040 — Machine-Native Source Profile 0.1

- Design: **PROPOSED**; implementation: **IMPLEMENTED_EXPERIMENTALLY** (experimental; confidence medium).
- Scope: Historical first executable source grammar; superseded in practice by later profiles but still accepted.
- Profiles: 0.1.
- Stages: source envelope, CST, AST.
- Depends on: RFC 0005.
- Required by: RFC 0042, RFC 0043.
- Tests: `examples/source/flagship.mncs`.
- Acceptance criteria:
  - [x] 0040-C1: Profile 0.1 grammar with envelope/CST/AST — evidence: `spec/source-profile-0.1.md`, `docs/development-evidence/source-profile-0.1-2026-08.md`
  - [x] 0040-C2: Continued acceptance under additive evolution — evidence: `docs/source-profile-0.10.md`

### RFC 0041 — Machine-Native Backend Plurality and Realization Contracts

- Design: **PROPOSED**; implementation: **BOUNDED_IMPLEMENTATION** (experimental; confidence high).
- Scope: Five executable adapters plus artifact-only RISC-V/eBPF/PTX with capability/refusal contracts and bounded agreement.
- Stages: lowering, backend execution.
- Depends on: RFC 0012, RFC 0017.
- Required by: RFC 0007, RFC 0012.
- Tests: `crates/mncs-cli/tests/backend_family.rs`, `crates/mncs-cli/tests/proof_kernel.rs`.
- Acceptance criteria:
  - [x] 0041-C1: Executable backend family with declared envelopes — evidence: `crates/mncs-codegen/src/lib.rs`, `docs/development-evidence/backend-plurality-assessment-2026-08.md`
  - [x] 0041-C2: Artifact-only paths with honest refusal states — evidence: `crates/mncs-codegen/src/external.rs`, `docs/development-evidence/exotic-backend-execution-2026-09.md`
  - [ ] 0041-C3: Universal backend equivalence
- Known gaps:
  - Equivalence is bounded agreement only, never claimed as proof.
- Pressure sources: RFC 0007 tranche (kernel on all five backends).

### RFC 0042 — Machine-Native Source Profile 0.3 Experiment Bootstrap

- Design: **PROPOSED**; implementation: **IMPLEMENTED_EXPERIMENTALLY** (experimental; confidence medium).
- Scope: Historical experiment bootstrap profile per its evidence record.
- Profiles: 0.3.
- Stages: source envelope.
- Depends on: RFC 0005, RFC 0040.
- Required by: RFC 0043.
- Tests: `examples/source/profile03-recursive-call.mncs`.
- Acceptance criteria:
  - [x] 0042-C1: Profile 0.3 experiment readiness — evidence: `docs/development-evidence/source-profile-0.3-experiment-readiness-2026-08.md`
- Note: RFC header reads 'Proposed and implemented as an experimental profile'.

### RFC 0043 — Machine-Native Source Profile 0.4 Bounded Iteration

- Design: **ACCEPTED**; implementation: **IMPLEMENTED_EXPERIMENTALLY** (experimental; confidence medium).
- Scope: Bounded iteration with static ceilings, authority closure, and completion modes.
- Profiles: 0.4.
- Stages: elaboration.
- Depends on: RFC 0042, RFC 0022.
- Tests: `examples/source/profile04-recursive-retry.mncs`, `examples/source/cre3-retry-authority.mncs`.
- Acceptance criteria:
  - [x] 0043-C1: Bounded iteration with ceilings and authority closure — evidence: `docs/development-evidence/source-profile-0.4-bounded-iteration-cre3-2026-08.md`
- Note: RFC header reads 'Implemented experimentally'; design recorded as ACCEPTED for the frozen experimental profile.

### RFC 0044 — Machine-Native Bounded Sequence, Byte, and View Semantics

- Design: **ACCEPTED**; implementation: **IMPLEMENTED_EXPERIMENTALLY** (experimental; confidence high).
- Scope: Exact sequences, bounded views, bytes, traversal-domain bounds discharge, canonical cells on five backends.
- Profiles: 0.7.
- Stdlib: mncs.core.sequences.v1, mncs.core.bytes.v1.
- Stages: elaboration, backend execution.
- Depends on: RFC 0019.
- Required by: RFC 0007.
- Tests: `crates/mncs-cli/tests/profile07_bounded_data.rs`.
- Acceptance criteria:
  - [x] 0044-C1: Bounded sequences, bytes, and views with traversal discharge — evidence: `docs/development-evidence/bounded-data-2026-08.md`, `library/core/sequences.mncs`
  - [x] 0044-C2: Five-backend realization with canonical cells — evidence: `docs/development-evidence/nested-composite-sequences-2026-08.md`
- Pressure sources: RFC 0007 tranche (proof buffers are bounded sequences).
- Note: RFC header reads 'Accepted for Source Profile 0.7 (experimental)'.

### RFC 0045 — Machine-Native Bounded Stateful Execution Traces

- Design: **ACCEPTED**; implementation: **IMPLEMENTED_EXPERIMENTALLY** (experimental; confidence medium).
- Scope: Bounded stateful traces (init, transition, finish) through ordinary value ABIs.
- Profiles: 0.10.
- Stages: backend execution.
- Depends on: RFC 0012.
- Acceptance criteria:
  - [x] 0045-C1: Bounded stateful traces on the experimental corpus — evidence: `docs/source-profile-0.10.md`, `crates/mncs-model/src/execution.rs`
- Known gaps:
  - Not a claim of persistent runtime semantics.
- Note: RFC header reads 'Accepted for the experimental execution corpus (2026-08-29)'.

### RFC 0046 — Machine-Native Cost, Reuse, and Evidence-Efficiency Tranche

- Design: **ACCEPTED**; implementation: **IMPLEMENTED_EXPERIMENTALLY** (experimental; confidence medium).
- Scope: Cost/reuse evidence-efficiency tranche per its evidence record; kernel cost measurement added by the RFC 0007 tranche.
- Depends on: RFC 0032.
- Required by: RFC 0007.
- Acceptance criteria:
  - [x] 0046-C1: Cost/reuse evidence-efficiency mechanisms — evidence: `docs/development-evidence/mncs-cost-reuse-evidence-efficiency-2026-08.md`
  - [~] 0046-C2: Proof-cache reuse bound to identity and assumptions — evidence: `crates/mncs-model/src/proof_kernel.rs`
- Known gaps:
  - Proof-cache reuse across changed dependencies is refused by binding checks; positive reuse infrastructure is future work.
- Pressure sources: RFC 0007 tranche (identity-bound proof reuse).
- Note: RFC header reads 'Implemented experimentally (2026-08-29)'.
