# Next-pass handoff: language experiments across Commons, Fabric, Harness, and control

## Established in this pass

- Source Profile 0.2 rejects duplicate functions/bindings, unresolved names, unsupported types,
  type mismatches, non-boolean conditions, invalid returns, and statements after terminal control.
- One selected-SSA identity can be realized by portable WASM MVP or canonical research bytecode
  through the same backend adapter contract.
- Backend capabilities, realization requests, plans, typed artifacts, validators, and experiment
  definitions/results are content-addressed language artifacts.
- `mncs experiment execute` is a recompilation-free runtime boundary suitable for an opaque Fabric
  bundle; `plan`, `run`, `inspect`, and `compare` own the remaining language lifecycle.
- Forge can persist and compare the new result contract without issuing assurance or conformance.

## Exact next vertical slice

1. **Commons:** define a generic frozen-workload envelope carrying command/entrypoint, opaque artifact
   identities, corpus identity, required host/runtime capabilities, expected outputs, time/resource
   bounds, and provenance. Do not add MNCS-language legality to Commons.
2. **Fabric:** map that envelope to worker capability matching, immutable bundle materialization,
   execution receipts, cancellation/timeouts, and result transport. Preserve backend identity,
   artifact identity, corpus identity, worker/run environment, and raw experiment result. Prove local
   Linux first, then Windows and Raspberry Pi; do not call cross-host agreement equivalence.
3. **Harness:** package the target-specific `mncs` executable, frozen `backend-artifact.json`, corpus,
   and a command invoking `mncs experiment execute`. Pin hashes and reject mutation after freeze.
4. **Controller:** add submission/status/cancel/retry/reconcile operations keyed by the language
   experiment definition and realization-request identities. Retries must not silently change backend,
   corpus, validator policy, or target profile.
5. **Forge:** attach returned Fabric receipts and language validator records as separately identified
   evidence. Add routing/tournament policy only after backend/worker capability mismatch is explicit;
   performance must never override `FAIL` or required `UNKNOWN`.
6. **Model/tool routing:** expose adapter capability manifests and worker capabilities to planners,
   while keeping selection policy distinct from language correctness and verifier authority.

## Known blockers and non-claims

- Research bytecode is an architectural control, not native object code or a production VM.
- Portable WASM execution uses the embedded research interpreter, not an independently identified
  host runtime.
- No distributed Linux/Windows/Raspberry Pi observation or scheduling/reconciliation result is in
  this repository pass.
- The type/contract calculus, memory model, ABI/object/linker contracts, modules/generics, and general
  capability/effect semantics remain incomplete.
- Translation validation currently shares some model/execution code with generators; independent
  validation and freshness-bound Forge evidence remain follow-up work.
