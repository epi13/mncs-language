# Atlas JSON/WASM vertical slice — 2026-08

This record captures the first real browser-facing consumer of the bounded
sequence/view and portable-WASM work in `mncs-language`. The companion Atlas
checkout owns the page and application adapter; this repository owns the
language, compiler, backend, and reusable standard-library substrate.

## Scope

The slice does not replace the Atlas production site. It adds an explicitly
experimental page that fetches `atlas.json` as bytes, writes 64-byte views into
WASM memory, and asks MNCS-produced modules for scalar observations:

- a structural JSON stream result (`1` for complete, `-1` for incomplete,
  `-101` for malformed);
- raw-key/member projections for the five maturity values and relationship
  `from` fields;
- a raw maturity-field count.

HTML/CSS remain the presentation shell. JavaScript owns fetch, linear-memory
writes, chunk scheduling, and DOM rendering. It does not call `JSON.parse` or
`Response.json()`. The production Atlas and its existing `site/assets/app.js`
remain unchanged as the default path, with the experimental page retaining a
visible static fallback if either module or the data fails.

## Reusable language work

The Atlas pressure generalized into three MNCS standard-library modules:

- `library/std/json.mncs` — a bounded complete-input JSON scanner covering
  objects, arrays, strings, numbers, literals, whitespace, separators,
  control-byte rejection, nesting, and JSON escapes. It reports a scalar
  summary/status rather than pretending to be a general heap-backed JSON DOM.
- `library/std/json_stream.mncs` — a scalar stream envelope for larger input,
  with quoted-string/escape state, four-digit Unicode escape tracking,
  control-byte rejection, matched object and array delimiters, a twelve-level
  bounded container stack, and one-root/trailing-byte rejection.
- `library/std/json_projection.mncs` — exact raw key/value projections over
  64-byte views with 32-byte target windows. Escaped text is deliberately not
  decoded here; lexical validation and text decoding are separate concerns.

The modules are linked by the normal Profile 0.6 import resolver and consumed
by Atlas adapters; no parser implementation was copied into the Atlas repo.
The committed corpora cover valid scalars, nested empty values, malformed
closes, truncation/incomplete state, escapes, and a schema-shaped projection.

## Backend and ABI pressure

The portable WASM backend gained the minimum host-facing behavior required by
the experiment:

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

All results below are bounded observations. `UNKNOWN` is retained when the
experiment has unresolved compiler obligations; it is not upgraded because
the returned corpus values look right.

| Experiment | Corpus observation | Artifact |
| --- | --- | --- |
| `json-stream-probe` / portable WASM | 10/10 expectations met across root, Unicode, and split-chunk cases; status `UNKNOWN` while unresolved obligations remain | final source probe remains bounded/research-only |
| `json-probe` / portable WASM | 7/7 focused added number cases met; status `UNKNOWN` while unresolved obligations remain | final source probe remains bounded/research-only |
| Atlas scan artifact / native Node host | 20,413 bytes in 319 chunks; result `1`; host region `{offset:8, capacity:64}` | 6,657 bytes; SHA-256 `fcca761e6ef4d4b268232b55ebff007a6a521f919c05ede5c86d4c874cf67998` |
| Atlas projection artifact / native Node host | `[16, 4, 5, 4, 1, 1, 19]`; one module instance; host region `{offset:8, capacity:64}` | 10,760 bytes; SHA-256 `ebe9a18522cef05c1cf76a9c173465762f83762ab4800d9f8df40710fb38f0c6` |

The first projection value is a raw count of `maturity` keys, including the
one documentation link in the machine map's top-level metadata. The page calls
this “Maturity fields” rather than claiming it is a semantic project count.
The remaining values correspond to the five project maturity counts and the
19 relationship `from` fields in the current Atlas map.

## Joern graph evidence

The required focused query was run before and after the edits:

```text
joern-parse --language rust -o /tmp/mncs-language-wasm-before-20260828.cpg crates
joern --script /tmp/mncs-wasm-vertical-slice.sc \
  --param cpgFile=/tmp/mncs-language-wasm-before-20260828.cpg --nocolors

joern-parse --language rust -o /tmp/mncs-language-wasm-final3-20260828.cpg crates
joern --script /tmp/mncs-wasm-vertical-slice.sc \
  --param cpgFile=/tmp/mncs-language-wasm-final3-20260828.cpg --nocolors
```

Both snapshots retain one `encode_module`, `decode_module`, and
`emit_alloc_helpers` method, with `lower_selected_ssa` as caller. Encoder
control counts remain `(IF,2),(WHILE,1)`; decoder counts remain
`(IF,3),(WHILE,1)`; allocator-helper counts remain `(IF,2),(WHILE,2)`.
The query also retains the byte-oriented memory boundary calls. Joern reports
the same CFG fallback warnings for Rust `break`/`continue`. Its current Rust
frontend does not expose the `decode_exports` `match` as a control structure in
this query, so that result is an analysis limitation, not evidence that the
branch is absent.

## Remaining blockers

This is not yet a production web runtime. The full Atlas card/status renderer,
navigation state, journal enhancement, formatting, DOM command model, UTF-8
decoding, and browser event/fetch host protocol remain HTML/CSS/JavaScript.
Python remains the repository integrity and Journal Maintainer implementation.
The current portable-WASM contract is a research execution envelope, and its
unresolved obligations keep these experiments `UNKNOWN`. Before a default-site
switch, MNCS still needs richer structured return or render-command values,
independent backend/runtime validation, and a larger corpus including
malformed/truncated Atlas data and text/UTF-8 cases.

## Reproduction

From the sibling checkouts, build the checked-in Atlas artifacts with:

```bash
cd mncs-atlas
python scripts/build_mncs_wasm.py
python3 -m http.server 8000 --directory site
```

Open `/experimental-atlas.html`. The page is intentionally `noindex`; the
canonical `/` path continues to use the static/progressive-enhancement site.
