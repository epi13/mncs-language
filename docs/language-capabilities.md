# MNCS language capability index

`language-capabilities.json` is the compact, revision-addressed language
knowledge projection. The source-profile registry, library modules, compiler
declarations, and tested examples provide its facts. The generator adds only
small curated topic and idiom guidance that points back to those facts.

Agents and tools should consume the index before scanning the repository:

- the capsule gives the current profile and common language shape;
- topic and symbol queries can select relevant modules, declarations, and examples;
- profile deltas are computed from the ordered profile records;
- provenance carries source paths and content identities for freshness checks.

Regenerate with `python scripts/generate_language_capabilities.py` and verify
with `python scripts/test_generate_language_capabilities.py`. The generated
index is content-addressed and contains no timestamps or machine-local paths.
