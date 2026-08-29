# Atlas JSON/WASM typed-model slice — 2026-08

This record captures the production-path Atlas consumer of the bounded
sequence/view and portable-WASM work in `mncs-language`. The companion Atlas
checkout owns the page and thin host adapter; this repository owns the
language, compiler, backend, and reusable standard-library substrate.

## Scope and authority

The slice remains explicitly experimental. It does not change conformance or
make the non-normative Atlas an authority. The canonical Atlas page and its
diagnostics page fetch `atlas.json` as bytes, validate the structural stream,
feed 64-byte host views to MNCS/WASM, reconstruct a bounded typed
`AtlasModel`, obtain a structured `RenderPlan`, and apply that plan through
safe browser DOM operations while retaining static HTML fallback.

The current Atlas schema is bounded in this artifact at thirty-two project
records, sixty-four relationship records, a twelve-level JSON container stack,
and a 65,536-byte model input. Text is represented as borrowed spans into the
original input, so the model does not materialize a JSON DOM or copy every
string into an unbounded heap.

## Reusable language work

The Atlas pressure generalized into these MNCS standard-library modules:

- `library/std/text_view.mncs` — `TextView { start, length, encoded,
  utf8_valid }`, fixed-window key matching, and explicit decodability flags.
- `library/std/json_cursor.mncs` — a streaming structural cursor with
  absolute byte position, container events, string start/end spans, bounded
  raw key bytes, JSON escape/unicode tracking, a twelve-level stack, strict
  UTF-8 lead/continuation validation, and UTF-16 surrogate-pair validation.
  Unknown keys longer than the 32-byte matcher window saturate instead of
  invalidating a document.
- `library/std/json_stream.mncs` — the existing scalar structural gate for
  complete JSON streams, with quoted-string/escape state, four-digit Unicode
  escape tracking, control-byte rejection, matched delimiters, a twelve-level
  bounded stack, and one-root/trailing-byte rejection. The Atlas host runs it
  before accepting the typed model result.
- `library/std/json_projection.mncs` — retained as the earlier raw scalar
  projection witness and comparison point. It exposes exact raw key/value
  projections over 64-byte views with 32-byte target windows; escaped text is
  deliberately not decoded here.
- `library/std/json.mncs` — a bounded complete-input JSON scanner covering
  objects, arrays, strings, numbers, literals, whitespace, separators,
  control-byte rejection, nesting, and JSON escapes. It reports a scalar
  summary/status rather than pretending to be a general heap-backed JSON DOM.

The model pressure also fixed two language/backend gaps:

1. local records can now appear inside bounded record fields such as
   `[Project; 15]`; the compiler seeds local record declarations into the
   provisional type namespace before resolving record fields;
2. portable WASM record lowering now distinguishes an eight-byte canonical
   slot from its low-32-bit cell-reference payload. Nested record/sequence
   fields no longer emit an invalid `i64.load`/`i32.local` or
   `i64.store`/`i32.local` pairing.
3. portable WASM checked i64 multiplication now uses guarded quotient checks,
   including the signed `MIN * -1` case, instead of rejecting the bounded
   cursor's generated index arithmetic as `CGN302`.

Composite modules use a bounded 512-page (32 MiB) arena in this experiment.
This is an implementation budget, not a claim of unbounded storage safety;
the allocator still traps when the fixed memory is exhausted and the Atlas
host rejects inputs above the model's declared 24 KiB bound.

## Typed model and render boundary

`mncs-atlas/mncs/atlas-model.mncs` owns schema-shaped meaning. Its
`AtlasModel` contains top-level text spans, a fixed `[Project; 15]`, project
and relationship counts, five maturity buckets, validity, and completion.
Its `RenderPlan` contains fixed render nodes. Node operations are:

| Operation | Meaning | Targets |
| --- | --- | --- |
| `1` | append card from text spans and maturity code | project grid or status grid |
| `2` | clear a render target | project grid or status grid |
| `3` | render summary metrics | summary surface |

The browser reads the canonical composite-cell ABI from the render-plan
pointer. It decodes `TextView` spans, validates external repository URLs to
HTTP(S), creates elements, and assigns `textContent`; it does not use
`JSON.parse`, `Response.json()`, or `innerHTML` for the experimental path.
Maturity labels/classes come from static legend data in the page, leaving the
MNCS model responsible for numeric classification and the host responsible
only for DOM presentation.

## Existing host ABI retained by the slice

- modules with memory now export it as `memory`, and the decoder accepts the
  function plus memory export forms while rejecting unsupported export kinds;
- the function-level fallback is emitted after the dispatcher loop, fixing a
  real standard-WASM validation error caused by unreachable placement;
- byte views use `i32.load8_u`, packed one-byte cells, and zero-byte alignment;
  other composite views retain the current eight-byte arena-cell layout;
- process-boundary marshaling now writes and reads byte views as packed host
  buffers while preserving the existing exact-sequence/composite-cell ABI.
- modules with composite memory now export
  `mncs_host_buffer(i32) -> i64` plus `mncs_host_buffer_reset()`; the low 32
  bits carry a linear-memory offset and the high 32 bits carry the reserved
  capacity. This is the first typed host-buffer contract and lets a host reuse
  one region while recycling target-array allocations without guessing an
  address.

The browser adapter passes an i64 descriptor with the low 32 bits as offset and
high 32 bits as length, reusing the region returned by `mncs_host_buffer`.
Projection selectors share one module instance, and the adapter refuses to
render projections unless the structural scan returns `1`.

For compiler-internal byte views derived from exact cell sequences, aligned
address bit 0 records the eight-byte source stride. Lowering masks that marker
before byte loads and the process-boundary reader removes it; host-reserved
addresses remain aligned and use the ordinary low-offset/high-length ABI.

## Observed executions

All results are bounded observations. `UNKNOWN` is retained when compiler or
independent-equivalence obligations remain unresolved.

| Experiment | Observation | Artifact |
| --- | --- | --- |
| Atlas scan / portable WASM + Node | 20,413 bytes in 319 chunks; result `1` | 6,658 bytes; SHA-256 recorded in the generated Atlas manifest |
| Atlas typed model / portable WASM + Node | 15 projects, 19 relationships, 33 render nodes, valid and complete | 153,696 bytes; 512 memory pages; SHA-256 recorded in the generated Atlas manifest |
| Atlas model corpus | one-project typed probe expectation met | status remains `UNKNOWN` |
| JSON cursor / portable WASM | 11/11 expectations met, including strict raw UTF-8 and surrogate-pair witnesses | status remains `UNKNOWN` because compiler obligations remain |
| `json-stream-probe` / portable WASM | 10/10 expectations met across root, Unicode, and split-chunk cases; status `UNKNOWN` while unresolved obligations remain | final source probe remains bounded/research-only |
| `json-probe` / portable WASM | 7/7 focused added number cases met; status `UNKNOWN` while unresolved obligations remain | final source probe remains bounded/research-only |
| Atlas scan artifact / native Node host | 20,413 bytes in 319 chunks; result `1`; host region `{offset:8, capacity:64}` | 6,658 bytes; SHA-256 recorded in the generated Atlas manifest |
| Atlas projection artifact / native Node host | `[16, 4, 5, 4, 1, 1, 19]`; one module instance; host region `{offset:8, capacity:64}` | 10,761 bytes; SHA-256 recorded in the generated Atlas manifest |

The current typed model's maturity distribution is `[4, 5, 4, 1, 1]` in
the model's fixed order: experimental, research, active infrastructure,
incubating, orientation.

## Joern graph evidence

The graph-sensitive workflow was run before and after source edits. Baseline
snapshots are stored under each checkout's ignored `.joern-agent/` directory.
The focused language query was run over `crates`, and the Atlas JavaScript
query was run over `site/assets` after the initial two-path parse attempt was
rejected by Joern's one-input CLI contract.

Representative commands:

```text
joern-parse --language rust -o /tmp/mncs-language-baseline-20260829.cpg crates
joern --script scripts/joern/source-vertical-slice.sc \
  --param cpgFile=/tmp/mncs-language-baseline-20260829.cpg --nocolors

joern-parse --language rust -o /tmp/mncs-language-post-20260829-v2.cpg crates
joern --script scripts/joern/source-vertical-slice.sc \
  --param cpgFile=/tmp/mncs-language-post-20260829-v2.cpg --nocolors

joern-parse --language javascript -o /tmp/mncs-atlas-post-20260829.cpg site/assets
```

The language focused slice retains the compiler/backend call boundary and
the expected control-flow counts for module encoding, decoding, lowering, and
allocation helpers. Joern reports the known Rust `break`/`continue` CFG
warnings. Its JavaScript frontend is a structural reachability observation,
not a browser semantic proof; it does not prove DOM safety or WebAssembly
runtime validity. The original Atlas two-file parse command failed because
`joern-parse` accepts one path per invocation; this limitation is recorded
rather than treated as a clean query result.

## Remaining cutover blockers

The production `/` path now uses the shared typed runtime as progressive
enhancement; navigation, journal enhancement, formatting, and the general
browser event/fetch protocol remain HTML/CSS/JavaScript. The host boundary is
documented in the Atlas checkout. The remaining blockers are malformed/
truncated full-Atlas differential execution across all backends, independent
equivalence for the full stateful model, and a formal cutover review. The
checked-in manifest therefore leaves artifact validation `UNKNOWN` even though
build-time WASM magic, SHA-256, corpus checks, and Node/browser QA pass.

## Reproduction

From sibling checkouts:

```bash
cd mncs-atlas
python scripts/build_mncs_wasm.py
python3 -m http.server 8000 --directory site
```

Open `/` or `/experimental-atlas.html`. The latter remains intentionally
`noindex` and exposes the same runtime with diagnostic framing.
