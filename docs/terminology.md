# Terminology

## Component

A named semantic unit with an interface, implementation, obligations, authority, and evidence relationships.

## Contract

A behavioral property associated with a component. Initial kinds include `requires`, `ensures`, `invariant`, `preserves`, and `budget`.

## Effect

An observable interaction with state or an external system, such as reading a file, modifying a balance, using randomness, or connecting to a service.

## Capability

Explicit authority that permits a class of effects. Possessing a capability does not prove correct use; it establishes that the operation is authorized within the declared boundary.

## Assumption

A named proposition consumed by a claim but not established inside the current proof boundary.

## Evidence claim

A relationship stating that a verifier has claimed, tested, analyzed, verified, or externally verified a property.

## Evidence artifact

A durable output supporting an evidence claim, ideally content-addressed and bound to the relevant subject and verifier version.

## Micro-verifier

A small verifier designed to answer one bounded question against a narrow semantic or IR region.

## Trust boundary

The point at which an argument depends upon an assumption, external component, unsafe region, foreign implementation, verifier, compiler, runtime, or hardware behavior not established by the current evidence.

## Invalidation

The process of determining which claims and artifacts are no longer reliable after a change to source, semantics, dependencies, assumptions, verifier versions, or environments.

## Assurance level

A declared evidence state. Initial vocabulary includes claimed, tested, analyzed, verified, and externally verified.

## Semantic identity

An identity derived from canonical program meaning rather than source spelling alone.

## Verified SSA

A static-single-assignment representation whose transformations and optimization-relevant claims are linked to explicit obligations or evidence.
