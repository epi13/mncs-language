# Effects and Capabilities 0.1

## 1. Purpose

Effects describe observable interactions. Capabilities describe authority. Keeping them separate permits the system to distinguish:

- an operation that is authorized but incorrect;
- an operation that is correct in isolation but unauthorized;
- an operation omitted from the declared effect boundary.

## 2. Effect declaration

Every effect MUST identify:

- an effect kind;
- a target or region;
- the capability authorizing it.

Examples include:

```text
read filesystem:/config
write Account.balance
append AuditLog
network.connect PublishingService
clock.read MonotonicClock
randomness.consume SecureRandom
```

The 0.1 prototype treats kind and target as strings. Later specifications MUST define effect composition, region identity, parameterization, and closure.

## 3. Capability declaration

A function MUST explicitly declare every capability named by its effects. An effect whose capability is absent is invalid.

Capability declarations SHOULD be unique and narrowly scoped. A future type system should permit capabilities to be passed, borrowed, attenuated, split, or consumed without ambient global authority.

## 4. Closure

A function's effective effect set includes effects introduced by called components unless those effects are isolated behind a declared boundary. Future versions MUST define transitive effect closure and polymorphism.

## 5. Ambient authority

Filesystem, network, process, environment, clock, randomness, credential, and device authority SHOULD NOT be granted implicitly. Runtime or foreign interfaces that retain ambient authority MUST be represented as explicit trust boundaries.

## 6. Unsafe and foreign operations

Unsafe regions and foreign calls MAY exist, but they MUST declare:

- authority received;
- effects permitted;
- assumptions introduced;
- invariants promised on return;
- evidence or review requirements;
- failure containment.
