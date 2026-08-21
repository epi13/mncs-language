# MNCS Source Profile 0.2

This specification extends Source Profile 0.1 with the smallest executable semantic slice that uses the existing contract, capability, binding, and body models. It is experimental.

## Envelope

`SourceEnvelope` schema_version remains `0.1`. `language_version` MUST equal `0.2`. A compiler version is not a language version; 0.2 source keeps 0.1 meaning for 0.1 envelopes.

## Grammar additions

Profile 0.2 function declarations MAY include:

- `requires` / `ensures` / `assumes` named clauses;
- `capability` declarations;
- `effect KIND authorized_by CAPABILITY`;
- `let NAME : TYPE = EXPR;`;
- `if EXPR { STMTS } else { STMTS }`;
- `fail MODE;`;
- integer literals and `+ - * == != < <= > >=`.

The function still ends with an explicit `return NAME;`. Integer expressions elaborate as checked `i64` operations. Comparison expressions elaborate as boolean conditions.

## Binding

Each `let` and parameter occupies a lexical scope. Duplicate names in the same scope are `MNE110` ambiguity. Resolution is innermost-scope lookup, never declaration-order search, hashmap iteration, or filesystem order.

## Authority

An effect cannot appear without a declared authorizing capability. The elaborator MUST NOT invent authority.

## Compatibility

Profile 0.1 source remains valid. Canonical semantic JSON remains a bootstrap transport and is not Source Profile 0.2.
