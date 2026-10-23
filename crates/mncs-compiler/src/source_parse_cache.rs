//! Bounded, Language-owned reuse of source parse results.
//!
//! The cache is caller-owned so a build service can persist it between
//! processes. Entries are reusable for the same parser inputs, parser policy,
//! source-profile registry, and syntax artifact schemas. Source-envelope
//! identity is rebound on reuse because origin, relationships, provenance,
//! and logical names do not affect parsing and source spans remain byte based.

use mncs_syntax::{
    source_profile_registry_snapshot, ParseOutput, SourceArtifactKind, SourceEnvelope,
    AST_SCHEMA_VERSION, CST_SCHEMA_VERSION, LEXICAL_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Write};

pub const SOURCE_PARSE_CACHE_SCHEMA_VERSION: &str = "mncs.language-source-parse-cache/2";
/// Bump this identity whenever parser behavior changes without a syntax
/// artifact schema change.
pub const SOURCE_PARSE_POLICY_IDENTITY: &str = "mncs-language-source-parser/1";
pub const SOURCE_PARSE_CACHE_OBSERVATION_SCHEMA_VERSION: &str =
    "mncs.language-source-parse-cache-observation/2";

const MAX_CACHE_ENTRIES: usize = 256;
const MAX_CACHE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceParseCache {
    #[serde(default = "cache_schema_version")]
    schema_version: String,
    #[serde(default)]
    entries: Vec<SourceParseCacheEntry>,
    /// Serialized entry sizes are derived bookkeeping. Rebuild them once when
    /// a caller restores the durable cache, then maintain them alongside LRU
    /// insertions and removals instead of serializing every entry on each
    /// source lookup.
    #[serde(skip)]
    entry_sizes: Vec<usize>,
}

impl PartialEq for SourceParseCache {
    fn eq(&self, other: &Self) -> bool {
        self.schema_version == other.schema_version && self.entries == other.entries
    }
}

impl Eq for SourceParseCache {}

impl Default for SourceParseCache {
    fn default() -> Self {
        Self {
            schema_version: cache_schema_version(),
            entries: Vec::new(),
            entry_sizes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SourceParseCacheEntry {
    key: String,
    source_identity: String,
    parser_policy_identity: String,
    profile_registry_identity: String,
    lexical_schema_version: String,
    cst_schema_version: String,
    ast_schema_version: String,
    output_digest: String,
    output: ParseOutput,
}

#[derive(Serialize)]
struct SourceParseCacheEntryRef<'a> {
    key: &'a str,
    source_identity: &'a str,
    parser_policy_identity: &'a str,
    profile_registry_identity: &'a str,
    lexical_schema_version: &'a str,
    cst_schema_version: &'a str,
    ast_schema_version: &'a str,
    output_digest: &'a str,
    output: &'a ParseOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceParseCacheObservation {
    pub schema_version: String,
    /// Source envelopes whose parse result was reused after all guards passed.
    pub reused_source_identities: Vec<String>,
    /// Source envelopes parsed during this frontend call.
    pub parsed_source_identities: Vec<String>,
    /// Prior entries rejected because their schema, digest, or parse output
    /// did not match the current source and parser contract.
    pub rejected_entry_count: usize,
    /// Older entries removed to keep the caller-owned cache within its size
    /// and entry-count limits.
    pub evicted_entry_count: usize,
    /// Parse results admitted to the cache during this frontend call.
    pub published_entry_count: usize,
}

impl Default for SourceParseCacheObservation {
    fn default() -> Self {
        Self {
            schema_version: SOURCE_PARSE_CACHE_OBSERVATION_SCHEMA_VERSION.to_owned(),
            reused_source_identities: Vec::new(),
            parsed_source_identities: Vec::new(),
            rejected_entry_count: 0,
            evicted_entry_count: 0,
            published_entry_count: 0,
        }
    }
}

impl SourceParseCache {
    pub(crate) fn parse(
        &mut self,
        envelope: &SourceEnvelope,
        observation: &mut SourceParseCacheObservation,
    ) -> ParseOutput {
        if self.schema_version != SOURCE_PARSE_CACHE_SCHEMA_VERSION {
            observation.rejected_entry_count += self.entries.len();
            self.entries.clear();
            self.entry_sizes.clear();
            self.schema_version = SOURCE_PARSE_CACHE_SCHEMA_VERSION.to_owned();
        }
        observation.evicted_entry_count += self.trim_bounded();

        let profile_registry_identity = profile_registry_identity();
        let key = cache_key(envelope, &profile_registry_identity);
        if envelope.identity_is_valid() {
            let matching = self
                .entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| (entry.key == key).then_some(index))
                .collect::<Vec<_>>();
            if matching.len() == 1 {
                let index = matching[0];
                let entry = &self.entries[index];
                if entry_matches(entry, envelope, &key, &profile_registry_identity) {
                    let entry = self.entries.remove(index);
                    let entry_size = self.entry_sizes.remove(index);
                    let mut output = entry.output.clone();
                    rebind_source_identity(&mut output, &envelope.identity);
                    self.entries.push(entry);
                    self.entry_sizes.push(entry_size);
                    observation
                        .reused_source_identities
                        .push(envelope.identity.clone());
                    return output;
                }
            }
            if !matching.is_empty() {
                observation.rejected_entry_count += matching.len();
                for index in matching.into_iter().rev() {
                    self.entries.remove(index);
                    self.entry_sizes.remove(index);
                }
            }
        }

        observation
            .parsed_source_identities
            .push(envelope.identity.clone());
        mncs_model::record_counter("source_parse");
        let output = mncs_syntax::parse(envelope);
        if envelope.identity_is_valid() && output_matches_envelope(&output, envelope) {
            let output_digest = match serialized_sha256_with_limit(&output, MAX_CACHE_BYTES) {
                Some((digest, _)) => digest,
                None => return output,
            };
            let entry_view = SourceParseCacheEntryRef {
                key: &key,
                source_identity: &envelope.identity,
                parser_policy_identity: SOURCE_PARSE_POLICY_IDENTITY,
                profile_registry_identity: &profile_registry_identity,
                lexical_schema_version: LEXICAL_SCHEMA_VERSION,
                cst_schema_version: CST_SCHEMA_VERSION,
                ast_schema_version: AST_SCHEMA_VERSION,
                output_digest: &output_digest,
                output: &output,
            };
            let entry_bytes = match serialized_entry_size_limited(&entry_view, MAX_CACHE_BYTES) {
                Some(size) => size,
                None => return output,
            };
            let entry = SourceParseCacheEntry {
                key,
                source_identity: envelope.identity.clone(),
                parser_policy_identity: SOURCE_PARSE_POLICY_IDENTITY.to_owned(),
                profile_registry_identity,
                lexical_schema_version: LEXICAL_SCHEMA_VERSION.to_owned(),
                cst_schema_version: CST_SCHEMA_VERSION.to_owned(),
                ast_schema_version: AST_SCHEMA_VERSION.to_owned(),
                output_digest,
                output: output.clone(),
            };
            if self.push_bounded(entry, entry_bytes, observation) {
                observation.published_entry_count += 1;
            }
        }
        output
    }

    fn push_bounded(
        &mut self,
        entry: SourceParseCacheEntry,
        entry_bytes: usize,
        observation: &mut SourceParseCacheObservation,
    ) -> bool {
        self.ensure_entry_sizes();
        if entry_bytes > MAX_CACHE_BYTES {
            return false;
        }
        let key = entry.key.clone();
        self.entries.push(entry);
        self.entry_sizes.push(entry_bytes);
        observation.evicted_entry_count += self.trim_bounded();
        self.entries.iter().any(|entry| entry.key == key)
    }

    fn ensure_entry_sizes(&mut self) {
        if self.entry_sizes.len() == self.entries.len() {
            return;
        }
        self.entry_sizes = self
            .entries
            .iter()
            .map(|entry| serialized_entry_size(entry).unwrap_or(MAX_CACHE_BYTES + 1))
            .collect();
    }

    fn trim_bounded(&mut self) -> usize {
        let mut removed = 0;
        if self.entries.len() > MAX_CACHE_ENTRIES {
            let entry_count = self.entries.len();
            let excess = entry_count - MAX_CACHE_ENTRIES;
            self.entries.drain(..excess);
            if self.entry_sizes.len() == entry_count {
                self.entry_sizes.drain(..excess);
            } else {
                self.entry_sizes.clear();
            }
            removed += excess;
        }
        self.ensure_entry_sizes();
        let mut total = self
            .entry_sizes
            .iter()
            .map(|size| *size as u64)
            .sum::<u64>();
        while self.entries.len() > MAX_CACHE_ENTRIES || total > MAX_CACHE_BYTES as u64 {
            if self.entries.is_empty() {
                break;
            }
            total -= self.entry_sizes.remove(0) as u64;
            self.entries.remove(0);
            removed += 1;
        }
        removed
    }
}

fn serialized_entry_size(entry: &SourceParseCacheEntry) -> Option<usize> {
    serialized_entry_size_limited(entry, MAX_CACHE_BYTES)
}

fn serialized_entry_size_limited<T: Serialize>(entry: &T, maximum_bytes: usize) -> Option<usize> {
    let mut counter = SerializedSizeCounter {
        bytes: 0,
        maximum_bytes,
    };
    serde_json::to_writer(&mut counter, entry).ok()?;
    Some(counter.bytes)
}

struct SerializedSizeCounter {
    bytes: usize,
    maximum_bytes: usize,
}

impl Write for SerializedSizeCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.maximum_bytes.saturating_sub(self.bytes) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "serialized source parse entry exceeds the cache byte limit",
            ));
        }
        self.bytes += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialized_sha256<T: Serialize>(value: &T) -> Option<String> {
    serialized_sha256_with_limit(value, usize::MAX).map(|(digest, _)| digest)
}

fn serialized_sha256_with_limit<T: Serialize>(
    value: &T,
    maximum_bytes: usize,
) -> Option<(String, usize)> {
    let mut writer = Sha256AndSizeWriter {
        digest: Sha256::new(),
        bytes: 0,
        maximum_bytes,
    };
    serde_json::to_writer(&mut writer, value).ok()?;
    let digest = writer.digest.finalize();
    Some((hex_digest(&digest), writer.bytes))
}

struct Sha256AndSizeWriter {
    digest: Sha256,
    bytes: usize,
    maximum_bytes: usize,
}

impl Write for Sha256AndSizeWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.maximum_bytes.saturating_sub(self.bytes) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "serialized source parse output exceeds the cache byte limit",
            ));
        }
        self.digest.update(bytes);
        self.bytes += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn cache_schema_version() -> String {
    SOURCE_PARSE_CACHE_SCHEMA_VERSION.to_owned()
}

pub(crate) fn profile_registry_identity() -> String {
    let snapshot = source_profile_registry_snapshot();
    let bytes = serde_json::to_vec(&snapshot).expect("profile registry snapshot is serializable");
    sha256_hex(&bytes)
}

fn cache_key(envelope: &SourceEnvelope, profile_registry_identity: &str) -> String {
    #[derive(Serialize)]
    struct Key<'a> {
        cache_schema_version: &'a str,
        parser_policy_identity: &'a str,
        profile_registry_identity: &'a str,
        source_envelope_schema_version: &'a str,
        language_profile: &'a str,
        artifact_kind: SourceArtifactKind,
        source_text_sha256: String,
        lexical_schema_version: &'a str,
        cst_schema_version: &'a str,
        ast_schema_version: &'a str,
    }
    let material = Key {
        cache_schema_version: SOURCE_PARSE_CACHE_SCHEMA_VERSION,
        parser_policy_identity: SOURCE_PARSE_POLICY_IDENTITY,
        profile_registry_identity,
        source_envelope_schema_version: &envelope.schema_version,
        language_profile: &envelope.language_version,
        artifact_kind: envelope.artifact_kind,
        source_text_sha256: sha256_hex(envelope.text.as_bytes()),
        lexical_schema_version: LEXICAL_SCHEMA_VERSION,
        cst_schema_version: CST_SCHEMA_VERSION,
        ast_schema_version: AST_SCHEMA_VERSION,
    };
    let bytes = serde_json::to_vec(&material).expect("parse cache key is serializable");
    sha256_hex(&bytes)
}

fn entry_matches(
    entry: &SourceParseCacheEntry,
    envelope: &SourceEnvelope,
    key: &str,
    profile_registry_identity: &str,
) -> bool {
    if entry.key != key
        || entry.parser_policy_identity != SOURCE_PARSE_POLICY_IDENTITY
        || entry.profile_registry_identity != profile_registry_identity
        || entry.lexical_schema_version != LEXICAL_SCHEMA_VERSION
        || entry.cst_schema_version != CST_SCHEMA_VERSION
        || entry.ast_schema_version != AST_SCHEMA_VERSION
        || !output_matches_entry(&entry.output, &entry.source_identity, envelope)
    {
        return false;
    }
    serialized_sha256(&entry.output).is_some_and(|digest| digest == entry.output_digest)
}

fn output_matches_entry(
    output: &ParseOutput,
    cached_source_identity: &str,
    envelope: &SourceEnvelope,
) -> bool {
    output.lexical.schema_version == LEXICAL_SCHEMA_VERSION
        && output.lexical.source_identity == cached_source_identity
        && output.cst.schema_version == CST_SCHEMA_VERSION
        && output.cst.source_identity == cached_source_identity
        && output.cst.source_length == envelope.text.len()
        && output.ast.as_ref().is_none_or(|ast| {
            ast.schema_version == AST_SCHEMA_VERSION
                && ast.source_identity == cached_source_identity
        })
}

fn output_matches_envelope(output: &ParseOutput, envelope: &SourceEnvelope) -> bool {
    output_matches_entry(output, &envelope.identity, envelope)
}

fn rebind_source_identity(output: &mut ParseOutput, source_identity: &str) {
    output.lexical.source_identity = source_identity.to_owned();
    output.cst.source_identity = source_identity.to_owned();
    if let Some(ast) = output.ast.as_mut() {
        ast.source_identity = source_identity.to_owned();
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_digest(&digest)
}

fn hex_digest(digest: &[u8]) -> String {
    let mut text = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut text, "{byte:02x}").expect("writing to a String is infallible");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{
        serialized_entry_size_limited, serialized_sha256, serialized_sha256_with_limit, sha256_hex,
        SourceParseCache, SourceParseCacheEntry, SourceParseCacheEntryRef,
        SourceParseCacheObservation, MAX_CACHE_ENTRIES,
    };
    use mncs_syntax::{
        SourceArtifactKind, SourceEnvelope, SourceOrigin, SourceOriginKind, SourceProvenance,
        SourceRelationship, SourceRelationshipKind,
    };

    #[test]
    fn parse_cache_reuses_exact_snapshots_after_serialized_process_restart() {
        let envelope = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "cache.exact",
            "mncs 0.19; module cache.exact; fn value() -> (result: u64) { return 9; }",
        );
        let mut first_cache = SourceParseCache::default();
        let mut first_observation = SourceParseCacheObservation::default();
        let first = first_cache.parse(&envelope, &mut first_observation);
        assert_eq!(
            first_observation.parsed_source_identities,
            [envelope.identity.clone()]
        );
        assert_eq!(first_observation.published_entry_count, 1);

        let bytes = serde_json::to_vec(&first_cache).expect("cache serializes");
        let mut restarted_cache: SourceParseCache =
            serde_json::from_slice(&bytes).expect("cache restores in a fresh process");
        let mut second_observation = SourceParseCacheObservation::default();
        let second = restarted_cache.parse(&envelope, &mut second_observation);
        assert_eq!(second, first);
        assert_eq!(
            second_observation.reused_source_identities,
            [envelope.identity]
        );
        assert!(second_observation.parsed_source_identities.is_empty());
    }

    #[test]
    fn parse_cache_reuses_same_parser_input_across_source_envelope_identities() {
        let text = "mncs 0.19; module cache.alias; fn value() -> (result: u64) { return 9; }";
        let original = SourceEnvelope::inline(SourceArtifactKind::Program, "cache.alias.a", text);
        let alias = SourceEnvelope::new(
            SourceArtifactKind::Program,
            "cache.alias.b",
            SourceOrigin {
                kind: SourceOriginKind::Generated,
                locator: Some("generated://cache.alias.b".to_owned()),
            },
            text,
        )
        .with_relationships(vec![SourceRelationship {
            kind: SourceRelationshipKind::DependsOn,
            target_identity: "mncs:source:dependency:cache.alias".to_owned(),
        }])
        .with_provenance(SourceProvenance {
            producer_identity: Some("mncs-language-test/1".to_owned()),
            derived_from: vec!["mncs:source:original:cache.alias".to_owned()],
        });
        assert_ne!(original.identity, alias.identity);

        let mut cache = SourceParseCache::default();
        let mut original_observation = SourceParseCacheObservation::default();
        let original_output = cache.parse(&original, &mut original_observation);
        let mut alias_observation = SourceParseCacheObservation::default();
        let alias_output = cache.parse(&alias, &mut alias_observation);

        assert_eq!(
            alias_observation.reused_source_identities,
            [alias.identity.clone()]
        );
        assert!(alias_observation.parsed_source_identities.is_empty());
        assert_eq!(alias_output.lexical.source_identity, alias.identity);
        assert_eq!(alias_output.cst.source_identity, alias.identity);
        assert_eq!(
            alias_output
                .ast
                .as_ref()
                .map(|ast| ast.source_identity.as_str()),
            Some(alias.identity.as_str())
        );
        assert_eq!(alias_output.lexical.tokens, original_output.lexical.tokens);
        assert_eq!(alias_output.cst.root, original_output.cst.root);
        assert_eq!(
            alias_output.ast.as_ref().map(|ast| &ast.functions),
            original_output.ast.as_ref().map(|ast| &ast.functions)
        );
    }

    #[test]
    fn changed_and_corrupted_snapshots_are_reparsed_with_current_diagnostics() {
        let envelope = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "cache.invalid",
            "mncs 0.19; module cache.invalid; fn value() -> (result: u64) { return 9; }",
        );
        let mut cache = SourceParseCache::default();
        let mut observation = SourceParseCacheObservation::default();
        let _ = cache.parse(&envelope, &mut observation);

        let changed = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "cache.invalid",
            "mncs 0.19; module cache.invalid; fn value() -> (result: u64) { return missing; }",
        );
        let mut changed_observation = SourceParseCacheObservation::default();
        let changed_output = cache.parse(&changed, &mut changed_observation);
        assert!(changed_observation.reused_source_identities.is_empty());
        assert_eq!(
            changed_observation.parsed_source_identities,
            [changed.identity]
        );
        assert!(changed_output.ast.is_some());

        let invalid = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "cache.invalid",
            "mncs 0.19 module cache.invalid;",
        );
        let mut invalid_observation = SourceParseCacheObservation::default();
        let invalid_output = cache.parse(&invalid, &mut invalid_observation);
        assert!(invalid_output.ast.is_none());
        assert!(!invalid_output.diagnostics.is_empty());

        let matching = cache
            .entries
            .iter_mut()
            .find(|entry| entry.source_identity == invalid.identity)
            .expect("invalid source parse is cacheable as a complete diagnostic result");
        matching.output_digest = "0".repeat(64);
        let mut tamper_observation = SourceParseCacheObservation::default();
        let reparsed = cache.parse(&invalid, &mut tamper_observation);
        assert_eq!(tamper_observation.rejected_entry_count, 1);
        assert!(tamper_observation.reused_source_identities.is_empty());
        assert_eq!(reparsed.diagnostics, invalid_output.diagnostics);
    }

    #[test]
    fn restored_cache_trims_oldest_entries_before_lookup() {
        let envelope = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "cache.restore_bound",
            "mncs 0.19; module cache.restore_bound; fn value() -> (result: u64) { return 9; }",
        );
        let mut cache = SourceParseCache::default();
        let mut initial_observation = SourceParseCacheObservation::default();
        let _ = cache.parse(&envelope, &mut initial_observation);
        let valid_entry = cache.entries[0].clone();
        for index in 0..MAX_CACHE_ENTRIES {
            let mut older_entry = valid_entry.clone();
            older_entry.key = format!("stale-entry-{index}");
            cache.entries.insert(0, older_entry);
        }
        assert_eq!(cache.entries.len(), MAX_CACHE_ENTRIES + 1);

        let bytes = serde_json::to_vec(&cache).expect("cache serializes");
        let mut restored: SourceParseCache =
            serde_json::from_slice(&bytes).expect("cache restores");
        let mut observation = SourceParseCacheObservation::default();
        let output = restored.parse(&envelope, &mut observation);

        assert!(output.ast.is_some());
        assert_eq!(observation.evicted_entry_count, 1);
        assert_eq!(observation.reused_source_identities, [envelope.identity]);
        assert!(observation.parsed_source_identities.is_empty());
        assert_eq!(restored.entries.len(), MAX_CACHE_ENTRIES);
    }

    #[test]
    fn cache_entry_size_accounting_stops_at_its_byte_limit() {
        let envelope = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "cache.size_limit",
            "mncs 0.19; module cache.size_limit; fn value() -> (result: u64) { return 9; }",
        );
        let mut cache = SourceParseCache::default();
        let mut observation = SourceParseCacheObservation::default();
        let _ = cache.parse(&envelope, &mut observation);
        let entry: &SourceParseCacheEntry = &cache.entries[0];
        let bytes = serde_json::to_vec(entry).expect("entry serializes for test fixture");
        let exact_size = bytes.len();
        let entry_ref = SourceParseCacheEntryRef {
            key: &entry.key,
            source_identity: &entry.source_identity,
            parser_policy_identity: &entry.parser_policy_identity,
            profile_registry_identity: &entry.profile_registry_identity,
            lexical_schema_version: &entry.lexical_schema_version,
            cst_schema_version: &entry.cst_schema_version,
            ast_schema_version: &entry.ast_schema_version,
            output_digest: &entry.output_digest,
            output: &entry.output,
        };

        assert_eq!(
            serialized_entry_size_limited(&entry_ref, exact_size),
            Some(exact_size)
        );
        assert_eq!(serialized_entry_size_limited(entry, exact_size - 1), None);
    }

    #[test]
    fn streamed_output_digest_matches_serialized_json_digest() {
        let envelope = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "cache.stream_digest",
            "mncs 0.19; module cache.stream_digest; fn value() -> (result: u64) { return 9; }",
        );
        let mut cache = SourceParseCache::default();
        let mut observation = SourceParseCacheObservation::default();
        let _ = cache.parse(&envelope, &mut observation);
        let output = &cache.entries[0].output;
        let bytes = serde_json::to_vec(output).expect("parse output serializes");

        let expected_digest = sha256_hex(&bytes);
        let exact_output_size = bytes.len();
        assert_eq!(serialized_sha256(output), Some(expected_digest.clone()));
        assert_eq!(
            serialized_sha256_with_limit(output, exact_output_size),
            Some((expected_digest, exact_output_size))
        );
        assert_eq!(
            serialized_sha256_with_limit(output, exact_output_size - 1),
            None
        );
    }
}
