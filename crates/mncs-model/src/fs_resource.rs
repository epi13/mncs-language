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
