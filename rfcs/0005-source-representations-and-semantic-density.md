# RFC 0005: Source Representations and Semantic Density

- **Status:** Proposed
- **Authors:** MNCS Language Project
- **Target:** syntax research track

## Summary

MNCS Language should use three related representations rather than forcing human source, agent communication, and backend lowering into one notation:

1. a readable, Zig-influenced human source form;
2. a compact canonical semantic form for agents, Forge, RAVEL, and micro-verifiers;
3. verified intermediate representations for control flow, state, memory, and backend lowering.

The project should optimize for **verified semantic density**: the amount of explicit, unambiguous, independently checkable meaning communicated per representation unit. It should not optimize only for bytes, characters, or one vendor's tokenizer.

This RFC establishes an experimental syntax laboratory. It does not select a final grammar.

## Motivation

A language designed only for minimum character count can hide authority, failure behavior, assumptions, or verification obligations. A language designed only for human familiarity can force agents to repeatedly ingest large source files and analyzer reports.

MNCS needs both:

- human-readable code suitable for review and intentional modification;
- compact semantic slices suitable for recursive debugging and targeted repair.

The largest token reduction should come from retrieving only relevant semantic nodes, not from making ordinary source cryptic.

## Representation model

```text
human source
    ↕ deterministic semantic round trip
canonical semantic graph/form
    ↓ verified lowering
high MNCS IR
    ↓
verified SSA
    ↓
backend IR and machine code
```

A semantic patch targets stable semantic identities and relationships. It does not perform unrestricted textual search and replacement.

## Human source direction

The leading candidate is a restrained, brace-based systems syntax influenced by Zig:

- explicit declarations;
- braces and semicolons;
- deterministic evaluation order;
- explicit mutation, allocation, effects, errors, and capabilities;
- compact error-union notation where it remains unambiguous;
- short semantic keywords such as `req`, `post`, `keep`, `read`, `write`, `emit`, `fail`, and `cost`;
- a structured `spec` region adjacent to a function;
- no unrestricted textual macros;
- canonical formatting.

Zig is an influence, not a compatibility target. MNCS semantics determine the language.

## Canonical semantic form

The canonical semantic form is intended for bounded machine exchange. It may use shorter, stable operators because it is generated and parsed mechanically rather than authored as ordinary application source.

Example:

```text
f transfer
in [src:mut Account,dst:mut Account,amount:Money]
cap [src_write:AccountWrite(src.id),dst_write:AccountWrite(dst.id),audit:AuditAppend]
out TransferError!Receipt
req [amount>0,src.status=active,dst.status=active,src.balance>=amount]
post [src.balance=old(src.balance)-amount,dst.balance=old(dst.balance)+amount]
keep [src.balance+dst.balance]
fx [r(src.status,src.balance,dst.status,dst.balance),w(src.balance|src_write),w(dst.balance|dst_write),emit(Audit.transfer|audit)]
fail atomic
cost [time=O(1),memory<=512B]
```

The canonical form MUST:

- preserve stable semantic identities;
- have one canonical serialization;
- expose all declared effects, capabilities, assumptions, and evidence relationships;
- support bounded subgraph projection;
- round-trip without hidden semantic loss;
- be versioned independently from surface syntax.

It is not the default human programming language.

## Semantic patch form

Recursive tooling should exchange semantic transformations rather than whole-file rewrites when possible.

A patch MUST identify:

- the subject identity;
- the diagnostic or causal basis;
- requested graph changes;
- protected properties;
- forbidden authority or scope expansion;
- expected invalidation;
- refinement budgets.

Patch application creates an isolated candidate. It does not directly alter the trusted baseline or promote its own evidence.

## Semantic density

Semantic density is not a single number. The initial laboratory records:

- bytes;
- characters;
- non-whitespace characters;
- deterministic lexical units;
- lines;
- identifier count and repetition;
- nesting depth;
- declared semantic-claim count;
- lexical units per claim;
- non-whitespace characters per claim.

Future experiments should add:

- model-tokenizer counts across multiple tokenizer families;
- parse success and recovery;
- source-to-graph round-trip accuracy;
- agent generation success;
- localized repair success;
- human comprehension;
- omission rates for effects and capabilities;
- semantic diff size;
- verifier workload.

No syntax wins solely because it has the fewest lexical units. A candidate that omits required meaning is not equivalent and must not be compared as though it were.

## Tokenizer neutrality

The language MUST NOT depend on one model tokenizer. Token vocabularies change by model and provider. Early metrics therefore use a deterministic lexical proxy and explicit semantic claims.

Model-specific token counts MAY be collected as additional evidence, but they must identify the tokenizer and version.

## Syntax tournament

Equivalent candidate programs should be stored with a tournament manifest declaring their shared semantic claims. The initial account-transfer corpus compares:

- Zig-influenced human source;
- fully structured human source;
- compact minimal human source;
- canonical agent/tooling form.

Additional corpora should cover:

- arithmetic and pure functions;
- bounded buffers;
- filesystem effects;
- network capabilities;
- concurrency;
- unsafe hardware access;
- foreign-function boundaries;
- recursive repair proposals;
- generics and collections.

## Canonical formatting

Every accepted surface candidate MUST define one canonical formatter. Formatting must not alter semantics. Source order should affect meaning only where order is explicitly semantic.

Equivalent sugar must lower deterministically to the same semantic graph or be rejected.

## Reflection and self-repair

Source reflection should expose semantic graph views rather than unrestricted source strings. Repair systems should request bounded slices such as:

```text
subject
failed obligation
minimal causal slice
affected evidence
protected properties
available authority
```

The response should normally be a semantic patch, not a complete regenerated file.

## Macro and compile-time generation constraints

Generated code is permitted only when:

- expansion is deterministic;
- expanded semantics are inspectable;
- generated effects and capabilities are visible;
- expansion contributes to semantic identity;
- diagnostics map to generator and generated nodes;
- no macro bypasses verification or capability closure.

Unrestricted textual macros are outside the proposed language model.

## Security consequences

Compactness must never erase:

- authority boundaries;
- effects;
- assumptions;
- failure behavior;
- unsafe regions;
- evidence status;
- candidate-versus-baseline identity.

The canonical form is security-sensitive. Ambiguous parsing, non-canonical serialization, or lossy source projection could attach evidence to the wrong subject or permit semantic substitution.

## Compatibility

This RFC adds an experimental track and tooling. It does not make the example syntax valid production source, select a file extension, or promise compatibility with later grammars.

## Unresolved questions

- ownership and borrowing notation;
- type and generic syntax;
- exact error-union representation;
- location and shape of `spec` blocks;
- whether compact semantic keywords are reserved globally;
- canonical semantic binary encoding;
- stable identity syntax;
- parser and formatter implementation strategy;
- acceptable compile-time generation model;
- how semantic density should weight readability, omission risk, and verifier cost.

## Acceptance criteria

This RFC is sufficiently implemented for the current phase when:

- at least three human source candidates express the same declared claims;
- a canonical semantic candidate expresses those claims;
- a semantic patch example exists;
- deterministic metrics are executable from the CLI;
- CI runs the tournament;
- project documentation clearly states that no final grammar has been selected.
