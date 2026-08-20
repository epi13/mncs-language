# RFC 0031: Machine-Native Build, Derivation, Reproducibility, Artifact Lineage, Supply-Chain, and Attestation Semantics

- **Status:** Draft
- **Authors:** MNCS project
- **Target:** pre-1.0 research roadmap

## Summary

MNCS needs a semantic model for why an artifact is accepted as the result of a declared derivation rather than merely a file that happened to appear after a build command.

This RFC establishes build and supply-chain semantics around explicit artifact identities, derivation graphs, build executions, environment and influence closure, source and build provenance, hermeticity, reproducibility, independent reconstruction, bootstrap roots, attestations, transparency, software-composition metadata, publication authority, and assurance.

The primary constitutional rule is:

> **An artifact is not identified by its name, release version, signature, build job, or provenance statement. A machine-native artifact is related to an explicit derivation graph whose inputs, transformations, environment assumptions, outputs, provenance, and evidence can be independently reasoned about.**

The second rule is:

> **Artifact identity, derivation identity, build-execution identity, provenance identity, publication identity, and release/deployment identity are distinct.**

The third rule is:

> **A valid signature establishes an authenticated statement about an artifact or claim. It does not by itself establish that the artifact was correctly built, reproducible, safe, provenance-complete, policy-compliant, or authorized for use.**

The fourth rule is:

> **Reproducibility, hermeticity, provenance completeness, source governance, builder trust, bootstrap trust, transparency, and publication authorization are independent assurance dimensions.**

The machine-native objective is not to standardize one package manager, CI system, provenance format, attestation envelope, transparency service, or secure-build profile. Nix-like derivations, Bazel-style sandboxing, Reproducible Builds, SLSA, in-toto, SPDX, CycloneDX, DSSE, Sigstore, TUF, bootstrappable builds, diverse double compilation, and future mechanisms are realization and evidence families over a smaller semantic model.

Programs, components, release policies, and Forge should be able to state the derivational properties required for an artifact. Builders, planners, package systems, caches, remote workers, rebuilders, and publication services may then realize those requirements only when their witnesses satisfy the protected relations.

---

## 1. Scope

RFC 0031 owns the semantics of:

- artifact identity as it relates to derivation;
- source revision and source-provenance claims used as build inputs;
- derivation descriptions and resolved build plans;
- build-execution identity;
- declared and observed build influences;
- build-environment contracts;
- hermeticity/isolation claims;
- deterministic derivation claims;
- reproducible-build claims;
- independent and diverse rebuild evidence;
- source and build provenance;
- derivation traces and expected-vs-observed supply-chain graphs;
- build-platform and builder identities as referenced from RFC 0025;
- dependency, toolchain, influence, and bootstrap closures;
- bootstrap seeds and source-to-binary correspondence evidence;
- artifact composition and SBOM-style inventory relations;
- typed attestations over artifacts and derivation claims;
- transparency inclusion/consistency claims;
- artifact publication authorization metadata;
- build cache and derivation-to-artifact binding semantics;
- supply-chain counterexamples and deltas;
- Forge derivation search and narrow derivation verification.

This RFC does **not** own:

- generic principal, key, credential, authentication, cryptographic algorithm, attestation trust, or delegation semantics: RFC 0025;
- generic evidence algebra, trust policy, freshness, independence, or action authorization: RFC 0018;
- generic artifact equality/refinement/equivalence relations: RFC 0020;
- generic canonical serialization of derivations/provenance: RFC 0027;
- local durable cache/store behavior: RFC 0026;
- distributed remote-build consistency/failure semantics: RFC 0028;
- builder placement and hardware capability: RFC 0029;
- deployment, activation, finalization, rollback, or retirement of produced artifacts: RFC 0030;
- quantitative build/evidence/storage/energy cost: RFC 0032;
- multi-objective selection among valid build strategies: RFC 0033.

---

## 2. Constitutional distinctions

The following distinctions are normative design rules for the semantic model.

1. **Artifact identity is not release identity.**
2. **Artifact identity is not derivation identity.**
3. **Derivation identity is not build-execution identity.**
4. **Build execution is not provenance.**
5. **Provenance is not attestation.**
6. **Attestation is not signature.**
7. **Signature validity is not claim truth.**
8. **Claim truth is not authorization to use an artifact.**
9. **Source provenance is not build provenance.**
10. **Source review does not establish builder integrity.**
11. **Builder integrity does not establish source correctness.**
12. **SBOM/composition inventory is not provenance.**
13. **Provenance is not an SBOM.**
14. **Content digest identifies bytes under an algorithm, not their origin.**
15. **A mutable tag, branch, package name, or URL is not immutable artifact identity.**
16. **Declared dependencies are not necessarily the full build-influence closure.**
17. **Dependency closure is not toolchain closure.**
18. **Toolchain closure is not bootstrap closure.**
19. **Hermeticity is not reproducibility.**
20. **Reproducibility is not hermeticity.**
21. **Determinism is not independently verified reproducibility.**
22. **Repeated builds on one builder are not independent rebuilds.**
23. **Reproduction evidence must name the compared output set and relation.**
24. **Bit reproducibility is distinct from semantic equivalence.**
25. **A cache hit is artifact reuse, not provenance verification.**
26. **A sandbox is evidence for influence isolation, not proof of artifact correctness.**
27. **Undeclared external influence must not silently enter a strong derivation claim.**
28. **Compiler, linker, assembler, generators, interpreters, and build-system binaries are build inputs when they can influence output.**
29. **Generated source retains generator provenance.**
30. **Vendoring does not erase external lineage.**
31. **A bootstrap seed is a trust assumption, not an incidental implementation detail.**
32. **Omitted bootstrap roots remain trust roots.**
33. **Source/binary correspondence evidence does not prove benign source.**
34. **Independent rebuilders are independent only to the extent their relevant influences are independent.**
35. **Transparency inclusion proves logged existence/consistency under a log model, not artifact correctness or legitimacy.**
36. **Publication authorization is not build provenance.**
37. **Old authentic provenance may be stale or policy-inapplicable.**
38. **Signed metadata must bind its intended predicate/type.**
39. **Review, build, attest, sign, publish, promote, deploy, and finalize authorities are distinct.**
40. **Packaging creates a new artifact identity while retaining lineage to contained/derived artifacts.**
41. **Primary output, evidence output, and diagnostic output are distinct.**
42. **Private provenance may require a deliberately related public projection.**
43. **A successful build does not imply a complete provenance graph.**
44. **A reproduction mismatch does not by itself prove compromise.**
45. **A stronger supply-chain profile is action-relative; no universal scalar trust level is assumed.**
46. **Forge may search derivations and build plans; a narrower checker must decide whether protected derivation obligations are satisfied.**

---

## 3. Semantic planes

RFC 0031 organizes its concepts across nine planes.

### 3.1 Artifact plane

- `Artifact`
- `ArtifactRef`
- `ArtifactDigest`
- `ArtifactSet`
- `DerivationOutput`
- `PrimaryOutput`
- `EvidenceOutput`
- `DiagnosticOutput`
- `PackagingArtifact`

This plane identifies concrete outputs and content references without claiming their origin.

### 3.2 Derivation plane

- `Derivation`
- `ResolvedDerivation`
- `BuildPlan`
- `BuildStep`
- `BuildDependency`
- `ToolDependency`
- `GenerationDependency`
- `PackagingDependency`
- `VerificationDependency`

This plane describes the declared causal program expected to produce artifacts.

### 3.3 Environment and influence plane

- `BuildEnvironment`
- `DeclaredInfluence`
- `ObservedInfluence`
- `BuildInfluenceClosure`
- `HermeticityContract`
- `HermeticityObservation`
- `EnvironmentProjection`

This plane describes what may or did influence a derivation realization.

### 3.4 Lineage and bootstrap plane

- `SourceRevision`
- `GeneratedArtifact`
- `DerivationGraph`
- `DependencyClosure`
- `ToolchainClosure`
- `BootstrapChain`
- `BootstrapSeed`
- `BootstrapRoot`

This plane makes transitive lineage and unavoidable roots explicit.

### 3.5 Reproducibility plane

- `DeterminismClaim`
- `ReproducibilityClaim`
- `ReproductionExecution`
- `IndependentReproductionWitness`
- `DiversityProfile`
- `ReproductionMismatch`
- `ReproductionDelta`

This plane distinguishes deterministic execution from independent reconstruction evidence.

### 3.6 Provenance plane

- `SourceProvenance`
- `BuildProvenance`
- `ProvenanceStatement`
- `ExpectedDerivationGraph`
- `ObservedDerivationGraph`
- `DerivationTrace`

This plane records claims about how source or artifacts came to exist.

### 3.7 Attestation and publication plane

- `SupplyChainAttestation`
- `AttestationSubject`
- `AttestationPredicate`
- `TransparencyRecord`
- `PublicationAuthorization`
- `RepositoryMetadataClaim`

RFC 0025 remains authoritative for cryptographic/principal trust. This plane gives those statements supply-chain meaning.

### 3.8 Composition/inventory plane

- `SoftwareComposition`
- `ComponentRelation`
- `SBOMProjection`
- `LicenseMetadata`
- `VulnerabilityMetadata`

SPDX, CycloneDX, and similar formats may realize this plane but are not the canonical ontology.

### 3.9 Assurance plane

- `SupplyChainClaim`
- `DerivationWitness`
- `BootstrapWitness`
- `SupplyChainPolicyProfile`
- `SupplyChainCounterexample`
- `DerivationDelta`

RFC 0018 remains authoritative for appraisal and action authorization.

---

## 4. Artifact identity, derivation identity, and build-execution identity

A derivation is the declared semantic recipe for producing one or more artifacts.

```text
Derivation D
    inputs      = I
    tools       = T
    environment = E
    transform   = F
    outputs     = O
```

A build execution is one realization of that derivation:

```text
BuildExecution E1 realizes D
BuildExecution E2 realizes D
```

An artifact is a concrete output:

```text
Artifact A1 produced_by E1
Artifact A2 produced_by E2
```

Possible outcomes include:

```text
D same
E1 != E2
A1 byte-identical A2
```

and also:

```text
D1 != D2
A1 byte-identical A2
```

Therefore neither byte equality nor derivation equality may fabricate the other.

### 4.1 Proposed shape

```text
Derivation {
    id
    semantic_operation
    declared_inputs
    tool_inputs
    configuration_inputs
    environment_contract
    transformation
    outputs
    execution_requirements
}

BuildExecution {
    id
    derivation
    builder
    environment_observation
    started_at
    completed_at
    observed_inputs
    produced_outputs
    provenance
}

Artifact {
    id
    media_kind
    content_identity
    size?
}
```

### 4.2 Content addressing

`ArtifactDigest` identifies exact bytes under an explicit digest algorithm and profile:

```text
ArtifactDigest {
    algorithm
    bytes_digest
}
```

A digest alone does not identify:

- derivation;
- source revision;
- builder;
- provenance;
- publication authority;
- semantic artifact identity.

OCI-style digest-addressed descriptors and Merkle DAGs are useful realization precedents, not the complete semantic model.

### 4.3 Derivation digest

A resolved derivation may have a canonical RFC 0027 representation and digest:

```text
DerivationDigest {
    canonical_profile
    digest_algorithm
    digest
}
```

Changing an actual semantic input must alter the resolved derivation identity. Reformatting source build-script text should not necessarily do so.

`BuildScriptTextIdentity != ResolvedDerivationIdentity`.

---

## 5. Derivation graphs and typed dependency edges

Real build lineage is a DAG rather than a flat package list.

```text
Source S -----\
Compiler C ----> Compile -> Object ----\
Generator G --/                      Link -> Executable
Library L ---------------------------/
```

Edges must identify their semantic role.

Candidate relations include:

- `Consumes`
- `UsesAsTool`
- `GeneratedFrom`
- `LinksAgainst`
- `Packages`
- `Embeds`
- `Verifies`
- `Signs`
- `Publishes`
- `DerivedFrom`

A generic `depends_on` edge may exist as a projection, but high-assurance policies should reason over typed dependencies.

### 5.1 Build versus runtime dependency

A compiler can be essential to the derivation without being required at runtime.

A dynamically linked library may affect build and runtime behavior.

A test harness may affect promotion evidence without appearing in output bytes.

The canonical graph therefore distinguishes at least:

```text
BuildDependency
ToolDependency
RuntimeDependency
TestDependency
PackagingDependency
VerificationDependency
```

### 5.2 Generated artifacts

Generated source or generated metadata retains lineage:

```text
GeneratedArtifact {
    generator_artifact
    generator_derivation
    source_inputs
    parameters
    output
}
```

Checking generated source into a repository does not erase its known generator lineage.

### 5.3 Vendoring

Copying a dependency into a repository changes storage and potentially governance. It does not automatically erase external origin.

A vendored artifact may carry:

```text
DerivedFromExternalArtifact
```

where known.

---

## 6. Source revision and source provenance

Source content, source revision, and source governance are separate identities.

```text
SourceContentIdentity
SourceRevisionIdentity
SourceGovernanceIdentity
```

A content digest may identify source bytes.

A VCS revision may identify history/context under that VCS's semantics.

Source provenance may additionally establish claims such as:

- repository/trust-domain identity;
- protected branch or integration path;
- review requirements;
- authorized authors/reviewers;
- source-system policy revision;
- immutable revision identity.

SLSA 1.2's distinct Source and Build tracks are an important precedent for keeping source governance and build provenance separate.

### 6.1 Mutable source names

Names such as:

```text
main
latest
release
v1
```

are locators or mutable labels unless independently bound to an immutable revision.

Strong provenance should resolve mutable references before build execution:

```text
SourceRef {
    repository_identity
    immutable_revision
    content_digest?
}
```

### 6.2 Source approval does not authorize building

RFC 0025 should distinguish:

- source-author authority;
- reviewer authority;
- merge/integration authority;
- build authority;
- provenance-attestation authority;
- signing authority;
- publication authority;
- deployment/finalization authority.

RFC 0031 consumes those distinctions rather than collapsing them into `created_by`.

---

## 7. Declared versus observed build influence

A derivation's declared input set is a claim about causal influence.

A build execution may observe additional input.

```text
DeclaredInputClosure = {source, compiler, library}
ObservedInputClosure = {source, compiler, library, /etc/local-config}
```

The excess is an explicit counterexample:

```text
UndeclaredBuildInfluence {
    execution
    influence
    observation
}
```

### 7.1 Build influence closure

`BuildInfluenceClosure` is the transitive set of entities capable of influencing a build under a declared influence model.

It may include:

- source inputs;
- build scripts;
- generators;
- compiler/linker/assembler;
- package manager;
- build-system evaluator;
- environment variables;
- filesystem content;
- filesystem enumeration order;
- locale;
- timezone/clock observations;
- randomness sources;
- network resources;
- host kernel/runtime;
- CPU/device feature observations;
- credentials when their value influences output;
- remote build/cache services;
- people/organizations where operational control can affect execution.

A dependency manifest is not proof of complete influence closure.

### 7.2 Influence classes

Not every influence has the same meaning.

```text
ContentInfluence
ExecutionInfluence
AuthorityDependency
AvailabilityDependency
EvidenceDependency
```

A credential used only to fetch a content-addressed immutable artifact may authorize access without changing artifact content. A credential embedded into output is a content influence.

The difference should remain explicit.

---

## 8. Build environment semantics

`BuildEnvironment` is a semantic artifact describing relevant execution conditions, not a string such as `linux`.

```text
BuildEnvironment {
    execution_platform
    toolchain
    environment_variables
    locale
    timezone
    filesystem_contract
    network_policy
    time_source
    randomness_source
    kernel_runtime_dependencies
    hardware_feature_contract
}
```

Not every derivation needs every field.

### 8.1 Environment minimization

Forge may experimentally or formally establish that a variable is irrelevant to protected output identity:

```text
TimezoneIrrelevant
BuildPathIrrelevant
LocaleIrrelevant
HostKernelIrrelevant
```

Such claims are relation- and evidence-scoped.

They allow the resolved derivation to shrink its true dependence surface over time.

### 8.2 Host-dependent optimization

Implicit host probing such as `-march=native` is either:

- an undeclared influence; or
- an explicit derivation parameter.

It may not remain hidden while a strong reproducibility/derivation claim is made.

### 8.3 Time and randomness

Time is a build input only when the contract intentionally makes it one.

Accidental `__DATE__`-style dependence should be reported as influence.

Randomness may be:

- forbidden;
- seeded by an explicit deterministic input;
- permitted nondeterminism with a weaker reproducibility contract;
- cryptographic entropy that must not be determinized for security reasons.

RFC 0023 and RFC 0025 remain authoritative for time and cryptographic/security semantics.

---

## 9. Hermeticity

Hermeticity describes isolation from undeclared influence under an explicit profile.

A naked `hermetic=true` is insufficient.

```text
HermeticityContract {
    filesystem
    network
    environment
    clock
    randomness
    toolchain
    host_introspection
    external_services
}
```

Example profile:

```text
filesystem       = DECLARED_READS_ONLY
network          = DENIED
environment      = DECLARED_ONLY
clock            = VIRTUALIZED
randomness       = FIXED_OR_DECLARED
host_introspection = DENIED
```

### 9.1 Sandbox evidence

A sandbox may produce observations:

```text
HermeticityObservation {
    permitted_reads
    observed_reads
    permitted_writes
    observed_writes
    network_events
    environment_reads
    clock_reads
    randomness_reads
}
```

A narrow checker can verify:

```text
ObservedInfluences ⊆ PermittedInfluences
```

Bazel-style sandboxing is one realization precedent.

### 9.2 Hermeticity does not imply correctness

A malicious source can be built perfectly hermetically.

A deterministic compiler defect can reproducibly produce a wrong artifact.

Therefore hermeticity supports a derivational claim; it does not establish semantic correctness.

### 9.3 Fetch/build split

A useful realization family separates external acquisition from the hermetic build:

```text
FetchDerivation
    -> immutable ArtifactRef

BuildDerivation
    consumes ArtifactRef
    network denied
```

Nix-like fixed-output/content-addressed fetching is a precedent, not a requirement.

---

## 10. Determinism and reproducibility

The Reproducible Builds definition provides the strong reference meaning for `ReproducibleBuild`: given the same source, relevant environment, and build instructions, any party can recreate bit-for-bit identical copies of all specified artifacts.

MNCS preserves that strong meaning.

### 10.1 Deterministic derivation

```text
DeterministicDerivationClaim {
    derivation
    output_set
    quantified_environment
    relation
}
```

Conceptually:

```text
same complete derivation inputs
    -> same specified output bytes
```

This may be established formally only for narrow systems; commonly it is supported empirically.

### 10.2 Reproducibility claim

```text
ReproducibilityClaim {
    derivation
    specified_outputs
    comparison_relation = ByteIdentical
    environment_contract
}
```

`ByteIdentical` is the default and preferred meaning for an unqualified reproducible-build claim.

If another output relation is used, it must be named explicitly, e.g. `SemanticBuildEquivalence<R>`, and must not be relabeled as ordinary bit reproducibility.

### 10.3 Repeatability versus independent reproduction

```text
LocalRepeatability
IndependentReproduction
DiverseIndependentReproduction
```

are distinct evidence families.

One builder repeating itself three times is not three independent rebuilders.

### 10.4 Reproduction execution

```text
ReproductionExecution {
    derivation
    builder
    environment
    produced_outputs
    comparison_result
}
```

### 10.5 Independent reproduction witness

```text
IndependentReproductionWitness {
    derivation
    executions
    specified_outputs
    common_artifact_digests
    diversity_dependencies
}
```

RFC 0018 evaluates actual independence. Different machines may still share a compiler, cloud, container image, operator, base filesystem, or build service.

### 10.6 Threshold reproduction

Policies may require:

```text
2-of-N independent matching rebuilds
3-of-5 diverse rebuilds
```

The threshold is an assurance policy. It does not redefine reproducibility.

### 10.7 Reproduction mismatch

A mismatch is not automatically evidence of malicious compromise.

Possible causes include:

- missing provenance;
- undeclared environment input;
- nondeterministic tool;
- different source;
- different toolchain;
- build defect;
- builder compromise;
- reproducer defect.

The result should therefore be:

```text
ReproductionMismatch
```

plus a diagnostic/counterexample rather than an ungrounded cause assertion.

### 10.8 Reproduction delta

Forge may compare mismatching artifacts at structured boundaries:

- archive ordering;
- timestamps;
- paths;
- sections;
- symbols;
- object members;
- compiler metadata;
- generated files;
- package metadata.

```text
ReproductionDelta {
    expected
    observed
    localized_differences
    candidate_influences
}
```

The delta is diagnostic evidence, not proof of causation by itself.

---

## 11. Build provenance

Build provenance is structured evidence/claim material describing how an artifact was produced.

```text
BuildProvenance {
    subject_artifacts
    source_materials
    resolved_derivation
    builder_identity
    build_execution
    environment_claims
    dependency_materials
    produced_at
    evidence
}
```

SLSA provenance is a realization precedent.

### 11.1 Provenance is a claim

A provenance document can be:

- authentic and true;
- authentic and false;
- unauthentic but accidentally true;
- incomplete;
- stale;
- inapplicable to the requested policy.

Therefore provenance is never promoted directly to truth merely because its signature verifies.

### 11.2 Provenance generation trust

Strong provenance should identify the platform/process that generated it and what could alter the provenance generation path.

A build process that can arbitrarily rewrite its own provenance offers weaker evidence than an isolated platform-generated provenance channel.

### 11.3 Expected versus observed derivation

in-toto-style planned supply chains motivate:

```text
ExpectedDerivationGraph
ObservedDerivationGraph
```

and a relation:

```text
TraceConformsToDerivationPolicy
```

A policy may require:

- exact steps;
- authorized builder principals;
- material/product identities;
- ordering constraints;
- mandatory verification gates;
- no undeclared transformation.

An unexpected `UnknownTransform` is a supply-chain deviation even when final tests pass.

---

## 12. Typed supply-chain attestations

RFC 0025 owns attestation trust semantics. RFC 0031 defines supply-chain predicate meanings.

```text
SupplyChainAttestation<Predicate> {
    subject
    predicate
    issuer
    evidence
}
```

Predicate classes may include:

- `BuildProvenancePredicate`
- `SourceProvenancePredicate`
- `HermeticityPredicate`
- `TestResultPredicate`
- `ReproductionPredicate`
- `SBOMPredicate`
- `PublicationPredicate`

A `TestResultPredicate` must not satisfy a `BuildProvenancePredicate` merely because both are authenticated JSON.

### 12.1 Envelope type binding

DSSE-style signing of both payload type and payload bytes is a useful precedent for avoiding cross-type interpretation.

MNCS itself remains format-neutral and relies on RFC 0027 for canonical serialization.

### 12.2 Signature scope

RFC 0025 determines whether a signature is cryptographically valid and whether the issuer is accepted for a particular claim class.

RFC 0031 determines what supply-chain claim the signed object makes.

RFC 0018 determines whether that evidence is sufficient for an action.

---

## 13. Transparency

Transparency systems such as Rekor can provide append-only/publicly auditable evidence that an artifact or attestation was logged under a given identity and log model.

RFC 0031 should represent:

```text
TransparencyRecord {
    subject
    statement_identity
    log_identity
    inclusion_witness
    consistency_witness?
    observation_time
}
```

### 13.1 Transparency is not correctness

A malicious artifact can be:

```text
SignatureValid         = true
TransparencyIncluded  = true
ArtifactPolicyAccepted = false
```

That is a required pilot fixture.

### 13.2 Transparency is not artifact immutability

A content-addressed artifact may be immutable yet untrusted.

A transparency record may be immutable yet accurately record a bad event.

`Immutable`, `Authentic`, `Transparent`, `Correct`, and `Authorized` remain different claims.

### 13.3 Confidential provenance

Transparency/publication requirements must compose with RFC 0024.

A project may need:

```text
PrivateProvenance
PublicProvenanceProjection
```

with a relation specifying exactly what was redacted or generalized.

Redaction must not transform `withheld` into misleading `absent` semantics.

---

## 14. Software composition and SBOM semantics

Software composition inventories answer what components, packages, files, licenses, vulnerabilities, or relationships are present or associated with an artifact.

They are not build provenance.

```text
SoftwareComposition {
    subject
    components
    relations
    metadata
}
```

SPDX 3.x and CycloneDX are strong foreign realization candidates.

### 14.1 Composition relation

Potential relations include:

- `Contains`
- `DependsOn`
- `BuildDependsOn`
- `RuntimeDependsOn`
- `GeneratedFrom`
- `PackageOf`
- `VariantOf`
- `PatchOf`

### 14.2 SBOM completeness

An SBOM completeness claim must name its scope:

```text
CompositionCompleteness {
    subject
    component_classes
    dependency_depth
    evidence
}
```

No universal `complete=true` exists without an inventory model.

### 14.3 Provenance and SBOM composition

An SBOM can be attached as one derivation/evidence artifact.

Build provenance can name an SBOM output.

Neither should be inferred from the other.

---

## 15. Dependency, toolchain, influence, and bootstrap closure

Four closures should remain distinct.

### 15.1 Dependency closure

All transitive artifact/package/component dependencies relevant under a declared dependency relation.

### 15.2 Toolchain closure

The transitive set of tools capable of transforming build inputs into outputs:

```text
compiler
linker
assembler
generator
interpreter
build evaluator
packager
```

### 15.3 Influence closure

The broader transitive set of entities capable of influencing build output or provenance under the declared model.

### 15.4 Bootstrap closure

The chain of tools/artifacts required to construct the toolchain itself back to explicit bootstrap seeds/roots.

`AllDependenciesPinned` does not imply `InfluenceClosureComplete`.

`ToolchainClosureKnown` does not imply `BootstrapClosureSourceReconstructed`.

---

## 16. Bootstrap trust

Compiler/toolchain provenance recursively depends on earlier toolchains.

```text
BootstrapSeed B0
    -> Compiler C0
    -> Compiler C1
    -> Compiler C2
    -> CurrentCompiler C3
```

MNCS must expose the point at which source-derived reasoning terminates in an assumed binary/hardware/manual root.

### 16.1 Bootstrap seed

```text
BootstrapSeed {
    artifact
    justification
    provenance?
    reproduction_evidence?
    inspection_evidence?
    trust_policy_scope
}
```

A seed may be accepted. It may not be silently omitted.

### 16.2 Source-derived claim

A claim such as `BuiltFromSource` must identify the start of source reconstruction.

Building C3 from C2 does not make the full chain source-derived when C2 is opaque.

### 16.3 Bootstrappable-build direction

Bootstrappable-build techniques that reduce opaque binary seeds are valuable realizations.

The semantic objective is to reduce, expose, and independently appraise roots—not to claim all physical trust can be eliminated.

### 16.4 Trusting Trust and diverse double compilation

Diverse Double Compilation (DDC) can provide evidence that a compiler executable corresponds to claimed source under explicit assumptions.

A narrow artifact may be:

```text
CompilerCorrespondenceClaim {
    compiler_source
    compiler_binary
    diverse_compiler
    comparison_relation
    assumptions
}
```

If DDC succeeds, it supports source/binary correspondence.

It does not establish:

- that compiler source is benign;
- that the diverse compiler is bug-free;
- that hardware is trustworthy;
- that the final program source is safe.

That limited scope is a feature, not a defect.

---

## 17. Builder and build-platform identity

A build service name is not enough.

Strong claims may need:

```text
BuilderIdentity {
    principal
    platform_identity
    runner_image
    builder_software
    isolation_profile
    policy_revision
    execution_instance?
}
```

RFC 0025 supplies principal/workload/platform identities and attestation.

RFC 0029 supplies physical placement/capability evidence.

### 17.1 Builder attestation

A high-assurance policy may require evidence that a builder is currently running an approved image/runtime.

`BuilderAttested` does not imply `BuildCorrect`.

It establishes a scoped state claim feeding build-provenance appraisal.

### 17.2 Common-mode dependence

If the same CI service:

- builds;
- generates provenance;
- runs tests;
- signs attestations;
- approves release;

five statements may still share one compromise domain.

RFC 0031 must expose common build/evidence producers so RFC 0018 can refuse false independence.

---

## 18. Independent and diverse rebuilders

Independent reconstruction is one of the strongest practical cross-checks between claimed source/derivation and distributed artifact bytes.

### 18.1 Independence dimensions

A `DiversityProfile` may describe differences/shared dependencies in:

- physical worker;
- operating system;
- compiler binary;
- compiler implementation;
- build-system implementation;
- base image;
- cloud/provider;
- organization/operator;
- architecture;
- package mirror;
- source acquisition path.

No single diversity score is assumed.

### 18.2 Provenance/reproduction consistency

When provenance declares:

```text
Source S
Compiler C
Flags F
```

and an independent reproducer resolves that derivation and generates the same bytes, MNCS may create:

```text
ProvenanceReproductionConsistency
```

This is stronger evidence that the declared derivation is operationally sufficient for the reproduced artifact than either provenance or self-repetition alone.

---

## 19. Build caches and derivation-artifact bindings

Remote/local caches are realization mechanisms for reusing earlier build executions.

A cache lookup conceptually maps:

```text
ResolvedDerivationIdentity -> CandidateArtifactSet
```

A cache hit is not automatically trusted.

### 19.1 Cache reuse obligations

Before reuse, policy may require:

- artifact digest verification;
- derivation identity match;
- required provenance;
- builder acceptability;
- freshness/applicability;
- reproduction evidence;
- cache-origin policy.

### 19.2 Cache poisoning

A cache can contain valid bytes bound to the wrong derivation key.

Therefore:

```text
DerivationArtifactBinding {
    derivation
    outputs
    provenance
    evidence
}
```

is a first-class relation.

This differs from ordinary byte corruption.

### 19.3 Reuse does not erase original execution identity

When a cached artifact is reused, the current build may have:

```text
CacheReuseExecution
```

but the artifact's producing `BuildExecution` remains the original execution referenced by provenance.

---

## 20. Packaging and multi-output derivations

A derivation may emit multiple outputs:

```text
binary
headers
debug symbols
documentation
SBOM
provenance
```

Each output has its own artifact identity.

One execution may produce an `ArtifactSet`.

### 20.1 Primary versus evidence outputs

Reproducibility claims name the specified primary outputs they compare.

Build logs and diagnostic traces are commonly evidence/diagnostic outputs and need not be byte-identical unless the derivation explicitly requires it.

### 20.2 Packaging creates lineage

```text
Executable E
    -> Tar T
    -> Package P
    -> ContainerLayer L
```

Each is a distinct artifact.

Relationships preserve containment/derivation:

```text
PackageOf(P,E)
Contains(P,E)
DerivedFrom(P,E)
```

as appropriate.

### 20.3 Node-local reproducibility

The compiler step may be reproducible while packaging is not due to timestamps or archive ordering.

Reproducibility claims therefore attach to specific derivation outputs/nodes rather than a vague project-wide Boolean.

---

## 21. Signing and external secret-bearing steps

Signing usually should be modeled as a separate derivation/effect from the reproducible build when nondeterministic signing material would alter output bytes.

```text
BuildDerivation -> UnsignedArtifact
SigningDerivation -> SignedDistributionArtifact
```

RFC 0025 owns key authority and signature semantics.

### 21.1 Access secrets versus content secrets

A secret credential used to authorize retrieval of immutable input may be an `AuthorityDependency` without being an output-content influence.

A secret embedded in the artifact is a `SecretInputInfluence`.

The latter affects reproducibility, confidentiality, and artifact semantics directly.

### 21.2 No secret smuggling

A build must not claim ordinary public reproducibility when undeclared secret material changes the compared artifact bytes.

---

## 22. Publication authorization and update metadata

Build provenance answers where an artifact came from.

Publication authorization answers whether a particular artifact is currently authorized for distribution under a repository/update policy.

TUF-style Root/Targets/Snapshot/Timestamp roles are a useful realization family for:

- delegated publication authority;
- repository metadata integrity;
- freshness;
- rollback/freeze resistance;
- target selection.

RFC 0031 therefore reserves:

```text
PublicationAuthorization {
    artifact
    repository_namespace
    authorized_by
    validity
    metadata_generation
}
```

Publication authorization still does not mean RFC 0030 may deploy the artifact.

### 22.1 Freshness

Old valid metadata may be stale.

RFC 0023 supplies temporal validity; RFC 0025 supplies credential/revocation semantics; RFC 0018 appraises current applicability.

---

## 23. Supply-chain policy profiles

MNCS should not hard-code one SLSA level or one scalar supply-chain grade.

Policies should be predicates over the lineage/evidence graph.

Example:

```text
SupplyChainPolicyProfile {
    source:
        protected_revision
        two_authorized_reviews

    derivation:
        immutable inputs
        declared toolchain

    build:
        isolated builder
        no undeclared network

    provenance:
        platform_generated
        authenticated
        typed

    reproduction:
        two independent byte-identical rebuilds

    bootstrap:
        approved seed set

    publication:
        fresh delegated authorization
}
```

SLSA Build/Source tracks may be imported as named profiles or mapped into these predicates.

A system can legitimately have:

```text
strong source governance
strong reproducibility
weak bootstrap evidence
partial provenance completeness
```

without forcing those dimensions into one score.

### 23.1 Action-relative policy

The same artifact may be authorized for:

```text
STORE
INSPECT
SANDBOX_EXECUTE
TEST
```

while rejected for:

```text
PUBLISH
PROMOTE
DEPLOY_PRODUCTION
BOOTSTRAP_TRUST_ROOT
```

RFC 0018 remains authoritative for the final decision.

---

## 24. Supply-chain relation family

RFC 0020 should host mechanics; RFC 0031 defines domain relation kinds including:

```text
ProducedBy
DerivedFrom
GeneratedFrom
Packages
Contains
UsesAsTool
BuildDependsOn
RuntimeDependsOn
ConformsToDerivation
DerivationArtifactBindingValid
SourceProvenanceValidFor
BuildProvenanceValidFor
HermeticUnder
DeterministicUnder
ReproducibleUnder
IndependentlyReproducedUnder
BootstrapDerivedFrom
CompilerCorrespondsToSourceUnder
TransparencyIncludedUnder
PublicationAuthorizedUnder
CompositionCompleteUnder
```

These relations are directional/scoped where appropriate.

### 24.1 Artifact byte identity versus semantic relation

Two builds may be:

```text
ByteIdentical
```

or only:

```text
BehaviorallyEquivalent<R>
```

The latter may be useful for compiler validation studies but does not satisfy an unqualified reproducible-build requirement.

### 24.2 Provenance completeness

No universal completeness Boolean exists.

```text
ProvenanceCompletenessClaim {
    influence_model
    graph_scope
    conclusion = COMPLETE | PARTIAL | UNKNOWN
}
```

---

## 25. Build system/evaluator trust

Build recipes are interpreted by software.

If the evaluator interprets a build definition maliciously or incorrectly, source build rules do not suffice.

Potential trusted-boundary artifacts include:

```text
DerivationEvaluator
DerivationChecker
```

Their versions and own derivations must be explicit.

This recursively exposes bootstrap/toolchain trust rather than hiding it.

### 25.1 Build plan proposal versus checking

Forge, LLMs, heuristic planners, Nix/Bazel generators, or future search systems may propose build plans.

Proposal generation remains outside the narrow trusted checker.

The checker validates:

- required inputs resolved;
- no forbidden mutable references;
- tool capabilities valid;
- builder requirements satisfied;
- output identities well-formed;
- policy constraints represented;
- derivation graph acyclic or intentionally recursive under explicit semantics;
- relevant cross-RFC obligations preserved.

Search difficulty may be high. Witness checking should remain substantially narrower where possible.

---

## 26. Derivation deltas and incremental assurance

Forge should represent exactly how a candidate derivation changed.

```text
DerivationDelta {
    source_changes
    dependency_changes
    tool_changes
    environment_changes
    build_policy_changes
    provenance_changes
    bootstrap_changes
    output_changes
}
```

Example:

```text
Source changed:            yes
Compiler changed:          no
Dependency changed:        yes
Build environment changed: no
Policy changed:            no
Artifact digest changed:   yes
```

### 26.1 Dependency-aware invalidation

If evidence for compiler C becomes stale/refuted, only artifacts whose relevant lineage depends on C should lose claims.

This reuses RFC 0018 dependency invalidation.

A supply-chain graph should improve precision, not trigger global invalidation.

---

## 27. Information-flow and privacy

Source provenance, SBOMs, build logs, platform identities, dependency graphs, and attestations may reveal sensitive information.

RFC 0024 applies to all RFC 0031 artifacts.

Possible objects:

```text
PrivateProvenance
PublicProvenanceProjection
```

A projection must preserve an explicit relation to the private record while making redaction visible.

Sensitive fields may be:

```text
WITHHELD
```

rather than falsely represented as absent.

### 27.1 Transparency boundaries

A requirement for public transparency cannot override RFC 0024 declassification policy.

A private/internal transparency realization may be required instead.

---

## 28. Cross-RFC obligations

### RFC 0004

Derivation candidates, supply-chain repairs, and reproduction debugging may participate in recursive refinement.

### RFC 0008

Filesystem, network, secret retrieval, signing, cache access, and publication are effects/capabilities.

### RFC 0014

Component/module dependency semantics feed artifact/component build lineage but remain owned by RFC 0014.

### RFC 0017

Runtime/target/environment facts are consumed as build-environment evidence.

### RFC 0018

Owns evidence algebra, trust, independence, freshness, appraisal, and action authorization.

### RFC 0020

Owns equality/refinement/equivalence mechanics used by artifact/reproduction relations.

### RFC 0023

Owns timestamps, freshness, validity intervals, and time-derived build inputs.

### RFC 0024

Owns confidentiality/integrity flows across source/provenance/SBOM/attestation artifacts.

### RFC 0025

Owns builder/signer/publisher identity, credentials, signing, attestation trust, delegation, and cryptographic policy.

### RFC 0026

Owns local crash-safe artifact stores, caches, journals, and build-state persistence.

### RFC 0027

Owns canonical serialization of derivations, provenance, SBOM projections, attestations, and artifact references.

### RFC 0028

Owns distributed/remote builder, cache, repository, and reproduction failure semantics.

### RFC 0029

Owns builder placement, hardware capabilities, isolation/fault-domain placement, and topology evidence.

### RFC 0030

Owns release/deployment lifecycle use of artifacts. RFC 0031 provides provenance/derivation evidence consumed by deployment policy.

### RFC 0032

Owns quantitative build time, CPU, storage, network, evidence, energy, and rebuild cost.

### RFC 0033

Owns multi-objective selection among semantically admissible derivation/supply-chain strategies.

---

## 29. Realization families

RFC 0031 deliberately permits competing realizations.

### 29.1 Derivation/build realization families

- Nix-like functional derivations;
- Bazel-like declared-action graphs and sandboxing;
- conventional build systems wrapped by observed-input isolation;
- content-addressed build graphs;
- remote execution systems;
- Forge-generated build DAGs.

### 29.2 Provenance/attestation realization families

- SLSA provenance predicates;
- in-toto layouts/link metadata;
- DSSE-like typed envelopes;
- Sigstore-like identity-bound signing/transparency;
- MNCS-native canonical provenance artifacts.

### 29.3 Composition realization families

- SPDX 3.x;
- CycloneDX;
- OCI annotations/index relationships;
- MNCS-native composition graphs.

### 29.4 Publication realization families

- TUF-style delegated repository metadata;
- content-addressed registries;
- signed release manifests;
- private policy repositories.

### 29.5 Bootstrap realization families

- conventional trusted binary seeds;
- reduced bootstrappable seed chains;
- manually audited roots;
- diverse double compilation;
- independently reconstructed toolchains.

None of these realizes every RFC 0031 property automatically.

---

## 30. Micro-verifier opportunities

RFC 0031 should expose narrow verifier classes.

### 30.1 Derivation closure verifier

Checks that every referenced input/tool resolves to an immutable identity and expected dependency class.

### 30.2 Hermeticity verifier

Checks observed filesystem/network/environment/time/randomness influence against the declared contract.

### 30.3 Derivation trace verifier

Checks observed build steps/materials/products against expected derivation policy.

### 30.4 Artifact-binding verifier

Checks that artifacts/digests correspond to the claimed build execution and derivation.

### 30.5 Reproduction verifier

Checks byte identities for specified outputs across declared independent executions.

### 30.6 Diversity/independence analyzer

Produces dependency/common-mode evidence for RFC 0018; it does not claim independence merely from different worker IDs.

### 30.7 Bootstrap-chain verifier

Checks toolchain lineage terminates in explicit accepted seeds rather than hidden roots.

### 30.8 DDC witness checker

Checks a narrowly defined diverse-double-compilation witness under declared assumptions.

### 30.9 Attestation predicate verifier

Ensures signed metadata predicate/type matches the policy claim it is being used to satisfy.

### 30.10 Transparency verifier

Checks inclusion/consistency witness under an explicit transparency-log identity/model.

### 30.11 Publication metadata verifier

Checks artifact target identity, freshness, delegation and threshold requirements under a publication policy.

Each verifier emits method- and scope-specific evidence. None should return a universal `supply_chain_secure` bit.

---

## 31. Bounded implementation pilot

The first implementation should remain deliberately small.

### 31.1 Initial artifact vocabulary

Implement versioned representations for:

```text
Artifact
ArtifactRef
SourceRevision
Derivation
ResolvedDerivation
BuildExecution
BuildEnvironment
DeclaredInput
ObservedInput
BuildProvenance
HermeticityClaim
ReproducibilityClaim
ReproductionWitness
BootstrapSeed
BootstrapChain
SupplyChainCounterexample
DerivationDelta
```

Optional second increment:

```text
ExpectedDerivationGraph
ObservedDerivationGraph
SupplyChainAttestation
TransparencyRecord
SoftwareComposition
PublicationAuthorization
```

### 31.2 Fixture A — same derivation, different execution

```text
D same
E1 != E2
A1 digest == A2 digest
```

Expected:

```text
DerivationIdentity same
BuildExecutionIdentity different
ArtifactByteIdentity same
```

### 31.3 Fixture B — different derivations, same bytes

Two different declared derivations intentionally emit the same fixed artifact.

Expected:

```text
ArtifactByteIdentity same
DerivationIdentity different
```

This proves content identity does not establish origin.

### 31.4 Fixture C — undeclared input

Declared:

```text
src.c
compiler C
```

Observed:

```text
src.c
compiler C
/etc/local-config
```

Expected:

```text
HermeticityClaim REFUTED
UndeclaredBuildInfluence emitted
```

### 31.5 Fixture D — repeatable but not independently reproducible

Builder A repeats and produces identical bytes twice.

Builder B resolves the same derivation and produces different bytes.

Expected:

```text
LocalRepeatability SUPPORTED
IndependentReproducibility REFUTED or CONFLICTED according to claim graph
```

No universal reproducibility claim.

### 31.6 Fixture E — independent reproduction

Two sufficiently distinct builders produce byte-identical specified outputs from one resolved derivation.

Expected:

```text
IndependentReproductionWitness
```

with shared dependencies visible to RFC 0018.

### 31.7 Fixture F — signature valid, provenance incomplete

Artifact signature verifies.

Provenance omits required compiler identity.

Expected:

```text
SignatureValid PASS
ProvenanceCompleteness FAIL/PARTIAL
ProductionPromotion REJECTED
```

### 31.8 Fixture G — source approved, builder unauthorized

Source-provenance policy passes.

Build execution uses a principal not authorized for build provenance.

Expected:

```text
SourcePolicy AUTHORIZED
BuildPolicy REJECTED
```

### 31.9 Fixture H — explicit bootstrap root

```text
C3 <- C2 <- C1 <- B0
```

where `B0` is opaque.

Expected:

```text
BootstrapRoot = B0
```

and refusal to claim complete source reconstruction beyond that root.

### 31.10 Fixture I — DDC correspondence

Given compiler source S, compiler binary C, and diverse compiler T, provide a bounded DDC witness that supports source/binary correspondence.

Expected:

```text
CompilerCorrespondence SUPPORTED
CompilerSourceBenign UNSUPPORTED
```

### 31.11 Fixture J — transparency without legitimacy

A deliberately disallowed artifact is correctly signed and correctly transparency-logged.

Expected:

```text
Signature PASS
Transparency PASS
ArtifactUse REJECTED
```

### 31.12 Fixture K — cache poisoning

Cache key claims derivation D but maps to artifact M produced by another derivation.

Expected:

```text
ArtifactDigest valid
DerivationArtifactBinding REFUTED
CacheReuse REJECTED
```

### 31.13 Fixture L — packaging-local reproducibility

Compile step is reproducible.

Package step embeds nondeterministic timestamps.

Expected:

```text
CompileReproducibility SUPPORTED
PackageReproducibility REFUTED
```

No project-wide scalar result.

### 31.14 Fixture M — provenance projection

Private provenance contains an internal builder path/identity field.

Public projection redacts it explicitly.

Expected:

```text
field = WITHHELD
```

not silently absent, with a checked projection relation.

---

## 32. First Forge studies

### 32.1 Build the MNCS validator/toolchain under a supply-chain contract

Declare:

```text
BuildContract {
    source:
        immutable revision
        approved source provenance

    derivation:
        immutable resolved inputs
        explicit compiler/toolchain closure

    build:
        no undeclared filesystem inputs
        no undeclared network

    provenance:
        builder-generated
        typed
        authenticated

    reproduction:
        >= 2 sufficiently independent builders
        byte-identical primary artifact

    bootstrap:
        explicit compiler chain and accepted roots

    promotion:
        required claims supported

    realization:
        unresolved
}
```

Candidate realizations may include:

- ordinary local build;
- containerized build;
- sandboxed declared-input build;
- Nix-like functional derivation;
- remote builder;
- independent multi-builder reproduction.

Possible result:

```text
Local build:
    fast
    undeclared host inputs -> invalid

Container build:
    improved isolation
    toolchain lineage incomplete -> unresolved/rejected for promotion

Hermetic single builder:
    derivation valid
    reproduction policy incomplete

Hermetic + independent rebuild:
    artifact digest matches
    required reproduction evidence satisfied
```

The artifact bytes may remain unchanged while assurance about origin evolves.

### 32.2 Poisoned derivation experiment

Expected:

```text
Approved Source S
Approved Derivation D
```

Observed:

```text
Compile
UnknownTransform X
Package
```

Even if tests pass and the final artifact is correctly signed:

```text
TraceConformsToDerivationPolicy REFUTED
Promotion REJECTED
```

### 32.3 Reproducibility-debugging experiment

Run the same resolved derivation across workers while varying:

- filesystem path;
- timezone;
- locale;
- host kernel;
- parallelism;
- filesystem enumeration;
- tool patch version.

On mismatch, emit `ReproductionDelta`, infer candidate hidden influences, refine the derivation/environment contract, and repeat within a bounded RFC 0004 search budget.

This creates a machine-native reproducible-build debugger rather than merely a pass/fail script.

### 32.4 Supply-chain strategy search

Forge may compare semantically admissible combinations such as:

- one highly attested builder;
- two independent hermetic rebuilders;
- three less-diverse rebuilders;
- stronger bootstrap reconstruction;
- transparency + provenance;
- private provenance + internal transparency.

RFC 0031 decides semantic admissibility/evidence shape.

RFC 0032/0033 later quantify and rank cost, latency, energy, independence, and operational burden.

---

## 33. 1.0 research-threshold requirements

RFC 0031 should add the following requirements to the MNCS 1.0 research threshold.

A 1.0 research system should include:

- a versioned artifact/derivation model distinguishing artifact, derivation, build execution, provenance, publication, release, and deployment identities;
- a versioned derivation graph with typed source/build/tool/generation/packaging/verification dependency edges;
- explicit immutable source/artifact references for claims that require stable identity;
- a versioned build-environment/influence model distinguishing declared inputs from observed influence closure;
- an explicit hermeticity contract with at least filesystem/network/environment dimensions and independently checked observations;
- a versioned reproducibility model distinguishing deterministic derivation, local repeatability, independent reproduction, and diversity/common-dependency evidence;
- a versioned source/build provenance model that refuses to equate source governance with build provenance;
- typed supply-chain attestations whose predicates cannot be substituted across claim classes merely because signatures verify;
- a versioned dependency/toolchain/bootstrap closure model with explicit bootstrap seeds/roots;
- at least one fixture where the same derivation has different build-execution identities but byte-identical outputs;
- at least one fixture where different derivations produce the same bytes while derivation identity remains distinct;
- at least one undeclared-build-influence fixture that refutes a hermeticity claim;
- at least one study where local repeatability succeeds but independent reproducibility fails;
- at least one study with an independently checked byte-identical reproduction witness across builders whose shared dependencies remain visible;
- at least one case where artifact signature validity passes while provenance completeness or builder authority fails and promotion is rejected;
- at least one source/build separation case where source policy passes while build policy fails;
- at least one bootstrap study that terminates in an explicit opaque seed/root instead of claiming source derivation beyond available evidence;
- at least one source/binary correspondence experiment, such as DDC or another independently checkable witness, whose result remains narrowly scoped and does not claim source correctness;
- at least one transparency fixture where signature and inclusion checks pass while the artifact remains unauthorized for use;
- at least one cache-binding fixture where valid bytes are rejected because they are associated with the wrong derivation;
- at least one node-local reproduction study where compilation is reproducible while packaging is not, preserving separate claims;
- at least one private/public provenance projection with explicit withholding semantics under RFC 0024;
- at least one expected-vs-observed derivation-graph conformance study that detects an undeclared transformation even when ordinary functional tests pass;
- at least one dependency-aware invalidation study where stale/refuted toolchain evidence invalidates only artifacts that depend on that toolchain;
- at least one Forge derivation study where a faster/easier build candidate is rejected because it violates a protected hermeticity, provenance, bootstrap, identity, reproduction, or builder-authority relation;
- at least one untrusted derivation/build-plan proposal whose witness is independently checked by narrower verifiers;
- at least one bounded reproducibility experiment that remains explicitly bounded evidence rather than being upgraded to a universal deterministic-build theorem.

---

## 34. Theory and standards synthesis

RFC 0031 should carry forward the following theory stack while remaining implementation-neutral.

### 34.1 Functional package management / Nix derivations

Use as precedent for:

- explicit derivations;
- immutable input identities;
- dependency realization;
- content-addressed outputs;
- multiple outputs;
- build/store separation.

MNCS generalizes beyond package management into semantic build lineage and assurance.

### 34.2 Hermetic build systems / Bazel-style sandboxing

Use as precedent for:

- declared-input closure;
- sandboxed execution;
- remote-cache correctness;
- undeclared-input detection.

### 34.3 Reproducible Builds

Use the standard strong definition for unqualified reproducibility:

- same source;
- same relevant environment;
- same build instructions;
- independently recreated;
- bit-for-bit identical specified artifacts.

### 34.4 SLSA 1.2

Use as precedent for:

- separate Source and Build tracks;
- provenance as a supply-chain artifact;
- builder/platform provenance properties;
- incremental policy profiles.

MNCS avoids adopting one global SLSA level as its complete semantic model.

### 34.5 in-toto

Use as precedent for:

- expected supply-chain layouts;
- authorized functionaries/builders;
- materials/products;
- observed-step conformance.

### 34.6 DSSE-style typed envelopes

Use as precedent for binding signed payload bytes to predicate/payload type.

### 34.7 OCI content-addressed DAGs

Use as precedent for immutable artifact references, descriptors, manifests, packaging DAGs, and registry transport.

### 34.8 SPDX and CycloneDX

Use as foreign composition/inventory/provenance representations, not the language ontology.

### 34.9 Sigstore/Fulcio/Rekor-style identity and transparency

Use as precedent for:

- short-lived identity-bound signing;
- transparency logging;
- auditable signature/attestation presence.

Keep transparency distinct from correctness/trust.

### 34.10 TUF-style delegated publication trust

Use as precedent for:

- delegated artifact publication authority;
- target metadata;
- freshness;
- rollback/freeze protections.

Keep publication distinct from build provenance and deployment authorization.

### 34.11 Bootstrappable Builds

Use as precedent for shrinking opaque bootstrap seeds and making source-reconstruction chains explicit.

### 34.12 Trusting Trust / Diverse Double Compilation

Use as high-assurance theory for source-to-compiler-binary correspondence under explicit assumptions.

### 34.13 Independent/diverse rebuilders

Use as an empirical cross-check on derivation/provenance claims with RFC 0018 common-mode-dependency analysis.

### 34.14 Proof-producing derivation verification

MNCS extends the theory stack by requiring:

```text
untrusted build/search/planning
        -> witness
        -> deterministic/narrow checker
```

where feasible.

---

## 35. Non-goals

RFC 0031 does not claim:

- that every build can be perfectly hermetic;
- that every artifact can be bit-reproducible;
- that reproducibility proves source correctness;
- that signed provenance is necessarily true;
- that public transparency is appropriate for confidential supply chains;
- that all binary bootstrap trust can be eliminated;
- that SBOM completeness implies provenance completeness;
- that one standard/tool captures all supply-chain semantics;
- that a provenance graph can prove physical hardware free from compromise;
- that empirical rebuild agreement is a universal proof of compiler correctness;
- that a successful supply-chain profile authorizes deployment independently of RFC 0030/RFC 0018.

The goal is explicit, composable, evidence-bearing semantics for what is known, assumed, reproduced, signed, traced, and authorized.

---

## 36. Design conclusion

Conventional build pipelines often reduce the supply-chain question to:

```text
Did CI succeed?
Is the artifact signed?
```

A machine-native system should instead ask:

```text
What exact semantic derivation should produce these bytes?

Which inputs were declared?
Which influences were actually observed?
Which tools produced those tools?
Where does bootstrap trust terminate?
Which source governance claims apply?
Who asserts the build provenance?
What authority did that principal possess for this claim class?
Could the build alter its own provenance?
Can an independent system reproduce the specified artifact?
How independent was that reproducer?
Does observed execution conform to the expected derivation graph?
Does current policy authorize this exact action?
```

Only then should RFC 0030 ask whether the artifact may become part of an authoritative running system.

The defining statement for RFC 0031 is therefore:

> **Software supply-chain trust is not a signature on an artifact. It is an evidenced relation between an artifact and the derivation history we are willing to rely upon.**

The machine-native formulation is:

> **Programs and policies specify the derivational facts and assurance an artifact must possess. Machines may choose builders, caches, package systems, attestation formats, reproducibility strategies, bootstrap paths, and publication mechanisms only when the resulting lineage graph establishes the required relation from accepted roots and declared inputs to exact output.**

---

## References and research directions

Primary theory and standards to carry into implementation research include:

- Nix derivation/store model and functional package management: https://nix.dev/ and https://releases.nixos.org/nix/
- Bazel sandboxing and hermetic builds: https://bazel.build/docs/sandboxing and https://bazel.build/basics/hermeticity
- Reproducible Builds definition: https://reproducible-builds.org/docs/definition/
- SLSA specification v1.2: https://slsa.dev/spec/v1.2/
- in-toto: https://in-toto.io/
- in-toto Attestation Framework / DSSE: https://github.com/in-toto/attestation and https://github.com/secure-systems-lab/dsse
- OCI Image Specification: https://specs.opencontainers.org/image-spec/
- SPDX: https://spdx.dev/
- CycloneDX: https://cyclonedx.org/
- Sigstore: https://docs.sigstore.dev/
- The Update Framework: https://theupdateframework.io/
- Bootstrappable Builds: https://bootstrappable.org/
- David A. Wheeler, Diverse Double-Compiling: https://dwheeler.com/trusting-trust/
- Ken Thompson, Reflections on Trusting Trust

These are evidence/theory sources, not normative dependencies of the final MNCS semantics.
