# Source Profile 0.6 — payload sums, strict booleans, division/modulo

Status: **experimental**, introduced 2026-08 by the Ox Alpha standard-library
tranche. Profiles 0.1–0.5 are unchanged; every 0.1–0.5 fixture still parses,
elaborates, and executes identically. Profile 0.6 exists so new syntax can be
gated instead of silently reinterpreting older profiles.

## Surface additions

### Payload-bearing finite variants

```text
enum Verdict { Accept { score: i32 }, Block { reason: i32 }, Defer }
```

- Variant payloads use the same field syntax as records. Payload-free
  variants are written exactly as in Profiles 0.3–0.5.
- Construction is qualified and record-style: `Verdict.Accept { score: x }`,
  `Verdict.Defer`. Every declared payload field must be supplied exactly once;
  values are carried in canonical (name-sorted) field order.
- Logical value model: a variant value is its discriminant plus its payload
  values keyed by canonical field names. No physical layout participates in
  meaning (RFC 0019 representation independence).

### Match patterns

```text
match verdict {
    Verdict.Accept { score } => score,
    Block { reason }         => 0 - reason,
    Defer                    => 1,
}
```

- Bare (`Accept`) and qualified (`Verdict.Accept`) patterns are both accepted;
  a qualified pattern must name the subject's own type.
- Payload binders bind each declared field exactly once: `{ score }` (shorthand)
  or `{ score: s }` (renamed). Bindings scope to the arm body.
- The wildcard `{ .. }` carries but does not observe a payload.
- Exhaustiveness is unchanged and remains mandatory over variants (RFC 0042):
  a match that omits a variant is rejected regardless of wildcards inside arms.

### Strict boolean operators

`&&` and `||` operate on two bool operands and produce bool. Precedence:
`||` < `&&` < equality < relational < additive < multiplicative.

Semantics are **strict**: both operand expressions are evaluated. Evaluation is
not short-circuited. Operands are total values of type bool; an effect-bearing
call used as an operand keeps its declared authority rather than gaining
conditional hiding. Short-circuit forms are future work and require explicit
control semantics (RFC 0042 authority closure).

### Division and modulo

`/` and `%` share the multiplicative precedence level and elaborate to checked
div/mod machine intent. Division by zero and `MIN / -1` surface as explicit
runtime failures under checked semantics (RFC 0021); they do not silently wrap.

## Realization envelope (observed 2026-08)

| Stage | Payload sums | Boolean ops | Div/mod |
| --- | --- | --- | --- |
| body reference executor | realizes | realizes | realizes (checked) |
| SSA reference executor | realizes | realizes | realizes (checked) |
| research bytecode | realizes | realizes | refuses explicitly |
| portable WASM MVP | refuses (CGN302) | realizes (i32 bitwise) | refuses (CGN301) |
| C11 / LLVM / Cranelift | refuse (CGx302) | refuse (CGx302) | refuse |

Backend refusal is an intentional, evidence-backed envelope, not semantic
disagreement. No semantic workaround was added for any backend.

## Identity compatibility

`FiniteConstruct.payload_fields`, `ExecutionValue::Finite.payload`,
`AstMatchArm.bindings/ignore_payload`, and `FiniteVariant.payload` serialize as
absent when empty, so canonical identities of all Profile ≤ 0.5 programs are
unchanged. Verified by the full fixture corpus passing without migration.

## Module imports (experimental)

Profile 0.6 includes all Profile 0.5 programs unchanged and adds module import
declarations, an experimental step toward RFC 0014's evidence-bearing binding
model.

## Syntax

```mncs
mncs 0.6;

module app.study;

use lib.evidence;

fn decide(first: Verdict) -> (result: Verdict) {
    return combine(first, first);
}
```

`use <dotted.name>;` declarations appear after the module declaration and
before every other declaration. In profiles below 0.6 the `use` keyword is an
ordinary identifier; a use-shaped line produces diagnostic `MNP136`, not
silence.

## Semantics

- **Elaboration-time linking.** Each imported module is elaborated
  independently against the same resolver; its exported declarations bind into
  the importing module's namespace. There is one semantic program after
  elaboration, so HIR, SSA, obligations, backends, and execution are unchanged
  by linking.
- **Names identify, semantics bind.** The imported name is a discovery route.
  Compatibility is established only by successful elaboration of the resolved
  module (RFC 0014, principle 1).
- **One namespace after binding.** Imported functions, finite types, and
  record types are referenced by their bare names. Collisions fail closed:
  - duplicate `use` of one module: `MNE170`;
  - import cycles, including self-import: `MNE171`;
  - unresolvable or unparsable dependency: `MNE173`, `MNE172`;
  - finite-type name collision: `MNE174`;
  - record-type name collision: `MNE175`;
  - function name collision: `MNE176`.
  A diamond re-export of the *same* identity binds once and is accepted.
- **Identity provenance.** Declarations keep identities anchored to their
  declaring module: functions carry `home_module`, types keep their declaring
  namespace in their identities. A linked declaration therefore has one
  stable identity in every importing program. `Program.dependencies` records
  the direct import closure as module identities, sorted.
- **Authority closure crosses the boundary.** A caller must re-declare every
  capability and matching effect pair of any callee, local or imported;
  undeclared authority across a module boundary fails with `MNE134`. Role
  boundaries survive composition (RFC 0014, principle 2).
- **Acyclicity.** Calls remain acyclic; imported callees are leaves already
  validated inside their own modules.

## Resolution contract

The compiler core consumes imports through the `ModuleResolver` trait; it
never touches the filesystem itself. Hosts choose their own layout rules:

- the research CLI resolves names relative to the importing file's directory,
  then its parent, trying `<full.dotted.path>.mncs`, a version-tail-stripped
  path, an `mncs.`-prefix-stripped path, then `<tail>.mncs`;
- after those source-local roots, the research CLI searches each directory
  listed in the `MNCS_LIBRARY_PATH` environment variable (`:`-separated, in
  order). This is how external consumers bind to `mncs.core.*` without
  vendoring the standard-library tree: point `MNCS_LIBRARY_PATH` at this
  repository's `library/` directory;
- language-service hosts may resolve against resident workspace documents.

A resolution miss is always a diagnostic (`MNE173`) in the importing module.
Library roots are a discovery convenience only: compatibility still comes from
elaborating the resolved module against its declared identity, so a stale
`MNCS_LIBRARY_PATH` entry degrades into an honest miss rather than silent
substitution.

## Non-goals for this profile

No qualified call syntax, no selective/re-named imports, no interface
signatures or requirement/provider binding (RFC 0014 components), no version
selection. These remain future work; nothing here forecloses them.
