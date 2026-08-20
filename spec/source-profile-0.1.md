# MNCS Source Profile 0.1

This specification defines the first executable MNCS source grammar. It is an experimental compatibility profile, not the complete language grammar.

## Envelope

`SourceEnvelope 0.1` fields are `schema_version`, `identity`, `language_version`, `artifact_kind`, `logical_name`, `origin`, `relationships`, `provenance`, and `text`. Identity is SHA-256 over every field except `identity`, prefixed by `mncs:source:artifact:`.

`origin.kind` is one of `inline`, `path`, `uri`, or `generated`. A compiler MUST NOT infer that every source artifact is a file.

Both `schema_version` and `language_version` MUST equal `0.1` for this profile.

Each relationship has a kind (`contains`, `depends_on`, `verifies`, `produces`, `executes`, or `refines`) and a non-empty target artifact identity. Provenance may identify a producer and zero or more derivation inputs. These fields refer to artifacts, not filesystem paths, and participate in source-envelope identity.

## Grammar

```ebnf
document        = header, module_decl, function_decl, { function_decl } ;
header          = "mncs", version, ";" ;
module_decl     = "module", qualified_name, ";" ;
qualified_name  = identifier, { ".", identifier } ;
function_decl   = "fn", identifier, parameter_list, "->",
                  parameter_list, block ;
parameter_list  = "(", [ parameter, { ",", parameter } ], ")" ;
parameter       = identifier, ":", identifier ;
block           = "{", return_statement, "}" ;
return_statement = "return", identifier, ";" ;
version         = digit, { digit | "." } ;
```

The header version MUST reproduce the envelope language version. Profile 0.1 requires exactly one output parameter. The returned identifier MUST resolve to an input, and its type MUST equal the output type.

## Lexical contract

- Identifiers begin with `_` or an alphabetic Unicode scalar and continue with `_` or alphanumeric scalars.
- Whitespace, `//` comments, and nested `/* ... */` comments are trivia tokens.
- `->` is one token.
- Unsupported characters produce `MNL002`.
- Unterminated block comments produce `MNL001`.
- Token spans use UTF-8 byte offsets `[start, end)` plus one-based line and Unicode-scalar column.
- Concatenating token text MUST equal the envelope text byte-for-byte.

## CST and AST

The CST stores the complete token stream and hierarchical nodes with half-open token ranges. Its root covers the entire source, including leading and trailing trivia.

The AST contains the language version, module, functions, parameters, types, return references, and their source spans. It excludes trivia. An AST MUST NOT be emitted while any envelope, lexical, or parse error remains.

## Elaboration

For each source function, elaboration constructs:

- semantic inputs and outputs;
- isolated failure mode;
- no contracts, effects, capabilities, assumptions, or evidence;
- one executable body parameter per input;
- one `entry` block;
- one explicit return terminator.

The resulting `Program` is validated before semantic graph construction. Semantic graph, identity map, HIR, and SSA are unavailable after failed elaboration or validation.

## Diagnostics

Every source diagnostic declares a stable code, stage, severity, message, primary span, expected token kinds, and found token kind when available. Parser codes use `MNP`; envelope/elaboration codes use `MNE`.

Recovery is bounded to producing CST and diagnostics. It does not authorize an AST or semantic artifact.

## Compatibility

Canonical semantic JSON remains a supported bootstrap transport. It is not source profile 0.1 and does not pass through lexical, CST, or AST stages.
