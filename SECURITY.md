# Security Policy

MNCS Language is experimental research software and must not currently be relied upon to establish the security or correctness of production systems.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting feature when available. Do not publish an exploitable issue before maintainers have had a reasonable opportunity to assess it.

A useful report includes:

- affected commit or version;
- semantic rule or implementation path involved;
- minimal reproducer;
- expected and observed validation result;
- whether the issue permits an undeclared effect, capability escalation, false evidence, unsound lowering, or verifier bypass;
- known mitigations.

## Research security priorities

The project treats the following as security-sensitive:

- false capability authorization;
- undeclared or incorrectly closed effects;
- evidence attached to the wrong property or implementation;
- stale evidence accepted after an invalidating change;
- assumptions represented as verified facts;
- unsafe or foreign code escaping its declared boundary;
- backend attributes emitted without sufficient proof;
- parser or canonicalization ambiguity that changes semantic identity;
- verifier replacement or artifact tampering without detection.

## Current limitations

The prototype validates structural consistency only. It does not prove contract expressions, execute verifiers, establish cryptographic artifact integrity, or guarantee sound code generation.
