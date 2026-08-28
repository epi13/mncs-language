# Module-linking maturity evidence — 2026-08-27

Status: **bounded development evidence**. This record describes the current
Profile 0.6 implementation slice; it is not a production-readiness claim.

## Scope

The compiler now accepts a host-provided `ModuleResolver`, elaborates imported
modules transitively, and links the resulting semantic closure before graph,
obligation, HIR, SSA, evidence, and reference execution stages. Resolution is
deterministic and fail-closed for missing modules, duplicate identities,
import collisions, and incompatible declared module names. A final `.vN`
version tail may be omitted by a discovery request, but this is an aliasing
convenience rather than package/version selection.

Imported declarations retain their declaring module in function-scoped
semantic identities. Canonical material records dependencies and imported
function home modules when present. The CLI `source-study` report exposes
machine-readable resolution provenance, including requested and declared names,
module identity, source identity, source logical name, and semantic fingerprint.

## Evidence

- `cargo test -p mncs-compiler --test module_imports` — 11 tests, including
  imported execution/lowering identity and fail-closed declared-module mismatch.
- `cargo test -p mncs-cli --test library_resolution` — 2 tests, including the
  external consumer executing linked `mncs.core.ordering.v1` through reference
  bytecode and portable WASM.
- `cargo test -p mncs-cli --test profile07_bounded_data
  serialization_groundwork_agrees_per_backend` — backend parity remains green
  after the resolver accepts the library's `.v1` discovery alias.
- Existing `mncs-tui` source fixtures validate against the linked library root;
  this confirms a real downstream consumer can use the resolver without
  duplicating library source.

## Deliberate boundary

The implementation does not yet provide qualified calls, selective or renamed
imports, component/provider contracts, semantic compatibility proofs,
package/version-range selection, or dynamic module bindings. Native SIMD and
backend-specific realization selection likewise remain separate maturity work.

## Reproducibility and graph analysis

The repository-wide baseline was captured with:

```text
/home/epi13/Documents/Projects/joern-agent-bridge/scripts/joern-snapshot baseline
/home/epi13/Documents/Projects/joern-agent-bridge/scripts/joern-check
```

The installed Joern distribution analyzed Python sources but does not support
the Rust frontend in this environment, so focused Rust call/reachability
queries returned `unsupported_language`. Post-edit snapshot, comparison, and
verification must retain that limitation explicitly rather than treating a
source-only query as graph evidence for the Rust implementation.
