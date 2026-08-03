# RFC 0001: Semantic Foundation

- **Status:** Implemented experimentally
- **Target:** 0.1

## Summary

Define the project around an explicit semantic graph rather than beginning with source syntax. Establish modules, functions, typed values, contract properties, effects, capabilities, assumptions, evidence claims, and failure modes as the initial entities.

## Motivation

A syntax-first language can easily become an attractive notation without solving the reconstruction problem MNCS is intended to address. The project needs an executable answer to a more fundamental question: which relationships must be part of program meaning so that verifiers and agents do not repeatedly infer them from conventional code?

## Proposed semantics

A program is a versioned module containing functions and named assumptions. A function declares its interface, behavior, authority, observable interactions, failure mode, and evidence relationships.

The semantic model is independent of textual spelling. JSON is used only as a bootstrap serialization.

## Validation obligations

The initial validator checks:

- supported schema version;
- non-empty and unique module-local identities;
- unique function-local contract identities;
- unique typed value names;
- effect authorization through declared capabilities;
- valid assumption references;
- evidence bound to known contract properties;
- explicit verifier identity.

## Security consequences

Making authority and effects explicit creates a place to reject hidden entry points and capability escalation. Structural validation does not establish that contract expressions are true or that declared effect sets are complete; those require later semantic analysis and verifiers.

## Alternatives

### Begin with a Rust-like parser

Rejected for the first milestone because it would force grammar and desugaring decisions before the required semantic entities are established.

### Emit LLVM IR directly

Rejected as the primary authoring model because LLVM IR lacks source-level ownership, capabilities, assumptions, and evidence semantics and contains optimization promises that are dangerous to author casually.

## Unresolved questions

- stable identity and canonical hashing;
- import and package semantics;
- expression language;
- state and temporal references;
- assurance profiles;
- effect polymorphism;
- ownership and concurrency.
