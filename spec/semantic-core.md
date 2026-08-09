# Semantic Core 0.1

## 1. Scope

The semantic core defines the minimum entities required to describe a verification-native component independently of final source syntax.

## 2. Program

A program MUST declare:

- a semantic schema version;
- a non-empty module identity;
- zero or more named assumptions;
- zero or more functions.

Within one program, assumption identities and function names MUST be unique.

## 3. Function

A function contains:

- a unique name within its module;
- typed inputs and outputs;
- contract clauses;
- declared effects;
- declared capabilities;
- references to program assumptions;
- evidence claims;
- a failure mode.

Input names MUST be unique within the input collection. Output names MUST be unique within the output collection.

## 4. Contract property

Every contract clause MUST have a non-empty identity, kind, and expression. Contract identities MUST be unique within the function.

The 0.1 model recognizes:

- `requires` — obligation at entry;
- `ensures` — obligation at successful exit;
- `invariant` — property maintained across a defined region;
- `preserves` — relation whose relevant value or structure is conserved;
- `budget` — resource or complexity constraint.

Expressions are opaque strings in the 0.1 prototype. A later version MUST define expression semantics, typing, scopes, and temporal references.

## 5. Failure mode

The initial failure vocabulary is:

- `isolated` — failure does not escape the component's declared boundary;
- `atomic` — externally visible state is committed completely or not at all;
- `partial` — documented partial state may remain;
- `compensating` — a declared compensation path restores or offsets effects;
- `fatal` — execution cannot safely continue within the current process or system boundary.

Declaring a failure mode is a claim, not proof. Verifiers and lowering rules MUST establish or conservatively preserve its meaning.

## 6. Deterministic diagnostics

Validators SHOULD emit stable diagnostic codes, semantic paths, and explanatory messages. Equivalent semantic input SHOULD produce equivalent validation output independent of source formatting.

## 7. Versioning

A validator MUST reject schema versions it does not support unless an explicit compatibility mode defines safe behavior. Silent reinterpretation is forbidden.

## 8. Canonical semantic representation and identity (0.2 foundation)

The executable model provides a versioned `0.2` canonical JSON representation independent of
human source syntax. Object fields are deterministic; function input and output order is retained;
set-like declaration collections are sorted by semantic key. Structural semantic identities are
namespaced separately from SHA-256 content fingerprints. Changing semantic content changes the
relevant fingerprint while preserving the structural identity when the declared subject remains
the same.
