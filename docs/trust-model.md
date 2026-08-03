# Trust Model

MNCS Language should make trust smaller and more visible, not claim to eliminate it.

## Trust categories

### Language and semantic model

Users trust that the specification gives constructs a consistent meaning and that canonicalization does not silently change that meaning.

### Frontend and lowering

Users trust—or verify—that parsing, type checking, graph construction, and lowering preserve the source program's semantics.

### Verifiers

A verifier is itself software with a version, implementation, configuration, and operating environment. Evidence must identify which verifier produced it.

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

## Assumptions are dependencies

Assumptions must be named and should eventually have stable identities. A change to an assumption, its supplier, or its confidence should invalidate dependent evidence.

## Evidence tampering

Long-term evidence artifacts should be content-addressed and authenticated. Replacing a test, verifier, manifest, or artifact must not preserve the appearance of the previous result.

## Root access

No local verification system can fully protect itself from an administrator who can replace the compiler, verifier, kernel, evidence store, and reported outputs. Stronger assurance requires separation of duties, signed artifacts, reproducible execution, remote witnesses, hardware-backed attestation, append-only transparency logs, or independent reruns.

The architecture should support these mechanisms without pretending that a local root-controlled environment is an absolute sandbox.
