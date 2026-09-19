# MNCS language capability index

`language-capabilities.json` is the compact, revision-addressed language
knowledge projection. The source-profile registry, library modules, compiler
declarations, and tested examples provide its facts. The generator adds only
small curated topic and idiom guidance that points back to those facts.

Agents and tools should consume the index before scanning the repository:

- the capsule gives the current profile and common language shape;
- topic and symbol queries can select relevant modules, declarations, and examples;
- module facts include imports, exported declaration names, declared capabilities,
  and effect/authorization requirements;
- profile and identity deltas are served from the compact content-addressed
  `docs/language-capability-deltas.json` chain;
- provenance carries source paths and content identities for freshness checks.

The compiler exposes `mncs language-inventory` for language facts and
`mncs declaration-inventory <module> [--syntax-only]` for generic declaration
and callable facts. The generator is therefore a thin projection over those
compiler-owned records. The syntax-only mode is used for large standard
library modules when imported implementation closure is not needed; it still
comes from the MNCS parser and identity constructors, never host regexes.

The inventory deliberately publishes only compiler-registered public
intrinsics. Private implementation helper names such as `elaborate_program`
cannot appear in the index unless a compiler table deliberately exports them.

Regenerate with `python scripts/generate_language_capabilities.py` and verify
with `python scripts/test_generate_language_capabilities.py`. The generated
index is content-addressed and contains no timestamps or machine-local paths.
