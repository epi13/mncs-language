# RFC 0027: Machine-Native Serialization, Canonical Encoding, Schema, Wire Contract, and Data-Evolution Semantics

- **Status:** Draft
- **Area:** Semantic foundation / serialization / schemas / interoperability / evolution
- **Depends on:** RFC 0001, RFC 0002, RFC 0007, RFC 0008, RFC 0009, RFC 0014, RFC 0015, RFC 0018, RFC 0019, RFC 0020, RFC 0021, RFC 0022, RFC 0023, RFC 0024, RFC 0025, RFC 0026
- **Informs:** RFC 0028, RFC 0030, RFC 0031, RFC 0032, RFC 0033, RFC 0034 and later protocol/tooling work

## Abstract

MNCS requires a machine-native theory for transporting, storing, hashing, signing, comparing, evolving, and recovering values across representation boundaries without collapsing semantic values into one preferred byte format.

A conventional serialization interface often appears to be only:

```text
encode(value) -> bytes
decode(bytes) -> value
```

but this hides several independent questions:

- which semantic type and value are being represented;
- which schema identifies the transferable structure;
- which field and variant identities remain stable across versions;
- which wire contract determines tags, ordering, framing, presence, duplicate handling, unknown-field treatment, numeric conversion, text encoding, and resource limits;
- whether multiple legal byte strings represent the same semantic value;
- whether one deterministic/canonical profile is required for hashing, signatures, cache keys, deduplication, or content addressing;
- which round-trip law is required;
- whether an older reader may read, preserve, rewrite, forward, display, or execute a newer message;
- what information is lost during migration;
- whether upgrade and downgrade transformations are total, partial, lossless, reversible, or merely refining;
- whether multiple migration paths commute;
- whether schema identities are linear versions or a branching evolution graph;
- whether a valid parse is also schema-valid, contract-valid, semantically valid, authorized, authentic, or trusted; and
- whether the chosen codec remains safe under bounded resources, malformed input, unknown semantics, and target-specific implementation assumptions.

This RFC establishes explicit semantic objects for those questions.

The central principle is:

> **Semantic value, schema, schema version, wire contract, encoded representation, canonical encoding, framing, and migration are distinct semantic concepts.**

A second constitutional principle is:

> **Successful decoding establishes only that an input can be interpreted under a particular schema and decoding policy. It does not by itself establish semantic validity, compatibility, canonicality, provenance, authenticity, authority, or trust.**

A third is:

> **Schema evolution is a typed transformation problem, not a version-number convention. Compatibility must name its direction, observer, information-preservation requirements, unknown-data policy, permitted losses, and evidence.**

A fourth is:

> **Serialization is an evidenced relation between semantic worlds across a representation boundary, not merely a conversion of values into bytes.**

The RFC deliberately does **not** choose JSON, CBOR, ASN.1, Protocol Buffers, Avro, FlatBuffers, Cap'n Proto, MessagePack, an ABI layout, or any other current format as the MNCS ontology. Such systems are treated as realization families whose properties must map into the MNCS semantic model.

---

## 1. Problem statement

Serialization sits at the boundary between otherwise separate semantic systems:

```text
logical program value
        |
        v
schema projection
        |
        v
wire contract
        |
        v
encoding realization
        |
        v
bytes / records / frames
        |
        v
decoding realization
        |
        v
reader schema
        |
        v
logical reader value
```

Each arrow can preserve, weaken, reinterpret, normalize, discard, or reject information.

Conventional systems frequently hide these transitions behind generated code or library APIs. That can work when all producers and consumers use exactly one version of one codec in one deployment. It becomes dangerous under:

- rolling upgrades;
- long-lived persisted data;
- autonomous code generation;
- multiple languages and runtimes;
- cache/content-address identities;
- cryptographic signatures;
- schema bridges;
- unknown future fields;
- legacy data;
- distributed protocol coexistence;
- data repair;
- migration synthesis;
- recursive optimization; and
- adversarial or malformed inputs.

MNCS is specifically intended to let machines search representation and implementation spaces aggressively. It therefore cannot rely on implicit folklore such as:

```text
JSON is interoperable.
```

or:

```text
The protobuf parser accepted it, therefore it is compatible.
```

or:

```text
We incremented schema_version, therefore migration is ordered.
```

or:

```text
The bytes hash the same, therefore the values are the same.
```

or:

```text
The values are the same, therefore the signatures must be the same.
```

The language needs a typed semantic vocabulary for every relevant distinction.

---

## 2. Non-goals

RFC 0027 does not:

- select one universal wire encoding;
- make source-language type syntax identical to wire-schema syntax;
- make in-memory layout identical to wire layout;
- define transport routing, distributed delivery, consensus, message ordering, or membership, which belong primarily to RFC 0028;
- define live deployment coordination, which belongs to RFC 0030;
- define build/supply-chain artifact trust, which belongs to RFC 0031;
- define general cryptographic algorithms or principal trust, which belong to RFC 0025 and RFC 0018;
- define durable crash-safe execution of a migration, which belongs to RFC 0026;
- define complete quantitative bandwidth/CPU/memory economics, which belong to RFC 0032;
- make every encoding self-describing;
- require a human-readable schema language;
- require every migration to be reversible;
- require every schema evolution to be backward and forward compatible;
- declare unknown fields safe to ignore universally;
- declare deterministic encoding sufficient for semantic canonicality without a profile;
- equate successful parsing with semantic validity;
- equate schema fingerprints with semantic identity; or
- equate bounded corpus testing with universal compatibility proof.

---

## 3. Foundational separation: abstract value versus transfer syntax

ASN.1 provides a useful precedent: abstract values and types are defined separately from transfer encodings, while BER, CER, and DER provide different encoding rules for those values.

MNCS SHOULD preserve the corresponding layers:

```text
SemanticValue<T>
WireSchema<S>
WireContract<W>
EncodingProfile<E>
EncodedArtifact<B>
```

The following must not be silently equated:

```text
LogicalType
    != WireSchema

LogicalValue
    != EncodedBytes

WireSchema
    != EncodingProfile

EncodingProfile
    != FramingContract
```

A logical record:

```text
Customer {
    id: CustomerId,
    name: Text,
}
```

may legally be realized through several wire families while retaining one logical identity.

---

## 4. Semantic serialization relation

Serialization SHOULD be modeled as a relation among:

```text
source logical value
source logical type
wire schema
wire contract
encoding profile
encoded artifact
```

A candidate artifact:

```text
SerializationRealization {
    source_type
    schema
    wire_contract
    encoding_profile
    encoder
    decoder
    obligations
    evidence
}
```

is valid only when it establishes the required relations.

This allows Forge to search serialization realizations without changing the logical program meaning.

---

## 5. Encoder and decoder are not presumed total inverses

A useful mathematical starting point is a partial-isomorphism style model:

```text
encode : Value<T> -> EncodeResult<Bytes>
decode : Bytes -> DecodeResult<Value<T>>
```

Some values may not be encodable under a particular profile.

Some byte strings may be malformed.

Some well-formed byte strings may be schema-invalid.

Some schema-valid byte strings may be semantically invalid under application contracts.

The language MUST therefore not assume ordinary mathematical bijection.

---

## 6. Value round-trip law

A fundamental claim is:

```text
Decode(Encode(v)) ≈ v
```

where `≈` names an RFC 0020 relation.

This relation may be:

- exact logical equality;
- numeric bounded equivalence under RFC 0021;
- normalization equivalence;
- projection equivalence; or
- another declared relation.

The relation MUST be explicit when exact equality does not hold.

Artifact:

```text
ValueRoundTripClaim {
    schema
    encoding_profile
    value_domain
    relation
    witness
    evidence
}
```

---

## 7. Byte round-trip is a different law

It is generally unsafe to require:

```text
Encode(Decode(b)) = b
```

because many formats admit multiple legal encodings of one value.

Instead a canonical profile may support:

```text
EncodeCanonical(Decode(b))
    = Canonicalize(b)
```

This is a different claim.

MNCS MUST distinguish:

```text
ValueRoundTrip
ByteRoundTrip
CanonicalRoundTrip
```

---

## 8. Cross-version round-trip is another distinct law

For mixed schema versions:

```text
v_new
  -> encode_new
  -> decode_old
  -> encode_old
  -> decode_new
```

may lose information even when every individual step succeeds.

MNCS SHOULD therefore represent:

```text
CrossVersionRoundTripClaim {
    source_schema
    intermediate_schema
    destination_schema
    unknown_policy
    preserved_projection
    losses
    relation
}
```

This is especially important for old intermediaries that receive and forward future data.

---

## 9. Decoding dispositions

The decoder SHOULD distinguish at least these conceptual states:

```text
MALFORMED
WELL_FORMED
FORMAT_VALID
SCHEMA_VALID
CONTRACT_VALID
```

The exact executable representation can differ, but these semantic distinctions MUST remain expressible.

CBOR's distinction among well-formed, valid, and application-expected data is a useful precedent.

A parser accepting bytes therefore does not authorize an application to use the resulting value.

---

## 10. Schema validity versus semantic validity

Example:

```text
field age : u8
```

may decode successfully as:

```text
age = 255
```

while an application invariant requires:

```text
age <= 130
```

Thus:

```text
SchemaValid
    != ContractValid
```

Semantic invariants remain owned by RFC 0002/RFC 0007 and related domain semantics.

---

## 11. Canonical encoding is profile-relative

RFC 0027 SHALL support explicit canonical encoding profiles.

Conceptually:

```text
CanonicalEncodingProfile {
    id
    schema_scope
    codec_family
    normalization_rules
    field_order_rules
    numeric_rules
    string_rules
    duplicate_rules
    unknown_field_rules
    framing_scope
    version
}
```

A naked assertion:

```text
canonical = true
```

is insufficient.

The profile identity is part of the claim.

---

## 12. Deterministic encoding is not semantic identity

Two deterministic codec profiles may encode the same logical value differently.

For example:

```text
CanonicalJSON(v) != DeterministicCBOR(v)
```

as bytes while:

```text
DecodeJSON(...) ≈ DecodeCBOR(...)
```

holds semantically.

Therefore:

```text
ByteIdentity
CanonicalEncodingIdentity
SemanticIdentity
```

remain separate RFC 0020 relations.

---

## 13. Canonical profile versioning

Canonicalization rules can evolve.

Changing:

- map ordering;
- integer shortening;
- numeric normalization;
- text normalization;
- treatment of unknown fields; or
- framing scope

may change bytes, hashes, and signatures.

Therefore:

```text
CanonicalProfileV1
    != CanonicalProfileV2
```

unless an explicit relation proves otherwise.

---

## 14. Digest identity must declare its subject

RFC 0027 SHOULD distinguish:

```text
RawEncodingDigest
CanonicalEncodingDigest
SchemaDigest
SemanticProjectionDigest
```

A digest artifact SHOULD carry:

```text
DigestClaim {
    subject_kind
    subject_identity
    canonical_profile?
    algorithm
    digest
}
```

A raw hash string must not imply which identity class it represents.

---

## 15. Exact-byte signatures versus canonical-semantic signatures

RFC 0025 owns signature trust and cryptographic authority.

RFC 0027 must tell it what bytes mean.

The serialization layer SHOULD distinguish:

```text
SignExactBytes
SignCanonicalEncoding<Profile>
```

Signing exact bytes protects exact transport identity.

Signing a canonical encoding can protect one normalized semantic representation.

Neither silently substitutes for the other.

---

## 16. Canonicalization is a transformation

Canonicalization SHOULD produce a first-class transformation artifact:

```text
CanonicalizationResult {
    input_encoding
    output_encoding
    profile
    semantic_relation
    byte_delta
    obligations
    evidence
}
```

It is not an identity assertion merely because output is canonical.

---

## 17. Map ordering is not automatically semantic

A logical map normally has no observable iteration order unless declared.

A deterministic encoder may impose one ordering solely to obtain stable bytes.

Therefore:

```text
Map
    != OrderedMap
```

and canonical key sorting must not silently change a logically ordered structure.

If order is semantically observable, the schema must declare it.

---

## 18. Duplicate-key/field handling is part of the wire contract

Formats and implementations differ on duplicate-key behavior.

RFC 0027 requires an explicit policy:

```text
DuplicateFieldPolicy =
    Reject
    FirstWins
    LastWins
    PreserveAll
```

Canonical MNCS profiles SHOULD default to `Reject` unless the semantic type is explicitly a multimap or equivalent structure.

Security-sensitive contracts SHOULD NOT rely on implementation-specific duplicate handling.

---

## 19. Field identity is not field name

Stable field identity is essential for evolution.

Conceptually:

```text
FieldIdentity {
    semantic_id
    wire_id?
    display_name
    historical_names
}
```

A rename may preserve semantic and wire identity.

A replacement must receive a new semantic identity.

Protocol Buffers' stable numeric field tags are a useful practical precedent.

---

## 20. Wire field identifiers

A tagged wire format MAY assign:

```text
WireFieldId
```

separately from semantic identity.

Changes to a wire ID can be wire-breaking even when a human-facing name remains unchanged.

Therefore:

```text
FieldSemanticIdentity
    != WireFieldIdentity
```

unless contractually bound.

---

## 21. Retired identifiers become tombstones

When a historical field is removed, its identity SHOULD remain reserved:

```text
ReservedFieldIdentity {
    prior_field
    retired_at_schema
    reason
}
```

This prevents historical data from being decoded under a new unrelated meaning.

---

## 22. Reuse of retired wire IDs is forbidden by default

Within one schema lineage/epoch:

> **A retired wire identifier may not silently acquire a different semantic meaning.**

A new schema epoch may deliberately establish a fresh identity space, but that epoch transition must be explicit and incompatible by default.

---

## 23. Variant identities follow the same rule

Sum/variant tags require stable identities too:

```text
VariantIdentity
ReservedVariantIdentity
```

Deleting a variant and reusing its historical tag for a different meaning is forbidden by default.

---

## 24. Presence is first-class

RFC 0027 MUST preserve the distinction:

```text
Absent
Present(v)
```

and, if null is a value:

```text
Present(Null)
```

Thus:

```text
Absent
    != Present(DefaultValue)
    != Present(Null)
```

unless a wire contract deliberately collapses those observations.

---

## 25. Explicit versus implicit presence

Some wire formats treat a default scalar value as indistinguishable from omission.

MNCS SHOULD model:

```text
PresencePolicy =
    Explicit
    ImplicitDefault
    Required
```

A codec that cannot preserve source-level explicit presence is invalid when presence is part of the protected observation model.

---

## 26. Read defaults are migration semantics

A default value used when a source field is absent SHOULD be represented explicitly:

```text
ReadDefault {
    field
    value
    source_condition
}
```

A default does not necessarily mean:

- the field was encoded;
- the writer intended that value;
- the field is optional in source semantics; or
- the value existed historically.

Avro's reader-default behavior is a useful precedent.

---

## 27. Unknown fields are first-class

RFC 0027 SHOULD support:

```text
UnknownFieldSet
```

or equivalent uninterpreted wire fragments.

An older reader may preserve a future field even when it cannot interpret it semantically.

This is different from understanding the field.

---

## 28. Unknown-field policy

A wire/schema contract MUST specify an unknown policy where extensibility is possible:

```text
UnknownFieldPolicy =
    RejectUnknown
    IgnoreUnknown
    PreserveUnknown
    PreserveUnknownButDoNotAuthorize
```

The last profile is important for systems that may store/forward future data without executing it.

---

## 29. Unknown is not invalid

An unknown extension may be:

- well formed;
- valid under the writer schema;
- unknown to the reader;
- preservable;
- unauthorized for local execution.

These states must not collapse.

---

## 30. Ignore is not preserve

A reader that safely ignores unknown fields can still destroy forward data when it rewrites the message.

Therefore RFC 0027 distinguishes:

```text
CanReadNewer
CanPreserveNewer
CanRewriteWithoutLoss
```

as separate compatibility claims.

---

## 31. Format bridges can lose unknown data

A binary tagged format may preserve unknown field identities while a bridge through a text representation discards them.

Such a bridge may satisfy:

```text
Readable
```

while failing:

```text
ForwardRoundTripPreserving
```

The bridge must expose that loss.

---

## 32. Open and closed record schemas

RFC 0027 SHOULD distinguish:

```text
ClosedRecord
OpenRecord
```

A closed record rejects unknown fields.

An open record reserves extension space.

Security-sensitive control artifacts MAY require closed schemas even when telemetry uses open schemas.

---

## 33. Open and closed variants

Similarly:

```text
ClosedVariant
OpenVariant
```

must remain distinct.

An open variant can represent:

```text
UnknownVariant(tag, payload?)
```

whereas a closed variant may reject any unrecognized tag.

---

## 34. Exhaustiveness intent is semantic

Adding a new case to a historically exhaustive closed sum is a semantic compatibility change even if the wire format can carry the new tag.

Therefore:

```text
WireExtensible
```

does not imply:

```text
SemanticallyForwardCompatible
```

---

## 35. Schema evolution is directional

RFC 0027 MUST reject a single universal `compatible` Boolean.

Compatibility has direction:

```text
OldWriter -> NewReader
NewWriter -> OldReader
```

Useful named relations include:

```text
BackwardReadable
ForwardReadable
BidirectionallyReadable
```

but these are only part of the full compatibility family.

---

## 36. Wire compatibility versus semantic compatibility

Two schemas may have wire-compatible fields with changed meaning.

Example:

```text
v1 amount: integer cents
v2 amount: integer dollars
```

The bytes may parse successfully while meaning changes catastrophically.

Therefore:

```text
WireCompatible
    != SemanticCompatible
```

This is a constitutional distinction.

---

## 37. Reader/writer schema resolution

RFC 0027 SHOULD support a model analogous to explicit writer/reader schema resolution:

```text
DecodeRequest {
    bytes
    writer_schema
    reader_schema
    resolution_policy
}
```

The decoder then establishes a transformation from writer semantics to reader semantics instead of merely interpreting bytes under whichever schema happens to be locally loaded.

---

## 38. Writer-schema identity may be external

Some compact wire formats do not carry enough schema information in each message.

Therefore RFC 0027 allows:

```text
SelfDescribingEncoding
ExternallySchematizedEncoding
PartiallySelfDescribingEncoding
```

The choice is a realization tradeoff, not a semantic hierarchy.

---

## 39. Schema references are dependencies

An external schema reference is part of the decoding semantics.

Conceptually:

```text
SchemaRef {
    semantic_identity
    representation_digest?
    trust_context?
}
```

Replacing the referenced schema may change meaning even when payload bytes remain identical.

RFC 0025 and RFC 0031 later govern deeper authentication/provenance of schema artifacts.

---

## 40. Schema and schema language are separate

The canonical MNCS schema model need not be its human source notation.

External schema notations such as:

- ASN.1;
- CDDL;
- `.proto`;
- Avro JSON schemas;
- JSON Schema;
- FlatBuffers schemas

may be imported/exported through RFC 0015 boundaries.

The schema language is a representation of the semantic schema.

---

## 41. Wire schema and logical source type are separate

A logical type may lower to a simpler wire primitive plus semantic conversion.

Example:

```text
Instant<UTC>
```

could encode as:

```text
i64 nanoseconds_since_epoch
```

The reader must reconstruct the temporal type under RFC 0023.

Thus:

```text
LogicalType
    != WireType
```

---

## 42. Numeric encoding reuses RFC 0021

If an exact decimal value is encoded in binary floating point, serialization introduces a numeric approximation.

That is not a purely representational change.

RFC 0027 MUST generate/retain RFC 0021 obligations for:

- rounding;
- overflow;
- underflow;
- precision loss;
- range narrowing;
- exceptional values; and
- reproducibility where relevant.

---

## 43. Numeric text encodings may have consumer limits

A logical `u64` rendered as a JSON number may be syntactically valid JSON while downstream implementations cannot represent it exactly.

A wire contract SHOULD therefore state numeric interoperability assumptions explicitly.

---

## 44. Text and bytes remain distinct

```text
Text
    != ByteString
```

A byte sequence becomes text only through a checked character encoding interpretation.

For UTF-8:

```text
DecodeUtf8(bytes) -> Text | InvalidUtf8
```

must reject invalid sequences under a strict profile.

---

## 45. Unicode normalization is explicit

Text can have multiple code-point sequences that humans perceive as the same grapheme.

RFC 0027 MUST NOT silently normalize unless the encoding/canonical profile declares a normalization form.

Relevant relations remain RFC 0020 concerns:

```text
ByteEqual
CodePointEqual
NormalizedUnicodeEquivalent
```

---

## 46. Framing is not serialization

Serialization answers:

> How is one logical value represented?

Framing answers:

> How are record boundaries represented in a larger byte/record stream?

RFC 0027 SHOULD define:

```text
FramingContract =
    LengthPrefixed
    Delimited
    FixedSize
    SelfDelimiting
    Chunked
    RecordSequence
    Custom
```

or equivalent extensible forms.

---

## 47. Stream semantics are not framing semantics

A logical:

```text
Stream<T>
```

may be framed through several wire strategies.

The stream/productivity semantics belong to RFC 0019/RFC 0022.

RFC 0027 only realizes the representation boundary.

---

## 48. Endianness is a realization parameter

A logical integer can be represented as:

- little-endian fixed width;
- big-endian fixed width;
- variable integer;
- decimal text;
- another target/wire encoding.

Endianness is not part of logical integer identity unless made observable by a wire contract.

---

## 49. Memory layout must not leak into wire layout accidentally

RFC 0027 SHALL reject the assumption:

```text
wire_bytes = memory_bytes(struct)
```

unless RFC 0019 representation requirements explicitly establish that relation.

Compiler padding, ABI alignment, pointer representation, endianness, and uninitialized bytes must not become accidental protocol semantics.

---

## 50. Zero-copy formats narrow realization freedom

A wire format may deliberately align encoded layout with directly readable memory.

That can be a valid optimization.

It must be expressed as an explicit requirement or realization property:

```text
ZeroCopyReadable
DirectMappedWireRepresentation
```

Such requirements narrow future representation and compatibility freedom.

---

## 51. Schema changes are encoding-profile dependent

Field reordering may be harmless in a tagged encoding and breaking in a positional fixed-layout encoding.

Therefore a compatibility claim SHOULD include:

```text
writer_schema
reader_schema
wire_contract
encoding_profile
```

not only two abstract schemas.

---

## 52. Compatibility claim structure

A candidate structure is:

```text
CompatibilityClaim {
    writer_schema
    reader_schema
    wire_profile

    direction
    operation_scope
    preserved_observations
    unknown_policy
    permitted_losses

    assumptions
    witness
    evidence
}
```

`operation_scope` may distinguish:

```text
READ
STORE
FORWARD
REWRITE
DISPLAY
EXECUTE
```

This is crucial: data may be safe to preserve but unsafe to execute.

---

## 53. Compatibility is action-relative

An older component may be allowed to:

```text
STORE newer_message
FORWARD newer_message
```

while forbidden to:

```text
EXECUTE newer_message
```

because unknown fields may carry semantics relevant to authorization or safety.

RFC 0018 can make the final action-relative decision.

---

## 54. Migration is explicit

A schema edge SHOULD be represented by a migration artifact:

```text
Migration {
    source_schema
    destination_schema
    transform
    domain
    codomain
    assumptions
    preserved_relations
    losses
    obligations
    evidence
}
```

Schema version metadata alone is insufficient.

---

## 55. Migration property family

MNCS SHOULD distinguish:

```text
TotalMigration
PartialMigration
LosslessMigration
LossyMigration
ReversibleMigration
InjectiveMigration
SurjectiveMigration
RefiningMigration
```

where useful.

These are not one ordered ladder.

---

## 56. Migration examples

A widening:

```text
u32 -> u64
```

can be total and lossless over the source domain.

A narrowing:

```text
u64 -> u32
```

is partial unless accompanied by a bound.

A transformation:

```text
full_name -> first_name
```

may be total but lossy.

The schema system must expose the difference.

---

## 57. Upgrade and downgrade are separate functions

RFC 0027 SHALL NOT assume:

```text
downgrade = inverse(upgrade)
```

without proof.

When a new schema contains information absent from the old schema, downgrade necessarily loses or externally stores that information unless another mechanism is declared.

---

## 58. Bidirectional transformation laws

Where both directions exist, lens-like laws are useful:

```text
downgrade(upgrade(old)) ≈ old
```

and possibly:

```text
upgrade(downgrade(new)) ≈ new
```

under explicit RFC 0020 relations.

The second law often intentionally fails when the older schema cannot represent new information.

---

## 59. Migration loss is first-class

A migration SHOULD identify loss explicitly:

```text
MigrationLoss {
    source_component
    lost_observation
    reason
    recoverable?
}
```

or a preserved projection:

```text
PreservedProjection
```

This enables action-relative policies to reject a migration for one purpose but permit it for another.

---

## 60. Schema aliases encode historical relations

A field/type alias SHOULD be represented as history:

```text
HistoricalNameBinding {
    historical_name
    current_identity
    validity_scope
}
```

A textual rename can preserve semantic identity.

Equal text does not prove equal identity.

---

## 61. Rename versus replacement

Schema deltas SHOULD distinguish operations such as:

```text
Rename
Replace
Split
Merge
Add
Remove
Deprecate
Reserve
Widen
Narrow
Refine
Open
Close
```

These are semantic change kinds, not merely text diffs.

---

## 62. Deprecation is not deletion

A deprecated field may stop being produced while remaining historically decodable.

Its field identity/tombstone remains part of schema history.

Deprecation must not free the identity for unrelated reuse.

---

## 63. Schema evolution is a graph

RFC 0027 SHOULD model schema lineage as a DAG or other explicit graph rather than one scalar counter.

Example:

```text
        v2a
       /   \
     v1     v4
       \   /
        v2b
```

Artifact:

```text
SchemaVersion {
    id
    schema_identity
    parents
    migration_edges
}
```

A display version number may exist, but it is not the topology.

---

## 64. Migration path coherence

When two supported migration paths reach one target schema, MNCS SHOULD be able to ask:

```text
m2a4(m12a(x))
    ≈
m2b4(m12b(x))
```

under a declared relation.

Artifact:

```text
MigrationPathCoherenceClaim {
    source
    destination
    path_a
    path_b
    relation
    witness
    evidence
}
```

---

## 65. Direct migration optimization

Forge may synthesize:

```text
v1 -> v4
```

instead of executing:

```text
v1 -> v2 -> v3 -> v4
```

but the optimized path requires a preservation/refinement claim against the declared historical semantics.

Speed cannot authorize semantic divergence.

---

## 66. Mixed-version populations are normal

A storage system may legitimately contain:

```text
records under schema v1
records under schema v2
records under schema v3
```

RFC 0027 SHOULD support:

```text
SchemaPopulation {
    readable_versions
    writable_versions
    preferred_write_version
    preservation_requirements
}
```

Lockstep migration is a stronger special case.

---

## 67. Rolling coexistence is the default assumption

Components in real systems are not generally upgraded simultaneously and may be rolled back.

RFC 0027 SHOULD treat mixed-version coexistence as the normal compatibility problem.

A claim requiring lockstep deployment MUST declare that assumption.

RFC 0030 will later own the deployment mechanics.

---

## 68. Compatibility envelope

A component MAY publish:

```text
SchemaCompatibilityEnvelope {
    reads
    writes
    preserves_unknown_from
    canonical_profiles
    migration_paths
    operation_restrictions
}
```

This is more useful than a lone protocol integer.

---

## 69. Version negotiation is not compatibility proof

Peers agreeing on:

```text
protocol = 3
```

is a runtime negotiation event.

It does not prove their exact schemas, canonical profiles, field meanings, or unknown-field behaviors are compatible.

RFC 0028 may use RFC 0027 compatibility claims during protocol negotiation.

---

## 70. Same wire type with narrower semantic domain

Example:

```text
v1 count: i32
v2 count: i32 where 0 <= count <= 100
```

Wire representation may be unchanged.

Old values can violate new semantic constraints.

Thus:

```text
WireCompatible
```

may hold while:

```text
SemanticBackwardCompatible
```

fails.

---

## 71. Semantic widening with wire breakage

Conversely:

```text
u32 -> u64
```

may be semantically lossless while a fixed-width positional format becomes wire-incompatible.

Again, semantic and wire relations are independent.

---

## 72. Compatibility counterexamples

A failed compatibility claim SHOULD produce a scoped counterexample where feasible:

```text
SchemaCounterexample {
    source_schema
    destination_schema
    source_value_or_wire_case
    failed_relation
    failed_observation
    path
}
```

This supports localized Forge repair.

---

## 73. Witness families

Compatibility/migration evidence may come from:

```text
StructuralSchemaWitness
FieldIdentityMappingWitness
MigrationProof
RoundTripProof
SMTCertificate
ReaderWriterSimulation
BoundedCorpusObservation
DifferentialCodecObservation
ExternalCertificate
```

The witness type does not alter the relation being claimed.

---

## 74. Bounded corpora remain bounded evidence

Testing millions of historical messages is useful evidence.

It does not prove universal compatibility unless the relevant domain is exhausted.

RFC 0027 SHOULD preserve:

```text
HistoricalCorpusCompatibility
```

separately from:

```text
UniversalCompatibility
```

through RFC 0018/RFC 0020 semantics.

---

## 75. Decoder ambiguity is a security concern

Security-sensitive profiles SHOULD minimize cases where different compliant implementations derive different values from the same bytes.

Risks include:

- duplicate keys;
- non-strict Unicode decoding;
- numeric range ambiguity;
- unknown-field semantics;
- map ordering assumptions;
- unbounded nesting;
- parser extension behaviors.

The wire contract must identify which interpretations are legal.

---

## 76. Decode budgets

Decoders SHOULD accept explicit bounded authority/resource contracts:

```text
DecodeBudget {
    max_bytes
    max_depth
    max_fields
    max_elements
    max_string_units
    max_allocations?
    max_semantic_steps?
}
```

Detailed quantitative semantics belong to RFC 0032.

The bounded-execution distinction from RFC 0022 applies.

---

## 77. Budget exhaustion is not malformed input

If valid data exceeds the current decode authority:

```text
DECODE_BUDGET_EXCEEDED
```

must remain distinct from:

```text
MALFORMED_ENCODING
```

The former says processing was not authorized/completed under this bound.

It does not prove the input invalid.

---

## 78. Decoder failure taxonomy

Diagnostics SHOULD distinguish at least:

```text
MalformedEncoding
EncodingProfileViolation
CanonicalityViolation
SchemaMismatch
UnknownFieldRejected
DuplicateFieldRejected
MigrationUnavailable
MigrationPartialForValue
MigrationLossForbidden
SemanticInvariantFailed
DecodeBudgetExceeded
UnsupportedWireFeature
```

These correspond to different repair actions.

---

## 79. Decodable and canonical are separate

A decoder may successfully parse a noncanonical but valid representation.

Therefore:

```text
Decodable
    != Canonical
```

A signature policy may require canonicality even when an ordinary storage reader does not.

---

## 80. Unknown preservation can conflict with canonicalization

An older implementation may preserve raw unknown fields without understanding how a newer semantic canonical profile would normalize them.

Therefore:

```text
UnknownPreserving
```

and:

```text
AuthorizedCanonicalReencoder
```

are separate claims.

A component may safely forward opaque future fields but be unauthorized to produce a canonical signed representation of them.

---

## 81. Security schemas may be closed by policy

For artifacts such as:

```text
DeploymentAuthorization
CapabilityGrant
AttestationResult
PromotionDecision
```

unknown fields may carry future authority semantics.

An older verifier ignoring them could become unsafe.

RFC 0027 SHOULD support closed-schema policies for such artifacts.

---

## 82. Information-flow labels survive serialization

RFC 0024 applies across the wire boundary.

Serializing:

```text
Secret<T>
```

into bytes does not declassify it.

The encoded artifact remains subject to confidentiality/integrity policy.

---

## 83. Serialization metadata can leak

RFC 0024 observer models may include:

```text
message_length
field_presence
field_order
variant_tag
schema_version
compression_ratio
unknown_field_count
framing_pattern
```

A wire encoding can therefore create side channels even when field contents are encrypted later.

---

## 84. Field omission can carry information

If a secret controls whether a field is present, presence itself is an observable channel.

RFC 0027 must preserve field-presence semantics so RFC 0024 can reason about this influence.

---

## 85. Compression is a separate transformation

RFC 0027 SHALL keep:

```text
Serialize
Compress
Encrypt
Authenticate
Frame
Transport
```

separate.

A pipeline may compose them, but their relations and obligations differ.

---

## 86. Encryption is not serialization

Encryption changes confidentiality properties under cryptographic assumptions.

It does not define schema meaning.

It also does not automatically declassify the serialized object, per RFC 0024.

---

## 87. Compression and canonical identity

Canonical uncompressed bytes may map to many compressed byte sequences depending on compressor/profile/options.

A signature or content identity must specify whether it covers:

- canonical pre-compression bytes;
- exact compressed bytes; or
- another defined projection.

---

## 88. Nested wire contracts

Frames may themselves be structured values:

```text
Frame {
    schema_ref
    payload_length
    checksum
    payload
}
```

The frame schema and payload schema may have independent identities and canonical profiles.

Nested serialization contracts are permitted.

---

## 89. Recursive schemas

Recursive logical data and schemas are legal.

The schema may be well founded as a type definition while a malicious encoded value is arbitrarily deep.

RFC 0027 therefore keeps:

```text
RecursiveSchemaValidity
```

separate from bounded decoding/termination/resource claims under RFC 0022/RFC 0032.

---

## 90. Streaming decoders

One schema may have several decoding realizations:

```text
WholeBufferDecoder
IncrementalDecoder
ZeroCopyDecoder
EventDecoder
```

They SHOULD establish an RFC 0020 relation such as:

```text
DecodeEquivalent
```

for the protected observations.

---

## 91. Partial/projection decoding

A consumer MAY request only part of a message:

```text
ProjectionDecode {
    schema
    requested_projection
    validation_scope
}
```

Such a decoder MUST NOT claim whole-message semantic validation unless it actually establishes it.

---

## 92. Validation coverage is explicit

A decode result may report:

```text
validated_projection
unvalidated_projection
opaque_unknown_projection
```

This allows RFC 0018 to gate actions according to the actual coverage.

---

## 93. Schema artifacts are machine-native values

Schemas themselves SHOULD have:

- semantic identity;
- canonical semantic representation;
- version lineage;
- dependencies;
- evidence;
- compatibility relations;
- canonical serialization.

Human schema syntax is secondary.

This allows the compiler, Forge, and verifiers to reason about schemas with the same semantic graph machinery used for programs.

---

## 94. Forge serialization-realization search

A logical contract may leave wire realization unresolved:

```text
SerializationRequirement {
    logical_type
    lossless = true
    unknown_fields = preserve
    canonical_signing = required
    rolling_compatibility = 3 generations
    encoding = ?
}
```

Forge may consider:

- canonical CBOR-like encoding;
- tagged protobuf-like binary;
- Avro-like compact binary;
- generated fixed layout;
- text encoding;
- custom bit-packed format;
- zero-copy representation.

Candidates remain untrusted until required relations are established.

---

## 95. Faster is not semantically better

If a candidate:

```text
codec A
```

drops unknown fields while:

```text
codec B
```

preserves them, then A is invalid when `ForwardRoundTripPreserving` is required regardless of throughput advantage.

RFC 0032/0033 may optimize only among admissible candidates.

---

## 96. Compact is not canonical

A compact encoding may permit multiple byte representations of the same value.

If the contract requires stable canonical digests, the codec must either:

- implement a canonical profile; or
- be rejected for that realization.

---

## 97. Codec presence semantics constrain realization

If logical semantics distinguish:

```text
Absent
Present(0)
```

then a codec that collapses those states is not a valid lossless realization.

Forge must either select another encoding or record an explicit weakening permitted by the contract.

---

## 98. Schema migration and durable migration execution remain separate

RFC 0027 defines the logical migration:

```text
Migrate<V1,V2>
```

RFC 0026 defines whether rewriting durable state using that migration is:

- atomic;
- durable;
- recoverable;
- crash safe.

A correct migration function can still be executed unsafely.

---

## 99. Lazy/virtual migration

A schema upgrade need not materialize all stored values immediately.

RFC 0027 SHOULD allow:

```text
ReadProjectionMigration
VirtualMigration
MaterializedMigration
```

as realization choices.

Their semantic relation to the target view must remain explicit.

---

## 100. Migration across mixed durable generations

During rolling migration, persistent state may contain multiple schema generations.

A durable layer MAY publish:

```text
ReadableSchemaSet
WritableSchemaSet
PreferredSchema
```

while RFC 0027 provides the transforms and RFC 0026 provides the crash-safe state transition.

---

## 101. Schema identity model

A likely canonical object is:

```text
Schema {
    id
    logical_type_ref
    structure
    openness
    presence_model
    fields
    variants
    constraints
    metadata
}
```

The exact initial executable subset SHOULD remain smaller.

---

## 102. Field identity model

```text
SchemaField {
    semantic_id
    name
    wire_id?
    type
    presence
    default?
    historical_aliases
    status
}
```

where status may include:

```text
Active
Deprecated
Reserved
```

---

## 103. Schema-version model

```text
SchemaVersion {
    id
    schema
    parents
    created_by
    migration_edges
}
```

Version topology remains explicit.

---

## 104. Wire-contract model

```text
WireContract {
    id
    schema
    field_identity_mode
    order_policy
    duplicate_policy
    unknown_policy
    presence_policy
    numeric_policy
    text_policy
    framing?
    resource_contract?
}
```

---

## 105. Encoding-profile model

```text
EncodingProfile {
    id
    wire_contract
    codec_family
    parameters
    deterministic?
    canonical_profile?
    target_constraints?
}
```

Codec-family names are realizations, not semantic truth.

---

## 106. Canonical profile model

```text
CanonicalEncodingProfile {
    id
    encoding_profile
    normalization_rules
    map_order
    field_order
    integer_encoding
    floating_encoding
    text_normalization
    unknown_handling
    duplicate_handling
    version
}
```

---

## 107. Migration model

```text
Migration {
    id
    source_schema
    destination_schema
    transform
    source_domain
    preserved_relations
    loss_set
    assumptions
    witness
    evidence
}
```

---

## 108. Compatibility-envelope model

```text
CompatibilityEnvelope {
    subject
    readable_schemas
    writable_schemas
    preservable_unknown_schemas
    canonical_profiles
    permitted_operations
    migration_refs
    assumptions
    evidence
}
```

---

## 109. Relation family

RFC 0027 SHOULD define or instantiate RFC 0020 relations including:

```text
ValueRoundTrip
ByteRoundTrip
CanonicalRoundTrip
WireReadable
WireWritable
SchemaResolvable
BackwardReadable
ForwardReadable
BidirectionallyReadable
UnknownPreserving
RewriteWithoutLoss
LosslessMigration
MigrationRefinement
MigrationPathCoherent
CanonicalEquivalent
DecodeEquivalent
```

No universal relation named merely `Compatible` should be sufficient for machine decisions.

---

## 110. Serialization counterexamples

Counterexamples SHOULD remain relation-scoped:

```text
SerializationCounterexample {
    relation
    schema_pair
    encoding_profile
    source_value?
    source_bytes?
    observed_result
    expected_relation
    explanation
}
```

A byte mismatch does not refute semantic equivalence if byte identity was not protected.

---

## 111. Serialization deltas

Forge SHOULD be able to compare candidate changes through:

```text
SerializationDelta {
    schema_changes
    wire_changes
    canonical_changes
    compatibility_changes
    migration_changes
    unknown_preservation_changes
    loss_changes
    evidence_changes
}
```

A representation optimization that changes canonical bytes is therefore visible even if decoded values remain equal.

---

## 112. Security-sensitive deltas

A candidate SHOULD flag changes that:

- move open schema to closed or vice versa;
- begin ignoring previously rejected unknown fields;
- alter canonical-signing scope;
- change duplicate handling;
- alter field presence;
- introduce numeric narrowing;
- change schema-reference trust assumptions; or
- lose unknown fields through a bridge.

These are potential authority/security changes, not ordinary performance tweaks.

---

## 113. Proof-producing codec verification

A codec generator MAY produce evidence for:

```text
ValueRoundTrip
NoDuplicateAcceptance
StrictUtf8
CanonicalEncoding
UnknownPreservation
BoundedDecode
```

Proof search/generation may remain outside the trusted kernel.

The resulting certificates should be independently checkable where practical.

---

## 114. Generated codecs are untrusted producers

Forge, LLMs, code generators, optimization passes, parser generators, and external schema compilers may all generate codecs.

Their generated output is not trusted merely because generation succeeded.

They must satisfy the declared codec obligations.

---

## 115. HIR/SSA preservation

Where serialization operations appear in program IR, lowering SHOULD preserve identities for:

- schema;
- wire contract;
- canonical profile;
- migration;
- compatibility obligation;
- numeric conversion;
- decode budget; and
- information-flow policy.

A backend optimization may not silently weaken these contracts.

---

## 116. Backend promise discipline

If a backend exposes fast-path assumptions such as:

```text
aligned load
native endianness
unchecked UTF8
trusted input
known field order
```

MNCS may emit/use them only when current evidence establishes the required conditions.

Otherwise it must use a conservative path.

---

## 117. Foreign codec boundaries

RFC 0015 governs foreign libraries and generated external runtimes.

RFC 0027 SHOULD require imported codec claims to state:

- exact library/runtime version;
- schema/profile version;
- relevant decoding options;
- unknown/duplicate handling;
- canonical mode;
- target assumptions; and
- evidence applicability.

Changing these dependencies can invalidate serialization evidence.

---

## 118. Reference theories and standards

RFC 0027 draws from, but is not limited to:

### ASN.1 and X.690

Useful for the abstract-syntax versus transfer-syntax separation and for understanding canonical restrictions layered on a more permissive encoding family.

Reference:

- ITU-T X.690, ASN.1 encoding rules: <https://www.itu.int/rec/T-REC-X.690>

### Partial isomorphisms / invertible syntax descriptions

Useful for deriving parser/printer relations and formalizing round-trip laws without assuming a total bijection.

Reference:

- Rendel and Ostermann, *Invertible Syntax Descriptions*: <https://www.informatik.uni-marburg.de/~rendel/unparse/>

### CBOR

Useful for separating generic data-model validity, well-formed encodings, and deterministic encoding profiles.

Reference:

- RFC 8949: <https://www.rfc-editor.org/rfc/rfc8949.html>

### JSON Canonicalization Scheme

Useful for canonical byte production intended for hashing/signing.

Reference:

- RFC 8785: <https://www.rfc-editor.org/rfc/rfc8785.html>

### CDDL

Useful as precedent for a schema notation independent of the wire encoding itself.

Reference:

- RFC 8610: <https://www.rfc-editor.org/rfc/rfc8610.html>

### JSON

Useful both as a widespread text realization and as evidence for ambiguity/interoperability issues such as duplicate object names and implementation-dependent numeric limits.

Reference:

- RFC 8259: <https://www.rfc-editor.org/rfc/rfc8259.html>

### UTF-8

Useful for strict byte-to-text validity and security implications of accepting illegal encodings.

Reference:

- RFC 3629: <https://www.rfc-editor.org/rfc/rfc3629.html>

### Protocol Buffers

Useful for stable field-number identity, prohibition on field-number reuse, explicit presence, unknown-field preservation, and rolling mixed-version operation.

References:

- Editions language guide: <https://protobuf.dev/programming-guides/editions/>
- Field presence: <https://protobuf.dev/programming-guides/field_presence/>
- Compatibility best practices: <https://protobuf.dev/best-practices/dos-donts/>

### Apache Avro

Useful for explicit writer-schema/reader-schema resolution, aliases, defaults, schema canonical forms, fingerprints, and schema evolution.

Reference:

- Avro specification: <https://avro.apache.org/docs/1.12.0/specification/>

### FlatBuffers and zero-copy schema evolution

Useful as a realization precedent showing how evolution constraints depend on a wire/layout strategy.

Reference:

- FlatBuffers evolution: <https://flatbuffers.dev/evolution/>

These sources inform the design but do not define the MNCS ontology.

---

## 119. Seven/eight semantic planes

RFC 0027 organizes the problem into eight conceptual planes.

### 119.1 Semantic-value plane

```text
LogicalType
LogicalValue
Presence
Open/Closed structure
```

### 119.2 Schema plane

```text
Schema
SchemaVersion
SchemaLineage
FieldIdentity
VariantIdentity
ReservedIdentity
```

### 119.3 Wire-contract plane

```text
WireType
WireFieldId
PresencePolicy
UnknownPolicy
DuplicatePolicy
OrderingPolicy
Framing
```

### 119.4 Encoding plane

```text
Encoder
Decoder
EncodingProfile
CanonicalEncodingProfile
```

### 119.5 Evolution plane

```text
SchemaDelta
Migration
MigrationLoss
Alias
Deprecation
CompatibilityEnvelope
```

### 119.6 Relation plane

```text
RoundTrip
WireCompatibility
SemanticCompatibility
UnknownPreservation
MigrationRefinement
PathCoherence
```

### 119.7 Realization plane

```text
text
binary tagged
binary positional
canonical CBOR-like
protobuf-like
Avro-like
zero-copy
custom generated
```

### 119.8 Assurance plane

```text
codec proof
schema witness
migration proof
canonicality evidence
bounded corpus evidence
foreign-runtime evidence
```

The planes are connected explicitly but not collapsed.

---

## 120. First implementation pilot

The initial executable RFC 0027 pilot SHOULD remain bounded.

Implement versioned artifacts for:

```text
Schema
SchemaVersion
FieldIdentity
ReservedFieldIdentity
WireContract
EncodingProfile
PresencePolicy
UnknownFieldPolicy
DuplicateFieldPolicy
CanonicalEncodingProfile
SchemaDelta
Migration
CompatibilityClaim
RoundTripClaim
SchemaCounterexample
SerializationDelta
```

The first pilot does not need a production-general schema compiler.

---

## 121. Fixture A — value round-trip

Define a small logical record with exact integer/text fields.

Establish:

```text
Decode(Encode(v)) == v
```

for a bounded corpus and at least one independently checkable structural relation where feasible.

Bounded corpus evidence MUST remain bounded evidence.

---

## 122. Fixture B — decodable but noncanonical bytes

Provide two legal encodings of one semantic value.

Expected:

```text
Decodable(A) = PASS
Decodable(B) = PASS
SemanticEquivalent(A,B) = SUPPORTED
Canonical(A) = PASS
Canonical(B) = FAIL
```

or the corresponding scoped dispositions.

This mechanically proves:

```text
Decodable != Canonical
```

---

## 123. Fixture C — canonical re-encoding

Decode both legal encodings and re-encode under one canonical profile.

Expected:

```text
CanonicalEncode(Decode(A))
    ==
CanonicalEncode(Decode(B))
```

while original raw byte identity remains false.

---

## 124. Fixture D — field rename preserves identity

Schema v1:

```text
field id=3 name=surname
```

Schema v2:

```text
field id=3 name=family_name
```

with an explicit rename/alias relation.

Expected semantic field identity preserved under declared migration.

---

## 125. Fixture E — illegal field-ID reuse

Historical schema:

```text
field 4 = email
```

Later schema retires field 4.

Candidate schema attempts:

```text
field 4 = payment_token
```

Expected rejection through `ReservedFieldIdentity`.

---

## 126. Fixture F — absent versus default

Use a logical field where:

```text
Absent
```

and:

```text
Present(0)
```

are semantically distinct.

A codec/profile that collapses them MUST fail the lossless realization requirement.

---

## 127. Fixture G — unknown-field forwarding

New schema adds field 17.

Old reader:

- cannot interpret field 17;
- preserves its raw wire representation;
- rewrites known fields;
- emits field 17 unchanged when permitted.

Expected:

```text
CanReadNewer = supported
CanPreserveNewer = supported
CanInterpretField17 = unsupported
```

---

## 128. Fixture H — lossy format bridge

Take a message with unknown future field 17.

Bridge:

```text
tagged binary -> text object -> tagged binary
```

where the text bridge drops unknown field metadata.

Expected:

```text
Readable = supported
ForwardRoundTripPreserving = refuted
```

This proves read compatibility does not imply preservation compatibility.

---

## 129. Fixture I — backward-only compatibility

Design a schema change where:

```text
old writer -> new reader
```

is safe using defaults/aliases, while:

```text
new writer -> old reader
```

cannot preserve/understand a mandatory new semantic distinction.

Expected directional compatibility results remain separate.

---

## 130. Fixture J — wire-safe but semantically narrowed

Keep the same wire integer field while changing semantic constraint from:

```text
all i32
```

to:

```text
0..100
```

Expected:

```text
WireCompatible = supported
UniversalSemanticBackwardCompatible = refuted
```

with a counterexample such as `101`.

---

## 131. Fixture K — migration diamond

Construct:

```text
        v2a
       /   \
     v1     v4
       \   /
        v2b
```

Provide two migration paths.

One test case SHOULD make them coherent.

Another SHOULD intentionally introduce a divergence and produce a path-scoped counterexample.

This becomes the first `MigrationPathCoherence` verifier study.

---

## 132. Fixture L — canonical signature scope

Take two byte encodings that decode to one semantic value.

Show:

```text
RawEncodingDigest(A) != RawEncodingDigest(B)
```

but:

```text
CanonicalEncodingDigest(A) == CanonicalEncodingDigest(B)
```

under the same canonical profile.

Do not treat the two digest kinds as interchangeable.

---

## 133. Fixture M — numeric narrowing

Encode an RFC 0021 exact numeric value through a narrower wire representation.

Expected:

- exact values inside range may pass;
- out-of-range values fail or generate explicit approximation obligations;
- serialization does not hide the numeric conversion.

---

## 134. Fixture N — decode budget

A syntactically valid recursive value exceeds configured maximum depth.

Expected:

```text
DECODE_BUDGET_EXCEEDED
```

not:

```text
MALFORMED
```

This links RFC 0022 bounded execution into serialization.

---

## 135. Fixture O — projection decode

Read only selected header fields.

Expected result records:

```text
validated_projection = header
unvalidated_projection = body
```

The system must refuse a whole-message validation claim.

---

## 136. First Forge experiment

The first cross-component Forge experiment SHOULD define:

```text
TelemetryRecord
```

with requirements:

```text
exact numeric preservation
stable field identities
explicit presence
future fields permitted
unknown future fields preserved
canonical representation required for signing
rolling compatibility across at least three schema generations
bounded decoder authority
```

Preferences:

```text
minimize encoded size
minimize encode latency
minimize decode latency
minimize evidence cost
```

Candidate realizations SHOULD include at least:

```text
A. text/object representation
B. deterministic CBOR-like representation
C. tagged protobuf-like binary
D. generated positional/fixed binary
```

The experiment MUST demonstrate at least one candidate that is faster/smaller but rejected for a protected semantic reason.

Example:

```text
A: readable but loses unknown fields through the selected bridge
B: valid + canonical + unknown-preserving
C: valid + compact + unknown-preserving but requires a canonical profile for signing
D: fastest but fails required rolling evolution
```

Forge may optimize only among candidates satisfying the mandatory relations.

---

## 137. Diagnostics

Diagnostics SHOULD be semantic and relation-scoped.

Suggested codes:

```text
E2701 malformed encoding
E2702 encoding valid but noncanonical under required profile
E2703 duplicate field rejected by wire contract
E2704 unknown field rejected by closed schema
E2705 unknown field ignored; required preservation relation cannot be established
E2706 retired field identifier reused with new meaning
E2707 presence collapse loses Absent/Present distinction
E2708 writer schema cannot resolve into reader schema
E2709 wire-compatible change violates semantic compatibility
E2710 migration partial for source value
E2711 migration loses protected observation
E2712 migration paths are incoherent
E2713 canonical profile changed; digest/signature evidence stale
E2714 numeric wire conversion violates RFC 0021 contract
E2715 UTF-8/text decoding invalid under strict profile
E2716 decode budget exceeded; input validity unresolved
E2717 projection decode cannot authorize whole-message validation
E2718 schema reference unresolved or wrong identity
E2719 codec runtime behavior conflicts with declared duplicate/unknown policy
E2720 candidate serialization realization violates protected compatibility relation
```

Diagnostics MUST distinguish malformed input from unsupported policy, bounded exhaustion, semantic incompatibility, and evidence failure.

---

## 138. Security consequences

This RFC intentionally makes several common shortcuts visible or illegal:

- parsing unknown authority-bearing fields and ignoring them cannot silently authorize execution;
- duplicate keys cannot be interpreted differently by different components under one strict contract;
- exact-byte signatures and canonical-semantic signatures cannot be confused;
- invalid UTF-8 cannot be accepted under a strict text profile and then normalized differently elsewhere;
- field-ID reuse cannot reinterpret historical data silently;
- schema hashes cannot masquerade as semantic identity without an explicit relation;
- numeric narrowing cannot hide behind a codec call;
- projection parsing cannot claim whole-message validation;
- an older component may store/forward a future artifact without being authorized to execute it;
- serialization metadata remains subject to RFC 0024 side-channel analysis; and
- a faster codec cannot bypass compatibility or canonicality obligations.

---

## 139. Scope boundaries

### RFC 0008

Owns I/O/effect/capability authority for reading, writing, sending, receiving, allocating, and accessing external codec resources.

### RFC 0009

Owns memory/storage/reference identity and provenance of source/encoded artifacts.

### RFC 0014

Owns module/component/link compatibility. RFC 0027 owns schema/wire compatibility; component compatibility may depend on it.

### RFC 0015

Owns foreign codec/schema/runtime boundaries and unsafe interoperability assumptions.

### RFC 0018

Owns appraisal of serialization, migration, compatibility, canonicalization, and codec evidence and final action-relative authorization.

### RFC 0019

Owns logical value/type semantics and representation-sensitive observations. RFC 0027 projects those values into wire schemas and encodings.

### RFC 0020

Owns the relation machinery used for round trips, encoding equivalence, compatibility, migration refinement, and path coherence.

### RFC 0021

Owns numeric conversion, approximation, overflow, precision, and reproducibility semantics used by numeric serialization.

### RFC 0022

Owns bounded computation/termination semantics used by bounded decoders and encoders.

### RFC 0023

Owns time values, time scales, temporal validity, schema-validity intervals, and time-dependent deprecation/application.

### RFC 0024

Owns confidentiality/integrity/side-channel policy over encoded data, schema metadata, field presence, message size, and migrations.

### RFC 0025

Owns principals, signatures, credentials, attestation, and trust for schemas and signed/canonical data.

### RFC 0026

Owns durable/crash-safe execution of stored schema migrations, mixed-version durable state, and recovery.

### RFC 0028

Will own distributed message semantics, negotiation, delivery, ordering, replication, partitions, and consensus. It should consume RFC 0027 wire/schema relations.

### RFC 0030

Will own deployment/live-upgrade coordination and mixed-version rollout using RFC 0027 compatibility envelopes.

### RFC 0031

Will own build/supply-chain artifact lineage and use RFC 0027 canonical serialization/digest semantics for signed attestations and manifests.

### RFC 0032

Will own quantitative encoded size, bandwidth, parser memory, CPU cost, and resource economics.

### RFC 0033

Will own multi-objective selection among valid codec/migration realizations.

---

## 140. 1.0 research threshold implications

RFC 0027 SHOULD contribute the following minimum requirements to the MNCS 1.0 research threshold:

- a versioned serialization model separating logical values/types, schemas, wire contracts, encoding profiles, canonical profiles, framing, and encoded artifacts;
- a versioned schema-evolution model with stable field/variant identities, reserved historical identities, explicit presence, open/closed structures, aliases/deprecation, schema lineage, and typed migration edges;
- a versioned compatibility relation family distinguishing wire readability, directional reader/writer compatibility, semantic compatibility, unknown-field preservation, rewrite preservation, round-trip preservation, and migration refinement;
- at least one value-round-trip study plus a case where two legal byte encodings decode to the same semantic value while only one satisfies a required canonical profile;
- at least one demonstration that raw byte identity differs while canonical semantic encoding identity agrees under one profile;
- at least one field-identity study where rename is safe while retired field-ID reuse is mechanically rejected;
- at least one presence study proving `Absent != Present(Default)` when the logical observation model requires the distinction;
- at least one unknown-field study where an older reader can preserve a future field without understanding it, plus one bridge that remains readable but fails forward round-trip preservation because the unknown field is lost;
- at least one directional compatibility study where old-writer/new-reader succeeds while the reverse relation fails;
- at least one schema pair that is wire compatible but semantically incompatible, with a concrete counterexample;
- at least one migration-diamond study establishing or refuting path coherence explicitly;
- at least one numeric serialization study carrying RFC 0021 conversion obligations through the wire boundary;
- at least one bounded decode study where budget exhaustion remains distinct from malformed input;
- at least one projection-decode study where partial validation cannot authorize a whole-message validity claim;
- at least one Forge serialization-realization study that rejects a faster/smaller candidate for violating protected canonicality, unknown-preservation, presence, compatibility, numeric, or migration relations; and
- documented schema, codec, canonical-profile, field-identity, presence, unknown-field, migration, numeric, Unicode, framing, resource, and external-runtime assumption limits.

---

## 141. Open research questions

RFC 0027 deliberately leaves several questions open:

- which minimal canonical schema representation belongs in the proof/core versus ordinary model artifacts;
- whether field semantic IDs should be content-derived, generative, hierarchical, or schema-local;
- whether all schema lineages should be DAGs or whether controlled cyclic projection relationships are useful;
- how much migration synthesis can be inferred automatically without fabricating semantic meaning;
- how to represent unknown-field canonicalization when a local reader lacks the newer semantic schema;
- how to combine canonicalization with selective disclosure and cryptographic commitments;
- whether semantic digests should ever hash a logical projection directly rather than a canonical serialization;
- how to verify streaming/zero-copy codecs independently while preserving identical semantic observations;
- how to express extensible recursive schemas without enabling unbounded parser authority;
- how to reconcile schema registries with content-addressed schema identities;
- how to model schema negotiation across mutually distrustful trust domains;
- how to preserve schema evolution under live distributed state migration;
- how much historical tombstone data should be retained indefinitely;
- how to model compatibility when consumers intentionally expose only a projection of a schema;
- how to compare evidence cost against bandwidth/CPU cost during Forge realization search; and
- which serialization subset should first become self-describing in MNCS's own canonical semantic representation.

These questions do not weaken the constitutional distinctions established here.

---

## 142. Summary of constitutional rules

1. **Value is not encoding.**
2. **Type is not schema.**
3. **Schema is not wire format.**
4. **Wire format is not framing.**
5. **In-memory layout is not wire layout.**
6. **Encoding and decoding are not presumed total bijections.**
7. **Value round-trip, byte round-trip, canonical round-trip, and cross-version round-trip are different claims.**
8. **Successful parse is not schema validity.**
9. **Schema validity is not semantic validity.**
10. **Decodable is not canonical.**
11. **Canonicality is profile-relative.**
12. **Canonical byte identity is not semantic identity.**
13. **Digest identity must state what was hashed.**
14. **A schema digest is not automatically semantic schema identity.**
15. **Canonicalization is a transformation, not an identity assertion.**
16. **Field name is not field identity.**
17. **Wire field ID is not necessarily semantic field identity.**
18. **Retired field/variant identities do not silently acquire new meaning.**
19. **Absence is not default value.**
20. **Absence is not null.**
21. **Unknown is not invalid.**
22. **Unknown is not automatically ignorable.**
23. **Ignoring unknown fields is not preserving them.**
24. **Reading future data is not rewriting future data without loss.**
25. **Closed and open schemas are different contracts.**
26. **Wire extensibility is not semantic forward compatibility.**
27. **Backward and forward compatibility are directional.**
28. **Wire compatibility is not semantic compatibility.**
29. **Schema version numbers do not define migration topology.**
30. **Migration is a typed transformation with explicit domain, losses, and evidence.**
31. **Upgrade and downgrade are not automatically inverses.**
32. **Multiple migration paths require coherence evidence where equivalent outcomes are required.**
33. **Deprecation is not historical deletion.**
34. **Default values are explicit reader/migration semantics.**
35. **Map ordering is not semantic unless declared observable.**
36. **Duplicate handling is part of the wire contract.**
37. **Bytes are not text.**
38. **Unicode normalization is not implicit.**
39. **Numeric serialization carries RFC 0021 obligations.**
40. **Compression is not serialization.**
41. **Encryption is not serialization.**
42. **Authentication/signing is not serialization.**
43. **Decode budget exhaustion is not malformed input.**
44. **Projection decode is not whole-message validation.**
45. **Bounded compatibility testing is not universal proof.**
46. **A correct logical migration is not automatically crash-safe when materialized.**
47. **Schema negotiation is not compatibility proof.**
48. **Schema metadata and field presence remain information-flow observations.**
49. **Generated codecs are untrusted until required relations are evidenced.**
50. **A faster or smaller codec cannot weaken protected serialization semantics.**

---

## 143. Closing principle

The RFC can be summarized by this model:

```text
LogicalValue
    != Schema
    != WireContract
    != Encoding
    != Bytes

SchemaVersion
    -> identifies one semantic schema state

Migration
    -> explicitly relates schema states

CompatibilityClaim
    -> states which cross-version operation is safe

CanonicalProfile
    -> selects one deterministic wire representative where required

RFC 0018 appraisal
    -> decides whether current evidence authorizes exact use U
```

MNCS should therefore permit machines to search aggressively across schema projections, encodings, framing strategies, canonicalization rules, parser implementations, and migration paths while keeping the semantic information that must survive the boundary explicit.

The machine-native objective is:

> **Specify the meaning that must survive crossing a representation boundary. Let machines choose and evolve the wire realization only when they can establish exactly which values, distinctions, identities, unknown information, compatibility relations, and cryptographic canonicalization properties survive that crossing.**
