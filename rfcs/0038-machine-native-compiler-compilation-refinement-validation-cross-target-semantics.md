# RFC 0038 — Machine-Native Compiler, Compilation, Refinement, Validation, and Cross-Target Semantics

- **Status:** Draft
- **Track:** Compiler architecture and toolchain semantics
- **Depends on:** RFC 0002, RFC 0003, RFC 0004, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0013, RFC 0014, RFC 0015, RFC 0016, RFC 0017, RFC 0018, RFC 0019, RFC 0020, RFC 0021, RFC 0022, RFC 0023, RFC 0024, RFC 0027, RFC 0029, RFC 0031, RFC 0032, RFC 0033, RFC 0034, RFC 0035, RFC 0036, RFC 0037
- **Primary ownership:** compilation identity; compiler pipeline contracts; cross-IR refinement; pass semantics; translation validation; backend contracts; host/build/target/run separation; compilation evidence; compiler/Forge/Fabric boundaries; cross-platform compiler studies

## Abstract

MNCS needs a compiler whose correctness model matches the language's existing semantic architecture.

A conventional compiler is often described as:

```text
source -> AST -> IR -> machine code
```

That description is insufficient for MNCS. The language already distinguishes semantic identity, proof and evidence, effects, authority, provenance, concurrency, failure, target facts, representation realizations, refinement relations, build derivations, optimization policy, empirical studies, elaboration, and runtime observation.

RFC 0038 therefore defines compilation as an **evidence-bearing refinement process between versioned semantic artifacts**.

The constitutional statement is:

> **An MNCS compiler does not gain authority because it emitted an artifact. Each semantically meaningful lowering or optimization claims a typed relation between input and output artifacts, under explicit assumptions, target facts, and evidence.**

The practical implementation direction is:

> **MNCS should begin as a certifying refinement compiler with independently checkable translation edges, not as either a conventional opaque optimizer or an immediately monolithic formally verified compiler.**

The compiler may contain ordinary hand-written passes, solver-driven transformations, equality-saturation searches, generated passes, model-proposed optimizations, or target-specific lowering. Those mechanisms may propose candidates. They do not define correctness.

RFC 0020 relations, RFC 0018 assurance policy, RFC 0007 proof objects, bounded translation validators, and conservative reference execution establish the authority of each transformation according to its claim class.

The initial compiler must also be deliberately multi-platform. MNCS currently has Fabric-connected execution environments on Linux, Windows, and Raspberry Pi-class Linux/ARM systems. Those systems should become an experimental compiler laboratory from the beginning rather than being treated as ports after a single-host compiler has hardened.

---

## 1. Scope

RFC 0038 owns the semantics and architecture of:

- compiler invocation and compilation identity;
- compiler host identity;
- build host identity;
- target contract identity;
- execution/run environment identity;
- source/profile-to-core compilation orchestration;
- elaboration handoff from RFC 0035;
- semantic-core to HIR refinement;
- HIR to SSA refinement;
- SSA transformation and optimization claims;
- target-lowering contracts;
- backend adapter interfaces;
- pass identity, input domain, output domain, assumptions, and produced obligations;
- translation-validation requests and results;
- compilation evidence bundles;
- conservative treatment of `UNKNOWN`;
- compiler/Forge/Fabric authority boundaries;
- cross-host and cross-target compiler experiments;
- incremental compilation identity and invalidation direction;
- compiler bootstrapping direction.

RFC 0038 does **not** redefine:

- source grammar or source representation — RFC 0005 / RFC 0035;
- executable language meaning — RFC 0012;
- generic proof theory — RFC 0007;
- evidence and assurance algebra — RFC 0018;
- generic equivalence/refinement relations — RFC 0020;
- target/runtime facts — RFC 0017;
- build derivation and reproducibility — RFC 0031;
- quantitative performance meaning — RFC 0032;
- optimization preference/selection policy — RFC 0033;
- empirical experiment methodology — RFC 0034;
- language-profile evolution — RFC 0036;
- runtime observation semantics — RFC 0037.

The compiler consumes those semantics and binds them into one executable derivation path.

---

## 2. Constitutional rules

### 2.1 Compilation is a relation, not a file-producing event

```text
compile(A) -> B
```

is operational syntax.

The semantic claim is closer to:

```text
RelationClaim(
    source = A,
    result = B,
    relation = R,
    assumptions = S,
    evidence = E
)
```

where `R` is an RFC 0020 relation appropriate to the edge.

### 2.2 Compiler success is not compiler correctness

A successful exit status proves only that a compiler execution reported success under its interface.

It does not prove semantic preservation, target validity, reproducibility, absence of compiler defects, or artifact authorization.

### 2.3 Refinement is the default cross-level relation

Compilation frequently reduces implementation freedom or nondeterminism.

Therefore universal equality or bisimulation is not required for every lowering.

The default compiler relation should be directional observational refinement unless a narrower or stronger relation is explicitly declared.

### 2.4 Every edge names its observation model

Functional result equivalence, trace refinement, timing preservation, constant-time preservation, authority preservation, representation equivalence, failure refinement, and information-flow preservation are different claims.

A pass cannot claim generic `equivalent=true`.

### 2.5 Passes are first-class transformation artifacts

A semantically relevant pass must have an identity and declare at least:

- accepted input representation/version;
- produced output representation/version;
- claimed relation class;
- required invariants/facts;
- assumptions consumed;
- obligations generated;
- evidence/certificate interface;
- target applicability where relevant;
- deterministic/nondeterministic behavior policy;
- invalidation dependencies.

### 2.6 The optimizer is not automatically trusted

A pass implementation, LLM, solver, superoptimizer, e-graph search, Forge strategy, or generated compiler component may propose a transformation.

Proposal authority is separate from promotion authority.

### 2.7 Translation validation is a first-class enforcement path

Where practical, validate the concrete transformation:

```text
A -> B
```

rather than requiring the entire transformation generator to belong to the trusted computing base.

### 2.8 `UNKNOWN` never authorizes stronger code generation

If a required refinement, target fact, alias fact, range fact, authority fact, or proof obligation is unresolved, the compiler must either:

- select a declared conservative lowering;
- retain the relevant dynamic check;
- retain the semantic operation for a later phase;
- reject that realization;
- or report an unresolved compilation result.

It must not silently assume the stronger fact.

### 2.9 Host, build, target, and run identities are distinct

```text
CompilerHost != BuildHost != Target != RunEnvironment
```

For a local native build some of these may coincide, but the semantic model must never depend on that coincidence.

### 2.10 Target names are candidate identifiers, not evidence

Strings such as `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, or `aarch64-unknown-linux-gnu` may identify target-profile candidates.

They do not by themselves establish ABI, CPU feature, memory-model, runtime-service, clock, atomic, alignment, or operating-environment facts.

RFC 0017 remains authoritative for target/environment evidence.

### 2.11 Backend choice is a realization

LLVM, Cranelift, WebAssembly, direct machine-code emission, an external assembler, or a future backend is not MNCS language semantics.

A backend is a realization provider behind an explicit target-lowering contract.

### 2.12 The reference implementation language is not ontology

The current Rust implementation is a practical implementation choice.

Compiler semantics must remain independently implementable.

### 2.13 Compilation evidence is an output artifact

The primary executable and the evidence explaining its derivation are distinct outputs with related identities.

### 2.14 Forge owns search, not semantic legality

Forge may generate, mutate, rank, or select candidate compiler transformations and realizations.

The compiler contract and validators owned by `mncs-language` determine whether a candidate is semantically admissible.

### 2.15 Fabric owns distributed realization and observation, not language truth

Fabric may dispatch compiler builds/runs across machines, report node facts, execute artifacts, collect traces and quantitative observations, and compare study results.

Fabric observations remain RFC 0034/0037 evidence. Fabric does not define whether a transformation is semantically valid.

### 2.16 Cross-platform disagreement is evidence, not noise

If Linux, Windows, and ARM/Linux realizations disagree under a relation that should be platform-independent, the mismatch must be retained as a compiler/target/specification counterexample until explained.

---

## 3. Compiler theory stack

RFC 0038 intentionally combines multiple CS theory families rather than selecting one universal compiler proof method.

### 3.1 Operational semantics and trace semantics

RFC 0012 remains the normative executable meaning.

Compilation ultimately relates lower representations back to the semantic machine and its observation projections.

### 3.2 Refinement calculus and simulation

Directional refinement is the constitutional relationship between abstraction levels.

Forward/backward simulation, trace inclusion, bisimulation, logical relations, separation-logic relations, or other witnesses may discharge particular edges.

### 3.3 Abstract interpretation

Compiler analyses should be formulated as conservative abstractions where practical.

Range, alias, provenance, authority, effect, resource, liveness, failure, and target analyses should preserve explicit uncertainty rather than inventing facts.

### 3.4 Translation validation

Aggressive and experimental passes should prefer per-transformation validation where feasible.

This permits the transformation generator to remain outside the trusted core.

### 3.5 Certifying compilation

Compiler stages may emit certificates, witnesses, proof terms, counterexample obligations, or checkable analysis results alongside transformed artifacts.

### 3.6 Typed low-level representations

Semantically useful type/effect/provenance/authority information should survive lowering as long as it continues to constrain legality or enable verified optimization.

### 3.7 Logical relations

Use selectively for higher-order functions, abstraction, polymorphism, closures, representation independence, and RFC 0013 transformations.

### 3.8 Separation/resource logic

Use selectively for memory, ownership, provenance, aliasing, unsafe boundaries, and concurrent low-level transformations grounded in RFC 0009/0010.

### 3.9 Equality saturation and superoptimization

Treat these as candidate-search mechanisms, primarily Forge-facing.

They do not replace the compiler's relation model or independent validation.

### 3.10 Mechanized compiler proof

CompCert/CakeML-style mechanization remains an intended high-assurance direction for selected compiler spine components and future bootstrap work.

It is not a prerequisite for the first executable compiler.

---

## 4. Canonical compiler artifact ladder

The compiler should expose a logical artifact ladder even when an implementation fuses internal stages.

```text
LanguageProfile + SourceArtifact
        |
        | parse / bind / elaborate (RFC 0035)
        v
ElaboratedSemanticArtifact
        |
        | semantic-core realization
        v
ExecutableSemanticArtifact (RFC 0012)
        |
        | lower-semantic-to-hir
        v
CanonicalHIR
        |
        | explicit control/effects/resources
        v
LoweredHIR
        |
        | lower-hir-to-ssa
        v
CanonicalSSA
        |
        | validated transforms / optimization search
        v
SelectedSSA
        |
        | target-contract resolution
        v
TargetLoweringPlan
        |
        | backend realization
        v
TargetIR / ObjectArtifact
        |
        | link / package
        v
ExecutableArtifact
        +
CompilationEvidenceBundle
```

The artifact names are architectural roles, not commitments to separate on-disk formats or crates.

### 4.1 Stable versus implementation-private representations

The project should distinguish:

- **semantic interchange representations**, whose version/identity are part of conformance;
- **compiler-internal representations**, which may change freely inside one compiler realization;
- **backend-native representations**, whose semantics are supplied by backend/target contracts.

Not every internal optimization form needs to become language specification.

---

## 5. Required compiler identities

At minimum, a compilation study should be able to distinguish:

- `CompilerImplementationIdentity`
- `CompilerArtifactIdentity`
- `CompilerHostIdentity`
- `BuildHostIdentity`
- `LanguageProfileIdentity`
- `SemanticCoreIdentity`
- `HIRSchemaIdentity`
- `SSASchemaIdentity`
- `PassPipelineIdentity`
- `BackendIdentity`
- `BackendVersionIdentity`
- `TargetContractIdentity`
- `TargetFactSetIdentity`
- `RuntimeEnvironmentIdentity`
- `LinkerIdentity`
- `CompilationInvocationIdentity`
- `CompilationEvidenceIdentity`
- `OutputArtifactIdentity`

This enables questions such as:

```text
Did the same semantic program compiled by the same compiler pipeline
on Linux and Windows for the same target produce the same semantic result?

Did the same SSA compiled for x86-64 and AArch64 preserve the same protected trace relation?

Did a backend upgrade invalidate only backend-dependent evidence?
```

---

## 6. Pass contract

A first-class pass should conceptually expose:

```text
CompilerPass {
    id
    version
    input_schema
    output_schema
    claimed_relation
    observation_model
    required_invariants
    required_target_facts
    consumed_assumptions
    generated_obligations
    deterministic_policy
    validator_profile
    fallback_policy
}
```

A transformation instance should produce:

```text
TransformationEdge {
    pass_identity
    input_artifact
    output_artifact
    relation_claim
    assumptions
    target_facts
    evidence
    diagnostics
    timing/resource observations
}
```

The edge is an independently identifiable artifact.

This is the compiler-scale extension of the transformation provenance already present in the prototype.

---

## 7. Translation-validation architecture

The preferred practical architecture is:

```text
Transformation generator
        |
        v
Candidate artifact
        |
        v
TranslationValidationRequest
        |
        +--> structural validator
        +--> relation-specific validator
        +--> proof/certificate checker
        +--> bounded differential executor
        +--> target legality checker
        |
        v
PASS / FAIL / UNKNOWN
        |
        v
RFC 0018 assurance policy
        |
        v
AUTHORIZED / REJECTED / UNRESOLVED
```

No single validator is expected to prove every relation class.

A bounded executor may support a claim without upgrading finite agreement into universal equivalence.

A proof checker may establish one theorem while leaving a different target property unresolved.

---

## 8. Backend contract

A backend adapter must be treated as an explicit semantic boundary.

A backend should expose or bind:

- supported target contract classes;
- backend and code-generator identity/version;
- calling/ABI realization assumptions;
- data-layout realization;
- integer/float operation mapping;
- atomic/memory-order support;
- trap/unwind behavior;
- object format and linker requirements;
- target-feature requirements;
- backend optimization settings;
- semantic promises consumed from SSA;
- backend-introduced assumptions;
- emitted artifact identity;
- available validation hooks.

The compiler must record why a backend promise such as no-overflow, non-aliasing, alignment, in-bounds access, fast-math permission, or target feature was emitted.

---

## 9. Initial backend strategy

RFC 0038 intentionally does not select one permanent backend.

The implementation program should begin with two complementary paths.

### 9.1 Portable baseline backend

Establish one small portable executable target for end-to-end compiler plumbing and differential studies.

WebAssembly/WASI is a strong candidate because it provides a portable execution baseline across the current Fabric host classes, but the experiment must evaluate it rather than make it language ontology.

The portable baseline exists to answer:

- does semantic -> HIR -> SSA -> executable work end to end;
- do all compiler hosts accept the same canonical inputs;
- can the same executable-level corpus run across all host classes;
- are compilation/evidence identities stable;
- can backend-independent bugs be separated from native-codegen bugs?

### 9.2 Native study backend

In parallel, prototype a replaceable native backend adapter.

Initial candidates may include Cranelift, LLVM, or another backend satisfying the adapter contract.

The first native study should cover at least:

- x86-64 Linux;
- x86-64 Windows;
- AArch64 Linux suitable for the current Raspberry Pi Fabric node where applicable.

Backend choice should be evaluated using RFC 0032/0033 criteria including implementation complexity, compile latency, output quality, target coverage, validation surface, proof/assurance cost, and maintainability.

---

## 10. Cross-platform compiler laboratory

The current Fabric topology should become a deliberate compiler experiment matrix.

### 10.1 Initial host classes

| Host class | Initial role |
| --- | --- |
| Linux x86-64 | primary development host; native x86-64 realization; cross-target study host |
| Windows x86-64 | independent host implementation/environment check; native Windows realization |
| Raspberry Pi / Linux AArch64 | architecture-diverse compiler host and native ARM realization |

If a Raspberry Pi node is running a 32-bit userspace, that fact must create a different host/target profile rather than being silently grouped with AArch64.

### 10.2 Initial target classes

| Target class | Purpose |
| --- | --- |
| portable executable target | common baseline runnable across Fabric hosts |
| Linux x86-64 native | Linux code-generation study |
| Windows x86-64 native | Windows ABI/object/code-generation study |
| Linux AArch64 native | Raspberry Pi / ARM code-generation study |

Optional cross-compilation edges may be added where toolchain closure is available, but native host/target paths should exist first so cross-linker/toolchain problems do not block semantic compiler research.

### 10.3 Required separation

Every study record must separately identify:

```text
compiler_host
build_host
target_contract
run_environment
backend
linker/toolchain
```

A Linux host cross-compiling an AArch64 artifact and an AArch64 Raspberry Pi compiling the same target are two distinct build executions even when they produce identical bytes.

### 10.4 Cross-platform comparison questions

Fabric studies should be able to ask:

- Is canonical semantic identity identical across hosts?
- Is elaborated core identity identical across hosts?
- Is HIR identity identical before target-specific lowering?
- Is SSA identity identical before target-specific specialization?
- Which differences first appear at the target boundary?
- Do native realizations satisfy the same RFC 0020 protected behavioral relation?
- Are failure outcomes consistent where the semantic contract requires consistency?
- Are target-specific differences explained by explicit target facts?
- Is the executable bit-reproducible when a derivation claims that property?
- If not bit-reproducible, is the weaker claimed semantic relation still satisfied?
- Which backend or target facts account for code-size/performance differences?
- Does one platform expose an implicit assumption hidden on another?

---

## 11. Fabric compiler-study protocol direction

Fabric should not need internal knowledge of compiler implementation details.

A bounded protocol should eventually carry artifacts such as:

### 11.1 `CompilerNodeProfile`

```text
node_identity
architecture
os/environment profile
compiler-host capabilities
available backend/toolchain identities
available target profiles
runtime facts
resource envelope
clock/measurement facts
```

### 11.2 `CompilationStudyRequest`

```text
source/semantic artifact identity
language profile
compiler identity
pipeline identity
target contract
backend candidate
assurance profile
requested evidence
requested empirical measurements
execution corpus
```

### 11.3 `CompilationStudyResult`

```text
build-execution identity
output artifact identity
compilation evidence identity
validator results
run results
trace/counterexample artifacts
quantitative observations
environment facts
unresolved assumptions
```

Fabric transports and executes these requests. The semantic schemas and validator meanings remain owned by `mncs-language` and the relevant RFCs.

---

## 12. Forge/compiler boundary

The boundary should follow one rule:

> **The language owns legality; Forge owns search.**

Forge may propose:

- pass orderings;
- optimization candidates;
- representation choices;
- specialization choices;
- backend candidates;
- target-specific realizations;
- equality-saturation extractions;
- superoptimized regions;
- generated compiler passes;
- experimental validation strategies.

The compiler must translate those proposals into explicit candidate artifacts and obligations.

Forge must not be able to self-declare a candidate correct merely because it generated or selected it.

---

## 13. Reference compiler implementation structure

The current repository already has useful compiler-spine code in `mncs-model`, including executable bodies, HIR, CFG/SSA, provenance, identities, obligations, and bounded execution.

The first implementation should extend that structure rather than rewriting it.

Proposed incremental crate boundaries are:

```text
crates/mncs-model
    normative/research semantic artifacts already implemented

crates/mncs-syntax
    source-representation experiments and parsing

crates/mncs-cli
    current inspection/reference commands; remains user/tool entry point initially

crates/mncs-compiler          [new, phase 1]
    orchestration of elaboration -> semantic -> HIR -> SSA -> target plan
    compilation request/result/evidence bundle
    no duplicated semantic definitions

crates/mncs-target            [new, phase 2]
    target-contract/profile adapters consuming RFC 0017 facts
    host/build/target/run identity structures

crates/mncs-codegen           [new, phase 3]
    backend-neutral codegen adapter contract
    portable and native backend experiments behind interfaces

crates/mncs-translation-check [new only when justified]
    relation-specific translation validators/certificate checkers
```

These are implementation-direction boundaries, not normative package names. They should be introduced only when code volume and dependency direction justify them.

The semantic artifact types should remain below compiler orchestration so the compiler cannot quietly redefine the language.

---

## 14. Compiler implementation roadmap

### Phase C0 — Freeze the compiler contract vocabulary

Deliver:

- compilation request/result schemas;
- host/build/target/run identity separation;
- pass/edge identity schemas;
- target-lowering-plan schema;
- compilation evidence bundle schema;
- explicit relation claim on every semantic edge;
- negative fixtures for identity laundering.

No native code generation is required.

### Phase C1 — Build the compiler driver over the existing semantic spine

Deliver:

```text
semantic artifact -> HIR -> SSA -> target-independent compilation bundle
```

with deterministic identities and existing bounded lowering checks.

Add `mncs compile --emit semantic,hir,ssa,evidence`-style functionality without claiming a production compiler.

### Phase C2 — Make compiler-host portability mandatory

The Rust workspace and compiler driver must build and run validation fixtures on:

- Linux x86-64;
- Windows x86-64;
- Linux AArch64/Raspberry Pi through Fabric.

GitHub CI should cover ordinary Linux and Windows host builds and an AArch64 compile-shape check. Fabric supplies the native Raspberry Pi execution evidence.

Platform-specific code must be isolated behind explicit adapters.

### Phase C3 — Portable executable backend

Lower the bounded scalar/control-flow subset to one portable executable target.

Run identical corpora across Linux, Windows, and Raspberry Pi Fabric nodes.

Compare semantic-body, SSA, and portable-backend behavior under explicit bounded-agreement relations.

### Phase C4 — First native backend adapter

Support the bounded subset on:

- Linux x86-64;
- Windows x86-64;
- Linux AArch64.

Record target and backend assumptions in the evidence bundle.

Do not enable aggressive optimization yet.

### Phase C5 — Translation validation

For selected SSA and backend transformations, implement independent or structurally separated checkers.

Require at least:

- one accepted transform;
- one intentionally invalid transform;
- one `UNKNOWN` case selecting a conservative fallback;
- one stale-evidence rejection after pipeline/backend change.

### Phase C6 — Analysis foundation

Add conservative analysis domains for the bounded subset, beginning with range/overflow and one memory/provenance fact.

Analysis results must retain `UNKNOWN` and dependency identities.

### Phase C7 — Forge candidate interface

Permit Forge to propose bounded SSA/pass/backend candidates.

Every candidate remains isolated until translation validation and RFC 0018 policy authorize it.

Run a small pass-order or realization tournament without allowing the search system to define correctness.

### Phase C8 — Fabric cross-platform differential study

Run one compiler study across all current host classes.

Required comparisons:

- same semantic identity;
- same pre-target HIR/SSA identity where expected;
- explicit target-divergence boundary;
- bounded behavioral relation on all three systems;
- code size and compile/run cost observations;
- one intentionally target-sensitive case;
- one implicit-host-assumption bug fixture.

### Phase C9 — Incremental compilation and invalidation

Use existing identities and dependency graphs to reuse unaffected compiler artifacts.

A local semantic or target-fact change should invalidate only dependent passes/evidence where possible.

### Phase C10 — Specialization and machine-directed compilation

Expose bounded realization holes to Forge for target, representation, scheduling, and code-generation decisions.

Specialization is a refinement with evidence, not a compiler side effect.

### Phase C11 — High-assurance spine

Select one or more critical compiler edges for mechanized proof or independently implemented checking.

Candidates include:

- semantic-core -> canonical HIR;
- canonical HIR -> canonical SSA;
- certificate checker;
- one target lowering subset.

### Phase C12 — Self-hosting/bootstrap study

Only after language and compiler artifacts stabilize sufficiently, begin a limited self-hosting experiment under RFC 0016/0031.

Self-hosting must not erase seed compiler, bootstrap binary, or diverse-rebuild trust roots.

---

## 15. Initial cross-platform acceptance matrix

The compiler should treat the following as research requirements, not marketing support claims.

| Capability | Linux x86-64 | Windows x86-64 | Raspberry Pi Linux ARM64 |
| --- | --- | --- | --- |
| build reference compiler | required | required | required through Fabric |
| semantic validation | required | required | required |
| emit canonical HIR | required | required | required |
| emit canonical SSA | required | required | required |
| deterministic semantic fingerprints | required | required | required |
| portable backend execution | phase C3 | phase C3 | phase C3 |
| native backend execution | phase C4 | phase C4 | phase C4 |
| Fabric study result | phase C8 | phase C8 | phase C8 |

For a 32-bit Raspberry Pi environment, add an explicit ARM32 row rather than weakening the ARM64 contract.

---

## 16. Required negative fixtures

At minimum, RFC 0038 experiments should include cases that reject or preserve uncertainty for:

1. host identity substituted for target identity;
2. target triple treated as proof of a CPU feature;
3. Windows ABI assumption reused for Linux;
4. Linux object/linker assumption reused for Windows;
5. x86 alignment/atomic assumption reused for AArch64 without target evidence;
6. AArch64 target compiled but executed on an incompatible run environment;
7. 32-bit Raspberry Pi userspace mislabeled as AArch64;
8. backend pass emitting a stronger no-overflow promise without evidence;
9. optimization candidate with no declared relation;
10. optimization candidate whose validator returns `UNKNOWN` but is promoted anyway;
11. bounded corpus agreement reported as universal equivalence;
12. stale validation evidence reused after pass-pipeline change;
13. stale backend evidence reused after backend upgrade;
14. compiler host difference silently changing semantic identity;
15. nondeterministic pass claiming reproducible output without recording nondeterminism inputs;
16. Fabric measurement reported as proof of semantic preservation;
17. Forge-selected candidate self-authorizing its own correctness;
18. backend-generated assumption omitted from the evidence bundle;
19. cross-compilation build execution collapsed into native build identity;
20. bitwise mismatch incorrectly reported as semantic failure when only a weaker protected relation was required;
21. semantic mismatch hidden because target binaries both exit successfully;
22. runtime/environment failure laundered into compiler semantic failure;
23. compiler diagnostic text used as canonical error identity instead of a structured diagnostic artifact;
24. external linker/tool silently omitted from RFC 0031 derivation identity;
25. target-specific optimization crossing an RFC 0024 information-flow or side-channel requirement.

---

## 17. Compilation evidence bundle

The first bundle should be deliberately compact but extensible.

```text
CompilationEvidenceBundle {
    schema_version
    compilation_identity
    compiler_identity
    language_profile
    semantic_identity
    host_identity
    build_identity
    target_contract
    backend_identity
    pipeline_identity
    artifact_chain[]
    transformation_edges[]
    validator_results[]
    assumptions[]
    target_facts[]
    unresolved_obligations[]
    derivation_reference
    empirical_result_references[]
}
```

The bundle is not itself proof that all contained claims are true.

It is the canonical dependency/provenance surface through which proof, validation, empirical evidence, and assurance can be appraised.

---

## 18. Reproducibility and cross-host study

RFC 0031 owns the meaning of reproducibility, but the compiler must expose sufficient identities to study it.

The first compiler experiments should distinguish:

1. same source + same host + same compiler + same target;
2. same source + different host + same compiler + same target;
3. same semantic artifact + different compiler build + same target;
4. same SSA + different backend version + same target;
5. same SSA + same backend + different target;
6. native compile versus cross-compile for the same target.

A mismatch should identify the earliest diverging compiler artifact in the chain.

This is more useful than comparing final binaries alone.

---

## 19. Diagnostics

Compiler diagnostics should be structured semantic artifacts first and human renderings second.

A compiler failure should distinguish at least:

- invalid source/profile;
- unresolved elaboration;
- semantic invalidity;
- failed transformation relation;
- unresolved proof/validator obligation;
- unsupported target realization;
- missing target evidence;
- unavailable backend capability;
- resource budget exhaustion;
- external tool failure;
- runtime study failure;
- internal compiler defect.

A machine should not need to parse English stderr to determine which class occurred.

---

## 20. Security and trust boundary

The initial trusted computing base should be kept smaller than the total compiler implementation.

Candidate trusted or high-assurance components include:

- canonical semantic identity machinery;
- normative semantic-core implementation or checked correspondence to it;
- proof kernel;
- selected certificate/translation checkers;
- relation-definition identities;
- target-fact authentication/appraisal paths where required.

Candidate untrusted proposal components include:

- heuristic optimizers;
- Forge search;
- LLM-generated passes;
- superoptimizers;
- equality-saturation engines;
- stochastic search;
- benchmark-driven selectors;
- experimental backend adapters until independently checked.

This partition may evolve as mechanized proofs expand.

---

## 21. First flagship experiment

The flagship experiment should compile one bounded but nontrivial semantic kernel through the complete research pipeline and execute it across the current Fabric host classes.

Suggested kernel properties:

- integer arithmetic with explicit overflow semantics;
- bounded loop/control flow;
- one state/memory region;
- one capability/effect edge;
- one failure/runtime-check path;
- deterministic input corpus plus edge cases.

Study sequence:

```text
one semantic identity
    -> canonical HIR
    -> canonical SSA
    -> portable executable realization
    -> Linux execution
    -> Windows execution
    -> Raspberry Pi execution
    -> bounded cross-platform relation report
    -> native x86-64 Linux realization
    -> native x86-64 Windows realization
    -> native AArch64 Linux realization
    -> per-target relation/evidence comparison
```

The experiment should deliberately include one invalid optimization candidate and one target-sensitive candidate.

The success criterion is not identical machine code.

It is that every divergence is either:

- prohibited and detected;
- permitted by an explicitly scoped relation;
- explained by target/runtime realization facts;
- or retained as `UNKNOWN`/counterexample rather than hidden.

---

## 22. Research questions retained open

RFC 0038 deliberately does not decide yet:

- final backend technology;
- final number of canonical IR levels;
- exact HIR/SSA schema boundaries after the prototype evolves;
- whether target IR becomes an MNCS-owned stable representation;
- which analyses are mechanized versus executable/reference only;
- which translation validators use SMT, proof terms, symbolic execution, or specialized checkers;
- whether the production compiler remains Rust;
- final incremental-compilation cache architecture;
- final linker ownership;
- final JIT/AOT split;
- final self-hosting strategy;
- final binary reproducibility requirements by assurance profile.

These should be selected through bounded experiments rather than incidental early implementation choices.

---

## 23. Research 1.0 compiler threshold

The compiler track should not claim research-1.0 maturity until at least:

- compilation has a versioned request/result/evidence contract;
- host/build/target/run identities are distinct in executable artifacts;
- the existing semantic core lowers through HIR and SSA without hidden semantic redefinition;
- at least one executable backend exists;
- Linux x86-64, Windows x86-64, and the available Raspberry Pi Linux/ARM Fabric node execute the supported compiler study subset;
- at least one portable and one native realization path have been studied;
- each semantically relevant transformation carries a relation claim and provenance;
- at least one transformation is independently translation-validated;
- `PASS`, `FAIL`, and `UNKNOWN` are preserved through compiler validation;
- `UNKNOWN` selects conservative fallback or refusal;
- one proof/certificate-bearing compiler edge exists;
- target/backend promises are evidence-bound;
- one Forge-generated optimization candidate is accepted and one rejected by independent checking;
- one Fabric cross-platform experiment localizes a target/host divergence;
- build/compiler/backend/toolchain identities feed RFC 0031 derivation evidence;
- quantitative compiler/runtime observations feed RFC 0032 without becoming correctness evidence;
- bounded empirical agreement remains RFC 0034 evidence rather than universal equivalence;
- structured compiler diagnostics distinguish semantic, target, toolchain, validator, runtime-study, and internal failures;
- bootstrap trust is documented even before self-hosting;
- an independent implementation or checker can consume at least one canonical compiler artifact format.

---

## 24. Summary direction

RFC 0038 establishes the following compiler philosophy:

```text
semantic truth        -> mncs-language
search/proposal       -> Forge
cross-machine study   -> Fabric
correctness authority -> typed relations + validators + evidence + assurance policy
```

The compiler is therefore not a privileged black box between source and executable code.

It is an inspectable derivation graph whose transformations can be generated aggressively while correctness authority remains narrow, explicit, and independently checkable.

That architecture lets MNCS use ordinary compiler engineering immediately, experimental machine-generated optimization soon, and stronger formal verification progressively—without making any one optimizer, backend, host operating system, or early compiler implementation the definition of the language.
