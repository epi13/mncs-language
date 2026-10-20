# Language source parse cache

`mncs.language-source-parse-cache/1` lets an embedding build service retain
complete parse outputs between Language frontend calls. The cache belongs to
the caller and can be serialized across process restarts; Language remains
responsible for deciding whether a saved parse can be reused.

An entry matches only when the sealed source envelope identity, parser policy
identity, lexical/CST/AST schema versions, and source-profile registry
fingerprint all match. The source envelope identity covers the exact source
text and its source metadata, so cached spans and diagnostics remain attached
to the same snapshot. A changed source is parsed again. Invalid source may be
cached only after a complete parse and returns its saved diagnostics only for
that exact snapshot. A changed parser policy or damaged output digest causes a
fresh parse.

The caller-owned cache is bounded to 256 entries and 32 MiB. Least-recently
used entries are evicted first. `SourceFrontEndResult::source_parse_cache_reuse`
reports exact source identities reused, parsed, rejected, evicted, and
published during that frontend call. These observations describe parser work;
they do not claim semantic verification or executable artifact reuse.

The parser policy identity is explicit so a behavior change can invalidate
saved parse results even when serialized AST schemas remain compatible. The
cache output digest detects accidental or partial cache corruption. It is an
integrity check for caller-owned cache data, not a cryptographic signature or
authority token.
