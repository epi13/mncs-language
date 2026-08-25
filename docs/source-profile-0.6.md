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
