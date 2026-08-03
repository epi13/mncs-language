# Vision

The MNCS Language Project explores a general-purpose programming language in which verification relationships are part of program meaning rather than metadata reconstructed after implementation.

## Machine-native, not machine-exclusive

The language is not intended to be a private notation for language models. It should be usable and inspectable by humans while exposing a stable semantic structure to agents, compilers, static analyzers, auditors, and independent verifiers.

Machine-native means that important relationships are directly addressable:

- behavioral obligations;
- state transitions;
- authority and capability flow;
- effects and trust boundaries;
- assumptions;
- verifier claims and evidence artifacts;
- resource and complexity bounds;
- invalidation relationships;
- diagnostic obligations, repair proposals, and refinement history.

## The central shift

A conventional workflow often looks like this:

```text
source code → compilation → large analysis → inferred claims → report
```

The desired workflow is:

```text
explicit claims + implementation → local obligations → micro-verifiers → evidence graph → executable
```

When a property fails or evidence weakens, the workflow continues:

```text
failed obligation
      ↓
minimal causal slice
      ↓
isolated repair proposal
      ↓
targeted independent verification
      ↓
semantic and evidence comparison
      ↓
explicit promotion or rejection
      ↺
```

The program is not merely a sequence of operations. It is a connected set of claims whose evidence can be independently inspected, selectively invalidated, and recursively refined through bounded, reviewable cycles.

## Recursive self-improvement

The language should be capable of describing enough of its own compiler, verifier, diagnostic, and transformation behavior that those systems can eventually analyze and improve themselves through the same semantic model used for ordinary programs.

This does not grant unrestricted self-modification. Candidate changes remain isolated until promotion. The generator does not automatically validate its own work. Every cycle exposes its objectives, protected properties, required capabilities, assumptions, invalidated evidence, resource limits, and promotion authority.

Recursive improvement is therefore an evidence loop rather than a source-rewriting loop.

## Success conditions

The project succeeds as research if it demonstrates that:

1. semantic obligations can be expressed without drowning implementation in proof syntax;
2. small verifiers can answer local questions more efficiently than repeated whole-program scans;
3. assumptions and trust boundaries can be reported honestly;
4. optimization backends can consume verified facts conservatively;
5. agents can debug against failed obligations and causal paths rather than enormous undifferentiated analyzer output;
6. diagnostic and repair artifacts can feed later bounded refinement cycles without laundering evidence;
7. candidate repairs can be compared across semantics, authority, complexity, assumptions, and evidence before promotion;
8. a future self-hosted toolchain can expose its bootstrap trust rather than hiding it;
9. human programmers can still read, review, and intentionally modify the program.

## Language character

The eventual syntax should be intentionally unsurprising. The innovation belongs in semantics, evidence, recursive debugging, and tooling—not exotic punctuation. Canonical formatting, explicit names, limited implicit conversion, predictable desugaring, and stable parse trees are preferred over character-level terseness.

## Long-term artifact

A successful compilation should produce more than an executable:

```text
program binary
semantic manifest
verification/evidence manifest
assumption and trust report
source-to-semantic map
backend provenance
diagnostic obligations and causal slices
candidate transformation and comparison records
promotion and rollback history
```

These outputs should remain linked by stable identities and content hashes.
