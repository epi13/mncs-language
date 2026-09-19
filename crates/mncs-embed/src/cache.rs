//! Content-addressed storage for compiled native application artifacts.
//!
//! The cache is deliberately an admission layer, not a semantic authority.
//! A caller supplies all identities that can affect compilation or admission;
//! the cache only returns an artifact after both the cache record and the
//! embedded artifact identity validate.  Missing entries are ordinary cache
//! misses.  Present-but-invalid entries are errors so corruption or a stale
//! record can never silently turn into execution of an unchecked artifact.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
}

/// A local content-addressed cache of verified backend artifacts.
#[derive(Debug, Clone)]
pub struct CompiledArtifactCache {
    root: PathBuf,
}

impl CompiledArtifactCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn load(&self, key: &CompiledArtifactCacheKey) -> Result<Option<Artifact>, EmbedError> {
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
        {
            return Err(EmbedError::new(
                "cache_stale",
                "compiled artifact cache payload identity does not match its record",
            ));
        }
        Ok(Some(artifact))
    }

    pub fn store(
        &self,
        key: &CompiledArtifactCacheKey,
        artifact: &Artifact,
    ) -> Result<(), EmbedError> {
        let key_identity = key.identity()?;
        let directory = self.entry_directory(&key_identity);
        fs::create_dir_all(&directory).map_err(|error| {
            EmbedError::new(
                "cache_unavailable",
                format!("compiled artifact cache directory could not be created: {error}"),
            )
        })?;
        let artifact_bytes = artifact.to_json_bytes();
        let record = CacheRecord {
            schema_version: COMPILED_ARTIFACT_CACHE_SCHEMA_VERSION.to_owned(),
            key_identity,
            key: key.clone(),
            artifact_identity: artifact.artifact_identity().to_owned(),
            artifact_sha256: artifact.digest().to_owned(),
        };
        let record_bytes = serde_json::to_vec_pretty(&record).map_err(|error| {
            EmbedError::new(
                "cache_key_invalid",
                format!("compiled artifact cache record serialization failed: {error}"),
            )
        })?;
        atomic_write(&directory.join("artifact.json"), &artifact_bytes)?;
        atomic_write(&directory.join("record.json"), &record_bytes)?;
        Ok(())
    }

    fn entry_directory(&self, key_identity: &str) -> PathBuf {
        let safe_identity = key_identity.strip_prefix("sha256:").unwrap_or(key_identity);
        self.root.join(safe_identity)
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), EmbedError> {
    let parent = path.parent().ok_or_else(|| {
        EmbedError::new(
            "cache_unavailable",
            "compiled artifact cache path has no parent",
        )
    })?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        nonce
    ));
    fs::write(&temporary, bytes).map_err(|error| {
        EmbedError::new(
            "cache_unavailable",
            format!("compiled artifact cache temporary write failed: {error}"),
        )
    })?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        EmbedError::new(
            "cache_unavailable",
            format!("compiled artifact cache publish failed: {error}"),
        )
    })
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
