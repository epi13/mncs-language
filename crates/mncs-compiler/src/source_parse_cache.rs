//! Bounded, Language-owned reuse of source parse results.
//!
//! The cache is caller-owned so a build service can persist it between
//! processes. Entries are reusable for the same parser inputs, parser policy,
//! source-profile registry, and syntax artifact schemas. Source-envelope
//! identity is rebound on reuse because origin, relationships, provenance,
//! and logical names do not affect parsing and source spans remain byte based.

use mncs_syntax::{
    source_profile_registry_snapshot, AbstractSyntaxTree, ConcreteSyntaxTree, CstNode,
    LexedDocument, ParseOutput, SourceArtifactKind, SourceEnvelope, SourceSpan, SourceToken,
    AST_SCHEMA_VERSION, CST_SCHEMA_VERSION, LEXICAL_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{self, Write};

pub const SOURCE_PARSE_CACHE_SCHEMA_VERSION: &str = "mncs.language-source-parse-cache/3";
/// Bump this identity whenever parser behavior changes without a syntax
/// artifact schema change.
pub const SOURCE_PARSE_POLICY_IDENTITY: &str = "mncs-language-source-parser/1";
pub const SOURCE_PARSE_CACHE_OBSERVATION_SCHEMA_VERSION: &str =
    "mncs.language-source-parse-cache-observation/3";

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
    source_envelope_schema_version: String,
    language_profile: String,
    artifact_kind: SourceArtifactKind,
    parser_policy_identity: String,
    profile_registry_identity: String,
    lexical_schema_version: String,
    cst_schema_version: String,
    ast_schema_version: String,
    significant_tokens_digest: String,
    output_digest: String,
    output: ParseOutput,
}

#[derive(Serialize)]
struct SourceParseCacheEntryRef<'a> {
    key: &'a str,
    source_identity: &'a str,
    source_envelope_schema_version: &'a str,
    language_profile: &'a str,
    artifact_kind: SourceArtifactKind,
    parser_policy_identity: &'a str,
    profile_registry_identity: &'a str,
    lexical_schema_version: &'a str,
    cst_schema_version: &'a str,
    ast_schema_version: &'a str,
    significant_tokens_digest: &'a str,
    output_digest: &'a str,
    output: &'a ParseOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceParseCacheObservation {
    pub schema_version: String,
    /// Source envelopes whose parse result was reused after all guards passed.
    pub reused_source_identities: Vec<String>,
    /// Source envelopes whose semantic syntax tree was reused after fresh
    /// lexing proved that every non-trivia token and parser assumption stayed
    /// identical. Current tokens and source locations are rebuilt for these
    /// snapshots.
    #[serde(default)]
    pub reused_same_significant_tokens_source_identities: Vec<String>,
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
            reused_same_significant_tokens_source_identities: Vec::new(),
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

        // A changed source snapshot can reuse its syntax tree only after a
        // fresh lexical pass proves the complete significant-token stream is
        // unchanged. The fresh lexical document is also used by the fallback
        // parser, so a miss does not lex the source twice.
        let lexical = mncs_syntax::lex(envelope);
        if envelope.identity_is_valid() && lexical.diagnostics.is_empty() {
            let significant_digest = significant_tokens_digest(&lexical.tokens);
            let matching = self
                .entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    semantic_candidate_matches(
                        entry,
                        envelope,
                        &profile_registry_identity,
                        &significant_digest,
                    )
                    .then_some(index)
                })
                .collect::<Vec<_>>();
            if matching.len() == 1 {
                let index = matching[0];
                let entry = &self.entries[index];
                if entry_matches_significant_tokens(
                    entry,
                    envelope,
                    &profile_registry_identity,
                    &significant_digest,
                    &lexical.tokens,
                ) {
                    if let Some(output) = rebase_parse_output(
                        &entry.output,
                        &envelope.identity,
                        &envelope.text,
                        &lexical,
                    ) {
                        let entry = self.entries.remove(index);
                        let entry_size = self.entry_sizes.remove(index);
                        self.entries.push(entry);
                        self.entry_sizes.push(entry_size);
                        observation
                            .reused_same_significant_tokens_source_identities
                            .push(envelope.identity.clone());
                        return output;
                    }
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
        let output = mncs_syntax::parse_with_lexical(envelope, lexical);
        let significant_digest = significant_tokens_digest(&output.lexical.tokens);
        if envelope.identity_is_valid() && output_matches_envelope(&output, envelope) {
            let output_digest = match serialized_sha256_with_limit(&output, MAX_CACHE_BYTES) {
                Some((digest, _)) => digest,
                None => return output,
            };
            let entry_view = SourceParseCacheEntryRef {
                key: &key,
                source_identity: &envelope.identity,
                source_envelope_schema_version: &envelope.schema_version,
                language_profile: &envelope.language_version,
                artifact_kind: envelope.artifact_kind,
                parser_policy_identity: SOURCE_PARSE_POLICY_IDENTITY,
                profile_registry_identity: &profile_registry_identity,
                lexical_schema_version: LEXICAL_SCHEMA_VERSION,
                cst_schema_version: CST_SCHEMA_VERSION,
                ast_schema_version: AST_SCHEMA_VERSION,
                significant_tokens_digest: &significant_digest,
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
                source_envelope_schema_version: envelope.schema_version.clone(),
                language_profile: envelope.language_version.clone(),
                artifact_kind: envelope.artifact_kind,
                parser_policy_identity: SOURCE_PARSE_POLICY_IDENTITY.to_owned(),
                profile_registry_identity,
                lexical_schema_version: LEXICAL_SCHEMA_VERSION.to_owned(),
                cst_schema_version: CST_SCHEMA_VERSION.to_owned(),
                ast_schema_version: AST_SCHEMA_VERSION.to_owned(),
                significant_tokens_digest: significant_digest,
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
    cache_key_for_parts(
        &envelope.schema_version,
        &envelope.language_version,
        envelope.artifact_kind,
        &sha256_hex(envelope.text.as_bytes()),
        profile_registry_identity,
    )
}

fn cache_key_for_parts(
    source_envelope_schema_version: &str,
    language_profile: &str,
    artifact_kind: SourceArtifactKind,
    source_text_sha256: &str,
    profile_registry_identity: &str,
) -> String {
    #[derive(Serialize)]
    struct Key<'a> {
        cache_schema_version: &'a str,
        parser_policy_identity: &'a str,
        profile_registry_identity: &'a str,
        source_envelope_schema_version: &'a str,
        language_profile: &'a str,
        artifact_kind: SourceArtifactKind,
        source_text_sha256: &'a str,
        lexical_schema_version: &'a str,
        cst_schema_version: &'a str,
        ast_schema_version: &'a str,
    }
    let material = Key {
        cache_schema_version: SOURCE_PARSE_CACHE_SCHEMA_VERSION,
        parser_policy_identity: SOURCE_PARSE_POLICY_IDENTITY,
        profile_registry_identity,
        source_envelope_schema_version,
        language_profile,
        artifact_kind,
        source_text_sha256,
        lexical_schema_version: LEXICAL_SCHEMA_VERSION,
        cst_schema_version: CST_SCHEMA_VERSION,
        ast_schema_version: AST_SCHEMA_VERSION,
    };
    let bytes = serde_json::to_vec(&material).expect("parse cache key is serializable");
    sha256_hex(&bytes)
}

fn semantic_candidate_matches(
    entry: &SourceParseCacheEntry,
    envelope: &SourceEnvelope,
    profile_registry_identity: &str,
    significant_digest: &str,
) -> bool {
    entry.source_envelope_schema_version == envelope.schema_version
        && entry.language_profile == envelope.language_version
        && entry.artifact_kind == envelope.artifact_kind
        && entry.parser_policy_identity == SOURCE_PARSE_POLICY_IDENTITY
        && entry.profile_registry_identity == profile_registry_identity
        && entry.lexical_schema_version == LEXICAL_SCHEMA_VERSION
        && entry.cst_schema_version == CST_SCHEMA_VERSION
        && entry.ast_schema_version == AST_SCHEMA_VERSION
        && entry.significant_tokens_digest == significant_digest
}

fn entry_matches(
    entry: &SourceParseCacheEntry,
    envelope: &SourceEnvelope,
    key: &str,
    profile_registry_identity: &str,
) -> bool {
    if entry.key != key
        || entry.source_envelope_schema_version != envelope.schema_version
        || entry.language_profile != envelope.language_version
        || entry.artifact_kind != envelope.artifact_kind
        || entry.parser_policy_identity != SOURCE_PARSE_POLICY_IDENTITY
        || entry.profile_registry_identity != profile_registry_identity
        || entry.lexical_schema_version != LEXICAL_SCHEMA_VERSION
        || entry.cst_schema_version != CST_SCHEMA_VERSION
        || entry.ast_schema_version != AST_SCHEMA_VERSION
        || entry.significant_tokens_digest
            != significant_tokens_digest(&entry.output.lexical.tokens)
        || !output_matches_entry(&entry.output, &entry.source_identity, envelope)
    {
        return false;
    }
    serialized_sha256(&entry.output).is_some_and(|digest| digest == entry.output_digest)
}

fn entry_matches_significant_tokens(
    entry: &SourceParseCacheEntry,
    envelope: &SourceEnvelope,
    profile_registry_identity: &str,
    significant_digest: &str,
    fresh_tokens: &[SourceToken],
) -> bool {
    if !semantic_candidate_matches(
        entry,
        envelope,
        profile_registry_identity,
        significant_digest,
    ) || !entry.output.is_valid()
        || entry.output.lexical.schema_version != LEXICAL_SCHEMA_VERSION
        || entry.output.lexical.source_identity != entry.source_identity
        || entry.output.cst.schema_version != CST_SCHEMA_VERSION
        || entry.output.cst.source_identity != entry.source_identity
        || entry.output.cst.tokens != entry.output.lexical.tokens
        || entry.output.cst.source_length != entry.output.lexical.reconstruct().len()
        || entry.output.cst.reconstruct() != entry.output.lexical.reconstruct()
        || entry.output.ast.as_ref().is_none_or(|ast| {
            ast.schema_version != AST_SCHEMA_VERSION || ast.source_identity != entry.source_identity
        })
        || entry.significant_tokens_digest
            != significant_tokens_digest(&entry.output.lexical.tokens)
    {
        return false;
    }
    let cached_text = entry.output.lexical.reconstruct();
    if entry.key
        != cache_key_for_parts(
            &entry.source_envelope_schema_version,
            &entry.language_profile,
            entry.artifact_kind,
            &sha256_hex(cached_text.as_bytes()),
            profile_registry_identity,
        )
    {
        return false;
    }
    if !serialized_sha256(&entry.output).is_some_and(|digest| digest == entry.output_digest) {
        return false;
    }
    let cached_significant = significant_tokens(&entry.output.lexical.tokens);
    let fresh_significant = significant_tokens(fresh_tokens);
    cached_significant.len() == fresh_significant.len()
        && cached_significant
            .iter()
            .zip(fresh_significant)
            .all(|(cached, fresh)| cached.kind == fresh.kind && cached.text == fresh.text)
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
        && output.cst.tokens == output.lexical.tokens
        && output.cst.reconstruct() == envelope.text
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

fn significant_tokens(tokens: &[SourceToken]) -> Vec<&SourceToken> {
    tokens
        .iter()
        .filter(|token| !token.kind.is_trivia())
        .collect()
}

fn significant_tokens_digest(tokens: &[SourceToken]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"mncs.language-significant-token-stream/1");
    for token in tokens.iter().filter(|token| !token.kind.is_trivia()) {
        let kind = serde_json::to_vec(&token.kind).expect("token kind is serializable");
        digest.update((kind.len() as u64).to_be_bytes());
        digest.update(kind);
        digest.update((token.text.len() as u64).to_be_bytes());
        digest.update(token.text.as_bytes());
    }
    let digest = digest.finalize();
    hex_digest(&digest)
}

struct SourceLocationMap {
    starts: HashMap<usize, usize>,
    ends: HashMap<usize, usize>,
    raw_indices: HashMap<usize, usize>,
}

impl SourceLocationMap {
    fn from_tokens(old_tokens: &[SourceToken], new_tokens: &[SourceToken]) -> Option<Self> {
        let old_significant = significant_tokens(old_tokens);
        let new_significant = significant_tokens(new_tokens);
        if old_significant.len() != new_significant.len()
            || !old_significant
                .iter()
                .zip(&new_significant)
                .all(|(old, new)| old.kind == new.kind && old.text == new.text)
        {
            return None;
        }

        let mut starts = HashMap::with_capacity(old_significant.len());
        let mut ends = HashMap::with_capacity(old_significant.len());
        let mut raw_indices = HashMap::with_capacity(old_significant.len());
        for (old, new) in old_tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| !token.kind.is_trivia())
            .zip(
                new_tokens
                    .iter()
                    .enumerate()
                    .filter(|(_, token)| !token.kind.is_trivia()),
            )
        {
            let (old_index, old_token) = old;
            let (new_index, new_token) = new;
            if insert_unique(&mut starts, old_token.span.start, new_token.span.start).is_err()
                || insert_unique(&mut ends, old_token.span.end, new_token.span.end).is_err()
            {
                return None;
            }
            raw_indices.insert(old_index, new_index);
        }
        Some(Self {
            starts,
            ends,
            raw_indices,
        })
    }

    fn rebase_span(
        &self,
        span: SourceSpan,
        old_source_length: usize,
        new_source: &str,
        lines: &SourceLineIndex,
    ) -> Option<SourceSpan> {
        let is_document_span = span.start == 0 && span.end == old_source_length;
        let (start, end) = if is_document_span {
            (0, new_source.len())
        } else if span.start == span.end {
            let position = if span.start == old_source_length {
                new_source.len()
            } else {
                self.starts
                    .get(&span.start)
                    .or_else(|| self.ends.get(&span.start))
                    .copied()?
            };
            (position, position)
        } else {
            (
                self.starts.get(&span.start).copied()?,
                self.ends.get(&span.end).copied()?,
            )
        };
        if start > end
            || end > new_source.len()
            || !new_source.is_char_boundary(start)
            || !new_source.is_char_boundary(end)
        {
            return None;
        }
        Some(lines.span_at(new_source, start, end))
    }
}

fn insert_unique(map: &mut HashMap<usize, usize>, old: usize, new: usize) -> Result<(), ()> {
    match map.insert(old, new) {
        Some(previous) if previous != new => Err(()),
        _ => Ok(()),
    }
}

struct SourceLineIndex {
    line_starts: Vec<usize>,
}

impl SourceLineIndex {
    fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        );
        Self { line_starts }
    }

    fn span_at(&self, source: &str, start: usize, end: usize) -> SourceSpan {
        let line_index = match self.line_starts.binary_search(&start) {
            Ok(index) => index,
            Err(next) => next.saturating_sub(1),
        };
        let line_start = self.line_starts[line_index];
        SourceSpan {
            start,
            end,
            line: line_index + 1,
            column: source[line_start..start].chars().count() + 1,
        }
    }
}

fn rebase_parse_output(
    previous: &ParseOutput,
    source_identity: &str,
    source_text: &str,
    lexical: &LexedDocument,
) -> Option<ParseOutput> {
    if !previous.is_valid()
        || previous.ast.is_none()
        || !lexical.diagnostics.is_empty()
        || lexical.reconstruct() != source_text
    {
        return None;
    }
    let old_source_length = previous.cst.source_length;
    let locations = SourceLocationMap::from_tokens(&previous.lexical.tokens, &lexical.tokens)?;
    let lines = SourceLineIndex::new(source_text);

    let mut cst = previous.cst.clone();
    if !rebase_cst_node(
        &mut cst.root,
        &previous.lexical.tokens,
        old_source_length,
        source_text,
        &lexical.tokens,
        &locations,
        &lines,
        true,
    ) {
        return None;
    }
    cst.source_identity = source_identity.to_owned();
    cst.source_length = source_text.len();
    cst.tokens = lexical.tokens.clone();

    let mut ast_value = serde_json::to_value(previous.ast.as_ref()?).ok()?;
    let mut rebased_span_count = 0;
    if !rebase_span_values(
        &mut ast_value,
        &locations,
        old_source_length,
        source_text,
        &lines,
        &mut rebased_span_count,
    ) || rebased_span_count == 0
    {
        return None;
    }
    let mut ast: AbstractSyntaxTree = serde_json::from_value(ast_value).ok()?;
    ast.source_identity = source_identity.to_owned();

    let mut diagnostics_value = serde_json::to_value(&previous.diagnostics).ok()?;
    let mut diagnostic_span_count = 0;
    if !rebase_span_values(
        &mut diagnostics_value,
        &locations,
        old_source_length,
        source_text,
        &lines,
        &mut diagnostic_span_count,
    ) {
        return None;
    }
    let diagnostics = serde_json::from_value(diagnostics_value).ok()?;

    let result = ParseOutput {
        lexical: lexical.clone(),
        cst: ConcreteSyntaxTree {
            source_identity: source_identity.to_owned(),
            ..cst
        },
        ast: Some(ast),
        diagnostics,
    };
    (result.is_valid()
        && result.cst.reconstruct() == source_text
        && result.cst.source_length == source_text.len()
        && result.cst.tokens == result.lexical.tokens)
        .then_some(result)
}

fn rebase_cst_node(
    node: &mut CstNode,
    old_tokens: &[SourceToken],
    old_source_length: usize,
    new_source: &str,
    new_tokens: &[SourceToken],
    locations: &SourceLocationMap,
    lines: &SourceLineIndex,
    is_document_root: bool,
) -> bool {
    if is_document_root {
        node.span = lines.span_at(new_source, 0, new_source.len());
        node.token_start = 0;
        node.token_end = new_tokens.len();
    } else {
        let Some(span) = locations.rebase_span(node.span, old_source_length, new_source, lines)
        else {
            return false;
        };
        let Some(new_start) = locations.raw_indices.get(&node.token_start).copied() else {
            return false;
        };
        let Some(old_last) = node.token_end.checked_sub(1) else {
            return false;
        };
        if old_tokens.get(old_last).is_none() {
            return false;
        }
        let Some(new_last) = locations.raw_indices.get(&old_last).copied() else {
            return false;
        };
        if new_tokens.get(new_start).is_none() || new_tokens.get(new_last).is_none() {
            return false;
        }
        node.span = span;
        node.token_start = new_start;
        node.token_end = new_last + 1;
    }
    node.children.iter_mut().all(|child| {
        rebase_cst_node(
            child,
            old_tokens,
            old_source_length,
            new_source,
            new_tokens,
            locations,
            lines,
            false,
        )
    })
}

fn rebase_span_values(
    value: &mut serde_json::Value,
    locations: &SourceLocationMap,
    old_source_length: usize,
    new_source: &str,
    lines: &SourceLineIndex,
    span_count: &mut usize,
) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            let span_fields = ["start", "end", "line", "column"];
            let is_source_span = object.len() == span_fields.len()
                && span_fields.iter().all(|field| {
                    object
                        .get(*field)
                        .and_then(serde_json::Value::as_u64)
                        .is_some()
                });
            if is_source_span {
                let read_usize = |field: &str| {
                    object
                        .get(field)
                        .and_then(serde_json::Value::as_u64)
                        .map(|number| number as usize)
                };
                let Some(start) = read_usize("start") else {
                    return false;
                };
                let Some(end) = read_usize("end") else {
                    return false;
                };
                let Some(line) = read_usize("line") else {
                    return false;
                };
                let Some(column) = read_usize("column") else {
                    return false;
                };
                let Some(span) = locations.rebase_span(
                    SourceSpan {
                        start,
                        end,
                        line,
                        column,
                    },
                    old_source_length,
                    new_source,
                    lines,
                ) else {
                    return false;
                };
                object.insert("start".to_owned(), serde_json::json!(span.start));
                object.insert("end".to_owned(), serde_json::json!(span.end));
                object.insert("line".to_owned(), serde_json::json!(span.line));
                object.insert("column".to_owned(), serde_json::json!(span.column));
                *span_count += 1;
                true
            } else {
                object.values_mut().all(|nested| {
                    rebase_span_values(
                        nested,
                        locations,
                        old_source_length,
                        new_source,
                        lines,
                        span_count,
                    )
                })
            }
        }
        serde_json::Value::Array(values) => values.iter_mut().all(|nested| {
            rebase_span_values(
                nested,
                locations,
                old_source_length,
                new_source,
                lines,
                span_count,
            )
        }),
        _ => true,
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
    fn parse_cache_reuses_comment_only_edits_after_restart_with_fresh_locations() {
        let source =
            "mncs 0.19;\nmodule cache.comment;\nfn value() -> (result: u64) { return 9; }\n";
        let original = SourceEnvelope::inline(SourceArtifactKind::Program, "cache.comment", source);
        let mut first_cache = SourceParseCache::default();
        let mut first_observation = SourceParseCacheObservation::default();
        let first = first_cache.parse(&original, &mut first_observation);
        assert!(first.is_valid(), "{:?}", first.diagnostics);

        let bytes = serde_json::to_vec(&first_cache).expect("cache serializes");
        let mut restarted_cache: SourceParseCache =
            serde_json::from_slice(&bytes).expect("cache restores after process restart");
        let changed = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "cache.comment",
            format!(
                "// π comment before the header\nmncs 0.19;\nmodule cache.comment;\n/* comment before function */\nfn value() -> (result: u64) {{ /* inside */ return 9; }}\n// trailing comment\n"
            ),
        );
        let mut observation = SourceParseCacheObservation::default();
        let reused = restarted_cache.parse(&changed, &mut observation);
        let freshly_parsed = mncs_syntax::parse(&changed);

        assert_eq!(observation.reused_source_identities, Vec::<String>::new());
        assert_eq!(
            observation.reused_same_significant_tokens_source_identities,
            [changed.identity.clone()]
        );
        assert!(observation.parsed_source_identities.is_empty());
        assert_eq!(observation.published_entry_count, 0);
        assert_eq!(reused, freshly_parsed);
        assert_eq!(reused.lexical.reconstruct(), changed.text);
        assert_eq!(reused.cst.reconstruct(), changed.text);
        assert_eq!(reused.cst.source_identity, changed.identity);
        assert_eq!(reused.cst.source_length, changed.text.len());
        assert_eq!(reused.cst.tokens, reused.lexical.tokens);
        assert_ne!(
            reused.ast.as_ref().expect("valid AST").span,
            first.ast.as_ref().expect("valid original AST").span
        );
    }

    #[test]
    fn parse_cache_does_not_reuse_significant_tokens_across_profile_assumptions() {
        let mut original = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "cache.profile",
            "mncs 0.19; module cache.profile; fn value() -> (result: u64) { return 9; }",
        );
        let mut cache = SourceParseCache::default();
        let mut original_observation = SourceParseCacheObservation::default();
        let original_output = cache.parse(&original, &mut original_observation);
        assert!(original_output.is_valid());

        original.language_version = "0.18".to_owned();
        original.seal();
        let mut observation = SourceParseCacheObservation::default();
        let changed_assumption = cache.parse(&original, &mut observation);
        let freshly_parsed = mncs_syntax::parse(&original);

        assert_eq!(changed_assumption, freshly_parsed);
        assert!(!changed_assumption.is_valid());
        assert!(observation
            .reused_same_significant_tokens_source_identities
            .is_empty());
        assert_eq!(observation.parsed_source_identities, [original.identity]);
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
        assert!(changed_observation
            .reused_same_significant_tokens_source_identities
            .is_empty());
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
            source_envelope_schema_version: &entry.source_envelope_schema_version,
            language_profile: &entry.language_profile,
            artifact_kind: entry.artifact_kind,
            parser_policy_identity: &entry.parser_policy_identity,
            profile_registry_identity: &entry.profile_registry_identity,
            lexical_schema_version: &entry.lexical_schema_version,
            cst_schema_version: &entry.cst_schema_version,
            ast_schema_version: &entry.ast_schema_version,
            significant_tokens_digest: &entry.significant_tokens_digest,
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

    #[test]
    fn parse_cache_reuses_comment_only_edit_in_digest_app_root_source() {
        // Regression fixture from mncs-test digest.mncs at source SHA-256
        // fa45534cdebb4de683675bfb70eed15cfa177a5198f036c0e3b7fcab68b91c30.
        let source = include_str!("../tests/fixtures/digest-app-root.mncs");
        let original =
            SourceEnvelope::inline(SourceArtifactKind::Program, "mncs.test.digest", source);
        let mut cache = SourceParseCache::default();
        let mut cold_observation = SourceParseCacheObservation::default();
        let previous = cache.parse(&original, &mut cold_observation);
        assert!(previous.is_valid(), "{:?}", previous.diagnostics);

        let changed = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "mncs.test.digest",
            format!("// π root-only source location refresh\n{source}\n// trailing comment\n"),
        );
        let mut observation = SourceParseCacheObservation::default();
        let reused = cache.parse(&changed, &mut observation);
        let freshly_parsed = mncs_syntax::parse(&changed);

        assert!(reused.is_valid(), "{:?}", reused.diagnostics);
        assert_eq!(reused, freshly_parsed);
        assert_eq!(
            observation.reused_same_significant_tokens_source_identities,
            [changed.identity.clone()]
        );
        assert!(observation.reused_source_identities.is_empty());
        assert!(observation.parsed_source_identities.is_empty());
        assert_eq!(observation.published_entry_count, 0);
        assert_eq!(reused.lexical.reconstruct(), changed.text);
        assert_eq!(reused.cst.reconstruct(), changed.text);
        assert_eq!(reused.cst.source_identity, changed.identity);
        assert_eq!(reused.cst.source_length, changed.text.len());
        assert_eq!(reused.cst.tokens, reused.lexical.tokens);
        assert_ne!(
            reused.ast.as_ref().expect("valid AST").functions[0].span,
            previous.ast.as_ref().expect("valid original AST").functions[0].span
        );
    }
}
