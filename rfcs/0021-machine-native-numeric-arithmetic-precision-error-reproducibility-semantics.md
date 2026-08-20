# RFC 0021: Machine-Native Numeric, Arithmetic, Precision, Error, and Reproducibility Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.2–1.0
- **Depends on:** RFC 0001, RFC 0002, RFC 0003, RFC 0004, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0013, RFC 0014, RFC 0015, RFC 0016, RFC 0017, RFC 0018, RFC 0019, RFC 0020

## Summary

MNCS Language should treat numerical meaning as a composition of **mathematical domain, operation semantics, numeric contract, representation realization, algorithmic realization, and evidence**, rather than treating a machine format such as `i32`, `f32`, or `f64` as if it fully defined the mathematical meaning of a computation.

The central rule is:

> **A mathematical quantity, an arithmetic operation, an approximation policy, and a machine representation are distinct semantic objects.**

A second rule is equally important:

> **No arithmetic law may be assumed merely because it holds over the mathematical numbers that inspired a machine representation.**

For example, mathematical real addition is associative. Ordinary finite binary floating-point addition generally is not. A backend therefore must not infer reassociation authority from the fact that an operation happens to be written with `+`.

RFC 0021 establishes a machine-native numerical model in which programs can state what numerical behavior actually matters while leaving representation, precision, accumulation strategy, vectorization, target format, and algorithmic realization open where the contract permits.

The intended result is a system capable of representing questions such as:

- Is this operation mathematically exact?
- If not exact, what relation must the computed result bear to the exact result?
- Which rounding mode applies?
- What happens on overflow, underflow, division by zero, invalid operations, or unrepresentable conversion?
- Is the result required to be correctly rounded, faithfully rounded, interval-enclosed, bounded by absolute or relative error, or only empirically accurate?
- Is the computation required to be bitwise reproducible, order-independent, schedule-independent, target-independent, or only statistically reproducible?
- May an optimizer reassociate operations?
- May it fuse multiply/add operations?
- May it flush subnormals, ignore signed zero, assume no NaNs, or replace division with reciprocal multiplication?
- Can a reduction use mixed precision?
- Can precision be chosen automatically to satisfy an error bound?
- Can a real-valued computation be realized by binary floating point, decimal, fixed point, rational arithmetic, intervals, balls, exact accumulators, posits, or another evidenced representation?
- Which numerical relation from RFC 0020 does the candidate realization establish?
- Which numerical evidence is sufficient under RFC 0018 to authorize experimentation or promotion?

This RFC does not require one universal number system. It establishes a semantic framework within which multiple exact, finite, approximate, validated, stochastic, and target-specific numerical systems can coexist without being conflated.

---

## Motivation

MNCS already requires explicit arithmetic edge behavior in RFC 0006 and uses typed values, machine-intent operations, target-dependent realizations, proof obligations, evidence, and recursive refinement throughout the architecture.

However, the current RFC set does not yet provide one constitutional answer to questions such as:

- what `Real` means independently of `f64`;
- whether `0.1` denotes a rational/decimal quantity or a pre-rounded binary value;
- when an arithmetic operation is exact;
- how rounding is selected and scoped;
- how overflow differs between modular, checked, saturating, widening, and floating semantics;
- how floating-point exceptional values enter the value domain;
- how numerical error is represented and compared;
- how reduction order affects meaning;
- how reproducibility should be classified;
- how mixed-precision realizations relate to one logical operation;
- how finite test agreement differs from a proven error bound;
- how rigorous enclosure arithmetic fits alongside ordinary approximate arithmetic; and
- how Forge may search numerical realizations without silently weakening protected numerical behavior.

Conventional languages often collapse these concerns into the choice of a primitive representation:

```text
f32
f64
i32
u64
```

That makes representation choice carry far more semantic responsibility than it should.

The result is frequently one of two extremes:

1. semantics are weak enough that implementations may change results substantially under optimization; or
2. semantics are fixed tightly to one machine representation even where the program only cares about a numerical requirement such as bounded error, determinism, or throughput.

MNCS should instead permit a program to state the **numerical contract** and allow the realization search space to remain open until a concrete representation and algorithm are actually required.

---

## Constitutional principles

### 1. Numbers are not representations

> **A mathematical quantity is not identical to the representation chosen to approximate, encode, or realize it.**

Examples:

```text
Real != Binary64
Real != Decimal64
Real != Fixed<i64, scale>
Real != Posit32
Rational != Pair<i64,i64>
```

A representation may realize a quantity under a declared relation and contract.

### 2. Approximation must be explicit

> **Any loss of mathematical information must occur under an explicit numerical contract.**

The system must not silently change exact arithmetic into approximate arithmetic merely because a backend prefers a finite representation.

### 3. Arithmetic laws are domain-specific

> **Associativity, commutativity, distributivity, monotonicity, cancellation, and related algebraic laws belong to operations over declared domains; they are not inferred from operator spelling.**

### 4. Rounding is semantic

> **Rounding mode and rounding points are part of numerical meaning whenever they may affect observable results.**

Ambient processor state must not silently redefine canonical arithmetic semantics.

### 5. Overflow is semantic

> **Trap, check, wrap, saturate, widen, infinity, arbitrary precision, and realization rejection are distinct overflow behaviors.**

### 6. Exceptional values are domain members only where declared

`NaN`, signed zero, positive infinity, negative infinity, and subnormal finite values belong to numerical domains that explicitly include them. They do not silently inhabit mathematical `Real`.

### 7. Error claims require a metric and scope

`error <= e` is incomplete unless the error relation is identified.

### 8. Reproducibility is multidimensional

Bitwise, order, schedule, backend, target, environment, and statistical reproducibility are separate claims.

### 9. Precision need not be logical identity

Where a numerical contract permits, working precision may remain an unresolved realization parameter.

### 10. Fast arithmetic relaxations must be decomposed

MNCS should not hide a collection of semantic relaxations behind one universal `fast_math` switch.

### 11. Conversions are numerical transformations

Any conversion that can round, saturate, overflow, trap, reject, or lose information must expose those semantics.

### 12. Finite evidence remains finite

A numerical corpus that exhibits acceptable error establishes bounded empirical evidence, not a universal theorem about all inputs.

### 13. Approximation provenance matters

Uncertainty or approximation terms sharing a source must not automatically be treated as independent.

### 14. A faster candidate cannot silently weaken protected numerical behavior

Performance improvement never creates authority to change a protected numerical relation.

---

## Semantic separation

RFC 0021 proposes at least six semantically distinct layers.

### Mathematical domain

```text
NumericDomain
```

A mathematical or machine-level domain over which values and operations are defined.

### Numeric operation

```text
NumericOperation
```

The mathematical or finite-domain operation to be performed.

### Numeric contract

```text
NumericContract
```

The required exactness, approximation, exceptional behavior, reproducibility, and allowed relaxations.

### Numeric representation

```text
NumericRepresentation
```

The format or encoding used to represent operands/results.

### Numeric realization

```text
NumericRealization
```

A concrete algorithm, working precision, accumulation strategy, target, and representation combination.

### Numeric evidence

```text
NumericEvidence
```

Evidence establishing the relation between the realization and the logical numerical contract.

These layers must remain distinguishable even when a surface syntax allows convenient shorthand.

---

## Candidate semantic objects

A future canonical model may include objects conceptually similar to:

```text
NumericDomain
NumericValue
NumericOperation
NumericContract
RoundingPolicy
OverflowPolicy
UnderflowPolicy
ExceptionalValuePolicy
ErrorModel
ErrorRequirement
ReproducibilityProfile
NumericRepresentation
NumericRealization
PrecisionVariable
PrecisionConstraint
AccumulationPolicy
ReductionContract
ConversionContract
TranscendentalAccuracyContract
ArithmeticLawClaim
ValidatedApproximation
IntervalEnclosure
BallEnclosure
UncertaintyDependency
ConditioningClaim
StabilityClaim
NumericRelationClaim
NumericCounterexample
NumericEvidence
```

This RFC does not freeze exact syntax or serialization names.

---

# Part I — mathematical domains

## Natural numbers

`Natural` denotes nonnegative mathematical integers where supported by the selected logical stratum.

It does not imply a fixed-width representation.

## Integers

`Integer` denotes exact mathematical integers.

Operations such as addition, subtraction, and multiplication are exact in the logical domain.

A bounded machine representation is a realization choice, not the definition of `Integer` itself.

## Rational numbers

`Rational` denotes ratios of integers with nonzero denominator, modulo an appropriate normalization/equivalence relation.

Rationals are particularly useful for exact constants and exact intermediate quantities.

Examples:

```text
1/10
1/3
355/113
```

may remain exact until a later realization requires finite approximation.

## Reals

`Real` denotes mathematical real quantities where the operation is semantically meaningful.

RFC 0021 does not claim that arbitrary exact real values are always finitely materializable or that arbitrary comparisons are decidable.

Computability and termination constraints interact with RFC 0022.

## Complex numbers

`Complex<D>` may be defined over an appropriate scalar domain `D`.

Complex arithmetic inherits exactness and approximation behavior from the scalar domain and operation contracts.

## Modular integers

`ModularInteger<N>` describes arithmetic modulo `N`.

For modular arithmetic, wraparound is not an overflow accident; it is the intended algebra.

This distinction is important because:

```text
u32 wrapping add
```

and:

```text
Integer add realized in 32 bits with overflow
```

are not the same semantic operation.

## Finite fields and specialized algebraic domains

The framework should permit additional exact domains such as finite fields where future workloads require them.

The language should not assume all numeric domains are ordered or support division in the same sense.

---

# Part II — algebraic capability rather than operator folklore

Numeric operations should be understood relative to explicit algebraic structures.

Candidate traits/properties include:

```text
Semigroup
Monoid
Semiring
Ring
Field
OrderedSemiring
OrderedRing
OrderedField
```

These mathematical classifications inform which rewriting laws can be validly claimed.

For example:

```text
Associative(add, Integer)
```

may hold exactly.

By contrast:

```text
Associative(add, BinaryFloat64)
```

does not generally hold under exact-result observation.

RFC 0020 relation claims should be used to express such laws where they become transformation authority.

---

## Arithmetic-law claims

Candidate properties include:

```text
Associative(op, domain)
Commutative(op, domain)
Distributive(op1, op2, domain)
Idempotent(op, domain)
Monotonic(op, domain)
Cancellation(op, domain)
```

Approximate versions may exist, but they must carry an explicit relation and error model:

```text
ApproximatelyAssociative(
    op,
    domain,
    metric,
    bound,
    assumptions
)
```

A backend optimization must consume a law claim valid for the exact operation/domain/context it transforms.

---

# Part III — exact arithmetic

## Exactness as an operation property

`ExactArithmetic` should describe the relation between logical operation and result, not simply a datatype.

Examples:

```text
Integer + Integer -> Integer
Rational * Rational -> Rational
Modular<N> + Modular<N> -> Modular<N>
```

can be exact.

A finite machine operation may also be exact where range evidence proves representability.

Example:

```text
u32 * u32 -> u64
```

can be exact for all input values.

## Exact intermediates

MNCS should permit a realization to use wider or arbitrary-precision intermediates to preserve an exact logical operation.

The width of the intermediate representation need not become part of logical program identity unless explicitly observed.

## Exact constants

Source literals should not necessarily be pre-rounded into their eventual machine formats.

Where feasible, canonical semantics may preserve exact literal meaning, for example:

```text
0.1 -> Rational(1,10)
```

or an exact decimal quantity, until a concrete realization is selected.

Surface-language policy remains future work.

---

# Part IV — correctly rounded finite arithmetic

## Exact-then-round semantic model

A strong finite-arithmetic contract should permit a model equivalent to:

```text
rounded_result = round_R(exact_operation(inputs), destination_format)
```

where `R` is an explicit rounding policy.

This model is strongly inspired by correctly rounded arbitrary-precision arithmetic systems such as MPFR.

The value of this design is that the language defines the mathematical relationship first and leaves instruction selection second.

## Correctly rounded versus faithfully rounded

RFC 0021 should distinguish:

```text
CorrectlyRounded
FaithfullyRounded
BoundedULP<N>
```

Correct rounding selects exactly the result mandated by the declared rounding rule.

Faithful rounding permits one of the adjacent representable results surrounding the exact mathematical result.

A ULP bound provides a different relation again.

They must not be collapsed into one generic `accurate` flag.

---

# Part V — IEEE-style binary floating-point domains

IEEE-style finite floating-point formats are important numerical domains, but they are not the mathematical real numbers.

A floating domain may include:

```text
finite normal values
finite subnormal values
positive zero
negative zero
positive infinity
negative infinity
NaN values
```

Exact details depend on the declared format/contract.

## Signed zero

`+0` and `-0` may compare numerically equal under one relation while remaining distinguishable under representation or operations such as reciprocal/sign extraction.

RFC 0020 observation scope is therefore important.

## NaN

NaN behavior must be explicit enough that optimizations cannot assume ordinary equality/order laws where NaNs remain possible.

A contract may:

- preserve NaN behavior;
- forbid NaN inputs;
- prove NaN impossibility;
- canonicalize NaNs under an explicit relation; or
- permit relaxed handling under a declared approximation profile.

## Infinity

Infinity may be part of a floating domain while remaining outside a mathematical finite-real subdomain.

## Subnormals

Subnormal handling can affect accuracy, performance, timing, and reproducibility.

Flush-to-zero behavior is therefore a semantic relaxation and must not be hidden as ambient target behavior.

---

# Part VI — rounding semantics

Candidate deterministic rounding policies include:

```text
NearestTiesEven
NearestTiesAway
TowardZero
TowardPositive
TowardNegative
AwayFromZero
```

Other format-specific policies may be defined explicitly.

## Rounding points

The location of rounding matters.

For example:

```text
round(a*b + c)
```

is not generally equal to:

```text
round(round(a*b) + c)
```

Therefore a realization must preserve declared rounding points unless the contract authorizes a relation that permits movement/fusion.

## Ambient rounding modes

Canonical semantics should not depend implicitly on mutable ambient hardware state.

If an execution environment exposes dynamic rounding modes, that dependency must become an explicit environment/state input.

---

# Part VII — overflow and underflow semantics

## Overflow policies

Candidate policies include:

```text
Trap
Checked
Wrap
Saturate
Widen
ProduceInfinity
PromotePrecision
RejectRealization
```

These policies are not interchangeable.

## Checked arithmetic

Checked arithmetic may return an explicit result/failure variant rather than causing an ambient trap.

## Saturation

Saturating arithmetic clamps to representable bounds and therefore realizes a different operation than modular or exact integer arithmetic.

## Widening

A widening operation changes destination representation/range to preserve exactness.

## Underflow

Underflow policy may distinguish:

```text
preserve subnormal
flush to zero
increase precision
signal failure
use gradual underflow
```

A target realization must establish whichever policy the numerical contract requires.

---

# Part VIII — fixed-point arithmetic

Fixed-point arithmetic should be first-class because it explicitly separates logical scale from integer storage.

Conceptually:

```text
Fixed<Rep, Scale>
```

represents a quantity related to its stored integer by a declared scale.

Important semantic parameters include:

```text
scale
range
resolution
rounding
rescaling
promotion
overflow
```

## Fixed-point multiplication

Multiplication changes scale unless the operation explicitly rescales the result.

That rescaling may itself require rounding.

## Fixed-point division

Division may require additional precision or rounding even when operands are exactly representable.

## Use cases

Fixed point is particularly relevant to:

- deterministic control systems;
- DSP;
- embedded systems;
- financial quantities;
- reproducible simulation; and
- ML inference.

RFC 0021 should allow these uses without forcing fixed point onto unrelated domains.

---

# Part IX — decimal semantics

Decimal arithmetic is distinct from binary floating point.

A decimal quantity such as `0.1` may be exact in a decimal representation while requiring approximation in a binary representation.

Candidate categories include:

```text
DecimalInteger
DecimalFixed
DecimalFloating
```

with explicit precision, exponent/scale, rounding, and exceptional behavior.

Decimal semantics are useful for human-entered quantities, finance, measurement conventions, and interoperability with decimal protocols.

RFC 0021 should not define decimal values as formatted binary floats.

---

# Part X — rational semantics

Exact rationals provide a bridge between symbolic exactness and finite realizations.

A canonical rational representation may normalize sign and common factors, but exact internal representation remains an implementation concern where identity is not observed.

Rational constants can defer rounding decisions.

Example:

```text
logical constant: 1/10
```

may later be realized as:

```text
decimal exact
fixed-point exact
binary rounded
rational pair
arbitrary precision
```

under the selected numeric contract.

---

# Part XI — validated interval arithmetic

## Enclosure semantics

Interval arithmetic represents a guarantee of the form:

```text
exact_value ∈ [lower, upper]
```

Rather than treating the interval as a vague estimate, RFC 0021 should model it as an explicit enclosure relation.

A valid interval realization must use outward rounding or another mechanism sufficient to preserve containment.

## Interval operations

Operations propagate enclosure guarantees.

The result interval may widen substantially where dependency information is lost; this is a quality issue but does not necessarily invalidate containment.

## Decorations/status

Future interval profiles may carry additional domain-validity or continuity metadata inspired by standardized interval arithmetic.

This RFC leaves exact decoration vocabulary open.

---

# Part XII — ball arithmetic

Ball arithmetic represents a midpoint plus a rigorous radius:

```text
m ± r
```

with the semantic requirement:

```text
exact_value ∈ [m-r, m+r]
```

Ball arithmetic is attractive for machine-native precision search because working precision can be increased until the resulting radius satisfies a contract.

Example:

```text
required radius <= 1e-30
```

Forge may search precision and algorithms until the requirement is established or the budget is exhausted.

The language should treat the enclosure relation, not one particular library representation, as the semantic concept.

---

# Part XIII — correlated uncertainty

Plain intervals may lose correlation when the same uncertain source appears multiple times.

RFC 0021 should therefore permit uncertainty objects to carry dependency identity.

Candidate future models include affine arithmetic:

```text
x = x0 + Σ ai*ei
```

where noise symbols record shared uncertain sources.

The constitutional rule is more important than one concrete calculus:

> **Uncertainty independence must never be invented by dropping provenance.**

This aligns directly with RFC 0018 evidence/provenance semantics.

---

# Part XIV — higher-order validated approximation

The framework should reserve room for models such as Taylor models:

```text
polynomial approximation + rigorous remainder enclosure
```

Such methods may provide stronger validated approximations for nonlinear systems.

They are not required in the initial core.

A generic category such as `ValidatedApproximationModel` should be sufficient for the first specification.

---

# Part XV — exact and computable real arithmetic

`Real` should remain conceptually independent of finite floating formats.

Possible exact-real realizations may use:

- Cauchy approximations;
- interval refinement;
- signed-digit streams;
- symbolic forms;
- lazy arbitrary precision; or
- other computable-analysis techniques.

However, RFC 0021 must not imply that every proposition about arbitrary reals is decidable or that every real-valued computation terminates.

Operations that require semidecidable/partial behavior interact with RFC 0022 termination/productivity semantics.

A future exact-real implementation is therefore a possible realization, not a promise of omniscient real-number computation.

---

# Part XVI — numerical error taxonomy

RFC 0021 should distinguish multiple error relations.

## Absolute error

```text
|computed - exact| <= epsilon
```

## Relative error

```text
|computed - exact| / |exact| <= epsilon
```

with explicit treatment near zero.

## ULP error

A result lies within a specified number of representable-unit steps from the correctly rounded target.

## Interval enclosure

The exact result lies within a declared interval.

## Ball radius

The exact result lies within a midpoint/radius enclosure.

## Probabilistic error

```text
Pr(error <= bound) >= p
```

under an explicit probability model.

## Backward error

The computed result is the exact result for a nearby input satisfying a declared perturbation bound.

## Empirical error

Observed error over a finite identified corpus/distribution.

Empirical error must not be promoted into a universal error theorem.

---

# Part XVII — forward error, backward error, and conditioning

These are separate numerical properties.

## Forward error

How far is the computed result from the exact result for the supplied input?

## Backward error

For what nearby input would the computed result be exact?

## Conditioning

How sensitive is the mathematical problem itself to input perturbation?

A numerically stable algorithm may produce a large forward error on an ill-conditioned problem.

Likewise, a well-conditioned problem may be solved poorly by an unstable algorithm.

Candidate evidence objects may include:

```text
ForwardErrorBound
BackwardErrorBound
ConditionEstimate
StabilityClaim
```

No single `numerically_stable = true` flag should replace these distinctions.

---

# Part XVIII — compensated arithmetic

Compensated algorithms such as compensated summation should be treated as realization strategies.

The semantic requirement should be stated independently:

```text
sum(values)
error <= e
```

Candidate realizations may include:

```text
naive sequential accumulation
pairwise reduction
compensated summation
higher-precision accumulation
exact accumulation
```

The language should not hard-code one compensation algorithm as the definition of summation.

---

# Part XIX — exact and Kulisch-style accumulators

An exact accumulator can preserve the exact mathematical sum of finite floating inputs before one final rounding.

This enables strong properties such as:

- order-independent accumulation;
- schedule-independent final results;
- deterministic parallel reduction; and
- correctly rounded dot products where a sufficient accumulator is available.

Candidate semantic pattern:

```text
ExactAccumulation {
    input_domain
    exact_accumulator_domain
    final_rounding
}
```

The exact bit width/representation of the accumulator belongs to realization design unless explicitly observed.

---

# Part XX — reduction semantics

Reduction operations deserve explicit contracts because associativity assumptions directly affect parallelization and reproducibility.

A conceptual reduction contract may include:

```text
ReductionContract {
    operator
    logical_domain
    input_order_semantics
    allowed_reordering
    accumulation_contract
    error_requirement
    reproducibility_requirement
}
```

Examples:

```text
sum exact integers
```

may permit arbitrary regrouping if overflow cannot occur in the logical domain.

By contrast:

```text
sum binary64 values with strict rounding order
```

may forbid regrouping.

Another program may permit arbitrary regrouping provided:

```text
relative_error <= 1e-6
```

and:

```text
schedule_independent
```

is satisfied.

This transforms reductions from compiler folklore into explicit semantic objects.

---

# Part XXI — reproducibility taxonomy

RFC 0021 should distinguish at least:

```text
BitwiseReproducible
OrderIndependent
ScheduleIndependent
ThreadCountIndependent
BackendIndependent
CompilerIndependent
TargetIndependent
EnvironmentIndependent
NumericallyReproducible<metric,bound>
StatisticallyReproducible<model>
```

These properties are incomparable in important ways.

A result may be numerically reproducible without being bitwise identical.

A stochastic computation may be statistically reproducible without reproducing individual samples.

A CPU and GPU realization may satisfy the same error contract while failing target-independent bitwise reproducibility.

RFC 0020 relation scopes should express these distinctions precisely.

---

# Part XXII — fused operations

Fused operations such as FMA have distinct semantics because intermediate rounding is removed.

Therefore:

```text
fma(a,b,c)
```

should not be assumed equivalent to:

```text
a*b + c
```

under strict finite arithmetic.

A numeric contract may explicitly authorize contraction or decontraction under a declared relation.

Fusion authority should be visible in transformation evidence.

---

# Part XXIII — decomposition of fast-math relaxations

MNCS should avoid one undifferentiated `fast_math` flag.

Candidate independent permissions include:

```text
allow_reassociation
allow_fma_contraction
allow_fma_decontraction
allow_signed_zero_collapse
assume_no_nan
assume_no_infinity
allow_subnormal_flush
allow_reciprocal_approximation
allow_approximate_division
allow_approximate_sqrt
allow_approximate_transcendentals
allow_reduction_reordering
allow_nondefault_rounding
```

Each permission must identify the relation that remains protected.

For example, reassociation may be permitted under a bounded relative-error contract while still prohibited under bitwise reproducibility.

---

# Part XXIV — approximate arithmetic as a first-class contract

Approximate arithmetic is not inherently invalid or second-class.

A program may intentionally require:

```text
relative_error <= 1e-4
```

rather than exact real arithmetic.

It may additionally prefer:

```text
minimize energy
maximize throughput
minimize memory bandwidth
```

This creates an explicit optimization envelope.

Forge may search representations and algorithms only within that envelope.

Approximation is therefore permission with constraints, not implicit loss.

---

# Part XXV — probabilistic and stochastic arithmetic

Stochastic rounding or probabilistic approximate algorithms must integrate with RFC 0011.

Candidate rounding contract:

```text
StochasticRounding {
    probability_model
    randomness_source
    replay_policy
}
```

A deterministic replay profile may bind the randomness source/seed.

A statistical profile may instead require distributional properties.

Probabilistic arithmetic must not silently satisfy deterministic relation claims.

---

# Part XXVI — alternative finite number systems

The RFC should explicitly permit alternative finite number systems as realizations where they satisfy the numerical contract.

Examples may include:

- binary floating formats;
- decimal floating formats;
- fixed point;
- block floating point;
- bfloat-like formats;
- tapered/posit-like formats;
- custom accelerator formats; and
- future machine-generated formats.

MNCS should not constitutionalize one of these as the universal numerical representation.

A candidate such as a posit/quire realization may be valuable for a particular range/error/accumulation contract without changing the language's logical numeric ontology.

---

# Part XXVII — mixed precision

A numerical realization may use different representations for different phases:

```text
input_representation
working_representation
accumulation_representation
output_representation
verification_representation
```

Example:

```text
f16 inputs
f32 multiply
f64 accumulation
f32 output
```

or:

```text
bfloat input
exact accumulator
correctly rounded f32 output
```

Mixed precision is a realization strategy.

The logical contract determines whether the strategy is valid.

---

# Part XXVIII — precision synthesis

Precision should be eligible to remain unresolved where the contract specifies required accuracy instead.

Conceptually:

```text
PrecisionVariable P
constraint:
    error <= epsilon
```

Forge or a numerical verifier may search:

```text
24 bits
53 bits
80 bits
113 bits
arbitrary precision
```

or target-native alternatives.

A successful candidate must carry evidence that the chosen precision satisfies the declared relation for the relevant domain/context.

Precision search itself must be bounded by RFC 0004 refinement budgets and resource policies.

---

# Part XXIX — transcendental functions

Operations such as:

```text
sqrt
sin
cos
tan
exp
log
pow
atan2
```

must declare accuracy semantics.

Candidate accuracy classes include:

```text
ExactWhereDefined
CorrectlyRounded
FaithfullyRounded
BoundedULP<N>
BoundedAbsoluteError<e>
BoundedRelativeError<e>
IntervalEnclosed
ImplementationSpecificApproximation
```

`whatever the host libm returns` should not be the canonical meaning of a portable operation.

Target-specific library calls may be realizations if their evidence satisfies the operation contract.

---

# Part XXX — conversion semantics

Conversion is a first-class numeric transformation.

Examples include:

```text
Integer -> BinaryFloat
BinaryFloat -> Integer
Decimal -> BinaryFloat
Rational -> Fixed
Binary64 -> Binary16
Interval -> point approximation
```

A `ConversionContract` may declare:

```text
rounding
overflow
underflow
saturation
invalid_input
information_loss
error_relation
```

An unchecked cast must not erase these semantics.

Conversions may generate obligations.

---

# Part XXXI — comparison semantics

Numerical comparison must respect the domain and approximation model.

Candidate relations include:

```text
ExactEquality
NumericEquality
RepresentationEquality
PartialOrderComparison
TotalOrderComparison
ApproximateEquality<metric,bound>
DefinitelyLess
PossiblyLess
```

For intervals or uncertain values, a comparison may legitimately be unresolved.

Example:

```text
[0,2] < [1,3]
```

is not definitely true or definitely false under ordinary interval interpretation.

The system should not force every numerical relation into a Boolean without a policy defining what that Boolean means.

---

# Part XXXII — arithmetic-generated obligations

Numeric operations should be able to generate semantic obligations automatically.

Examples:

### Division

```text
x / y
```

may generate:

```text
y != 0
result domain valid
result representable or permitted to approximate
error bound satisfied
```

### Square root

```text
sqrt(x)
```

under real semantics may generate:

```text
x >= 0
```

### Narrowing conversion

```text
Binary64 -> Integer32
```

may generate:

```text
finite
within range
rounding policy satisfied
```

### Reassociation

A candidate transformation may generate:

```text
reassociation permitted by contract
error relation preserved
reproducibility requirement preserved
```

These obligations connect RFC 0021 to RFC 0007 and RFC 0018.

---

# Part XXXIII — numeric relation claims under RFC 0020

Numerical correctness is best expressed through explicit relations.

Candidate relations include:

```text
ExactValueEquality
CorrectRoundingRelation
FaithfulRoundingRelation
AbsoluteErrorRelation
RelativeErrorRelation
ULPBoundRelation
IntervalContainmentRelation
BallContainmentRelation
BackwardStabilityRelation
ReproducibilityRelation
StatisticalAccuracyRelation
```

A realization should identify which relation it claims.

Example:

```text
RelationClaim {
    relation: RelativeError <= 1e-8
    left: exact_dot_product
    right: candidate_dot_product
    domain: D
    context: target/profile
    evidence: ...
}
```

A bounded test may support a bounded relation claim.

A proof/certificate may support a universal relation claim where appropriate.

The claim type must make that distinction visible.

---

# Part XXXIV — counterexamples are numeric-relation scoped

A candidate may satisfy one numerical relation and fail another.

Example:

```text
candidate A:
    relative error <= 1e-8        PASS
    bitwise reproducible          FAIL
    schedule independent          PASS
    target independent            UNKNOWN
```

This is not contradictory.

RFC 0020 relation semantics and RFC 0018 claim disposition should preserve these distinctions.

---

# Part XXXV — numerical assurance under RFC 0018

RFC 0021 does not define a universal numerical assurance ladder.

Instead, RFC 0018 assurance policies decide what numerical evidence is sufficient for a requested action.

Example experimental policy:

```text
bounded corpus agreement
AND no known contract violation
```

may authorize:

```text
run candidate in isolated benchmark
```

while a stronger policy may require:

```text
verified error bound
AND target applicability
AND independent numeric verifier
AND no conflicting evidence
```

for production promotion.

A policy may accept weaker evidence but may never relabel empirical accuracy as a proof of universal error bounds.

---

# Part XXXVI — numerical representation and RFC 0019

RFC 0019 separates logical type from physical representation.

RFC 0021 specializes that distinction for numerical values.

Conceptually:

```text
NumericValue<Real>
```

may have candidate realizations:

```text
Binary32
Binary64
Decimal64
FixedPoint
Rational
ArbitraryPrecision
Interval
Ball
PositLike
CustomAcceleratorFormat
```

The logical value and numerical contract determine which realizations are admissible.

Representation observations such as exact bit pattern, size, ABI layout, or encoding narrow that freedom explicitly.

---

# Part XXXVII — numeric target semantics and RFC 0017

Target support is evidence, not folklore.

A target may expose facts such as:

```text
native binary64
native FMA
flush-to-zero modes
vector widths
decimal instructions
accelerator tensor formats
exact accumulator support
```

These are realization facts.

They do not redefine the logical operation.

A target-specific realization must carry evidence that its actual behavior satisfies the declared numeric contract under the selected environment.

---

# Part XXXVIII — numeric memory/storage interactions

RFC 0009 owns storage, references, provenance, and lifetime.

RFC 0021 owns numeric value/operation semantics.

A bit pattern resident in memory does not automatically constitute a valid numeric value unless it satisfies the representation's validity rules.

This matters for:

- padding/trap representations where applicable;
- NaN payload handling;
- packed fixed-point formats;
- custom accelerators;
- endian conversions; and
- foreign boundaries.

---

# Part XXXIX — foreign numeric semantics

RFC 0015 owns unsafe/foreign boundaries and ABI.

A foreign numeric function should state which numeric relation its result is believed or evidenced to satisfy.

Examples:

```text
foreign sin() -> BoundedULP<2>
foreign decimal library -> CorrectlyRounded
foreign GPU kernel -> RelativeError<1e-5>
```

The language must not assume a foreign function has the same numeric semantics merely because its C signature uses `double`.

---

# Part XL — numeric observability

What is observable determines which numerical transformations are legal.

Candidate observations include:

```text
logical numeric value
rounded result
bit pattern
signed zero
NaN payload
exception flags
execution time
energy
reduction order
number of operations
working precision
```

A transformation may preserve logical numeric value within tolerance while changing representation or performance observations.

RFC 0020 observation models must therefore be used when claiming equivalence/refinement.

---

# Part XLI — numerical side effects and environment state

Some conventional floating environments expose mutable exception flags, rounding modes, or trap masks.

If MNCS supports these, they must be modeled explicitly as effects/state rather than invisible ambient behavior.

A strict portable numerical contract should be able to avoid dependence on ambient environment state entirely.

---

# Part XLII — numerical diagnostics

Numeric diagnostics should be structured and machine-readable.

Examples:

```text
rounding_requirement_unsatisfied
overflow_policy_mismatch
reassociation_not_authorized
fma_contraction_changes_relation
error_bound_unproven
error_bound_refuted
precision_budget_exhausted
reproducibility_requirement_unsatisfied
nan_precondition_unproven
subnormal_policy_mismatch
interval_enclosure_invalid
mixed_precision_relation_unknown
```

Diagnostics should identify:

- subject operation;
- claimed relation;
- realization;
- failed obligation;
- evidence/counterexample;
- assumptions;
- affected observation model; and
- conservative fallback where available.

---

# Part XLIII — numerical refinement in Forge

RFC 0021 should turn numeric design into a bounded realization search problem.

Example logical operation:

```text
dot(a,b)
```

Contract:

```text
logical_domain: Real
relative_error <= 1e-6
schedule_independent
no_nan_result
maximize throughput
```

Representation and algorithm are unresolved.

Forge may consider:

```text
f32 naive
f32 pairwise
f32 compensated
f64 accumulation
exact accumulator
mixed precision
posit-like + exact quire
SIMD implementation
GPU implementation
```

Each candidate produces relation/evidence obligations.

The search may compare:

```text
accuracy
reproducibility
latency
throughput
energy
memory bandwidth
proof cost
verification cost
portability
```

A candidate that violates a hard numeric requirement is outside the feasible region regardless of performance.

This direction anticipates RFC 0033 multi-objective realization search while remaining useful independently.

---

# Part XLIV — numerical deltas

Recursive refinement should report explicit numerical semantic changes.

Candidate delta fields include:

```text
representation_changed
precision_changed
working_precision_changed
accumulation_changed
rounding_changed
overflow_changed
exceptional_value_policy_changed
allowed_relaxations_changed
error_contract_changed
reproducibility_changed
numeric_relation_changed
```

A change that weakens a protected numerical guarantee must be visible as a semantic delta rather than hidden as an optimization detail.

---

# Part XLV — deterministic canonical numeric artifacts

Canonical semantic artifacts should avoid dependence on host parsing/printing quirks.

Exact constants, floating representations, decimal values, intervals, balls, and error bounds need deterministic serialization rules.

RFC 0027 will own general serialization/wire semantics, but RFC 0021 should require enough canonical numeric identity to support hashing, evidence dependencies, and deterministic verification.

---

# Part XLVI — numeric literal direction

RFC 0021 does not choose final source-literal syntax.

However, the language should avoid forcing every decimal token immediately into a binary floating value.

Possible elaboration directions include:

```text
42 -> exact integer
0.1 -> exact decimal/rational literal value
1e-9 -> exact scaled decimal value
```

followed by a context-driven conversion contract.

The syntax tournament in RFC 0005 should compare how well candidate syntaxes express these distinctions without excessive verbosity.

---

# Part XLVII — type-system interaction

The type system may expose numeric domains and contracts through type-level information where useful, but RFC 0021 should avoid requiring every numerical property to become nominal type identity.

For example:

```text
Real
```

and a requirement:

```text
relative_error <= 1e-8
```

need not imply a unique type named `Real1e8`.

Some properties are better represented as operation contracts, refinement predicates, or evidence obligations.

RFC 0007 and RFC 0013 own proof/type abstraction details.

---

# Part XLVIII — quantity and unit semantics

Physical units/dimensions are closely related but should not be silently conflated with numeric arithmetic.

RFC 0021 may support numeric domains parameterized by future quantity/unit semantics, but a dedicated dimensional-analysis RFC may be warranted later if the roadmap requires it.

For now, the numeric layer should remain capable of expressing arithmetic independent of physical units.

---

# Part XLIX — what belongs in the trusted kernel

RFC 0021 should keep the proof kernel small.

The kernel may eventually check proofs/certificates about numeric relations where encoded in RFC 0007 terms.

But the following should remain outside the kernel unless explicitly reduced to checkable proof objects:

- arbitrary precision search;
- interval/ball execution engines;
- SMT solvers;
- numerical analyzers;
- fuzzers;
- benchmark harnesses;
- probabilistic estimators;
- condition-number estimators;
- Forge search;
- target benchmarking; and
- empirical error measurement.

The evidence layer records their outputs without upgrading them to kernel truth.

---

# Part L — theoretical synthesis

RFC 0021 intentionally synthesizes ideas from multiple traditions rather than selecting one universal numerical calculus.

### Algebraic structures

Use algebraic structure to state which laws operations actually satisfy.

### Exact arithmetic

Treat integers, modular domains, and rationals as exact where their logical operations permit.

### Correct rounding

Adopt an exact-then-round semantic ideal for strongly specified finite arithmetic.

### IEEE-style finite arithmetic

Model finite binary floating point as its own numerical domain including special values and rounding behavior.

### Fixed-point and decimal arithmetic

Provide explicit scale, range, precision, and rounding semantics.

### Interval arithmetic

Use rigorous containment as a first-class relation.

### Ball arithmetic

Permit midpoint/radius validated approximation with adaptive precision.

### Affine/Taylor-style methods

Reserve structured correlated/higher-order approximation models.

### Computable exact reals

Permit exact-real semantics/realizations without pretending every real proposition is decidable or terminating.

### Numerical analysis

Distinguish forward error, backward error, conditioning, and stability.

### Compensated and exact accumulation

Treat them as realization strategies for strong reduction contracts.

### Reproducible arithmetic

Make order, schedule, target, and bitwise reproducibility explicit.

### Stochastic rounding

Integrate probabilistic arithmetic with RFC 0011.

### Alternative number systems

Permit posit-like and future formats as realization candidates, not constitutional commitments.

### Mixed precision

Treat precision schedules as realizations of one logical operation.

### Precision synthesis

Permit numerical precision to be solved from error requirements under bounded search.

---

# Part LI — scope boundaries

## RFC 0006 — machine intent

RFC 0006 owns explicit low-level operation intent and lowering envelopes. RFC 0021 supplies the fuller numerical semantic vocabulary that such intent may reference.

## RFC 0007 — proof core

RFC 0007 owns proof theory and kernel-checkable propositions.

## RFC 0009 — memory/storage

RFC 0009 owns storage, references, lifetime, provenance, and memory validity.

## RFC 0010 — concurrency

RFC 0010 owns concurrency and memory consistency. RFC 0021 specifies reduction/numeric reproducibility requirements that concurrent realizations must preserve.

## RFC 0011 — nondeterminism

RFC 0011 owns stochastic/nondeterministic execution semantics.

## RFC 0012 — executable semantics

RFC 0012 owns the general transition/execution model.

## RFC 0013 — abstraction/polymorphism

RFC 0013 owns generic numeric abstraction and evidence parameterization.

## RFC 0015 — FFI/unsafe

RFC 0015 owns numeric trust transfer across foreign boundaries.

## RFC 0017 — target/environment

RFC 0017 owns target facts and execution-context facts.

## RFC 0018 — assurance

RFC 0018 decides what numeric evidence authorizes a requested action.

## RFC 0019 — values/types/representations

RFC 0019 owns the general logical-value/representation distinction. RFC 0021 specializes it for numerical domains.

## RFC 0020 — relations

RFC 0020 owns equality/equivalence/refinement/substitutability relation machinery. RFC 0021 defines important numeric relation families.

## RFC 0022 — termination/productivity

RFC 0022 should own termination/productivity/partiality issues arising from exact-real or adaptive-precision computation.

## RFC 0027 — serialization

RFC 0027 should own general wire/schema serialization semantics, while RFC 0021 requires deterministic canonical numeric artifacts sufficient for identity/evidence.

## RFC 0032 — quantitative resources

RFC 0032 should own time/memory/energy/resource costs of numeric realizations.

## RFC 0033 — realization optimization

RFC 0033 should own general multi-objective realization selection; RFC 0021 exposes numeric choices into that search space.

---

# Part LII — non-goals

RFC 0021 does **not**:

- mandate IEEE binary floating point as the universal numeric model;
- mandate posits or any alternative format;
- require arbitrary precision everywhere;
- require interval or ball arithmetic everywhere;
- make every numerical property part of type identity;
- choose final literal syntax;
- choose a universal numerical-analysis library;
- promise decidable equality for arbitrary exact reals;
- promise termination for arbitrary adaptive/exact-real operations;
- define physical units/dimensional analysis fully;
- define a universal `fast_math` mode;
- treat empirical accuracy as proof;
- specify one universal accumulation algorithm;
- require bitwise reproducibility for all programs; or
- make target-specific numerical behavior part of the portable logical ontology.

---

# Part LIII — security and safety consequences

Numerical semantics can become security- or safety-relevant.

Examples include:

- NaN behavior changing comparisons or guards;
- integer overflow invalidating bounds checks;
- signed-zero collapse changing branch behavior;
- flush-to-zero affecting control loops;
- reassociation changing convergence;
- precision loss invalidating physical safety margins;
- stochastic arithmetic weakening reproducibility assumptions;
- target-specific approximations violating validated error bounds; and
- unchecked conversions corrupting indexes, sizes, or financial values.

Therefore protected numeric contracts must participate in Forge promotion policies and trust boundaries.

RFC 0024 may later add information-flow/side-channel properties to numeric observations such as timing or data-dependent floating behavior.

---

# Part LIV — first implementation experiment

The first executable experiment should remain deliberately bounded.

It should not attempt to implement the entire numerical vision.

## Artifact model

Introduce versioned research artifacts for at least:

```text
NumericDomain
NumericContract
RoundingPolicy
OverflowPolicy
ErrorRequirement
ReproducibilityProfile
NumericRepresentation
NumericRealization
NumericRelationClaim
NumericEvidence
```

## Minimal domains

Support a narrow set:

```text
Integer
Modular<2^32>
Rational
Binary32
Binary64
Fixed<i64, 1e-3>
```

The exact names are provisional.

## Minimal rounding modes

Support at least:

```text
NearestTiesEven
TowardZero
TowardPositive
TowardNegative
```

## Minimal error models

Support:

```text
Exact
AbsoluteError
RelativeError
ULPBound
```

## Minimal reproducibility properties

Support:

```text
BitwiseReproducible
OrderIndependent
ScheduleIndependent
```

## Numeric operations

Implement a small set:

```text
add
sub
mul
div
convert
reduce_sum
fma
```

## Reference evaluator

Implement a high-precision/exact reference path sufficient to compare selected finite realizations over bounded corpora.

This reference path is a study oracle unless backed by a stronger proof/certificate.

## Required fixtures

The pilot should include:

1. exact integer addition;
2. modular wraparound distinguished from integer overflow;
3. rational `1/10` converted into binary32/binary64 under explicit rounding;
4. fixed-point exact representation of a decimal quantity that is not exactly binary-representable;
5. a floating reassociation counterexample;
6. an FMA contraction example where strict and fused semantics differ;
7. a reduction whose naive result changes with order;
8. a compensated or wider-accumulation realization with improved error;
9. one exact/order-independent accumulator experiment if feasible;
10. one error-bound contract satisfied by multiple different realizations;
11. one realization rejected for violating the same contract;
12. one precision-search experiment that selects the smallest tested precision satisfying a target error bound;
13. one explicit `BoundedAgreement` report that is refused as a universal proof under RFC 0018; and
14. one numerical relation counterexample that refutes one relation while leaving a narrower relation intact.

## Optional enclosure pilot

If implementation cost remains modest, add one interval or ball enclosure example with outward/rigorous containment checking.

This should remain optional for the first slice rather than block the core numeric-contract experiment.

## Forge integration

Expose one bounded numeric realization search:

```text
logical operation: dot/reduce_sum
hard requirement: relative error <= bound
hard requirement: schedule independent
preference: throughput
```

Candidate realizations may vary accumulation precision/algorithm.

No candidate may be promoted solely because it is faster.

## RFC 0018 integration

Define at least two assurance profiles:

### ExperimentalNumericUse

May accept bounded empirical error evidence for isolated experiments.

### StrongNumericPromotion

Requires stronger relation evidence, target applicability, freshness, and no unresolved critical refutation.

The exact profile predicates remain research artifacts.

---

# Part LV — success criteria for the first experiment

The pilot succeeds if it demonstrates all of the following:

- mathematical operation identity remains distinct from representation;
- the same logical quantity can have multiple numeric representations;
- rounding policy changes observable result in a controlled fixture;
- overflow policy changes operation semantics explicitly;
- one floating algebraic rewrite is rejected without authority;
- one relaxed contract permits a transformation strict semantics rejects;
- one reduction contract distinguishes order-sensitive and order-independent realizations;
- one mixed/higher-precision realization satisfies a contract a lower-precision realization does not;
- bounded empirical evidence remains tagged as bounded;
- numeric relation evidence integrates with RFC 0020;
- action authorization integrates with RFC 0018; and
- diagnostics explain failed numeric obligations in machine-readable form.

The pilot does **not** establish complete numerical correctness for MNCS Language.

---

# Part LVI — unresolved questions

The following remain intentionally open:

1. Which exact numeric domains belong in the minimal portable core?
2. Should arbitrary-precision integers/rationals be mandatory semantic domains or library-level realizations?
3. How should exact decimal literals elaborate before a surrounding type/contract is known?
4. Which IEEE floating exception/status semantics should be portable language behavior?
5. Should signaling NaNs be represented in the portable core?
6. How much NaN payload identity should be observable?
7. Should dynamic rounding modes exist as ordinary stateful effects or only through explicit operation parameters?
8. Which decimal formats belong in the first implementation?
9. What is the minimum fixed-point vocabulary?
10. How should interval decorations/status be represented?
11. Should ball arithmetic become a standard library concept or a core semantic relation?
12. Which uncertainty dependency model is sufficient for early validated numerics?
13. How should exact-real operations report unresolved comparisons or exhausted precision budgets?
14. Which error metrics should be primitive versus extensible?
15. How should near-zero relative error contracts be normalized?
16. How should probabilistic error interact with RFC 0018 assurance policies?
17. How should statistical reproducibility be specified independently of a particular RNG?
18. What reproducibility guarantees are realistic across CPU/GPU/accelerator targets?
19. Should exact/Kulisch accumulators be a standard reduction realization?
20. How should compiler transformations compose local numeric error bounds into whole-program bounds?
21. What relation algebra is needed to compose rounding/error claims through HIR/SSA lowering?
22. How should quantitative proof cost influence precision synthesis?
23. When may empirical evidence justify widening a realization envelope?
24. Which transcendental accuracy classes belong in the portable core?
25. How should foreign libm implementations advertise evidenced accuracy?
26. How should numeric contracts interact with future physical-unit/dimensional semantics?
27. How should verified numerical kernels expose proofs without bloating ordinary artifacts?
28. How much numerical metadata should survive to machine code versus evidence manifests?
29. Can Forge safely invent new custom finite formats under explicit encode/decode/error obligations?
30. What independent implementations/verifiers are needed before claiming strong numeric portability?

---

# Part LVII — longer-term experiments

After the bounded first slice, promising experiments include:

- binary32/binary64 correctly-rounded reference comparison;
- decimal versus binary constant realization studies;
- fixed-point control kernels;
- interval and ball validated kernels;
- precision synthesis for transcendental chains;
- mixed-precision matrix kernels;
- compensated versus exact accumulation;
- CPU/GPU reproducibility studies;
- deterministic parallel reductions;
- stochastic-rounding training/inference kernels;
- posit-like/quire realization experiments;
- automatic numeric contract inference from verified downstream requirements;
- error-bound propagation through HIR/SSA;
- relation-preserving reassociation under explicit approximate contracts;
- target-specific numerical realization search; and
- machine-generated numeric formats constrained by range/error/energy requirements.

Every experiment must preserve the distinction between:

```text
what the program requires
what the realization claims
what evidence establishes
what assurance policy authorizes
```

---

## Proposed direction

RFC 0021 proposes that MNCS adopt the following broad direction:

> **Specify the mathematics and acceptable loss of information first; choose the arithmetic machinery second.**

A logical operation should be able to state:

```text
exact when required
otherwise bounded by a declared relation
round under an explicit policy
handle exceptional conditions explicitly
meet a declared reproducibility profile
```

while leaving:

```text
format
precision
accumulator
algorithm
vectorization
hardware realization
```

open when they are not semantically observed.

That turns numerical programming from a fixed primitive-type choice into an evidence-bearing realization problem.

Forge can then search numerical design space aggressively without receiving authority to silently redefine numerical meaning.

This is the intended machine-native numerical foundation for MNCS Language.
