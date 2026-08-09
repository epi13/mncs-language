# Recursive Introspection and Refinement 0.1 Direction

This document defines architectural requirements for recursive debugging and improvement. The Rust
reference model now provides an intentionally partial versioned `0.2` artifact set for diagnostic
obligations, graph-based causal slices, finite refinement budgets, isolated repair proposals,
semantic/authority/evidence deltas, and promotion decisions. These types are an executable pilot,
not a complete refinement protocol or accepted surface schema.

## 1. Principle

An MNCS toolchain SHOULD represent diagnostics, repair proposals, semantic deltas, evidence deltas, and promotion decisions as machine-readable semantic artifacts. Outputs from one refinement cycle MAY become inputs to a later cycle.

Recursive refinement MUST remain bounded, authority-aware, and evidence-preserving.

## 2. Introspection surface

A conforming implementation SHOULD expose stable references to:

- semantic graph nodes;
- contract and obligation identities;
- effect and capability edges;
- assumptions and trust boundaries;
- evidence nodes and invalidation edges;
- IR transformations and provenance;
- diagnostics and causal slices;
- refinement history.

The introspection surface MUST NOT silently grant mutation authority. Observation and modification are distinct capabilities.

## 3. Diagnostic obligation

A diagnostic obligation SHOULD identify:

- subject identity;
- property identity or missing property class;
- diagnostic category;
- evidence state;
- assumptions consumed;
- causal slice or localization evidence;
- reproduction context where applicable;
- confidence and severity.

A diagnostic message without a stable subject and obligation identity is insufficient for recursive processing at higher assurance levels.

## 4. Repair proposal

A repair proposal MUST be treated as untrusted until promoted. It SHOULD declare:

- generator identity;
- baseline semantic identity;
- intended objective properties;
- protected properties;
- permitted semantic region;
- required capabilities;
- proposed transformation;
- predicted invalidation set;
- verification plan;
- resource and recursion budgets.

The proposal MUST NOT modify the trusted baseline merely by being generated.

## 5. Candidate isolation

Automated transformations MUST be applied to an isolated candidate state unless an assurance profile explicitly permits direct modification. The candidate state SHOULD have a distinct semantic identity and evidence namespace.

## 6. Verification and comparison

Before promotion, the toolchain SHOULD produce a comparison that includes:

- changed semantic identities;
- contract changes;
- added or removed effects;
- added, removed, or broadened capabilities;
- new or changed assumptions;
- invalidated evidence;
- newly produced evidence;
- protected-property regressions;
- complexity and resource deltas;
- unresolved consequences.

Passing the originally failing test is not sufficient evidence of a valid repair.

## 7. Independent evidence

A generator MUST NOT elevate the assurance of its own proposal merely by asserting success. Assurance profiles SHOULD define acceptable independence boundaries for promotion, which may include a different verifier implementation, environment, authority, machine, or external witness.

Repeated self-confirmation MUST NOT be interpreted as stronger evidence solely because recursion occurred multiple times.

## 8. Bounded recursion

Every automated refinement operation MUST be subject to explicit or inherited limits. Limits SHOULD include:

- maximum iterations or recursion depth;
- wall-clock or instruction budget;
- memory budget;
- candidate count;
- verifier-call budget;
- mutation scope;
- capability budget;
- stopping and escalation conditions.

Unbounded “continue until improved” behavior is non-conforming.

## 9. Promotion

Promotion MUST be explicit and policy-controlled. A promotion record SHOULD include:

- baseline and candidate identities;
- policy identity;
- evidence consumed;
- human or automated authority approving promotion;
- accepted tradeoffs;
- timestamp or sequence identity where relevant;
- rollback reference.

Higher-assurance profiles SHOULD preserve promotion history in append-only, signed, or externally witnessed form.

## 10. Self-hosting

An MNCS compiler or verifier MAY be implemented in the language and MAY participate in recursive refinement of its own implementation. Such self-hosting MUST preserve explicit bootstrap assumptions and MUST NOT be described as proving the toolchain correct by itself.

## 11. Interoperability

The semantic artifacts defined here SHOULD be usable by MNCS Forge, RAVEL, and independent tools without requiring one privileged orchestrator implementation.

## 12. Current executable subset

The current model serializes the artifacts above with stable semantic identities and deterministic
ordering. A causal slice contains the connected semantic-graph neighborhood currently defensible;
it is marked incomplete because control-flow, data-flow, verifier, and artifact causality are not
yet represented. A repair proposal can apply only one explicitly permitted contract-expression
change to an isolated clone, subject to finite budgets and explicit mutation authority. The result
includes a semantic delta and does not promote the candidate. Promotion is represented separately
and requires an explicit promotion authority. Invalid manifests retain dependency-only diagnostic
slices rather than being presented as complete localization.
