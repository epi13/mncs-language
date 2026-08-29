# Atlas JSON/WASM typed-model slice — 2026-08

This record captures the second browser-facing consumer of the bounded
sequence/view and portable-WASM work in `mncs-language`. The companion Atlas
checkout owns the page and thin host adapter; this repository owns the
language, compiler, backend, and reusable standard-library substrate.

## Scope and authority

The slice remains explicitly experimental. It does not replace the Atlas
production site, change conformance, or make the non-normative Atlas an
authority. The experimental page fetches `atlas.json` as bytes, validates the
structural stream, feeds 64-byte host views to MNCS/WASM, reconstructs a
bounded typed `AtlasModel`, obtains a structured `RenderPlan`, and applies
that plan through safe browser DOM operations.

The current Atlas schema is bounded in this artifact at fifteen project
records, nineteen relationship records, a twelve-level JSON container stack,
and a 24 KiB model input. Text is represented as borrowed spans into the
original input, so the model does not materialize a JSON DOM or copy every
string into an unbounded heap.

## Reusable language work

The Atlas pressure generalized into these MNCS standard-library modules:

- `library/std/text_view.mncs` — `TextView { start, length, encoded,
  utf8_valid }`, fixed-window key matching, and explicit decodability flags.
- `library/std/json_cursor.mncs` — a streaming structural cursor with
  absolute byte position, container events, string start/end spans, bounded
  raw key bytes, JSON escape/unicode tracking, a twelve-level stack, and basic
  UTF-8 lead/continuation validation. Unknown keys longer than the 16-byte
  matcher window saturate instead of invalidating a document.
- `library/std/json_stream.mncs` — the existing scalar structural gate for
  complete JSON streams; the Atlas host runs this before accepting the typed
  model result.
- `library/std/json_projection.mncs` — retained as the earlier raw scalar
  projection witness and comparison point; it is no longer the experimental
  page's application path.

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

## Observed executions

All results are bounded observations. `UNKNOWN` is retained when compiler or
independent-equivalence obligations remain unresolved.

| Experiment | Observation | Artifact |
| --- | --- | --- |
| Atlas scan / portable WASM + Node | 20,413 bytes in 319 chunks; result `1` | 6,658 bytes; SHA-256 recorded in the generated Atlas manifest |
| Atlas typed model / portable WASM + Node | 15 projects, 19 relationships, 33 render nodes, valid and complete | 53,066 bytes; 512 memory pages; SHA-256 recorded in the generated Atlas manifest |
| Atlas model corpus | one-project typed probe expectation met | status remains `UNKNOWN` |
| JSON cursor / five executable backends | all five realizations agree on complete, incomplete-root, long-key, and malformed-UTF-8 witnesses | status remains `UNKNOWN` because compiler obligations remain |

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

This is not a production web runtime. The production `/` path remains the
static/progressive-enhancement Atlas and its existing `app.js`. The
experimental path still needs richer generic text/DOM/event host contracts,
strict Unicode scalar validation, independent runtime equivalence beyond the
Node smoke path, malformed/truncated full-Atlas differential corpora, and a
formal cutover review. The checked-in manifest therefore leaves artifact
validation `UNKNOWN` even though build-time WASM magic, SHA-256, and corpus
checks pass.

## Reproduction

From sibling checkouts:

```bash
cd mncs-atlas
python scripts/build_mncs_wasm.py
python3 -m http.server 8000 --directory site
```

Open `/experimental-atlas.html`. The page is intentionally `noindex`; the
canonical `/` path continues to use the static guide.
