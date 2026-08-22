# Concept Reconstruction Experiments

Status: CRE-1 and CRE-2 local bootstrap paths implemented / non-normative authority

## Implemented Family Record compiler reference

`CompilationStudyResult::family_reference` exposes
`mncs-language.family-compiler-reference.v0.1`. It binds compiler/pipeline identities,
semantic/HIR/SSA/selected-SSA stage fingerprints, request/run identity, target, backend,
realization plan, backend artifact, unresolved obligations, and compilation status. The language
profile remains part of the compilation request and target/backend realization remains separate.
`mncs compiler-study ... --family-reference` emits the native reference for Control/Commons
attachment; it is backend-neutral and makes no experiment-success claim.

## Purpose

MNCS Language should be forced to prove that it can express the concepts from which the MNCS family itself is constructed. Concept Reconstruction Experiments (CREs) provide that pressure without merely transpiling current Python/Rust implementations.

A CRE selects one fundamental computing concept or invariant required by the family, freezes a bounded study contract, asks multiple independent experimenters to realize it using the current language profile, and then evaluates the results across compiler stages and targets.

## Language-owned facts

MNCS Language remains the authority for:

- source-profile legality;
- semantic elaboration;
- semantic graph identities;
- HIR/SSA/selected-SSA identities;
- lowering and backend contracts;
- translation-validation records;
- explicit unsupported/UNKNOWN semantics;
- compiler diagnostics and unresolved obligations.

A CRE record may reference these identities but must not redefine them.

## Reconstruction, not transpilation

The existing implementation of a family component is a requirements and comparison source, not the candidate template. Where practical, the first implementation round should hide the original body and expose only:

- governing RFC/contract references;
- valid/invalid behavior;
- invariants and falsifiers;
- resource/authority boundaries;
- test corpus and protected properties.

After independent candidates exist, the current implementation may be used as an oracle or comparison target.

## Why failure matters

CRE failure should directly inform language evolution. A failed implementation attempt may reveal:

- language expressivity gap;
- executable semantic-model gap;
- compiler/lowering gap;
- backend/portability gap;
- verifier gap;
- experiment/tooling gap;
- specification ambiguity;
- ordinary candidate implementation error.

The failed experiment should remain immutable/addressable. After a language or compiler change, rerun the same frozen study so the project can observe whether the deficiency was actually repaired.

## Record-spine integration

Compiler and semantic artifacts remain producer-native Language records. The Family Record Spine should reference them by stable identity rather than copying compiler meaning into Commons, Control or Forge.

A Concept Experiment should be able to bind at least:

```text
language_profile
compiler_identity
source_identity
semantic_graph_identity
HIR_identity
SSA_identity
selected_SSA_identity
target/backend identity
translation_validation identity
unresolved obligations
```

Fabric owns execution facts; Forge owns bounded evaluation; Commons indexes the graph; MNCDS may later use eligible studies as development evidence; MNCS owns assurance/conformance semantics.

## Bootstrap actor roles

The first studies do not require RAVEL or MNEL. Ordinary models may be assigned exact Harness roles such as builder, experiment-investigator, adaptive-experiment-critic and skeptic. Their exact model/worker/provider identity must be retained. These stand-ins are baselines, not RAVEL/MNEL records.

## Recommended progression

Early CREs should expand in difficulty as the source language grows:

1. tri-state evidence/result lattice;
2. capability/effect authorization gate;
3. retry authority under uncertain failure;
4. structured state transitions and provenance relations;
5. dependency invalidation;
6. bounded resource ownership and scheduling concepts;
7. progressively larger slices of Forge/Fabric/RAVEL-like logic only when language features support them.

This sequence should help prioritize missing language features with empirical evidence rather than roadmap intuition alone.

## First CRE: tri-state lattice

Reconstruct the MNCS `PASS`, `UNKNOWN`, `FAIL` combination law and test:

- commutativity;
- associativity;
- idempotence;
- PASS as neutral element;
- FAIL as dominating/absorbing result;
- UNKNOWN preservation;
- explicit failure for invalid discriminants.

Compile through every currently available stage, compare independent candidates, inject incorrect candidates, and run portable backends across Fabric hosts where supported.

The local source-level fixture is `examples/source/cre1-evidence-combine.mncs`. It compiles from
Profile 0.3 source through semantic/HIR/SSA into both current adapters. The frozen corpus exhausts
the three-by-three input domain and evaluates all six declared algebraic laws. The sibling
`cre1-evidence-combine-wrong.mncs` deliberately violates UNKNOWN idempotence/preservation and yields
a language-owned `FAIL` with retained case and property counterexamples. This is bounded evidence,
not a proof of the law outside the declared finite domain.

## Second CRE: authority gate

`examples/source/cre2-authorized-call.mncs` carries the callee's required capability/effect closure
at the caller and executes through both adapters. `cre2-undeclared-capability.mncs` is rejected with
`MNE111`; `cre2-authority-laundering.mncs` is rejected with `MNE134` before HIR/backend realization.
The declarations make the authority path machine-visible even though Profile 0.3 does not perform an
external effect at runtime.

## Third CRE blocker

Profile 0.3 can model retry outcomes and authority-closed calls, but it cannot represent a retry
sequence with a semantically visible attempt bound without hand-unrolling calls. The exact blocker
and rejected shortcuts are recorded in
`examples/experiments/cre3-retry-authority-blocker.json`. Bounded iteration is the next pressure
experiment; unrestricted loops and recursive retry remain intentionally unsupported.

## Long-term recursive objective

The desired loop is:

```text
MNCS concept
 -> independent MNCS Language realization
 -> compiler + Forge + Fabric evidence
 -> failure attribution
 -> language/compiler/tooling proposal
 -> rerun frozen experiment
 -> retained verified experience
```

This is more useful than self-hosting as a symbolic milestone: it continuously tests whether the language can honestly express the semantic machinery on which the wider system depends.
