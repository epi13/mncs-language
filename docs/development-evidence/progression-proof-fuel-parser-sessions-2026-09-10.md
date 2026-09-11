# Progression run: proof transport, uniform fuel, parser bounds, provider sessions (2026-09-10)

Branch: `feat/proof-transport-exhaustion-hardening` (from `feat/mncs-native-jit`
tip `890a653`, which trails `origin/main` by badge-only commits).
No new source profile: all work holds under the existing 0.13 contract.

## Implemented

- Tranche-0.3 proof transport (`P1`): `BackendArtifact` carries
  `proof_bindings: Vec<ProofBindingRef>` (schema 0.2 → 0.3), each ref
  versioned and bound to proof identity, obligation, kernel,
  slot-ordered dependency fingerprints, and the exact SSA fingerprint
  lowered. All six backend constructors populate from
  `SsaModule::proof_binding_refs()` (sorted, deduplicated; empty for
  proof-free SSA). Bindings join the artifact identity, so
  proof-bearing and proof-free compilations never share an identity;
  legacy JSON without the field still parses (fail-closed identity
  check covers the bindings). E2E: admitted proof → SSA relationship →
  backend artifact ref asserted through the real CLI.
- Uniform call-depth fuel (RFC 0047 §5, `R1`): a dedicated native
  status code 3 (`BudgetExhausted`) on Cranelift (own `exhausted`
  block), C11 (`*mncs_status = 3`), and LLVM (`mncs_exhausted` block),
  decoded by all three C drivers (`budget_exhausted` observation) and
  `native.rs`. Same-module callers propagate the callee code instead
  of overwriting it (Cranelift `propagate` block; LLVM
  `mncs_propagate`; C11 already early-returned; WASM/reference already
  propagated). Explicit per-request budgets are now honored on every
  backend via entry-depth seeding (`MAX - budget`, validated exactly
  like the reference: 0/over-cap → `InvalidRequest`); previously
  native backends silently granted full fuel against an explicit
  budget (before-evidence: budget=1 reported `returned`).
- Parser nesting bound (`R2`, `MNP206` at 256 shared levels over
  expression/binary/primary/statement/type-annotation entries).
  Pre-change, 5,000 nested parens aborted the parser with SIGABRT
  (reproduced on a clean HEAD worktree); post-change it reports
  `MNP206` with no AST. 20-deep nesting still parses (deepest
  real-world fixture nests 9).
- Retained native provider sessions (`J1`): `BackendExecutionSession`
  now prepares C11/LLVM/Cranelift sessions once per artifact (one JIT
  compile; one toolchain compile per entry/seed) and reuses them
  across cases, with explicit one-shot fallback where preparation
  fails. Measured on a trivial Cranelift module: 10× one-shot
  57.7 ms vs 10× retained 73 µs (~790x per-call; ~5.8 ms → ~7 µs).
  CLI `experiment run` on the 4-case fuel corpus: 1.98 s before
  (4 JIT compiles + silent budget over-grant) → 1.39 s after
  (1 prepare + 4 retained calls, correct exhaustion).
- Stateful Cranelift soundness: `CraneliftStatefulSession::execute`
  now clears and checks the JIT-OOB flag exactly like the one-shot
  path (previously neither — an out-of-image cell access would have
  decoded silently).
- Docs/conformance (`D1`): README Cranelift-JIT status corrected;
  profile-0.13 fuel-gap paragraph rewritten to the tested state;
  ledger entries 0003 (C4 unsatisfied → partial), 0007 (G4 narrowed),
  0047 (scope/gaps/evidence), 0048 (retained-session scope,
  whole-program-publication gap) updated; `docs/rfc-conformance.md`
  regenerated from the ledger (generator-owned, not hand-edited).

## Pressure ledger

Closed:
- NATIVE-FUEL-DIVERGENCE (reference `BudgetExhausted` vs native
  `RuntimeFailure`): closed by executable test on all five backends
  plus the three native cap cases.
- NATIVE-BUDGET-IGNORED (explicit `call_depth_budget` silently
  over-granted on wasm/native): closed the same way, incl. negative
  budgets → `InvalidRequest`.
- PARSER-HOST-STACK-OVERFLOW (unbounded parser recursion): closed by
  `MNP206` bound + tests + pre/post evidence.
- JIT-PRESSURE-0013 (cross-module enum construction unattempted):
  resolved at language level — qualified `alias.Type.Variant`
  construction exists and is pinned by `pressure_enum_payloads.rs`;
  `library/jit` keeping u64 codes at rest is a workload choice.
- Audit P2 (plain lowering paths "silently proof-free"):
  reclassified intentional — with no proof inputs there are no
  relationships to lose; the with-proofs path is explicit.

Reclassified (audit claim corrected by code evidence):
- `observable_agree` "masking": WRONG — it requires exact triple
  equality first; no change needed.
- Audit P3 (`assumption_bits` has no producer): intentional layering —
  bits are MNCS-owned policy produced in `library/jit/binding.mncs`,
  not a missing Rust mapping. Remaining boundary recorded below.
- JIT-PRESSURE-0004: narrowed — pure field chains (`a.b.c`) work on
  0.13; indexed-tail chains (`a.b.c[i]`) still refused (open).

Still open (forcing workload → owner):
- JIT-PRESSURE-0001 select-literal arms (MNE115 on 0.13):
  compiler/elaboration, low. Repro: `let p: u64 = select(c, 8, 0)`.
- JIT-PRESSURE-0002 large literal operand inference (MNE119 on 0.13):
  compiler/elaboration, low.
- JIT-PRESSURE-0003 record separator rule (comma-less multi-line
  still MNP127): parser/tooling cosmetic, low; workaround is
  permanent-quality code.
- JIT-PRESSURE-0004 indexed projection chains: language/parser,
  medium-verbosity. Awaiting a real workload beyond verbosity.
- JIT-PRESSURE-0005 reserved words (`next`/`over`): intentional
  design; owed a published reserved-word list (docs, low).
- JIT-PRESSURE-0006 strings for identity: intentional (boundedness);
  future `text_view`-at-boundary work, medium.
- JIT-PRESSURE-0007 ambient time: intentional boundary, low.
- JIT-PRESSURE-0008 maps/heap: intentional (bounded tables); chunked
  stdlib structures future work (§7 pattern, not a language feature
  yet), low-medium.
- JIT-PRESSURE-0009 whole-program publication per generation:
  provider, medium. Retained sessions amortize execution; incremental
  publication still future (narrow Rust layer when it lands).
- JIT-PRESSURE-0010 one-shot recompilation: largely closed for batch
  execution by retained sessions; one-shot remains the correct
  default for single calls.
- JIT-PRESSURE-0011 strict select: intentional semantics (pinned).
- JIT-PRESSURE-0012 public resolver: tooling, medium (extraction).

Newly recorded boundaries (not defects):
- PROOF-EXEC-REVALIDATION: artifacts transport refs; backend
  execution does not yet re-validate them against live SSA
  (0003-C4 partial, 0007-G4).
- ASSUMPTION-BIT-BINDING: MNCS `assumption_bits` ↔ kernel
  `DepAssumptionSet` linkage is era-convention only (0047 gap).
- REFERENCE-HOST-STACK: reference interpreters recurse on the host
  stack; 1025-deep activation overflows debug builds (release likely
  survives — unmeasured). Budgeted exhaustion covers them
  deterministically; the cap case is native-only by construction.
- SESSION-CACHE-KEYING: fuel seeds join native session cache keys
  (correct); per-(entry, budget) recompiles are expected, not a leak.

## Correctness / performance / self-hosting notes

- Every cache/session reuse is keyed by exact identities (artifact
  identity, entry+seed, CLIF match); tamper tests pin fallback.
- No profile change, no ceiling raised, no obligation suppressed, no
  MNCS-owned policy moved into Rust (sessions/generations/bindings
  stay in `library/jit/`; Rust owns only provider mechanics).
- Self-hosting: `mncs-compiler` pressure items S1/S2 (bounded-table
  rewrites, strict select) recorded above, not implemented — no
  speculative language features added.
- Slow tests (kept, recorded): `proof_dep_admission` e2e (~3 min,
  genuine MNCS admission executions), fuel corpora on C11/LLVM
  (external toolchain compiles, cache-amortized on repeat).

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --no-fail-fast`
  (full suite; results in the final report)
