//! Content-addressed storage for compiled native application artifacts.
//!
//! The cache is deliberately an admission layer, not a semantic authority.
//! A caller supplies all identities that can affect compilation or admission;
//! the cache only returns an artifact after both the cache record and the
//! embedded artifact identity validate.  Missing entries are ordinary cache
//! misses.  Present-but-invalid entries are errors so corruption or a stale
//! record can never silently turn into execution of an unchecked artifact.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mncs_model::sha256_hex;
use serde::{Deserialize, Serialize};

use crate::{Artifact, EmbedError};

pub const COMPILED_ARTIFACT_CACHE_SCHEMA_VERSION: &str = "mncs.compiled-native-artifact-cache/1";

/// Every semantic/compiler input that can affect a compiled application is
/// represented here.  The key is content-addressed after serialization, so
/// changing any field deterministically invalidates the old entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledArtifactCacheKey {
    pub schema_version: String,
    pub source_identity: String,
    pub source_locator: String,
    pub library_identity: String,
    pub compiler_identity: String,
    pub compiler_inventory_identity: String,
    pub language_profile_identity: String,
    pub backend_identity: String,
    pub grant_identity: String,
    #[serde(default)]
    pub interface_identity: Option<String>,
}

impl CompiledArtifactCacheKey {
    pub fn new(
        source_identity: impl Into<String>,
        source_locator: impl Into<String>,
        library_identity: impl Into<String>,
        compiler_identity: impl Into<String>,
        compiler_inventory_identity: impl Into<String>,
        language_profile_identity: impl Into<String>,
        backend_identity: impl Into<String>,
        grant_identity: impl Into<String>,
        interface_identity: Option<String>,
    ) -> Self {
        Self {
            schema_version: COMPILED_ARTIFACT_CACHE_SCHEMA_VERSION.to_owned(),
            source_identity: source_identity.into(),
            source_locator: source_locator.into(),
            library_identity: library_identity.into(),
            compiler_identity: compiler_identity.into(),
            compiler_inventory_identity: compiler_inventory_identity.into(),
            language_profile_identity: language_profile_identity.into(),
            backend_identity: backend_identity.into(),
            grant_identity: grant_identity.into(),
            interface_identity,
        }
    }

    pub fn identity(&self) -> Result<String, EmbedError> {
        let bytes = serde_json::to_vec(self).map_err(|error| {
            EmbedError::new(
                "cache_key_invalid",
                format!("cache key serialization failed: {error}"),
            )
        })?;
        Ok(format!("sha256:{}", sha256_hex(&bytes)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheRecord {
    schema_version: String,
    key_identity: String,
    key: CompiledArtifactCacheKey,
    artifact_identity: String,
    artifact_sha256: String,
    /// Hash the complete envelope, not only the embedded executable bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    payload_sha256: Option<String>,
}

/// Limits apply to recognized cache entries; unrelated files are preserved.
#[derive(Debug, Clone)]
pub struct CacheLimits {
    pub max_bytes: u64,
    pub max_entries: usize,
    pub max_age: Duration,
}

impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            max_bytes: 2 * 1024 * 1024 * 1024,
            max_entries: 128,
            max_age: Duration::from_secs(14 * 24 * 60 * 60),
        }
    }
}

impl CacheLimits {
    fn from_environment() -> Self {
        let defaults = Self::default();
        let number = |name: &str, fallback: u64| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(fallback)
        };
        Self {
            max_bytes: number(
                "MNCS_NATIVE_APPLICATION_CACHE_MAX_BYTES",
                defaults.max_bytes,
            ),
            max_entries: number(
                "MNCS_NATIVE_APPLICATION_CACHE_MAX_ENTRIES",
                defaults.max_entries as u64,
            )
            .min(usize::MAX as u64) as usize,
            max_age: Duration::from_secs(number(
                "MNCS_NATIVE_APPLICATION_CACHE_MAX_AGE_SECONDS",
                defaults.max_age.as_secs(),
            )),
        }
    }
}

struct ManagedEntry {
    directory: PathBuf,
    record: CacheRecord,
    payload_id: String,
    payload_bytes: u64,
    record_bytes: u64,
    used_at: SystemTime,
}

struct PendingDirectory(PathBuf);

impl Drop for PendingDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A local content-addressed cache of verified backend artifacts.
#[derive(Debug, Clone)]
pub struct CompiledArtifactCache {
    root: PathBuf,
    storage_root: PathBuf,
    limits: CacheLimits,
}

impl CompiledArtifactCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let cache = Self::with_limits(root, CacheLimits::from_environment());
        if let Some(storage) = std::env::var_os("MNCS_NATIVE_APPLICATION_CACHE_STORAGE_ROOT") {
            if cache.valid_storage_scope(Path::new(&storage)) {
                return Self {
                    storage_root: PathBuf::from(storage),
                    ..cache
                };
            }
        }
        cache
    }

    pub fn with_limits(root: impl Into<PathBuf>, limits: CacheLimits) -> Self {
        let root = root.into();
        Self {
            storage_root: root.clone(),
            root,
            limits,
        }
    }

    fn valid_storage_scope(&self, storage: &Path) -> bool {
        self.root.parent() == Some(storage)
            && self
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(namespace_identity)
    }

    fn storage_directories(&self) -> Result<Vec<PathBuf>, EmbedError> {
        if self.storage_root == self.root {
            return Ok(vec![self.root.clone()]);
        }
        let mut directories = Vec::new();
        for child in fs::read_dir(&self.storage_root).map_err(unavailable)? {
            let child = child.map_err(unavailable)?;
            if child.file_name().to_str().is_some_and(namespace_identity)
                && child.file_type().map_err(unavailable)?.is_dir()
            {
                directories.push(child.path());
            }
        }
        Ok(directories)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn load(&self, key: &CompiledArtifactCacheKey) -> Result<Option<Artifact>, EmbedError> {
        if !self.root.exists() {
            return Ok(None);
        }
        // Readers retain the lock through payload validation. Eviction can
        // never remove a record between its lookup and payload read.
        let _lock = self.lock(false)?;
        self.load_unlocked(key)
    }

    fn load_unlocked(
        &self,
        key: &CompiledArtifactCacheKey,
    ) -> Result<Option<Artifact>, EmbedError> {
        let key_identity = key.identity()?;
        let directory = self.entry_directory(&key_identity);
        let record_path = directory.join("record.json");
        if !record_path.exists() {
            return Ok(None);
        }
        let record_bytes = fs::read(&record_path).map_err(|error| {
            EmbedError::new(
                "cache_unavailable",
                format!("compiled artifact cache record could not be read: {error}"),
            )
        })?;
        let record: CacheRecord = serde_json::from_slice(&record_bytes).map_err(|error| {
            EmbedError::new(
                "cache_corrupt",
                format!("compiled artifact cache record is invalid JSON: {error}"),
            )
        })?;
        if record.schema_version != COMPILED_ARTIFACT_CACHE_SCHEMA_VERSION
            || record.key_identity != key_identity
            || record.key != *key
        {
            return Err(EmbedError::new(
                "cache_stale",
                "compiled artifact cache record does not match the requested identities",
            ));
        }
        let artifact_path = directory.join("artifact.json");
        let artifact_bytes = fs::read(&artifact_path).map_err(|error| {
            EmbedError::new(
                "cache_corrupt",
                format!("compiled artifact cache payload could not be read: {error}"),
            )
        })?;
        let artifact = Artifact::from_json(&artifact_bytes)?;
        if artifact.artifact_identity() != record.artifact_identity
            || artifact.digest() != record.artifact_sha256
            || record
                .payload_sha256
                .as_ref()
                .is_some_and(|digest| *digest != sha256_hex(&artifact_bytes))
        {
            return Err(EmbedError::new(
                "cache_stale",
                "compiled artifact cache payload identity does not match its record",
            ));
        }
        // Access time is advisory LRU metadata, never artifact authority.
        if let Ok(file) = OpenOptions::new().write(true).open(record_path) {
            let _ = file.set_modified(SystemTime::now());
        }
        Ok(Some(artifact))
    }

    pub fn store(
        &self,
        key: &CompiledArtifactCacheKey,
        artifact: &Artifact,
    ) -> Result<(), EmbedError> {
        let _lock = self.lock(true)?;
        self.remove_abandoned_publications()?;
        self.prune()?;
        let key_identity = key.identity()?;
        let directory = self.entry_directory(&key_identity);
        let artifact_bytes = artifact.to_json_bytes();
        let payload_digest = sha256_hex(&artifact_bytes);
        let record = CacheRecord {
            schema_version: COMPILED_ARTIFACT_CACHE_SCHEMA_VERSION.to_owned(),
            key_identity: key_identity.clone(),
            key: key.clone(),
            artifact_identity: artifact.artifact_identity().to_owned(),
            artifact_sha256: artifact.digest().to_owned(),
            payload_sha256: Some(payload_digest.clone()),
        };
        let record_bytes = serde_json::to_vec_pretty(&record).map_err(|error| {
            EmbedError::new(
                "cache_key_invalid",
                format!("compiled artifact cache record serialization failed: {error}"),
            )
        })?;
        // An oversized artifact can still execute; it simply stays uncached.
        if self.limits.max_entries == 0
            || artifact_bytes.len() as u64 + record_bytes.len() as u64 > self.limits.max_bytes
        {
            return Ok(());
        }
        if directory.exists() {
            let existing = self.load_unlocked(key)?;
            if existing.is_some_and(|value| value.to_json_bytes() == artifact_bytes) {
                return self.prune();
            }
            return Err(EmbedError::new(
                "cache_conflict",
                "an existing cache key has a different payload",
            ));
        }

        let pending = PendingDirectory(self.root.join(format!(
            ".pending-{}-{}",
            key_identity.trim_start_matches("sha256:"),
            nonce(),
        )));
        fs::create_dir(&pending.0).map_err(unavailable)?;
        let payload_path = pending.0.join("artifact.json");
        let mut linked = false;
        for entry in self.entries()? {
            let possible_match = entry.record.payload_sha256.as_ref() == Some(&payload_digest)
                || (entry.record.payload_sha256.is_none()
                    && entry.record.artifact_identity == record.artifact_identity
                    && entry.record.artifact_sha256 == record.artifact_sha256);
            if possible_match {
                let candidate = entry.directory.join("artifact.json");
                // Preserve every admission key while storing identical complete
                // envelopes once. Matching embedded bytecode alone is insufficient.
                if fs::read(&candidate).map_err(unavailable)? == artifact_bytes {
                    linked = fs::hard_link(candidate, &payload_path).is_ok();
                    break;
                }
            }
        }
        if !linked {
            fs::write(&payload_path, &artifact_bytes).map_err(unavailable)?;
        }
        fs::write(pending.0.join("record.json"), &record_bytes).map_err(unavailable)?;
        // Publish the complete record/payload pair in one rename. A killed
        // writer leaves only an unaddressable pending directory, reaped by
        // the next writer after acquiring the process-scoped file lock.
        fs::rename(&pending.0, directory).map_err(unavailable)?;
        self.prune()
    }

    fn entry_directory(&self, key_identity: &str) -> PathBuf {
        let safe_identity = key_identity.strip_prefix("sha256:").unwrap_or(key_identity);
        self.root.join(safe_identity)
    }

    fn lock(&self, exclusive: bool) -> Result<File, EmbedError> {
        fs::create_dir_all(&self.root).map_err(unavailable)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.storage_root.join(".cache.lock"))
            .map_err(unavailable)?;
        if exclusive {
            file.lock()
        } else {
            file.lock_shared()
        }
        .map_err(unavailable)?;
        Ok(file)
    }

    fn entries(&self) -> Result<Vec<ManagedEntry>, EmbedError> {
        let mut entries = Vec::new();
        for namespace in self.storage_directories()? {
            for child in fs::read_dir(namespace).map_err(unavailable)? {
                let child = child.map_err(unavailable)?;
                let name = child.file_name().to_string_lossy().into_owned();
                if !hex_identity(&name) || !child.file_type().map_err(unavailable)?.is_dir() {
                    continue;
                }
                let directory = child.path();
                if !only_cache_files(&directory)? {
                    continue;
                }
                let record_path = directory.join("record.json");
                let Ok(record_bytes) = fs::read(&record_path) else {
                    continue;
                };
                let Ok(record) = serde_json::from_slice::<CacheRecord>(&record_bytes) else {
                    continue;
                };
                if record.schema_version != COMPILED_ARTIFACT_CACHE_SCHEMA_VERSION
                    || record.key_identity != format!("sha256:{name}")
                    || record.key.identity().ok().as_ref() != Some(&record.key_identity)
                {
                    continue;
                }
                let payload = directory.join("artifact.json");
                let Ok(metadata) = fs::symlink_metadata(&payload) else {
                    continue;
                };
                if !metadata.file_type().is_file() {
                    continue;
                }
                entries.push(ManagedEntry {
                    directory,
                    record,
                    payload_id: payload_id(&payload, &metadata),
                    payload_bytes: metadata.len(),
                    record_bytes: record_bytes.len() as u64,
                    used_at: fs::metadata(record_path)
                        .and_then(|value| value.modified())
                        .unwrap_or(UNIX_EPOCH),
                });
            }
        }
        Ok(entries)
    }

    fn prune(&self) -> Result<(), EmbedError> {
        let mut entries = self.entries()?;
        entries.sort_by(|a, b| {
            a.used_at
                .cmp(&b.used_at)
                .then(a.directory.cmp(&b.directory))
        });
        while let Some(oldest) = entries.first() {
            let expired = SystemTime::now()
                .duration_since(oldest.used_at)
                .unwrap_or_default()
                > self.limits.max_age;
            if !expired
                && entries.len() <= self.limits.max_entries
                && cache_bytes(&entries) <= self.limits.max_bytes
            {
                break;
            }
            fs::remove_dir_all(&oldest.directory).map_err(unavailable)?;
            entries.remove(0);
        }
        Ok(())
    }

    fn remove_abandoned_publications(&self) -> Result<(), EmbedError> {
        for namespace in self.storage_directories()? {
            for child in fs::read_dir(namespace).map_err(unavailable)? {
                let child = child.map_err(unavailable)?;
                let name = child.file_name().to_string_lossy().into_owned();
                let Some((identity, suffix)) = name
                    .strip_prefix(".pending-")
                    .and_then(|value| value.split_once('-'))
                else {
                    continue;
                };
                if hex_identity(identity)
                    && suffix.parse::<u128>().is_ok()
                    && child.file_type().map_err(unavailable)?.is_dir()
                    && only_cache_files(&child.path())?
                {
                    fs::remove_dir_all(child.path()).map_err(unavailable)?;
                }
            }
        }
        Ok(())
    }
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn unavailable(error: std::io::Error) -> EmbedError {
    EmbedError::new(
        "cache_unavailable",
        format!("compiled artifact cache filesystem operation failed: {error}"),
    )
}

fn hex_identity(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|value| value.is_ascii_hexdigit() && !value.is_ascii_uppercase())
}

fn namespace_identity(name: &str) -> bool {
    name.len() == 16
        && name
            .bytes()
            .all(|value| value.is_ascii_hexdigit() && !value.is_ascii_uppercase())
}

fn only_cache_files(directory: &Path) -> Result<bool, EmbedError> {
    for child in fs::read_dir(directory).map_err(unavailable)? {
        let child = child.map_err(unavailable)?;
        if !child.file_type().map_err(unavailable)?.is_file()
            || !matches!(
                child.file_name().to_str(),
                Some("artifact.json" | "record.json")
            )
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(unix)]
fn payload_id(_path: &Path, metadata: &fs::Metadata) -> String {
    use std::os::unix::fs::MetadataExt;
    format!("{}:{}", metadata.dev(), metadata.ino())
}

#[cfg(not(unix))]
fn payload_id(path: &Path, _metadata: &fs::Metadata) -> String {
    // Conservative accounting where stable file IDs are unavailable.
    path.to_string_lossy().into_owned()
}

fn cache_bytes(entries: &[ManagedEntry]) -> u64 {
    let mut payloads = HashSet::new();
    entries
        .iter()
        .map(|entry| {
            entry.record_bytes
                + if payloads.insert(&entry.payload_id) {
                    entry.payload_bytes
                } else {
                    0
                }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn key() -> CompiledArtifactCacheKey {
        CompiledArtifactCacheKey::new(
            "sha256:source",
            "/workspace/app.mncs",
            "sha256:libs",
            "sha256:compiler",
            "sha256:inventory",
            "0.18",
            "mncs-research-bytecode/0.1",
            "sha256:grants",
            Some("sha256:interface".to_owned()),
        )
    }

    fn test_cache() -> CompiledArtifactCache {
        CompiledArtifactCache::with_limits(
            std::env::temp_dir().join(format!(
                "mncs-cache-lifecycle-{}-{}",
                std::process::id(),
                nonce()
            )),
            CacheLimits::default(),
        )
    }

    fn artifact(value: i64) -> Artifact {
        Artifact::from_source(
            &format!("mncs 0.6;\nmodule cache.lifecycle;\nfn value() -> (result: i64) {{ return {value}; }}\n"),
            "mncs-research-bytecode",
        ).expect("compile artifact")
    }

    fn different_key(label: &str) -> CompiledArtifactCacheKey {
        let mut value = key();
        value.library_identity = label.to_owned();
        value
    }

    fn set_used_at(
        cache: &CompiledArtifactCache,
        key: &CompiledArtifactCacheKey,
        seconds_ago: u64,
    ) {
        let path = cache
            .entry_directory(&key.identity().unwrap())
            .join("record.json");
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(seconds_ago))
            .unwrap();
    }

    #[test]
    fn distinct_admission_keys_share_identical_complete_payloads() {
        let cache = test_cache();
        let artifact = artifact(7);
        let first = key();
        let second = different_key("changed-library");
        cache.store(&first, &artifact).unwrap();
        cache.store(&second, &artifact).unwrap();
        assert_eq!(
            cache.load(&first).unwrap().unwrap().artifact_identity(),
            artifact.artifact_identity()
        );
        assert_eq!(
            cache.load(&second).unwrap().unwrap().artifact_identity(),
            artifact.artifact_identity()
        );
        let entries = cache.entries().unwrap();
        assert_eq!(entries.len(), 2);
        #[cfg(unix)]
        assert_eq!(entries[0].payload_id, entries[1].payload_id);
        let full_digest = sha256_hex(&artifact.to_json_bytes());
        assert!(entries
            .iter()
            .all(|entry| entry.record.payload_sha256.as_ref() == Some(&full_digest)));
        fs::remove_dir_all(cache.root()).unwrap();
    }

    #[test]
    fn toolchain_namespaces_share_one_storage_budget_and_payload_pool() {
        let first = test_cache();
        let storage = first.root.clone();
        let mut first = CompiledArtifactCache::with_limits(
            storage.join("a".repeat(16)),
            CacheLimits::default(),
        );
        assert!(first.valid_storage_scope(&storage));
        assert!(!first.valid_storage_scope(Path::new("/unrelated")));
        first.storage_root = storage.clone();
        first.limits.max_entries = 2;
        let mut second = first.clone();
        second.root = storage.join("b".repeat(16));
        let artifact = artifact(7);
        first.store(&key(), &artifact).unwrap();
        set_used_at(&first, &key(), 100);
        second.store(&key(), &artifact).unwrap();
        let entries = first.entries().unwrap();
        assert_eq!(entries.len(), 2);
        #[cfg(unix)]
        assert_eq!(entries[0].payload_id, entries[1].payload_id);
        second.store(&different_key("new"), &artifact).unwrap();
        assert!(first.load(&key()).unwrap().is_none());
        assert!(second.load(&key()).unwrap().is_some());
        assert_eq!(second.entries().unwrap().len(), 2);
        fs::remove_dir_all(storage).unwrap();
    }

    #[test]
    fn recent_reads_protect_entries_from_lru_eviction() {
        let mut cache = test_cache();
        cache.limits.max_entries = 2;
        let artifact = artifact(7);
        let (first, second, third) = (key(), different_key("b"), different_key("c"));
        cache.store(&first, &artifact).unwrap();
        cache.store(&second, &artifact).unwrap();
        set_used_at(&cache, &first, 100);
        set_used_at(&cache, &second, 50);
        assert!(cache.load(&first).unwrap().is_some());
        cache.store(&third, &artifact).unwrap();
        assert!(cache.load(&first).unwrap().is_some());
        assert!(cache.load(&second).unwrap().is_none());
        assert!(cache.load(&third).unwrap().is_some());
        fs::remove_dir_all(cache.root()).unwrap();
    }

    #[test]
    fn byte_and_age_limits_remove_old_generated_entries() {
        let mut cache = test_cache();
        let (first, second, third) = (key(), different_key("b"), different_key("c"));
        cache.store(&first, &artifact(1)).unwrap();
        let entry_bytes = cache_bytes(&cache.entries().unwrap());
        cache.limits.max_bytes = entry_bytes * 2;
        set_used_at(&cache, &first, 100);
        cache.store(&second, &artifact(2)).unwrap();
        set_used_at(&cache, &second, 50);
        cache.store(&third, &artifact(3)).unwrap();
        assert!(cache_bytes(&cache.entries().unwrap()) <= cache.limits.max_bytes);
        assert!(cache.load(&first).unwrap().is_none());
        set_used_at(&cache, &third, cache.limits.max_age.as_secs() + 1);
        cache.store(&different_key("d"), &artifact(4)).unwrap();
        assert!(cache.load(&third).unwrap().is_none());
        fs::remove_dir_all(cache.root()).unwrap();
    }

    #[test]
    fn oversized_artifacts_execute_without_becoming_persistent_cache_entries() {
        let mut cache = test_cache();
        cache.limits.max_bytes = 1;
        cache.store(&key(), &artifact(7)).unwrap();
        assert!(cache.load(&key()).unwrap().is_none());
        assert!(cache.entries().unwrap().is_empty());
        fs::remove_dir_all(cache.root()).unwrap();
    }

    #[test]
    fn eviction_preserves_unrecognized_files_and_reaps_interrupted_publications() {
        let mut cache = test_cache();
        fs::create_dir_all(cache.root().join("user-notes")).unwrap();
        fs::write(cache.root().join("user-notes/keep"), b"durable evidence").unwrap();
        let pending = cache.root().join(format!(".pending-{}-1", "a".repeat(64)));
        fs::create_dir(&pending).unwrap();
        fs::write(pending.join("artifact.json"), b"partial publication").unwrap();
        cache.store(&key(), &artifact(7)).unwrap();
        let protected = cache.entry_directory(&key().identity().unwrap());
        fs::write(protected.join("keep-notes"), b"unknown local output").unwrap();
        cache.limits.max_entries = 0;
        cache.store(&different_key("b"), &artifact(7)).unwrap();
        assert!(!pending.exists());
        assert!(protected.join("keep-notes").exists());
        assert!(cache.root().join("user-notes/keep").exists());
        fs::remove_dir_all(cache.root()).unwrap();
    }

    #[test]
    fn existing_cache_records_remain_readable_and_can_supply_deduplicated_payloads() {
        let cache = test_cache();
        let artifact = artifact(7);
        cache.store(&key(), &artifact).unwrap();
        let record_path = cache
            .entry_directory(&key().identity().unwrap())
            .join("record.json");
        let mut old: serde_json::Value =
            serde_json::from_slice(&fs::read(&record_path).unwrap()).unwrap();
        old.as_object_mut().unwrap().remove("payload_sha256");
        fs::write(record_path, serde_json::to_vec(&old).unwrap()).unwrap();
        assert!(cache.load(&key()).unwrap().is_some());
        cache.store(&different_key("b"), &artifact).unwrap();
        let entries = cache.entries().unwrap();
        #[cfg(unix)]
        assert_eq!(entries[0].payload_id, entries[1].payload_id);
        fs::remove_dir_all(cache.root()).unwrap();
    }

    #[test]
    fn concurrent_writers_and_readers_never_admit_partial_or_evicted_payloads() {
        let mut cache = test_cache();
        cache.limits.max_entries = 4;
        let threads: Vec<_> = (0..6)
            .map(|worker| {
                let cache = cache.clone();
                std::thread::spawn(move || {
                    let artifact = artifact(7);
                    for iteration in 0..4 {
                        let key = different_key(&format!("worker-{worker}-{iteration}"));
                        cache.store(&key, &artifact).unwrap();
                        if let Some(loaded) = cache.load(&key).unwrap() {
                            assert_eq!(loaded.artifact_identity(), artifact.artifact_identity());
                        }
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert!(cache.entries().unwrap().len() <= 4);
        assert!(fs::read_dir(cache.root()).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".pending-")));
        fs::remove_dir_all(cache.root()).unwrap();
    }

    #[test]
    fn key_identity_changes_with_semantic_inputs() {
        let first = key().identity().expect("identity");
        let mut changed = key();
        changed.compiler_inventory_identity = "sha256:changed".to_owned();
        assert_ne!(first, changed.identity().expect("identity"));
    }

    #[test]
    fn absent_entry_is_a_miss() {
        let root = std::env::temp_dir().join(format!(
            "mncs-cache-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let cache = CompiledArtifactCache::new(&root);
        assert!(cache.load(&key()).expect("cache read").is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn present_corrupt_entry_fails_closed() {
        let root = std::env::temp_dir().join(format!(
            "mncs-cache-corrupt-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let cache = CompiledArtifactCache::new(&root);
        let key = key();
        let identity = key.identity().expect("identity");
        let directory = root.join(identity.strip_prefix("sha256:").unwrap());
        fs::create_dir_all(&directory).expect("cache directory");
        fs::write(directory.join("record.json"), b"{}\n").expect("corrupt record");
        match cache.load(&key) {
            Err(error) => assert_eq!(error.code, "cache_corrupt"),
            Ok(_) => panic!("corrupt entries must fail closed"),
        }
        let _ = fs::remove_dir_all(root);
    }
}
