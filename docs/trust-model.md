# Trust Model

MNCS Language should make trust smaller and more visible, not claim to eliminate it.

## Trust categories

### Language and semantic model

Users trust that the specification gives constructs a consistent meaning and that canonicalization does not silently change that meaning.

### Proof core and kernel

The proof-carrying direction in [RFC 0007](../rfcs/0007-proof-carrying-dependent-core.md) introduces a deliberately small formal trust boundary.

The proof kernel should check only the selected dependent core, proof terms, definitional equality, universe constraints, admitted inductive/recursive constructions, and explicit assumptions. It should be deterministic, versioned, and small enough to support independent implementations and cross-checking.

Proof generation is not automatically trusted. Elaborators, tactics, LLMs, SMT solvers, Forge workflows, optimization search, specialized models, and other agents may produce candidate proofs or certificates, but their output receives formal authority only after the relevant checker accepts it under an explicit scope and assumptions.

A proof kernel bug can invalidate formal soundness claims within its model, so kernel identity and version belong in the trust and evidence boundary.

### Frontend and lowering

Users trust—or verify—that parsing, type checking, graph construction, and lowering preserve the source program's semantics. A future proof-producing frontend or lowering pass should reduce this trust by emitting independently checkable preservation evidence; until then, the boundary remains explicit.

### Verifiers

A verifier is itself software with a version, implementation, configuration, and operating environment. Evidence must identify which verifier produced it.

A verifier result is not automatically equivalent to a kernel-checked proof. A checkable certificate, a translated proof term, a soundness-scoped verifier result, and an empirical observation remain distinguishable evidence classes.

### Backends and runtime

LLVM, Cranelift, linkers, runtimes, operating systems, firmware, and hardware introduce behavior outside the source-level proof boundary unless separately addressed.

### External systems

Storage durability, service responses, clocks, random sources, credentials, network behavior, and operator actions commonly remain assumptions.

## Required honesty

A report should prefer:

```text
Property P is verified under assumptions A and B using verifier V.
```

over:

```text
The program is verified.
```

For formal proof, the stronger form should still retain its boundary:

```text
Property P has a kernel-checked proof under assumptions A and B in proof-core version K.
```

This does not imply that every external-system or physical assumption has itself been proved.

## Assumptions are dependencies

Assumptions must be named and should eventually have stable identities. A change to an assumption, its supplier, or its confidence should invalidate dependent evidence.

Formal proof objects should enumerate or make derivable the assumptions they consume so that a proof cannot silently outlive a changed premise.

## Evidence tampering

Long-term evidence artifacts should be content-addressed and authenticated. Replacing a test, verifier, proof object, proof kernel, manifest, or artifact must not preserve the appearance of the previous result.

## Root access

No local verification system can fully protect itself from an administrator who can replace the compiler, verifier, proof kernel, evidence store, and reported outputs. Stronger assurance requires separation of duties, signed artifacts, reproducible execution, remote witnesses, hardware-backed attestation, append-only transparency logs, independent kernel implementations, or independent reruns.

The architecture should support these mechanisms without pretending that a local root-controlled environment is an absolute sandbox.
