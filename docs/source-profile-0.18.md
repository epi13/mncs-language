# Source Profile 0.18 — typed structured artifacts

Status: **implemented, experimental (current)**. Profile 0.18 is additive
over Profile 0.17. Profiles 0.1–0.17 remain sealed.

## Purpose

Profile 0.18 gives an MNCS application one generic external artifact boundary:

```text
bounded bytes → schema/interface checks → typed nominal MNCS value
typed nominal MNCS value → bounded bytes → atomically published artifact
```

The runtime owns file access and JSON transport. The linked MNCS program owns
the record, finite, scalar, and bounded-sequence contract. Applications do
not receive an untyped dynamic JSON tree and no `actions_json_decode`-style
operation exists.

## Intrinsics

```text
structured_read(path, schema)
structured_write(path, schema, value)
```

Both operations require the enclosing function to declare the matching
capability and effect. `path` is a UTF-8 relative path of normal components
under the explicit granted filesystem root; absolute paths, `..`, `.`, NUL,
backslashes, missing roots, and final-component symlinks fail closed.

`structured_read` requires an expected concrete result type. A generic
envelope must have exactly the requested `schema_version`, the expected
nominal `interface_identity`, and a typed `value`; envelope payloads are
checked against the linked declarations, including exact field/variant sets
and bounded sequence lengths. Contract-owned external documents may expose a
forward-compatible wire view: the requested schema revision and every
declared field are still required and typed, while unknown fields are not
silently treated as part of the native identity until that contract publishes
a complete view. Malformed or mismatched documents are `InvalidRequest`.

`structured_write` emits the same deterministic envelope. It validates the
typed value before publication, uses a bounded temporary file followed by an
atomic replace, and synchronizes the file and containing directories. Record
policy validates and sizes the document without mutating the filesystem;
realize policy publishes it.

The artifact payload is capped at 4 MiB, nesting at 64 levels, collections at
8192 members, strings at 1 MiB, paths at 1024 bytes, and schema labels at 64
bytes. These are transport ceilings; larger data belongs behind a different
bounded resource abstraction rather than an unbounded source value.

## Identity

The interface identity is derived from the expected `BodyType`'s canonical
nominal identity. Consequently a changed record field type or nominal module
identity cannot be accepted merely because a host supplied a self-consistent
JSON projection. The artifact schema label remains an external revision
selected by the owning contract.

`structured_digest(value)` uses the same type-directed external contract
projection as `structured_write`: record keys are canonical JSON object keys,
finite values use their external variant spelling, and byte sequences use
the declared bound (`[byte; 32]` is lower-case hexadecimal; other bounded
byte views use UTF-8 text or a numeric byte array). The digest is SHA-256 of
the compact UTF-8 JSON projection. This keeps native identity material and
the external artifact identity byte-for-byte identical; the tagged internal
`ExecutionValue` representation is never hashed as a wire contract.

## Evolution

The generic codec is a language/runtime mechanism. Commons-owned contracts
such as verification plans, test results, receipts, and evidence manifests
must reuse it; they must not define application-specific decoders.
