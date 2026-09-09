# JIT language/compiler pressure registry (2026-09)

Workload: MNCS-native JIT/execution orchestration (`library/jit/`,
8 modules) compiled and executed on `mncs-research-bytecode` and
`mncs-cranelift`, driven end-to-end by
`crates/mncs-embed/tests/jit_execution.rs`.
Branch: `feat/mncs-native-jit`.
Language base: profile 0.10 surface.

Headline result: **zero production Rust changes were required.**
No file under `crates/*/src/` was modified for this subsystem; the
only Rust additions are two test files
(`crates/mncs-cli/tests/jit_orchestration.rs`,
`crates/mncs-embed/tests/jit_execution.rs`). Every pressure below
was either worked around cleanly in MNCS or recorded as future
provider/tooling work. Categories: language / compiler /
runtime / backend (Cranelift) / tooling.

## Performance evidence (wall clock, this machine)

| stage | observed |
|---|---|
| orchestration program compile (bytecode) | ~55 s |
| orchestration program compile (Cranelift) | ~59 s |
| orchestration session open, bytecode (compile+prepare) | ~74 s total |
| orchestration session open, Cranelift (stateful prepare) | ~7 s |
| tiny definition compile (Cranelift) | ~0.1 s |
| tiny definition provider open (Cranelift) | ~0.1 s |
| per-call marginal (either provider, retained session) | milliseconds |
| corpus Cranelift runs (compile-dominated): types 20 s, depends 75 s, session 131 s, binding 343 s, lifecycle 33 s, plan 31 s, profile 6 s, proof 6 s | |
| heaviest reference steps: binding supersede ~50k, session redefine ~46k | well inside budgets |

Obvious cost centers (no blind optimization; recorded as future
pressure): whole-program Cranelift lowering per generation
(JIT-PRESSURE-0009); one-shot `execute` recompiling per call vs the
stateful session amortizing it (JIT-PRESSURE-0010).

## Registry

### JIT-PRESSURE-0001 — `select` literal arms ignore let-annotation

- Category: compiler (elaboration). Severity: low.
- Workload: packing bit tables in `types.mncs` candidates, e.g.
  `let p: u64 = select(c, 8, 0) +% select(d, 4, 0);`
- Observed: MNE115 (initializer type mismatch). `select` with
  integer-literal arms pins i32; the `u64` annotation does not
  propagate. Operator contexts DO propagate (`x as i64 *% 100`
  unifies the literal).
- Minimal reproducer: `/tmp` probe (`single_select` returns fine,
  `chained_select` let-binding fails; `as u64` casts fix it).
- Expected: annotation-driven unification, or a diagnostic naming
  the pinned type.
- Workaround: `select(...) as u64` on every literal-armed select in
  a u64 context (applied across `library/jit/`).
- Correctness: none (workaround is exact). Performance: none.
- Rust files changed: none.
- Survives self-hosting: the workaround is valid MNCS regardless;
  the elaboration gap should be fixed in both compilers together.
- MNCS-native solution: N/A (compiler concern).

### JIT-PRESSURE-0002 — oversized integer literals default to i64

- Category: compiler (elaboration). Severity: low.
- Workload: `saturating_add` in `profile.mncs` (`u64::MAX` bound).
- Observed: MNE119 (`18446744073709551615 -% b`: left i64 vs
  right u64). Bare literals DO coerce to annotated let-types
  (`let max: u64 = 18446744073709551615;` works), but an
  out-of-i32-range literal in operator position defaults to i64.
- Expected: literal types unify with the known operand type.
- Workaround: bind `max` first, then compute.
- Correctness/performance: none. Rust files changed: none.
- Survives self-hosting: workaround is permanent-quality code.
- MNCS-native solution: N/A.

### JIT-PRESSURE-0003 — multi-line record literals need commas

- Category: compiler (parser). Severity: low.
- Workload: every multi-line record in `library/jit/`.
- Observed: MNP007 without trailing commas; single-line
  space-separated records (e.g. `relation.mncs` `Edge`) parse.
  Newline handling is inconsistent between the two shapes.
- Expected: one separator rule for both shapes (or a diagnostic
  suggesting the comma).
- Workaround: trailing commas everywhere (applied).
- Correctness/performance: none. Rust files changed: none.
- Survives self-hosting: cosmetic; align both compilers.

### JIT-PRESSURE-0004 — no chained `a.b.c[i]` projections

- Category: language/parser. Severity: medium (verbosity only).
- Workload: shifting the dependents window in `depends.mncs`
  (`fold.st.affected[1]` refused with MNP016).
- Observed: only single-level `name.field[i]` / `name[i].field`
  chains parse.
- Expected: full postfix projection chains (or documented limit).
- Workaround: intermediate lets (one level per binding); full
  `Session` reconstructions where many fields change at once.
- Correctness: none. Performance: copies are backend-optimized;
  no measured effect.
- Rust files changed: none.
- Survives self-hosting: surface decision for both compilers;
  deeper session nesting later will want chains.
- MNCS-native solution: N/A.

### JIT-PRESSURE-0005 — `next` and `over` are reserved words

- Category: language (lexical). Severity: low.
- Workload: natural locals (`let next: Executable`, `let over: u64`
  in `binding.mncs` / `profile.mncs`) refused (MNP050 et al.).
- Expected: a published reserved-word list; these two are
  unsurprising (`next` is the iteration keyword, `over` the
  traversal keyword) but easy to trip on.
- Workaround: renamed (`stepped`, `past`).
- Correctness/performance: none. Rust files changed: none.

### JIT-PRESSURE-0006 — no string type for definition identity

- Category: language. Severity: medium (API ergonomics).
- Workload: naming logical definitions (`foo`, `bar`).
- Observed: MNCS has no string values; definitions are
  host-assigned u64 codes (the `token_set` pattern).
- Expected: bounded text identity eventually (`text_view` covers
  host inputs today, not identities).
- Workaround: allocator-owned u64 codes starting at 1; documented
  boundary. The e2e driver maps spellings to codes.
- Correctness: codes are exact, never reinterpreted. Performance:
  comparisons are O(1).
- Rust files changed: none.
- Candidate future MNCS-native solution: bounded [`byte; N`]
  spelling windows compared with `text_scan`-style matchers at the
  define boundary; codes stay the runtime identity.

### JIT-PRESSURE-0007 — no ambient time; wall clock threaded in

- Category: language/runtime boundary. Severity: low (by design).
- Workload: compile/execution timings in `profile.mncs`.
- Observed: pure MNCS cannot read a clock (the `clock` pattern);
  observations enter as host-supplied u64 millis.
- Workaround: `observe_call` / `observe_compile` take millis as
  parameters; counters saturate (never wrap).
- Correctness: saturating accounting cannot misreport hot as cold.
- Rust files changed: none (driver uses `Instant`).

### JIT-PRESSURE-0008 — no maps/heap; bounded tables

- Category: language. Severity: low (by design).
- Workload: definition/binding/artifact/edge registries.
- Observed: storage-plus-count windows with explicit-full
  rejections (the `relation` pattern).
- Workaround: capacities 8/8/16/16 with reason-coded refusals;
  documented as the extraction unit's contract.
- Performance: scans are tiny; heaviest flow ~50k steps.
- Candidate future: chunked/overflow generations if sessions
  outgrow 8 live definitions (policy work, not MMU work).

### JIT-PRESSURE-0009 — whole-program artifacts per generation

- Category: backend (Cranelift provider) + future compiler.
- Severity: medium (scaling, not correctness).
- Workload: redefinition recompiles the whole definition program
  (~0.1 s at probe scale; orchestration-scale programs ~60 s).
- Observed: `JitSession::new` compiles the entire scalar module;
  there is no incremental "add one definition" publication.
- Expected: one JIT module per session, definitions published
  incrementally; old generations retired by explicit transition.
- Workaround (current): generational whole-program artifacts kept
  alive in retained provider sessions keyed by MNCS artifact code
  (exactly what the e2e driver does).
- Correctness: none (old code provably still invocable; tested).
  Performance: recompile cost per generation is whole-program.
- Rust files changed: none.
- Candidate solution: provider-side incremental publication
  behind the unchanged MNCS generation/binding interface.

### JIT-PRESSURE-0010 — one-shot execute recompiles per call

- Category: backend (Cranelift adapter). Severity: low-medium.
- Workload: corpus runs vs retained sessions.
- Observed: one-shot `execute` builds a fresh JIT module per call;
  the stateful session prepares once (~7 s Cranelift open) and
  serves millisecond calls after.
- Expected: compiler-service use must go through retained
  provider sessions, never one-shot per invocation.
- Workaround: the e2e driver retains one provider session per
  artifact generation (documented provider-cache discipline).
- Rust files changed: none.

### JIT-PRESSURE-0011 — strict `select` needs lazy guards

- Category: language semantics (known, applied). Severity: low.
- Workload: `mean_exec_ms` division; index selection.
- Observed: `select` evaluates both arms (division by zero traps
  even when untaken); indexes must select INSIDE the brackets.
- Workaround: `if/else` for trapping computations;
  `arr[select(cond, idx, 0)]` pattern (as in `relation.mncs`).
- Rust files changed: none.

### JIT-PRESSURE-0012 — no public library module resolver

- Category: tooling. Severity: medium (extraction friction).
- Workload: the e2e test compiles a multi-module MNCS program
  in-process.
- Observed: the CLI `FileModuleResolver` is private; the test
  reimplements ~20 lines mapping `mncs.jit.*` to `library/jit/`.
- Expected: a public file/library resolver for embedders (the
  future `mncs-jit` crate needs exactly this).
- Workaround: test-side resolver (no production impact).
- Rust files changed: none (test-only code).
- Survives self-hosting: yes — resolver shape is host-side API.

### JIT-PRESSURE-0013 — cross-module enum values untested at boundaries

- Category: language/tooling. Severity: low.
- Workload: Provider/Lifecycle/etc. across module boundaries.
- Observed: same-module enum match/construction verified
  (`types.mncs`); cross-module record construction verified;
  cross-module enum VALUE construction not attempted — records
  carry u64 codes at rest instead (host-assigned-codes pattern).
- Workaround: checked vocabulary + converters in `types.mncs`;
  codes at rest. No expressiveness lost.
- Candidate future: qualify whether `types.Provider.Cranelift`
  spells across modules for the `mncs-jit` extraction facelift.
