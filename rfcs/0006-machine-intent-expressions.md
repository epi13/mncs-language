# RFC 0006: Machine-Intent Expressions and Verified Lowering Envelopes

- **Status:** Draft
- **Authors:** MNCS Language Project
- **Target:** semantic core, high-level IR, verified SSA, and backend research tracks

## Summary

MNCS Language should represent machine-oriented behavior as explicit semantic operations rather
than relying on accidental source patterns, implementation-defined behavior, undefined behavior,
unchecked compiler promises, or broad optimization switches.

A **machine-intent expression** states:

1. the operation's exact value and failure semantics;
2. the machine-oriented intent motivating the operation;
3. the invariants and capabilities required to use it;
4. the compiler transformations permitted by its lowering envelope;
5. the target and portability envelope in which a selected lowering is valid; and
6. the evidence required to justify the selected lowering and backend promises.

The central rule is:

> Machine intent may expand the compiler's verified choices, but it may not weaken declared
> semantics or convert an unproven assumption into a fact.

This RFC proposes expression families, obligation generation, evidence-bearing capabilities,
reference semantics, target-specific realizations, intent-preserving IR, and a graduation path from
experimental deviations to standardized language primitives. It does not select final surface
syntax or implement a backend.

## Motivation

Systems programmers and optimizing compilers regularly communicate through indirect conventions:

- a particular loop shape is written in the hope that it vectorizes;
- signed overflow is assumed to wrap because the target CPU uses two's complement arithmetic;
- pointer casts or `restrict`-like annotations are used to promise non-aliasing;
- arithmetic tricks are written to induce branchless or constant-time code;
- computed jumps are encoded through raw addresses or implementation extensions;
- broad fast-math modes permit several unrelated floating-point relaxations at once;
- data is manually reorganized to influence cache and SIMD behavior;
- target intrinsics embed one realization into application semantics;
- comments claim that an unsafe region is bounded without attaching checkable obligations; and
- source-level intent is discarded before later verifiers inspect optimized IR or machine code.

These techniques can be useful, but their meaning is often split across source syntax, language
standards, compiler flags, target behavior, programmer assumptions, and undocumented folklore. The
result may be fast while remaining difficult to verify, or may appear correct while relying on
behavior the optimizer is permitted to invalidate.

MNCS Language is intended to make contracts, authority, assumptions, evidence, and recursive
refinement part of program meaning. Machine-oriented intent should receive the same treatment.
Rather than asking a verifier to reverse-engineer why unusual code exists, the language should
expose the intended operation and generate bounded proof obligations before lowering.

## Design questions

This RFC answers the repository's design-review questions as follows.

### What new fact becomes expressible?

A program can state not only the result it requires, but also a bounded set of implementation
freedoms and machine-level properties that are semantically relevant, such as wrapping arithmetic,
constant-time selection, closed computed dispatch, verified alignment, explicit reduction error,
or target-specialized layout.

### Can the fact be checked independently?

Each expression lowers to a stable semantic operation with named obligations. Compiler analyses,
micro-verifiers, runtime checks, differential tests, symbolic tools, and machine-code inspection may
discharge those obligations independently.

### What assumptions remain outside the proof boundary?

Examples include the correctness of a target timing model, verifier soundness, compiler backend
correctness, hardware conformance, environmental stability, and any provider-declared dependency
envelope. These assumptions must remain named and evidence-bound.

### What evidence is produced?

Evidence may include obligation results, capability construction records, reference-versus-candidate
comparisons, IR transformation records, target feature identities, compiler and backend identities,
machine-code inspection, resource measurements, and portability results.

### What changes invalidate that evidence?

Material changes include semantic expression identity, operands, contracts, capabilities,
assumptions, reference implementation, lowering rule, target profile, compiler/backend version,
optimization pipeline, verifier identity, dependency slice, environment, or emitted artifact.

### Can a human understand the result?

The semantic operation, required invariants, accepted lowering choices, target envelope, and evidence
summary must remain inspectable without reconstructing compiler internals. Detailed proof artifacts
may remain provider-owned but must be addressable.

### Does the proposal constrain ordinary programs unnecessarily?

Ordinary operations remain available with conservative semantics. Machine-intent expressions are
used only when a program needs edge behavior, target specialization, security properties, or
optimization freedom beyond the ordinary expression.

## Terminology

### Semantic operation

A stable language-level operation with exact value, state, effect, failure, and edge-case behavior.

### Machine intent

The reason an implementation may prefer a particular machine property, such as vectorization,
branch elimination, cache locality, constant-time execution, or bounded allocation.

Machine intent is not itself proof that the property was achieved.

### Obligation

A named proposition that must be discharged, checked at runtime, or remain explicitly `UNKNOWN`
before a particular lowering or backend promise may be accepted.

### Evidence-bearing capability

A typed value or semantic graph edge demonstrating that an obligation was discharged for a bounded
subject and context. It is not a free-form `trust_me` assertion.

### Lowering envelope

The set of transformations and target realizations permitted for an expression while preserving its
semantics and declared properties.

### Portability envelope

The languages, targets, features, data models, endianness, compiler/backend versions, and runtime
conditions for which a selected realization and its evidence apply.

### Reference semantics

A clear implementation-independent meaning, executable reference, mathematical relation, or
contract against which optimized realizations can be checked.

### Realization

A concrete lowering or implementation candidate for a semantic operation.

### Preference, fact, and requirement

- A **preference** may guide selection but may be ignored without changing semantics.
- A **fact** may be consumed only when established by language rules, evidence, or an explicit
  runtime check.
- A **requirement** must be preserved by every accepted realization.

These categories must never be conflated.

## Proposed semantic model

A machine-intent expression is modeled conceptually as:

```text
MachineIntentExpression {
    operation: SemanticOperation,
    operands: [ValueRef],
    result_type: TypeRef,
    value_semantics: PropertySet,
    failure_semantics: FailureSet,
    effects: EffectSet,
    required_capabilities: [CapabilityRef],
    intent: IntentSet,
    preferences: PreferenceSet,
    facts: [EstablishedFactRef],
    requirements: PropertySet,
    lowering_envelope: LoweringEnvelope,
    portability_envelope: PortabilityEnvelope,
    reference_semantics: ReferenceRef?,
    obligations: [ObligationRef],
    evidence_policy: EvidencePolicy,
}
```

Surface syntax may use concise built-ins, structured blocks, library-defined semantic operations,
or generated canonical form. Equivalent source forms must lower to the same semantic identity.

An expression MUST NOT obtain stronger semantics merely because its source resembles a recognized
optimization pattern.

## Expression families

The following families establish the initial design space. Names are illustrative rather than final
grammar.

### Explicit integer arithmetic

Integer edge behavior must be selected rather than inherited accidentally.

```text
add.wrap<i32>(a, b)
add.checked<i32>(a, b)
add.saturate<i32>(a, b)
add.trap<i32>(a, b)
mul.widen<i32, i64>(a, b)
convert.exact<i64, i32>(value)
convert.saturate<i64, i32>(value)
shift.checked<u32>(value, amount)
shift.masked<u32>(value, amount)
rotate.left<u32>(value, amount)
```

Required properties:

- `wrap` has modulo-`2^N` semantics;
- `checked` returns or propagates explicit overflow state;
- `saturate` clamps to the destination limits;
- `trap` follows a declared failure path;
- `widen` computes in a type able to represent the full declared operand domain;
- conversions define rounding, truncation, sign, and out-of-range behavior; and
- lowering must not add backend no-overflow promises unless overflow impossibility is separately
  established.

Signed overflow must never be obtained by relying on undefined behavior in another language or IR.

### Floating-point policy

Floating-point relaxation should be local and decomposed rather than controlled only by a broad
fast-math switch.

```text
reduce.sum<f32>(
    values,
    order = pairwise,
    allow_fma = true,
    allow_reassociation = true,
    nan = preserve,
    signed_zero = preserve,
    max_error = 4_ulp,
)
```

A strict alternative may require exact IEEE-observable behavior for the supported environment.

The expression must identify independently:

- rounding mode assumptions;
- NaN and infinity behavior;
- signed-zero behavior;
- reassociation permission;
- contraction permission;
- reduction order constraints;
- error metric and bound; and
- target-specific floating-point assumptions.

A backend may consume only the individual relaxations established by the expression and evidence.
It must not enable unrelated relaxations.

### Verified memory access

Memory operations should distinguish bounds, alignment, aliasing, provenance, lifetime, volatility,
and atomicity.

```text
let aligned = verify.aligned<64>(buffer)?
let separate = verify.disjoint(input.region, output.region)?

load.aligned<u32, 64>(aligned, index, bounds)
load.unaligned<u32>(buffer, index, bounds)
load.volatile<u32>(device_region, offset, device_capability)
```

Evidence-bearing capabilities may include:

```text
Aligned<Subject, 64>
InBounds<Allocation, Range>
Disjoint<RegionA, RegionB>
Live<Allocation, Epoch>
Provenance<Pointer, Allocation>
Exclusive<Region, Scope>
```

A capability must bind the exact subject, scope, and assumptions under which it was established.
Changing an allocation, slice, lifetime epoch, or dependency invalidates the capability.

### Closed computed dispatch

Indirect control flow may be represented as a bounded semantic operation:

```text
dispatch.closed(
    selector = opcode,
    targets = instruction_handlers,
    default = invalid_instruction,
)
```

Required obligations include:

- the target set is closed and identity-bound;
- the selector is checked against the declared domain;
- target entries are immutable for the operation's scope;
- every target has a compatible signature and calling convention;
- untrusted input cannot directly construct an address; and
- control cannot enter an undeclared point within a target.

Permitted realizations may include a jump table, decision tree, conditional sequence, predicated
form, or target-specific indirect branch. The source expression does not require one assembly shape.

### Branchless and constant-time selection

Optimization intent and security requirements must remain distinct.

```text
select.branchless(condition, when_true, when_false)

select.constant_time(
    condition,
    when_true,
    when_false,
    secret = condition,
    model = target_timing_model,
)
```

`branchless` is a lowering preference unless elevated to a requirement. `constant_time` is a
security requirement evaluated under a declared target model.

Constant-time obligations may include absence of:

- secret-dependent control flow;
- secret-dependent memory addresses;
- secret-dependent variable-latency operations; and
- target-specific microarchitectural behavior excluded by policy.

Source-level branchlessness is not sufficient evidence. The accepted artifact must be checked at an
appropriate IR or machine-code level.

### Logical type and physical layout

Logical values should be separable from target-selected storage layout.

```text
storage particles {
    logical = Particle,
    layout = aosoa<8>,
    alignment = 64,
    constraints = [stable_serialization, bounded_padding],
}
```

A future search expression may define candidate layouts and an objective:

```text
layout.choose {
    logical_type = Particle,
    objective = throughput,
    candidates = [aos, soa, aosoa<8>, aosoa<16>],
    constraints = [stable_serialization, bounded_padding, alignment_safe],
}
```

Layout selection must preserve logical semantics, declared serialization behavior, ownership,
aliasing, and bounds. Benchmark evidence may guide selection but cannot replace semantic evidence.

### Target-specialized operations with fallback

A semantic operation may provide target preferences and a portable fallback:

```text
vector.dot<i16, i64>(
    left,
    right,
    preferred = [avx512, avx2, sve, neon],
    fallback = scalar_reference,
    overflow = widen<i64>,
)
```

Each realization must share the same semantic operation identity. Runtime target dispatch must be
explicit, deterministic under the declared environment, and included in the artifact identity.

Unsupported targets must use the declared fallback, reject compilation, or produce an explicit
unsupported/`UNKNOWN` result according to policy. They must not silently change semantics.

### Concurrency and synchronization

Atomic ordering is already explicit in several languages, but MNCS should connect operations to the
higher-level relation they intend to establish.

```text
let publication = publish.release(object)
let received = consume.acquire(publication)
```

A lock-free structure may declare:

```text
queue.enqueue(value) {
    progress = lock_free,
    reclamation = epoch_based,
    linearization = compare_exchange_success,
}
```

These are obligations, not descriptive comments. The verifier network may prove only a subset,
leaving unsupported properties `UNKNOWN`.

### Security-bound values and authority

Types and capabilities may represent trust and authority:

```text
Untrusted<Bytes>
Secret<Key>
Authenticated<User>
Authorized<User, Resource, Operation>
ConfinedPath<Root>
```

A destructive operation may require an authorization capability:

```text
let authorization = authorize(user, resource, Delete)?
delete(resource, authorization)
```

A filesystem operation may require a confined path rather than a raw string:

```text
let path = path.confine<UploadRoot>(raw_path)?
filesystem.open(path, upload_read_capability)
```

The language should make common security invariants difficult to bypass structurally, while keeping
unsafe or foreign boundaries explicit.

### Effect-bounded regions

A function or optimization region may declare permitted and forbidden effects:

```text
fn process(packet: Packet)
    effects {
        read(network_buffer)
        write(output_queue)
        allocate(max = 4096)
        forbid(filesystem)
        forbid(network_send)
        forbid(privilege_transition)
    }
```

Effect closure is both a security property and a compiler fact. The compiler may optimize based on
absence of effects only after closure is established.

### Reference semantics and optimized realizations

An operation may bind a clear reference meaning to one or more candidates:

```text
operation classify_packet(packet) {
    reference = classify_packet_reference(packet)

    realization vector_branchless {
        target = x86_64 + avx2
    }

    requires {
        output == reference.output
        no_out_of_bounds_access
        constant_time(secret_fields)
    }
}
```

Reference semantics may be executable, relational, axiomatic, or contractual. A reference is not
assumed correct merely because it is simple; its own trust and evidence boundary remains explicit.

### Optimization regions

A bounded region may expose controlled implementation freedom:

```text
optimize throughput {
    reference = parse_reference
    mutation_scope = parse_candidate_region

    require {
        output == reference.output
        memory <= 64_KiB
        allocations == 0
        constant_time(secret_fields)
    }

    prefer {
        target = x86_64 + avx2
        objective = packets_per_second
    }
}
```

An optimization region must identify:

- semantic reference;
- candidate mutation scope;
- required preserved properties;
- authority and effect ceiling;
- target and resource envelope;
- permitted candidate count and search budget;
- required verifier set;
- selection metric and tie-breaking policy; and
- promotion authority.

Generation creates isolated candidates. It does not mutate the trusted baseline or certify its own
result.

## Facts, preferences, and requirements

The semantic graph must keep these categories separate.

```text
fact length_multiple_of_16: established_by(bound_check_42)
prefer vector_width = 16
require output == scalar_reference.output
```

A preference may influence cost modeling and candidate ordering. It must not be emitted as a backend
semantic promise.

A fact must reference a construction rule, static proof, runtime check, or evidence object. If the
fact becomes stale, any lowering that consumed it becomes stale.

A requirement is part of the semantic contract. Every accepted lowering must preserve it.

## Obligation generation

Each machine-intent operation defines an obligation template. Instantiation produces stable
obligation identities bound to the expression and operands.

For example, `dispatch.closed` may generate:

```text
O1 selector_within_domain
O2 target_table_immutable
O3 target_signatures_compatible
O4 target_set_closed
O5 target_address_not_untrusted
O6 control_entry_declared
```

An obligation result has `PASS`, `FAIL`, or `UNKNOWN` status under its declared method and scope.

- `PASS` permits only the lowering choices whose requirements it satisfies.
- `FAIL` rejects those choices and may reject the expression if no valid fallback remains.
- `UNKNOWN` prevents use of dependent promises but may permit a conservative fallback.

A compiler must never reinterpret `UNKNOWN` as `PASS` because a target ordinarily behaves a
particular way.

## Evidence-bearing capabilities

Capabilities created by proof or checks should be linear, affine, scoped, copyable, or persistent
according to the property they represent.

Examples:

- a runtime bounds check may produce a capability valid for one slice identity;
- an exclusive borrow may be affine and valid only for one lexical or dynamic scope;
- a static type-layout fact may be persistent for one compiler/type identity;
- an authorization decision may be single-use when the operation consumes authority; and
- an epoch-liveness token must not escape the reclamation epoch.

The type and semantic systems should prevent capabilities from being applied to unrelated subjects.

## Lowering selection

The compiler should perform lowering in this order:

```text
source expression
    ↓
canonical semantic operation
    ↓
obligation instantiation
    ↓
static discharge / runtime-check insertion / verifier requests
    ↓
valid lowering candidates
    ↓
policy and preference selection
    ↓
high-level MNCS IR
    ↓
verified SSA
    ↓
backend IR and machine code
    ↓
artifact plus evidence manifest
```

Selection policy may consider performance, code size, energy, determinism, portability, or other
metrics, but only among candidates that satisfy required semantics.

## Intent-preserving IR

High-level MNCS IR must preserve machine intent and obligations as semantic nodes. Illustrative
operations include:

```text
mncs.add.wrap.i32
mncs.reduce.sum.f32 policy_ref
mncs.load.aligned.u32 alignment_cap bounds_cap
mncs.dispatch.closed target_set_ref
mncs.select.constant_time timing_model_ref
mncs.layout.realize logical_type_ref layout_ref
mncs.authorization.consume authority_cap
```

Verified SSA may lower these into ordinary blocks and instructions only while retaining traceability
from emitted operations to:

- source/semantic expression identity;
- consumed facts and capabilities;
- transformation rule;
- discharged obligations;
- target profile;
- generated runtime checks; and
- evidence records.

Metadata that can be silently discarded is insufficient when later verification depends on it. The
IR design must distinguish semantically required intent from optional diagnostic annotations.

## Backend promises

Backend flags and attributes are semantic promises. Examples include LLVM-style `nsw`, `nuw`,
`inbounds`, `noalias`, non-null, dereferenceable, alignment, fast-math flags, and atomic-order
reductions.

The compiler may emit such a promise only when:

1. the source or semantic operation permits it;
2. every required obligation is discharged for the exact subject;
3. the consumed evidence is current;
4. the target/backend interpretation is known; and
5. the transformation record identifies the basis for the promise.

Removing a promise for conservatism is permitted unless the corresponding machine property is a
semantic requirement. Adding an unsupported promise is a compiler error.

## Portability and target envelopes

A target-specific realization must bind:

- architecture and feature set;
- data layout and pointer width;
- endianness when relevant;
- calling convention and ABI;
- language/IR semantic version;
- compiler and backend identity;
- optimization pipeline identity;
- runtime dispatch policy;
- target model assumptions; and
- fallback or unsupported behavior.

Evidence from one envelope must not be generalized to another without a declared equivalence or new
verification.

## Evidence and freshness

A lowering evidence record should eventually identify:

```text
expression_identity
semantic_operation_identity
reference_identity
realization_identity
obligation_identities
obligation_results
capability_identities
source_semantic_ir_ssa_artifact identities
target_profile
compiler_backend_pass identities
verifier identities and versions
assumptions and limitations
dependency envelope
resource measurements
status and freshness
```

Any material identity change invalidates dependent evidence unless a complete dependency argument
establishes independence.

## Diagnostics and micro-debugging

A failed machine-intent obligation should produce a bounded diagnostic object rather than only a
backend error string.

Example:

```text
expression: dispatch.closed at semantic node expr:42
failed obligation: target_address_not_untrusted
source: request.opcode_handler
path: request -> decoded_handler -> table_write -> dispatch target
required repair boundary: handler-table construction
safe fallback: decision_tree over immutable declared targets
```

Forge micro-verifiers may answer narrow questions such as:

```text
prove these regions disjoint for this loop
check constant-time lowering for this target artifact
compare vector realization with scalar reference on the bounded domain
verify this computed-dispatch target set is closed
identify which backend promises depend on capability C
```

Forge coordinates evidence; the language owns the semantic operations and obligation vocabulary.

## Recursive discovery and graduation

A non-orthodox implementation technique may progress through these states:

```text
intentional deviation
    ↓ repeated bounded verification
experimental semantic operation
    ↓ multiple independent realizations and failure studies
provisional library primitive
    ↓ RFC and compatibility review
standardized language expression
```

Graduation must preserve the complete pattern:

```text
purpose
+ exact semantics
+ required invariants
+ capabilities
+ target envelope
+ lowering alternatives
+ failure modes
+ evidence and invalidation rules
```

The language must not learn only the unusual syntax or emitted instruction sequence.

## Unsafe, foreign, and legacy boundaries

Machine-intent expressions do not eliminate unsafe code, FFI, inline assembly, or legacy backends.
They provide a structure for bounding them.

An unsafe or foreign realization must declare:

- imported semantics or unresolved behavior;
- memory, authority, and effect boundary;
- target and ABI assumptions;
- obligations verified internally;
- obligations delegated to the caller or environment;
- artifact identity; and
- conservative fallback or failure behavior where possible.

Undefined behavior must not be treated as an optimization primitive. If a useful operation depends
on behavior another language leaves undefined, MNCS must define the behavior explicitly or reject
the operation.

## Validation and verification obligations

An implementation of this RFC must eventually test:

### Semantic validation

- expression families have explicit edge and failure semantics;
- facts reference valid current evidence or construction rules;
- capabilities match the exact required subject and scope;
- preferences cannot satisfy requirements;
- required obligations cannot be omitted;
- fallback behavior is explicit; and
- portability envelopes are complete enough for the selected realization.

### Lowering validation

- accepted lowerings preserve semantic operation identity;
- unsupported obligations prevent dependent backend promises;
- conservative fallback remains available where declared;
- intent survives through high-level IR and verified SSA;
- generated runtime checks are linked to the facts they establish; and
- stale evidence invalidates dependent lowerings.

### Positive and negative examples

The corpus should include:

- wrapping, checked, saturating, trapping, and widening arithmetic;
- rejection of accidental signed-overflow reliance;
- strict and relaxed floating-point reductions;
- valid and invalid alignment/alias capabilities;
- closed and attacker-influenced dispatch tables;
- branchless preference versus verified constant-time requirement;
- portable fallback and missing-target rejection;
- valid and stale authorization capabilities;
- layout candidates that preserve or violate serialization;
- backend promise emission and deliberate omission; and
- an optimization region whose faster candidate fails a protected property.

### Differential and artifact-level checks

At least one pilot should compare:

- a reference realization;
- a conventional source implementation;
- one machine-intent realization;
- high-level IR and SSA traces;
- emitted backend attributes; and
- machine-code or runtime evidence.

## Trust and security consequences

### Benefits

- edge behavior becomes explicit;
- security properties can be represented as requirements rather than source idioms;
- compiler promises become traceable to evidence;
- unusual code can be preserved without broad scanner suppression;
- target specialization does not erase portable reference semantics;
- micro-verifiers receive bounded, meaningful questions; and
- recursive optimization can search within explicit semantic and authority limits.

### Risks

- a rich expression vocabulary may create false confidence if verifiers are unsound;
- target timing and microarchitectural models may be incomplete;
- evidence-bearing capabilities may be forged by compiler or model bugs;
- incorrect dependency envelopes may retain stale evidence;
- reference implementations may contain shared defects;
- overly broad lowering envelopes may hide semantic changes;
- machine-specific expressions may fragment portability; and
- complex evidence graphs may become difficult to audit.

### Required mitigations

- preserve `UNKNOWN` for unsupported or incomplete verification;
- separate language semantics from performance evidence;
- bind every fact and capability to exact identities;
- require negative and adversarial examples;
- retain conservative lowering paths;
- expose assumptions and limitations;
- permit independent verifier implementations; and
- prevent generators, compilers, or local Forge runs from promoting their own results without
  explicit policy.

## Alternatives considered

### Continue using ordinary source idioms

Rejected as the primary design because intent remains implicit and often disappears during lowering.
Idioms may remain input patterns for compatibility frontends, but they should lower to explicit
semantic operations or conservative ordinary operations.

### Use compiler annotations and pragmas only

Insufficient because annotations often mix facts, preferences, and requirements, may be unchecked,
and may not survive as semantic identities.

### Expose backend intrinsics directly

Useful for narrow interop, but it couples program semantics to one backend and target. Intrinsics
should normally implement a realization behind a stable semantic operation.

### Require formal proof for every optimized operation

Impractical for the research language's full scope. The evidence model supports proof gradients:
static proof, bounded model checking, testing, differential evidence, runtime checks, analyzer
results, and `UNKNOWN`. Policies decide which level is sufficient for each property.

### Let the compiler infer all machine intent

Inference remains useful, but inferred opportunities must not alter edge semantics or security
requirements. The compiler may propose a machine-intent candidate, yet acceptance requires explicit
semantic equivalence and obligations.

### Make every machine property part of ordinary syntax

Rejected because ordinary code would become noisy and unnecessarily target-aware. Machine-intent
operations should be explicit only where the distinction matters.

## Compatibility and migration

This RFC adds a semantic research track. It does not change the current JSON semantic model or make
illustrative syntax executable.

Future migrations should:

- version machine-intent operation identities;
- keep old operation semantics immutable;
- require explicit migration when edge behavior changes;
- preserve reference and evidence identities across compatible syntax changes;
- invalidate backend evidence when target or lowering meaning changes; and
- support conservative import of conventional code with unresolved properties marked `UNKNOWN`.

## Unresolved questions

- Which expression families belong in the language core versus libraries?
- How should evidence-bearing capabilities interact with ownership and borrowing?
- Which properties may be discharged during compilation versus delegated to Forge providers?
- How are runtime checks represented and optimized without losing their evidence relation?
- How should target timing models be versioned and challenged?
- Can user-defined semantic operations define obligation templates safely?
- Which IR metadata must be semantic instructions rather than side tables?
- How should multi-versioned target dispatch contribute to reproducible artifact identity?
- What proof gradient is required for constant-time, lock-free, and memory-safety claims?
- How are performance objectives normalized across heterogeneous machines?
- When may an inferred optimization be promoted into explicit source or canonical semantics?
- How should a standardized primitive be deprecated when later evidence reveals an unsound model?

## Implementation phases

### Phase 1: semantic vocabulary

- add model objects for operation, intent, preference, fact, requirement, obligation, lowering
  envelope, portability envelope, reference, and realization;
- validate identity and dependency relationships;
- add accepted and rejected JSON fixtures; and
- expose deterministic diagnostics.

### Phase 2: arithmetic pilot

- implement explicit integer arithmetic semantics;
- lower to a small high-level IR;
- test edge behavior exhaustively for small widths; and
- verify backend no-overflow flags are emitted only when justified.

### Phase 3: memory and dispatch pilot

- implement evidence-bearing alignment, bounds, and disjointness capabilities;
- add closed dispatch and conservative fallback; and
- integrate bounded Forge verifier requests.

### Phase 4: reference and target realizations

- bind scalar references to SIMD candidates;
- record target and compiler identities;
- compare emitted artifacts; and
- demonstrate invalidation after target or pass-pipeline changes.

### Phase 5: optimization regions

- represent isolated candidates and search budgets;
- preserve protected properties and authority ceilings;
- compare semantic and performance evidence; and
- require explicit policy-controlled promotion.

## Acceptance criteria

This RFC may advance from **Draft** to **Proposed** when:

- the semantic vocabulary has an executable model;
- at least two arithmetic operations and one memory or dispatch operation are represented;
- accepted and rejected fixtures exercise facts, preferences, requirements, and obligations;
- a canonical source/semantic example exists;
- the high-level IR preserves operation and obligation identities;
- one backend pilot demonstrates conservative promise emission;
- Forge can run at least one bounded obligation verifier;
- stale evidence prevents reuse of a previously valid lowering; and
- documentation continues to state that no production grammar or soundness claim exists.
