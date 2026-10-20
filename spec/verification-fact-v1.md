# Language verification facts

`mncs.language-verification-fact/1` records a successful Language function
body validation that may be considered for reuse in a later validation of the
same current program. A fact is evidence supplied back to Language; it is not
an executable artifact or replacement for current declaration validation,
and it is not an instruction to skip validation. Language independently
reconstructs the current subject and all supported dependency guards before
reuse.

## Fact contents

Each fact contains:

- `subject` and `subject_fingerprint`: the existing function-body semantic
  identity and body fingerprint;
- `declaration_interface`: the existing callable identity and a fingerprint
  of its inputs, outputs, contracts, declared assumptions, effects, and
  capabilities, excluding the implementation body;
- `dependencies`: sorted guards for callable interfaces, nominal types,
  assumptions, and generic specializations referenced by the body;
- `verifier_identity`, `policy_identity`, and `language_profile`: the
  implementation and assumptions under which the body was checked;
- `declared_effects` and `declared_capabilities`: the exact effect surface
  checked for the function;
- `result`: `verified` only when the body check succeeded;
- `verification_witness_identity`: SHA-256 over all semantic fact fields
  except the independently refreshed source-location identity;
- `invalidating_changes`: the subject, interface, dependency, verifier,
  policy, and profile changes that invalidate the fact;
- optional `source_location_identity`: the current source-map identity,
  refreshed independently and excluded from semantic reuse decisions.

The witness detects accidental corruption and inconsistent records. It is not
a signature and does not authenticate an untrusted producer.

## Reuse rules

`Program::validate_with_verified_facts` always performs current whole-program
declaration validation. It reuses a body fact only when there is exactly one
prior fact for the same subject, the schema and result are supported, the
witness matches, and the subject fingerprint, callable interface, complete
dependency set, verifier, policy, language profile, effects, and capabilities
match current values. Missing, duplicated, malformed, unsupported, or stale
facts cause ordinary body verification and a precise reuse-miss observation.

Only successfully verified bodies produce facts. Failed or interrupted
validation therefore cannot publish a successful fact. Diagnostics and
source locations are produced from the current source snapshot; source-map
identity changes do not force semantic body checking.

Dependency extraction is intentionally conservative and versioned with this
contract. A verifier or policy change must use a new identity. A future
transport that accepts facts from an untrusted machine must add authenticated
producer provenance before those facts can be treated as trusted evidence.

The current Compiler integration stores these facts with the application
response in its atomic, checksummed local cache, then passes them back to
Language for independent guard checks. The witness is a content identity for
Language-produced verification evidence, not a digital signature. Cross-
machine or untrusted-store transport needs authenticated producer provenance.

Version 1 reuses function-body validation only. It does not claim reuse of
HIR, SSA, proof admission, executable artifacts, or downstream callers'
behavioral properties.
