# Source Profile 0.16 — durable filesystem mutation

Status: **implemented, experimental (current)**. Profile 0.16 is
additive over Profiles 0.1–0.15 and is the explicit evolution home
(RFC 0036) for the durable filesystem-mutation family (Tranche A:
P1-001/P1-002/P1-003, P2-005 atomic publication). Older profiles
retain their historical syntax, semantics, and canonical fingerprints;
the `crates/mncs-cli/tests/profile_compat.rs` suite pins both directions
(old-profile refusals and 0.16 admissions).

Predecessor: Profile 0.15 (`docs/source-profile-0.15.md`).
Machine-readable policy: `mncs-syntax` registry
(`crates/mncs-syntax/src/profile.rs`,
`spec/source-profile-registry.json`).

## The `fs_write` effect family

Profile 0.12 gave the granted filesystem its observation half
(`fs_list`, `fs_read`); 0.16 lands the mutation half behind the same
`--grant-fs capability=root-path` authority (no new grant shape, no new
CLI flag) under one new declared effect, `effect fs_write
authorized_by <capability>`. Exactly one `fs_write` declaration per
function, mirroring the `fs_list`/`fs_read` authority rule (missing:
MNE277; doubled: MNE278).

| intrinsic | operands | returns |
|---|---|---|
| `fs_create_file(name, content)` | two views | new entry index (`u64`) |
| `fs_write_bytes_at(entry, offset, bytes)` | `u64, u64`, view | written count (`u64`) |
| `fs_append_bytes_at(entry, bytes)` | `u64`, view | appended count (`u64`) |
| `fs_mkdir(name)` | view | new entry index (`u64`) |
| `fs_delete_at(entry)` | `u64` | removed kind (`u64`: 0 file, 1 empty dir) |
| `fs_rename_at(entry, new_name)` | `u64`, view | new entry index (`u64`) |
| `fs_sync_at(entry)` | `u64` | `1` barrier receipt (`u64`) |

Below 0.16 the parser refuses every spelling with MNP211
(`filesystem mutation intrinsics require source profile 0.16 or
later`); wrong arity is MNP205, the shared filesystem-arity code.
Operand type errors reuse the shared filesystem codes (MNE261 result
mismatch, MNE262 non-`u64` scalar, MNE279 non-view name/content).

## Authority and boundedness rules

- A program never spells a pathname. Names are bare single-component
  byte views (`[byte; up_to 64]`); separators, NUL, `.`, `..`, and
  empty/overlong names refuse with `InvalidRequest` at realization.
  (There is still no path type — that is Tranche E work; the byte view
  is policed, never normalized.)
- Creates are exclusive at the syscall (`create_new`/`create_dir`):
  an existing entry of any kind refuses, never overwrites.
- Positioned writes never create sparse gaps (`offset <= len`, else
  `InvalidRequest`); a call grows a file by at most one 64-byte view.
- Rename stays in the source's parent directory (never a
  cross-directory move) and atomically replaces a non-directory
  destination where the platform provides atomic replace; replacing a
  directory is always refused. Stage-then-rename is the
  atomic-publication primitive (P2-005).
- Delete removes files and empty directories only; `other` entries
  (symlinks, sockets, devices) are never touched, and non-empty
  directories refuse.
- Nothing fsyncs implicitly. Durability is the separate explicit
  barrier `fs_sync_at` (P1-002): file bytes plus containing-directory
  and root edges. Directory-edge sync is POSIX-only; elsewhere the file
  barrier still holds and the gap is recorded in effect provenance,
  never hidden.
- Every mutation re-snapshots and reports post-state: the returned
  index is observed in the new listing (rename/create/mkdir), and
  provenance carries the new generation. Indices stay
  generation-scoped: re-list when `fs_generation` moves.

## Verification posture

Concurrent-mutation boundary (TOCTOU): every mutation resolves its
index against a fresh snapshot, then re-canonicalizes plus
prefix-checks before touching the tree — the same posture as the 0.12
read half. A hostile co-writer inside the granted root that wins the
race between check and syscall can redirect an open-for-write onto a
swapped-in symlink; create/mkdir/rename/delete are structurally safe
(`create_new` fails on symlinks, rename replaces the link itself,
`other` entries are never touched), but positioned writes and appends
follow the final component. Mutation during a call otherwise surfaces
as IO failure (fail closed) or generation drift (re-list). The granted
root is operator-owned; mutually untrusting writers must not share
one.

Under the `Record` effect policy, mutations are intent-only:
validated and valued, never realized — so replay stays side-effect
free and the logical mutation realizes exactly once. Compiled backends
(C11, WASM, LLVM, Cranelift) refuse `HostCall` explicitly at lowering
through the pre-existing refusal arm; only the research bytecode path
executes mutations, with an explicit `--grant-fs` grant.

Evidence: `examples/source/pressure-fs-mutation.mncs`,
`examples/execution/pressure-fs-mutation-corpus.json`,
`crates/mncs-cli/tests/pressure_fs_mutation.rs`.

## Explicit non-claims

- No path type, no leased views, no cross-directory moves, no
  recursive delete, no quota/ownership model beyond host policy
  (disk-full surfaces as `RuntimeFailure`, never silent truncation).
- No typed error sums: failures are realization-time
  `InvalidRequest`/`RuntimeFailure` (Tranche C work), and power-loss
  survival must never be inferred from a barrier receipt alone.
- Unrestricted `while` loops, heap allocation, traits, unrestricted
  callable values, and a conventional runtime model are all out of
  scope; boundedness, proof/evidence discipline, semantic identity, and
  the explicit backend capability model still govern.
