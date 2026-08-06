# Verified Intermediate Representation 0.1 Direction

This document states design constraints rather than a completed IR grammar.

## 1. Goals

The MNCS IR should:

- preserve source-level contracts and evidence identities;
- preserve machine-intent operation, lowering-envelope, and obligation identities;
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
- assumption and evidence references;
- machine-intent operations;
- facts, preferences, and requirements as distinct categories;
- generated obligation identities;
- permitted lowering and portability envelopes;
- reference and realization identities; and
- target-specific requirements and fallback behavior.

Semantically required intent must not survive only as optional debugging metadata. A later verifier
must be able to determine why a wrapping operation, indirect dispatch, branchless selection,
aligned load, target-specific layout, or backend promise exists.

Illustrative high-level operations include:

```text
mncs.add.wrap.i32
mncs.add.checked.i32
mncs.reduce.sum.f32 policy_ref
mncs.load.aligned.u32 alignment_cap bounds_cap
mncs.dispatch.closed target_set_ref
mncs.select.constant_time timing_model_ref
mncs.layout.realize logical_type_ref layout_ref
mncs.authorization.consume authority_cap
```

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
- proof obligations attached to transformations;
- links from lowered instructions to machine-intent operation identities;
- consumed evidence-bearing capabilities;
- generated runtime checks and the facts they establish;
- target-profile and realization identities; and
- conservative fallback identity where applicable.

A machine-intent operation may lower to ordinary SSA instructions only if traceability to its
semantic requirements and evidence remains available to validation and micro-debugging.

## 4. Machine-intent lowering

Lowering a machine-intent expression should follow:

```text
semantic operation
    ↓
obligation instantiation
    ↓
static discharge, runtime-check insertion, or verifier request
    ↓
valid realization set
    ↓
policy and preference selection
    ↓
high-level MNCS IR
    ↓
verified SSA
```

The IR must distinguish:

- a preference that guides realization selection;
- a fact established for an exact subject and scope;
- a requirement every accepted realization must preserve; and
- an unresolved obligation that prevents a dependent lowering promise.

An `UNKNOWN` obligation may select a declared conservative fallback. It must never be treated as
`PASS` because the compiler or target ordinarily behaves in a particular way.

## 5. Optimization claims

Backend attributes are semantic promises. The compiler MUST NOT emit a stronger promise than established by language rules or evidence.

Examples:

- non-aliasing claims require established region separation;
- no-signed-overflow claims require an established bound;
- no-unsigned-overflow claims require semantics that exclude wrapping or a separately established bound;
- in-bounds pointer arithmetic requires an established allocation relation;
- non-null claims require an established construction or check;
- alignment claims require an identity-bound alignment fact or capability;
- floating-point relaxation flags require the corresponding explicit policy permission;
- constant-time claims require evidence under a declared target model; and
- atomic ordering reductions require a verified concurrency argument.

The compiler may omit an optional promise for conservatism. It may not add a promise merely because
source syntax resembles a common optimization idiom.

## 6. Evidence-bearing capabilities

An IR value may carry or reference a capability such as:

```text
Aligned<Subject, N>
InBounds<Allocation, Range>
Disjoint<RegionA, RegionB>
Live<Allocation, Epoch>
Provenance<Pointer, Allocation>
Authorized<Principal, Resource, Operation>
```

The capability must bind the exact subject, scope, construction or verifier identity, assumptions,
and dependencies. A transformation that changes a material identity must invalidate or re-establish
the capability before consuming it.

## 7. Transformation evidence

An IR pass SHOULD be able to report:

- input semantic identity;
- output semantic identity;
- transformation rule;
- machine-intent operation and realization identities;
- obligations required by the rule;
- evidence and capabilities consumed;
- runtime checks introduced;
- evidence produced or preserved;
- backend promises emitted or removed;
- target and pass-pipeline identity; and
- properties invalidated.

A pass that selects a target-specific realization should also record the portable fallback or
explicit unsupported behavior.

## 8. Artifact traceability

The emitted artifact and evidence manifest should allow a reviewer or verifier to trace:

```text
machine instruction or backend operation
    ↑
verified SSA transformation
    ↑
high-level machine-intent operation
    ↑
canonical semantic expression
    ↑
source representation and contract
```

This trace does not prove compiler correctness by itself. It makes the selected semantic and trust
boundary inspectable and supports targeted independent verification.

## 9. Debugging

Verifier failures should identify the smallest defensible causal path and repair boundary rather than only dumping a whole-program analyzer trace.

For machine-intent expressions, diagnostics should identify:

- expression and operation identity;
- failed or unresolved obligation;
- consumed fact or capability;
- dependent realization or backend promise;
- target and environment;
- available conservative fallback; and
- evidence and artifacts invalidated by a repair.

See [Machine-Intent Expressions](machine-intent-expressions.md) and
[RFC 0006](../rfcs/0006-machine-intent-expressions.md).
