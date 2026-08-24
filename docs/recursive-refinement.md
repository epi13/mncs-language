# Recursive Debugging and Refinement

Recursive improvement is a first-class architectural goal of the MNCS Language Project. The language should not merely produce code that external tools scan. It should expose enough of its own semantic and evidence structure that failures can be localized, candidate repairs can be proposed, and each repair can be independently checked before becoming trusted.

## What recursive means here

Recursive does not mean that a running program may silently rewrite itself. It means that the artifacts produced by one diagnostic or refinement pass are represented in a form that the next pass can consume.

```text
program + semantic graph + evidence graph
                    ↓
            diagnostic obligation
                    ↓
              causal localization
                    ↓
               repair proposal
                    ↓
           isolated candidate state
                    ↓
       targeted verification and comparison
                    ↓
       promotion decision + recorded evidence
                    ↺
```

The loop may repeat, but every iteration is bounded and auditable.

## First-class recursive artifacts

### Diagnostic obligation

A diagnostic obligation records more than an error string. It should eventually include:

- the affected semantic subject;
- the failed, missing, weakened, or conflicting property;
- the verifier and evidence involved;
- assumptions consumed;
- a minimal causal slice;
- reproduction inputs and environment where relevant;
- severity and confidence;
- dependencies whose change could resolve or invalidate the diagnosis.

### Repair proposal

A repair proposal is an untrusted candidate transformation. It should include:

- the transformation author or generator;
- the intended objective properties;
- protected properties that must not regress;
- the exact semantic region permitted to change;
- capabilities required to create and test the candidate;
- predicted effect, assumption, and evidence changes;
- a verification plan;
- resource and recursion budgets.

### Semantic and evidence delta

The comparison between baseline and candidate must show more than whether tests passed. It should report:

- changed semantic identities;
- added or removed effects and capabilities;
- new, removed, or weakened assumptions;
- invalidated evidence;
- newly produced evidence;
- regressions and unresolved consequences;
- complexity and resource changes.

### Promotion decision

Promotion is a policy decision, not a side effect of successful generation. A candidate may be automatically promotable only when the relevant assurance profile permits it and all protected obligations remain satisfied.

## Micro-debugging

The recursive model is intended to avoid depending exclusively on enormous Clang, LLVM, Joern, or whole-program reports. The semantic graph should let the Forge ask bounded questions such as:

```text
Why did property transfer.conservation fail?
Which state transition first introduced the violating value?
Which evidence nodes became stale after this edit?
What is the smallest region that can be changed without broadening authority?
Did the candidate repair add an undeclared effect?
Which protected properties regressed?
```

A micro-verifier may answer one of these questions and emit an artifact that becomes input to the next refinement step.

## Guardrails

Recursive improvement must be constrained by design:

- candidate changes occur outside the trusted baseline;
- the generator does not automatically certify its own repair;
- recursion depth, time, memory, candidate count, and verifier calls are bounded;
- capability and effect expansion is visible and blocks silent promotion;
- evidence status cannot increase merely because the same claim was repeated;
- rejected candidates and their reasons are retained long enough to prevent blind repetition;
- promotion history is append-only or externally witnessed for stronger assurance profiles;
- human review remains available and may be mandatory for selected properties.

## Self-hosting

A future MNCS compiler may be written partly or entirely in the MNCS language and may expose its own passes and contracts through the same introspection surface. This would permit the compiler to diagnose and propose improvements to itself.

Self-hosting is not self-proof. The bootstrap compiler, runtime, verifier implementations, operating system, and hardware remain trust boundaries unless addressed independently. Reproducible builds, diverse implementations, signed evidence, and remote witnesses are complementary rather than optional philosophical extras.

## Division of responsibility

### MNCS Language

Defines the semantic structures for diagnostics, causal slices, transformation proposals, budgets, evidence deltas, and promotion decisions.

### MNCS Forge

Runs the micro-verifiers, debug queries, candidate checks, and evidence production against those structures.

### RAVEL

Coordinates recursive or distributed refinement across agents, machines, verifier implementations, and trust boundaries.

This separation keeps recursive improvement native to the language model without coupling the language to one orchestration implementation.

## Roadmap 0.5 bounded cycle

`mncs experiment refine BASELINE CANDIDATE... --budget N` implements the first closed bounded
realization cycle. It accepts only an identity-valid PASS baseline, refuses a zero or exceeded
candidate budget, and records for every candidate:

- the frozen baseline and candidate result identities;
- the single localized dimension (`backend:<baseline>-><candidate>`);
- source, semantic, HIR, SSA, artifact, corpus, and bounded-observation comparison;
- every translation-validator identity consumed;
- unresolved evidence or validator failures;
- an explicit `accepted_as_experimental_realization` or `rejected_and_retained` disposition; and
- human-readable reasons inside the identity-sealed machine artifact.

Promotion requires a valid candidate identity, PASS status, no unresolved required evidence, current
PASS translation validators, identical source/semantic/HIR/SSA identities, and bounded public
behavior agreement. It changes only the experimental realization; it cannot change MNCS semantics.
The CLI integration test uses a research-bytecode baseline, accepts a WASM candidate over five
arithmetic edges, and retains an intentionally incorrect CRE-1 candidate as rejected. Forge/search
may supply candidates but appears in the artifact as explicitly lacking promotion authority.
