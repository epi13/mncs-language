# RFC 0007 Fabric-mediated proof check (2026-09)

Same frozen proof artifact, second mediation path: the curated proof-kernel
corpus (`examples/execution/proof-kernel-corpus.json`, 8 cases) was executed
through `mncs-fabric run local` instead of direct CLI invocation. The Fabric
job ran the MNCS-native kernel on the portable WASM backend and reduced the
per-case verdicts to a single PASS/FAIL with content-addressed receipts.

## Outcome

- Job `rfc0007-proof-check:wasm:0.1`, plan identity
  `sha256:337dab28953119878bfcabcf205669c4a5c339af3f61bf3d52ac6ef477494314`.
- Task manifest `sha256:4c5fd3f8e4f397f38983f60226e6718873ee058051a9f916523966dd9c468268`
  (one file: the shell task that invokes the pinned `mncs` binary with
  `MNCS_LIBRARY_PATH` and reduces `raw.json` to `result.json`).
- Two consecutive runs: outcome `PASS`, termination `COMPLETED`, exit `0`,
  stdout verdict `{"verdict": "PASS", "cases": 8}`.
- Execution records `sha256:842edd7a74727a8c08f5316665eab0233aeeb46ed5162acf1265f9f4b6392440`
  and `sha256:636be5b5d99d604bdc9c7ca58df2aa29c3d03cece199fb87e690f0ed9c5a9f14`;
  result artifact `sha256:a3e21b8219f0fd3fb9f837dcf16b3ce9353d37da705a06a09097cdab8cc0799e`.
- Node class: Linux x86_64 (Fedora), 8 CPUs, DECLARED_OFFLINE policy.

The full execution records are not checked in: they embed worker-local
network identifiers (MAC addresses, neighbor IPs) with no evidentiary value.
Everything needed to reproduce them is above: the corpus, the kernel source,
the pinned plan/task identities, and this command shape:

```sh
mncs-fabric run local --root <task-dir> --manifest manifest.json \
  --label rfc0007-proof-check job-plan.json
```

## Honest boundaries

- This is same-host mediation, not heterogeneous validation: both enrolled
  remote workers (`fabric-worker-01`, `worker-03`) report `UNAVAILABLE` /
  `ABSENT`, and no Windows or Pi worker was reachable. Ledger criterion
  `0007-C16` stays partial for exactly this reason.
- A Fabric PASS is an execution observation with receipts, never a proof.
  Checker agreement (MNCS kernel versus the independent reference checker)
  remains the consistency evidence; Fabric adds a second mediation path with
  its own content-addressed trail.
