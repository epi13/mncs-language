# Filesystem/resource effects, watch hints, and the persistence substrate

Index PRESS-003/007/013/016/017/018. Status: filesystem listing,
chunked reads, and the durable mutation family (§8) are bounded
implementations on the research path; generation polling is an
experimental watch hint; the broader persistence substrate stays
contracts-only (Section 5).

## 1. Authority model

A program never spells a pathname. Filesystem authority travels only in
an explicit capability grant:

- source declares `capability fs_root` plus `effect fs_list
  authorized_by fs_root` (enumeration/identity) or `effect fs_read
  authorized_by fs_root` (content);
- the host grants `--grant-fs capability=root-path` (CLI) or
  `Grant::fs_root` (embed);
- the executor canonicalizes the root at realization and fails the
  call closed when the root is missing, unreadable, or not a
  directory;
- traversal outside the granted root is impossible by construction
  (the walk starts at the root; relative paths are built from real
  components, never `..`), and reads re-canonicalize plus
  prefix-check per call, so a symlink swap between listing and read
  resolves outside the root and is refused rather than followed;
- symlinks are listed as `other` and never followed; reads through
  non-files are refused with `InvalidRequest`.

There is no ambient filesystem access. This is the same fail-closed
posture as `host_read`/`host_write`, extended from one blob to a tree.

## 2. Intrinsics (source profile 0.12)

| intrinsic | effect | returns |
|---|---|---|
| `fs_list_count()` | `fs_list` | `u64` entry count |
| `fs_entry_name_at(i)` | `fs_list` | `[byte; up_to 64]` relative path |
| `fs_entry_kind_at(i)` | `fs_list` | `u64`: 0 file, 1 dir, 2 other |
| `fs_generation()` | `fs_list` | `u64` mutation hint |
| `fs_read_bytes_at(entry, offset, length)` | `fs_read` | `[byte; up_to 64]` chunk |

Indices are data, never authority: a stale or wild index fails closed
(`InvalidRequest` with a re-list hint), never with a value. Operands
must be `u64` (MNE262). Reads clamp to 64 bytes and to end-of-input;
short reads and empty views at end-of-input are the EOF signal.

## 3. Determinism contract

- The entry sequence is sorted by relative-path byte order, never by
  host readdir order. Creation/enumeration order cannot leak
  (proven by `fs_listing_is_independent_of_creation_order`).
- Every listing carries a content identity: SHA-256 over the canonical
  entry encoding, plus a generation counter covering names, kinds,
  sizes, and mtimes. Provenance records both (`entries:N
  gen:… snapshot:…`).
- Entries whose relative path exceeds 64 bytes are skipped from the
  listing but still feed the generation (mirroring the PRESS-014
  skip-vs-truncate policy: never truncate into a wrong identity).
- More than 1024 entries fails the listing closed (the declared bound,
  matching `MAX_SEQUENCE_BOUND`).
- Generation values are wall data: compare for change, never pin.
  Mutation during enumeration surfaces as IO failure (fail closed) or
  generation drift (re-list). There is no snapshot isolation in this
  slice; that is PRESS-017's job (Section 4).

## 4. Watch hints (PRESS-007, experimental)

`fs_generation()` is a mutation/event hint, not push delivery: poll
between bounded quiet windows; a changed value means re-list. The
missing watch half of PRESS-007 (true event delivery) remains future
work. Recorded as experimental, not as event semantics.

## 5. Persistence substrate (PRESS-013/016/017/018, RFC 0026)

`mncs.std.store.v1` lands the semantic foundation as pure contracts:
generations, atomic compare-and-transition (stale writers get a typed
observed-generation result), durability-level receipts (`VOLATILE` /
`PROCESS` / `POWER`, with power-loss survival explicitly UNKNOWN),
snapshot handles (acquire pins generation+value; release is total),
and retention policy as data (reclaim scans never report protected
generations). No durable mechanism exists yet: no files, no locks, no
fsync, no crash evidence. Process-death testing is not claimed, and
power-loss survival must never be inferred from these contracts. The
realization (POSIX files with flock/fsync as one mechanism, evidence
that it satisfies the contracts) is the defined next slice.

## 6. Backend boundaries

Filesystem intrinsics execute on the research bytecode path (body and
SSA executors share one realization in
`crates/mncs-model/src/fs_resource.rs`, proven to agree by
`fs_body_and_ssa_layers_agree`). Portable WASM, C11, LLVM IR, and
Cranelift refuse explicitly at lowering through the pre-existing
`HostCall` refusal arm — compilation failure with a clear message,
never a silent no-op. The pure `mncs.std.chunk.v1` cursor/span
vocabulary and `mncs.std.store.v1` contracts execute on all five
backends.

## 7. Diagnostics

- MNE257/MNE258: `fs_list` authority missing/doubled.
- MNE259/MNE260: `fs_read` authority missing/doubled.
- MNE261: filesystem intrinsic result-type mismatch.
- MNE262: filesystem index/offset/length operand must be `u64`.
- MNP204: filesystem intrinsics require source profile 0.12+.
- MNP205: filesystem intrinsic arity.
- MNE277/MNE278: `fs_write` authority missing/doubled.
- MNE279: filesystem mutation name/content operand must be a byte view.
- MNP211: filesystem mutation intrinsics require source profile 0.16+.

## 8. Durable mutations (Tranche A, profile 0.16)

Status: implemented on the research path (P1-001/P1-002/P1-003,
P2-005 atomic publication). The mutation family from §2's read-only
world:

| intrinsic | effect | returns |
|---|---|---|
| `fs_create_file(name, content)` | `fs_write` | `u64` new entry index (exclusive) |
| `fs_write_bytes_at(entry, offset, bytes)` | `fs_write` | `u64` written count (no sparse gaps) |
| `fs_append_bytes_at(entry, bytes)` | `fs_write` | `u64` appended count |
| `fs_mkdir(name)` | `fs_write` | `u64` new entry index (exclusive) |
| `fs_delete_at(entry)` | `fs_write` | `u64` removed kind (0 file, 1 empty dir) |
| `fs_rename_at(entry, new_name)` | `fs_write` | `u64` new entry index (same-dir, atomic replace of files) |
| `fs_sync_at(entry)` | `fs_write` | `u64` `1` barrier receipt (file + dir edges) |

Authority reuses the granted root (`--grant-fs`, `Grant::fs_root`):
no new grant shape. Names are bare single-component byte views —
separators, NUL, `.`/`..` refuse; there is still no path type
(Tranche E). Every call re-snapshots and reports post-state
generation; returned indices are observed, never predicted. Under the
`Record` policy mutations are intent-only (validated and valued,
never realized); compiled backends refuse `HostCall` explicitly at
lowering. Nothing fsyncs implicitly — durability is the explicit
`fs_sync_at` barrier, with the POSIX-only directory-edge gap recorded
in provenance. Normative profile text:
`docs/source-profile-0.16.md`.
