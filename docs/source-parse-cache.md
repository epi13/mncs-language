# Language source parse cache

`mncs.language-source-parse-cache/3` lets an embedding build service retain
complete parse outputs between Language frontend calls. The cache belongs to
the caller and can be serialized across process restarts; Language remains
responsible for deciding whether a saved parse can be reused.

An entry matches only when the parser inputs match: source text, declared
language profile, source-envelope schema, artifact kind, parser policy,
lexical/CST/AST schema versions, and source-profile registry fingerprint.
Envelope identity also includes provenance, relationships, origin, and logical
name, none of which affect parsing. When those fields change while parser
inputs remain the same, Language reuses the parse and rewrites the source
identity on the returned lexical, CST, and AST artifacts. For changed source
text, Language freshly lexes and compares every non-trivia token. If the token
stream and all parser assumptions are unchanged, it reuses the saved syntax
tree while rebuilding current lexical tokens, CST token indexes, and AST and
diagnostic source spans. Span rebasing recognizes the four numeric
`SourceSpan` coordinates, so AST fields named `start` and `end` that hold
expressions are traversed as ordinary AST values. Empty source points map to
the next significant token start, the prior token end, or the new end of file;
an unresolved point still falls back to a full parse. Invalid syntax can be
reused only for identical parser inputs and returns the same diagnostics
under the current source identity. A changed parser policy or damaged output
digest causes a fresh parse.

The caller-owned cache is bounded to 256 entries and 32 MiB. Least-recently
used entries are evicted first, including when a restored cache exceeds the
entry limit. Cache-size accounting and parse-output integrity checks stream
their serialized bytes, so they do not allocate a second serialized copy of
each parse result. Per-entry accounting stops at the 32 MiB cache limit; an
oversized parse result is returned normally but is not retained in the cache.
`SourceFrontEndResult::source_parse_cache_reuse` reports exact source
identities reused, syntax-tree reuse after fresh lexing, parsed, rejected,
evicted, and published during that frontend call. These observations describe
parser work; they do not claim semantic verification or executable artifact
reuse.

The parser policy identity is explicit so a behavior change can invalidate
saved parse results even when serialized AST schemas remain compatible. The
cache output digest detects accidental or partial cache corruption. It is an
integrity check for caller-owned cache data, not a cryptographic signature or
authority token.
