# RAVEL source-subject provenance repair (2026-09-25)

## Finding and ownership

Compiler-scale planning exposed a missing generic CLI artifact fact. A source
`mncs impact` response carried semantic graph impact, while the source program
identity and production fingerprint were only present inside the optional
source test inventory. RAVEL therefore could not bind an external
repository-obligation plan without requesting that inventory. Omitting it
reproduced an UNKNOWN planner result (`typed sequence length 0 does not match
exact 32` after 4.32 seconds), even though the repository's own obligation
inventory already described the external verification subjects.

This is a compiler command-artifact contract issue owned by `mncs-language`,
not a runtime or language pressure. A search of current MNCS-Commons
`origin/main` at `fa6d38922c18dc7ffe32431af876e856dafe40dd` found no existing
family pressure for this missing source-subject projection. The existing
minimum-sufficient-plan record concerns validator/schema duplication and is a
different capability.

## Repair

`mncs impact <source.mncs>` now includes an optional compiler-issued
`source_subject` projection containing source artifact identity, module,
source profile, semantic program identity, and production fingerprint. It is
built from the same front-end result as semantic impact. The optional
`--include-test-inventory` response remains available for consumers that need
local compiler test cases.

RAVEL consumes `source_subject` for source-bound plans and supplies no source
test cases when planning repository-owned obligations. It does not request
`mncs test-inventory` or perform another source front-end invocation on this
path. Compiler-owned identities and fingerprints remain authoritative; RAVEL
only adapts their transport shape.

## Evidence

- `cargo test --locked -p mncs-cli --test semantic_commands`: 40 passed,
  including the new no-test-inventory source-impact case.
- Focused current RAVEL `tests.test_impact`: 12 passed with
  `PYTHONPATH` pinned to the dedicated RAVEL `origin/main` worktree.
- Direct `mncs impact` without `--include-test-inventory` returned a
  `source_subject` and no `source_test_inventory`.
- The real `flow.lower_unit` RAVEL request completed in about 15 seconds
  without requesting source test inventory. It selected the flow CFG
  obligation, found it current from identity-bound PASS evidence, and required
  no new execution. Its plan remains non-stopping at the direct-dependents
  boundary, as expected for an isolated changed function.

The initial UNKNOWN is retained as historical failure evidence; it is not
reclassified as zero work or PASS. The source-subject interface repair removes
the cause, while future provider failures still fail closed as UNKNOWN.
