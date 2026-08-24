# External Language Ecosystem Integration

MNCS should be usable outside the core compiler repository without allowing editors, agents, hosting platforms, or third-party tooling to become alternate semantic authorities.

This document defines the intended boundary between the authoritative MNCS language/compiler, the resident `mncs-language-service`, and external language ecosystems such as editors, GitHub, code hosts, CI systems, and developer tooling.

The goal is interoperability without semantic duplication.

## Architectural principle

`mncs-language` remains the authority for syntax, semantics, identities, validation, obligations, lowering contracts, IR, and backend-facing meaning.

`mncs-language-service` keeps that language state resident and exposes role-appropriate interfaces over the same semantic core.

External integrations should consume one of those interfaces or implement narrowly scoped syntax/indexing adapters when a platform cannot consume the language service directly.

```text
                         clients
        ┌──────────────────┼──────────────────┐
        │                  │                  │
      editors            agents          MNCS systems
        │                  │                  │
       LSP                MCP           MNCS-native API
        └──────────────────┼──────────────────┘
                           │
                 mncs-language-service
                           │
                      mncs-language
                           │
                 authoritative semantics

External platform adapters
────────────────────────────────────────────────────────
TextMate grammar        syntax highlighting only
Tree-sitter grammar     syntax tree / indexing only
GitHub Linguist         language classification/statistics
GitHub code navigation  Tree-sitter tags/FQ-name indexing
GitHub Actions/App      validation/review via MNCS tooling
```

A TextMate grammar or Tree-sitter grammar must never be treated as a replacement for the MNCS parser, compiler, semantic model, or language service.

## Integration surfaces

### LSP

Status: **implemented / exercised in `mncs-language-service`**.

Use LSP for editors and applications that expect conventional language-server capabilities such as:

- diagnostics;
- hover;
- go-to-definition;
- references;
- document/workspace symbols;
- semantic tokens;
- completion;
- folding ranges.

The LSP adapter should remain thin. Language understanding discovered through editor requirements belongs upstream in `mncs-language`, not in an editor-specific implementation.

### MCP

Status: **implemented / exercised in `mncs-language-service`**.

Use MCP for agents and machine-oriented clients that benefit from structured semantic inspection rather than editor-oriented protocol shapes.

Current examples include:

- workspace and document status;
- structured diagnostics;
- identity-at-position;
- semantic subject descriptions;
- definitions and references;
- symbol inventories;
- semantic dependencies;
- obligations;
- bounded context packets;
- isolated candidate analysis and semantic deltas.

MCP is an adapter over the same resident state as LSP. It is not a second semantic implementation.

### MNCS-native ecosystem interface

Status: **planned / deferred in `mncs-language-service` Phase 6**.

A richer protocol-neutral interface is expected for RAVEL, Forge, Controller, Fabric-facing orchestration, Commons/Family Record references, and other MNCS-native systems.

Possible uses include:

- identity-bound task contexts;
- semantic context packets;
- candidate snapshot coordination;
- relation/evidence queries;
- bounded verification requests;
- durable Family Record references;
- structured agent handoff without reconstructing program meaning from prose.

The native interface should emerge from demonstrated ecosystem needs and should not freeze schemas prematurely.

### Client SDKs

Status: **planned**.

As external use grows, provide small stable clients over the language-service protocols rather than requiring every application to implement transport details independently.

Candidate clients:

```text
clients/
  rust/
  typescript/
  python/
```

The SDKs should expose protocol operations, snapshot identities, structured errors, and capability discovery. They should not duplicate compiler semantics.

## GitHub language recognition

GitHub repository language percentages are produced by **GitHub Linguist**, not by LSP or MCP.

Therefore, making `.mncs` files appear as `MNCS` in the repository language bar requires upstream Linguist recognition.

### Linguist prerequisites

As of 2026-08-24, GitHub Linguist's contribution process for a new language requires:

1. an entry in `lib/linguist/languages.yml`;
2. a supported syntax-highlighting grammar;
3. representative real-world samples rather than tutorial/"Hello world" examples;
4. generation of a unique language ID with `script/update-ids`;
5. a pull request using the required template and linking to public GitHub usage evidence.

The `.mncs` extension is the intended primary source extension for MNCS.

A future Linguist entry would conceptually resemble:

```yaml
MNCS:
  type: programming
  color: "#<community-selected-color>"
  extensions:
    - ".mncs"
  tm_scope: source.mncs
  ace_mode: text
  language_id: <generated-by-linguist>
```

Do not reserve or invent the final `language_id`; Linguist generates it.

### Usage threshold

Linguist intentionally rejects very new languages without sufficiently broad public use.

As of 2026-08-24 its published requirement for extensions expected to occur multiple times per repository is:

- at least **2,000 indexed files in the previous year**, excluding forks; and
- a reasonable distribution across unique users/repositories.

Linguist may filter out a primary language owner's repositories when assessing whether adoption is genuinely distributed.

This means MNCS should **not** create artificial files or repositories to satisfy the threshold. Eligibility should arise naturally from real MNCS programs, experiments, applications, libraries, examples, and outside adoption.

These thresholds and procedures are external policy and can change. Re-check the upstream Linguist contribution guide before submitting an integration PR.

Current upstream reference:

- <https://github.com/github-linguist/linguist/blob/main/CONTRIBUTING.md>

## TextMate grammar

Status: **planned; high-value prerequisite**.

GitHub Linguist uses TextMate-compatible grammars for syntax highlighting. A maintained MNCS TextMate grammar therefore provides value before Linguist eligibility because it can also be consumed by editors and extensions that support TextMate scopes.

The grammar should:

- use the canonical scope `source.mncs`;
- follow the current accepted source profiles without pretending the grammar defines semantics;
- highlight declarations, identifiers, contracts, capabilities/effects, types, literals, control forms, comments, and punctuation conservatively;
- include representative fixtures drawn from real MNCS programs;
- carry a license acceptable to Linguist;
- be versioned independently enough that highlighting updates do not require modifying compiler semantics.

The grammar should preferably live in an MNCS-owned repository or clearly owned subtree with tests and release/version metadata.

## Tree-sitter grammar

Status: **planned; strategically important**.

Tree-sitter serves a different purpose from the authoritative MNCS parser. It is useful for incremental syntax trees, structural editor tooling, code-host indexing, and GitHub code navigation.

A future MNCS Tree-sitter implementation should:

- parse the accepted public source syntax, not canonical semantic JSON;
- preserve error recovery suitable for partially edited files;
- remain syntax-only where semantic resolution would require `mncs-language`;
- include a substantial corpus of real MNCS fixtures;
- test ambiguous/error cases against the authoritative parser where meaningful;
- publish and maintain a Rust crate if GitHub code-navigation support is pursued;
- provide standard query files such as highlights and tags where useful.

Tree-sitter must not become a second normative grammar. When behavior differs, `mncs-language` is authoritative and the Tree-sitter grammar should be corrected.

## GitHub code navigation

Status: **blocked on Linguist recognition and a mature Tree-sitter parser**.

GitHub's current public process requires:

1. the language to exist in Linguist;
2. a mature, maintained Tree-sitter parser;
3. the parser to publish a Rust crate on crates.io;
4. Tree-sitter tag queries for supported symbol definitions/references;
5. fully-qualified-name queries where the language needs them;
6. a language-support request to GitHub's code-navigation project.

For MNCS, useful initial symbol classes are likely to include modules, functions, record/type declarations, enum declarations/variants, and fields. The exact set should follow actual language semantics rather than mimicking another language.

Current upstream reference:

- <https://github.com/github/code-navigation>

GitHub retains discretion over whether to enable a language, even when the technical prerequisites exist.

## GitHub Actions and GitHub App integration

Status: **planned; does not require Linguist recognition**.

MNCS can integrate deeply with GitHub long before GitHub natively recognizes the language.

A GitHub Action can run repository-local MNCS tooling on pushes and pull requests to provide, for example:

- parse/elaboration/validation checks;
- obligation summaries;
- fail-closed backend/translation checks when explicitly requested;
- identity-bound semantic-diff reports;
- candidate analysis;
- experiment or evidence validation;
- machine-readable artifacts for later consumption.

A future GitHub App could provide richer PR review using the language service or an isolated service instance. It should report semantic evidence rather than fabricate GitHub-native semantics from textual heuristics.

Ordinary PR checks must remain bounded. Expensive Forge search, Fabric execution, or distributed verification should require explicit workflows or policy gates rather than being triggered on every keystroke or lightweight query.

## Ownership boundaries

### `mncs-language` owns

- normative source syntax and parsing behavior;
- semantic models and identities;
- validation and diagnostics;
- obligations and evidence semantics;
- compiler/lowering/backend contracts;
- semantic diff/invalidation behavior;
- language conformance fixtures.

### `mncs-language-service` owns

- resident workspace/document state;
- snapshots and caches;
- semantic indexes derived from authoritative artifacts;
- LSP/MCP/native protocol adaptation;
- stale-state and candidate-analysis interaction policy.

### External syntax adapters own

TextMate and Tree-sitter implementations may own:

- syntax highlighting patterns;
- incremental concrete syntax trees;
- host-specific symbol/tag queries;
- editor/code-host packaging.

They do **not** own MNCS semantics.

### GitHub integration owns

GitHub-specific adapters may own:

- workflow packaging;
- Checks/PR presentation;
- repository event handling;
- mapping authoritative MNCS results into GitHub annotations.

They should not reimplement validation, name resolution, obligations, or evidence rules.

## Roadmap

### Stage A — establish portable editor/tooling assets

Status: **planned**.

- create and test an MNCS TextMate grammar;
- create a Tree-sitter grammar against the current accepted source profile;
- build differential fixtures against the authoritative parser where appropriate;
- define ownership/versioning for both adapters;
- document how external clients launch or connect to `mncs-language-service`.

These tasks may begin before Linguist eligibility.

### Stage B — broaden application integration

Status: **planned**.

- exercise LSP in multiple editors/applications;
- exercise MCP with multiple agent clients;
- derive the Phase 6 MNCS-native API from RAVEL/Forge/Controller use;
- provide small Rust/TypeScript/Python client packages when interfaces stabilize;
- add bounded CI/GitHub Action integration.

### Stage C — GitHub Linguist submission

Status: **adoption-gated**.

Begin only when genuine public usage meets the then-current Linguist requirements.

- verify `.mncs` usage through GitHub Search;
- verify distribution outside repositories dominated by the language owner;
- ensure the TextMate grammar is stable and acceptably licensed;
- select a language color through an appropriate public/project process;
- prepare representative licensed samples;
- submit the upstream Linguist PR and follow its required checks.

### Stage D — GitHub code navigation

Status: **blocked on Stage C plus Tree-sitter maturity**.

- publish the mature Tree-sitter parser as a Rust crate;
- add tags queries;
- add fully-qualified-name queries if required by MNCS declaration structure;
- benchmark parser resource behavior on real repositories;
- file the GitHub code-navigation language-support request.

### Stage E — richer hosted semantic integration

Status: **future / experimental**.

- use GitHub Actions or an App to expose semantic diagnostics and identity-bound review information;
- evaluate whether Family Record references should be attached to CI artifacts/check runs;
- support explicit bounded verification workflows;
- preserve the distinction between GitHub's syntax/indexing facilities and MNCS semantic authority.

## Success criteria

External integration is successful when:

1. `.mncs` source is pleasant to read and edit across common tools;
2. editors and agents query the same authoritative semantic model rather than divergent reimplementations;
3. external parsers remain clearly subordinate to `mncs-language`;
4. GitHub can eventually classify and highlight MNCS based on real adoption;
5. GitHub code navigation can be added without weakening or duplicating MNCS semantics;
6. CI and hosted tooling preserve identities, evidence scope, and `PASS`/`FAIL`/`UNKNOWN` distinctions;
7. the ecosystem can expand without turning a platform-specific integration into the language definition.

## Non-goals

This track does not:

- make Linguist recognition a language-design milestone;
- treat GitHub support as proof of production readiness;
- manufacture usage merely to meet an external popularity threshold;
- make TextMate or Tree-sitter normative;
- force the MNCS-native ecosystem interface to conform to LSP or GitHub schemas;
- trigger expensive Fabric/Forge work from ordinary editor or repository queries;
- claim code-navigation support before GitHub actually enables it.

External integration should make MNCS easier to use while preserving the project's central rule: one semantic authority, many adapters.