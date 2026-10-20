//! Bounded, Language-owned reuse of exact source parse results.
//!
//! The cache is caller-owned so a build service can persist it between
//! processes. Entries are reusable only for the same sealed source envelope,
//! parser policy, source-profile registry, and syntax artifact schemas.

use mncs_syntax::{
    AST_SCHEMA_VERSION, CST_SCHEMA_VERSION, LEXICAL_SCHEMA_VERSION, ParseOutput, SourceEnvelope,
    source_profile_registry_snapshot,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SOURCE_PARSE_CACHE_SCHEMA_VERSION: &str = "mncs.language-source-parse-cache/1";
/// Bump this identity whenever parser behavior changes without a syntax
/// artifact schema change.
pub const SOURCE_PARSE_POLICY_IDENTITY: &str = "mncs-language-source-parser/1";
pub const SOURCE_PARSE_CACHE_OBSERVATION_SCHEMA_VERSION: &str =
    "mncs.language-source-parse-cache-observation/1";

const MAX_CACHE_ENTRIES: usize = 256;
const MAX_CACHE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceParseCache {
    #[serde(default = "cache_schema_version")]
    schema_version: String,
    #[serde(default)]
    entries: Vec<SourceParseCacheEntry>,
}

impl Default for SourceParseCache {
    fn default() -> Self {
        Self {
            schema_version: cache_schema_version(),
            entries: Vec::new(),
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
            self.schema_version = SOURCE_PARSE_CACHE_SCHEMA_VERSION.to_owned();
        }
        observation.evicted_entry_count += self.trim_bounded();

        let profile_registry_identity = profile_registry_identity();
        let key = cache_key(&envelope.identity, &profile_registry_identity);
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
                    let output = entry.output.clone();
                    self.entries.push(entry);
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
                }
            }
        }

        observation
            .parsed_source_identities
            .push(envelope.identity.clone());
        mncs_model::record_counter("source_parse");
        let output = mncs_syntax::parse(envelope);
        if envelope.identity_is_valid() && output_matches_envelope(&output, envelope) {
            let output_bytes = match serde_json::to_vec(&output) {
                Ok(bytes) => bytes,
                Err(_) => return output,
            };
            let entry = SourceParseCacheEntry {
                key,
                source_identity: envelope.identity.clone(),
                parser_policy_identity: SOURCE_PARSE_POLICY_IDENTITY.to_owned(),
                profile_registry_identity,
                lexical_schema_version: LEXICAL_SCHEMA_VERSION.to_owned(),
                cst_schema_version: CST_SCHEMA_VERSION.to_owned(),
                ast_schema_version: AST_SCHEMA_VERSION.to_owned(),
                output_digest: sha256_hex(&output_bytes),
                output: output.clone(),
            };
            if self.push_bounded(entry, observation) {
                observation.published_entry_count += 1;
            }
        }
        output
    }

    fn push_bounded(
        &mut self,
        entry: SourceParseCacheEntry,
        observation: &mut SourceParseCacheObservation,
    ) -> bool {
        let entry_bytes = match serde_json::to_vec(&entry) {
            Ok(bytes) => bytes.len(),
            Err(_) => return false,
        };
        if entry_bytes > MAX_CACHE_BYTES {
            return false;
        }
        let key = entry.key.clone();
        self.entries.push(entry);
        observation.evicted_entry_count += self.trim_bounded();
        self.entries.iter().any(|entry| entry.key == key)
    }

    fn trim_bounded(&mut self) -> usize {
        let mut sizes = self
            .entries
            .iter()
            .map(|entry| {
                serde_json::to_vec(entry)
                    .map(|bytes| bytes.len())
                    .unwrap_or(usize::MAX)
            })
            .collect::<Vec<_>>();
        let mut total = sizes
            .iter()
            .fold(0usize, |sum, size| sum.saturating_add(*size));
        let mut removed = 0;
        while self.entries.len() > MAX_CACHE_ENTRIES || total > MAX_CACHE_BYTES {
            if self.entries.is_empty() {
                break;
            }
            total = total.saturating_sub(sizes.remove(0));
            self.entries.remove(0);
            removed += 1;
        }
        removed
    }
}

fn cache_schema_version() -> String {
    SOURCE_PARSE_CACHE_SCHEMA_VERSION.to_owned()
}

fn profile_registry_identity() -> String {
    let snapshot = source_profile_registry_snapshot();
    let bytes = serde_json::to_vec(&snapshot).expect("profile registry snapshot is serializable");
    sha256_hex(&bytes)
}

fn cache_key(source_identity: &str, profile_registry_identity: &str) -> String {
    #[derive(Serialize)]
    struct Key<'a> {
        cache_schema_version: &'a str,
        parser_policy_identity: &'a str,
        profile_registry_identity: &'a str,
        source_identity: &'a str,
        lexical_schema_version: &'a str,
        cst_schema_version: &'a str,
        ast_schema_version: &'a str,
    }
    let material = Key {
        cache_schema_version: SOURCE_PARSE_CACHE_SCHEMA_VERSION,
        parser_policy_identity: SOURCE_PARSE_POLICY_IDENTITY,
        profile_registry_identity,
        source_identity,
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
        || entry.source_identity != envelope.identity
        || entry.parser_policy_identity != SOURCE_PARSE_POLICY_IDENTITY
        || entry.profile_registry_identity != profile_registry_identity
        || entry.lexical_schema_version != LEXICAL_SCHEMA_VERSION
        || entry.cst_schema_version != CST_SCHEMA_VERSION
        || entry.ast_schema_version != AST_SCHEMA_VERSION
        || !output_matches_envelope(&entry.output, envelope)
    {
        return false;
    }
    serde_json::to_vec(&entry.output)
        .ok()
        .is_some_and(|bytes| sha256_hex(&bytes) == entry.output_digest)
}

fn output_matches_envelope(output: &ParseOutput, envelope: &SourceEnvelope) -> bool {
    output.lexical.schema_version == LEXICAL_SCHEMA_VERSION
        && output.lexical.source_identity == envelope.identity
        && output.cst.schema_version == CST_SCHEMA_VERSION
        && output.cst.source_identity == envelope.identity
        && output.cst.source_length == envelope.text.len()
        && output.ast.as_ref().is_none_or(|ast| {
            ast.schema_version == AST_SCHEMA_VERSION && ast.source_identity == envelope.identity
        })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut text, "{byte:02x}").expect("writing to a String is infallible");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{SourceParseCache, SourceParseCacheObservation};
    use mncs_syntax::{SourceArtifactKind, SourceEnvelope};

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
}
