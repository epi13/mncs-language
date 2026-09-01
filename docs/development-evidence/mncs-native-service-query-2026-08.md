# MNCS-native language-service query pressure — 2026-08

Status: **implemented / exercised, experimental**.

The first operational MNCS-native semantic query in
[`mncs-language-service`](https://github.com/epi13/mncs-language-service) is a
bounded obligation-status summary. The service projects the authoritative
Rust frontend's obligation statuses into the language-owned `Status` lattice,
executes `mncs.core.status.v1::summarize8` through the research-bytecode
backend, validates the returned nominal record, and compares it with the Rust
control result. A mismatch or unsupported execution is reported as
`UNSUPPORTED`; the service does not guess or silently promote the result.

## Language pressure

The service adapter initially exposed a real compiler issue: an imported
function signature such as `[Status; 8]` was reconstructed with a textual
`Named("Status")` sequence element, while the local call site used the
identity-bearing imported `Status` type. The authoritative frontend therefore
rejected a semantically valid wrapper at the module boundary.

The compiler now resolves bounded-sequence element names in imported function
signatures against the declaring module's nominal namespace and canonicalizes
the resulting nested nominal type. This is a general module-linking fix, not a
service-specific type workaround. Unknown imported sequence elements still
fail closed with a diagnostic.

## Evidence

- `imported_bounded_sequences_retain_nominal_element_identity` proves the
  positive frontend/linking case.
- `imported_bounded_sequences_with_unknown_elements_fail_closed` proves that
  unresolved element types are not accepted by the new path.
- `mncs-language-service` differentially exercises the real `StatusSummary`
  kernel against `LanguageService::obligations` and checks frozen-artifact
  reuse for an unchanged source/dependency identity pair.
- The language service currently selects only
  `mncs-research-bytecode` for this experimental query. No universal backend
  support is claimed; unsupported backends remain an honest future tranche.

## Boundary and next pressure

Rust continues to own document/workspace I/O, lifecycle, locks, protocol
adapters, and authoritative artifact acquisition. MNCS owns the deterministic
bounded status aggregation that runs after projection. The next evidence-led
conversion should pressure bounded symbol filtering and deterministic
relationship summaries; larger maps, unrestricted graph traversal, and host
capabilities remain out of scope until the language demonstrates the required
general primitives.

