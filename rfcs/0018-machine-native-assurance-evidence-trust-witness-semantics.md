# RFC 0018: Machine-Native Assurance Profiles, Evidence Algebra, Trust Policy, and Witness Semantics

- **Status:** Draft
- **Target:** Cross-cutting 0.2–1.0
- **Depends on:** RFC 0001, RFC 0002, RFC 0003, RFC 0004, RFC 0006, RFC 0007, RFC 0008, RFC 0009, RFC 0010, RFC 0011, RFC 0012, RFC 0013, RFC 0014, RFC 0015, RFC 0016, RFC 0017

## Summary

MNCS Language should treat assurance as an **action-relative judgment over an explicit evidence, assumption, provenance, trust, freshness, independence, conflict, and witness graph**, not as a scalar confidence score, a global `verified` bit, or a fixed ladder of verifier labels.

The central rule is:

> **Evidence is not truth. Evidence changes what the system is justified in concluding, and assurance policy determines what those justified conclusions authorize for a particular action.**

A second rule is equally important:

> **Supporting evidence, refuting evidence, absence of evidence, and contradictory evidence are distinct semantic states and must never be collapsed by arrival order, majority count, or a single confidence value.**

RFC 0018 extends RFC 0002's initial contract/evidence model into a machine-native epistemic and authorization layer capable of representing:

- claims and exact claim subjects;
- support, refutation, conflict, and lack of support;
- evidence methods and evidence artifacts;
- derivation and provenance structure;
- assumptions and assumption-scoped conclusions;
- evidence composition without dependency erasure;
- verifier/appraiser identity and scoped trust;
- delegation and revocation of evidentiary authority;
- raw evidence versus appraisal results;
- evidence applicability, freshness, and invalidation;
- evidence independence and common-mode dependencies;
- external witnesses and transparency commitments;
- optional uncertainty models whose assumptions remain explicit;
- assurance profiles as predicates over evidence graphs;
- action-relative authorization decisions; and
- explicit unresolved outcomes when the available evidence does not justify a requested action.

The intended result is not a universal theorem of correctness. It is a precise answer to questions such as:

- What evidence currently supports this claim?
- What evidence currently refutes it?
- Which assumptions does each justification depend upon?
- Which support paths remain valid after one dependency changes?
- Are two verifier results genuinely independent or merely duplicated through a shared implementation or dependency?
- Which principal produced or appraised each result?
- Was that principal trusted for this exact property class, subject, method, environment, and validity interval?
- Which evidence is stale, superseded, revoked, contradicted, or merely inapplicable?
- Which external witnesses observed a result or artifact?
- Does the evidence satisfy the policy for experimentation, execution, backend promise emission, deployment, or promotion?
- Why was an action authorized, rejected, or left unresolved?

This RFC deliberately keeps proof checking, empirical execution, statistical inference, attestation mechanisms, transparency-log implementation, quorum protocols, and policy-solving engines outside the RFC 0007 trusted proof kernel unless their outputs are themselves reduced to kernel-checkable objects.

---

## Motivation

MNCS already relies on evidence and assurance concepts throughout the architecture.

RFC 0002 establishes properties, evidence claims, verifier identities, assumptions, and assurance gradients, but intentionally leaves major questions open:

- evidence composition;
- contradictory evidence;
- verifier trust and certification;
- expiration and environment-sensitive evidence;
- remote witnesses; and
- stronger assurance profiles.

Later RFCs depend increasingly on those unresolved concepts. Examples include:

- RFC 0004 requiring independent verification before candidate promotion;
- RFC 0006 gating stronger backend promises on current evidence;
- RFC 0007 distinguishing proof, certificates, verifier results, empirical evidence, assumptions, and `UNKNOWN`;
- RFC 0009 and RFC 0010 using evidence to authorize stronger memory and concurrency realizations;
- RFC 0011 separating world state from knowledge state;
- RFC 0014 requiring evidence-bearing compatibility and provider binding;
- RFC 0015 transferring unsafe and foreign guarantees into explicit boundary evidence regimes;
- RFC 0016 requiring stage-valid evidence and independent promotion authority; and
- RFC 0017 requiring evidence for target and runtime properties rather than trusting names or conventions.

Without one cross-cutting assurance model, each subsystem risks inventing a slightly different answer to the same epistemic questions.

A naive model such as:

```text
claimed < tested < analyzed < verified < externally_verified
```

is insufficient.

A kernel-checked mathematical proof, a physical hardware measurement, an independent machine-code inspection, an external witness signature, a fuzzing campaign, and a target attestation establish different kinds of facts. They may be complementary, incomparable, contradictory, dependent on different assumptions, or applicable to different actions.

Likewise:

```text
VerifierA = PASS
VerifierB = FAIL
```

must not become whichever result arrived last.

And:

```text
10 verifier passes
```

must not automatically be stronger than:

```text
2 verifier passes
```

if all ten depend upon the same flawed implementation, parser, theorem library, compiler, target description, corpus, or administrative trust domain.

Machine-native software requires an explicit model of **what is known, how it became known, under which assumptions it is known, who is authorized to establish it, how independent the evidence is, and what the resulting knowledge permits the system to do**.

---

## Constitutional principles

### 1. Evidence is not truth

> **An evidence artifact or verifier result is a claim-supporting or claim-refuting object with method, scope, provenance, assumptions, dependencies, and validity conditions. Its existence does not make the underlying proposition metaphysically true.**

Formal proof may establish a theorem inside a defined logical system. Empirical execution may establish observations over a bounded domain. Attestation may establish measured state under an attestation model. Witnessing may establish that an entity observed or committed to an artifact.

These remain different claims.

### 2. Unknown and conflicted are different

> **Absence of sufficient support must remain distinguishable from simultaneous credible support and refutation.**

The initial claim-disposition model should preserve at least:

```text
UNSUPPORTED
SUPPORTED
REFUTED
CONFLICTED
```

These names describe the available evidence state, not the ultimate truth value of the claim.

### 3. Contradiction is preserved, not overwritten

> **New evidence may add support, add refutation, invalidate a dependency, or alter applicability. It does not delete contradictory evidence merely because it is newer or preferred.**

Supersession and revocation are explicit relations with explicit scope.

### 4. More evidence is not necessarily stronger assurance

> **Evidence count alone does not determine strength. Dependence, method, coverage, assumptions, trust domain, implementation lineage, and common-mode failure structure remain visible.**

Repeated execution of the same verifier against the same artifact may improve an empirical sampling claim while adding no meaningful implementation independence.

### 5. Independence is itself a claim

> **Two evidence producers are not considered independent merely because they have different process IDs, binary hashes, hostnames, model names, or verifier labels.**

Independence may depend on dimensions such as:

- implementation lineage;
- algorithmic lineage;
- compiler/toolchain lineage;
- theorem-library dependencies;
- parser/frontend dependencies;
- target/environment dependencies;
- corpus or reference-data dependencies;
- administrative trust domains;
- operator or model lineage; and
- shared infrastructure.

Any independence assertion used by policy must itself have provenance or remain an assumption.

### 6. Trust is scoped

> **Trusting a principal or verifier for one claim class does not grant global evidentiary authority.**

A trust grant should be constrainable by:

```text
principal
property class
subject class
method
assurance role
target/environment
stage
validity interval
maximum delegation depth
```

### 7. Raw evidence and appraisal are distinct

> **A measurement, execution trace, proof object, signed statement, attestation blob, benchmark record, or witness receipt is not automatically the conclusion an appraiser derives from it.**

The evidence object, appraisal method, appraisal result, and final assurance decision remain separately identifiable.

### 8. Freshness is applicability

> **Evidence may remain historically valid while no longer applying to the current subject, environment, dependency closure, target, policy, or action.**

Staleness must therefore be represented as failed applicability rather than rewriting history.

### 9. Witnessing is not verification

> **A witness proves only what its witness contract establishes. Observation, receipt, inclusion, consistency, reproduction, appraisal, proof checking, and behavioral verification are different methods.**

A witness must not silently inherit verifier authority.

### 10. Assurance is action-relative

> **There is no universal `verified = true` state. An evidence graph is evaluated against the policy for a particular action.**

The same artifact may be authorized for:

```text
RunInResearchSandbox
```

while remaining unresolved or rejected for:

```text
PromoteToTrustedBaseline
EmitBackendNoAliasPromise
DeployToProduction
DelegateMutationAuthority
```

### 11. Probability requires a model

> **A confidence, probability, belief mass, empirical rate, or uncertainty estimate has semantic force only under an identified model and dependency assumptions.**

A naked `confidence = 0.97` is not universal assurance authority.

### 12. Policy never fabricates evidence

> **An assurance policy may accept weaker evidence for a lower-risk action, but it must never relabel empirical observation as proof, witness receipt as verification, assumption as fact, or unresolved evidence as PASS.**

This rule preserves RFC 0007's formal proof boundary.

### 13. Assurance decisions are explainable artifacts

> **Every authorization, rejection, or unresolved assurance decision should identify the policy, evidence paths, conflicts, assumptions, trust grants, freshness facts, and unresolved conditions that produced it.**

### 14. Evidence composition preserves dependency structure

> **Composite support must not be flattened so far that later invalidation cannot determine which justification paths remain valid.**

---

## Selected theoretical synthesis

RFC 0018 should synthesize several complementary traditions rather than selecting one universal logic.

### Belnap-Dunn four-valued reasoning and bilattices

Paraconsistent four-valued logics provide a useful model for distinguishing:

```text
no support
support only
refutation only
both support and refutation
```

The key MNCS import is not a commitment that semantic truth itself is four-valued. It is the separation between **evidence polarity** and **amount of information**.

A conceptual evidence disposition is:

```text
               CONFLICTED
               /        \
         SUPPORTED     REFUTED
               \        /
               UNSUPPORTED
```

Additional policy-specific states may exist, but contradiction must not collapse into ordinary unknown.

This model permits monotonic accumulation of information while allowing authorization to become more conservative as contradictory evidence appears.

### Abstract argumentation and attack relations

Argumentation frameworks contribute the insight that evidence can interact in more ways than direct support and negation.

MNCS should eventually distinguish relations such as:

```text
Supports(Evidence, Claim)
Refutes(Evidence, Claim)
Undercuts(Evidence, EvidenceOrDerivation)
Invalidates(Change, Evidence)
Supersedes(NewEvidence, OldEvidence)
DependsOn(Derivation, AssumptionOrEvidence)
```

For example, a verifier need not prove `not P` to weaken evidence for `P`; it may instead establish that an assumption consumed by the original derivation is false or stale.

General-purpose argumentation search is not required in the trusted kernel. The semantic requirement is that these relationships can be represented explicitly.

### Truth-maintenance and assumption-based truth-maintenance systems

Truth-maintenance systems and ATMS-style reasoning contribute a particularly valuable structure: a conclusion may have several justification environments, each depending on different assumptions.

Conceptually:

```text
Claim P

support path 1:
    Evidence E1
    assumptions {A1, A4}

support path 2:
    Evidence E9
    assumptions {A1, A7}
```

If `A4` becomes invalid, only support path 1 is removed from the applicable justification set. `P` does not automatically become unsupported while path 2 remains valid.

This aligns directly with MNCS dependency-aware invalidation and should be treated as a core design principle.

### Provenance algebras and semiring-style lineage

Database provenance research demonstrates that derivation structure can be represented algebraically rather than reduced to a flat list of source records.

MNCS should investigate an evidence-provenance algebra capable of representing at least:

- alternative justification paths;
- jointly required evidence;
- dependency identity;
- derivation identity;
- invalidation-sensitive simplification; and
- exact explanation slices.

Illustratively, without selecting final notation:

```text
E1 + E2
```

may mean alternative sufficient support paths, while:

```text
E1 * E2
```

may mean a derivation jointly depends upon both.

The algebra must not claim logical soundness merely because provenance is compositional. It records derivation structure; the evidentiary meaning of each method remains separately governed.

### Structured assurance cases

Claims-arguments-evidence traditions and structured assurance-case metamodels contribute an organizational pattern:

```text
Claim
    <- Derivation / Argument
        <- Evidence
        <- Assumptions
```

MNCS should make this structure machine-readable and executable where possible.

Human narrative may be projected from the graph, but an MNCS assurance path should prefer explicit rule identity, dependency identity, and checkable derivation over prose asserting that evidence is persuasive.

### Authorization logics: SPKI/SDSI, SecPAL, DKAL, and evidential authorization

Authorization-logic traditions contribute a critical distinction among:

```text
Principal V says P
P
Policy trusts V to establish P under scope S
```

These are different propositions.

MNCS should therefore model verifier trust as scoped authorization rather than a global Boolean field such as:

```text
trusted_verifier = true
```

Delegation may be useful, but delegation chains must remain explicit, bounded, revocable where the trust model permits it, and incapable of granting evidence authority broader than the delegator possesses unless an independent root policy explicitly authorizes that expansion.

### Remote attestation architecture

Remote-attestation architectures contribute the separation among:

```text
Evidence
    -> Appraisal
        -> Appraisal Result
            -> Relying-Party Decision
```

MNCS should generalize that separation beyond hardware attestation.

A proof object, benchmark result, test trace, machine measurement, signed receipt, remote execution observation, or target measurement may all be raw or intermediate evidence. A verifier/appraiser interprets them under a method and set of reference claims. An assurance policy then determines whether the resulting appraisal is sufficient for a specific action.

### Transparency, witness cosigning, and quorum theory

Append-only transparency structures, external witnesses, and Byzantine quorum theory contribute useful semantics for externally observed evidence and anti-equivocation policies.

MNCS should define semantic objects for witness relationships without requiring one concrete Merkle tree, transparency service, cosigning protocol, or quorum algorithm.

A high-assurance policy may require evidence diversity such as:

```text
at least 3 verifier results
and at least 2 implementation lineages
and at least 2 administrative trust domains
and no unresolved shared critical dependency
and at least 1 external witness
```

Counts are inputs to policy, not assurance truth by themselves.

### Dempster-Shafer, Subjective Logic, Bayesian, and statistical uncertainty models

Some evidence is inherently uncertain or statistical.

MNCS should permit uncertainty-bearing evidence while refusing to select one universal uncertainty calculus.

An evidence result may identify a model such as:

```text
BayesianModel<...>
BeliefFunctionModel<...>
SubjectiveOpinionModel<...>
ConfidenceIntervalModel<...>
EmpiricalFrequencyModel<...>
DistributionFreeBound<...>
```

The model identity, calibration data, independence assumptions, parameterization, and scope become evidence dependencies.

Combining uncertain evidence is legal only under rules justified for the selected model. The base assurance layer must not invent a probability when evidence is merely unknown.

### Partial orders instead of a universal assurance ladder

Assurance methods should generally form a multidimensional partial order rather than one global ranking.

Relevant dimensions may include:

```text
formal strength
subject coverage
runtime/environment coverage
implementation independence
administrative independence
freshness
reproducibility
adversarial robustness
empirical breadth
assumption burden
trusted-computing-base size
```

Two evidence sets may be incomparable.

An assurance policy may choose preferences or minimum requirements for a particular action, but those policy choices do not create a universal ordering among methods.

---

## Semantic planes

RFC 0018 should keep several concerns orthogonal.

### Claim-disposition plane

Answers:

```text
What support and refutation currently apply to this claim?
```

Initial disposition:

```text
UNSUPPORTED
SUPPORTED
REFUTED
CONFLICTED
```

### Provenance and dependency plane

Answers:

```text
How was this evidence or derivation produced?
What assumptions and dependencies does it consume?
Which changes invalidate applicability?
```

### Argument and conflict plane

Answers:

```text
Which artifacts support, refute, undercut, invalidate, or supersede which others?
```

### Principal and trust plane

Answers:

```text
Who produced or appraised this evidence?
Who authorized that principal to establish this claim class?
What delegation and revocation apply?
```

### Appraisal plane

Answers:

```text
How was raw evidence interpreted?
Under which method, reference values, models, and assumptions?
```

### Independence and witness plane

Answers:

```text
Which results share critical dependencies?
Which external parties observed or reproduced them?
Which anti-equivocation or quorum requirements hold?
```

### Assurance-policy plane

Answers:

```text
Does the current evidence graph authorize action A under profile P?
```

The result should be at least:

```text
AUTHORIZED
REJECTED
UNRESOLVED
```

The policy may return dimension-specific reasons rather than collapsing all deficiencies into one status.

---

## Proposed object model

Names are illustrative rather than final canonical schema.

### `Claim`

```text
Claim {
    identity
    subject
    property
    scope
    context
    assumptions
}
```

A claim must identify the exact semantic subject to which evidence applies.

### `EvidenceArtifact`

```text
EvidenceArtifact {
    identity
    artifact_kind
    content_identity
    producer
    production_context
    dependencies
    assumptions
    validity_scope
}
```

Examples include proof terms, certificates, execution traces, fuzz corpora, reports, measurements, signed receipts, attestations, and machine-code inspection records.

### `EvidenceMethod`

Describes the semantics of the procedure used to produce or interpret evidence.

Examples:

```text
KernelProofCheck
CertificateCheck
AbstractInterpretation
ModelCheck
SymbolicExecution
StaticAnalysis
DifferentialExecution
PropertyTest
FuzzCampaign
FaultInjection
Measurement
Benchmark
RemoteAttestation
WitnessObservation
Reproduction
```

Method identity does not imply trust.

### `Appraisal`

```text
Appraisal {
    identity
    evidence_inputs
    method
    appraiser
    reference_claims
    assumptions
    environment
    result
    dependencies
}
```

### `EvidenceResult`

```text
EvidenceResult {
    identity
    claim
    polarity: support | refute | neutral
    method
    appraisal
    scope
    limitations
    uncertainty_model?
    freshness
}
```

A result may fail to support a claim without refuting it.

### `SupportRelation`

Links evidence or derivations to a claim.

### `RefutationRelation`

Links evidence or derivations to a claim they refute under the declared scope.

### `UndercutRelation`

Challenges an evidence artifact, derivation, trust grant, assumption, or applicability condition rather than directly asserting the negation of the target claim.

### `AssumptionEnvironment`

A canonical set of assumptions under which a justification path applies.

### `Derivation`

Represents evidence composition while preserving exact inputs and rule identity.

```text
Derivation {
    identity
    rule
    inputs
    assumptions
    output_claim
    method_class
    proof_or_certificate?
}
```

### `Principal`

Identifies an actor relevant to production, appraisal, witnessing, policy, or delegation.

Principal identity should not itself imply authentication semantics; RFC 0018 needs only enough structure to express trust relationships. A later identity/credential RFC may deepen this model.

### `TrustGrant`

```text
TrustGrant {
    grantor
    grantee
    property_scope
    subject_scope
    allowed_methods
    allowed_roles
    context_scope
    validity
    delegation_policy
}
```

### `Delegation`

An explicit transfer of bounded evidentiary authority.

### `Revocation`

An explicit statement that a prior trust or credential relationship no longer applies from a declared point or epoch.

Revocation does not rewrite historical facts; it changes current applicability.

### `IndependenceClaim`

```text
IndependenceClaim {
    evidence_set
    dimensions
    shared_dependencies
    excluded_common_modes
    assumptions
    supporting_evidence
}
```

### `WitnessReceipt`

Records what one principal claims to have observed or checked.

```text
WitnessReceipt {
    witness
    subject
    observation_kind
    observed_identity
    time_or_epoch
    context
    signature_or_authentication_artifact?
}
```

The authentication realization is intentionally outside the core object definition.

### `WitnessPolicy`

Describes required diversity, count, intersection, independence, or trust properties of acceptable witness sets.

### `TransparencyRecord`

Represents inclusion, consistency, append-only, anti-equivocation, or externally committed history claims without requiring one concrete log technology.

### `FreshnessScope`

Describes the applicability interval and invalidation dimensions for evidence.

```text
FreshnessScope {
    subject_identity
    dependency_fingerprints
    target_identity?
    environment_identity?
    toolchain_identity?
    stage?
    validity_interval?
    revocation_epoch?
}
```

### `AssuranceProfile`

An action-oriented predicate or policy over the evidence graph.

```text
AssuranceProfile {
    identity
    action_class
    required_claims
    accepted_methods
    minimum_diversity
    freshness_requirements
    trust_requirements
    conflict_policy
    assumption_policy
    uncertainty_policy
    unresolved_policy
}
```

### `AssuranceQuery`

```text
AssuranceQuery {
    subject
    action
    profile
    context
    requested_claims
}
```

### `AssuranceDecision`

```text
AssuranceDecision {
    query_identity
    outcome: AUTHORIZED | REJECTED | UNRESOLVED
    satisfied_requirements
    failed_requirements
    unresolved_requirements
    applicable_support
    applicable_refutation
    conflicts
    assumptions
    trust_paths
    witness_paths
    independence_basis
    freshness_basis
    explanation_slice
}
```

A decision is itself a versioned artifact with dependencies. If any consumed evidence, trust grant, policy, target fact, or assumption changes, the decision may become stale.

---

## Evidence disposition

The initial four-state disposition is derived from applicable support and refutation.

Conceptually:

```text
support = false, refute = false -> UNSUPPORTED
support = true,  refute = false -> SUPPORTED
support = false, refute = true  -> REFUTED
support = true,  refute = true  -> CONFLICTED
```

This is deliberately not a majority vote.

A policy may reject `CONFLICTED` immediately for high-risk actions, permit conflict resolution through a designated adjudication method, or permit a low-risk experimental action while preserving the conflict in the decision artifact.

A `REFUTED` state likewise does not imply a philosophical theorem that the claim is false. It means applicable evidence currently refutes the claim under the active semantic scope.

---

## Evidence composition

Evidence composition must distinguish at least three situations.

### Alternative sufficient paths

Either path independently satisfies the same requirement under policy.

```text
PathA OR PathB
```

### Joint requirements

Several distinct results are all required.

```text
ProofOfFunctionalProperty
AND
TargetAttestation
AND
FreshBackendInspection
```

### Derived support

A rule derives a new claim from existing claims/evidence.

```text
E1 + E2 + RuleR -> ClaimP
```

The exact algebra remains research work, but every composed result must retain enough structure to:

- identify consumed evidence;
- identify composition rule;
- identify consumed assumptions;
- independently check the derivation where possible;
- calculate invalidation;
- detect common dependencies;
- reconstruct explanation; and
- avoid counting the same underlying evidence multiple times through different paths.

---

## Trust, delegation, and verifier authority

Verifier identity and verifier authority are separate.

A verifier result should conceptually state:

```text
Verifier V
using Method M
under Context C
produced Result R
about Claim P
```

A trust policy separately determines whether `V` is authorized to establish evidence acceptable for `P` in that context.

Possible roles include:

```text
EvidenceProducer
ProofChecker
CertificateChecker
EmpiricalVerifier
Appraiser
Witness
Reproducer
PolicyAuthority
PromotionAuthority
```

Roles do not imply one another.

A proof checker does not automatically become promotion authority. A witness does not automatically become an appraiser. A benchmark producer does not automatically define the performance policy.

Delegation chains must be explicit and bounded. A grantee cannot silently broaden evidentiary authority beyond the grant.

---

## Independence and common-mode failure

Evidence independence is multidimensional.

MNCS should allow a policy to ask questions such as:

```text
Do these verifiers share implementation source?
Do they share the same parser?
Do they share the same solver?
Do they share the same proof checker?
Do they share the same target model?
Do they share training/model lineage?
Do they run under the same administrative control?
Do they share the same corpus or reference implementation?
Do they share the same compiler or runtime?
```

An implementation SHOULD compute conservative common-dependency closures from known provenance.

If independence cannot be established, policy must not silently assume it.

A useful initial representation may classify evidence-set relationships as:

```text
KNOWN_SHARED_DEPENDENCY
ESTABLISHED_DIVERSE_ON<Dimension>
ASSUMED_INDEPENDENT<Dimension>
UNKNOWN_INDEPENDENCE<Dimension>
```

High-assurance policies should be able to reject `ASSUMED` or `UNKNOWN` independence for selected dimensions.

---

## Freshness, applicability, supersession, and revocation

Evidence validity is not a single timestamp comparison.

Evidence applicability may depend on:

```text
semantic subject identity
artifact identity
compiler identity
backend identity
pass-pipeline identity
target identity
runtime identity
environment identity
assumption identities
reference model identity
verifier identity/version
policy identity
stage
time interval
revocation epoch
```

A change may therefore produce:

```text
still applicable
stale for this query
superseded for this role
revoked for current trust policy
historically valid but no longer current
```

Historical evidence should remain auditable even when it no longer authorizes current actions.

---

## Witness and transparency semantics

Witnessing should be represented relationally.

A witness may establish claims such as:

```text
I observed artifact hash H at epoch E
I checked inclusion of H in log L
I observed verifier V return result R
I independently reproduced execution request Q with result R
I checked consistency between log roots A and B
```

Those statements are not equivalent.

A `WitnessPolicy` may require diversity across:

- witness implementations;
- organizations;
- machines;
- geographic or network domains;
- cryptographic roots;
- execution environments; or
- administrative trust domains.

RFC 0018 should not mandate one Byzantine threshold or quorum rule. The chosen rule and its assumptions are policy objects.

Transparency history is particularly relevant to recursive refinement. Higher-assurance profiles may require promotion decisions, trust-grant changes, verifier upgrades, and baseline replacements to be externally committed or witnessed so a malicious repair process cannot quietly rewrite its own evidence history.

---

## Uncertainty-bearing evidence

Uncertainty should not be collapsed into the four-state disposition.

For example:

```text
SUPPORTED
```

may be supported by:

- a kernel proof;
- an empirical reliability estimate;
- a statistical test;
- a calibrated probabilistic model; or
- a bounded differential corpus.

Those methods have different semantics.

An `UncertaintyModel` should therefore remain attached to the evidence method/result when relevant.

The base assurance engine may ask whether the active policy accepts the model and whether required model assumptions are established. It does not need to implement every uncertainty calculus itself.

`UNKNOWN` external state from RFC 0011 must never be converted to a probability unless an explicit model justifies that conversion.

---

## Assurance policy semantics

An assurance profile is not an evidence producer.

It is a decision procedure over already represented claims, evidence, trust, conflicts, and context.

Conceptually:

```text
assure(subject, action, profile, evidence_graph)
    -> AUTHORIZED | REJECTED | UNRESOLVED
```

Example policy:

```text
Action: PromoteCandidate

requires:
    FunctionalCorrectness has KernelProof
    MemorySafety has IndependentVerifierPass
    BackendRefinement has (
        KernelProof
        OR (
            IndependentVerifierPass count >= 2
            AND BoundedDifferentialEvidence
        )
    )
    all required evidence fresh
    no unresolved critical refutation
    independence established on implementation_lineage
    authority_delta within approved boundary
```

A weaker development policy may instead permit unresolved formal proof while allowing sandbox execution.

The profile must expose exactly which requirements differ. It must never rewrite the underlying evidence classes.

---

## Relationship to `PASS`, `FAIL`, and `UNKNOWN`

Existing RFCs use `PASS`, `FAIL`, and `UNKNOWN` for method- and obligation-specific verifier results.

RFC 0018 does not remove those statuses.

It distinguishes:

```text
VerifierResult
    PASS | FAIL | UNKNOWN
```

from:

```text
ClaimDisposition
    UNSUPPORTED | SUPPORTED | REFUTED | CONFLICTED
```

from:

```text
AssuranceDecision
    AUTHORIZED | REJECTED | UNRESOLVED
```

These layers answer different questions.

A verifier `FAIL` may refute one claim, identify an invalid input, or merely fail to complete depending on the verifier contract. Mapping verifier results into evidence polarity must therefore be method-defined rather than globally assumed.

---

## Relationship to the proof kernel

RFC 0007 remains the formal proof boundary.

RFC 0018 must not enlarge that boundary by treating policy evaluation, trust configuration, transparency services, remote attestation, witness counts, Bayesian updates, or argumentation search as theorem proving.

A proof-bearing assurance derivation MAY itself carry a kernel-checkable term for a narrowly formal proposition.

For example:

```text
ProofChecker K accepted proof term T
```

may support a formal claim whose proof semantics are defined by RFC 0007.

But:

```text
Policy P trusts K
```

and:

```text
K was actually the binary that executed on remote machine M
```

remain separate trust/attestation claims unless independently formalized.

---

## Relationship to recursive refinement

RFC 0004 promotion decisions should consume RFC 0018 assurance queries.

Candidate generation never creates promotion authority.

A bounded refinement cycle may produce:

```text
Candidate
EvidenceSet
EvidenceDelta
ConflictSet
AssuranceQuery<PromoteCandidate>
AssuranceDecision
```

A repair that modifies:

- its own verifier trust grants;
- its assurance profile;
- witness requirements;
- evidence independence metadata; or
- the rules deciding promotion

must expose those as protected-policy changes rather than silently improving its own apparent assurance.

High-assurance profiles may require promotion-policy changes to be approved under a distinct meta-policy.

---

## Relationship to Forge, Fabric, and RAVEL

### Forge

Forge may:

- select evidence requests needed to satisfy an assurance query;
- dispatch micro-verifiers;
- identify missing evidence dimensions;
- calculate common dependency closures;
- localize conflicts;
- propose additional independent verification;
- estimate evidence-acquisition cost; and
- return evidence artifacts and appraisal results.

Forge does not become trusted simply because it orchestrates evidence production.

### Fabric

Fabric may execute bounded verification, testing, reproduction, benchmarking, fault injection, proof checking, or attestation jobs and return observations/receipts.

A Fabric receipt establishes only the claim defined by its execution/evidence contract.

### RAVEL

RAVEL may coordinate distributed evidence production, cross-node witnesses, verifier diversity, external reproduction, quorum policies, and recursive evidence gathering across trust boundaries.

RAVEL must preserve exact provenance and must not aggregate distributed results into a stronger assurance class without an explicit RFC 0018 derivation and policy.

---

## Security consequences

A machine-native assurance layer becomes security-critical because it governs what evidence authorizes stronger actions.

The design must account for:

- verifier impersonation;
- trust-grant forgery;
- delegated authority expansion;
- evidence replay outside its validity scope;
- stale evidence laundering;
- conflict suppression;
- selective deletion of refuting evidence;
- duplicated evidence counted as independent;
- hidden common-mode dependencies;
- compromised witnesses;
- equivocation by transparency services;
- malicious policy weakening;
- recursive repair modifying its own assurance requirements;
- probabilistic confidence laundering;
- benchmark or corpus gaming;
- evidence artifacts whose content changes while identity metadata is reused;
- revoked verifiers continuing to authorize current actions;
- trust roots whose bootstrap assumptions are hidden; and
- collusion among generator, verifier, appraiser, witness, and promotion authority.

Higher-assurance modes should support append-only or externally witnessed histories for policy, trust, baseline, and promotion changes.

The language must preserve the distinction between **evidence compromise** and **authority compromise**. A malicious verifier may invalidate conclusions derived from its results, but it must not automatically obtain mutation, deployment, or promotion authority.

---

## Design goals

RFC 0018 should make the following possible.

1. Represent support, refutation, unsupported state, and conflict distinctly.
2. Preserve every applicable evidence path rather than choosing a single winner.
3. Track exact assumptions and dependencies for each justification path.
4. Invalidate only dependent support/refutation paths after a change.
5. Separate raw evidence, appraisal, evidence result, claim disposition, and assurance decision.
6. Scope verifier trust by claim class, subject, method, context, role, and validity.
7. Support bounded delegation and explicit revocation.
8. Represent evidence independence and known common-mode dependencies.
9. Keep uncertainty models explicit and method-specific.
10. Represent witness and transparency evidence without making one implementation mandatory.
11. Express assurance profiles as predicates over evidence graphs rather than scalar thresholds.
12. Produce action-relative `AUTHORIZED`, `REJECTED`, or `UNRESOLVED` decisions.
13. Preserve the distinction between proof, certificate, verification, observation, reproduction, attestation, and witnessing.
14. Integrate directly with recursive candidate promotion and backend-promise gating.
15. Produce deterministic, machine-readable explanation slices for assurance decisions.
16. Keep formal proof checking inside RFC 0007 rather than expanding the proof kernel with policy machinery.
17. Permit independent implementations to agree on disposition and supported policy subsets.
18. Preserve historical evidence even when current applicability changes.
19. Make policy changes and trust changes first-class, auditable semantic events.
20. Permit evidence acquisition itself to become a bounded optimization/search problem without allowing the optimizer to redefine assurance.

---

## Non-goals

RFC 0018 does not:

- define one universal numeric confidence score;
- define one universal assurance-level ladder;
- claim evidence is identical to truth;
- require Dempster-Shafer, Bayesian inference, Subjective Logic, or another uncertainty calculus;
- require one argumentation semantics;
- require one theorem prover or proof assistant;
- replace RFC 0007's proof kernel;
- make majority vote equivalent to truth;
- treat verifier count as independence;
- require one PKI, certificate format, signature scheme, transparency log, Merkle tree, quorum protocol, or attestation technology;
- define the final principal/authentication/credential model for the language;
- make a witness automatically a verifier;
- make an appraiser automatically trusted;
- make signed evidence automatically valid or applicable;
- allow policy to convert assumptions into facts;
- permit `UNKNOWN` to become support merely because an action is desirable;
- require every research artifact to satisfy a production assurance profile;
- require all historical evidence to remain forever applicable;
- guarantee that arbitrary conflicting evidence can be automatically adjudicated; or
- claim that all independence relationships are statically decidable.

Unsupported policy constructs may remain `UNRESOLVED` or be rejected by the implementation subset under test.

---

## First implementation experiment

The first executable slice should remain deliberately small.

### Objects

Implement versioned research artifacts for:

```text
Claim
EvidenceResult
AssumptionEnvironment
EvidenceDependency
TrustGrant
IndependenceClaim
AssuranceProfile
AssuranceQuery
AssuranceDecision
```

Do not initially implement cryptographic identities, distributed transparency, a full argumentation engine, or probabilistic evidence combination.

### Claim disposition

Implement deterministic derivation of:

```text
UNSUPPORTED
SUPPORTED
REFUTED
CONFLICTED
```

from applicable evidence results.

Fixtures should include:

1. no evidence;
2. one supporting verifier result;
3. one refuting verifier result;
4. independent support and refutation producing conflict;
5. stale support excluded from current applicability;
6. support path invalidated by a changed assumption while another support path remains valid.

### Action-relative profiles

Define at least two tiny profiles:

```text
ResearchExecution
TrustedPromotion
```

`ResearchExecution` may accept bounded verifier evidence with unresolved stronger obligations.

`TrustedPromotion` should require stronger fresh evidence and established verifier independence for one selected property.

The same evidence set should demonstrate that one action can be authorized while another remains unresolved.

### Independence pilot

Represent two verifier results whose verifier identities differ but which share one declared critical implementation dependency.

A profile requiring implementation diversity must refuse to count them as two independent results.

Then provide a third result with a distinct declared implementation lineage and demonstrate satisfaction of the diversity predicate.

### Conflict pilot

Create one exact claim with:

```text
VerifierA -> PASS/support
VerifierB -> FAIL/refutation
```

The result must remain `CONFLICTED` until explicit new evidence or a policy-defined adjudication relation changes applicability. Arrival order must not matter.

### Freshness pilot

Bind one evidence result to an exact semantic subject and dependency fingerprint. Modify one consumed dependency and demonstrate that the evidence becomes inapplicable without deleting its historical record.

### Integration pilot

Use the resulting assurance decision to gate one existing operation, ideally:

- candidate promotion from RFC 0004; or
- one backend promise from RFC 0006/0.4 SSA work.

The experiment should demonstrate that policy consumes evidence but does not modify the underlying verifier result.

---

## Compatibility requirements

RFC 0018 must preserve established MNCS principles:

- semantic identity remains stable and explicit;
- evidence remains scoped to exact claims and subjects;
- assumptions remain visible;
- `PASS`, `FAIL`, and `UNKNOWN` remain method-specific results;
- proof remains distinguishable from empirical evidence;
- `UNKNOWN` remains first-class;
- authority remains explicit;
- target and environment facts remain evidence-bound;
- recursive refinement remains candidate-first;
- promotion remains policy-controlled;
- backend promises remain conservative;
- evidence invalidation remains dependency-aware; and
- the final source grammar remains undecided.

---

## Rejected directions

### Universal assurance score

Rejected. A scalar destroys method, assumption, conflict, independence, and scope information and encourages false comparisons among incomparable evidence classes.

### Last verifier wins

Rejected. Arrival order has no semantic authority to erase contradictory evidence.

### Majority vote as truth

Rejected. Several dependent verifiers may share one defect, while one independent refutation may expose it.

### Trusted verifier Boolean

Rejected. Trust must be scoped to property, method, subject, context, role, and validity.

### Witness signatures imply correctness

Rejected. A signature or witness receipt establishes only the witness contract.

### Policy upgrades evidence class

Rejected. A permissive policy may authorize an action with weaker evidence; it may not rename that evidence as proof or stronger verification.

### Force all evidence into probability

Rejected. Unknown is not random, conflict is not a probability distribution, and many evidence methods do not justify numeric probabilities.

### Put assurance policy inside the proof kernel

Rejected as the default. Policy, trust configuration, and distributed witnessing have different trust assumptions from formal theorem checking.

---

## Unresolved questions

- exact canonical representation of claim disposition;
- whether the four-state model should be represented directly as a bilattice or derived from orthogonal support/refutation sets;
- canonical evidence-provenance algebra;
- rules for evidence subsumption and redundancy;
- proof/certificate representation for evidence-composition derivations;
- conflict adjudication semantics;
- treatment of evidence that weakens but does not fully refute a claim;
- trust-grant composition and delegation depth;
- revocation semantics across disconnected/offline environments;
- principal identity and authentication boundaries;
- exact common-mode dependency taxonomy;
- independence inference versus explicit declaration;
- evidence diversity metrics without turning them into assurance scores;
- witness-set and transparency semantics;
- anti-equivocation requirements;
- remote-attestation integration;
- probabilistic and belief-model plugin boundaries;
- assurance-policy language and decidable subset;
- policy conflict and policy hierarchy;
- meta-policy governing policy changes;
- assurance decisions across distributed nodes;
- historical evidence retention and summarization;
- privacy/confidentiality of evidence and dependency graphs;
- proof that an explanation slice is sufficient;
- performance of large evidence graphs;
- interaction with future principal/credential semantics;
- how assurance profiles compose across module/component boundaries;
- how package/build provenance contributes to independence claims;
- whether rejected or refuted evidence may ever be garbage-collected; and
- how much of assurance evaluation should be independently re-checkable from a compact certificate.

---

## Research traditions

RFC 0018 is informed by a synthesis of:

- Belnap-Dunn paraconsistent four-valued logic and bilattice reasoning;
- Dung-style abstract argumentation and attack/acceptability relations;
- Doyle-style truth-maintenance systems and de Kleer-style assumption-based truth maintenance;
- provenance semirings and algebraic data provenance;
- W3C-style provenance separation among entities, activities, agents, and derivations;
- structured assurance cases and claims-arguments-evidence models;
- SPKI/SDSI-style authorization and delegation concepts;
- SecPAL, DKAL, and evidential-authorization logics;
- remote-attestation architectures separating evidence, appraisal, and relying-party decisions;
- append-only transparency logs, witness cosigning, and Byzantine quorum theory;
- Dempster-Shafer evidence theory, Subjective Logic, Bayesian methods, and statistical uncertainty models as optional evidence-model families; and
- order-theoretic partial-order reasoning for multidimensional assurance.

None of these systems is adopted wholesale. Their useful structures are selected to support the MNCS requirement that autonomous systems remain explicit about **what they know, why they know it, what contradicts it, who they trust, where that trust applies, and exactly what the resulting assurance authorizes them to do**.
