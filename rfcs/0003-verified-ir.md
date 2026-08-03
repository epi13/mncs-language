# RFC 0003: Verified Intermediate Representation

- **Status:** Draft
- **Target:** 0.3–0.5

## Summary

Introduce two MNCS-owned intermediate layers between source semantics and a conventional backend: a high-level semantic IR and a lower verified SSA IR.

## Motivation

Generating LLVM IR directly exposes explicit control and dataflow, but discards or obscures many relationships the language is intended to preserve. It also requires authors or agents to make subtle claims about aliasing, overflow, provenance, alignment, and undefined behavior.

An MNCS-owned IR can retain contracts, authority, assumptions, and evidence while lowering conservative facts into LLVM or another backend.

## High-level IR

The high-level representation should make explicit:

- typed state regions;
- ownership and capability flow;
- state transitions;
- effect closure;
- normal and abnormal exits;
- contracts and proof obligations;
- source and semantic identity.

## Verified SSA

The lower representation should make explicit:

- blocks and branches;
- SSA dataflow;
- memory operations and region identities;
- atomic ordering;
- overflow semantics;
- call effects;
- obligations attached to transformations.

## Backend policy

Potentially dangerous backend attributes should be derived from established facts. Absence of evidence produces conservative code, not an optimistic annotation.

## Micro-verifier interface

A verifier request should identify:

- semantic subject;
- property or obligation;
- required inputs;
- accepted assumptions;
- output status and artifact;
- invalidation dependencies.

## Alternatives

### Direct LLVM IR generation

Useful as a backend experiment, but unsuitable as the sole semantic representation.

### Reuse Rust MIR

Potentially useful for interoperability research, but ties the language's semantic identity and evolution to another compiler's internal representation.

### Emit C

Portable and easy to bootstrap, but weak as an evidence-preserving semantic boundary and susceptible to implementation-defined behavior.

## Unresolved questions

- memory model;
- region and provenance semantics;
- verifier protocol;
- canonical serialization;
- transformation proof format;
- debug source mapping;
- interaction with unsafe and foreign code.
