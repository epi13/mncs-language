# RFC 0004: Recursive Introspection and Refinement

- **Status:** Draft
- **Target:** 0.2–0.5

## Summary

Make recursive debugging and improvement a foundational property of the MNCS language architecture. The language and toolchain should expose their own semantic, evidence, diagnostic, and transformation structures in forms that can be inspected by the same verifier and agent network used for ordinary programs.

Recursion does not mean unrestricted runtime self-modification. It means that the output of one analysis or repair cycle becomes structured input to the next cycle, with bounded authority, explicit objectives, preserved properties, independent verification, and auditable promotion.

## Motivation

The Forge and RAVEL already point toward recursive verification and reasoning. A language designed around MNCS should not merely emit code for external tools to inspect. It should be able to represent:

- why one of its own obligations failed;
- the smallest causal slice that explains the failure;
- a proposed semantic transformation;
- the properties the transformation intends to improve;
- the properties it must preserve;
- the evidence invalidated by the change;
- the new evidence required before promotion.

Without these structures, recursive improvement becomes an opaque agent repeatedly rewriting source and rerunning large analyzers. The language should instead make the refinement loop local, inspectable, and evidence-driven.

## Terminology

### Introspection surface

A stable, queryable representation of the semantic graph, IR, evidence graph, diagnostics, transformation history, and relevant toolchain identities.

### Diagnostic obligation

A first-class record of a failed, missing, weakened, or conflicting property. It identifies the subject, property, evidence state, assumptions, causal slice, and reproduction context.

### Repair proposal

A candidate transformation that has not yet been trusted. It identifies the intended objective, affected semantic identities, preserved properties, expected invalidation set, authority required, and verification plan.

### Refinement cycle

One bounded observe–propose–verify–compare–promote iteration.

### Promotion

The policy-controlled act of accepting a candidate transformation into a trusted baseline.

## Required refinement cycle

A conforming recursive refinement mechanism SHOULD support the following stages:

1. **Observe** — inspect the current semantic and evidence graphs.
2. **Localize** — derive a minimal defensible causal slice for a failed or weak obligation.
3. **Propose** — create one or more candidate transformations in an isolated candidate state.
4. **Predict** — declare intended improvements, preserved properties, required capabilities, and expected evidence invalidation.
5. **Verify** — rerun the smallest sufficient independent verifier set against the candidate.
6. **Compare** — produce a semantic and evidence delta between the trusted baseline and candidate.
7. **Promote or reject** — apply explicit policy; never silently replace the trusted baseline.
8. **Record** — persist the cycle, including rejected candidates, so later recursion does not rediscover the same failure without new evidence.

The resulting diagnostic, proposal, evidence, and delta artifacts MAY be consumed by a later refinement cycle. This is the recursive property.

## Boundedness and termination

Every automated refinement request MUST declare or inherit limits such as:

- maximum recursion depth or iteration count;
- time, memory, verifier-call, and candidate-count budgets;
- the semantic region that may change;
- capabilities available to the repair process;
- properties that must not regress;
- stopping conditions and escalation behavior.

An implementation MUST NOT treat "continue until improved" as a sufficient termination policy.

## Preservation and improvement

A repair proposal MUST distinguish:

- **objective properties** — properties the proposal is intended to improve;
- **protected properties** — properties that must be preserved;
- **permitted regressions** — explicitly accepted tradeoffs;
- **unknown consequences** — unresolved effects that block automatic promotion.

A candidate MUST NOT be promoted solely because the originally failing test now passes. The evidence delta must include regressions, newly introduced assumptions, broadened capabilities, expanded effects, and weakened assurance states.

## Evidence independence

A transformation generator MUST NOT be allowed to certify its own repair as sufficient evidence without an explicitly declared trust policy. Independent verification may mean a different verifier implementation, execution environment, authority boundary, or externally witnessed rerun depending on the assurance profile.

Recursive cycles MUST NOT convert repeated self-assertion into stronger assurance. Evidence status is determined by method and trust boundary, not iteration count.

## Self-description and self-hosting

The toolchain SHOULD progressively expose its own:

- compiler passes;
- lowering transformations;
- verifier contracts;
- diagnostic rules;
- evidence dependencies;
- promotion policies.

A future self-hosted MNCS compiler MAY analyze and refine its own implementation, but bootstrap boundaries must remain explicit. Self-hosting does not prove the compiler correct. Diverse implementations, reproducible builds, signed artifacts, remote witnesses, and independent reruns remain valuable.

## Relationship to Forge and RAVEL

- **MNCS Language** defines the semantic objects and protocols required for introspection, diagnostics, repair proposals, evidence deltas, and bounded promotion.
- **MNCS Forge** executes micro-verifiers, causal localization, candidate checking, and debug queries against those objects.
- **RAVEL** may coordinate recursive, distributed, multi-agent, or multi-verifier refinement across nodes and trust boundaries.

The language must not hard-code one Forge or RAVEL implementation. It should define interoperable semantic contracts they can consume.

## Security consequences

Recursive improvement increases authority and therefore attack surface. The design must account for:

- verifier or test replacement by the repair agent;
- objective manipulation and reward hacking;
- capability expansion disguised as a fix;
- evidence laundering across iterations;
- candidate escape from its isolation boundary;
- semantic drift hidden by source-level similarity;
- unbounded resource consumption;
- rollback suppression or history deletion;
- collusion among generator and verifier components.

Promotion logs and evidence graphs should be append-only or externally witnessed at higher assurance levels.

## Initial implementation direction

The 0.1 Rust model does not yet execute recursive refinement. Initial implementation work should first define stable identities and schemas for:

- diagnostic obligations;
- causal slices;
- repair proposals;
- transformation plans;
- semantic and evidence deltas;
- refinement budgets;
- promotion decisions.

These artifacts should be serializable independently of the eventual surface syntax.

## Alternatives

### Leave recursion entirely to agents

Rejected. Opaque prompting and source rewriting cannot provide stable identities, precise invalidation, or trustworthy evidence deltas.

### Permit direct self-modification

Rejected as the default. Candidate changes should occur in an isolated state and require explicit promotion.

### Require monotonic improvement on one score

Rejected. A single score hides regressions and encourages objective gaming. Improvement must be property-specific and accompanied by protected-property checks.

## Unresolved questions

- canonical representation of causal slices;
- equivalence and subsumption among repair proposals;
- proof of transformation preservation;
- conflict resolution across distributed verifiers;
- promotion-policy language;
- memory and summarization across long refinement histories;
- when rejected proposals may be safely forgotten;
- how recursive depth relates to MNCS complexity measures.
