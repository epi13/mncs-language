# Ingest language-pressure repair campaign (2026-09)

Unattended language-repair campaign driven by the pressures in
`mncs-ingest/docs/LANGUAGE_PRESSURE.md` (ledger reconciled against Source
Profile 0.14; `mncs-language` has since shipped 0.15/0.16 plus numerics,
store, and compiler pressure work). Method: reproduce or inspect every
pressure against current `main` first, classify, then fix at the owning
layer. `mncs-ingest` stayed read-only throughout (no commits there); all
proof below is `mncs-language`-owned.

Branch: `feat/ingest-pressure-2026-09-12` (from `origin/main` @ `c93c144`).

## Classification summary

| ID | Ingest claim (0.14-era) | Current-main reproduction | Classification | Resolution |
|----|------------------------|---------------------------|----------------|------------|
| P-001 | No bounded validated text value; spelling forced through code/span duplication | Views, view returns, and sub-slices exist; `text_view` is span-based and producer-attested; no nominal validated-text type (generic records do not exist) | **Still live; fixed as composition, no new type** | `examples/source/pressure-validated-spelling.mncs`: classifier returns the carved sub-view + MNCS-observed UTF-8 verdict in one `CheckedWord` record value; view IS the bounded text value |
| P-002 | No `view[a..b]` sub-slicing | Dynamic `view[a..b]` elaborates and executes correctly on all five backends, provenance-relative, fail-closed (verified before any change) | **Substantially already resolved; closed with regression + one backend bugfix** | Committed corpus proves dynamic/static sub-slice semantics + deterministic traps; fixed latent LLVM SSA-name collision it exposed |
| P-003 | No reusable UTF-8 substrate; `text_view` trusts producers | `json_cursor` validates for real internally; nothing reusable exported | **Still live; fixed in stdlib** | New `mncs.std.text_utf8.v1`: `validate_generic`, `step_scalar_generic`, `fold_scalar` (ASCII + Latin-1 C0–DE minus ×), importing the `json_cursor` predicates — no second decoder |
| P-004 | No deterministic finite map | `mncs.std.text_map.v1` exists with `lookup16/32` | **Already resolved; closed with wiring** | Orphaned `text-map-corpus.json` (14 cases, no consumer) is now executed on all backends; remaining friction is P-002/P-008, not a map feature |
| P-005 | Dead lanes clobber carried state; hand-written `select` keeps are droppable | Reproduced the hazard shape; `next` cannot be guarded by `if` (MNP101) | **Still live; fixed via uniform lazy `match`** | `next acc = match live { true => update, false => acc }`: dead lanes skip evaluation AND keep state; deleting the keep arm is MNE140 |
| P-006 | `select` strict, no lazy branch | Proved strictness both ways: trap in untaken bool-match arm fired; integer-match arm did not | **Still live; fixed with P-005** | Bool `match` now lowers through the finite/integer branch-chain shape (only the taken arm evaluates); `select` intentionally stays strict |
| P-007 | Import failures reduce leaf diagnostics to bare codes | Reproduced exactly: `MNE172 ... [MNP127, MNP128, MNP007]` for the `over`-field leaf | **Still live; fixed in compiler** | `SourceDiagnostic.related` carries leaf diagnostics verbatim; outer MNE172 keeps code/span/shape + source locator; nested edges chain |
| P-008 | Stdlib only resolvable via working-tree paths / vendoring | Confirmed: `MNCS_LIBRARY_PATH` + source-local roots only | **Still live; fixed as bundle distribution** | Content-addressed `library/stdlib-bundle.json` (58 modules, identity `mncs:stdlib-bundle:d2647f…`), `mncs bundle generate/verify`, `MNCS_STDLIB_BUNDLE`, in-crate `pinned_bundle()` |
| P-009 | Runtime length needs a second `length` argument | Single `[byte; up_to 1024]` view observes 0/256/1000 via `.len` identically on all five backends (verified before any change) | **Already resolved; consumer adoption debt** | Committed regression; `(storage, explicit_length)` is old consumer convention |
| P-010 | No retained sessions; validation pays open-per-call | `Session::open`/`call`, C ABI `mncs_session_call_batch`, ms-scale tests all in-tree; ingest `src/embed.rs` holds an `EmbedRuntime` on a retained session | **Already resolved + adopted; closed with evidence** | No language change; long validation runs are reference-interpreter cost, not a missing feature |
| P-011 | Scanners/readers fixed to 64-byte views | Capacity-generic views (`[byte; up_to N]`, iterate/index/`.len`) work on all backends (verified before any change); only the stdlib stayed width-monomorphic | **Stdlib pressure; fixed in stdlib** | `find_newline_generic` + cursor family, `read_u16_le_generic`/`read_u32_le_generic`; 64-byte entry points delegate unchanged; chunk/encoding raised to 0.13 |

Fixed in this campaign: P-001, P-003, P-005, P-006, P-007, P-008,
P-011, plus a P-002-exposed backend bug. Already resolved (evidence +
closure): P-002 (core), P-004, P-009, P-010. No new source profile was
required: every semantic change fit an already-admitted construct
(match lowering, diagnostic transport, resolvers, stdlib).

## What changed, by layer

**Compiler (`mncs-syntax`, `mncs-compiler`).**
- `SourceDiagnostic` gains `related: Vec<SourceDiagnostic>` (serde
  default + skip-if-empty: old artifacts still read, empty adds no
  bytes). `elaborate_import_closure` attaches full leaf diagnostics at
  both MNE172 sites, appends the resolved source locator to the outer
  message, and re-wraps transitive MNE172 failures so every import edge
  is recorded (cycle/unavailable/incompatible errors pass through
  untouched). Tests: parse failure, elaboration failure, nested two-hop
  chain, all in `module_imports.rs`.
- Bool `match` elaborates through the finite/integer branch-chain shape
  (one arm block each + value-carrying join) instead of `Select`. Values
  are unchanged; only untaken-arm evaluation is skipped. `select` is
  untouched. Documented in `docs/source-profile-0.13.md`.

**Codegen (`mncs-codegen`, LLVM backend).**
- View-construction/check temporaries used bare digit-suffix
  concatenation (`bad1`+6 and `bad`+16 both spelled `%bad16`; same for
  `vb`/`vb32`), redefining SSA names whenever two views shared a
  function with aligning tags. All check temporaries now use underscore
  separators; the P-002 corpus (which failed on LLVM before the fix) is
  the regression. C11 (inline expressions) and Cranelift (per-op stems)
  were audited and are immune.

**Standard library.**
- New `mncs.std.text_utf8.v1` (`library/std/text_utf8.mncs`, profile
  0.13): window validation reporting the first bad index (`-1` =
  valid), single-scalar stepping with progress guarantees
  (`next = at+1` on malformed input, `next = at` past the window), and
  the documented ASCII + Latin-1 fold. Lead/continuation predicates are
  imported from `mncs.std.json_cursor.v1`. Unsupported Unicode semantics
  (normalization, full case folding, graphemes) are stated as non-goals
  in the module header. 17-case corpus, all backends.
- `mncs.std.chunk.v1` and `mncs.std.encoding.v1` gain capacity-generic
  scans/readers; 64-byte entry points delegate with identical shapes;
  both modules declare 0.13 (generic specializations are checked against
  the defining module's ceiling — a 0.10 module cannot serve 256-byte
  stages, enforced by MNE182). Narrow-profile consumers are unaffected
  (0.7 `subtype-windows` and profile-0.7 suites pass unchanged).

**Distribution.**
- `crates/mncs-compiler/src/bundle.rs`: bundle format, verification,
  `BundleResolver`, the shared library collector, and the embedded
  `PINNED_STDLIB_BUNDLE_JSON`.
- `library/stdlib-bundle.json`: the checked-in pin (58 modules).
- CLI: `mncs bundle generate|verify`, `MNCS_STDLIB_BUNDLE` as an
  additional resolution authority under the existing collapse/conflict
  rules, usage text. See `docs/stdlib-bundle.md`.

**Regression corpora + examples** (each executed on all five executable
backends: research-bytecode, portable WASM MVP, C11, LLVM IR, Cranelift).
- `pressure-view-subslice` (12: dynamic/static sub-slices, narrow
  runtime windows, four deterministic `runtime_failure` escapes).
- `pressure-view-length-carries` (7: `.len` at 0/256/1000, head/tail,
  sub-view checksums, no length parameter).
- `pressure-validated-spelling` (7: keyword hits, mid-window carve,
  `[0xC3,0x28]` rejection, `Zoë` byte-identity, empty word).
- `pressure-wide-stage-scans` (7: shared generic scans/readers at
  64/256/392).
- `pressure-guarded-match` (8: untaken-trap skip, taken-trap escape,
  dead-lane skip, all-dead anti-clobber).
- `text-utf8-corpus` (17), previously orphaned `text-map-corpus` (14,
  now wired; wide cases re-budgeted 1024 → 8192 steps for the WASM step
  model — bytecode uses 950).

## Deliberately not done

- No new source profile: bool-match laziness, diagnostic transport, and
  bundle resolution all fit admitted constructs. Static discharge of
  `view-range-valid` for provably-valid ranges remains future work (the
  runtime check is retained everywhere; CMP301 records it).
- No heap strings, no generic records (the type system refuses them),
  no unrestricted loops, no package manager. The P-001 "text value" is
  the bounded view itself — a nominal text type would need generic
  records the language deliberately lacks.
- `text_scan` haystack widths were not generalized (out of the accepted
  slice; same generic pattern applies when needed).

## Downstream adoption (mncs-ingest, later run, read-only this campaign)

- Replace `(storage, explicit_length)` signatures with single bounded
  views (P-009 proven); replace `(text, start, length)` triples with
  sub-views (P-002 proven).
- Return spellings as values via the `CheckedWord` pattern; delete
  `object_label`-style span mirroring and the bifurcated
  code+spelling equality (P-001 proven).
- Adopt the `match`-guarded fold idiom for liveness (P-005 proven);
  replace defensive clamping around `select` with lazy `match` where the
  untaken side may trap (P-006 proven); `select` stays strict.
- Generalize `lines.mncs`/`frames.mncs` over the generic
  `find_newline_generic`/`read_u16_le_generic` and delete local
  duplicate folds (P-011 proven at 64/256/392).
- Resolve `mncs.std.*` from the pinned bundle instead of `vendor/`
  (P-008 proven; recipe in `docs/stdlib-bundle.md`).
- Consume `MNE172.related` in root diagnostics instead of recompiling
  leaves (P-007 proven).
- Do not re-run the 2,000-record benchmark or streaming fuzz suites as
  language gates; the targeted embed-parity case suffices for
  confirmation.

## Validation

- New tests: `module_imports` (+3 P-007), `stdlib_bundle` compiler (+9)
  and CLI (+5), `pressure_guarded_match` (+2), `collections`
  (+1 wide-stage), `library_core` (+2 utf8/text_map), `abi_boundary`
  (+3 subslice/length/spelling). Every semantic feature ran on all five
  executable backends.
- Full suites green: `mncs-compiler`, `mncs-model`, `mncs-syntax`,
  `mncs-codegen`, `mncs-embed`, plus CLI `library_core`,
  `abi_boundary`, `collections`, `profile_compat`, `profile07`,
  `profile06`, `tranche1`, `semantic_commands`,
  `pressure_scalar_match`, `host_generic_entrypoints`,
  `pressure_guarded_match`, `stdlib_bundle`.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D
  warnings` clean.
- Final `cargo test --workspace`: exit 0, 110 suites green, zero
  failures (includes the slow JIT/cranelift, proof-kernel, and
  store-reclamation suites; no exclusions).
- Pre-existing stash `stash@{0}` (WIP codegen/machine-intent changes
  stashed by a language-service task) was left untouched throughout.
