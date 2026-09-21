# Language migrations

`language-migrations.json` is the machine-readable migration surface owned by
MNCS Language. Doctor reads it when it is available beside a consumer
repository or through `MNCS_LANGUAGE_ROOT` / `MNCS_LANGUAGE_MIGRATIONS`.

The manifest contains only source transformations whose semantic boundary is
explicit. A chronological implementation suffix is not retained as a
permanent compatibility path: the consumer is rewritten to the canonical
identity and then verified against that implementation. Protocol, serialized,
schema, and external contract identities remain versioned in their owning
artifacts and are not listed here unless their source migration is genuinely
mechanical.
