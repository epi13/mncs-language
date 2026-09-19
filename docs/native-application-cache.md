# Compiled native-application reuse

`mncs run-app` uses the generic `mncs.compiled-native-artifact-cache/1`
admission path. The cache stores a backend `EmbedArtifact` plus an inspectable
record under a content-addressed key. A hit still opens a fresh execution
session, so cache reuse never becomes a semantic authority or a session that
can leak grants between invocations.

The key binds:

- source content and canonical source locator;
- recursively hashed library contents;
- compiler identity and compiler-inventory identity;
- language profile and backend identity;
- capability/grant identity; and
- optional exported-interface identity.

Changing any input selects a new slot. A missing slot is a normal miss; a
present corrupt, stale, or artifact-identity-mismatched slot fails closed.
`--no-cache` provides an explicit ephemeral path. Callers that launch
descriptors from temporary working directories can share a stable cache with
`--cache-dir PATH` or `MNCS_NATIVE_APPLICATION_CACHE_DIR=PATH`.

`MNCS_NATIVE_CACHE_TRACE=1` reports hit/miss, key, artifact identity, and cache
root on stderr. RAVEL, Actions native applications, Debug native applications,
and the provider applications all use the same launcher and therefore the
same generic cache contract; they do not maintain repository-specific caches.
