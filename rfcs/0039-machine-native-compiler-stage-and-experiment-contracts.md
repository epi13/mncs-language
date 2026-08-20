# RFC 0039 — Machine-Native Compiler Stage and Experiment Contracts

- **Status:** Proposed
- **Track:** Compiler architecture and Forge integration
- **Depends on:** RFC 0002, RFC 0003, RFC 0005, RFC 0018, RFC 0020, RFC 0031, RFC 0034, RFC 0035, RFC 0036, RFC 0038
- **Primary ownership:** logical compiler stages; stage capability disclosure; compiler experiment observations; Forge consumption boundary

## Abstract

RFC 0038 defines compilation as evidence-bearing refinement. This RFC closes two implementation gaps discovered while building C0–C2:

1. the reference compiler did not expose the complete logical pipeline, so absent frontend stages could be mistaken for fused or implemented stages; and
2. compiler studies did not carry enough language-owned structure for Forge to compare IR and pass behavior without inventing a competing compiler schema.

The reference implementation therefore exposes two machine-readable contracts:

- `mncs:language:compiler-stage-architecture:0.1`; and
- `mncs:language:compilation-study-result:0.1`.

These contracts describe architecture and observations. They do not establish assurance or conformance.

## 1. Constitutional decisions

### 1.1 Logical stages cannot disappear

An implementation may fuse stages for performance, but its architecture record must still name:

```text
source / intent representation
lexical analysis
parsing
concrete syntax tree
abstract syntax tree
semantic graph construction
identity resolution
type and contract analysis
high-level IR
verification passes
lowering / backend generation
executable artifact
```

Each stage declares availability, integration location, input/output contracts, determinism requirements, ownership, decisions, and blockers.

### 1.2 Missing stages remain visible

`planned`, `bootstrap_only`, `partial`, and `experimental` are not synonyms for `available`.

The current semantic JSON decode is a bootstrap transport path. It is not a selected lexer, parser, CST, AST, or surface-language contract. The architecture must say so explicitly.

### 1.3 Artifacts are not assumed to be files

Stage inputs and outputs are contract identities and artifact roles. A filesystem path may locate a payload but does not define semantic identity.

Future source envelopes must admit programs, models, workflows, verification systems, infrastructure definitions, autonomous components, and machine processes without collapsing them into a source-file ontology.

### 1.4 Compiler observations are language-owned

A compilation study result identifies:

- compiler and pipeline;
- compiler and build hosts;
- target contract when present;
- compilation completion state;
- per-stage fingerprints;
- exact pass/edge identities and pass-local status;
- unresolved obligations and assumptions;
- diagnostics and execution references.

Forge consumes this contract. Forge must not publish a second schema that changes the meaning of these fields.

### 1.5 Observation, assurance, and conformance remain separate

The compilation study contract is marked:

```text
observation_only_not_assurance_or_conformance
```

A pass-local `PASS` states the result recorded by that pass edge. It does not imply independent assurance, MNCS conformance, promotion, certification, or governance approval.

Forge comparison may answer:

- which stage fingerprints agree;
- where the earliest observed difference appears;
- which pass statuses changed;
- which obligations remain unresolved.

It may not answer “valid” or “conformant” without separately identified assurance and conformance evaluation.

## 2. Current stage assessment

| Stage | Current state | Integration |
| --- | --- | --- |
| source / intent | bootstrap only | external semantic JSON transport |
| lexical analysis | planned | not integrated |
| parsing | bootstrap only | Serde JSON decode outside compiler driver |
| CST | planned | not integrated |
| AST | planned | not integrated |
| semantic graph | experimental | language library |
| identity resolution | experimental | language library |
| type and contract analysis | partial | compiler driver |
| HIR | experimental | compiler driver |
| verification | partial | compiler driver and language library |
| lowering / backend | partial | target plan only |
| executable artifact | planned | not integrated |

The exact record is emitted by `mncs compiler-architecture`; this table is explanatory, not authoritative.

## 3. Debt-prevention rules

Implementation must not assume:

- one file equals one artifact;
- syntactic identity equals semantic identity;
- JSON object order or source trivia is semantic without an explicit profile;
- successful parsing implies valid semantics;
- semantic graph construction is equivalent to scope/binding resolution;
- validation is a complete type system;
- structural SSA validity proves lowering correctness;
- a target name supplies target facts;
- compilation completion means executable output exists;
- compiler or Forge self-reports are independent evidence.

## 4. Initial implementation

The proposed contract is implemented experimentally by:

- `CompilerArchitectureContract` and `CompilerStageContract` in `mncs-model`;
- `reference_compiler_architecture()` in `mncs-compiler`;
- `mncs compiler-architecture` in `mncs-cli`;
- the strengthened `CompilationStudyResult` contract and pass observations; and
- Forge-side observation projection and comparison in `mncs-forge-mcp`.

Tests require all logical stages to appear exactly once in canonical order, preserve planned gaps, validate content identity, retain unresolved obligations, expose pass executions, localize the earliest observed IR difference, and refuse observation laundering.

## 5. Next implementation sequence

1. Define a representation-neutral source artifact envelope and relationship/provenance fields.
2. Freeze lossless token, source-span, diagnostic, and CST contracts before selecting grammar breadth.
3. Define AST-to-semantic-graph elaboration, scope, binding, and identity-resolution edges under RFC 0035.
4. Integrate semantic graph and identity outputs into compiler evidence rather than invoking them only as library commands.
5. Add a portable backend adapter only after target legality and translation-validation request/result contracts are executable.
6. Let Forge track tournaments over language-owned pass/backend candidates while retaining separate assurance and conformance results.

## 6. Acceptance criteria

This RFC may advance when:

- the architecture record remains deterministic across supported hosts;
- every logical stage has an explicit owner and state;
- the language experiment result round-trips with a valid content identity;
- Forge compares language records without creating a compiler verdict;
- changed HIR or SSA fingerprints localize the first observed divergence;
- pass `UNKNOWN` remains distinct from missing assurance; and
- documentation and tests do not claim lexer, CST, AST, backend, executable, Raspberry Pi, or conformance support that has not been demonstrated.
