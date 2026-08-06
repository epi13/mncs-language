# Machine-intent expressions

Machine-intent expressions are the proposed bridge between verification-native source semantics and
aggressive machine-oriented lowering.

The language should not require a programmer or agent to communicate with a compiler by arranging
ordinary syntax into fragile patterns. Instead, a program should name the edge behavior, security
property, target preference, required facts, and accepted transformation freedom directly.

The detailed proposal is [RFC 0006](../rfcs/0006-machine-intent-expressions.md). The emerging
semantic requirements are summarized in the
[machine-intent expression specification direction](../spec/machine-intent-expressions.md).

## Accidental behavior versus defined behavior

A conventional implementation may depend on assumptions such as:

```text
signed overflow will probably wrap
this arithmetic trick will probably remain branchless
this pointer cast probably communicates non-aliasing
this loop will probably vectorize
this computed jump probably remains inside the table
```

MNCS Language should instead distinguish operations explicitly:

```text
add.wrap<i32>
add.checked<i32>
select.branchless
select.constant_time
dispatch.closed
load.aligned with alignment and bounds capabilities
```

The difference is not cosmetic. Each semantic operation defines its edge cases and generates the
obligations required for a safe lowering.

## Five parts of an expression

A machine-intent expression combines:

```text
exact semantics
+ machine intent
+ required invariants and capabilities
+ permitted lowering envelope
+ evidence and target envelope
```

The compiler may choose among multiple realizations, but only after required obligations are
accounted for.

## Facts, preferences, and requirements

The model separates three concepts that ordinary compiler annotations often blur:

```text
fact length_is_multiple_of_16 established_by bounds_check_42
prefer vector_width = 16
require output == scalar_reference.output
```

A preference may guide selection but cannot authorize a backend promise. A fact must have an
identity-bound basis. A requirement is semantic and must survive every accepted lowering.

## Evidence-bearing capabilities

An optimized operation may consume typed evidence such as:

```text
Aligned<buffer,64>
InBounds<allocation,range>
Disjoint<input_region,output_region>
Authorized<principal,resource,operation>
ConfinedPath<upload_root>
```

The capability applies only to its exact subject and scope. It becomes stale when a material
allocation, region, lifetime, authority, target, compiler, or dependency identity changes.

## Reference semantics and realizations

A clear reference meaning can coexist with specialized implementations:

```text
semantic operation: widened i16 dot product
reference: portable scalar
realizations: AVX-512, AVX2, SVE, NEON, scalar
requirements: equal result, bounded access, explicit overflow
```

The optimized implementation may be difficult to maintain manually. The semantic operation,
reference relation, target envelope, and evidence remain inspectable.

## Intent-preserving IR

High-level MNCS IR should retain semantic operations such as:

```text
mncs.add.wrap.i32
mncs.dispatch.closed
mncs.select.constant_time
mncs.load.aligned.u32
```

Later lowering may replace them with ordinary blocks and instructions, but traceability to the
operation, consumed capabilities, obligations, target profile, and evidence must remain.

This prevents a verifier from receiving an unexplained indirect branch or arithmetic sequence after
the reason for it has been erased.

## Backend conservatism

Backend attributes are promises, not harmless hints. The compiler should emit non-aliasing,
no-overflow, in-bounds, alignment, floating-point relaxation, or reduced atomic-order claims only
when current evidence establishes them for the exact operation.

Missing evidence produces conservative lowering, explicit fallback, or `UNKNOWN`; it does not
produce optimistic backend metadata.

## Recursive discovery

A useful non-orthodox technique may begin as an intentional deviation, accumulate repeated bounded
evidence, become an experimental semantic operation, and eventually graduate through an RFC into a
standardized expression.

The network must retain the complete pattern:

```text
purpose
+ semantics
+ invariants
+ target limits
+ lowering choices
+ failure behavior
+ evidence and invalidation
```

It must not learn only the unusual source shape or instruction sequence.

## Relationship to Forge and MNCS

MNCS Language owns the semantic operation and obligation vocabulary. Forge can route bounded
micro-verifiers, preserve results, localize failures, compare candidates, and manage freshness.
MNCS and MNCDS retain their separate validation, evidence, and promotion authorities.

A local compiler or Forge result cannot establish independent evaluation, protected custody,
certification, or governance approval.
