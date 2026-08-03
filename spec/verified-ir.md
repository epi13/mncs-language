# Verified Intermediate Representation 0.1 Direction

This document states design constraints rather than a completed IR grammar.

## 1. Goals

The MNCS IR should:

- preserve source-level contracts and evidence identities;
- make control flow, dataflow, state regions, and failures explicit;
- support small independently callable verifiers;
- permit deterministic serialization and hashing;
- support incremental invalidation;
- lower conservatively to conventional backends.

## 2. High-level IR

The high-level IR should retain:

- component and type identities;
- ownership and borrowing relationships;
- state transitions;
- effect and capability edges;
- contract expressions;
- normal, error, compensation, and fatal paths;
- assumption and evidence references.

## 3. Verified SSA

The lower IR should include:

- basic blocks;
- explicit branch conditions;
- SSA values and join semantics;
- typed memory regions;
- explicit loads, stores, and calls;
- overflow behavior;
- aliasing claims;
- atomic and concurrency ordering;
- proof obligations attached to transformations.

## 4. Optimization claims

Backend attributes are semantic promises. The compiler MUST NOT emit a stronger promise than established by language rules or evidence.

Examples:

- non-aliasing claims require established region separation;
- no-signed-overflow claims require an established bound;
- in-bounds pointer arithmetic requires an established allocation relation;
- non-null claims require an established construction or check;
- atomic ordering reductions require a verified concurrency argument.

## 5. Transformation evidence

An IR pass SHOULD be able to report:

- input semantic identity;
- output semantic identity;
- transformation rule;
- obligations required by the rule;
- evidence consumed;
- evidence produced or preserved;
- properties invalidated.

## 6. Debugging

Verifier failures should identify the smallest defensible causal path and repair boundary rather than only dumping a whole-program analyzer trace.
