# Process-effect ABI

Owner: `mncs-language` runtime (`crates/mncs-model/src/process.rs`), with
declaration twins in `mncs-stdlib` (`library/std/process.mncs`,
`library/std/application.mncs`).

## Launch vectors

Process launch vectors admit **256** entries for both `argv` and
`environment`:

```text
argv:        [[byte; up_to 1024]; up_to 256]
environment: [EnvironmentEntry; up_to 256]
```

The authoritative bound is `PROCESS_LAUNCH_VECTOR_CAPACITY` in
`mncs-model/src/process.rs`. `MAX_ARGUMENTS` and `MAX_ENVIRONMENT` are
aliases of it; validation, nominal shape tables, host policy, and the CLI
launcher all consume the constant. There is no layout or serialization
basis for a smaller bound: the previous capacity of 16 was an arbitrary
historical limit that real development-provider invocations (24+
arguments) exceeded.

The stdlib declarations are twins of the runtime constant, guarded by a
contract test. Changing the capacity means changing the constant, both
twins, the bundle pin, and the boundary tests together.

Each element is capped at 1024 bytes, so the worst-case request stays
small (256 x 1 KB per vector).

## Capture budgets vs the observation window

Execution capture budgets and the typed observation window are separate
concerns:

- **Capture budget**: how much child output the runtime retains per
  stream. Ceiling `MAX_CAPTURE_BYTES` (4 MB), enforced at request
  validation. Forge requests its real configured budget
  (`PROCESS_CAPTURE_BUDGET_MAX`, same 4 MB ceiling, documented mirror).
- **Observation window**: the 1 KB typed source-record bound applied at
  materialization. Output beyond the window truncates and reports
  `truncated=true`; it never kills the run.

Consumers of observation output must check the truncation flag; the
window is not the complete stream.

## Invalidation

A capacity change alters the process-effect ABI. Dependent state
invalidates through content identity: Forge admission keys on the
compiler/runtime source identity, so a new ABI generation causes exactly
one rebuild and re-admission, after which warm reuse resumes. The
environment session `toolchain.revision` label tracks the checkout HEAD
and is metadata only; Forge tracks the toolchain by content.
