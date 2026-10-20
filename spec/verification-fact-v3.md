# Language verification facts (v3)

`mncs.language-verification-fact/3` records a successful Language function-body
validation that may be considered for reuse in a later validation of the same
current program. The reuse observation remains
`mncs.language-verification-fact-reuse-observation/2`. Facts are evidence
supplied back to Language; they are not executable artifacts or replacements
for current declaration validation. Language independently reconstructs the
current subject and every dependency guard before skipping a body check.

## Fact contents

Each fact contains:

- `subject` and `subject_fingerprint`: the existing function-body semantic
  identity and body fingerprint;
- `declaration_interface`: the checked function's callable interface,
  excluding its implementation body;
- `dependencies`: sorted guards for callable interfaces, callable
  implementations, nominal types, assumptions, generic specializations,
  effects, and capabilities. Each guard uses an existing Language semantic
  identity and its current semantic fingerprint;
- `verifier_identity`, `policy_identity`, and `language_profile`: the
  implementation and assumptions under which the body was checked;
- the declared effect and capability surfaces, successful result, witness
  identity, invalidating changes, and optional source-location identity.

Call and nominal dependencies come from typed semantic references and resolved
type identities. A call guards the interface and implementation of every
function in its reachable call closure, including dependencies used by those
implementations. A change at any reachable implementation therefore
invalidates its transitive callers, while a change to an unrelated function
does not. Recursive call graphs are visited once per semantic identity. A
generic call also guards the exact selected specialization and the nominal
type arguments it carries. Effect and capability assumptions keep
role-specific guards and invalidation records. Assumption dependencies remain
conservative while their body references are string-addressed.

`verification_witness_identity` is SHA-256 over all semantic fact fields
except the independently refreshed source-location identity. It detects
accidental corruption and inconsistent records; it is not a signature and
does not authenticate an untrusted producer.

## Reuse rules

`Program::validate_with_verified_facts` still performs current whole-program
declaration validation. It reuses a body fact only when there is exactly one
prior fact for the same subject, the schema and result are supported, the
witness matches, and the subject fingerprint, callable interface, complete
dependency set, verifier, policy, language profile, effects, and capabilities
match current values. Effect and capability edits report role-specific misses.
Missing, duplicated, malformed, unsupported, or stale facts cause ordinary
body verification and a reuse-miss observation.

Only successfully verified bodies produce facts. Failed validation produces
fresh diagnostics and cannot publish a fact for the failed body. Diagnostics
and source locations come from the current source snapshot; source-map identity
changes do not force semantic body checking.

Version 3 does not accept version 1 or 2 facts. Old entries are recomputed once
and replaced with v3 evidence. A verifier or policy change must use a new
identity. A future transport that accepts facts from an untrusted machine
must add authenticated producer provenance before those facts can be treated
as trusted evidence.

The v3 wire shape is unchanged by transitive call-closure guards. The verifier
identity advances to `mncs-language-function-body-validator/4`, so v3 facts
issued by verifier version 3 are recomputed before the new dependency closure
can be reused.

Compiler owns application-build orchestration and executable artifact
generation. Language owns these semantic facts and independently checks every
guard before reuse. The witness is a content identity for Language-produced
evidence, not a digital signature; cross-machine or untrusted-store transport
needs authenticated producer provenance.

Version 3 reuses function-body validation only. It does not claim reuse of
HIR, SSA, proof admission, or backend artifacts.
