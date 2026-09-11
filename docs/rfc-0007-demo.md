# RFC 0007 Flagship Demonstration

Executed as `crates/mncs-cli/tests/proof_demo.rs` (5 tests, all passing).
Every arrow below is an assertion, not a diagram of intent.

## The pipeline

```text
examples/source/proof-demo-no-overflow.mncs
    fn id(x: i64) -> (result: i64) { return x; }
    fn add_small() -> (result: i64) { let total: i64 = id(2) + id(3); return total; }
        |
        | elaborate + generate_obligations
        v
O = mncs:0.2:obligation:body:integer-overflow:<digest>
    status UNKNOWN, method symbolic-Checked
    (a closed `2 + 3` would discharge statically through the compiler's
    conservative range analysis, P1-021; the `id` call boundary is an
    inter-procedural opacity the analysis conservatively does not see
    through, so O stays genuinely UNKNOWN while evaluating to 2 + 3)
        |
        | lower_to_ssa  (the SAME identity rides along)
        v
SSA Integer { add, i64 signed, Checked } fed by two Calls to one function
with constant arguments 2 and 3 (traced and pinned by the test),
instruction.obligations = [O]
        |
        | untrusted generation: Plus 2 3 = 5 in the kernel Nat fragment
        v
P = 10 cells: Nat, Zero, Succ×5, Plus(3,4), Eq(Nat, Plus, 5), Refl
    (byte-identical to corpus case plus-two-three-is-five)
        |
        +---> MNCS kernel (all five backends) ......... PASS
        +---> independent reference checker ........... PASS
        v
binding: proof mncs:proof:<sha256>, kernel mncs:proof-kernel:0.1,
         obligation O, no assumptions, exact dependency fingerprints
        |
        +---> kernel_backed_range_result(add/i64/2/3) . PASS (KernelProof)
        +---> attempt_no_overflow_elision_with_proof ... PERMITTED,
               transformation record cites the proof identity
        +---> proof_backed_no_overflow_certificate mints from the binding
               plus exact evaluation of the caller-witnessed operands (the
               constant path withholds: no SSA operand is a constant);
               certificate.with_proof_binding attaches the proof identity
               (proof-bound and unbound certificates have different identities)
```

## Invalidation (all asserted)

- Tampered computation (`Plus(2,4)`): reference checker rejects, no binding.
- Forged identity (mutated sealed bytes): identity check fails, no binding.
- Changed dependency fingerprints: reuse refused, elision withheld.
- Proof for O attached to another obligation: withheld.
- Overflowing operation (`i64::MAX + 1`): the conjunction fails even with a
  well-formed binding — the Nat proof alone never grants a machine range fact.
- Proof-backed minting fails closed: a foreign obligation, a mismatched
  operator, or an overflowing witness each mint no certificate even with a
  valid binding.

## What this does not claim

- The Nat proof is not an i64 proof; the range fact comes from the exact
  constant evaluation, stated in the result limitations.
- The proof-backed certificate does not re-verify callee transparency: the
  caller presents the operand trace (both producers are calls to one
  function with constant arguments 2 and 3) and those identities travel in
  the certificate dependencies, so the claim stays auditable. The minting
  checks everything else mechanically: instruction shape, obligation
  carriage, operator/type match, non-overflowing evaluation, and binding
  reusability.
- Agreement between the checkers is consistency evidence, not correctness.
- File-based proof ingestion into `mncs compile` is follow-up work (gap
  0007-G5); the demo drives the same APIs the compiler calls.
