# Roadmap

This roadmap separates semantic research from final surface-language selection while treating recursive introspection, representation efficiency, machine-intent semantics, proof-carrying semantics, and refinement as cross-cutting requirements.

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

## 0.1 — Executable semantic model

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

- lower a constrained subset to LLVM IR, Cranelift IR, or WebAssembly;
- emit optimization attributes only from verified facts;
- demonstrate wrapping, checked, saturating, trapping, and widening integer operations;
- record why backend no-overflow, in-bounds, alignment, non-aliasing, or relaxation promises are emitted or withheld;
- produce an executable and evidence manifest together;
- compare generated output with equivalent Rust or Zig implementations;
- bind target-specific realizations to compiler, backend, feature, ABI, and pass-pipeline identities;
- document all backend-introduced assumptions;
- complete one bounded observe–localize–propose–verify–compare–promote cycle;
- require independent evidence before automatic candidate promotion;
- retain an auditable record of accepted and rejected candidate transformations;
- test proof-carrying lowering for one narrowly defined backend promise; and
- keep proof search outside the trusted kernel even when Forge orchestrates it.

The body and SSA reference evaluators are study oracles and finite conformance tools, not substitutes
for the proof kernel or a production backend.

## 0.6 — Surface-language and self-description experiments

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
- conservative and traceable backend-promise emission;
- bounded unsafe and foreign-function interfaces;
- a versioned recursive diagnostic and refinement protocol;
- isolated candidate transformations and explicit promotion policy;
- documented recursion, authority, resource, target, representation, universe, equality, numeric, termination, fairness, clock, temporal-validity, scheduling, and assumption limits;
- documented soundness and bootstrap limits;
- conformance tests and independent implementation guidance.