# Source Representations

This document defines early requirements for source, canonical semantic, and semantic patch representations. It does not define a complete grammar.

## Representation classes

An MNCS implementation SHOULD distinguish:

1. **human source**, intended for normal authoring and review;
2. **canonical semantic form**, intended for deterministic machine exchange and bounded graph projection;
3. **semantic patch form**, intended to propose isolated graph transformations;
4. **verified IR**, intended for explicit control flow, state, and lowering.

A tool MUST identify which representation class it consumes or emits.

## Human source requirements

Human source MUST:

- parse deterministically;
- lower to an inspectable semantic graph;
- expose declared effects and authorizing capabilities;
- preserve contract, assumption, failure, and evidence relationships;
- support canonical formatting;
- map diagnostics to stable semantic subjects and source spans;
- avoid semantics that exist only in comments or formatting;
- avoid unrestricted textual macro expansion.

Human source SHOULD:

- use common ASCII delimiters and short recognizable keywords;
- make simple functions visually simple;
- allow specification clauses to be grouped without repeating their subject;
- use deterministic evaluation order;
- require explicit returns, mutation, error propagation, and ambient-resource access.

## Canonical semantic form requirements

Canonical semantic form MUST:

- be versioned;
- serialize one semantic graph in one canonical way;
- retain stable subject, property, capability, assumption, evidence, and transformation identities;
- expose every declared effect and its authority edge;
- distinguish baseline, candidate, and promoted identities;
- support bounded projection by semantic identity;
- be lossless with respect to defined language semantics;
- reject ambiguous or duplicate encodings.

Canonical semantic form MAY be less readable and more compact than human source. It MUST remain inspectable with standard tooling.

## Semantic patch requirements

A semantic patch MUST identify:

- target semantic identity;
- diagnostic, obligation, or explicit user intent motivating the patch;
- requested node and edge additions, removals, or replacements;
- protected properties;
- forbidden changes;
- required authority;
- expected evidence invalidation;
- refinement and resource budgets.

Applying a patch MUST create a candidate identity before verification. Patch application MUST NOT automatically promote evidence or modify the trusted baseline.

## Round-trip requirements

Once a grammar is selected, the following should hold for supported source:

```text
parse(format(parse(source))) == parse(source)
```

and:

```text
semantic(parse(render(semantic_graph))) == semantic_graph
```

where equality is defined over canonical semantic identity rather than incidental source trivia.

## Semantic density measurement

A syntax comparison MUST declare the semantic claims shared by all candidates. A candidate missing a claim is not equivalent.

The initial deterministic metrics are:

- lexical units after comments and whitespace are removed;
- non-whitespace characters;
- lines;
- identifiers and unique identifiers;
- literal counts;
- maximum delimiter nesting;
- lexical units per semantic claim;
- non-whitespace characters per semantic claim.

A report MUST state that these metrics are tokenizer-neutral proxies. Model-specific token counts MUST name the tokenizer and version.

## Keyword direction

The experimental compact vocabulary includes:

- `req` — caller obligation or precondition;
- `post` — postcondition;
- `keep` — preserved property;
- `read` — read effect;
- `write` — write effect and authority binding;
- `emit` — event or append effect and authority binding;
- `call` — component or external call effect;
- `assume` — named assumption;
- `prove` — evidence requirement;
- `fail` — failure semantics;
- `cost` — resource or complexity bound;
- `guard` — runtime-checked condition.

These names are experimental. Single-character semantic keywords are discouraged because they reduce inspectability and may not improve tokenization consistently.

## Zig influence

The leading surface candidate may adopt Zig-like characteristics such as explicit declarations, compact error unions, braces, semicolons, and visible low-level operations. Compatibility with Zig syntax or semantics is not required.

## Token efficiency constraints

Token efficiency MUST NOT justify hiding or dropping:

- authority;
- effects;
- assumptions;
- failure behavior;
- unsafe boundaries;
- evidence status;
- semantic identity;
- candidate state.

The preferred optimization order is:

1. retrieve a smaller semantic slice;
2. remove representational repetition through canonical grouping;
3. use short stable keywords;
4. reduce punctuation only when ambiguity does not increase;
5. consider model-specific token behavior only as additional evidence.

## Open requirements

The following remain unresolved:

- ownership and reference notation;
- generic syntax;
- compile-time execution;
- pattern matching;
- concurrency syntax;
- exact `spec` placement;
- error propagation syntax;
- stable identity notation;
- canonical binary representation.
