# Language verification facts (v2)

This document records the historical v2 contract. The current contract is
[`verification-fact-v3.md`](verification-fact-v3.md).

`mncs.language-verification-fact/2` records a successful Language function-body
validation that may be considered for reuse in a later validation of the same
current program. Its reuse observation is
`mncs.language-verification-fact-reuse-observation/2`. Facts are evidence
supplied back to Language; they are not executable artifacts or replacements
for current declaration validation. Language independently reconstructs the
current subject and every supported dependency guard before it skips a body
check.

## Fact contents

Each fact contains:

- `subject` and `subject_fingerprint`: the existing function-body semantic
  identity and body fingerprint;
- `declaration_interface`: the existing callable identity and a fingerprint
  of its inputs, outputs, contracts, declared assumptions, effects, and
  capabilities, excluding the implementation body;
- `dependencies`: sorted guards for callable interfaces, nominal types,
  assumptions, generic specializations, effects, and capabilities referenced
  by the body. Each guard uses the existing Language semantic identity and its
  current semantic fingerprint. Effect and capability guards retain their
  own dependency roles so invalidation identifies which proof assumption
  changed;
- `verifier_identity`, `policy_identity`, and `language_profile`: the
  implementation and assumptions under which the body was checked;
- `declared_effects` and `declared_capabilities`: the exact effect surface
  checked for the function;
- `result`: `verified` only when the body check succeeded;
- `verification_witness_identity`: SHA-256 over all semantic fact fields
  except the independently refreshed source-location identity;
- `invalidating_changes`: the subject, interface, dependency, effect,
  capability, verifier, policy, and profile changes that invalidate the fact;
- optional `source_location_identity`: the current source-map identity,
  refreshed independently and excluded from semantic reuse decisions.

The witness detects accidental corruption and inconsistent records. It is not
a signature and does not authenticate an untrusted producer.

## Reuse rules

`Program::validate_with_verified_facts` still performs current whole-program
declaration validation. It reuses a body fact only when there is exactly one
prior fact for the same subject, the schema and result are supported, the
witness matches, and the subject fingerprint, callable interface, complete
dependency set, verifier, policy, language profile, effects, and capabilities
match current values. A changed effect or capability assumption has a
role-specific recomputation reason. Missing, duplicated, malformed,
unsupported, or stale facts cause ordinary body verification and a precise
reuse-miss observation.

Only successfully verified bodies produce facts. Failed validation produces
fresh diagnostics and cannot publish a fact for the failed body. Diagnostics
and source locations come from the current source snapshot; source-map identity
changes do not force semantic body checking.

Version 2 does not accept version 1 facts. Old cache entries are recomputed
once and replaced with version 2 evidence.

Call, generic-specialization, and nominal-type dependencies are derived from
the typed body references and resolved type identities. Arbitrary strings in
call labels, diagnostics, or data do not create those dependency edges.
Assumption dependencies remain conservative while assumption references are
string-addressed in the body model. This extraction change uses verifier
identity `mncs-language-function-body-validator/2`, so facts issued by the
previous dependency algorithm are recomputed. A policy change must likewise
use a new policy identity. A future transport that accepts facts from an
untrusted machine must add authenticated producer provenance before those
facts can be treated as trusted evidence.

The current Compiler integration stores these facts with the application
response in its atomic, checksummed local cache, then passes them back to
Language for independent guard checks. The witness is a content identity for
Language-produced verification evidence, not a digital signature. Cross-machine
or untrusted-store transport needs authenticated producer provenance.

Version 2 reuses function-body validation only. It does not claim reuse of
HIR, SSA, proof admission, executable artifacts, or downstream callers'
behavioral properties.
