//! Machine-native filesystem/resource effects (index PRESS-003/007).
//!
//! A granted filesystem root is a bounded, explicitly authorized resource
//! namespace — never ambient host paths. Programs name no paths at all:
//! they enumerate a canonical entry sequence under a capability grant and
//! read bounded chunks by entry index and offset. Authority travels only
//! in the capability grant (`--grant-fs capability=root-path`); a call
//! without a matching grant fails closed, and traversal outside the
//! granted root is impossible by construction (the walk starts at the
//! root and relative paths never escape it).
//!
//! Determinism contract (what `mncs-index` discovery needs):
//! - the entry sequence is sorted by relative-path bytes, never by host
//!   readdir order, so creation/enumeration order cannot leak;
//! - every listing carries a content identity (SHA-256 over the canonical
//!   entry encoding) plus a generation counter covering names, kinds,
//!   sizes, and mtimes, so readers detect mutation and re-list;
//! - entries whose relative path exceeds 64 bytes are skipped from the
//!   listing (mirroring the PRESS-014 truncation-vs-skip policy: never
//!   truncate into a wrong identity) but still feed the generation, so
//!   their mutation is observable;
//! - symlinks are listed as `other` and never followed; reads through
//!   them are refused;
//! - more than 1024 entries fails the listing closed (the declared
//!   bound, matching `MAX_SEQUENCE_BOUND`); IO errors fail closed.
//!
//! The watch half of PRESS-007 is served as mutation/event hints, not
//! push delivery: `fs_generation` is a monotonic-per-change counter the
//! source polls between bounded quiet windows. True event delivery stays
//! future work and is recorded as such.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::canonical::sha256_hex;
use crate::execution::{ExecutionValue, HostGrant};

/// Largest listable directory tree: matches `MAX_SEQUENCE_BOUND` so an
/// entry index always fits the language's sequence domain.
pub const FS_LIST_MAX_ENTRIES: usize = 1024;

/// Longest listable relative path: matches the 64-byte view bound so
/// every name fits one `[byte; up_to 64]` value without truncation.
pub const FS_NAME_MAX_BYTES: usize = 64;

/// Largest single chunk read: one view's worth of bytes.
pub const FS_READ_MAX_BYTES: u64 = 64;

/// Entry kinds: 0 = regular file, 1 = directory, 2 = anything else
/// (symlink, socket, device, or an unreadable type probe).
pub const FS_KIND_FILE: u64 = 0;
pub const FS_KIND_DIR: u64 = 1;
pub const FS_KIND_OTHER: u64 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsEntry {
    /// Relative path from the granted root, as raw bytes (`/`-separated).
    pub rel: Vec<u8>,
    pub kind: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsSnapshot {
    pub entries: Vec<FsEntry>,
    /// Content identity: SHA-256 over the canonical entry encoding
    /// (`kind-byte || rel-bytes || 0x00` per entry, in order).
    pub snapshot_sha256: String,
    /// Mutation hint: first 8 bytes of SHA-256 over
    /// (`kind || rel || 0x00 || len-le64 || mtime-nanos-le64-or-zero`)
    /// per walked path in order, including skipped overlong names.
    pub generation: u64,
}

#[cfg(unix)]
fn os_bytes(name: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    name.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn os_bytes(name: &std::ffi::OsStr) -> Vec<u8> {
    name.to_string_lossy().as_bytes().to_vec()
}

fn push_component(current: &mut Vec<u8>, name: &std::ffi::OsStr) {
    if !current.is_empty() {
        current.push(b'/');
    }
    current.extend_from_slice(&os_bytes(name));
}

/// Walk the granted root into a canonical snapshot. Fails closed on IO
/// errors and on trees beyond [`FS_LIST_MAX_ENTRIES`]; overlong relative
/// paths are skipped from the listing (but feed the generation).
pub fn fs_snapshot(root: &Path) -> Result<FsSnapshot, String> {
    let canonical = std::fs::canonicalize(root)
        .map_err(|error| format!("fs grant root {root:?} is not a readable directory: {error}"))?;
    if !canonical.is_dir() {
        return Err(format!("fs grant root {root:?} is not a directory"));
    }
    let mut ordered: BTreeMap<Vec<u8>, u64> = BTreeMap::new();
    let mut gen_parts: Vec<u8> = Vec::new();
    let mut stack: Vec<(PathBuf, Vec<u8>)> = vec![(canonical.clone(), Vec::new())];
    // Iterative descent with an explicit stack; directory handles are
    // never held across iterations, so concurrent mutation can only
    // surface as IO errors (fail closed) or generation drift (re-list).
    while let Some((dir, rel)) = stack.pop() {
        let read = std::fs::read_dir(&dir)
            .map_err(|error| format!("fs list of {dir:?} failed: {error}"))?;
        let mut children: Vec<(Vec<u8>, PathBuf, std::fs::FileType)> = Vec::new();
        for child in read {
            let child = child.map_err(|error| format!("fs list of {dir:?} failed: {error}"))?;
            let file_type = child
                .file_type()
                .map_err(|error| format!("fs type probe of {dir:?} failed: {error}"))?;
            let mut child_rel = rel.clone();
            push_component(&mut child_rel, &child.file_name());
            children.push((child_rel, child.path(), file_type));
        }
        children.sort_by(|left, right| left.0.cmp(&right.0));
        for (child_rel, child_path, file_type) in children {
            let kind = if file_type.is_symlink() {
                FS_KIND_OTHER
            } else if file_type.is_dir() {
                FS_KIND_DIR
            } else if file_type.is_file() {
                FS_KIND_FILE
            } else {
                FS_KIND_OTHER
            };
            // Generation covers every walked path, including skipped
            // overlong names, so no mutation is invisible to pollers.
            let metadata = std::fs::symlink_metadata(&child_path);
            let (len, mtime_nanos) = match metadata {
                Ok(meta) => {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|elapsed| elapsed.as_nanos().min(u128::from(u64::MAX)) as u64)
                        .unwrap_or(0);
                    (meta.len(), mtime)
                }
                Err(_) => (0, 0),
            };
            gen_parts.push(kind as u8);
            gen_parts.extend_from_slice(&child_rel);
            gen_parts.push(0);
            gen_parts.extend_from_slice(&len.to_le_bytes());
            gen_parts.extend_from_slice(&mtime_nanos.to_le_bytes());
            if child_rel.len() > FS_NAME_MAX_BYTES {
                continue;
            }
            if ordered.len() >= FS_LIST_MAX_ENTRIES && !ordered.contains_key(&child_rel) {
                return Err(format!(
                    "fs list of {root:?} exceeds the {FS_LIST_MAX_ENTRIES}-entry bound; refusing"
                ));
            }
            ordered.insert(child_rel.clone(), kind);
            if kind == FS_KIND_DIR {
                stack.push((child_path, child_rel));
            }
        }
    }
    let mut snapshot_parts: Vec<u8> = Vec::new();
    let entries: Vec<FsEntry> = ordered
        .into_iter()
        .map(|(rel, kind)| {
            snapshot_parts.push(kind as u8);
            snapshot_parts.extend_from_slice(&rel);
            snapshot_parts.push(0);
            FsEntry { rel, kind }
        })
        .collect();
    let snapshot_sha256 = sha256_hex(&snapshot_parts);
    let gen_digest = crate::execution::sha256_digest_bytes(&gen_parts);
    let mut gen_bytes = [0u8; 8];
    gen_bytes.copy_from_slice(&gen_digest[..8]);
    Ok(FsSnapshot {
        entries,
        snapshot_sha256,
        generation: u64::from_le_bytes(gen_bytes),
    })
}

/// Realized effect record for one filesystem call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsEffect {
    pub kind: String,
    pub target: String,
    pub provenance: String,
}

/// Typed realization failure: callers map these to InvalidRequest or
/// RuntimeFailure without inventing a third meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsFail {
    InvalidRequest(String),
    RuntimeFailure(String),
}

fn u64_operand(args: &[u64], position: usize, intrinsic: &str) -> Result<u64, FsFail> {
    args.get(position).copied().ok_or_else(|| {
        FsFail::InvalidRequest(format!(
            "{intrinsic} is missing its operand at position {position}"
        ))
    })
}

/// Realize one filesystem intrinsic against an already-matched grant.
/// `args` are the call's u64 operands in order (indices, offsets,
/// lengths); view-returning calls deliver `[byte; up_to 64]` values.
pub fn fs_realize(
    operation: &str,
    grant: &HostGrant,
    args: &[u64],
) -> Result<(ExecutionValue, FsEffect), FsFail> {
    let root = Path::new(&grant.locator);
    let snapshot = fs_snapshot(root).map_err(FsFail::InvalidRequest)?;
    let list_provenance = format!(
        "grant:{} entries:{} gen:{:016x} snapshot:{}",
        grant.locator,
        snapshot.entries.len(),
        snapshot.generation,
        snapshot.snapshot_sha256
    );
    match operation {
        "fs_list_count" => Ok((
            ExecutionValue::Integer {
                value: snapshot.entries.len() as i128,
                ty: crate::IntegerType {
                    bits: 64,
                    signed: false,
                },
            },
            FsEffect {
                kind: "fs_list".to_owned(),
                target: "dir_list".to_owned(),
                provenance: list_provenance,
            },
        )),
        "fs_generation" => Ok((
            ExecutionValue::Integer {
                value: snapshot.generation as i128,
                ty: crate::IntegerType {
                    bits: 64,
                    signed: false,
                },
            },
            FsEffect {
                kind: "fs_list".to_owned(),
                target: "dir_list".to_owned(),
                provenance: format!("grant:{} gen:{:016x}", grant.locator, snapshot.generation),
            },
        )),
        "fs_entry_name_at" | "fs_entry_kind_at" => {
            let index = u64_operand(args, 0, operation)? as usize;
            let entry = snapshot.entries.get(index).ok_or_else(|| {
                FsFail::InvalidRequest(format!(
                    "{operation} index {index} is outside the {}-entry listing; re-list, the tree may have changed",
                    snapshot.entries.len()
                ))
            })?;
            if operation == "fs_entry_kind_at" {
                Ok((
                    ExecutionValue::Integer {
                        value: entry.kind as i128,
                        ty: crate::IntegerType {
                            bits: 64,
                            signed: false,
                        },
                    },
                    FsEffect {
                        kind: "fs_list".to_owned(),
                        target: "dir_list".to_owned(),
                        provenance: format!("{list_provenance} index:{index}"),
                    },
                ))
            } else {
                Ok((
                    ExecutionValue::Sequence {
                        values: entry
                            .rel
                            .iter()
                            .map(|byte| ExecutionValue::Byte {
                                value: *byte as i128,
                            })
                            .collect::<Vec<_>>()
                            .into(),
                    },
                    FsEffect {
                        kind: "fs_list".to_owned(),
                        target: "dir_list".to_owned(),
                        provenance: format!("{list_provenance} index:{index}"),
                    },
                ))
            }
        }
        "fs_read_bytes_at" => {
            let index = u64_operand(args, 0, operation)? as usize;
            let offset = u64_operand(args, 1, operation)?;
            let length = u64_operand(args, 2, operation)?;
            let entry = snapshot.entries.get(index).ok_or_else(|| {
                FsFail::InvalidRequest(format!(
                    "{operation} index {index} is outside the {}-entry listing; re-list, the tree may have changed",
                    snapshot.entries.len()
                ))
            })?;
            if entry.kind != FS_KIND_FILE {
                return Err(FsFail::InvalidRequest(format!(
                    "{operation} entry index {index} is not a regular file; refusing"
                )));
            }
            let canonical_root = std::fs::canonicalize(root).map_err(|error| {
                FsFail::InvalidRequest(format!("fs grant root is not readable: {error}"))
            })?;
            // `rel` is built from real walked components (never `..`),
            // joined byte-exact so non-UTF8 names cannot collide through
            // lossy rendering. The joined path is re-canonicalized and
            // prefix-checked, so a symlink swap between listing and read
            // resolves outside the root and is refused rather than
            // followed.
            let mut path = canonical_root.clone();
            path.push(rel_os_str(&entry.rel));
            let resolved = std::fs::canonicalize(&path).map_err(|error| {
                FsFail::RuntimeFailure(format!(
                    "{operation} entry index {index} vanished or is unreadable: {error}"
                ))
            })?;
            if !resolved.starts_with(&canonical_root) {
                return Err(FsFail::InvalidRequest(format!(
                    "{operation} entry resolves outside the granted root; refusing"
                )));
            }
            // Bounded read: only the requested window (at most 64 bytes
            // past the clamped start) is ever loaded, so a large file
            // under a granted root cannot exhaust host memory. The
            // length comes from metadata and the content read is capped
            // independently; a truncation race between the metadata
            // probe and the read fails closed (RuntimeFailure) rather
            // than delivering a torn prefix as if it were complete.
            let file_len = std::fs::metadata(&resolved)
                .map(|meta| meta.len())
                .unwrap_or(0);
            let start = (offset as usize).min(file_len as usize);
            let end = start
                .saturating_add((length.min(FS_READ_MAX_BYTES)) as usize)
                .min(file_len as usize);
            let mut bytes = vec![0u8; end.saturating_sub(start)];
            if !bytes.is_empty() {
                use std::io::{Read, Seek, SeekFrom};
                let mut file = std::fs::File::open(&resolved).map_err(|error| {
                    FsFail::RuntimeFailure(format!(
                        "{operation} read of entry index {index} failed: {error}"
                    ))
                })?;
                file.seek(SeekFrom::Start(start as u64)).map_err(|error| {
                    FsFail::RuntimeFailure(format!(
                        "{operation} read of entry index {index} failed: {error}"
                    ))
                })?;
                file.read_exact(&mut bytes).map_err(|error| {
                    FsFail::RuntimeFailure(format!(
                        "{operation} read of entry index {index} failed: {error}"
                    ))
                })?;
            }
            let chunk = &bytes[..];
            Ok((
                ExecutionValue::Sequence {
                    values: chunk
                        .iter()
                        .map(|byte| ExecutionValue::Byte {
                            value: *byte as i128,
                        })
                        .collect::<Vec<_>>()
                        .into(),
                },
                FsEffect {
                    kind: "fs_read".to_owned(),
                    target: "file_read".to_owned(),
                    provenance: format!(
                        "grant:{} path:{} sha256:{} gen:{:016x}",
                        grant.locator,
                        String::from_utf8_lossy(&entry.rel),
                        sha256_hex(chunk),
                        snapshot.generation
                    ),
                },
            ))
        }
        _ => Err(FsFail::InvalidRequest(format!(
            "unknown filesystem operation {operation:?}; fail closed"
        ))),
    }
}

/// Largest single mutation payload: one view's worth of bytes. Names and
/// content both arrive as `[byte; up_to 64]` values, so no mutation call
/// carries more than 64 bytes of new state; multi-chunk staging composes
/// `fs_create_file` with `fs_append_bytes_at`. Aggregate quota (disk
/// full, operator limits) stays host policy and surfaces as
/// `RuntimeFailure`, never as silent truncation.
pub const FS_WRITE_MAX_BYTES: usize = 64;

/// Operand positions holding byte-view operands (entry names, content)
/// for each mutating operation; every other operand position holds a
/// `u64` scalar. Both executors split call operands through this table,
/// so the position rule lives in exactly one place. `fs_realize` (reads)
/// takes no views; only the operations named here reach [`fs_mutate`].
pub fn fs_view_positions(operation: &str) -> &'static [usize] {
    match operation {
        "fs_create_file" => &[0, 1],
        "fs_write_bytes_at" => &[2],
        "fs_append_bytes_at" => &[1],
        "fs_mkdir" => &[0],
        "fs_rename_at" => &[1],
        _ => &[],
    }
}

fn u64_value(value: u64) -> ExecutionValue {
    ExecutionValue::Integer {
        value: value as i128,
        ty: crate::IntegerType {
            bits: 64,
            signed: false,
        },
    }
}

/// Validate a caller-supplied entry name (P1-003/P1-017): names are bare
/// single components, never paths. There is no path type yet (Tranche E),
/// so the byte view is policed here — fail closed, never normalized: no
/// separators, no NUL, never `.` or `..`, non-empty, at most 64 bytes
/// (already enforced by the view bound; re-checked, never trusted).
fn check_entry_name(name: &[u8], operation: &str) -> Result<(), FsFail> {
    if name.is_empty() || name.len() > FS_NAME_MAX_BYTES {
        return Err(FsFail::InvalidRequest(format!(
            "{operation} entry name must be 1..={FS_NAME_MAX_BYTES} bytes, not {}",
            name.len()
        )));
    }
    if name.contains(&b'/') || name.contains(&0) {
        return Err(FsFail::InvalidRequest(format!(
            "{operation} entry name is a bare component, never a path; refusing"
        )));
    }
    if name == b"." || name == b".." {
        return Err(FsFail::InvalidRequest(format!(
            "{operation} entry name must not be `.` or `..`; refusing"
        )));
    }
    Ok(())
}

/// Resolve one listing index against a fresh snapshot. Indices are data,
/// never authority: a stale or wild index is `InvalidRequest` with a
/// re-list hint, never a value.
fn snapshot_entry(snapshot: &FsSnapshot, index: usize, operation: &str) -> Result<FsEntry, FsFail> {
    snapshot.entries.get(index).cloned().ok_or_else(|| {
        FsFail::InvalidRequest(format!(
            "{operation} index {index} is outside the {}-entry listing; re-list, the tree may have changed",
            snapshot.entries.len()
        ))
    })
}

/// Re-snapshot after a mutation: post-state (new index, new generation)
/// is observed, never predicted. A vanished root here means the mutation
/// may have landed without observable post-state — `RuntimeFailure`,
/// never a fabricated index.
fn resnapshot(root: &Path, operation: &str) -> Result<FsSnapshot, FsFail> {
    fs_snapshot(root).map_err(|error| {
        FsFail::RuntimeFailure(format!(
            "{operation} succeeded but post-state is unreadable: {error}"
        ))
    })
}

/// Compute the index a new relative path WOULD take in a snapshot's
/// canonical order, without mutating anything. Intent-only realization
/// (`realize == false`) reports post-state values computed from observed
/// pre-state: the exclusivity check already refused an existing equal
/// path, so the rank is exact, never a guess.
fn would_be_index(snapshot: &FsSnapshot, rel: &[u8]) -> u64 {
    snapshot
        .entries
        .iter()
        .filter(|entry| entry.rel.as_slice() < rel)
        .count() as u64
}

/// Locate one relative path in a snapshot. Absence is `RuntimeFailure`
/// (the just-mutated entry must be listable); callers only ask for paths
/// their own call created or moved.
fn snapshot_index_of(snapshot: &FsSnapshot, rel: &[u8], operation: &str) -> Result<u64, FsFail> {
    snapshot
        .entries
        .iter()
        .position(|entry| entry.rel == rel)
        .map(|index| index as u64)
        .ok_or_else(|| {
            FsFail::RuntimeFailure(format!(
                "{operation} post-state does not list the mutated entry; re-list"
            ))
        })
}

/// Join a snapshot-built relative path under the canonical root and
/// resolve it with a prefix check, mirroring the read path: a symlink
/// swap between listing and mutation resolves outside the root and is
/// refused rather than followed.
fn resolve_existing(
    canonical_root: &Path,
    rel: &[u8],
    operation: &str,
    index: usize,
) -> Result<PathBuf, FsFail> {
    let mut path = canonical_root.to_path_buf();
    path.push(rel_os_str(rel));
    let resolved = std::fs::canonicalize(&path).map_err(|error| {
        FsFail::RuntimeFailure(format!(
            "{operation} entry index {index} vanished or is unreadable: {error}"
        ))
    })?;
    if !resolved.starts_with(canonical_root) {
        return Err(FsFail::InvalidRequest(format!(
            "{operation} entry resolves outside the granted root; refusing"
        )));
    }
    Ok(resolved)
}

/// Realize one filesystem mutation against an already-matched grant
/// (Tranche A: P1-001/P1-002/P1-003, effect `fs_write`). `ints` and
/// `views` are the call's operands in position order, already split by
/// the executors through [`fs_view_positions`]; names and content travel
/// only in views, indices/offsets only in `ints`.
///
/// `realize == false` is the verify-only path (Record policy): every
/// check runs — grant, snapshot, names, indices, bounds, exclusivity —
/// and the would-be value is computed from observed pre-state, but no
/// syscall mutates the tree. Only the designated real execution phase
/// passes `realize == true`, exactly once (see
/// `host_operation_mutates`).
///
/// Positioning rules (bounded, explicit, fail-closed):
/// - writes never create sparse gaps: `offset <= file_len`, else
///   `InvalidRequest`; `offset + len` may grow the file by at most one
///   view (64 bytes) per call;
/// - create/mkdir/rename-destination are exclusive at the syscall
///   (`create_new` / `create_dir` fail when present) and address the
///   granted root (create/mkdir) or the source's own parent directory
///   (rename) — never an ambient path, never a cross-directory move;
/// - rename atomically replaces a non-directory destination where the
///   platform provides atomic replace (POSIX); replacing a directory
///   is always refused;
/// - delete removes files and empty directories only; `other` entries
///   (symlinks, sockets, devices) are never touched;
/// - nothing fsyncs implicitly: durability is the separate explicit
///   barrier [`fs_sync_at`](self::fs_mutate#fs_sync_at) (P1-002), so a
///   crash between mutation and barrier is observable generation drift,
///   never a false durability claim.
pub fn fs_mutate(
    operation: &str,
    grant: &HostGrant,
    ints: &[u64],
    views: &[Vec<u8>],
    realize: bool,
) -> Result<(ExecutionValue, FsEffect), FsFail> {
    let root = Path::new(&grant.locator);
    let snapshot = fs_snapshot(root).map_err(FsFail::InvalidRequest)?;
    let canonical_root = std::fs::canonicalize(root).map_err(|error| {
        FsFail::InvalidRequest(format!("fs grant root is not readable: {error}"))
    })?;
    // Post-mutation observation shared by every arm: re-snapshot once
    // the syscall lands (or immediately for intent-only, where pre- and
    // post-state coincide absent concurrent host mutation), then report
    // the new generation with the arm's value.
    let finish = |snapshot: FsSnapshot,
                  value: ExecutionValue,
                  target: &str,
                  detail: String|
     -> Result<(ExecutionValue, FsEffect), FsFail> {
        Ok((
            value,
            FsEffect {
                kind: "fs_write".to_owned(),
                target: target.to_owned(),
                provenance: format!(
                    "grant:{} op:{operation} {detail} gen:{:016x}",
                    grant.locator, snapshot.generation,
                ),
            },
        ))
    };
    match operation {
        "fs_create_file" => {
            let name = views.first().cloned().unwrap_or_default();
            let content = views.get(1).cloned().unwrap_or_default();
            if !ints.is_empty() || views.len() != 2 {
                return Err(FsFail::InvalidRequest("fs_create_file takes no u64 operands and two byte-view operands (name, content)".to_owned()));
            }
            check_entry_name(&name, operation)?;
            if content.len() > FS_WRITE_MAX_BYTES {
                return Err(FsFail::InvalidRequest(format!(
                    "fs_create_file content must fit one view (at most {FS_WRITE_MAX_BYTES} bytes)"
                )));
            }
            let mut target = canonical_root.clone();
            target.push(rel_os_str(&name));
            if std::fs::symlink_metadata(&target).is_ok() {
                return Err(FsFail::InvalidRequest(
                    "fs_create_file entry already exists; refusing to overwrite".to_owned(),
                ));
            }
            if realize {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)
                    .map_err(|error| {
                        if error.kind() == std::io::ErrorKind::AlreadyExists {
                            FsFail::InvalidRequest(
                                "fs_create_file entry already exists; refusing to overwrite"
                                    .to_owned(),
                            )
                        } else {
                            FsFail::RuntimeFailure(format!("fs_create_file write failed: {error}"))
                        }
                    })?;
                file.write_all(&content).map_err(|error| {
                    FsFail::RuntimeFailure(format!("fs_create_file write failed: {error}"))
                })?;
            }
            // Intent-only reports the computed rank in observed
            // pre-state; realized calls observe the post-state listing.
            let (post, index) = if realize {
                let post = resnapshot(root, operation)?;
                let index = snapshot_index_of(&post, &name, operation)?;
                (post, index)
            } else {
                (snapshot.clone(), would_be_index(&snapshot, &name))
            };
            finish(
                post,
                u64_value(index),
                "file_create",
                format!(
                    "path:{} sha256:{}",
                    String::from_utf8_lossy(&name),
                    sha256_hex(&content),
                ),
            )
        }
        "fs_write_bytes_at" => {
            if ints.len() != 2 || views.len() != 1 {
                return Err(FsFail::InvalidRequest("fs_write_bytes_at takes two u64 operands (entry, offset) and one byte-view operand".to_owned()));
            }
            let index = ints[0] as usize;
            let offset = ints[1];
            let bytes = &views[0];
            if bytes.len() > FS_WRITE_MAX_BYTES {
                return Err(FsFail::InvalidRequest(format!("fs_write_bytes_at payload must fit one view (at most {FS_WRITE_MAX_BYTES} bytes)")));
            }
            let entry = snapshot_entry(&snapshot, index, operation)?;
            if entry.kind != FS_KIND_FILE {
                return Err(FsFail::InvalidRequest(format!(
                    "{operation} entry index {index} is not a regular file; refusing"
                )));
            }
            let resolved = resolve_existing(&canonical_root, &entry.rel, operation, index)?;
            let file_len = std::fs::metadata(&resolved)
                .map(|meta| meta.len())
                .unwrap_or(0);
            if (offset as u128) > file_len as u128 {
                return Err(FsFail::InvalidRequest(format!("{operation} offset {offset} is past end-of-file {file_len}; writes never create sparse gaps")));
            }
            if realize {
                use std::io::{Seek, SeekFrom, Write};
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .open(&resolved)
                    .map_err(|error| {
                        FsFail::RuntimeFailure(format!(
                            "{operation} write of entry index {index} failed: {error}"
                        ))
                    })?;
                file.seek(SeekFrom::Start(offset)).map_err(|error| {
                    FsFail::RuntimeFailure(format!(
                        "{operation} write of entry index {index} failed: {error}"
                    ))
                })?;
                file.write_all(bytes).map_err(|error| {
                    FsFail::RuntimeFailure(format!(
                        "{operation} write of entry index {index} failed: {error}"
                    ))
                })?;
            }
            let post = resnapshot(root, operation)?;
            finish(
                post,
                u64_value(bytes.len() as u64),
                "file_write",
                format!(
                    "path:{} offset:{offset} sha256:{}",
                    String::from_utf8_lossy(&entry.rel),
                    sha256_hex(bytes),
                ),
            )
        }
        "fs_append_bytes_at" => {
            if ints.len() != 1 || views.len() != 1 {
                return Err(FsFail::InvalidRequest(
                    "fs_append_bytes_at takes one u64 operand (entry) and one byte-view operand"
                        .to_owned(),
                ));
            }
            let index = ints[0] as usize;
            let bytes = &views[0];
            if bytes.len() > FS_WRITE_MAX_BYTES {
                return Err(FsFail::InvalidRequest(format!("fs_append_bytes_at payload must fit one view (at most {FS_WRITE_MAX_BYTES} bytes)")));
            }
            let entry = snapshot_entry(&snapshot, index, operation)?;
            if entry.kind != FS_KIND_FILE {
                return Err(FsFail::InvalidRequest(format!(
                    "{operation} entry index {index} is not a regular file; refusing"
                )));
            }
            let resolved = resolve_existing(&canonical_root, &entry.rel, operation, index)?;
            if realize {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&resolved)
                    .map_err(|error| {
                        FsFail::RuntimeFailure(format!(
                            "{operation} append to entry index {index} failed: {error}"
                        ))
                    })?;
                file.write_all(bytes).map_err(|error| {
                    FsFail::RuntimeFailure(format!(
                        "{operation} append to entry index {index} failed: {error}"
                    ))
                })?;
            }
            let post = resnapshot(root, operation)?;
            finish(
                post,
                u64_value(bytes.len() as u64),
                "file_append",
                format!(
                    "path:{} sha256:{}",
                    String::from_utf8_lossy(&entry.rel),
                    sha256_hex(bytes),
                ),
            )
        }
        "fs_mkdir" => {
            let name = views.first().cloned().unwrap_or_default();
            if !ints.is_empty() || views.len() != 1 {
                return Err(FsFail::InvalidRequest(
                    "fs_mkdir takes no u64 operands and one byte-view operand (name)".to_owned(),
                ));
            }
            check_entry_name(&name, operation)?;
            let mut target = canonical_root.clone();
            target.push(rel_os_str(&name));
            if std::fs::symlink_metadata(&target).is_ok() {
                return Err(FsFail::InvalidRequest(
                    "fs_mkdir entry already exists; refusing".to_owned(),
                ));
            }
            if realize {
                std::fs::create_dir(&target).map_err(|error| {
                    if error.kind() == std::io::ErrorKind::AlreadyExists {
                        FsFail::InvalidRequest("fs_mkdir entry already exists; refusing".to_owned())
                    } else {
                        FsFail::RuntimeFailure(format!("fs_mkdir failed: {error}"))
                    }
                })?;
            }
            let (post, index) = if realize {
                let post = resnapshot(root, operation)?;
                let index = snapshot_index_of(&post, &name, operation)?;
                (post, index)
            } else {
                (snapshot.clone(), would_be_index(&snapshot, &name))
            };
            finish(
                post,
                u64_value(index),
                "dir_create",
                format!("path:{}", String::from_utf8_lossy(&name)),
            )
        }
        "fs_delete_at" => {
            if ints.len() != 1 || !views.is_empty() {
                return Err(FsFail::InvalidRequest(
                    "fs_delete_at takes exactly one u64 operand (entry)".to_owned(),
                ));
            }
            let index = ints[0] as usize;
            let entry = snapshot_entry(&snapshot, index, operation)?;
            if entry.kind != FS_KIND_FILE && entry.kind != FS_KIND_DIR {
                return Err(FsFail::InvalidRequest(format!(
                    "{operation} entry index {index} is neither a file nor a directory; refusing"
                )));
            }
            let resolved = resolve_existing(&canonical_root, &entry.rel, operation, index)?;
            if entry.kind == FS_KIND_DIR {
                let empty = std::fs::read_dir(&resolved)
                    .map_err(|error| {
                        FsFail::RuntimeFailure(format!(
                            "{operation} read of entry index {index} failed: {error}"
                        ))
                    })?
                    .next()
                    .is_none();
                if !empty {
                    return Err(FsFail::InvalidRequest(format!(
                        "{operation} entry index {index} is a non-empty directory; refusing"
                    )));
                }
                if realize {
                    std::fs::remove_dir(&resolved).map_err(|error| {
                        FsFail::RuntimeFailure(format!(
                            "{operation} delete of entry index {index} failed: {error}"
                        ))
                    })?;
                }
            } else if realize {
                std::fs::remove_file(&resolved).map_err(|error| {
                    FsFail::RuntimeFailure(format!(
                        "{operation} delete of entry index {index} failed: {error}"
                    ))
                })?;
            }
            let post = resnapshot(root, operation)?;
            finish(
                post,
                u64_value(entry.kind),
                "entry_delete",
                format!(
                    "path:{} was_kind:{}",
                    String::from_utf8_lossy(&entry.rel),
                    entry.kind,
                ),
            )
        }
        "fs_rename_at" => {
            if ints.len() != 1 || views.len() != 1 {
                return Err(FsFail::InvalidRequest("fs_rename_at takes one u64 operand (entry) and one byte-view operand (new name)".to_owned()));
            }
            let index = ints[0] as usize;
            let new_name = &views[0];
            check_entry_name(new_name, operation)?;
            let entry = snapshot_entry(&snapshot, index, operation)?;
            if entry.kind != FS_KIND_FILE && entry.kind != FS_KIND_DIR {
                return Err(FsFail::InvalidRequest(format!(
                    "{operation} entry index {index} is neither a file nor a directory; refusing"
                )));
            }
            let resolved = resolve_existing(&canonical_root, &entry.rel, operation, index)?;
            let parent = resolved
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| canonical_root.clone());
            if !parent.starts_with(&canonical_root) {
                return Err(FsFail::InvalidRequest(format!(
                    "{operation} entry parent is outside the granted root; refusing"
                )));
            }
            let mut dest = parent.clone();
            dest.push(rel_os_str(new_name));
            if let Ok(meta) = std::fs::symlink_metadata(&dest) {
                if meta.is_dir() && !meta.file_type().is_symlink() {
                    return Err(FsFail::InvalidRequest(format!("{operation} destination already names a directory; atomic replace covers files only")));
                }
            }
            if realize {
                std::fs::rename(&resolved, &dest).map_err(|error| {
                    FsFail::RuntimeFailure(format!(
                        "{operation} rename of entry index {index} failed: {error}"
                    ))
                })?;
            }
            // The renamed entry's relative path: same parent components,
            // new leaf. Track it through post-state rather than trusting
            // string surgery on `rel`.
            let mut new_rel = entry.rel.clone();
            if let Some(slash) = new_rel.iter().rposition(|byte| *byte == b'/') {
                new_rel.truncate(slash + 1);
                new_rel.extend_from_slice(new_name);
            } else {
                new_rel = new_name.clone();
            }
            let (post, new_index) = if realize {
                let post = resnapshot(root, operation)?;
                let new_index = snapshot_index_of(&post, &new_rel, operation)?;
                (post, new_index)
            } else {
                // Intent-only: rank the new path against pre-state with
                // the moved source removed (the destination is absent
                // and exclusivity already refused a collision).
                let mut projected = snapshot.clone();
                projected
                    .entries
                    .retain(|candidate| candidate.rel != entry.rel);
                (snapshot.clone(), would_be_index(&projected, &new_rel))
            };
            finish(
                post,
                u64_value(new_index),
                "entry_rename",
                format!(
                    "from:{} to:{}",
                    String::from_utf8_lossy(&entry.rel),
                    String::from_utf8_lossy(&new_rel),
                ),
            )
        }
        "fs_sync_at" => {
            if ints.len() != 1 || !views.is_empty() {
                return Err(FsFail::InvalidRequest(
                    "fs_sync_at takes exactly one u64 operand (entry)".to_owned(),
                ));
            }
            let index = ints[0] as usize;
            let entry = snapshot_entry(&snapshot, index, operation)?;
            if entry.kind != FS_KIND_FILE {
                return Err(FsFail::InvalidRequest(format!("{operation} entry index {index} is not a regular file; the barrier covers staged file content")));
            }
            let resolved = resolve_existing(&canonical_root, &entry.rel, operation, index)?;
            let mut dirsync = "dirsync:ok";
            if realize {
                std::fs::File::open(&resolved)
                    .and_then(|file| file.sync_all())
                    .map_err(|error| {
                        FsFail::RuntimeFailure(format!(
                            "{operation} barrier for entry index {index} failed: {error}"
                        ))
                    })?;
                // The commit barrier is file bytes PLUS the namespace
                // edges that publish them: containing directory and root.
                // Directory fsync is POSIX-only; elsewhere the file
                // barrier still holds and the gap is recorded in
                // provenance, never hidden (P1-002 honesty).
                for dir in resolved
                    .parent()
                    .into_iter()
                    .chain(std::iter::once(canonical_root.as_path()))
                {
                    match std::fs::File::open(dir).and_then(|file| file.sync_all()) {
                        Ok(()) => {}
                        Err(_) if cfg!(not(unix)) => {
                            dirsync = "dirsync:unsupported-platform";
                        }
                        Err(error) => {
                            return Err(FsFail::RuntimeFailure(format!(
                                "{operation} directory barrier failed: {error}"
                            )));
                        }
                    }
                }
            }
            let post = resnapshot(root, operation)?;
            finish(
                post,
                u64_value(1),
                "file_sync",
                format!(
                    "path:{} sha256:of-content-unread {dirsync}",
                    String::from_utf8_lossy(&entry.rel),
                ),
            )
        }
        _ => Err(FsFail::InvalidRequest(format!(
            "unknown filesystem mutation {operation:?}; fail closed"
        ))),
    }
}

/// Extract one u64 call operand from evaluated values. Signed or
/// out-of-range integers are InvalidRequest: the caller's index is
/// ill-formed, never a default. Generic over the value-map key so the
/// body executor (`String`) and the SSA executor (`SemanticId`) share
/// one extraction rule.
pub fn host_u64_by<K: Ord>(
    operands: &[K],
    values: &std::collections::BTreeMap<K, ExecutionValue>,
    position: usize,
) -> Option<u64> {
    let binding = operands.get(position)?;
    match values.get(binding)? {
        ExecutionValue::Integer { value, ty } if ty.bits == 64 && !ty.signed => {
            u64::try_from(*value).ok()
        }
        _ => None,
    }
}

#[cfg(unix)]
fn rel_os_str(rel: &[u8]) -> std::ffi::OsString {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::OsStr::from_bytes(rel).to_owned()
}

#[cfg(not(unix))]
fn rel_os_str(rel: &[u8]) -> std::ffi::OsString {
    String::from_utf8_lossy(rel).into_owned().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "mncs-fs-mutate-{}-{}-{:?}",
            tag,
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create test root");
        root
    }

    fn grant_for(root: &std::path::Path) -> HostGrant {
        HostGrant {
            capability: "fs_root".to_owned(),
            locator: root.to_string_lossy().into_owned(),
            bytes: Vec::new(),
        }
    }

    fn u64_of(value: &ExecutionValue) -> u64 {
        match value {
            ExecutionValue::Integer { value, ty } if ty.bits == 64 && !ty.signed => {
                u64::try_from(*value).expect("u64 range")
            }
            other => panic!("expected u64, got {other:?}"),
        }
    }

    fn effect_kind_of(effect: &FsEffect) -> &str {
        effect.kind.as_str()
    }

    /// The staged lifecycle lands byte-exact on disk: create, append,
    /// positioned overwrite, rename, barrier, and both deletes.
    #[test]
    fn staged_lifecycle_lands_byte_exact() {
        let root = test_root("lifecycle");
        let grant = grant_for(&root);
        // Stage "chunk" = [1,2,3,4]; fresh root, so index 0.
        let (created, effect) = fs_mutate(
            "fs_create_file",
            &grant,
            &[],
            &[b"chunk".to_vec(), vec![1, 2, 3, 4]],
            true,
        )
        .expect("create");
        assert_eq!(u64_of(&created), 0);
        assert_eq!(effect_kind_of(&effect), "fs_write");
        assert_eq!(effect.target, "file_create");
        assert!(effect.provenance.contains("op:fs_create_file"));
        assert_eq!(
            std::fs::read(root.join("chunk")).expect("read"),
            vec![1, 2, 3, 4]
        );
        // Append [5,6] and overwrite position 0 with [9].
        let (appended, _) =
            fs_mutate("fs_append_bytes_at", &grant, &[0], &[vec![5, 6]], true).expect("append");
        assert_eq!(u64_of(&appended), 2);
        let (written, _) =
            fs_mutate("fs_write_bytes_at", &grant, &[0, 0], &[vec![9]], true).expect("write");
        assert_eq!(u64_of(&written), 1);
        assert_eq!(
            std::fs::read(root.join("chunk")).expect("read"),
            vec![9, 2, 3, 4, 5, 6]
        );
        // Publish: rename to "final"; the read half sees the bytes.
        let (moved, effect) =
            fs_mutate("fs_rename_at", &grant, &[0], &[b"final".to_vec()], true).expect("rename");
        assert_eq!(effect.target, "entry_rename");
        let (seen, _) =
            fs_realize("fs_read_bytes_at", &grant, &[u64_of(&moved), 0, 64]).expect("read back");
        match seen {
            ExecutionValue::Sequence { values } => {
                let bytes: Vec<u8> = values
                    .iter()
                    .map(|value| match value {
                        ExecutionValue::Byte { value } => *value as u8,
                        other => panic!("expected byte, got {other:?}"),
                    })
                    .collect();
                assert_eq!(bytes, vec![9, 2, 3, 4, 5, 6]);
            }
            other => panic!("expected sequence, got {other:?}"),
        }
        // Barrier receipt, then retire the file (kind 0).
        let (sealed, effect) =
            fs_mutate("fs_sync_at", &grant, &[u64_of(&moved)], &[], true).expect("sync");
        assert_eq!(u64_of(&sealed), 1);
        assert_eq!(effect.target, "file_sync");
        let (removed, _) =
            fs_mutate("fs_delete_at", &grant, &[u64_of(&moved)], &[], true).expect("delete");
        assert_eq!(u64_of(&removed), 0);
        assert!(!root.join("final").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Intent-only realization (Record policy) validates and values the
    /// call but never touches the tree.
    #[test]
    fn intent_only_validates_without_touching_the_tree() {
        let root = test_root("intent");
        let grant = grant_for(&root);
        let (created, _) = fs_mutate(
            "fs_create_file",
            &grant,
            &[],
            &[b"staged".to_vec(), vec![7, 8]],
            false,
        )
        .expect("intent create");
        assert_eq!(u64_of(&created), 0);
        assert!(!root.join("staged").exists(), "intent must not create");
        // The tree is still empty, so a wild index still refuses even as
        // intent: validation is not skipped.
        let refused = fs_mutate("fs_delete_at", &grant, &[3], &[], false);
        assert!(
            matches!(refused, Err(FsFail::InvalidRequest(_))),
            "wild index refuses as intent: {refused:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The refusal matrix: every ill-formed mutation is InvalidRequest
    /// (caller's data is wrong) and leaves no trace; missing post-state
    /// is RuntimeFailure, never a fabricated value.
    #[test]
    fn refusal_matrix_leaves_no_trace() {
        let root = test_root("refusals");
        let grant = grant_for(&root);
        let (created, _) = fs_mutate(
            "fs_create_file",
            &grant,
            &[],
            &[b"victim".to_vec(), vec![1]],
            true,
        )
        .expect("setup file");
        assert_eq!(u64_of(&created), 0);
        // Double create refuses; original bytes intact.
        let refused = fs_mutate(
            "fs_create_file",
            &grant,
            &[],
            &[b"victim".to_vec(), vec![2]],
            true,
        );
        assert!(
            matches!(refused, Err(FsFail::InvalidRequest(_))),
            "{refused:?}"
        );
        assert_eq!(std::fs::read(root.join("victim")).expect("read"), vec![1]);
        // Path-shaped, empty, dot, and dot-dot names refuse.
        for bad in [&b"a/b"[..], &b""[..], &b"."[..], &b".."[..]] {
            let refused = fs_mutate("fs_mkdir", &grant, &[], &[bad.to_vec()], true);
            assert!(
                matches!(refused, Err(FsFail::InvalidRequest(_))),
                "{bad:?}: {refused:?}"
            );
        }
        assert!(!root.join("a").exists() && !root.join("b").exists());
        // Sparse-gap write refuses; file intact.
        let refused = fs_mutate("fs_write_bytes_at", &grant, &[0, 99], &[vec![1]], true);
        assert!(
            matches!(refused, Err(FsFail::InvalidRequest(_))),
            "{refused:?}"
        );
        assert_eq!(std::fs::read(root.join("victim")).expect("read"), vec![1]);
        // Wild index refuses on every indexed mutation.
        for operation in [
            "fs_write_bytes_at",
            "fs_append_bytes_at",
            "fs_delete_at",
            "fs_rename_at",
            "fs_sync_at",
        ] {
            let ints: Vec<u64> = match operation {
                "fs_write_bytes_at" => vec![7, 0],
                _ => vec![7],
            };
            let views: Vec<Vec<u8>> = match operation {
                "fs_write_bytes_at" | "fs_append_bytes_at" => vec![vec![1]],
                "fs_rename_at" => vec![b"elsewhere".to_vec()],
                _ => Vec::new(),
            };
            let refused = fs_mutate(operation, &grant, &ints, &views, true);
            assert!(
                matches!(refused, Err(FsFail::InvalidRequest(_))),
                "{operation}: {refused:?}"
            );
        }
        // The barrier covers files only; directories refuse. ("d" sorts
        // before "victim", so the new directory is index 0.)
        let (dir, _) = fs_mutate("fs_mkdir", &grant, &[], &[b"d".to_vec()], true).expect("mkdir");
        assert_eq!(u64_of(&dir), 0);
        let refused = fs_mutate("fs_sync_at", &grant, &[u64_of(&dir)], &[], true);
        assert!(
            matches!(refused, Err(FsFail::InvalidRequest(_))),
            "{refused:?}"
        );
        // Non-empty directories refuse deletion; empty ones report kind 1.
        std::fs::write(root.join("d").join("inner"), b"x").expect("fixture");
        let refused = fs_mutate("fs_delete_at", &grant, &[u64_of(&dir)], &[], true);
        assert!(
            matches!(refused, Err(FsFail::InvalidRequest(_))),
            "{refused:?}"
        );
        assert!(root.join("d").join("inner").exists());
        std::fs::remove_file(root.join("d").join("inner")).expect("cleanup");
        let (removed, _) =
            fs_mutate("fs_delete_at", &grant, &[u64_of(&dir)], &[], true).expect("rmdir");
        assert_eq!(u64_of(&removed), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Symlinks are listed as `other` and never touched: delete and
    /// rename refuse, and reads already refuse through non-files.
    #[test]
    #[cfg(unix)]
    fn symlinks_are_never_touched() {
        let root = test_root("symlink");
        let grant = grant_for(&root);
        std::fs::write(root.join("real"), b"r").expect("fixture");
        std::os::unix::fs::symlink("real", root.join("link")).expect("symlink");
        let snapshot = fs_snapshot(root.as_path()).expect("snapshot");
        let link_index = snapshot
            .entries
            .iter()
            .position(|entry| entry.rel == b"link")
            .expect("link is listed") as u64;
        assert_eq!(snapshot.entries[link_index as usize].kind, FS_KIND_OTHER);
        let refused = fs_mutate("fs_delete_at", &grant, &[link_index], &[], true);
        assert!(
            matches!(refused, Err(FsFail::InvalidRequest(_))),
            "{refused:?}"
        );
        let refused = fs_mutate(
            "fs_rename_at",
            &grant,
            &[link_index],
            &[b"moved".to_vec()],
            true,
        );
        assert!(
            matches!(refused, Err(FsFail::InvalidRequest(_))),
            "{refused:?}"
        );
        assert!(root.join("link").is_symlink(), "link untouched");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Renaming onto a directory never replaces it; renaming onto a
    /// file atomically replaces the destination content.
    #[test]
    fn rename_replace_covers_files_never_directories() {
        let root = test_root("rename-replace");
        let grant = grant_for(&root);
        fs_mutate(
            "fs_create_file",
            &grant,
            &[],
            &[b"a".to_vec(), vec![1]],
            true,
        )
        .expect("a");
        fs_mutate(
            "fs_create_file",
            &grant,
            &[],
            &[b"b".to_vec(), vec![2]],
            true,
        )
        .expect("b");
        let (dir, _) = fs_mutate("fs_mkdir", &grant, &[], &[b"d".to_vec()], true).expect("d");
        assert_eq!(u64_of(&dir), 2);
        // a -> b replaces the file destination.
        let snapshot = fs_snapshot(root.as_path()).expect("snapshot");
        let a_index = snapshot
            .entries
            .iter()
            .position(|entry| entry.rel == b"a")
            .expect("a listed") as u64;
        fs_mutate("fs_rename_at", &grant, &[a_index], &[b"b".to_vec()], true).expect("replace");
        assert_eq!(std::fs::read(root.join("b")).expect("read"), vec![1]);
        assert!(!root.join("a").exists());
        // b -> d refuses: directories are never replaced.
        let snapshot = fs_snapshot(root.as_path()).expect("snapshot");
        let b_index = snapshot
            .entries
            .iter()
            .position(|entry| entry.rel == b"b")
            .expect("b listed") as u64;
        let refused = fs_mutate("fs_rename_at", &grant, &[b_index], &[b"d".to_vec()], true);
        assert!(
            matches!(refused, Err(FsFail::InvalidRequest(_))),
            "{refused:?}"
        );
        assert!(root.join("d").is_dir(), "directory untouched");
        let _ = std::fs::remove_dir_all(&root);
    }
}
