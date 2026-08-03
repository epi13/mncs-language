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
- invalidation relationships.

## The central shift

A conventional workflow often looks like this:

```text
source code → compilation → large analysis → inferred claims → report
```

The desired workflow is:

```text
explicit claims + implementation → local obligations → micro-verifiers → evidence graph → executable
```

The program is not merely a sequence of operations. It is a connected set of claims whose evidence can be independently inspected and selectively invalidated.

## Success conditions

The project succeeds as research if it demonstrates that:

1. semantic obligations can be expressed without drowning implementation in proof syntax;
2. small verifiers can answer local questions more efficiently than repeated whole-program scans;
3. assumptions and trust boundaries can be reported honestly;
4. optimization backends can consume verified facts conservatively;
5. agents can debug against failed obligations and causal paths rather than enormous undifferentiated analyzer output;
6. human programmers can still read, review, and intentionally modify the program.

## Language character

The eventual syntax should be intentionally unsurprising. The innovation belongs in semantics, evidence, and tooling—not exotic punctuation. Canonical formatting, explicit names, limited implicit conversion, predictable desugaring, and stable parse trees are preferred over character-level terseness.

## Long-term artifact

A successful compilation should produce more than an executable:

```text
program binary
semantic manifest
verification/evidence manifest
assumption and trust report
source-to-semantic map
backend provenance
```

These outputs should remain linked by stable identities and content hashes.
