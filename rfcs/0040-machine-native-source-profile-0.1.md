# RFC 0040: Machine-Native Source Profile 0.1

- **Status:** Proposed
- **Authors:** MNCS Language Project
- **Target:** compiler phase 1/2 vertical slice

## Summary

This RFC defines the first accepted-for-experiment MNCS human-source profile and its deterministic path through lexical tokens, a lossless concrete syntax tree, an abstract syntax tree, semantic elaboration, semantic graph construction, identity resolution, validation, HIR, and SSA.

The profile is intentionally small. It proves the compiler-stage contracts without allowing surface syntax to outrun the semantic model.

## Source envelope

Compiler input is a `SourceEnvelope 0.1`, not necessarily a file. The envelope declares:

- language and envelope schema versions;
- artifact kind;
- logical name;
- origin kind and optional locator;
- typed relationships to other artifact identities;
- producer and derivation provenance identities;
- exact source text;
- reproducible content identity.

Artifact kinds include programs, models, workflows, verification systems, infrastructure definitions, autonomous components, and machine processes. Profile 0.1 elaborates only `program`; the other kinds are retained as explicit, unsupported contracts rather than coerced into files or programs.

Relationships are recursive identity references with `contains`, `depends_on`, `verifies`, `produces`, `executes`, or `refines` meaning. They are not path references and do not imply that the target is source text. The envelope identity binds relationship and provenance fields, so substituting a model, workflow, verifier, generator, or derived artifact invalidates the envelope identity.

## Accepted grammar

Profile 0.1 accepts one module containing one or more pure functions. Each function has explicit typed inputs, exactly one named typed output, and one explicit return of an input value.

```text
mncs 0.1;
module examples.identity;

fn identity(value: i64) -> (result: i64) {
    return value;
}
```

The complete grammar and diagnostic rules are in `spec/source-profile-0.1.md`.

## Representation contracts

Lexical output retains whitespace and nested block or line comments. Byte spans are half-open and include one-based line and Unicode-scalar column projections. Concatenating token text MUST reproduce the original source exactly.

The CST owns the lossless token stream and hierarchical token ranges. The AST retains source spans but not trivia. AST identity is representation identity and MUST NOT be treated as semantic identity.

Elaboration creates an executable semantic body with one entry block and an explicit return terminator. The returned input must resolve and its type must match the declared output type. Invalid source cannot emit a semantic graph, identity map, HIR, or SSA.

## Compiler evidence

The reference pipeline records these additional artifact representations:

1. `source`;
2. `lexical_tokens`;
3. `concrete_syntax_tree`;
4. `abstract_syntax_tree`;
5. `semantic`;
6. `semantic_graph`;
7. `identity_map`;
8. `validation`.

Every successful front-end transformation emits a deterministic pass execution observation. These observations describe what ran; they are not assurance or conformance verdicts.

## Decisions

- Human source and canonical semantic JSON remain separate representations.
- Source provenance may change source/CST/AST identities without changing semantic, HIR, or SSA identity.
- Comments and formatting never carry semantics.
- Parser recovery is diagnostic-only; recovered input with errors emits no AST.
- The profile adds no ambient effects, implicit return, type inference, overloading, generics, ownership, macros, or target behavior.

## Architectural debt avoided

The profile does not collapse source identity into semantic identity, treat paths as universal artifact identities, hide trivia from the CST, attach evidence to source offsets alone, or let successful parsing imply semantic validity.

## Acceptance criteria

- a fixture reconstructs exactly from tokens and CST;
- an envelope binds at least two relationships to non-file artifacts and rejects identity substitution;
- malformed syntax produces structured span diagnostics;
- the fixture elaborates to a valid semantic graph and reproducible identities;
- the same fixture reaches HIR and SSA;
- every executed stage appears in the compilation-study result;
- Forge can persist and compare the language-owned result without issuing a verdict.

## Deferred work

Contracts, effects, capabilities, assumptions, multiple statements, expressions, constants, calls, branching, cross-artifact references, non-program elaboration, formatting, and recovery nodes remain future profiles.
