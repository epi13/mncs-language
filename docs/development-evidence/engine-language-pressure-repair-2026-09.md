# Engine pressure repair — language tranche (2026-09)

Branch `feat/pressure-native-soundness`, reconciling
`mncs-engine/docs/PRESSURE.md` (ENG-PRESSURE-0001–0023) against current
`mncs-language/main` and implementing the substantial repair tranche below.
Write target is `mncs-language` only; `mncs-engine` was a read-only
reference. No engine-specific workarounds were added.

## Reconciliation against current main

| ID | Ledger status | Standing on current main |
| --- | --- | --- |
| 0001 u64 `>>` on WASM | open | NOT REPRODUCED — logical on all 5 backends; pinned, engine can close |
| 0002 no float type | open | STALE — binary64 exists (Profile 0.12) and executes; engine can close |
| 0003 leading-literal intent ops | open (confirmation) | CONFIRMED on all 5 backends; pinned, engine can close |
| 0004 `up_to 32` cap | open | ALREADY RAISED (1..=1024); `up_to 64` pinned, engine can close |
| 0005 i64 `>>` MNB017 | open | NOT REPRODUCED — elaborates, arithmetic on all 5; pinned, close |
| 0006 u64 high-bit args | open | NOT REPRODUCED — intact on all 5 backends; pinned, engine can close |
| 0007 enum payload restrictions | open | FIXED this tranche (profile+canonical payload types); engine retest |
| 0008 division obligations | open | SPLIT this tranche (divisor-nonzero / signed-overflow, literal discharge); engine retest |
| 0009 no integer `\|`/`&` | open | STALE for 32-bit (`(px >> 8) & 255` passes everywhere); pinned, close |
| 0010 `[u64; N]` domain MNB101 | open | NOT REPRODUCED — traverses with discharge; pinned, engine can close |
| 0011 nested sequences | open | FIXED this tranche (descending source spelling); engine retest |
| 0012 frustum clipping | closed (engine) | no language action |
| 0013 unbounded sequences | open | DEFERRED — fixed-pool model is a design position, not a defect |
| 0014 RNG | open | DEFERRED — stdlib tranche (seeded deterministic RNG + contract) |
| 0015 maps / spatial index | open | DEFERRED — stdlib tranche (uniform-grid neighbor query design) |
| 0016 cost units / scale | open | DEFERRED — cost-model design (per-backend `steps` semantics) |
| 0017 native symbol collision | open | FIXED this tranche (module-qualified symbols); engine retest |
| 0018 flat 64-lane cap | open | ALREADY RAISED (1024); 128-lane row pinned, engine can close |
| 0019 generic records | open | PARTIAL — deterministic generic inference landed; `Box<N: Nat>`-style generic record *declarations* still unimplemented |
| 0020 repeat literals | open | IMPLEMENTED this tranche (`[value; N]`); engine retest |
| 0021 rebinding MNE110 | open | IMPLEMENTED this tranche (shadowing); engine retest |
| 0022 strict `select` | open | PINNED + DOCUMENTED (strictness is semantic); engine can close |
| 0023 `fx_narrow_trunc` | open, engine-owned | no language action (engine numerics contract) |

## Implemented this tranche (new, on top of the prior fixes)

- ENG-0020 `[value; N]`: parser (`MNP203` on non-integer count, inherits
  the Profile 0.7 gate), AST `SequenceRepeat`, elaboration desugar to
  `SequenceConstruct` (element elaborates once under the declared element
  type; operand duplicated N times; count/bound mismatch is `MNE184`,
  non-length count is `MNE256`). All backends free via the existing
  construct lowering. Fixture `pressure-repeat.mncs` + corpus,
  `pressure_repeat_literals.rs` (3 cases x 5 backends).
- ENG-0021 shadowing: `BindingEnv::bind` allows a plain `let` over a
  `Binding`/`Parameter` in the same scope; index/state names stay reserved
  (`MNE110` retained). Monotonic per-scope slot allocator keeps every
  declaration's `binding_id` distinct; numbering of programs without
  shadowing is unchanged (fingerprint-stable). Fixture
  `pressure-rebind.mncs` + corpus, `pressure_rebinding.rs` (chains,
  parameter shadow, type-changing shadow, branch shadow, retained-MNE110
  negative pin).
- Traversal-discharge soundness fix (found while implementing 0021):
  `is_traversal_index` used `.any()` over the scope stack, so a nested
  `let` shadowing an index name inherited the traversal-domain discharge
  for uses that resolve to the plain value. Now the innermost binding
  decides (fail-closed to `RuntimeChecked`). No valid program breaks;
  unsound discharges become explicit checks.
- ENG-0022: strictness proved uniform (both-arm traps on all 5 backends),
  pinned by `pressure-select-strict.mncs` + corpus +
  `pressure_select_strict.rs`, and stated normatively in
  `docs/source-profile-0.8.md`. No lazy conditional was added: engine
  already standardized on total-arms guards + statement-level `if`, and a
  lazy expression form is a larger control-flow feature.
- Confirmation/regression pins (all 5 backends): `pressure-bitops.mncs`
  (0001/0005/0006/0009, 8 cases), `pressure-leading-intent.mncs` (0003, 7
  cases), `pressure-u64-domain.mncs` (0010, 1 case),
  `pressure-ceilings.mncs` (0004/0018, 2 cases).
- Docs: repeat in `source-profile-0.7.md` (+`MNE256`/`MNP203` rows),
  shadowing in `source-profile-0.9.md`, `select` eagerness consequence in
  `source-profile-0.8.md`.

## Deferred with rationale

- 0013: unbounded sequences contradict the bounded-execution model
  (step budgets, exact bounds as type identity). The correct next step is
  a design RFC (pool/allocator model), not an implementation tranche.
- 0014/0015: need stdlib design first (determinism contract for RNG;
  data-structure semantics for maps). Correct next tranche: stdlib RFC
  slices with `library/` implementations + backend-agnostic tests.
- 0016: needs a cost-model decision (what `steps` means per backend)
  before code; measurement work, not a correctness defect.
- 0019 remainder: generic record declarations (`Box<N: Nat>`) need a
  type-parameter representation across `BodyType`/identity/HIR — a design
  tranche on its own.

## Proof-kernel integration

New obligations: none. `[value; N]` reuses `SequenceConstruct` (no new
proof surface); shadowing reuses per-value SSA identities (no new
obligations; one discharge path narrowed fail-closed). Division
obligations from the prior tranche (`divisor-nonzero`,
`signed-division-overflow`) are unchanged. Authority stays intrinsic:
all facts discharge through the MNCS proof kernel; no external oracles.

## Backend matrix (observed)

Every new/changed test below ran green on all five executable backends
(`mncs-research-bytecode`, `mncs-portable-wasm-mvp`, `mncs-c11`,
`mncs-llvm-ir`, `mncs-cranelift`):
`pressure_repeat_literals`, `pressure_rebinding`,
`pressure_leading_intent`, `pressure_select_strict`,
`pressure_bitwise_shifts`, `pressure_u64_domain`, `pressure_ceilings`.
Full workspace suite: `cargo test --workspace --no-fail-fast` green
except one deliberate contract change — `mncs-compiler --lib`
`source_profile_0_2_..._duplicate_...` asserted the old ENG-0021
rejection (`let` over a parameter is `MNE110`); updated to assert the new
shadowing rule (valid, no `MNE110`; index/state reservation still pinned
by `pressure_rebinding::rebind_index_name_stays_reserved`), re-run green.
All other targets passed unmodified. PTX/CUDA paths: not run (no GPU), as
before.

## Consumer handoff (engine retest in a later run)

- Can close after retest: 0001, 0002, 0003, 0004, 0005, 0006, 0009, 0010,
  0018, 0022 (pins committed this tranche).
- Should retest for workaround removal: 0007 (payload types), 0008
  (division discharge), 0011 (nested sequences), 0017 (native symbols),
  0019 (generic inference; record declarations still pending), 0020
  (repeat literals), 0021 (shadowing; replace `v0`/`v1` chains).
- No action: 0012, 0023 (engine-owned).

## Repository integrity

Only `mncs-language` modified (`git status` at commit shows no other
repository touched; `mncs-engine` read-only throughout).
