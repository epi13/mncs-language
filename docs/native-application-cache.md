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

Storage is disposable and bounded automatically on publication. Each cache
root retains at most 2 GiB of unique payload data and records, 128 admission
entries, and 14 days of unused entries. Successful reads refresh recency.
The limits can be set with `MNCS_NATIVE_APPLICATION_CACHE_MAX_BYTES`,
`MNCS_NATIVE_APPLICATION_CACHE_MAX_ENTRIES`, and
`MNCS_NATIVE_APPLICATION_CACHE_MAX_AGE_SECONDS`; invalid settings use the
defaults. An artifact larger than the byte limit still executes and is not
retained. Limits count logical file bytes, so compressed filesystems may use
less physical space. Corrupt records, unrelated files, symlinks, and entries
containing additional local files are preserved rather than reclassified as
disposable data.

Consumers that isolate toolchains in sibling 16-digit hexadecimal namespace
directories can explicitly set `MNCS_NATIVE_APPLICATION_CACHE_STORAGE_ROOT`
to their common parent. These namespaces then share one byte/entry budget,
one process lock, and one payload pool. Only an immediate parent of the
selected namespace is accepted; this setting cannot widen cleanup to an
arbitrary ancestor. Test supplies this scope for its inferred cache, so more
worktrees or toolchain namespaces do not multiply the storage allowance.

Different admission keys that produce byte-for-byte identical complete
artifact envelopes share a payload through hard links where supported.
Every key and its admission checks remain distinct. The payload hash covers
the complete envelope, including contracts and metadata; matching embedded
bytecode alone never authorizes reuse. Filesystems without hard-link support
retain independent bounded copies.

Readers and publishers coordinate with an operating-system file lock that
is released on process exit. A publication atomically exposes its record
and payload together. The next writer removes incomplete pending
publications after obtaining exclusive access. Eviction waits for readers
to finish loading and validating an artifact; loaded sessions retain their
own artifact data. Existing cache records remain readable and are subject
to the same retention limits on subsequent publication.
