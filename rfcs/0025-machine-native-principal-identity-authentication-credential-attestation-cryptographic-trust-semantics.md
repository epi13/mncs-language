# RFC 0025: Machine-Native Principal, Identity, Authentication, Credential, Attestation, and Cryptographic Trust Semantics

- **Status:** Draft
- **Area:** Semantic foundation / security / authority / trust
- **Depends on:** RFC 0001, RFC 0002, RFC 0007, RFC 0008, RFC 0009, RFC 0015, RFC 0017, RFC 0018, RFC 0020, RFC 0023, RFC 0024
- **Informs:** RFC 0026, RFC 0028, RFC 0030, RFC 0031, RFC 0034 and later distributed/deployment/tooling work

## Abstract

MNCS requires a machine-native theory for answering questions such as:

- which semantic principal is acting;
- which name, key, authenticator, credential, account, workload, device, process, agent, or execution instance refers to that principal;
- how a current actor proves control of an authenticator;
- how identity proofing, enrollment, authentication, credential validation, authorization, delegation, revocation, attestation, and action trust differ;
- which issuer is permitted to establish which class of claim;
- how authority is attenuated through delegation;
- how short-lived credentials can represent long-lived principal identity;
- how trust domains and federation translate identity and authority without erasing trust boundaries;
- how an attester reports measured state, how a verifier appraises that evidence, and how a relying policy decides what the resulting claims authorize;
- how cryptographic validity differs from semantic trust;
- how key rotation, compromise, revocation, recovery, threshold authority, and algorithm migration change current bindings without rewriting historical identity;
- how workload, device, code-artifact, execution-instance, agent, and platform identities remain distinct while being related explicitly;
- how identity and attestation evidence remain subject to RFC 0024 information-flow policy; and
- how Forge, Fabric, RAVEL, the compiler, verifiers, runtimes, and future distributed MNCS components can rely on narrowly scoped, current, proof-relevant identity and trust artifacts rather than ambient accounts or boolean `trusted` flags.

This RFC establishes the semantic vocabulary and constitutional constraints for those questions. It deliberately does **not** make X.509, SPIFFE, WebAuthn, Verifiable Credentials, EAT, TPMs, TEEs, JWTs, macaroons, Sigstore, or any other current mechanism the language ontology. Such systems are treated as potential credential, authentication, delegation, or attestation realizations whose claims must map into the MNCS semantic model.

The central principle is:

> **Principal identity, name, key identity, authenticator identity, credential validity, authentication, authorization, delegation authority, attestation, trustworthiness, and trust are distinct semantic concepts. None implies another without an explicit relation and sufficient evidence.**

A second constitutional principle is:

> **A valid cryptographic proof establishes only the claim defined by that proof under its cryptographic and contextual assumptions. It does not silently establish principal meaning, authorization, trustworthiness, or action authority.**

A third is:

> **Authority is scoped and derivational. Delegation may attenuate existing authority but may not silently manufacture broader authority.**

And a fourth is:

> **Attestation reports evidence about state. Appraisal interprets that evidence. Trust and authorization are relying-policy decisions.**

These distinctions are required if MNCS is to permit aggressive machine-generated orchestration while keeping trust boundaries explicit, auditable, replaceable, and independently checkable.

---

## 1. Problem statement

Conventional software frequently collapses several independent questions into one token, certificate, account, role, login session, or boolean:

```text
if authenticated {
    trusted = true
}
```

or:

```text
if verify_signature(cert) {
    allow()
}
```

or:

```text
if attestation_passed {
    production_trusted = true
}
```

These shortcuts are especially dangerous in a machine-native system because agents and compilers may recursively compose the shortcut long after the original author has forgotten the unstated assumptions behind it.

A signature may establish that a particular key produced a signature over particular bytes.

A certificate may establish that an issuer made a claim binding a subject to a key or attribute.

An authentication protocol may establish that the present peer possesses an authenticator bound to a principal.

An authorization credential may establish that some issuer delegated a bounded action set.

An attestation may establish that an attester reported a particular measured state with adequate freshness.

None of those alone establishes that the action currently requested should be authorized.

MNCS therefore needs explicit semantic objects for each step in the trust chain.

The language and toolchain must be able to represent:

```text
Principal P

Name N refers to P
    under namespace NS
    during interval T
    because issuer I established binding B

Authenticator K is bound to P
    for relying context C
    during interval T2

Current actor proved possession of K
    under protocol A
    using challenge Q

Credential G was issued by J
    to P
    with claims S
    under audience D
    during interval T3

J is authorized by relying policy R
    to establish claim class S1
    but not claim class S2

P currently has authority X
    derived from delegation chain D1
    subject to caveats C1..Cn

Attestation E reports
    code artifact H
    platform state M
    epoch E7

Verifier V appraised E
    against reference values RV

Policy R decides
    VERIFY is authorized
    MUTATE is not authorized
    PROMOTE remains unresolved
```

This RFC exists to make that shape first-class.

---

## 2. Non-goals

RFC 0025 does not:

- choose one global human identity system;
- require legal or civil identity for machine principals;
- require globally unique human-readable names;
- require public-key cryptography for every principal relation;
- define a universal PKI;
- make X.509 the canonical credential representation;
- make SPIFFE the canonical workload identity system;
- make WebAuthn the canonical human authenticator protocol;
- make Verifiable Credentials the canonical claim representation;
- make EAT the canonical attestation encoding;
- require TPM, TEE, HSM, secure enclave, secure boot, or any specific hardware root of trust;
- define all cryptographic algorithms or their implementation details;
- define confidentiality/information-flow policy, which belongs to RFC 0024;
- define general effect/capability semantics, which belong to RFC 0008;
- define global evidence appraisal or action-relative assurance, which belongs to RFC 0018;
- define full software-supply-chain provenance or build attestation, which belongs to RFC 0031;
- define distributed consensus or membership semantics, which belong to RFC 0028;
- define persistent credential stores or crash-consistent revocation databases, which belong to RFC 0026;
- claim that authentication strength or credential quality is one scalar ranking; or
- equate successful cryptographic verification with semantic authorization.

---

## 3. Constitutional distinctions

MNCS MUST preserve the following distinctions unless a separate explicit relation proves otherwise:

```text
PrincipalIdentity
    != Name
    != NamespaceBinding
    != AccountIdentity
    != KeyIdentity
    != AuthenticatorIdentity
    != CredentialIdentity
    != DeviceIdentity
    != WorkloadIdentity
    != CodeArtifactIdentity
    != PlatformIdentity
    != ExecutionInstanceIdentity
    != SessionIdentity
```

Likewise:

```text
IdentityProofing
    != Enrollment
    != Authentication
    != CredentialValidation
    != Authorization
    != Delegation
    != Attestation
    != TrustDecision
```

And:

```text
CryptographicValidity
    != ClaimTruth
    != ClaimAcceptance
    != Trustworthiness
    != Trust
    != ActionAuthorization
```

These are foundational invariants, not documentation conventions.

---

## 4. Semantic planes

RFC 0025 organizes the model into eight interacting planes.

### 4.1 Principal plane

Describes semantic actors and subjects:

```text
Principal
PrincipalKind
PrincipalIdentity
PrincipalSet
PrincipalLineage
Name
Namespace
NameBinding
TrustDomain
```

### 4.2 Cryptographic-binding plane

Describes authenticators, keys, bindings, and use purposes:

```text
Authenticator
KeyIdentity
KeyMaterialReference
PrincipalAuthenticatorBinding
ProofOfPossessionBinding
KeyUsagePurpose
KeyProvenance
KeyRotationEvent
KeyCompromiseEvent
```

### 4.3 Authentication plane

Describes proofing, enrollment, challenge-response, sessions, and freshness:

```text
IdentityProofingClaim
EnrollmentEvent
AuthenticationRequest
AuthenticationEvent
AuthenticationClaim
AuthenticationContext
SessionCredential
SessionPrincipal
```

### 4.4 Credential plane

Describes claims made by issuers about subjects:

```text
Credential
CredentialClaim
Issuer
Subject
Audience
ValidityInterval
CredentialStatus
CredentialStatusEvidence
```

### 4.5 Authority plane

Describes actions and delegation:

```text
AuthorityScope
AuthorizationClaim
Delegation
DelegationPath
Caveat
AttenuationRelation
ThresholdPrincipal
RecoveryAuthority
```

### 4.6 Trust plane

Describes reliance boundaries and federation:

```text
TrustAnchor
TrustPolicy
TrustRelation
TrustDomain
FederationRelation
CredentialPath
PathValidationResult
IssuerAuthority
```

### 4.7 Attestation plane

Describes measured state and appraisal:

```text
Attester
AttestationEvidence
AttestationClaim
Measurement
ReferenceValue
AttestationEndorsement
AttestationVerifier
AttestationResult
FreshnessEvidence
```

### 4.8 Assurance plane

Consumes all prior planes through RFC 0018:

```text
ClaimDisposition
EvidenceApplicability
AssuranceProfile
ActionAuthorization
```

RFC 0025 defines what identity/security claims mean. RFC 0018 decides whether the current evidence graph is sufficient for a specific action.

---

## 5. Principals

A `Principal` is the semantic entity to which identity, claims, authority, responsibility, or policy may refer.

A principal is not required to be human.

Initial principal categories SHOULD include at least:

```text
HumanPrincipal
WorkloadPrincipal
DevicePrincipal
ServicePrincipal
OrganizationPrincipal
AgentPrincipal
RolePrincipal
EphemeralPrincipal
CompositePrincipal
ThresholdPrincipal
```

The taxonomy remains extensible.

A principal MAY be opaque to ordinary program code while remaining structurally visible to authorized tooling.

### 5.1 Principal identity

`PrincipalIdentity` identifies a semantic principal within a declared identity regime.

A principal identity MUST NOT be inferred solely from:

- equal display names;
- equal usernames;
- equal public keys;
- equal credential subjects;
- equal device serial numbers;
- equal process IDs;
- equal network addresses; or
- equal artifact hashes.

Those may contribute evidence to an explicit identity relation but are not themselves universal principal identity.

### 5.2 Principal sets

Some policies only establish membership in a class:

```text
principal ∈ ApprovedBuildWorkers
```

without establishing which exact worker is acting.

MNCS SHOULD permit claims scoped to a `PrincipalSet` where exact identity is unnecessary.

This avoids fabricating identity precision that the evidence does not support.

---

## 6. Names and namespaces

Names are references inside namespaces.

```text
Name {
    namespace
    lexical_name
}
```

The string `build-agent` is not globally meaningful.

A binding SHOULD be represented approximately as:

```text
NameBinding {
    name
    principal
    issuer
    scope
    validity
    evidence
}
```

### 6.1 Local names

MNCS SHOULD support local or principal-scoped naming inspired by SDSI-like models.

```text
A / verifier
```

and:

```text
B / verifier
```

need not refer to the same principal.

### 6.2 Name equality

`NameEquality` MUST NOT imply `PrincipalIdentity`.

Name comparison and principal comparison are separate RFC 0020 relations.

### 6.3 Namespace federation

Cross-namespace translation MUST be explicit and evidence-bearing.

A foreign name does not automatically become a local principal identity merely because the string matches a local name.

---

## 7. Trust domains

A `TrustDomain` is a scoped administrative or cryptographic reliance boundary.

Trust domains MAY correspond to:

- an MNCS installation;
- a Fabric cluster;
- an organization;
- a deployment realm;
- a project;
- a tenant;
- a hardware-security domain; or
- another explicitly defined security boundary.

Trust domains MUST NOT imply global mutual trust.

A principal identity may be qualified by trust domain:

```text
mncs://domain-A/forge/verifier/17
```

without making URI syntax itself canonical semantics.

### 7.1 Federation

Federation is an explicit relation between trust domains.

```text
FederationRelation {
    source_domain
    destination_domain
    accepted_claim_classes
    translation_policy
    issuer_requirements
    validity
    evidence
}
```

Federation MUST NOT mean `trust everything from source_domain`.

---

## 8. Keys and authenticators

A key or authenticator is a mechanism through which a principal may prove a relation.

```text
Authenticator {
    id
    kind
    capabilities
    provenance
}
```

Examples include:

- hardware-bound asymmetric key;
- software asymmetric key;
- passkey/WebAuthn credential;
- password-derived authenticator;
- OTP authenticator;
- TPM-resident key;
- HSM-managed key;
- workload key;
- secure-enclave credential;
- threshold key share; or
- future mechanisms.

### 8.1 Key identity is not principal identity

One principal may possess many keys.

One principal may rotate keys.

One key may be shared, stolen, or repurposed.

Therefore:

```text
KeyIdentity != PrincipalIdentity
```

### 8.2 Principal-authenticator binding

A current relationship SHOULD be represented as:

```text
PrincipalAuthenticatorBinding {
    principal
    authenticator
    relying_scope
    key_usage
    issuer_or_registrar
    validity
    evidence
}
```

### 8.3 Binding history

Bindings are historical artifacts.

A binding may be:

```text
ACTIVE
SUPERSEDED
REVOKED
COMPROMISED
EXPIRED
UNRESOLVED
```

without changing historical facts about when it was valid.

---

## 9. Key usage and domain separation

Keys MUST have explicit usage scope where use purpose affects security.

Examples:

```text
ArtifactSigning
CredentialIssuance
Authentication
AttestationSigning
DelegationSigning
RecoveryAuthorization
TrustAnchorSigning
```

A key permitted for `ArtifactSigning` MUST NOT automatically be permitted for `CredentialIssuance`.

Cross-purpose reuse requires explicit policy.

This prevents accidental protocol/domain confusion.

---

## 10. Key provenance

`KeyProvenance` captures evidence about key origin and handling.

Possible claims include:

```text
GeneratedInHardware
GeneratedInSoftware
Imported
Exportable
NonExportable
HardwareBound
ThresholdGenerated
Recovered
RotatedFrom
```

These are claims, not intrinsic truths.

Hardware-binding claims MAY require attestation or other evidence.

RFC 0018 decides what current actions they authorize.

---

## 11. Algorithm agility

Principal identity MUST survive cryptographic algorithm migration where the identity policy permits it.

```text
Principal P
    -> key K1 / algorithm A1
    -> key K2 / algorithm A2
```

need not produce a new principal.

A cryptographic binding SHOULD include:

```text
CryptographicBinding {
    principal
    key
    algorithm
    parameters
    usage
    validity
    evidence
}
```

Cryptographic algorithm acceptability is policy-relative and time-sensitive.

A mathematically valid signature under a deprecated algorithm may remain historically valid while being unacceptable for a current production action.

---

## 12. Identity proofing

Identity proofing establishes evidence about the relationship between a principal and an external identity claim.

Examples:

- a human principal corresponds to a legally identified person;
- a device principal corresponds to a manufacturer serial and hardware root;
- a workload principal was created from an approved deployment declaration;
- an organization principal was established by an administrative trust process.

Identity proofing is OPTIONAL for principal categories that do not require real-world identity.

A machine workload SHOULD NOT require human identity proofing merely to obtain an internal principal identity.

```text
IdentityProofingClaim {
    subject
    asserted_identity
    method
    proofing_authority
    scope
    evidence
}
```

Identity proofing does not itself authenticate a current actor.

---

## 13. Enrollment

Enrollment binds an authenticator or credential mechanism to a principal.

```text
EnrollmentEvent {
    principal
    authenticator
    registrar
    prerequisites
    context
    validity
    evidence
}
```

Enrollment MUST remain distinct from identity proofing and later authentication.

Successful proofing does not silently authorize arbitrary new authenticators.

---

## 14. Authentication

Authentication is a scoped event establishing that a current actor satisfied an authentication relation.

It MUST NOT be modeled as a timeless principal property.

```text
AuthenticationEvent {
    principal
    authenticator
    verifier
    protocol
    relying_context
    challenge_or_freshness
    observed_at
    valid_until
    evidence
}
```

### 14.1 Authentication claim

```text
AuthenticationClaim {
    subject
    principal
    authenticator
    protocol
    context
    freshness
    assumptions
    evidence
}
```

### 14.2 Authentication is not authorization

A successful `AuthenticationClaim` provides evidence about the actor.

It does not itself permit an action.

```text
Authenticated(P) != Authorized(P, action)
```

This is constitutional.

---

## 15. Freshness of authentication

Authentication freshness MUST be explicit where replay matters.

Supported freshness mechanisms may include:

```text
NonceFreshness
TimestampFreshness
EpochFreshness
SessionFreshness
ChannelBindingFreshness
```

No mechanism is universally stronger than every other mechanism.

For example, nonce freshness may avoid synchronized-clock requirements while timestamp freshness may be suitable for another protocol.

RFC 0023 defines temporal meaning for timestamp-based freshness.

---

## 16. Bearer credentials and proof of possession

MNCS MUST distinguish bearer credentials from proof-of-possession credentials.

```text
BearerCredential
ProofOfPossessionCredential
```

A bearer credential authorizes whoever possesses the credential under its policy.

A proof-of-possession credential additionally requires proof that the presenter controls a bound authenticator/key.

Copying the credential representation MUST NOT automatically satisfy proof of possession.

This distinction SHOULD be visible to RFC 0018 assurance policy.

---

## 17. Credential model

A credential is a proof-bearing container for one or more claims issued by an issuer about a subject.

```text
Credential {
    id
    issuer
    subject
    claims
    proof_binding
    audience
    purpose
    context
    validity
    status_reference
    representation
}
```

The semantic credential model is representation-independent.

Possible realization encodings include:

```text
X509
JWT
CWT
VerifiableCredential
SVID
EAT
MNCSNative
FutureFormat
```

The encoding does not determine semantic authority.

---

## 18. Credential identity

Credential artifact identity is distinct from:

```text
IssuerPrincipalIdentity
SubjectPrincipalIdentity
KeyIdentity
ClaimIdentity
SignatureIdentity
```

Two credentials may assert the same claim while remaining different artifacts.

One credential may be re-encoded without preserving artifact identity while preserving a narrower semantic relation under RFC 0020.

---

## 19. Credential cryptographic validity

A credential may have a claim such as:

```text
CredentialCryptographicallyValid
```

This establishes only that the credential proof validates under the declared cryptographic model and current inputs.

It MUST NOT imply:

```text
IssuerAuthorizedForClaim
ClaimTrue
SubjectAuthenticatedNow
ActionAuthorized
```

Those require separate evidence and policy.

---

## 20. Issuer authority

Issuer authority is claim-class specific.

```text
IssuerAuthority {
    issuer
    claim_class
    subject_scope
    audience_scope
    validity
    evidence
}
```

An issuer authorized to assert:

```text
DeviceModel
```

is not automatically authorized to assert:

```text
ProductionDeploymentAuthority
```

Credential verification therefore includes at least two conceptually distinct steps:

```text
1. Was this claim issued authentically by I?
2. Does relying policy accept I as authoritative for this claim class?
```

---

## 21. Audience and purpose binding

Credentials SHOULD support explicit audience and purpose constraints.

A credential intended for:

```text
staging deployment
```

MUST NOT silently become valid for:

```text
production trust-root rotation
```

Credential scope may include:

```text
Audience
Environment
Resource
Operation
Purpose
TrustDomain
```

Cross-context replay requires explicit policy approval.

---

## 22. Credential validity and time

Credential time fields MUST use RFC 0023 temporal semantics.

Relevant concepts include:

```text
not_before
not_after
issued_at
expires_at
status_valid_until
```

Near a temporal boundary, clock uncertainty may produce:

```text
CredentialCurrentlyValid = UNRESOLVED
```

rather than a fabricated boolean.

---

## 23. Expiration and revocation

Expiration and revocation are distinct.

`Expired` means the declared validity interval no longer includes the relevant time.

`Revoked` means an authorized revocation mechanism has invalidated the binding or credential.

```text
Expired != Revoked
```

Historical credential claims remain historically representable even after expiration or revocation.

---

## 24. Credential status

Credential status SHOULD permit at least:

```text
GOOD
REVOKED
UNKNOWN
```

and MAY additionally distinguish:

```text
SUSPENDED
SUPERSEDED
COMPROMISED
EXPIRED
```

where semantics require it.

The constitutional rule is:

```text
UNKNOWN != GOOD
```

An unavailable revocation source MUST NOT silently produce a valid status unless policy explicitly defines a fail-open behavior and assumes its risks.

---

## 25. Status freshness

Revocation/status evidence has its own validity.

```text
CredentialStatusEvidence {
    credential
    status
    source
    observed_at
    validity
    evidence
}
```

A previously good status may become stale.

RFC 0018 governs whether stale status evidence remains sufficient for a particular action.

---

## 26. Authorization

Authorization is the action-relative conclusion that a principal has authority to perform a defined action under current policy.

RFC 0025 defines authority objects and derivation.

RFC 0018 owns the final action-relative assurance decision.

```text
AuthorizationClaim {
    principal
    action
    resource_scope
    context
    authority_source
    validity
    evidence
}
```

Authentication is an input to authorization, not a synonym.

---

## 27. Authority scope

Authority MUST be explicitly scoped.

```text
AuthorityScope {
    actions
    resources
    environments
    targets
    purpose
    temporal_scope
    delegation_rights
    caveats
}
```

The language MUST avoid a universal `admin = true` semantic primitive.

Human-facing role names may elaborate into explicit authority sets but do not replace them in canonical semantics.

---

## 28. Delegation

Delegation is an explicit transfer or derivation of bounded authority.

```text
Delegation {
    issuer
    subject
    parent_authority
    delegated_authority
    delegable
    caveats
    validity
    evidence
}
```

Delegation MUST preserve its derivation path.

---

## 29. Attenuation

Delegated authority MUST NOT silently exceed parent authority.

Conceptually:

```text
Authority(child) ⊆ Authority(parent)
```

unless an independent authority source contributes additional authority.

This relation SHOULD be expressible as an RFC 0020 refinement/containment claim.

Valid attenuation may:

- remove actions;
- reduce resource scope;
- narrow environment scope;
- shorten validity;
- forbid delegation;
- add caveats;
- bind audience;
- bind workload or platform identity;
- reduce target scope; or
- add stronger authentication/attestation prerequisites.

---

## 30. Delegation authority

Permission to perform an action does not automatically imply permission to delegate that action.

```text
MayExecute(X) != MayDelegate(X)
```

Delegation rights MUST be explicit.

---

## 31. Caveats

A `Caveat` is an additional predicate that narrows credential or delegated authority.

```text
Caveat {
    predicate
    evaluation_scope
    evidence_requirements
}
```

Examples:

```text
repo == X
branch matches feature/*
clock < T
platform in ApprovedWorkers
attestation.artifact == H
request.operation == VERIFY
```

Delegation may add caveats.

Ordinary delegation MUST NOT remove inherited caveats.

This provides a macaroon-like monotonic attenuation direction without requiring macaroons as the canonical encoding.

---

## 32. Delegation paths

Authority provenance SHOULD be represented as a derivation graph:

```text
RootAuthority
    -> Forge
        -> AgentA
            -> AgentB
```

```text
DelegationPath {
    root_authority
    edges
    effective_authority
    inherited_caveats
    validity
}
```

Revocation, expiration, or invalidation of a dependency SHOULD invalidate dependent authority paths through RFC 0018 without invalidating unrelated paths.

---

## 33. Multiple independent authority sources

A principal may hold authority through more than one path.

MNCS MUST preserve those alternative derivations rather than flattening them into one bit.

Example:

```text
Authority path 1:
    organization -> Forge -> verifier

Authority path 2:
    emergency operator -> verifier
```

Revoking path 1 need not remove path 2.

RFC 0018 provenance algebra is the appropriate appraisal mechanism.

---

## 34. Threshold principals

A `ThresholdPrincipal` represents authority requiring a threshold of member principals.

```text
ThresholdPrincipal {
    members
    threshold
    composition_policy
}
```

Examples:

```text
2-of-3 operators
controller + hardware root
3-of-5 trust officers
```

Threshold principals are especially suitable for:

- trust-anchor changes;
- root-key use;
- emergency recovery;
- production deployment approval;
- release-key use; and
- irreversible policy changes.

Threshold identity is not the same as simple set membership.

---

## 35. Sessions

A successful authentication MAY create a derived session principal or session credential.

```text
SessionCredential {
    parent_authentication
    principal
    authority
    context
    validity
    binding
}
```

Session authority SHOULD usually be narrower than root principal authority.

Long-lived root credentials SHOULD NOT be required for ordinary repeated actions where an attenuated session credential suffices.

---

## 36. Workload identities

Workloads are first-class principals.

A workload principal may be stable across:

- process restart;
- key rotation;
- credential rotation;
- replica changes; or
- migration between approved workers,

when the declared identity policy permits it.

```text
WorkloadPrincipal {
    trust_domain
    workload_identity
    deployment_scope
}
```

Credential realization MAY be short-lived and frequently rotated.

This follows the useful separation demonstrated by SPIFFE-like workload identity systems:

```text
WorkloadIdentity != CredentialPresentation
```

---

## 37. Device principals

Device identity is distinct from:

```text
HardwareSerial
PublicKey
NetworkAddress
PlatformMeasurement
WorkloadIdentity
```

A device principal MAY be bound to hardware evidence, manufacturer claims, or locally provisioned keys, but those are explicit relations.

---

## 38. Service principals

Services may be logical principals independent of any single process or node.

A service identity may therefore survive instance replacement while individual execution instances retain separate identities.

---

## 39. Agent principals

Machine-generated or orchestrated agents SHOULD be first-class principals.

```text
AgentPrincipal {
    identity
    parent_or_creator
    purpose
    lifetime
    trust_domain
}
```

Agent lineage is not authority inheritance.

```text
SpawnedBy(A, B) != DelegatedAuthority(A, B)
```

A parent may spawn a child while granting only a narrow subset of authority.

This is essential for Fabric subagents and future machine-native recursive agent workflows.

---

## 40. Principal lineage

Principal lineage SHOULD capture relationships such as:

```text
created_by
spawned_by
derived_from
replaced_by
migrated_from
```

Lineage is provenance.

Authority is a separate relation.

This boundary prevents systems from accidentally treating ancestry as privilege.

---

## 41. Code artifact identity

Code artifact identity belongs to the artifact rather than the principal.

```text
CodeArtifactIdentity {
    semantic_identity?
    content_identity?
    build_identity?
}
```

RFC 0020 and future RFC 0031 define the detailed relation.

RFC 0025 only requires that workload/principal identity not collapse into artifact identity.

---

## 42. Execution instance identity

A running instance is distinct from:

```text
WorkloadPrincipal
CodeArtifactIdentity
DevicePrincipal
PlatformPrincipal
ProcessIdentifier
```

An explicit relation may establish:

> execution instance E is acting as workload W, executing artifact H, on platform P.

Attestation can provide evidence for those relations.

---

## 43. Trust relations

Trust is a scoped relying decision.

```text
TrustRelation {
    relying_principal
    trusted_principal_or_authority
    claim_class
    subject_scope
    context
    validity
    assumptions
}
```

Trust MUST NOT be globally transitive.

```text
A trusts B for X
B trusts C for Y
```

does not imply:

```text
A trusts C for X or Y
```

unless an explicit delegation/federation/policy relation establishes it.

---

## 44. Trust anchors

A `TrustAnchor` is a policy point at which a relying system accepts an issuer/key/authority without requiring an upstream certification path under that same policy.

It is not universal truth.

```text
TrustAnchor {
    subject
    accepted_claim_classes
    namespace_scope
    target_scope
    validity
    policy
}
```

Different actions MAY use different trust-anchor sets.

---

## 45. Trust stores

A trust store MUST be scoped by purpose where the distinction matters.

The fact that a root is trusted for public web TLS MUST NOT automatically make it trusted for:

- MNCS compiler identity;
- verifier identity;
- attestation endorsement;
- build signing;
- production deployment;
- trust-policy changes; or
- recovery authority.

---

## 46. Certification paths

Certification-path validation MAY be one credential realization.

```text
CredentialPath {
    leaf
    intermediates
    trust_anchor
    constraints
}
```

```text
PathValidationResult {
    path
    cryptographic_validity
    name_constraints
    policy_constraints
    temporal_validity
    revocation_status
    result
}
```

A valid path does not itself authorize an application action.

---

## 47. Foreign identity and federation

Foreign credentials crossing RFC 0015 boundaries MUST undergo explicit translation.

```text
ForeignCredential
    -> CredentialVerification
    -> ExternalIdentityClaim
    -> FederationPolicy
    -> LocalPrincipalClaim
```

No foreign issuer gains internal authority merely by presenting a cryptographically valid credential.

---

## 48. Identity conflict

Conflicting identity bindings MUST be preservable.

Example:

```text
Credential A: key K -> P
Credential B: key K -> Q
```

If both are independently supported under current trust policy, the result may be:

```text
CONFLICTED
```

rather than an arbitrary winner.

RFC 0018 determines claim disposition.

---

## 49. Identity uncertainty

Insufficient evidence MAY produce:

```text
UNSUPPORTED
UNRESOLVED
```

or a broader principal-set claim.

Lack of exact identity MUST NOT automatically be treated as authentication failure if policy only requires membership in a permitted principal class.

---

## 50. Key rotation

Key rotation SHOULD preserve principal identity when authorized by an explicit rotation relation.

```text
KeyRotationEvent {
    principal
    old_authenticator
    new_authenticator
    authorization
    validity_transition
    evidence
}
```

Historical signatures remain attributable to the historical binding when evidence remains valid.

---

## 51. Key compromise

Key compromise and principal compromise are distinct claims.

```text
KeyCompromised(K) != PrincipalCompromised(P)
```

A compromised key may be revoked and replaced while principal identity survives.

A compromised principal may require broader recovery and invalidation than one key.

---

## 52. Recovery

Identity and trust systems require explicit recovery semantics.

```text
RecoveryPolicy {
    subject
    authorized_recovery_principals
    threshold?
    prerequisites
    effects
}
```

Recovery may:

- bind a new authenticator;
- revoke old credentials;
- rotate trust roots;
- re-establish a workload identity;
- invalidate sessions; or
- transition a principal to a restricted state.

Recovery MUST be auditable and evidence-bearing.

---

## 53. Recovery is not ordinary authentication

A recovery credential can be more dangerous than an ordinary authenticator.

MNCS MUST NOT treat:

```text
CanRecover(P)
```

as equivalent to:

```text
CanActAs(P)
```

unless policy explicitly defines that equivalence.

Recovery authority SHOULD generally be separately scoped.

---

## 54. Attestation overview

Attestation reports evidence about properties or state of an attester.

It is distinct from authentication.

The canonical conceptual pipeline SHOULD align with the IETF RATS architecture:

```text
Attester
    -> Evidence

Verifier
    -> AttestationResult

RelyingPolicy
    -> ActionDecision
```

The RFC 0018 assurance system provides the relying-policy/appraisal layer.

---

## 55. Attester

An `Attester` is the entity/environment that produces attestation evidence.

An attester MAY be:

- hardware root;
- firmware component;
- boot environment;
- operating system;
- hypervisor;
- container runtime;
- workload;
- application;
- agent; or
- composite attestation environment.

Attester identity and attested subject identity may differ.

---

## 56. Attestation evidence

```text
AttestationEvidence {
    attester
    subject
    claims
    measurements
    freshness
    proof
    evidence_format
    dependencies
}
```

Attestation evidence is not intrinsically trusted merely because it parses or carries a valid signature.

---

## 57. Measurements

A `Measurement` reports observed state.

Examples:

```text
firmware hash
boot-chain digest
kernel measurement
runtime measurement
container image digest
verifier artifact hash
configuration digest
model identity
policy digest
```

A measurement does not by itself state whether the measured value is acceptable.

---

## 58. Reference values

A `ReferenceValue` describes expected or acceptable state under an appraisal policy.

```text
ReferenceValue {
    claim_class
    accepted_values_or_predicate
    target_scope
    validity
    issuer
    evidence
}
```

Attestation appraisal compares measurements/claims against applicable reference values.

```text
Measurement != ReferenceValue
```

---

## 59. Attestation endorsements

To avoid confusion with RFC 0024 integrity endorsement, this RFC uses `AttestationEndorsement` for contextual claims supplied by a manufacturer, operator, or other authority to support attestation appraisal.

```text
AttestationEndorsement {
    issuer
    subject
    claims
    validity
    evidence
}
```

This object does **not** perform RFC 0024 integrity reclassification.

---

## 60. Attestation verifier

An attestation verifier appraises evidence according to a defined verification/appraisal procedure.

```text
AttestationVerifier {
    principal
    method
    supported_formats
    trusted_claim_classes
}
```

Verifier identity and trust are RFC 0018 concerns.

---

## 61. Attestation result

```text
AttestationResult {
    evidence_id
    verifier
    appraisal_policy
    established_claims
    refuted_claims
    unresolved_claims
    freshness
    assumptions
    dependencies
}
```

An attestation result is evidence for relying policy.

It is not itself an unrestricted authorization token.

---

## 62. Trust and trustworthiness

RFC 0025 adopts the RATS distinction:

- **trustworthiness** is a property/evidence concerning the subject/system;
- **trust** is a decision by a relying party or policy.

Therefore:

```text
AttestationResult
    != TrustDecision
```

and:

```text
TrustworthyFor(X)
    != AuthorizedFor(Y)
```

unless an explicit assurance policy connects them.

---

## 63. Authentic attestation is not acceptable attestation

A system can authentically attest:

```text
measurement = H_bad
```

with perfect cryptographic integrity.

If the applicable reference values permit only:

```text
{H_good_1, H_good_2}
```

then the evidence may be authentic and fresh while the state is unacceptable.

The RFC MUST preserve both truths.

---

## 64. Attestation freshness

Freshness is part of attestation meaning.

Possible mechanisms include:

```text
NonceFreshness
TimestampFreshness
EpochFreshness
SequenceFreshness
SessionBinding
```

A valid historical attestation MUST NOT automatically establish current state.

---

## 65. Nonce freshness

A verifier MAY issue a challenge nonce.

The attestation proof binds evidence to that nonce.

This establishes challenge freshness without requiring synchronized physical clocks.

```text
AttestationChallenge {
    verifier
    nonce
    scope
}
```

---

## 66. Time freshness

Time-based freshness MUST use RFC 0023 clock and uncertainty semantics.

If the evidence validity boundary overlaps clock uncertainty, freshness may be `UNRESOLVED`.

---

## 67. Epoch freshness

MNCS SHOULD support epoch-based freshness.

Useful epochs may include:

```text
BootEpoch
DeploymentEpoch
FabricMembershipEpoch
RefinementEpoch
PolicyEpoch
TrustRootEpoch
```

Evidence may be valid only within a named epoch even if physical time remains acceptable.

---

## 68. Layered attestation

MNCS SHOULD support layered attestation relationships.

Example:

```text
hardware
    -> firmware
        -> bootloader
            -> kernel
                -> runtime
                    -> workload
                        -> agent
```

Each layer may establish claims about the next layer while retaining dependency information.

---

## 69. Composite attestation

A composite system may require several attesters.

```text
CompositeAttestationClaim {
    component_claims
    composition_rule
}
```

One failing or unresolved lower-layer dependency may weaken or invalidate a higher-layer claim depending on the relying policy.

RFC 0018 owns composition/appraisal.

---

## 70. Measured identity versus declared identity

A workload may declare:

```text
Principal = ForgeVerifier
```

while attestation establishes:

```text
ArtifactHash = H
```

These are different identities.

An explicit relation can claim:

```text
ExecutionInstance E
    acts_as Principal ForgeVerifier
    executing Artifact H
    on Platform P
```

That relation can be supported or refuted independently.

---

## 71. Platform identity

Platform principal identity is distinct from measured platform state.

```text
PlatformPrincipal != PlatformMeasurement
```

The same platform can transition through many measured states over time.

---

## 72. Hardware roots of trust

Hardware roots of trust are realizations, not mandatory semantics.

Potential realizations include:

```text
TPM
TEE
SecureEnclave
HSM
SecureBootChain
CloudAttestationService
SoftwareOnlyMeasurement
```

RFC 0025 defines the required claims/evidence relationships.

Target-specific realization belongs to RFC 0017 and later implementation work.

---

## 73. Attestation privacy

Attestation evidence can reveal sensitive information including:

- device identity;
- workload identity;
- hardware model;
- firmware version;
- security configuration;
- software inventory;
- deployment topology; or
- vulnerability-relevant state.

Attestation evidence MUST therefore remain ordinary information subject to RFC 0024 information-flow policy.

Attestation is not a security-exempt channel.

---

## 74. Identity privacy

MNCS MUST NOT require globally linkable identifiers for all principals.

The model SHOULD permit:

```text
ScopedPrincipalIdentifier
PairwisePrincipalIdentifier
PseudonymousPrincipal
EphemeralPrincipal
```

with explicit relations to underlying principals where policy authorizes such linkage.

This supports privacy and compartmentalization.

---

## 75. Credential privacy and selective disclosure

Credential realizations MAY support selective disclosure.

The semantic model remains claim-centric:

```text
which claim
about which subject
issued by whom
for which audience
under which authority
```

Selective disclosure does not change the meaning of the underlying claim.

---

## 76. Identity data is information-flow controlled

Principal bindings, credentials, authentication events, device identifiers, and trust-domain relations may themselves be confidential.

RFC 0024 labels and policies apply to identity artifacts.

No identity subsystem bypasses information-flow policy by default.

---

## 77. Cryptographic operations are effects

Operations such as:

```text
sign
authenticate
attest
retrieve_credential
rotate_key
revoke_credential
change_trust_anchor
```

are not pure computations.

Their authority must be represented through RFC 0008 capabilities/effects.

RFC 0025 defines what their resulting claims mean.

---

## 78. Key observation versus key use

Authority to inspect key metadata MUST remain distinct from authority to use a key.

Examples:

```text
ObserveKeyMetadata<K>
SignWith<K>
IssueCredentialWith<K>
AttestWith<K>
Rotate<K>
Destroy<K>
```

A process may possess a handle without possessing every use authority.

---

## 79. Root authority minimization

High-authority root credentials SHOULD be used to derive narrower intermediates and short-lived credentials where feasible.

Conceptually:

```text
RootTrustKey
    -> IntermediateIssuer
        -> ShortLivedWorkloadCredential
            -> SessionCredential
```

The derivation remains machine-readable and auditable.

---

## 80. Keyless and ephemeral signing realizations

RFC 0025 permits identity systems where persistent semantic identity is bound to ephemeral signing keys through short-lived credentials and external authentication.

This direction is exemplified by Sigstore-like workflows.

The constitutional rule is:

```text
PersistentPrincipalIdentity
    does not require
PersistentSigningKey
```

Transparency or witness evidence remains governed by RFC 0018 and future RFC 0031.

---

## 81. Transparency is not legitimacy

A transparency log may establish that a binding or signing event was recorded consistently.

It does not by itself establish that the event was authorized or correct.

```text
Logged(binding) != Legitimate(binding)
```

The distinction follows RFC 0018 witness semantics.

---

## 82. Cryptographic proof objects

Cryptographic verification results SHOULD identify their exact scope.

```text
CryptographicProofResult {
    algorithm
    key
    input
    proof
    purpose
    domain_separation
    result
    assumptions
}
```

The result MUST NOT be flattened to an unqualified `verified = true`.

---

## 83. Algorithm policy

Algorithm acceptability is separate from proof validity.

```text
ProofValidUnderAlgorithm(A)
```

may be true while:

```text
AlgorithmAcceptedForProduction(A)
```

is false.

Algorithm policy may depend on:

- date/epoch;
- key size;
- target environment;
- claim class;
- security profile;
- implementation assurance; or
- migration status.

---

## 84. Foreign credential translation

RFC 0015 foreign boundaries SHOULD expose foreign identity material explicitly.

A foreign token/certificate MUST pass through:

```text
ForeignCredentialValidation
ForeignIssuerAppraisal
ClaimTranslation
LocalPolicyAppraisal
```

before becoming an internal principal or authority claim.

---

## 85. Identity changes as semantic deltas

Recursive refinement SHOULD expose identity/trust changes as deltas.

```text
IdentityDelta {
    new_bindings
    removed_bindings
    rotated_authenticators
    changed_issuer_authority
    changed_trust_roots
    revoked_credentials
    changed_attestation_requirements
    authority_broadening
    authority_narrowing
}
```

Authority broadening MUST be particularly visible.

---

## 86. Trust deltas

A candidate transformation or deployment may change trust dependencies even when functional behavior is unchanged.

Examples:

- new certificate authority dependency;
- replacing local verifier with remote verifier;
- adding cloud attestation service;
- widening credential audience;
- changing workload trust domain;
- changing proof-of-possession requirement to bearer;
- weakening freshness requirement; or
- adding delegation capability.

These MUST be visible in candidate comparison.

---

## 87. Forge identity-realization search

Forge MAY search identity/authentication/credential realizations while protected semantic requirements remain fixed.

Example requirement:

```text
Required principal class:
    MNCSVerifier

Authentication:
    proof of possession

Authority:
    read baseline
    read candidate
    emit verifier result

Forbidden:
    mutate baseline
    change assurance policy

Credential lifetime:
    <= 15 minutes

Attestation:
    approved verifier artifact
    approved runtime environment
```

Candidate realizations may include:

```text
short-lived workload certificate
PoP token
SPIFFE-like SVID
hardware-backed credential
MNCS-native capability credential
```

No realization is preferred merely because it is more powerful.

---

## 88. Least sufficient credential principle

MNCS SHOULD prefer the least authority that satisfies the protected requirements, subject to declared optimization preferences.

A longer-lived, broader credential is not inherently stronger in the sense relevant to safe authorization.

This RFC forbids a universal credential-strength ladder.

---

## 89. Forge candidate example

Candidate A:

```text
long-lived bearer token
broad repo read/write authority
no attestation binding
```

Candidate B:

```text
10-minute PoP credential
read-only baseline access
emit-result capability
approved attestation
```

Candidate C:

```text
valid short-lived credential
approved principal
stale verifier attestation
```

Candidate D:

```text
valid attestation
credential includes baseline mutation
```

Expected policy outcomes:

```text
A -> REJECTED for required profile
B -> AUTHORIZED
C -> UNRESOLVED or REJECTED depending action policy
D -> REJECTED due authority broadening
```

The result demonstrates that credential power is not equivalent to credential suitability.

---

## 90. Principal claims

The initial canonical artifact family SHOULD include:

```text
PrincipalClaim {
    id
    subject
    principal
    claim_kind
    namespace
    context
    validity
    assumptions
    witness
    evidence
}
```

Claims remain proof-relevant and scope-specific.

---

## 91. Authentication artifacts

The initial canonical artifact family SHOULD include:

```text
AuthenticationClaim {
    id
    principal
    authenticator
    protocol
    verifier
    relying_context
    freshness
    assumptions
    witness
    evidence
}
```

A machine-readable explanation MUST identify why authentication is accepted, rejected, or unresolved.

---

## 92. Credential artifacts

The initial canonical artifact family SHOULD include:

```text
Credential {
    id
    issuer
    subject
    claims
    proof_binding
    audience
    purpose
    validity
    status_reference
    representation_ref
}
```

and:

```text
CredentialStatusEvidence
IssuerAuthority
CredentialPath
PathValidationResult
```

---

## 93. Delegation artifacts

The initial canonical artifact family SHOULD include:

```text
AuthorityScope
Delegation
DelegationPath
Caveat
AttenuationClaim
ThresholdPrincipal
```

The verifier MUST reject a child delegation that broadens authority without an independent authority source.

---

## 94. Attestation artifacts

The initial canonical artifact family SHOULD include:

```text
AttestationEvidence
Measurement
ReferenceValue
AttestationEndorsement
AttestationResult
FreshnessEvidence
```

The model MUST permit established, refuted, and unresolved attestation claims simultaneously across different claim classes.

---

## 95. Relation integration with RFC 0020

RFC 0025 uses RFC 0020 relation machinery for claims such as:

```text
PrincipalIdentityRelation
NameBindingRelation
KeyBindingRelation
AuthorityAttenuation
CredentialSemanticEquivalence
AuthenticationSubstitutability
TrustDomainCompatibility
AttestationRefinement
```

Examples of intentionally distinct relations:

```text
same principal
same key
same credential
same current authority
same trust domain
same measured platform state
```

A pair may satisfy one while failing another.

---

## 96. Assurance integration with RFC 0018

RFC 0025 MUST NOT define a universal final `trusted` bit.

RFC 0018 consumes:

- principal claims;
- authentication claims;
- issuer authority;
- credential proof results;
- credential status;
- delegation derivations;
- attestation results;
- reference values;
- trust anchors;
- freshness evidence;
- conflicting identity evidence; and
- environment/target facts.

It then returns action-relative decisions such as:

```text
AUTHORIZED
REJECTED
UNRESOLVED
```

for specific actions.

---

## 97. Information-flow integration with RFC 0024

Identity artifacts are ordinary information.

RFC 0024 governs whether principals may observe:

- real-world identity bindings;
- pseudonym mappings;
- device identity;
- credential contents;
- attestation measurements;
- trust relations;
- revocation reason; or
- deployment topology.

Authentication/attestation mechanisms MUST NOT become implicit declassification operations.

---

## 98. Runtime and target integration with RFC 0017

Runtime facts may affect:

- credential availability;
- authenticator capabilities;
- hardware key binding;
- attestation support;
- trust-domain membership;
- cryptographic algorithm support;
- revocation connectivity; or
- secure storage guarantees.

Such facts are target/runtime evidence, not intrinsic principal meaning.

---

## 99. Unsafe/foreign integration with RFC 0015

Unsafe and foreign interfaces MUST NOT silently erase identity/trust requirements.

Foreign credentials, OS accounts, platform tokens, hardware attestations, and external identity-provider assertions must enter through typed trust-boundary translation artifacts.

---

## 100. Failure semantics with RFC 0011

Authentication and credential operations may fail with semantically distinct outcomes:

```text
BadProof
UnknownPrincipal
ExpiredCredential
RevokedCredential
StatusUnknown
AudienceMismatch
FreshnessFailure
IssuerUntrusted
ClaimUnauthorized
AttestationUnacceptable
AttestationUnresolved
```

These failures MUST NOT be flattened when policy distinguishes them.

Failure information itself may be RFC 0024-sensitive.

---

## 101. Temporal integration with RFC 0023

RFC 0023 owns:

- credential validity intervals;
- timestamp-based freshness;
- revocation/status freshness;
- trust-anchor validity;
- policy validity;
- attestation validity; and
- temporal uncertainty.

RFC 0025 consumes those semantics.

---

## 102. Evidence invalidation

Changes to any of the following SHOULD invalidate dependent identity/trust claims:

- principal-key binding;
- trust anchor;
- issuer authority;
- credential status;
- federation policy;
- audience/purpose scope;
- authenticator compromise;
- cryptographic algorithm policy;
- attestation reference value;
- freshness context;
- trust-domain membership; or
- verifier identity/trust.

Invalidation SHOULD remain dependency-local where possible.

---

## 103. No universal authentication score

MNCS MUST NOT introduce one scalar `authentication_strength` as canonical semantics.

Authentication evidence may have independent dimensions such as:

```text
identity_proofing_basis
authenticator_class
proof_of_possession
phishing_resistance
hardware_binding
user_presence
verifier_binding
channel_binding
freshness
recovery_risk
```

Different action policies may value those dimensions differently.

---

## 104. No universal trust score

Likewise, MNCS MUST NOT define:

```text
trust = 0.93
```

without an explicit uncertainty model and claim semantics.

Trust is action-relative relying policy over evidence.

---

## 105. No silent trust transitivity

The compiler/runtime MUST NOT infer:

```text
A trusts B
B trusts C
therefore A trusts C
```

unless a declared trust/delegation rule establishes exactly that relation and scope.

---

## 106. No silent name-to-identity equivalence

Equal names do not establish principal equality.

Equal certificate subject strings do not establish principal equality.

Equal account names do not establish principal equality.

---

## 107. No silent key-to-identity permanence

Equal key identity does not establish eternal principal identity.

Bindings have context, issuer, validity, and history.

---

## 108. No silent credential-to-authorization upgrade

Cryptographically valid credentials MUST still pass:

- issuer-authority appraisal;
- subject/audience/context validation;
- status/freshness checks; and
- action policy.

---

## 109. No silent attestation-to-trust upgrade

Authentic and fresh attestation MUST still be appraised against applicable reference values and relying policy.

---

## 110. No silent signature-to-safety upgrade

A signed artifact may be:

- malicious;
- incorrect;
- vulnerable;
- policy-incompatible; or
- signed by an unauthorized issuer.

Signature validity is not semantic safety.

---

## 111. No silent certificate-path-to-action upgrade

A valid certificate path proves only the path claim under its policy.

Application authority requires a separate action policy.

---

## 112. No silent bearer-to-PoP substitution

A bearer credential and a proof-of-possession credential are not interchangeable security relations.

A realization change from PoP to bearer is a security delta requiring explicit review.

---

## 113. No silent authority broadening

Any candidate transformation that broadens:

- action set;
- resource scope;
- audience;
- validity;
- delegation rights;
- trust roots; or
- accepted issuer classes

MUST expose an authority-broadening delta.

Forge may not hide this behind functional equivalence.

---

## 114. Reference theories and precedents

The design is informed by, but does not canonically adopt, the following theory/standards families:

- NIST Digital Identity Guidelines for separation of identity proofing, enrollment, authentication, authenticators, federation, and assertions;
- SPKI/SDSI for key-centered principals, local names, direct authorization, delegation, threshold subjects, and validity;
- capability-security attenuation principles for monotonic narrowing of authority;
- macaroons for contextual cryptographic caveats and decentralized attenuation;
- X.509/PKIX for certification paths, trust anchors, constraints, and revocation-oriented PKI semantics;
- OCSP-style explicit `good`/`revoked`/`unknown` credential-status semantics;
- WebAuthn/FIDO-style relying-party-scoped public-key credentials and authenticator binding;
- SPIFFE/SVID for workload identity, trust domains, federation, and short-lived credentials;
- W3C Verifiable Credentials for issuer/holder/verifier claim structures and selective presentation;
- proof-of-possession token models for separating bearer possession from bound-key presentation;
- IETF RATS for Attester → Evidence → Verifier → Attestation Result → Relying Party and the trust/trustworthiness distinction;
- EAT as one portable attestation-claim encoding family;
- layered/composite attestation models;
- Sigstore-style ephemeral/keyless signing bindings and transparency-supported provenance; and
- threshold/multi-principal authorization for critical security actions.

These are sources of theory and realization patterns, not the MNCS ontology itself.

---

## 115. Bounded implementation pilot

The first executable RFC 0025 pilot SHOULD remain deliberately bounded.

### 115.1 Initial artifacts

Implement versioned forms of:

```text
Principal
PrincipalKind
NameBinding
TrustDomain
PrincipalAuthenticatorBinding
AuthenticationClaim
Credential
CredentialStatusEvidence
IssuerAuthority
AuthorityScope
Delegation
Caveat
DelegationPath
TrustAnchor
FederationRelation
AttestationEvidence
Measurement
ReferenceValue
AttestationResult
IdentityDelta
TrustDelta
```

### 115.2 Fixture A — principal identity survives key rotation

```text
P -> K1
rotate
P -> K2
```

Expected:

```text
PrincipalIdentity(P_before, P_after) = preserved
KeyIdentity(K1, K2) = different
```

### 115.3 Fixture B — valid signature does not authorize

Credential signature verifies.

Issuer lacks authority for requested claim class.

Expected:

```text
CryptographicValidity = PASS
ActionAuthorization = REJECTED
```

### 115.4 Fixture C — bearer versus proof of possession

A copied bearer credential remains usable under bearer semantics.

A copied PoP credential without the bound key fails authentication.

Expected distinct relation results.

### 115.5 Fixture D — authority attenuation

Parent:

```text
repo X: read/write
```

Child:

```text
repo X: read
```

Expected delegation accepted.

### 115.6 Fixture E — privilege-escalation rejection

Parent:

```text
repo X: read
```

Child attempts:

```text
repo X: read/write
```

Expected delegation rejected unless an independent write-authority path exists.

### 115.7 Fixture F — caveat accumulation

Parent credential:

```text
repo X
```

Delegated credential adds:

```text
branch feature/*
expires T
```

Expected effective authority equals parent authority intersected with new caveats.

### 115.8 Fixture G — revocation status unknown

Credential proof is valid.

Revocation source unavailable.

Expected:

```text
CredentialStatus = UNKNOWN
```

and policy MUST NOT silently relabel it `GOOD`.

### 115.9 Fixture H — federation claim rejection

Foreign credential validates cryptographically.

Federation policy does not authorize foreign issuer for requested internal claim class.

Expected local claim rejected.

### 115.10 Fixture I — authentic acceptable attestation

Attestation reports:

```text
artifact = H1
```

Applicable reference values permit:

```text
{H1, H2}
```

Freshness valid.

Expected state claim supported.

### 115.11 Fixture J — authentic unacceptable attestation

Attestation is cryptographically valid and fresh but reports:

```text
artifact = Hbad
```

Expected:

```text
AttestationAuthentic = true
ApprovedState = false
```

### 115.12 Fixture K — stale attestation

Evidence signature validates but nonce/epoch freshness fails.

Expected current-state claim rejected or unresolved according to profile.

### 115.13 Fixture L — agent delegation

Controller spawns Agent A.

Agent A spawns Agent B.

Lineage shows ancestry.

Authority grants only:

```text
A: read candidate + request verifier
B: read candidate only
```

Expected lineage preserved without authority inheritance.

---

## 116. First Forge experiment

The first cross-component Forge experiment SHOULD request a verifier worker with:

```text
principal class:
    MNCSVerifier

authentication:
    proof_of_possession

authority:
    read baseline
    read candidate
    emit verifier result

forbidden:
    mutate baseline
    modify assurance policy
    delegate mutation authority

attestation:
    approved verifier artifact
    approved runtime environment

credential lifetime:
    <= 15 minutes
```

Evaluate at least four candidate realizations:

```text
A. broad long-lived bearer credential
B. short-lived PoP credential with exact authority + fresh attestation
C. correct credential with stale/unresolved attestation
D. correct identity + attestation but broader mutation authority
```

The experiment MUST demonstrate:

```text
A is not preferred merely because it is broader/longer-lived.
B is authorized.
C does not become authorized because credential proof succeeds.
D is rejected for authority broadening despite valid identity evidence.
```

This should become the first executable proof of the least-sufficient-credential principle.

---

## 117. Diagnostics

Diagnostics SHOULD be semantic and scoped.

Examples:

```text
E2501 principal name is ambiguous across namespaces
E2502 authenticator is not currently bound to principal
E2503 authentication proof is valid but stale
E2504 credential proof valid; issuer unauthorized for claim class
E2505 credential status UNKNOWN under required fail-closed profile
E2506 delegation broadens parent authority
E2507 child delegation removes inherited caveat
E2508 trust anchor not authorized for requested purpose
E2509 federation relation does not permit requested claim translation
E2510 attestation proof valid but measured state is not an approved reference value
E2511 attestation freshness unresolved
E2512 current principal relation conflicted
E2513 PoP credential presented without bound-key proof
E2514 candidate broadens delegation rights
E2515 algorithm verifies cryptographically but is disallowed by current policy
```

Diagnostics MUST distinguish cryptographic failure from policy rejection whenever that difference affects repair.

---

## 118. Security consequences

This model intentionally makes several common shortcuts illegal or visible:

- a stolen bearer credential remains a bearer-risk issue rather than being mislabeled authentication failure;
- a valid but overprivileged credential can be rejected even when every signature validates;
- a fresh attestation can prove an unacceptable state;
- a stale attestation can remain cryptographically authentic without proving current state;
- key rotation can preserve principal identity without pretending keys are identical;
- a foreign identity can be authenticated but unauthorized in the local trust domain;
- a spawned agent can have lineage without inheriting parental authority;
- a trust anchor can be valid for one claim class while unusable for another;
- a valid credential with unknown revocation status can remain unresolved; and
- a cryptographic algorithm can remain mathematically functional while unacceptable under current policy.

---

## 119. Scope boundaries

### RFC 0008

Owns capability/effect authority for authentication, signing, credential retrieval, key use, attestation, and trust-store mutation.

### RFC 0009

Owns memory/reference/storage provenance of credential/key handles and persistent identity artifacts.

### RFC 0015

Owns unsafe/foreign boundaries through which external identity and attestation mechanisms enter.

### RFC 0017

Owns runtime/target facts about available authenticators, hardware roots, cryptographic support, secure storage, and attestation mechanisms.

### RFC 0018

Owns evidence appraisal, trust policy, freshness, conflict, independence, witnesses, and action-relative authorization.

### RFC 0020

Owns identity/equality/refinement relation machinery used to state principal identity, credential equivalence, delegation attenuation, and attestation refinement.

### RFC 0023

Owns temporal validity, clock uncertainty, credential expiry, freshness windows, and time-dependent trust applicability.

### RFC 0024

Owns confidentiality/integrity policy over identity, credentials, trust relations, and attestation evidence.

### RFC 0026

Will own durable storage, crash consistency, revocation persistence, and stateful identity/trust transaction behavior.

### RFC 0028

Will own distributed membership, partial failure, replicated trust state, and identity behavior under distributed partitions.

### RFC 0030

Will own deployment/live-upgrade transitions involving identity and attestation migration.

### RFC 0031

Will own artifact/build lineage, software-supply-chain attestations, signing provenance, and transparency integration at build/deployment scale.

---

## 120. 1.0 research threshold implications

RFC 0025 should contribute the following minimum requirements to the MNCS 1.0 research threshold:

- a versioned principal/identity model that distinguishes principal, name, namespace, key/authenticator, credential, workload, artifact, platform, execution-instance, and session identity;
- a versioned authentication/credential model that distinguishes identity proofing, enrollment, cryptographic validity, proof of possession, authentication, authorization, issuer authority, audience/context scope, expiration, revocation, and unknown status;
- a versioned delegation model with explicit authority scope, delegation rights, caveats, provenance, and machine-checked attenuation;
- a versioned trust-domain/federation model that keeps trust roots claim-class specific and does not assume global trust transitivity;
- a versioned attestation model separating attester, evidence, measurements, reference values, attestation endorsements, verifier appraisal, attestation result, freshness, trustworthiness, and final relying decisions;
- at least one key-rotation study proving principal identity can be preserved while key identity changes;
- at least one credential study where cryptographic validity passes while authorization fails because issuer authority or action scope is insufficient;
- at least one delegation study where a narrower child authority is accepted and a broader child authority is mechanically rejected;
- at least one credential-status study where `UNKNOWN` remains distinct from `GOOD`;
- at least one attestation study where evidence is authentic but the measured state is unacceptable, demonstrating `authentic attestation != trusted state`;
- at least one freshness study where valid historical attestation fails to establish current state;
- at least one Forge realization study where a broader/longer-lived credential is rejected in favor of a least-sufficient, short-lived, proof-of-possession credential; and
- documented trust-anchor, issuer, principal, credential, delegation, algorithm, freshness, federation, and attestation assumption limits.

---

## 121. Open research questions

The RFC intentionally leaves several questions open for experimentation:

- how much of principal structure belongs in the proof kernel versus certificate verifiers;
- whether canonical principal IDs should be content-derived, generative, hierarchical, or hybrid;
- how to model anonymous/pseudonymous principal relations without undermining auditability;
- how threshold principals compose with nested delegation;
- how capability tokens and credential claims interoperate without duplicating authority semantics;
- how much credential inference should happen statically versus dynamically;
- whether caveat predicates should share the general contract language;
- how independent trust domains negotiate federated claim vocabularies;
- how attestation reference values evolve across live upgrades;
- how to represent compromise confidence without universal scalar trust;
- how proof-carrying credentials might integrate with RFC 0007;
- how post-quantum algorithm migration affects long-lived historical identity evidence;
- how agent principals and `mncs-lineage` should eventually relate to parent/child model evolution;
- how principal privacy and auditability trade off under different assurance profiles; and
- how much credential/attestation evidence should be retained after authorization decisions.

None of these open questions weakens the constitutional distinctions established here.

---

## 122. Summary of constitutional rules

1. **Principal is not name.**
2. **Principal is not key.**
3. **Key is not credential.**
4. **Credential is not authentication.**
5. **Authentication is not authorization.**
6. **Authorization is not delegation authority.**
7. **Identity proofing is not enrollment.**
8. **Enrollment is not current authentication.**
9. **Credential cryptographic validity is not claim truth.**
10. **Claim authenticity is not issuer authority.**
11. **Certificate-path validity is not application authorization.**
12. **Trust is scoped and policy-relative.**
13. **Trustworthiness is evidence; trust is a relying decision.**
14. **Trust is not silently transitive.**
15. **Trust anchors are scoped policy roots, not universal truth roots.**
16. **Names are namespace-relative.**
17. **Long-lived principal identity does not require long-lived credentials.**
18. **Key rotation need not change principal identity.**
19. **Key compromise need not imply permanent principal loss.**
20. **Bearer and proof-of-possession credentials are different security relations.**
21. **Delegation may attenuate but may not silently broaden authority.**
22. **Permission to act does not imply permission to delegate.**
23. **Caveats accumulate unless independent authority explicitly establishes broader rights.**
24. **Expiration is not revocation.**
25. **UNKNOWN revocation/status is not GOOD.**
26. **Authentication and attestation both require freshness where replay matters.**
27. **Attestation is not authentication.**
28. **Attestation is not authorization.**
29. **Authentic attestation is not necessarily acceptable attestation.**
30. **Measurements are not reference values.**
31. **Hardware roots are realizations, not language requirements.**
32. **Identity and attestation evidence remain subject to RFC 0024 information-flow policy.**
33. **External identity is not automatically internal identity.**
34. **Algorithm validity is not policy acceptability.**
35. **No universal authentication-strength score exists.**
36. **No universal trust score exists.**
37. **A more powerful credential is not necessarily a better credential.**
38. **Authority broadening is a first-class semantic delta.**
39. **Principal lineage is not authority lineage.**
40. **Final action authority remains an RFC 0018 assurance decision.**

---

## 123. Closing principle

The RFC can be summarized by the following model:

```text
Principal
    != Name
    != Key
    != Credential

Credential proof
    -> establishes authentic issuance claim

Authentication
    -> establishes current actor relation

Delegation
    -> establishes bounded authority derivation

Attestation
    -> establishes evidence about state

RFC 0018 appraisal
    -> decides whether current evidence authorizes exact action A
```

MNCS should therefore reason about identity as a graph of scoped bindings and current evidence rather than a username, certificate, or key masquerading as a complete security model.

The machine-native objective is:

> **Represent exactly who or what a machine believes it is dealing with, why that belief is justified, what authority follows from it, what state has actually been attested, and which assumptions remain unresolved—then authorize only the narrow action that the current evidence legitimately supports.**
