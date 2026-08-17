# RFC 0007: Proof-Carrying Dependent Core

- **Status:** Draft
- **Target:** Cross-cutting 0.3–1.0

## Summary

Make formal proof a native semantic dimension of MNCS Language rather than a verifier added after ordinary code generation.

The proposed direction is a small, independently checkable dependent core inspired by the major dependent-type traditions, surrounded by elaboration, automation, external solvers, empirical verifiers, and effectful execution that do not automatically become part of the trusted kernel.

An MNCS artifact may therefore contain executable semantics, claims, generated proof obligations, formal proof terms or checkable certificates, assumptions, empirical evidence, and unresolved obligations as separately identified objects.

The language should be designed so that machine authors normally generate implementation and justification together. Constructs that introduce risk should generate obligations automatically. Transformations should preserve or explicitly change claims through checkable evidence rather than relying on compiler folklore.

This RFC establishes a direction, not a final calculus. Universe structure, equality, erasure, effect semantics, recursion, and proof-object encoding remain research decisions.

## Motivation

MNCS Language already treats contracts, effects, capabilities, assumptions, evidence, machine intent, lowering envelopes, and transformation provenance as program meaning. RFC 0003 also requires proof obligations to survive through HIR and verified SSA.

A conventional verification architecture would still leave a gap:

```text
program -> external verifier -> report
```

The verifier report can say that a property passed, but the language has no native logical object that a small checker can independently validate.

A machine-native language can choose a different default:

```text
semantic artifact
      +
formal obligations
      +
proofs/certificates
      ↓
small proof kernel
      ↓
accepted formal claims
```

Large and probabilistic systems may search for proofs. The final formal trust decision should not require trusting those systems.

This is especially valuable for autonomous generation and recursive refinement. A generator may be creative, stochastic, self-modifying, or wrong; a proof checker should remain narrow, deterministic, versioned, and independently implementable.

## Design principle: executable artifact plus assurance structure

A future MNCS program is not merely source text that happens to compile. Its semantic identity should be able to bind:

```text
Executable semantics
Contracts and claims
Assumptions
Effects and capabilities
Proof obligations
Formal proof objects
Checkable certificates
Empirical evidence
Unresolved obligations
Transformation provenance
```

Compilation under an assurance profile may then mean:

> produce an executable realization whose required obligations are discharged to the level demanded by policy, while preserving explicit UNKNOWN where they are not.

The base language MAY continue to represent incomplete candidate artifacts. Policy, not representation alone, determines whether an artifact with unresolved obligations can be promoted or executed.

## Dependent-type traditions considered

MNCS should not copy one proof assistant wholesale. The relevant traditions solve different parts of the problem.

### Intensional Martin-Löf type theory

Useful ideas:

- dependent function (`Pi`) and dependent pair (`Sigma`) types;
- propositions-as-types and proofs-as-terms;
- inductive families for making invariants part of data representation;
- constructive computation and normalization;
- explicit identity types rather than global equality reflection.

MNCS direction:

Use an intensional, computational core as the conservative baseline because explicit proof objects and decidable checking are well aligned with a small kernel and content-addressed artifacts.

### Extensional dependent type theory

Extensional systems can make ordinary mathematical equality much more convenient by reflecting propositional equality into definitional equality.

MNCS direction:

Do not adopt unrestricted equality reflection in the initial kernel. It risks making checking and implementation more complex and can blur whether equality was computed, assumed, or proved. Extensional reasoning should instead be explored through explicit principles, elaboration, observational techniques, or separately versioned extensions.

### Calculus of Constructions and Calculus of Inductive Constructions

Useful ideas:

- one typed language for functions, propositions, and proofs;
- universe hierarchies;
- inductive and potentially coinductive definitions;
- small proof-checking kernels with richer elaborators outside the trusted core;
- proof/program separation through erasure where justified.

MNCS direction:

Adopt the architectural lesson more strongly than any exact calculus: keep the trusted core small enough for independent implementations, while allowing rich machine-facing elaboration and proof search outside it.

### Lean-style elaboration and kernel separation

Useful ideas:

- expressive surface terms elaborate to a smaller core;
- tactics, metaprograms, automation, and unification need not be trusted if the kernel rechecks the resulting term;
- recursive definitions can elaborate into primitive recursors checked by the kernel.

MNCS direction:

Treat LLMs, Forge workflows, SMT solvers, proof search, specialized MNEL models, and future agent tooling as untrusted producers of candidate terms or certificates. They may improve proof search without enlarging the formal trust boundary.

### Agda/Idris-style dependently typed functional programming

Useful ideas:

- indexed data and pattern matching as normal programming tools;
- elaboration that lets users or machines omit reconstructible proof detail;
- total functional definitions where totality matters;
- type-directed program construction.

MNCS direction:

The semantic graph should support dependent functional structure even if the eventual machine form is not optimized for human-written theorem syntax. Proof burden should be synthesized from semantics whenever possible instead of requiring hand-authored tactic scripts.

### Quantitative and linear dependent type theory

Useful ideas:

- track not only what a value is, but how often or under what usage discipline it may be consumed;
- express resource protocols, state transitions, erasure, and linear capabilities in the type system.

MNCS direction:

This is particularly relevant to MNCS capabilities, authority, memory regions, channels, and machine resources. The initial logical kernel should not require a final quantitative calculus, but the semantic model SHOULD reserve usage/grade information so that linear or quantitative obligations can become native rather than reconstructed later.

### Homotopy type theory

Useful ideas:

- treat identity as structured rather than automatically proof-irrelevant;
- reason about equivalence and higher structure explicitly;
- distinguish equality of representation from equivalence of behavior.

MNCS direction:

Do not require higher-dimensional identity in the initial programming kernel. Preserve enough semantic separation that later equivalence-oriented extensions are possible, especially for transformations and representation-independent identities.

### Cubical type theories

Useful ideas:

- computational treatment of paths and extensional principles;
- constructive accounts of function extensionality and univalence in suitable systems;
- equality with computational content rather than only axiomatic postulates.

MNCS direction:

Reserve cubical or path-based equality as a research extension rather than the baseline. It may later offer useful machinery for verified equivalence and transport across representations, but should earn its kernel complexity through experiments.

### Observational and reflection-free extensional approaches

Useful ideas:

- recover useful extensional equality behavior without unrestricted equality reflection;
- make observationally equal programs easier to relate while retaining computational checking goals.

MNCS direction:

Investigate these approaches for transformation equivalence and ergonomic equality after the minimal intensional core is executable.

## Proposed logical stratification

MNCS should not force every executable program to live entirely inside a terminating pure theorem language.

Use at least two semantic strata.

### Total proof/core stratum

The trusted logical core should contain only constructs whose checking and computation rules are precisely specified. Initial research candidates include:

- universes;
- dependent functions and pairs;
- inductive data/families;
- explicit equality;
- structural recursion/recursors;
- propositions or proposition-like types;
- typed proof terms;
- assumption constants with explicit identity.

Formal proof terms in this stratum are kernel-checkable.

### Executable/effectful stratum

General programs may contain:

- partial or long-running computation;
- state;
- I/O;
- concurrency;
- foreign calls;
- unsafe machine operations;
- target-specific realizations.

These operations do not enter the pure logic by pretending they are total. Instead, their language semantics generate contracts and obligations about the behavior that matters.

This avoids two bad extremes:

1. making the language impractical by requiring every real program to be a total pure function; or
2. making the logic unsound by admitting unrestricted effects or nontermination into proof computation.

The exact boundary remains design work.

## Proof obligations should be generated by language constructs

The authoring machine should not have to remember every safety theorem manually.

Examples:

```text
index(array, i)
  -> obligation: 0 <= i < length(array)

divide(x, y)
  -> obligation: y != 0

raw_load(region, address, T)
  -> obligations:
       address belongs to region
       region permits read(T)
       alignment is sufficient
       lifetime is valid

spawn_with(capability)
  -> obligations:
       capability is transferable
       source retains only permitted authority

transform(before, after)
  -> obligation:
       protected semantics are preserved or explicitly weakened
```

The obligation identity must be stable enough to participate in evidence invalidation and transformation provenance.

## Formal proof is not the same as all evidence

RFC 0002 remains valid. MNCS must not collapse theorem proving, analysis, testing, and observation into one status.

A future evidence object should distinguish at least the method class:

- **formal proof term** — directly checked by the MNCS proof kernel;
- **checkable certificate** — validated by a small deterministic checker or translated into a kernel term;
- **soundness-scoped verifier result** — produced by a verifier whose model and assumptions are explicit;
- **empirical observation** — produced by bounded execution or measurement;
- **assumption** — accepted premise supplied across a trust boundary.

`PASS`, `FAIL`, and `UNKNOWN` remain obligation outcomes under a declared method and scope. `UNKNOWN` must never be silently converted into proof.

Formal proof can establish only a formalized proposition under its assumptions. Empirical claims about clocks, hardware, networks, sensors, performance, external services, or physical systems remain observations or assumptions unless a model and its boundary are explicitly represented.

## Small kernel rule

The MNCS proof kernel should aim to do less, not more.

It should ideally be responsible for:

- checking well-formed core terms;
- checking types and universe constraints;
- checking definitional equality/reduction for the selected core;
- validating inductive/recursive constructions admitted by the core;
- checking proof terms against propositions;
- reporting exact assumptions used;
- producing deterministic diagnostics and canonical identities.

It should not contain:

- LLM inference;
- heuristic proof search;
- tactics;
- SMT search;
- Forge orchestration;
- optimizer heuristics;
- network access;
- policy decisions;
- empirical benchmarking;
- automatic promotion.

Where an external solver can emit a compact independently checkable certificate, the certificate checker may either remain a distinct trusted component with an explicit identity or translate the result into a kernel-checkable proof object. The trust distinction must remain visible.

## Proof-carrying transformations

Machine-authored software will be transformed frequently. The language should therefore prove transformations, not only final functions.

A transformation should identify:

```text
input semantic identity
output semantic identity
protected claims
permitted semantic delta
required assumptions
proof/certificate obligations
target and lowering envelope
```

An optimization may be accepted because it carries evidence that the protected semantics are preserved. If only a weaker relation is established, the artifact must state that weaker relation rather than laundering it into equivalence.

This should compose with RFC 0003 transformation provenance and verified SSA.

## Proof-aware generation objective

For a machine-native language, provability can be an optimization dimension.

An agent may choose between two implementations based on:

```text
runtime cost
code size
proof search cost
proof checking cost
invalidation radius
assumption count
portability
required authority
assurance profile
```

A slightly slower implementation that is dramatically easier to prove may be the better MNCS artifact.

This is a feature, not overhead accidentally imposed on an otherwise conventional language.

## Proof-gated unsafe operations

Unsafe operations should be explicit and should introduce stronger proof/evidence obligations rather than acquiring ambient permission.

Examples include:

- raw memory access;
- pointer reinterpretation;
- unchecked arithmetic assumptions;
- authority transfer;
- foreign calls;
- target-specific intrinsics;
- relaxed atomic behavior;
- floating-point relaxation;
- externally visible state mutation.

A candidate artifact may retain unresolved obligations for research, but an assurance profile may prohibit lowering or execution until the required obligations are discharged.

## Relationship to Forge, Fabric, and recursive refinement

The language defines the semantic subject, claims, obligations, proof objects, and evidence identities.

The proof kernel checks formal proof objects.

Forge may orchestrate proof search, external verification, testing, comparison, and policy-controlled candidate evaluation.

Fabric may execute bounded jobs and produce observations and receipts.

Recursive refinement may generate a new candidate implementation and corresponding proof obligations.

None of those systems receives formal authority merely because it generated the artifact being checked.

## Initial implementation experiment

Do not attempt a complete theorem prover immediately.

A useful first experiment is a tiny proof-core artifact independent of final source syntax:

1. a versioned term representation;
2. a small universe hierarchy;
3. natural numbers or another minimal inductive type;
4. dependent functions;
5. equality;
6. a handful of proof terms;
7. deterministic kernel checking;
8. canonical proof identity/hash;
9. one generated obligation from an existing machine-intent or executable-body construct;
10. one valid proof, one invalid proof, and one unresolved obligation;
11. an independent second checker or cross-check experiment before stronger trust claims.

The first target should prove something already present in the executable subset, such as a bounded arithmetic property or a transformation precondition. It should not yet attempt general memory safety, compiler correctness, or full program equivalence.

## Compatibility requirements

This direction must preserve existing MNCS principles:

- properties remain separate from evidence;
- evidence remains scoped to exact subjects and assumptions;
- empirical execution is not reclassified as proof;
- UNKNOWN remains first-class;
- authority and effects remain explicit;
- canonical identities and invalidation remain deterministic;
- candidate generation does not self-certify promotion;
- backend promises remain conservative;
- the final grammar remains undecided.

## Rejected directions

### Make Lean, Rocq/Coq, Agda, or Idris the MNCS language

Rejected as the architectural default. These systems are important references and may be excellent external verifiers, but MNCS needs semantics for authority, evidence gradients, machine intent, target realizations, recursive refinement, and empirical artifacts that are not merely theorem-prover libraries.

### Trust a powerful omniscient verifier

Rejected. It enlarges the trusted computing base and conflicts with independent checking.

### Require every program to be fully proved before it can exist

Rejected. Candidate artifacts and UNKNOWN obligations are useful research objects. Promotion and execution policies may require stronger assurance without making incomplete semantic states unrepresentable.

### Treat tests as proofs

Rejected. Bounded observation remains evidence, not universal formal validity.

### Put all compiler optimizations inside the proof kernel

Rejected. Optimization search should remain outside the kernel; only the relevant proof or certificate should cross the formal trust boundary.

## Unresolved questions

- exact dependent calculus and universe hierarchy;
- cumulative versus non-cumulative universes;
- `Prop` separation, proof irrelevance, and erasure;
- definitional equality and normalization strategy;
- equality principles and possible observational/cubical extensions;
- inductive, coinductive, quotient, and higher-inductive support;
- totality checking and the boundary to partial computation;
- effect typing and capability interaction;
- linear/quantitative usage semantics;
- proof-term canonicalization and identity;
- proof compression and certificate formats;
- proof composition across modules and packages;
- trusted versus independently checkable solver certificates;
- FFI and unsafe boundaries;
- concurrency and memory-model propositions;
- floating-point and target semantics;
- proof-preserving HIR/SSA lowering;
- proof invalidation granularity;
- independent kernel implementations and cross-check policy;
- whether compiled artifacts carry full proofs, compressed certificates, hashes, or references.

## Research references

This RFC is informed by the broad dependent-type lineage rather than one implementation, including:

- Martin-Löf type theory and the program/specification correspondence developed in *Programming in Martin-Löf's Type Theory*;
- the Calculus of Constructions and Calculus of Inductive Constructions used by Rocq/Coq;
- Lean's dependent core and minimal proof-checking kernel architecture;
- Agda and Idris approaches to dependent functional programming;
- Quantitative Type Theory as applied in Idris 2;
- Homotopy Type Theory and computational cubical type theory;
- observational and reflection-free approaches to extensional equality.

These references are design inputs, not compatibility commitments.
