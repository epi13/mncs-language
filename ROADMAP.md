# Roadmap

This roadmap separates semantic research from final surface-language selection while treating recursive introspection, representation efficiency, machine-intent semantics, proof-carrying semantics, and refinement as cross-cutting requirements.

## Status vocabulary

Use these labels mechanically. Adjacent infrastructure is not completion.

| Label | Meaning |
| --- | --- |
| **Implemented / exercised** | Code exists, tests/experiments run, identities retained. |
| **Implemented but partial** | Real code path exists; envelope, execution, or evidence is incomplete. |
| **In progress** | Active implementation; acceptance criteria not satisfied. |
| **Scaffolded / interface exists** | Types, traits, or docs exist without a meaningful lowering/execution. |
| **Blocked / unresolved** | Known missing evidence, proof, or host capability. |
| **Planned** | Intended, not started. |
| **Research-only / RFC-defined** | Design exists; not an executable claim. |

Source Profiles (**0.1–0.9** language envelopes) are **not** project roadmap
milestones (0.1–0.9). Source Profile 0.4 does not mean roadmap 0.4 is the
current completion gate, and Source Profile 0.5 does not complete roadmap
milestone 0.5.

## Current implementation snapshot (2026-08)

### Language

Source Profiles **0.1–0.10** exist and remain additive. Profiles 0.6 (payload
sums, strict booleans, checked division, explicit arithmetic intents, module
imports) and 0.7 (bounded data: `byte`, fixed sequences `[E; N]`, bounded
views `[E; up_to M]` with runtime length observations, three-state bounds
evidence on element observation, checked view ranges, bounded sequence
traversal, explicit total conversions, integer shifts), and 0.8 (branchless
selection, integer vectors, logical masks, and fixed-order reductions) are
described in their profile documents. Profile 0.9 adds namespace-qualified
imports/references, aliases, fail-closed duplicate handling, and an
identity-stable semantic binding/reference table for tooling and evidence.
Earlier-profile programs keep their semantics and canonical identities.

Profiles 0.1–0.5 remain as originally summarized below. Programs can express modules, functions, contracts/effects/capabilities, scalar integers with wrapping/checked intent, finite enums, exhaustive match, calls with authority closure, and Profile 0.4 bounded iteration (`iterate … up_to N carrying …`, bound 1–32). **Profile 0.5 adds logical record values** (RFC 0019): nominal record declarations with canonical field identity (`record_type_id` hashes sorted `name:type;` pairs — declaration order never changes the logical type), construction with exact field coverage, functional update from a base value, and field projection. Records are values without physical layout; record equality operators are explicitly **not defined** in 0.5. Records cannot yet carry through bounded-iteration state or cross module boundaries beyond calls.

### Compiler

Executable stages: source envelope → CST/AST → semantic program → HIR → SSA → selected SSA → `TargetLoweringPlan` → backend adapter → `BackendArtifact` + evidence. Transformations are identity-bound. Profile 0.9 also publishes the semantic binding substrate from the same elaboration that feeds body, HIR, SSA, and execution; Profile 0.10 adds explicit type/Nat parameters and deterministic monomorphization on the same path. Generic templates are rejected at the lowering boundary, and concrete SSA is checked for unresolved generic types, parameterized bounds, and invalid specialization provenance. Experimental: evidence-gated no-overflow elision, translation validators, backend promises. Missing: general memory/layout, unrestricted loops, proof kernel, and soundness beyond the bounded explicit-polymorphism tranche.

### Backends

| Realization | Exists | Local execute | External tools | Status (2026-08) |
| --- | --- | --- | --- | --- |
| portable WASM MVP | yes | embedded interpreter | no | records, payload sums, bytes, sequences/views including language-level signatures, traversal, wrapping vectors/masks realized |
| research bytecode | yes | SSA interpreter | no | full language envelope including sequence-, view-, vector-, and mask-typed language-level signatures |
| LLVM IR | yes | external clang | clang/llc | records, payload sums, bytes, sequences/views/vectors/masks including language-level signatures through canonical cells + packed descriptors |
| C11 | yes | external clang/gcc | clang/gcc | same composite/sequence/view/vector/mask envelope as LLVM, including process-boundary marshal |
| Cranelift CLIF/JIT/AOT | yes | host JIT for the scalar envelope | host ISA and executable-memory policy | composite cells via libcalls, including sequence/view/vector/mask signatures; guarded division into structured failure; saturating/widening per declared promises |
| RISC-V 32 / eBPF / PTX | yes | artifact generation only | llc (+ target tools) | RV32IM ELF and PTX modules produced + structurally validated; execution honestly unavailable on hosts without emulators/GPUs; eBPF refuses >5-register signatures precisely (`CGX410`) |
| SPIR-V, extra VMs | no | n/a | n/a | planned |

All five executable backends agree over the Profile 0.7 bounded-data and
serialization differential corpora and over sequence-/view-/vector-typed
library exports (layered body/SSA/backend validation PASS). Overall
experiment status stays UNKNOWN where runtime-checked view ranges and
dynamic indexes retain unresolved obligations — by design. Nested composite
sequence elements and logical `[bool; N]` windows are now realized on the
five executable backends; sequences of masks or vectors remain refused.

WASM is one realization. Generic orchestration is adapter-neutral. The
Profile 0.10 generic corpus is retained as backend-by-backend evidence; an
experiment remains `UNKNOWN` when unresolved runtime obligations remain, even
when returned values agree.

### Verification

MNCS verifies capability/effect closure, finite exhaustiveness, bounded-iteration metadata, SSA validation, evidence freshness, and translation validators over finite corpora. `UNKNOWN` remains a first-class outcome (including exact instruction cost). Bounded corpus agreement is not proof.

### Experiments

CRE-1, CRE-2, and CRE-3 run through WASM, research bytecode, LLVM, and C11. Cranelift lowers the same selected SSA; this host reports the JIT realization `UNSUPPORTED` because its executable-memory transition is denied. That is recorded as an execution-host limitation, not a semantic or CLIF-lowering failure. See `docs/development-evidence/backend-family-2026-08.md`.

### Forge

Forge may search backend choice, configuration, and pass/backend combinations. It can persist and compare language experiment records. It does **not** decide MNCS semantics, emit missing evidence, or promote a faster candidate that violates protected obligations.

### Family Record / Commons handoff

Language-owned experiment and compiler-study records emit producer-native Family Record references (source/profile, compiler/pipeline, semantic/HIR/SSA/selected-SSA, realization, backend/artifact, corpus, validator, status, unresolved obligations). Broader Commons/Fabric ingestion remains ecosystem-owned.

### Immediate next work

0. **Implemented / exercised (experimental):** Source Profile 0.9 namespace-aware
   binding. Qualified and aliased imports preserve declaring-module identities;
   `source-study` emits inspectable namespaces, scopes, bindings, references,
   provenance, and navigation spans; duplicate unqualified exports fail closed.
   The bounded stdlib consumer is covered by research-bytecode execution.
   Sequence-, view-, vector-, and mask-typed language-level signatures now
   cross the process boundary on all five executable backends through
   canonical cells and packed descriptors, including nested composite
   sequence elements and logical `[bool; N]` windows. The historical generic
   design gate is closed by the bounded Profile 0.10 tranche below, which
   carries type identity, instantiation, recursion, and backend obligations
   through the executable path. Native SIMD selection and executable
   RISC-V/eBPF/PTX
   remain later work.

1. **Implemented / exercised (experimental):** Source Profile 0.10 explicit polymorphism (RFC 0013 Track 1 + Track 12).
   Explicit generic parameters (`<T>`, `<N: Nat>`) on functions; generic calls with explicit arguments (`f<4>`, `f<Point, 4>`);
   deterministic monomorphization with canonical instantiation/specialization identity and call-site rewriting;
   provenance tracked through HIR/SSA/selected-SSA; generic specializations recorded in program/HIR/SSA artifacts.
   Stdlib arity-specific families (`contains4`, `sum4`, `min4`, `max4`, `count4`, `contains_byte4`, `count_nonzero4`,
   `count_true4`, `any_true4`, `all_true4`) delegate to N-parameterized generics (`contains<N>`, `sum<N>`, `min<N>`,
   `max<N>`, `count<N>`, `contains_byte<N>`, `count_nonzero<N>`, `count_true<N>`, `any_true<N>`, `all_true<N>`).
   Geometry stdlib sequence helpers generalized (`first_point<N>`, `contains_any<N>`, `union<N>`) with `*4` wrappers delegating.
   Five executable backends are audited against the generic specialization corpus with per-backend status retained;
   returned-value agreement does not promote unresolved studies to PASS. Negative diagnostics cover missing/mismatched
   args, kind errors, non-constant args, bound errors, recursion/expansion limits, malformed syntax, and unsupported features.

1. ~~Logical product/record values as Source Profile 0.5~~ — **implemented (experimental)** as of
   2026-08-23: nominal records with canonical field identity, construction, functional update,
   projection; differential agreement demonstrated across WASM (forwarding realization) and
   research bytecode; LLVM/C11/Cranelift fail closed. Remaining for maturity: record equality
   semantics, records through bounded-iteration state and block parameters on WASM, scalar-backend
   realizations, CRE coverage that pressures nested records.
2. Cranelift object/external-link fallback for hosts that prohibit JIT executable mappings; the
   present compiler/CLIF path is complete for the declared scalar envelope and reports the host
   refusal without fabricating execution evidence.
3. Extend saturating/widening realization beyond the bounded WASM and reference envelopes while
   retaining capability-gated refusal in LLVM/C11/Cranelift.
4. Generalize proof-carrying promises beyond the completed constant-range `nsw`/`nuw` pilot to
   memory, aliasing, alignment, and target relaxation operations when those semantics exist.
5. Frozen-experiment replication is wired end to end: `experiment execute --baseline` seals a
   replicated result against an identity-verified baseline (fail-closed on corpus/artifact/baseline
   mismatches), and the sibling Control/Fabric/Forge/Commons tooling executes the exact frozen
   realization on one explicitly requested worker and publishes a Family Record replication
   (demonstrated 2026-08 for CRE-1 through WASM and research-bytecode on a remote Linux worker;
   bounded-corpus agreement only, no cross-host equivalence claim).

## 0.1–0.9 milestone reconciliation

Roadmap **0.5 is complete as a bounded experimental milestone.** Completion does not imply a
production compiler, universal equivalence, a general proof kernel, or completion of Source Profile
0.5 maturity work. The acceptance record is in
`docs/development-evidence/roadmap-0.5-2026-08.md`.

## Active cross-cutting track — source representations

The project now maintains an experimental syntax laboratory before committing to a grammar.

Current work:

- compare Zig-influenced, fully structured, and minimal human-source candidates;
- maintain a distinct compact canonical semantic form for agents and tooling;
- represent recursive changes as semantic patches rather than unrestricted text replacement;
- declare equivalent semantic claims before comparing candidate density;
- collect deterministic tokenizer-neutral metrics in CI;
- extend later studies with multiple model tokenizers, parser behavior, repair success, human comprehension, and verifier cost.

A final grammar must not be selected from character count alone.

## Active cross-cutting track — machine-intent expressions

[RFC 0006](rfcs/0006-machine-intent-expressions.md) proposes explicit semantic operations for
machine-oriented behavior that would otherwise depend on source idioms, undefined behavior,
unchecked compiler promises, or target folklore.

Current design work:

- separate exact semantics, intent, preferences, facts, requirements, and obligations;
- define explicit arithmetic edge behavior;
- model evidence-bearing memory, authority, and confinement capabilities;
- preserve closed dispatch, constant-time, layout, and target-specialization intent through IR;
- define lowering and portability envelopes;
- bind backend promises to current evidence;
- retain portable references and conservative fallbacks; and
- allow repeatedly verified non-orthodox techniques to graduate through explicit semantic review.

The first implementation pilot should remain narrow: integer arithmetic, one memory capability,
canonical serialization, IR preservation, and one conservative backend-promise experiment.

## Active cross-cutting track — proof-carrying dependent core

[RFC 0007](rfcs/0007-proof-carrying-dependent-core.md) proposes a small dependent proof core as the
formal trust boundary for machine-generated proofs while keeping general execution, empirical
evidence, automation, and orchestration outside that kernel.

Current design work:

- use intensional dependent type theory as the conservative starting point while studying extensional,
  observational, homotopical, cubical, linear, and quantitative variants selectively;
- keep proof generation and proof checking separate so LLMs, solvers, Forge workflows, and other
  agents can remain untrusted proof producers;
- distinguish kernel-checkable proof terms, checkable certificates, verifier results, empirical
  observations, assumptions, and unresolved obligations;
- derive proof obligations from language constructs instead of relying on authors to remember them;
- preserve `PASS`, `FAIL`, and `UNKNOWN` as method- and scope-specific obligation outcomes;
- model a total logical stratum separately from partial, effectful, concurrent, foreign, and
  target-specific execution;
- make transformation preservation/refinement claims proof-carrying where feasible;
- investigate linear or quantitative usage information for capabilities, resources, regions, and
  authority without prematurely fixing the final calculus; and
- keep the kernel small, deterministic, versioned, and suitable for independent implementations.

The first implementation pilot should be deliberately tiny: a versioned proof-term representation,
a minimal universe/inductive/dependent-function core, deterministic checking and proof identity, and
one automatically generated proof obligation tied to an existing executable or machine-intent
construct. It should include accepted, rejected, and unresolved fixtures before claiming a theorem
prover or verified compiler.

## Active cross-cutting track — assurance, evidence, trust, and witnesses

[RFC 0018](rfcs/0018-machine-native-assurance-evidence-trust-witness-semantics.md) extends the initial
contract/evidence model into an action-relative assurance system over explicit support, refutation,
provenance, assumptions, trust, freshness, independence, conflict, appraisal, and witness structure.

Current design work:

- distinguish `UNSUPPORTED`, `SUPPORTED`, `REFUTED`, and `CONFLICTED` claim dispositions;
- keep verifier `PASS`, `FAIL`, and `UNKNOWN` distinct from claim disposition and final assurance decisions;
- preserve alternative assumption-scoped justification paths instead of flattening evidence into one status;
- model support, refutation, undercutting, supersession, dependency, and invalidation relations;
- investigate provenance-algebra representations for alternative and jointly required evidence paths;
- separate raw evidence from appraisal results and relying-policy decisions;
- scope verifier/appraiser trust by claim class, subject, method, context, role, and validity;
- represent bounded delegation, revocation, witnesses, transparency commitments, and common-mode dependencies;
- require independence claims themselves to be evidenced or explicitly assumed;
- permit explicit Bayesian, belief-function, subjective-opinion, statistical, or other uncertainty models without adopting a universal confidence score;
- express assurance profiles as predicates over evidence graphs rather than a global assurance ladder; and
- return action-relative `AUTHORIZED`, `REJECTED`, or `UNRESOLVED` decisions with machine-readable explanation slices.

The first implementation pilot should remain narrow: versioned claim/evidence/assumption/trust/profile
artifacts, deterministic four-state claim disposition, two action profiles with different assurance
requirements, one shared-dependency independence refusal, one contradictory-evidence fixture, one
freshness invalidation case, and one existing promotion or backend promise gated by an assurance decision.

## Active cross-cutting track — values, data, types, and representation realizations

[RFC 0019](rfcs/0019-machine-native-value-data-type-representation-semantics.md) separates logical
values and logical types from observation interfaces, physical representation realizations, storage,
and serialized encodings so the language can preserve strong semantics while leaving machine layout
open to evidence-bearing realization search.

Current design work:

- establish a compact logical data vocabulary around zero, unit, products, sums, records, variants,
  functions, inductive data, coinductive observation, indexed data, opaque/nominal identities, and
  existential/dynamic packages;
- treat constructors and semantic views as logical structure rather than mandatory field/tag layout;
- preserve an explicit recursive boundary in canonical semantics while allowing surface elaboration to
  hide fold/unfold details where appropriate;
- investigate selective union, intersection, and exclusion descriptions without collapsing nominal or
  generative identity into pure set equality;
- keep dynamic values existentially typed and witness-bearing rather than introducing an unchecked `Any`;
- distinguish logical, view, representation, storage, and serialization observations;
- make representation-sensitive observations such as layout, size, alignment, field order, tag encoding,
  and addressability explicit so they narrow realization freedom visibly;
- represent hard representation requirements separately from preferences and target-dependent facts;
- model candidate representation realizations with validity, constructor-distinction, decode,
  observation-preservation, target-applicability, and evidence obligations;
- preserve logical type identity across legal boxing, unboxing, packing, niche encoding, field
  reordering, AoS/SoA changes, closure conversion, defunctionalization, and similar realization changes;
- permit unresolved representation variables/holes until a phase genuinely requires a concrete realization;
- use RFC 0018 assurance profiles to distinguish experimental representation use from stronger promotion;
- keep allocation, provenance, lifetime, ABI, persistence, serialization, detailed equality, and numeric
  semantics owned by their respective RFCs rather than folding them into the value ontology; and
- expose representation choice as a bounded Forge search dimension whose candidates remain untrusted
  until their semantic and target obligations are discharged.

The first implementation pilot should remain deliberately bounded: a versioned logical datatype schema
for a small product/sum/record/variant subset, separate logical-type and representation identities,
two or three representation families, a small representation-requirement vocabulary, one semantic
view, one niche-encoding validity experiment with conservative fallback, one field-reordering case that
becomes illegal after an explicit representation observation, bounded differential reference execution,
and two RFC 0018 assurance policies for experimental versus stronger representation use.

## Active cross-cutting track — identity, equality, equivalence, refinement, and compatibility relations

[RFC 0020](rfcs/0020-machine-native-identity-equality-equivalence-refinement-substitutability-compatibility-semantics.md)
establishes a typed relation framework so MNCS can state exactly how two values, artifacts, programs,
representations, components, or realizations are related without collapsing every notion of “same”
into one equality or verifier bit.

Current design work:

- distinguish identity, definitional equality, propositional equality, structural equality,
  alpha-equivalence, isomorphism, logical equivalence, behavioral equivalence, refinement,
  substitutability, compatibility, and sound abstraction;
- make observation, context, environment, target, quantified-domain, and direction scopes part of
  relation identity where they affect meaning;
- represent relation claims as proof-relevant semantic artifacts with explicit subjects, assumptions,
  witness references, obligations, evidence, and provenance;
- use logical relations for representation independence and parametric reasoning without making them
  the only equivalence theory;
- use simulation, bisimulation, coinduction, and trace relations as candidate witnesses for lowering,
  stateful, concurrent, reactive, and potentially infinite behavior;
- treat refinement as directional behavior inclusion so valid implementations may reduce nondeterminism
  or remove permitted failure behavior without falsely claiming equality;
- model contextual refinement/substitutability as replacement claims over declared context classes;
- keep compatibility contract-class specific rather than assuming a single symmetric compatibility relation;
- define generalized congruence/rewrite rules so substitution is legal only where an operator or context
  is known to respect the relation;
- distinguish sound abstract interpretation from exact semantic equality;
- introduce bounded agreement as an explicit relation so finite execution corpora cannot be laundered
  into universal equivalence;
- keep counterexamples relation-scoped so a timing mismatch does not automatically refute a narrower
  functional relation;
- preserve relation identities and evidence through composition, invalidation, and Forge relation deltas;
- use RFC 0018 assurance policies to decide which evidenced relation claims authorize experimentation,
  lowering, representation use, linking, or candidate promotion; and
- reserve full-abstraction and stronger secure-compilation relations for boundaries that genuinely need
  preservation and reflection of contextual distinctions.

The first implementation pilot should remain bounded: versioned `RelationDefinition`, `RelationClaim`,
`ObservationModel`, `BoundedAgreementReport`, `Counterexample`, and `RelationDelta` artifacts; at least
two observation profiles; examples where the same subjects satisfy one relation and fail another; one
directional refinement case; one RFC 0019 representation-equivalence case; one scoped counterexample;
and RFC 0018 assurance profiles that refuse to upgrade finite agreement into universal equivalence.

## Active cross-cutting track — numeric semantics, precision, error, and reproducibility

[RFC 0021](rfcs/0021-machine-native-numeric-arithmetic-precision-error-reproducibility-semantics.md)
separates mathematical quantity, arithmetic operation, numerical contract, representation,
algorithmic realization, and evidence so numerical meaning is not reduced to primitive machine formats.

Current design work:

- distinguish exact arithmetic, correctly rounded finite arithmetic, bounded approximation, rigorous
  enclosure, empirical accuracy, and statistical accuracy as different numerical claims;
- represent mathematical domains separately from binary/decimal/fixed/posit-like and accelerator formats;
- make rounding, overflow, underflow, exceptional values, conversion, and fused-operation semantics explicit;
- require algebraic rewrite authority such as reassociation to come from laws valid for the actual domain
  and observation relation rather than operator spelling;
- model absolute, relative, ULP, forward, backward, interval, ball, and probabilistic error claims without
  flattening them into one confidence score;
- separate bitwise, order, schedule, backend, target, numerical, and statistical reproducibility;
- decompose fast-math behavior into individual semantic permissions;
- support mixed precision, exact/compensated accumulation, rigorous interval/ball realization, and
  alternative number systems as candidate strategies rather than language-wide commitments;
- expose precision, representation, accumulator, and algorithm choice as bounded Forge search dimensions;
- connect numeric preservation/refinement to RFC 0020 relation claims and promotion authority to RFC 0018; and
- preserve exact-real computability/termination questions for RFC 0022 rather than making false totality promises.

The first implementation pilot should remain bounded: a small exact/finite numeric-domain set, explicit
rounding/overflow contracts, several error models and reproducibility profiles, a reference evaluator,
reassociation and FMA counterexamples, order-sensitive reduction, one compensated/wider or exact
accumulation experiment, bounded precision search, RFC 0020 numeric relation claims, and RFC 0018
policies that distinguish experimental numerical evidence from stronger promotion authority.

## Active cross-cutting track — termination, productivity, progress, liveness, and bounded computation

[RFC 0022](rfcs/0022-machine-native-termination-productivity-progress-liveness-bounded-computation-semantics.md)
establishes completion/progress semantics that distinguish totality, divergence, productivity,
local progress, concurrent progress, temporal liveness, fairness, quiescence, stabilization, and
bounded execution rather than treating all of them as one `terminates` bit.

Current design work:

- preserve `TERMINATES`, `DIVERGES`, and `UNRESOLVED` as distinct scoped conclusions;
- make `BUDGET_EXHAUSTED` a bounded noncompletion observation that can never by itself prove divergence;
- distinguish total and explicitly partial computation rather than hiding nontermination inside ordinary values;
- support structural descent, well-founded measures, ranking functions, lexicographic/multicomponent
  rankings, size-change certificates, and sized indices as complementary termination-witness families;
- keep proof-kernel strong normalization separate from ordinary program termination;
- distinguish may-, must-, and almost-sure termination under nondeterministic/probabilistic execution;
- model guarded/coinductive productivity for intentionally nonterminating streams and services;
- distinguish local non-stuck progress from eventual temporal progress;
- represent deadlock-free, starvation-free, obstruction-free, lock-free, and wait-free concurrent
  progress as separate claims over RFC 0010 execution models;
- represent eventuality/leads-to properties with explicit weak/strong fairness and runtime assumptions;
- reserve quiescence and stabilization for persistent/open systems that need not terminate;
- distinguish proof step indices from runtime fuel and abstract computation budgets from RFC 0023 clocks/deadlines;
- make termination/progress/liveness claims proof-relevant, dependency-aware, counterexample-scoped, and
  compatible with RFC 0018 assurance and RFC 0020 relations; and
- expose finite execution authority as a safety mechanism for untrusted Forge candidates without laundering
  successful testing into universal termination evidence.

The first implementation pilot should include versioned termination/ranking/budget/productivity/fairness
artifacts; structural and ranking-function termination examples; an intentional divergent computation
that produces only `BUDGET_EXHAUSTED` under fuel; a finite computation that exhausts a small budget but
returns under a larger one; a productive nonterminating producer; a may-versus-must termination fixture;
a fairness-dependent liveness model; and RFC 0018 policies separating sandbox authority from promotion.

## Active cross-cutting track — time, clocks, deadlines, temporal validity, and real-time realizations

[RFC 0023](rfcs/0023-machine-native-time-clock-deadline-temporal-validity-real-time-semantics.md)
establishes a temporal model that keeps causal/logical order, physical clock observation, monotonic
elapsed time, reference/civil time, event/processing time, temporal validity, deadlines, schedules,
and bounded computation distinct rather than collapsing them into one timestamp primitive.

Current design work:

- require every timestamp/instant observation to identify its clock or temporal domain rather than
  providing one ambient universal `Timestamp`;
- preserve causal order from RFC 0010 separately from physical timestamp order and forbid timestamp
  sorting from fabricating causal relationships;
- represent physical clock observations with source identity, time scale, synchronization state,
  resolution, uncertainty, stability/drift information, and evidence where required;
- distinguish monotonic clocks, synchronized reference clocks, civil representations, logical/vector/
  hybrid clocks, virtual time, simulation time, replay time, event time, ingestion time, and processing time;
- represent explicit clock mappings with offset/rate/uncertainty/validity evidence before comparing
  readings from different clock domains;
- distinguish instants, durations, temporal intervals, and interval relations with explicit boundary semantics;
- separate observation/creation/verification time from valid/effective/expiration and knowledge/record time;
- derive temporal applicability/freshness using validity intervals and clock uncertainty, preserving
  `UNRESOLVED` when uncertainty overlaps a validity boundary;
- distinguish deadlines from timeout reaction policies and periods from deadlines, with separate release,
  minimum-interarrival, jitter, and miss-semantics contracts;
- support timed-state-machine and metric-temporal constraints, including bounded eventuality and explicit
  non-Zeno/time-divergence obligations where required;
- separate logical effect time from physical execution time so schedules can vary within evidenced temporal envelopes;
- treat schedules as target-specific realizations and support untrusted schedule synthesis followed by
  narrow schedule-witness verification under explicit execution-time/workload/platform assumptions;
- consume target/runtime clock and scheduler facts from RFC 0017 and assurance-gate clock, WCET, schedule,
  synchronization, and freshness claims through RFC 0018;
- expose clock reads/timers as RFC 0008 effects rather than ambient pure operations;
- support simulation/replay clocks and event-time frontiers so testing/dataflow systems can preserve
  temporal semantics without following wall-clock execution rate;
- treat watermarks/frontiers as epistemic temporal-progress claims rather than clock readings;
- connect temporal equivalence/refinement/counterexamples to RFC 0020 relation scope; and
- expose schedule, core assignment, preemption, clock realization, and communication windows as bounded
  Forge realization dimensions while keeping protected deadlines/jitter/order constraints explicit.

The first implementation pilot should remain bounded: versioned clock-domain/observation/mapping,
instant/duration/interval, deadline/timeout/period/jitter, timed-constraint, schedule-witness, and
temporal-relation artifacts; definite versus unresolved ordering under clock uncertainty; monotonic
versus civil-time separation; deadline-versus-timeout and period-versus-deadline fixtures; a small timed
state machine; one valid and one invalid schedule witness; two physical schedules preserving one logical
execution-time contract; a simulation-clock run; and an RFC 0018 freshness example where clock
uncertainty overlapping expiration produces `UNRESOLVED`.

## Active cross-cutting track — information flow, confidentiality, integrity, and side-channel semantics

[RFC 0024](rfcs/0024-machine-native-information-flow-confidentiality-integrity-declassification-side-channel-semantics.md)
establishes information-flow security as explicit policy over permitted influence, permitted
observation, bounded reclassification, observer/channel models, and evidence rather than reducing
security to access checks, taint bits, or a universal `secure` flag.

Current design work:

- distinguish access authority from information-flow authority so permission to read/write a resource
  does not silently authorize dissemination of information derived from it;
- model confidentiality and integrity as orthogonal policy dimensions with explicit flow ordering and
  composition rather than one scalar security level;
- support decentralized/owner-scoped flow policies while leaving principal authentication and credential
  semantics to RFC 0025;
- preserve data, control, effect, failure, termination, timing, scheduling, and declared resource influence
  as potential information channels rather than tracking only explicit assignments;
- make observer and channel models part of security-claim identity so value-only, termination-sensitive,
  timing-sensitive, address-trace, scheduler-sensitive, and physical-channel claims cannot be conflated;
- use RFC 0020 relational/hyperproperty-style claims for noninterference, observational determinism,
  security refinement, and scoped security counterexamples;
- model declassification by explicit `what`, `who`, `where`, and `when` dimensions with narrow release
  functions/relations rather than unchecked secret-to-public casts;
- model endorsement as a separate integrity reclassification and keep validation evidence distinct from
  authority to raise integrity;
- reserve robust declassification, transparent endorsement, and nonmalleable-flow profiles for boundaries
  where attacker influence and reclassification interact;
- allow static IFC, dynamic/floating-label IFC, hybrid enforcement, secure multi-execution, isolation,
  relational verification, and proof-carrying techniques as competing realizations of the same policy;
- treat side channels as explicit observer channels, decomposing constant-time claims into named branch,
  address, instruction, size, timing, schedule, resource, or target-specific observation restrictions;
- forbid encryption, hashing, compression, aggregation, or successful testing from silently granting
  declassification authority;
- support metric-scoped quantitative leakage claims without adopting one universal leakage score;
- preserve security labels, control influence, release events, endorsement events, and observer scope
  through HIR/SSA/backend lowering where the supported subset allows it;
- make target/backend/microarchitecture dependencies explicit so stale side-channel evidence invalidates
  narrowly through RFC 0018 rather than becoming permanent source truth;
- require machine inference to be conservative: it may strengthen confidentiality, weaken assumed integrity,
  add observer channels, or add obligations, but may not fabricate downgrade/endorsement authority;
- expose security relation deltas and counterexamples to Forge recursive refinement; and
- permit Forge to search algorithms, branch structures, lookup strategies, representations, padding,
  scheduling, isolation, and other realizations while mandatory security relations remain protected.

The first implementation pilot should remain bounded: versioned flow-label/confidentiality/integrity,
observer/channel, reclassification, and security-counterexample artifacts; direct and implicit-flow
rejections; one low-integrity-to-high-integrity rejection; narrow predicate declassification and an
over-broad/attacker-controlled release rejection; validation-versus-endorsement fixtures; value-only,
termination-sensitive, and branch/address observer profiles; paired bounded executions that remain
explicitly empirical; an address-trace comparison between direct lookup and oblivious scan; RFC 0020
security relation claims where the same subject passes one observer profile and fails another; stale
target-dependent security evidence; and RFC 0018 policies separating sandbox experimentation from
stronger security-boundary promotion.

## Active cross-cutting track — principals, identity, credentials, delegation, and attestation

[RFC 0025](rfcs/0025-machine-native-principal-identity-authentication-credential-attestation-cryptographic-trust-semantics.md)
establishes principal identity and cryptographic trust as a graph of scoped bindings, authentication
events, credentials, delegation paths, trust-domain relations, and attestation evidence rather than a
username, certificate, key, account, or universal `trusted` bit.

Current design work:

- distinguish principal identity, names/namespaces, key/authenticator identity, credentials, workload,
  device, service, agent, code-artifact, platform, execution-instance, and session identity;
- distinguish identity proofing, enrollment, current authentication, credential cryptographic validity,
  claim acceptance, authorization, delegation authority, attestation, trustworthiness, and trust;
- support local/scoped names and explicit trust domains so equal strings do not fabricate principal identity;
- model principal↔authenticator bindings with key usage, provenance, context, validity, rotation, compromise,
  recovery, and algorithm agility rather than equating a principal with one permanent key;
- distinguish bearer credentials from proof-of-possession credentials and make audience/purpose/context
  bindings explicit;
- make issuer authority claim-class specific so authentic issuance cannot silently establish claim truth;
- preserve credential expiration, revocation, compromise, and `UNKNOWN` status as distinct evidence states;
- represent authority scopes, delegation rights, attenuation, caveats, delegation provenance, and
  threshold principals so delegated authority cannot silently broaden;
- treat long-lived semantic identity and short-lived credential realizations separately, including
  SPIFFE-like workload identity and ephemeral/keyless signing directions;
- model trust anchors as scoped policy roots and federation as explicit claim translation between trust
  domains rather than assuming global trust transitivity;
- align attestation with the RATS Attester → Evidence → Verifier → Attestation Result → Relying Policy
  pipeline while keeping measurements, reference values, attestation endorsements, appraisal, and final
  action authorization distinct;
- support nonce-, time-, epoch-, and session-based freshness without requiring one universal mechanism;
- preserve layered/composite attestation dependencies from hardware/firmware/runtime through workloads and agents;
- keep authentic attestation distinct from acceptable measured state and trustworthiness distinct from
  relying trust decisions;
- apply RFC 0024 information-flow policy to identity, credential, pseudonym-linkage, and attestation evidence;
- expose signing/authentication/attestation/key-management operations as RFC 0008 effects and foreign
  credential translation through RFC 0015 trust boundaries;
- preserve identity/trust/authority deltas through recursive refinement, with authority broadening made
  explicitly reviewable; and
- allow Forge to search authentication, credential, delegation, and attestation realizations while
  protected least-authority, freshness, identity, and state requirements remain explicit.

The first implementation pilot should remain bounded: versioned principal/name/trust-domain,
principal-authenticator binding, authentication, credential/status/issuer-authority, delegation/caveat,
trust-anchor/federation, and attestation/reference-value artifacts; a key-rotation case preserving principal
identity; valid-signature-but-unauthorized and bearer-versus-PoP fixtures; accepted attenuation and rejected
privilege escalation; caveat accumulation; `UNKNOWN` credential status; rejected foreign claim translation;
authentic acceptable and authentic-but-unacceptable attestation cases; stale attestation; agent lineage
without authority inheritance; and RFC 0018 policies separating verifier, mutation, and promotion authority.

## Active cross-cutting track — persistent state, transactions, durability, and crash recovery

[RFC 0026](rfcs/0026-machine-native-persistent-state-durability-transaction-snapshot-journal-crash-consistency-semantics.md)
establishes persistent-state semantics that keep logical state, transaction order, visibility, acknowledgement,
durability, persistence ordering, snapshots, journals, crash states, and recovered state distinct while
allowing storage mechanisms to remain evidence-bearing realization choices.

Current design work:

- distinguish version-persistent logical data from durable storage so retained old versions do not imply
  power-loss survival;
- require durability claims to name explicit failure models and persistence domains rather than a universal
  `durable` bit;
- separate RFC 0010 memory visibility/order from persistence visibility/order and represent explicit
  `PersistBefore` obligations where crash safety depends on write persistence order;
- decompose ambiguous `commit` semantics into serialization, visibility, durability, acknowledgement, and
  recovery-inclusion points that may coincide only under an explicit contract;
- keep concurrency atomicity, failure atomicity, isolation, durability, and application-invariant preservation
  as separate claims;
- distinguish snapshot isolation, serializability, strict serializability, opacity, durable linearizability,
  and buffered durable linearizability rather than treating transactions as one `ACID` profile;
- treat MVCC, locking, optimistic validation, WAL, undo/redo logging, ARIES-style recovery, copy-on-write,
  shadow paging, persistent-memory transactions, and log-structured storage as realization families rather
  than language-level meanings;
- model recovery journals, logical/physical recovery records, journal positions, checkpoints, snapshots,
  persistent roots, generations, and point-in-time recovery as explicit state-derivation artifacts;
- keep recovery journals distinct from event histories and audit logs, checkpoints distinct from snapshots,
  and snapshots distinct from independent backups;
- represent crash contracts with normal postconditions, crash conditions, recovery procedures, recovery
  postconditions, failure models, witnesses, and evidence;
- permit crash/recovery proof systems in the Crash Hoare Logic / concurrent crash-logic family without
  forcing one theorem prover into the language core;
- require recovery itself to remain failure-aware so a crash during recovery does not become an unstated
  trusted assumption;
- represent persistent references and durable object identities separately from process virtual addresses,
  with generation-sensitive validity after rollback/recovery;
- keep logical deletion separate from physical erasure and expose snapshot/journal retention as potential
  erasure blockers under RFC 0024;
- preserve persistence obligations through HIR/SSA/backend lowering so functional equivalence cannot erase
  flush/order/journal/root-switch obligations;
- expose persist-order checking, bounded crash-schedule exploration, recovery-relation checking, and
  transaction-history checking as candidate micro-verifiers whose evidence remains method-scoped;
- make persistence evidence target-sensitive through RFC 0017 and assurance-gated through RFC 0018;
- expose durability/crash/recovery deltas to Forge recursive refinement; and
- leave replicated durability, consensus, partitions, distributed logs, and cross-node transactions to
  RFC 0028 rather than extending local transaction atomicity over arbitrary remote effects.

The first implementation pilot should remain bounded: versioned state/transaction/failure-model/
persistence-domain, persist-order, durability/acknowledgement, journal/snapshot/checkpoint/root/generation,
crash-contract/recovery-result, counterexample, and persistence-delta artifacts; `Visible != Durable` and
ack-before-durability fixtures; a two-field failure-atomic update; WAL ordering; snapshot-isolation versus
serializability; a copy-on-write root-switch example; checkpointed replay; a crash-during-recovery case; a
stale persistent-reference case; a logical-delete-versus-snapshot-erasure case; and an RFC 0020 study where
functional behavior agrees while one candidate fails the required crash relation. The first Forge study
should compare undo-journal, redo-WAL, and copy-on-write realizations and reject any faster candidate that
violates a protected persistence relation.

## Active cross-cutting track — serialization, canonical encoding, schemas, wire contracts, and data evolution

[RFC 0027](rfcs/0027-machine-native-serialization-canonical-encoding-schema-wire-contract-data-evolution-semantics.md)
establishes serialization as an explicit relation among logical values/types, schemas, wire contracts,
encoding/canonicalization profiles, framed representations, and migrations rather than reducing data
interchange to an ambient `encode(value) -> bytes` convention.

Current design work:

- separate logical value/type, semantic schema, wire schema/contract, encoding profile, canonical profile,
  framing, and encoded artifact identities;
- model encode/decode as potentially partial relations and distinguish value round-trip, raw byte
  round-trip, canonical round-trip, and cross-version round-trip claims;
- distinguish malformed, well-formed, format-valid, schema-valid, and contract-valid inputs so parser
  success cannot silently establish semantic validity;
- require canonical encoding claims to name an explicit profile/version and keep canonical byte identity
  separate from semantic identity;
- distinguish raw-encoding, canonical-encoding, schema, and semantic-projection digests so a bare content
  hash cannot fabricate an identity class;
- keep exact-byte signing scope separate from signing a canonical semantic encoding under RFC 0025;
- make duplicate-key/field behavior explicit and reject implementation-dependent interpretation in strict
  profiles;
- separate field/variant semantic identity, wire identity, display name, aliases, deprecation, and reserved
  historical tombstones; retired identifiers may not silently acquire new meaning;
- distinguish `Absent`, `Present(Default)`, and `Present(Null)` unless a declared wire contract intentionally
  collapses them;
- represent unknown data explicitly and distinguish `RejectUnknown`, `IgnoreUnknown`, `PreserveUnknown`,
  and preserve-but-do-not-authorize behavior;
- distinguish reading newer data from preserving/re-emitting newer data, including bridges that parse
  successfully while losing unknown information;
- support explicit open/closed record and variant semantics rather than assuming all schemas are extensible;
- model compatibility directionally and distinguish wire readability, semantic compatibility,
  unknown-preservation, rewrite preservation, and action-relative store/forward/display/execute authority;
- model writer-schema/reader-schema resolution explicitly where formats depend on schema resolution;
- represent schema evolution as a graph of typed migrations rather than a scalar `version++`, including
  rename/replace/split/merge/add/remove/deprecate/reserve/widen/narrow/refine deltas;
- distinguish total, partial, lossless, lossy, reversible, and refining migrations and make migration losses
  machine-readable;
- require migration-path coherence evidence where multiple supported paths should reach equivalent target
  semantics, and require direct migration optimizations to preserve the declared historical relation;
- treat mixed-version data populations and rolling coexistence as normal rather than assuming lockstep upgrades;
- preserve RFC 0021 numeric conversion obligations, RFC 0023 temporal meaning, and RFC 0024 information-flow
  labels/observer channels across serialization;
- keep serialization, compression, encryption/authentication, framing, and transport as separate transforms;
- expose bounded decoder depth/size/element authority and keep decode-budget exhaustion distinct from
  malformed input under RFC 0022;
- support whole-buffer, streaming, zero-copy, event, and projection decoders as competing realizations with
  explicit validation coverage;
- preserve schema/wire/canonical/migration identities through HIR/SSA/backend lowering and foreign codec
  boundaries; and
- allow Forge to search text, tagged binary, deterministic CBOR-like, protobuf-like, Avro-like, fixed-layout,
  zero-copy, and custom generated realizations while mandatory round-trip/canonical/compatibility/migration
  relations remain protected.

The first implementation pilot should remain bounded: versioned schema/version/field/reserved-field,
wire-contract/encoding/canonical-profile, presence/unknown/duplicate-policy, migration/compatibility/
round-trip/counterexample/delta artifacts; value and canonical round-trip fixtures; legal-but-noncanonical
bytes; safe rename versus illegal field-ID reuse; explicit absence/default distinction; unknown-field
forwarding and a lossy bridge; backward-only compatibility; wire-compatible but semantically narrowed
schemas; a migration diamond with coherent and incoherent paths; canonical-digest scope; one RFC 0021
numeric conversion case; bounded decode exhaustion; projection decode coverage; and RFC 0018 policies that
separate storing/forwarding an unknown future artifact from executing it. The first Forge study should
compare several wire realizations and reject any faster/smaller candidate that violates a protected
canonicality, unknown-preservation, presence, numeric, compatibility, or migration relation.

## Active cross-cutting track — distribution, messaging, partial failure, consistency, replication, and agreement

[RFC 0028](rfcs/0028-machine-native-distribution-messaging-partial-failure-consistency-replication-consensus-failure-detector-semantics.md)
establishes distribution as explicit semantics over participants, messages, remote-operation knowledge,
network/failure models, replica histories, consistency, convergence, quorum, agreement, coordination, and
global state rather than treating remote execution as an ordinary local call with latency.

Current design work:

- distinguish participant, principal, process, replica, logical operation, invocation attempt, message,
  configuration, and observer identities;
- model message lifecycle stages such as send, receive, delivery, application, persistence, acknowledgement,
  and sender observation separately so no stage fabricates the next;
- distinguish at-most/at-least-once delivery from exactly-once logical or durable effect, preserving retry,
  idempotency, and durable deduplication as different mechanisms;
- preserve observer-relative `OUTCOME_UNKNOWN` when a remote effect may have occurred but its response is lost;
- make synchronous, asynchronous, and partially synchronous network assumptions explicit and treat FLP-style
  impossibility boundaries as obligation generators rather than hidden implementation assumptions;
- distinguish crash, omission, communication partition, suspicion, and Byzantine fault models;
- model failure detectors as evidence-producing mechanisms with explicit completeness/accuracy/network
  assumptions so timeout observations never silently become failure facts;
- distinguish reliable, causal, total-order, atomic, and Byzantine reliable broadcast relations;
- scope membership, leader, quorum, consensus, and replicated-state claims to explicit configuration epochs;
- model fencing epochs and leases as bounded distributed authority, reusing RFC 0023 clocks and RFC 0025
  principal/authority semantics;
- represent quorum systems as subset predicates with independently checkable intersection obligations rather
  than inferring safety from replica counts;
- distinguish linearizability, sequential consistency, causal consistency, session guarantees, and eventual
  convergence through RFC 0020 relation claims;
- keep convergence distinct from invariant preservation and make conflict-resolution policy part of logical
  data semantics rather than replication housekeeping;
- support state- and operation-based CRDT realization families with machine-checkable merge/commutativity laws;
- use CALM-style monotonicity and invariant-confluence reasoning to generate coordination requirements from
  semantics rather than requiring programmers to choose consensus first;
- interpret CAP only through explicit consistency, availability, operation, and partition predicates;
- model consensus as agreement/validity/safety/liveness semantics while treating Paxos/Raft/Viewstamped and
  related protocols as realization families;
- keep consensus safety and liveness separate, with liveness evidence scoped to synchrony, failure-detector,
  quorum, fairness, or randomization assumptions;
- distinguish authenticated participant identity from honesty under Byzantine models and keep fault thresholds
  scoped to configuration, authentication, network, quorum, and independence assumptions;
- represent distributed snapshots as consistent global cuts distinct from RFC 0026 durable snapshots;
- distinguish local transactions, distributed atomic commit, blocking 2PC-style realizations, consensus-backed
  commit, sagas, and compensation; compensation may restore a declared business relation without erasing history;
- compose RFC 0026 local durability into explicit replicated/distributed durability and acknowledgement claims;
- preserve distribution obligations through HIR/SSA/backend lowering and expose distribution deltas to Forge;
- expose quorum, consistency-history, failure-model, CRDT-law, coordination, reconfiguration, fencing,
  distributed-snapshot, and atomic-commit checks as narrow micro-verifier opportunities; and
- leave physical placement, topology, locality, mobility, and heterogeneous node assignment primarily to RFC 0029.

The first implementation pilot should remain bounded: versioned participant/configuration/epoch, channel/message,
logical-operation/attempt, network-model/failure-observation/failure-detector, replica/quorum, consistency,
consensus, coordination-requirement, counterexample, and distribution-delta artifacts; `Sent != Applied`,
applied-but-caller-unknown, retry/deduplication, timeout-versus-suspicion, FLP-boundary, valid/invalid quorum,
convergent-versus-linearizable, session-guarantee, CRDT merge-law, convergence-versus-invariant, coordination
necessity, stale-leader fencing, distributed-snapshot, 2PC safety-versus-liveness, and saga-compensation fixtures.
The first Forge studies should demonstrate both a coordination-free realization selected when algebra/invariants
permit it and a globally constrained operation that either generates coordination or is reported unrealizable
under the requested partition/availability model.

## Active cross-cutting track — placement, topology, locality, mobility, and heterogeneous execution

[RFC 0029](rfcs/0029-machine-native-placement-topology-locality-mobility-migration-heterogeneous-execution-semantics.md)
establishes a spatial semantic model that separates logical identity from physical location, logical topology
from physical topology, computation placement from data placement, hard placement requirements from preferences,
and mobility/migration semantics from deployment/version change. Placement remains an unresolved realization
choice until a contract or observation makes spatial relationships semantically relevant.

Current design work:

- distinguish logical identity, place identity, virtual-place identity, execution-slot identity, physical-resource
  identity, topology identity, physical-instance identity, and placement identity;
- model physical machines as evidence-bearing resource/topology graphs rather than assuming one static hierarchy,
  while retaining containment, accessibility, transfer, sharing, trust, and fault-domain relations;
- keep logical/application topology separate from physical topology and represent mapping as an explicit
  proof-bearing relation;
- prohibit identifier adjacency from fabricating proximity and require locality claims to name an actual
  relation/domain rather than a universal `local` bit;
- separate `PlacementRequirement` from `PlacementPreference` so optimization can trade preferences but cannot
  override hard semantic constraints;
- model affinity, anti-affinity, colocation, direct-access, communication-boundary, trust-domain, and fault-domain
  constraints explicitly;
- provide spatial evidence for RFC 0028 replica independence so replica count cannot substitute for actual
  separation under the declared failure model;
- allow location variables/unresolved placement holes and explicit `SAT`, `UNSAT`, and `UNKNOWN` placement
  conclusions;
- separate computation placement, logical data identity, physical data instances, and memory-resource placement;
- use capability-based heterogeneous resource requirements rather than assuming CPU/GPU/device-class labels
  establish required execution semantics;
- bind device realizations to RFC 0019 representation, RFC 0021 numeric, RFC 0023 timing, RFC 0024 security,
  RFC 0025 trust/attestation, RFC 0026 persistence, RFC 0027 serialization, and RFC 0028 distribution obligations;
- allow placement to generate explicit serialization/distribution obligations when a realization crosses
  address-space, device, process, node, or independent-failure boundaries;
- keep static, dynamic, and adaptive placement legal while treating work stealing, overdecomposition, graph/
  hypergraph partitioning, HEFT-like mapping, learned mappers, and runtime heuristics as proposal strategies;
- model mobility boundaries, migration units, migration closure, transfer, cutover, destination activation,
  source retirement, capability rebinding, rollback, and `UNKNOWN` migration outcome explicitly;
- preserve logical identity across valid movement while keeping migration transparency relation- and
  observation-scoped;
- require migration to surface stale or invalidated topology, target, attestation, clock, schedule, persistent-
  reference, locality, side-channel, and fault-domain evidence;
- prevent migration-induced split brain through explicit cutover authority/fencing when one-writer semantics
  are required;
- preserve placement/migration obligations through HIR/SSA/backend lowering and expose topology, placement,
  and migration deltas to Forge recursive refinement;
- separate untrusted placement search from narrow deterministic checking of hard resource, capability,
  locality, separation, trust, boundary, and migration constraints; and
- leave quantitative placement quality, latency, bandwidth, energy, migration cost, and Pareto ranking to
  RFCs 0032–0033 after semantic admissibility is established.

The first implementation pilot should remain bounded: versioned place/topology/resource/capability,
location-variable, computation/data-placement, requirement/preference, affinity/anti-affinity/fault-domain,
mobility/migration/rebinding, witness/counterexample/delta artifacts; identity-survives-movement and
placement-observation fixtures; preference-versus-requirement; topology-ID-versus-proximity; replica
fault-domain separation; capability-based CPU/GPU admissibility; unresolved placement holes; move-task-
versus-move-data; migration-attestation invalidation; capability rebinding; distributed-boundary crossing;
communication-free placement; fenced migration cutover; stale topology evidence; numeric-invalid accelerator;
and placement-generated serialization obligations. The first Forge studies should compare heterogeneous
placement candidates and reject any faster candidate that violates protected numeric, trust, memory,
fault-domain, security, serialization, distribution, or migration relations.

## Active cross-cutting track — deployment, lifecycle, live upgrade, version coexistence, rollback, and retirement

[RFC 0030](rfcs/0030-machine-native-deployment-lifecycle-live-upgrade-version-coexistence-state-migration-rollback-retirement-semantics.md)
establishes software lifecycle change as an explicit transition through mixed-version system configurations
rather than treating deployment as replacement of one version string with another. Endpoint correctness is
insufficient: allowed intermediate states, coexistence, state migration, version consistency, activation,
cutover, rollback, finalization, and retirement must be represented and checked.

Current design work:

- distinguish release/artifact version, deployment generation, execution-instance identity, component version,
  protocol/schema/configuration version, feature state, and lifecycle configuration;
- distinguish installed, loaded, eligible, active, serving, authoritative, draining, finalized, and retired
  states rather than collapsing lifecycle into a single `deployed` bit;
- model `LifecycleTransition` and lifecycle traces with explicit allowed/forbidden intermediate configurations;
- keep all-old/all-new endpoint validity separate from mixed-version transition validity;
- represent directional runtime compatibility matrices/envelopes and derive partial-order upgrade dependencies;
- permit explicit bridge versions/compatibility shims when direct rolling coexistence is unsatisfiable;
- model update points, quiescence conditions, active-code transition, and request/transaction/session/
  continuation version consistency without requiring global quiescence;
- bind lifecycle phases to RFC 0027 typed state migrations and RFC 0026 crash-safe durable migration;
- represent eager, lazy, background, virtual, dual-read, and dual-write migration as realization families while
  preserving state authority/read/write semantics explicitly;
- treat rolling, blue/green, canary, shadow, hot-update, bridge-release, and all-at-once approaches as candidate
  lifecycle realizations rather than language primitives;
- model canary/shadow exposure scopes and preserve bounded observations as scoped RFC 0018 evidence;
- require shadow execution to carry explicit effect authority so non-authoritative output cannot duplicate
  destructive effects;
- distinguish code/feature activation, traffic authority, state authority, cutover, finalization, and retirement;
- make finalization an explicit boundary that can retire compatibility or enable irreversible migration/features;
- model irreversibility explicitly and surface every rollback path narrowed or destroyed by finalization;
- model rollback as a separately checked forward transition, keeping code rollback, state rollback, snapshot
  restore, compensation, and roll-forward repair distinct;
- keep retirement granular across executable, protocol, schema-reader/writer, state representation, bridge,
  feature, credential, and migration artifacts and require absence-of-dependency evidence;
- treat lifecycle orchestration itself as failure-prone persistent/distributed computation with explicit
  `UNKNOWN` remote-step outcomes, reconciliation, and retry-safety requirements;
- consume RFC 0028 quorum/membership semantics and RFC 0029 fault-domain placement so rollout batches cannot
  silently destroy distributed safety or independence;
- separate untrusted lifecycle-plan synthesis from narrow deterministic checking of compatibility, version
  consistency, migration, authority, rollback, finalization, retirement, quorum, and topology obligations; and
- leave rollout latency, resource cost, blast-radius metrics, energy, and Pareto ranking to RFCs 0032–0033.

The first implementation pilot should remain bounded: versioned release/version/deployment-generation,
lifecycle-configuration/transition/phase, population/compatibility/dependency, update-point/version-consistency,
state-migration-phase, feature/exposure, finalization/rollback/retirement, witness/counterexample/delta artifacts;
valid-endpoints-invalid-transition, transactional-version-consistency, generated-upgrade-order, bridge-cycle,
scoped-canary, shadow-effect-isolation, full-traffic-not-finalized, rollback-loss-after-finalization, code-rollback-
state-incompatibility, old-session-blocks-retirement, crash-during-migration, unknown-remote-step, and topology/
quorum rollout fixtures. The first Forge studies should compare all-at-once, rolling, bridge, canary, blue/green,
and hot-update candidates and reject any easier/faster lifecycle realization whose intermediate states violate a
protected compatibility, state, authority, version-consistency, quorum, placement, rollback, or retirement relation.

## 0.1 — Executable semantic model

**Status: Implemented / exercised.** Acceptance items below are satisfied by `mncs-model` validation, diagnostics, fixtures, and CI. This is the executable semantic foundation, not a source language.

**Goal:** demonstrate that MNCS relationships can be represented and mechanically checked before a source grammar exists.

Acceptance criteria:

- represent modules, functions, contracts, effects, capabilities, assumptions, evidence, and failure modes;
- reject an effect that lacks a declared authorizing capability;
- reject evidence that references an unknown property;
- report contracts that have no evidence;
- emit deterministic, machine-readable diagnostics;
- include accepted and intentionally rejected examples;
- document trust boundaries, recursive-refinement requirements, machine-intent direction, and non-goals;
- run formatting, linting, tests, example validation, and syntax measurements in CI.

## 0.2 — Semantic graph, identity, and recursive artifacts

**Status: Implemented but partial.** Identities, graph, evidence manifests, slices, patches, and promotion records exist. Causal slices are conservative; promotion is an explicit record, not automatic mutation.

Initial executable foundation delivered in the Rust model and CLI:

- deterministic 0.2 canonical JSON and SHA-256 fingerprints;
- structural identities for programs, functions, contracts, assumptions, effects, capabilities, and evidence claims;
- typed semantic graph nodes and relationship edges;
- conservative dependency-aware evidence invalidation;
- executable evidence manifests with dependency fingerprints and stale assessment;
- CLI commands for canonicalization, identities, graph emission, evidence manifests, diffs, and freshness checks; and
- a narrow machine-intent pilot preserving explicit operation and obligation identities;
- executable diagnostic obligations and conservative graph-based causal slices;
- finite refinement budgets, untrusted isolated repair proposals, and explicit promotion decisions;
- semantic, authority, and evidence delta records with explicit authority-broadening flags; and
- distinct observation, verification, mutation, generation, and promotion capabilities;
- typed semantic patches, protected-property evaluations, realization/portability envelopes, and
  transformation provenance; and
- versioned verifier request/result artifacts with exact subject, scope, dependency, freshness, and
  verifier-independence checks.

The implemented recursive artifacts are intentionally partial: causal slices are conservative
backward graph slices rather than formal minimal slices, repair application supports a small typed
patch vocabulary, realization selection is envelope-only, and promotion remains an explicit record
rather than an automatic mutation. Control/data-flow-complete refinement and target-specific
realizations remain future work.

- define stable identities for components, properties, assumptions, verifiers, artifacts, diagnostics, transformations, machine-intent operations, obligations, references, and realizations;
- model dependency and invalidation edges;
- define canonical serialization and content hashing;
- distinguish source identity, semantic identity, implementation identity, candidate identity, realization identity, and evidence identity;
- produce a first evidence-manifest schema;
- define schemas for diagnostic obligations, causal slices, repair proposals, refinement budgets, semantic deltas, evidence deltas, and promotion decisions;
- define facts, preferences, requirements, lowering envelopes, and portability envelopes as distinct semantic objects;
- ensure observation and mutation require distinct capabilities;
- version the canonical semantic representation independently from human syntax.

The first executable subset of these artifacts is in `mncs-model::refinement` and
`mncs-model::delta`; the JSON shapes are versioned `0.2` research artifacts, not an accepted RFC.

## 0.3 — High-level MNCS IR

**Status: Implemented but partial.** Deterministic HIR, bodies, CFG, obligations, and traceability exist for the supported subset. Complete lowering correctness is not claimed.

Early executable foundation now present for the supported 0.1 manifest subset:

- deterministic `0.3` high-level IR serialization and SHA-256 fingerprints;
- typed values, explicit state regions, normal/error/compensation/fatal blocks, and transitions;
- effect operations with explicit capability uses;
- contract, assumption, evidence, and semantic graph traceability;
- generated obligations carried by the IR;
- executable semantic bodies with symbolic constants/integer/effect/runtime-check operations;
- direct body-to-HIR lowering with machine-intent links and transformation provenance;
- versioned lowering and portability envelopes with conservative realization selection; and
- a pure verifier request/result protocol with a deterministic capability-consistency verifier and
  import validation for external artifacts;
- conservative diagnostic output connected to generated obligations.

This remains an experimental subset and does not claim complete lowering correctness. CFG reachability,
predecessor/successor relationships, dominance, block-argument checking, and unreachable-block
rejection are now executable. A small block-parameter SSA artifact is available as the first 0.4
foundation, including deterministic identities, validation, and body→HIR→SSA traceability.

- define typed state, transitions, ownership relationships, and regions;
- represent normal, error, compensation, and diagnostic paths;
- make effects and capability flow explicit;
- define deterministic lowering from the semantic graph;
- preserve source-to-semantic traceability;
- preserve machine-intent operation and obligation identities;
- represent explicit integer edge behavior without backend undefined behavior;
- preserve enough structure to identify bounded repair regions and protected properties;
- represent candidate transformations without modifying the trusted baseline;
- demonstrate lossless projection from human source to canonical semantic form for the supported subset;
- prototype a proof-term subject/obligation link without making HIR itself the proof kernel; and
- preserve proof-object and assumption identities through HIR traceability where the supported subset allows it.

## 0.4 — Verified SSA and micro-debugging

**Status: Implemented but partial.** Validated block-parameter SSA, body/SSA reference execution, and evidence-gated no-overflow elision exist. This is not verified compiler correctness, memory SSA, or a completed backend.

Initial bounded foundation now present:

- versioned `0.4` SSA values, instructions, block parameters, terminators, and failure targets;
- deterministic SSA serialization/fingerprints and structural validation;
- semantic-body → HIR → SSA trace entries and transformation provenance;
- verifier/evidence authority classes that keep diagnostic observations from satisfying proof obligations; and
- one evidence-gated no-overflow-elision experiment that refuses missing, stale, wrong-scope, or non-verifier PASS evidence.

This is not verified compiler correctness, SSA optimization, memory SSA, or a backend.

The Luna bounded reference-execution increment is now also present:

- versioned JSON execution requests/results with explicit returned, runtime-failure, unsupported,
  budget-exhausted, and invalid-request outcomes;
- deterministic body-level interpretation with a mandatory finite step budget and capped identity
  traces;
- typed integer comparison (`eq`, `ne`, `lt`, `le`, `gt`, `ge`) through body validation, HIR, and
  SSA, with signed/unsigned interpretation;
- conservative effect handling with explicit deterministic recording policy and no ambient authority;
- reusable corpus comparison reports with mismatch localization and finite-equivalence language;
- a bounded-sum baseline, structural equivalent refactor, intentional regression, and frozen edge corpus;
- CLI commands `execute` and `compare-execution`; and
- Forge development verifier `mncs-language.execution-equivalence`.

The next bounded conformance slice is now present:

- an independent SSA reference evaluator with versioned results, SSA/HIR fingerprints, bounded
  traces, explicit unsupported/failure/budget outcomes, and semantic/HIR/SSA divergence context;
- CLI commands `execute-ssa` and `check-lowering-execution`;
- body↔SSA consistency checks over the bounded-sum corpus and a six-case bounded range/offset kernel;
- scalar `sub`, `mul`, and wrapping bitwise `and`/`or`/`xor` propagation through validation, HIR,
  SSA, body execution, and SSA execution; and
- Forge development verifier `mncs-language.lowering-execution-consistency`, kept separate from
  structural SSA validation.

This enables experimental MNCS-Language refactoring/reference studies before backend completion. It
does not establish universal equivalence, production runtime behavior, or compiler correctness.

- introduce basic blocks, SSA values, phi-like joins, and explicit memory regions;
- attach proof obligations to transformations;
- define conservative aliasing and overflow semantics;
- implement the first evidence-bearing bounds or alignment capability;
- preserve machine-intent traceability through SSA lowering;
- ensure `UNKNOWN` obligations select only declared conservative fallbacks;
- integrate the first micro-verifier interfaces;
- demonstrate incremental invalidation after a local change;
- derive a machine-readable diagnostic obligation from a failed verifier;
- produce a minimal defensible causal slice;
- generate and evaluate at least one isolated repair proposal;
- compare the baseline and candidate across contracts, effects, capabilities, assumptions, complexity, target support, backend promises, and evidence;
- enforce recursion, authority, candidate-count, and verifier-call budgets;
- add the first kernel-checkable proof or independently checkable certificate to a transformation obligation; and
- demonstrate that stale proof dependencies invalidate only the dependent transformation authority.

## 0.5 — Backend and bounded recursive-refinement experiment

**Status: Complete (bounded experimental milestone, 2026-08-23).** Backend count alone did not
complete this milestone; the evidence-gated promise, arithmetic, control comparison, and retained
refinement-cycle criteria below were exercised together.

| Acceptance item | Status |
| --- | --- |
| constrained subset to LLVM IR, Cranelift IR, or WASM | **Implemented / exercised** (WASM, LLVM IR, Cranelift CLIF; research bytecode and C11 are additional realizations) |
| optimization attributes only from verified facts | **Implemented / exercised** (`nsw`/`nuw` require a selected-SSA-bound constant-range certificate that is rechecked on the release authorization path; otherwise LLVM uses overflow intrinsics) |
| wrapping, checked, saturating, trapping, widening integers | **Implemented / exercised** as language semantics through body/HIR/SSA/reference execution; bounded saturating/widening widths execute in WASM and research bytecode, while unsupported scalar backends fail closed |
| record why no-overflow/in-bounds/alignment/non-aliasing/relaxation promises are emitted or withheld | **Implemented / exercised** as identity-bound `promise_decisions`; no-overflow has emitted and withheld cases, and non-applicable memory/alignment/alias/relaxation promises are explicitly withheld |
| executable + evidence manifest together | **Implemented / exercised** for adapter artifacts; native packaging is external clang/llc |
| compare generated output with equivalent Rust or Zig | **Implemented / exercised** by `experiment rust-control`; five WASM arithmetic edge cases agree with an independently compiled Rust control |
| bind realizations to compiler/backend/feature/ABI/pass identities | **Implemented / exercised** |
| document backend-introduced assumptions | **Implemented / exercised** |
| one bounded observe–localize–propose–verify–compare–promote cycle | **Implemented / exercised** by `experiment refine` over a frozen bytecode baseline, accepted WASM realization, and retained rejected semantic mutant |
| independent evidence before automatic promotion | **Implemented / exercised**: valid PASS translation-validator artifacts and protected identity/behavior agreement are mandatory; Forge/search has no promotion authority |
| auditable accepted/rejected transformation history | **Implemented / exercised** in sealed `BoundedRefinementCycle` decisions with evidence identities and rejection reasons |
| proof-carrying lowering for one backend promise | **Implemented / exercised** for LLVM signed/unsigned no-overflow attributes using exact constant-range certificates |
| untrusted proof search / narrow trusted checking | **Implemented / exercised** for the pilot: certificate discovery proposes; a narrow identity/dependency/range checker authorizes emission; Forge remains untrusted search |

The body and SSA reference evaluators are study oracles and finite conformance tools, not substitutes
for the proof kernel or a production backend. Roadmap 0.5 completion is scoped to the table above;
the residual maturity items are documented as post-0.5 work rather than silently treated as proof.

## 0.6 — Surface-language and self-description experiments

**Status: Planned / research-only**, with a syntax laboratory already exercising tournaments. Grammar selection is deliberately deferred.

> Note (2026-08): *Source* Profile 0.6 — payload-bearing sums, strict boolean
> operators, checked division/modulo — landed experimentally via the
> standard-library tranche (`docs/source-profile-0.6.md`,
> `docs/development-evidence/ox-alpha-tranche-2026-08.md`). That source-profile
> numbering is independent of this roadmap section, which concerns grammar
> selection and self-description experiments.

- expand the syntax tournament to a representative corpus;
- implement parsers and canonical formatters for at least two competing source candidates;
- include machine-intent expressions in syntax and semantic-density comparisons;
- measure human readability, agent generation, localized repair, model-token counts, parser recovery, canonical formatting, graph round trips, omission rates, proof search/checking cost, and verifier workload;
- ensure every compared candidate represents the same declared semantic claims;
- avoid syntax that cannot round-trip to the semantic graph without hidden meaning;
- select or reject the Zig-influenced direction using collected evidence;
- expose compiler and verifier contracts, operation vocabularies, obligation templates, proof-core terms, and lowering rules through the same introspection model used for ordinary programs;
- begin a limited self-hosting experiment without obscuring bootstrap trust;
- select a grammar only after semantic coverage and repairability are demonstrated.

## 0.7 — Target realizations and optimization regions

**Status: Planned.** Host triples and LLVM/Cranelift target facts exist so x86-64/AArch64 can be realizations, not language variants. SIMD regions, Pareto selection, and stale-after-toolchain-change proofs are not implemented.

- bind portable reference semantics to at least two target-specific realizations;
- implement explicit target dispatch and conservative fallback;
- compare scalar and SIMD realizations under one semantic operation identity;
- add a bounded optimization-region model with mutation, target, authority, resource, candidate, and verifier limits;
- reject a faster candidate that violates a protected requirement;
- measure performance, code size, evidence cost, proof cost, portability, and invalidation behavior;
- preserve accepted and rejected realization histories; and
- demonstrate stale evidence after target, compiler, backend, or pass-pipeline changes.

## 1.0 research threshold

A 1.0 designation would indicate a coherent research language and toolchain, not necessarily production maturity. It requires:

- a versioned specification;
- a stable semantic core;
- deterministic parsing and canonical formatting;
- a versioned canonical semantic representation;
- explicit machine-intent, obligation, lowering-envelope, and portability-envelope semantics;
- a documented formal proof boundary and versioned proof-core representation;
- at least one independently checkable proof path for a language-generated obligation;
- at least one executable backend;
- an evidence manifest with verifiable artifacts;
- a versioned assurance model that preserves support, refutation, conflict, provenance, trust, freshness, independence, and action-relative policy decisions;
- a versioned logical value/type model that distinguishes logical identity, semantic observation, representation realization, storage, and serialization;
- at least one demonstrated logical datatype with multiple evidence-bearing representation realizations and an explicit representation-sensitive constraint that narrows the realization envelope;
- a versioned relation model that distinguishes identity, equality, equivalence, refinement, substitutability, compatibility, abstraction, and bounded agreement with explicit observation/context scope where required;
- at least one transformation study in which the same subject pair satisfies one declared relation while failing or leaving unresolved another, without collapsing the results into a universal equivalence claim;
- a versioned numeric model that separates mathematical domains, finite/approximate realizations, rounding/error contracts, reproducibility, and numerical evidence;
- at least one numeric-realization study where precision/representation/accumulation can vary while a protected numerical relation remains explicit and a numerically invalid faster candidate is rejected;
- a versioned termination/progress model that distinguishes totality, divergence, productivity, concurrent progress, liveness, fairness dependencies, and bounded execution;
- at least one bounded-computation study proving mechanically that `BUDGET_EXHAUSTED` leaves termination unresolved rather than implying divergence, alongside one independent positive termination or productivity witness;
- a versioned temporal model that distinguishes causal/logical order, physical/monotonic/reference/civil/virtual time, clock uncertainty, instants/durations/intervals, temporal validity, deadlines/timeouts/periods/jitter, logical effect time, and schedule realizations;
- at least one temporal study where uncertain clock observations make one ordering or freshness claim `UNRESOLVED`, alongside one independently checkable schedule witness that satisfies explicit deadlines under current target/execution-time assumptions;
- at least one demonstration that two different physical schedules can preserve the same declared logical execution-time contract without implying physical timing identity;
- a versioned information-flow model that distinguishes confidentiality, integrity, flow ordering, observer/channel scope, noninterference families, bounded declassification/endorsement, and target-sensitive side-channel claims;
- at least one information-flow study where the same program satisfies a narrow observer relation while failing a broader termination/timing/address/scheduler-sensitive relation, without collapsing either result into a universal security bit;
- at least one explicit declassification/endorsement study proving that validation or successful execution cannot fabricate downgrade/endorsement authority, plus one Forge candidate rejected for violating a protected security relation despite otherwise valid functional behavior;
- a versioned principal/identity model that distinguishes principals, names/namespaces, keys/authenticators, credentials, workloads, devices, agents, code artifacts, platforms, execution instances, and sessions;
- a versioned authentication/credential model that distinguishes identity proofing, enrollment, cryptographic validity, bearer versus proof-of-possession semantics, issuer authority, authentication, authorization, audience/context, expiration, revocation, and `UNKNOWN` status;
- a versioned delegation/trust model with explicit authority scope, delegation rights, attenuation, caveats, derivation provenance, threshold principals, trust domains, federation, and claim-class-specific trust anchors;
- a versioned attestation model that separates attesters, evidence, measurements, reference values, attestation endorsements, verifier appraisal, results, freshness, trustworthiness, and final relying decisions;
- at least one identity study where principal identity survives key rotation while key identity changes, and one credential study where cryptographic validity passes while action authorization fails;
- at least one delegation study where a narrower child authority is accepted while a broader child authority is mechanically rejected, plus one status case where `UNKNOWN` remains distinct from `GOOD`;
- at least one attestation study where evidence is authentic and fresh but the measured state is unacceptable, plus one stale-attestation study that cannot establish current state;
- at least one Forge identity-realization study where a broader or longer-lived credential is rejected in favor of a least-sufficient, short-lived proof-of-possession realization with appropriate current attestation;
- a versioned persistent-state model that distinguishes logical state/version, transaction intent/attempt/outcome, volatile and persistent realization, snapshot, checkpoint, journal, persistent root/generation, crash state, and recovered state;
- a versioned durability model that requires explicit failure models and persistence domains and distinguishes memory order/visibility from persistence order/durability;
- a versioned transaction/recovery model that distinguishes failure atomicity, isolation, serializability, snapshot isolation, visibility, durability, acknowledgement, crash conditions, recovery procedures, and recovered-state relations;
- at least one persistence study mechanically demonstrating `Visible != Durable` and one profile where acknowledgement-before-durability is rejected because `AckImpliesDurability` is required;
- at least one persist-order study where ordinary source/memory order is insufficient and an explicit persistence-order witness is required;
- at least one failure-atomic update where crash exploration permits only old/new logical state and rejects a torn mixture;
- at least one transaction study where snapshot isolation is established while serializability is refuted without collapsing the two claims;
- at least one recovery study that remains correct across a crash during recovery, or otherwise provides an independently checkable repeated-recovery convergence witness;
- at least one snapshot/checkpoint study proving they are distinct semantic objects and one persistent-reference generation study rejecting a stale reference after rollback/recovery;
- at least one RFC 0020 relation study where crash-free functional behavior agrees while a required crash relation fails;
- at least one Forge persistence-realization study comparing multiple journal/copy-on-write strategies and rejecting a faster candidate that violates a protected durability, isolation, persist-order, or recovery relation;
- at least one bounded crash-testing result that remains explicitly bounded instead of being upgraded to universal crash-safety evidence;
- a versioned serialization/schema model that distinguishes logical values/types, schemas, schema versions/lineage, wire contracts, encoding profiles, canonical profiles, framing, and encoded artifacts;
- a versioned data-evolution model with stable field/variant identities, reserved historical identities, explicit presence, open/closed structure, aliases/deprecation, typed migrations, migration losses, and compatibility envelopes;
- a versioned serialization relation family distinguishing value/raw-byte/canonical/cross-version round trips, directional wire readability, semantic compatibility, unknown-field preservation, rewrite-without-loss, migration refinement, and migration-path coherence;
- at least one encoding study where two legal byte encodings decode to one semantic value while only one is canonical, plus one canonical re-encode/digest study proving raw byte identity and canonical semantic encoding identity remain distinct;
- at least one schema-identity study where a rename preserves a field identity while reuse of a retired field/wire identity is mechanically rejected;
- at least one presence study proving `Absent != Present(Default)` when that distinction is logically observable;
- at least one unknown-field study where an older reader preserves a future field without understanding it, plus one readable format bridge that fails forward round-trip preservation by losing the unknown field;
- at least one directional compatibility study where old-writer/new-reader succeeds while the reverse relation fails, and one schema pair that remains wire-readable while semantic compatibility is refuted by a concrete counterexample;
- at least one migration graph/diamond study establishing or refuting path coherence explicitly and preserving losses/projections as machine-readable artifacts;
- at least one numeric serialization study carrying RFC 0021 conversion obligations through the wire boundary;
- at least one bounded decoder study where resource/budget exhaustion remains distinct from malformed input and one projection-decode study that cannot authorize whole-message validation;
- at least one Forge serialization-realization study comparing multiple wire formats/profiles and rejecting a faster/smaller candidate that violates a protected canonicality, presence, unknown-preservation, numeric, compatibility, or migration relation;
- a versioned distributed-execution model distinguishing participant, principal/process/replica, logical operation, invocation attempt, message, configuration/epoch, and observer identities;
- a versioned network/failure model distinguishing synchronous/asynchronous/partially synchronous assumptions, communication failures, partitions, crash/omission faults, failure suspicion, and Byzantine/authenticated-Byzantine behavior;
- explicit message lifecycle, delivery, ordering, retry, deduplication, and exactly-once logical-effect semantics so `Sent`, `Received`, `Applied`, `Persisted`, and `Acknowledged` cannot be conflated;
- at least one remote-outcome study where the receiver establishes `Applied` while the caller remains `OUTCOME_UNKNOWN` after response loss;
- at least one failure-detector study where a timeout produces only a scoped suspicion and a later observation clears suspicion without rewriting the historical timeout into a false failure fact;
- at least one consensus study that refuses universal deterministic termination under the fully asynchronous one-crash model while preserving a separate safety claim, plus one stronger assumption profile with a scoped liveness witness;
- a versioned consistency relation family distinguishing linearizability, sequential consistency, causal consistency, session guarantees, and eventual convergence;
- at least one consistency study proving `Convergent != Linearizable` and at least one session-guarantee study over temporarily stale replicas;
- a versioned quorum model with explicit configuration scope and independently checkable intersection evidence, including one valid and one refuted quorum configuration;
- at least one CRDT/convergent-replica study with a checked merge/commutativity witness and one case where replicas converge while an application invariant is refuted;
- a versioned coordination-requirement model that can use monotonicity and invariant-confluence evidence to distinguish coordination-free regions from regions that require coordination;
- at least one Forge study where a coordination-free realization is selected because the protected semantics permit it, and one study where the requested invariant/partition behavior generates coordination or an explicit unrealizability result;
- a versioned membership/reconfiguration/leadership model with configuration epochs and fencing semantics, including one stale-leader write rejected by a newer fencing epoch;
- at least one distributed-snapshot study distinguishing a consistent global cut from an inconsistent arbitrary cut without relying on simultaneous physical timestamps;
- a versioned distributed-transaction model distinguishing local transactions, distributed atomic commit, blocking two-phase-commit-style realization, consensus-backed commit, saga, and compensation;
- at least one distributed-commit study where safety remains protected while liveness blocks after a coordinator failure, and one compensation study that restores a declared business relation without claiming historical rollback;
- at least one Byzantine/fault-threshold claim whose meaning is explicitly scoped to configuration, network, authentication, quorum, and independence assumptions;
- at least one bounded partition/failure exploration result that remains explicitly bounded evidence rather than being upgraded to universal distributed correctness;
- a versioned spatial model distinguishing logical identity, place/virtual-place identity, physical resource identity, physical data-instance identity, topology identity, and placement identity;
- a versioned topology model supporting containment plus at least one non-tree relation such as accessibility, affinity, transfer, trust sharing, or fault-domain sharing;
- a versioned placement contract model that distinguishes hard requirements from preferences and represents affinity, anti-affinity, direct-access, communication-boundary, trust-domain, and fault-domain constraints explicitly;
- explicit computation placement and data placement with unresolved location variables or equivalent placement holes and `SAT`, `UNSAT`, and `UNKNOWN` placement conclusions;
- a capability-based heterogeneous resource model that distinguishes device class from actual execution/memory capabilities and cross-RFC realization applicability;
- at least one placement study where logical task/object identity survives a physical movement while placement identity changes;
- at least one RFC 0020 placement-observation study where two placements satisfy a narrow functional relation but fail a location-observing relation after placement becomes observable;
- at least one preference-versus-requirement study where a candidate remains semantically admissible but suboptimal under a preference and becomes invalid when the same locality condition is promoted to a hard requirement;
- at least one topology study proving identifier adjacency does not establish proximity;
- at least one fault-domain study where multiple RFC 0028 replicas fail an independence claim because physical topology reveals a shared failure domain, plus one mapping that satisfies the required separation;
- at least one heterogeneous-device study where different resource classes remain admissible because they satisfy the actual capability set and one nominally appropriate device is rejected because a required capability is absent;
- at least one placement-hole study where the checker establishes `SAT`, `UNSAT`, or explicit `UNKNOWN` under a concrete topology snapshot;
- at least one communication-free/locality study that passes under one placement and produces a concrete counterexample after required data crosses the protected boundary;
- a versioned mobility model distinguishing migration unit, mobility boundary, migration closure, transfer, cutover, destination activation, source retirement, rollback, and unknown outcome;
- at least one migration study where logical identity survives movement while destination trust/attestation evidence must be refreshed and source evidence cannot be reused as destination evidence;
- at least one capability-rebinding study that preserves, explicitly rebinds, invalidates, or refuses a location-bound capability without fabricating new authority;
- at least one migration-cutover study using fencing or equivalent authority to reject stale-source mutation after destination activation;
- at least one RFC 0028 boundary-crossing study where moving a previously local interaction across an independent failure domain is rejected unless a distributed realization preserves the required relation;
- at least one RFC 0027 placement-generated serialization study where a boundary introduces a wire obligation and an invalid codec causes the placement realization to fail;
- at least one RFC 0021 placement study where a faster accelerator candidate is rejected because its available numerical realization violates the protected numeric relation;
- at least one topology-change study where stale topology/capability evidence invalidates only dependent placement claims rather than globally invalidating unrelated mappings;
- at least one placement witness produced by an untrusted search mechanism and independently checked by a narrower placement verifier;
- at least one Forge heterogeneous-placement study comparing multiple valid/invalid candidates and rejecting a faster candidate that violates a protected numeric, trust, memory-access, fault-domain, security, serialization, distribution, or migration relation;
- at least one bounded placement/migration exploration result that remains explicitly bounded rather than being upgraded to universal placement independence or migration transparency;
- a versioned lifecycle model distinguishing release/artifact-version, deployment-generation, execution-instance, version-population, protocol/schema/configuration/feature, and lifecycle-configuration identities;
- explicit lifecycle-state distinctions including installed, loaded, active, serving, authoritative, finalized, and retired states;
- a versioned lifecycle-transition model with allowed/forbidden intermediate configurations and trace-scoped upgrade claims;
- directional runtime/version-coexistence compatibility matrices and envelopes plus an independently checkable upgrade-dependency graph;
- at least one study where all-old and all-new endpoints are valid but a mixed-version rolling transition is mechanically refuted;
- at least one request, transaction, session, or equivalent version-consistency study where a protected semantic unit remains on one valid version while the surrounding system upgrades;
- at least one active-update study where a naive in-place transition violates version consistency while pinning, quiescence, or verified continuation migration preserves the declared relation;
- at least one upgrade-order study where directional compatibility generates a valid partial order and one cyclic dependency is reported unrealizable until a bridge release/compatibility shim is introduced;
- lifecycle binding to RFC 0027 typed state migrations and RFC 0026 crash-safe migration with explicit old/dual/new authority phases;
- at least one state-lifecycle study proving code deployment, state migration, full traffic cutover, finalization, and retirement remain distinct;
- explicit canary/shadow exposure semantics with RFC 0018-scoped evidence and effect authority, including one canary result that cannot be promoted to universal safety and one shadow candidate rejected for ambient authoritative effects;
- explicit cutover/finalization/irreversibility semantics, including one 100%-traffic state that remains unfinalized and one finalization step that explicitly narrows or destroys a rollback path;
- a versioned rollback model treating rollback as a separately checked transition, including one case where reinstalling old code is rejected because current authoritative state is incompatible;
- granular retirement semantics with dependency guards, including one case where an old session/continuation blocks executable retirement after all new traffic has moved to the successor version;
- persistent/failure-aware lifecycle orchestration with one controller-crash or lost-response case yielding `UNKNOWN` and requiring reconciliation rather than fabricated completion;
- at least one RFC 0028/RFC 0029 rollout study where a candidate batch is rejected because its temporary population violates quorum, availability, or physical fault-domain requirements;
- at least one lifecycle witness produced by an untrusted planner and independently checked by a narrower compatibility/version-consistency/migration/rollback/retirement verifier set;
- at least one Forge lifecycle-realization study comparing all-at-once, rolling, bridge, canary, blue/green, hot-update, or similar candidates and rejecting an easier/faster plan because an intermediate-state, state, authority, version-consistency, quorum, placement, rollback, or retirement relation fails;
- at least one bounded canary/mixed-version/rollback exploration result that remains explicitly bounded evidence rather than being upgraded to universal lifecycle correctness;
- conservative and traceable backend-promise emission;
- bounded unsafe and foreign-function interfaces;
- a versioned recursive diagnostic and refinement protocol;
- isolated candidate transformations and explicit promotion policy;
- documented recursion, authority, resource, target, representation, universe, equality, numeric, termination, fairness, clock, temporal-validity, scheduling, information-flow, observer, reclassification, side-channel, principal, identity, credential, delegation, trust-domain, federation, revocation, cryptographic-algorithm, freshness, attestation, persistent-state, transaction, failure-model, persistence-domain, persist-order, durability, journal, snapshot, checkpoint, recovery, persistent-reference, retention, schema, schema-lineage, field-identity, presence, unknown-field, duplicate-field, wire-contract, encoding-profile, canonicalization, framing, migration, Unicode, decode-budget, compatibility, participant, message, logical-operation, invocation-attempt, network-model, partition, failure-detector, suspicion, configuration, epoch, replica, quorum, consistency, convergence, conflict-resolution, consensus, broadcast, Byzantine, lease, fencing, distributed-snapshot, distributed-transaction, coordination, place, topology, topology-snapshot, resource-capability, location-variable, computation-placement, data-placement, affinity, anti-affinity, fault-domain, mobility-boundary, migration-closure, cutover, capability-rebinding, heterogeneous-realization, placement-witness, release, deployment-generation, lifecycle-configuration, lifecycle-transition, version-population, version-coexistence, update-point, version-consistency, exposure-scope, feature-activation, finalization, irreversibility, rollback, retirement, lifecycle-witness, and assumption limits;
- documented soundness and bootstrap limits;
- conformance tests and independent implementation guidance.
