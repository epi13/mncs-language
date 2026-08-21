# Concept Reconstruction Experiments

Status: architecture proposal / non-normative

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
