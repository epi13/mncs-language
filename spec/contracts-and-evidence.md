# Contracts and Evidence 0.1

## 1. Separation of property and support

A contract property and the evidence supporting it are distinct entities. Declaring a property does not verify it. Producing evidence does not create a property that was never declared.

## 2. Evidence binding

Every evidence claim MUST identify:

- the contract property it supports;
- the verifier that produced the claim;
- an evidence status;
- optionally, a durable artifact reference.

The referenced property MUST exist in the same function in the 0.1 model. Future versions may support evidence over module, type, dependency, or whole-program properties.

## 3. Evidence status

The initial statuses are ordered only as descriptive categories; they do not automatically form a sound mathematical lattice:

- `claimed` — asserted without stronger machine-checked support;
- `tested` — observed through one or more executions;
- `analyzed` — established by a non-proof analysis under stated assumptions;
- `verified` — established by a verifier intended to prove the property within its model;
- `externally_verified` — verified by an independent authority or execution boundary.

A verifier MUST NOT report a stronger status than its method establishes.

## 4. Missing evidence

A structurally valid program MAY contain a contract with no evidence during early research stages. Validators SHOULD report this condition. Future assurance profiles MAY reject it.

## 5. Assumptions

An assumption MUST include:

- a stable local identity;
- a proposition;
- the supplier or boundary responsible for it;
- a confidence classification.

A function may consume only assumptions declared by its program or imported through a future dependency mechanism.

## 6. Evidence artifacts

A future artifact format SHOULD bind:

- semantic subject identity;
- implementation identity;
- property identity;
- verifier identity and version;
- verifier configuration;
- assumptions consumed;
- dependency identities;
- result status;
- artifact hash;
- invalidation conditions.

## 7. Invalidation

Evidence MUST be treated as stale when a change can affect the subject, property, assumptions, verifier semantics, or dependencies consumed by the claim. The language and toolchain should minimize invalidation while remaining conservative.
