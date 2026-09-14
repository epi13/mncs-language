# Selective-development native policy evidence

The selective-development campaign moved Ravel's level and escalation
decision into `library/family/verification_plan.mncs`.

The native module receives a bounded integer membrane for the compiler impact
facts and returns a typed `SelectionDecision` record. Ravel retains only the
filesystem/process transport, canonical plan construction, and a documented
offline fallback for provider-failure and fixture callers. The normal plan
request invokes `mncs.family.verification_plan.v1::select_codes`.

The executable regression cases are in
`crates/mncs-cli/tests/verification_plan_policy.rs` and cover:

- direct dependents remaining at `direct_dependents` with
  `direct_dependents_affected`;
- public-contract risk promoting to `repository_canonical` with
  `public_contract_changed`; and
- parallel native invocations using independent request identities.

During implementation, the policy was deliberately kept within the current
language profile. A compact boolean-negation expression was not accepted by
the active frontend/runtime boundary, so the policy uses explicit integer
flags. This is a revalidation of the existing Commons pressure
`MNCS-COMPILER-723468BE594D` / legacy CP-0015; it is not a new duplicate
pressure and is not claimed resolved by this campaign.

The remaining host-language code is therefore bounded transport and
serialization. Selection semantics have one native implementation in the
normal path, with the Python fallback clearly limited to offline failure
handling until every embedding caller supplies the native runtime.
