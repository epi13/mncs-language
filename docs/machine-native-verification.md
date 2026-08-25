# Machine-native verification

This note records a forward-looking design principle for MNCS Language. It is not a commitment to a
visual UI framework, rendering subsystem, or new surface syntax. The immediate goal is to preserve
language and IR properties that make future verification systems more rigorous and less dependent on
reconstructing meaning from human-facing artifacts.

## Principle

> Machines should not have to reconstruct information that another machine already knew.

Conventional verification often consumes the final human representation of a system: logs, textual
diagnostics, screenshots, serialized reports, or other flattened artifacts. That is sometimes
necessary, but it is a poor default when the producing system already possessed richer semantic,
relational, geometric, provenance, or intent information before producing the human-facing form.

MNCS should therefore prefer preserving machine-usable meaning through lowering and execution when
that meaning is relevant to correctness, verification, repair, or evidence.

## Why visual verification exposes the issue clearly

A rendered interface illustrates the information-loss problem well. A screenshot contains pixels,
but the renderer may already know:

- semantic element identity;
- role and intent;
- parent/child and control relationships;
- bounds, transforms, clipping, z-order, and hit regions;
- text and glyph metrics;
- layout constraints and responsive state;
- accessibility semantics;
- the source value or component that produced the element; and
- which state transition caused a visual change.

A multimodal verifier can infer some of this from pixels, but that inference repeats work and loses
certainty. A machine-native verifier should instead consume structured render state when available
and use pixels as one evidence layer rather than the sole source of truth.

Conceptually:

```text
program intent
    ↓
semantic state
    ↓
structural / relational state
    ↓
geometric / render state
    ↓
pixels
```

Verification can operate across all of these layers. Pixel-level or multimodal judgment remains
valuable for genuinely perceptual questions, but it should be an escalation layer rather than the
only representation available.

## Language implications

This design direction favors several properties that are broadly useful beyond graphics.

### Preserve semantic identity

Stable semantic identities should survive long enough to associate runtime or backend artifacts with
the source-level concepts that produced them. Optimized lowering may erase source syntax, but a
verifier should not be forced to rediscover the subject of an obligation from anonymous addresses,
blocks, pixels, or strings when identity can be preserved explicitly.

### Preserve intent separately from realization

The language and semantic graph should keep requirements, facts, preferences, relationships,
capabilities, and realization choices separately addressable. A verifier should be able to ask what
must remain true without confusing that requirement with one current implementation.

### Treat relationships as verification subjects

Many important properties are relational rather than object-local:

```text
A contains B
A controls C
D derives from E
X must remain disjoint from Y
P must remain reachable from Q
```

The semantic graph should continue to make such relationships explicit and stable enough to support
obligations, evidence, invalidation, and repair.

### Make provenance cheap to retain

Where practical, artifacts should remain traceable through a chain such as:

```text
observed output
    → realization object
    → semantic entity
    → source expression
    → input / state transition
```

Not every production artifact must carry the full chain inline. Sidecars, content-addressed evidence,
or scoped instrumentation may carry it instead. The architectural requirement is that provenance is
not discarded merely because the final human representation does not need it.

### Prefer structured observations over textual logs

Runtime and verifier observations should be representable as typed, identity-bound data rather than
only prose. Examples include state transitions, obligation results, occlusion, bounds violations,
capability use, changed regions, or invalidated evidence.

Human-readable diagnostics can be rendered from those observations. Agents and micro-verifiers
should be able to consume the structured form directly.

### Support bounded determinism where verification needs it

Some verification problems become substantially stronger when execution or rendering can be replayed
under a declared deterministic envelope: fixed seeds, event ordering, time sources, scheduling
policy, target profile, resource limits, or viewport/state inputs. MNCS need not make all execution
deterministic, but the language and runtime should avoid designs that make bounded deterministic
replay impossible to request or describe.

### Preserve domain distinctions

Verification benefits when domain-specific values do not collapse prematurely into undifferentiated
numbers or byte sequences. Units, coordinates, dimensions, time, regions, color values, target
profiles, and similar domains should remain expressible in forms that retain enough identity and
meaning for obligations and diagnostics.

## Expectations as a future semantic direction

Programs describe what exists and what happens. Verification also needs a machine-readable form of
what should remain true.

MNCS already models requirements, contracts, obligations, capabilities, and evidence. Future work may
find value in a more general expectation vocabulary built on those existing structures. For example,
a UI library might eventually express requirements conceptually similar to:

```text
require save_action visible
require save_action reachable
require save_action associated_with current_document
require toolbar controls remain_non_overlapping
```

This example does **not** propose grammar or UI-specific language keywords. It illustrates the value
of retaining a verification subject, relationship, and requirement in a form that Forge or another
verifier can inspect directly.

## Forge-style perceptual exposure

A future visual verifier could mirror Forge's micro-verifier model by exposing a rendered system to
bounded perturbations and checking structured consequences:

- viewport and scale changes;
- long or localized text;
- delayed or missing resources;
- unusual list sizes;
- keyboard and focus state;
- RTL layout;
- alternate fonts or glyph availability;
- transient overlays and interaction combinations; and
- target or renderer variation.

Specialized verifiers could inspect layout, typography, accessibility, interaction state, render
differentials, or perceptual features independently. A larger multimodal model could be reserved for
ambiguous perceptual cases rather than acting as the primary verifier for every frame.

This follows the same architectural pattern as Forge: bounded questions, narrow evidence, explicit
subjects, traceable results, and escalation only when lower-cost verification cannot resolve the
question.

## Generalization beyond vision

The principle applies anywhere software first creates a rich machine state and later flattens it into
a representation primarily designed for people. Examples include diagnostics, telemetry, reports,
network traces, configuration summaries, deployment status, and generated documentation.

MNCS should prefer:

```text
semantic state + relationships + provenance + evidence
```

over forcing downstream machines to infer those properties again from:

```text
text + pixels + incidental formatting
```

when the richer state is already available.

## Current design guidance

This note does not add a 0.x milestone or require implementation work. It records constraints that can
inform current language evolution:

- avoid unnecessary early information loss;
- preserve stable semantic identity through important transformations;
- keep requirements, facts, preferences, and realizations distinct;
- retain provenance and invalidation relationships where verification depends on them;
- prefer typed observations and evidence artifacts over log-only interfaces;
- keep deterministic replay expressible as a bounded policy rather than an ambient assumption; and
- design introspection surfaces so machines can inspect existing structured state instead of
  reconstructing it from human-oriented outputs.

These properties strengthen Forge, recursive refinement, debugging, distributed verification,
language services, future UI/render verification, and other machine-native consumers without making
visual verification itself a core language feature.
