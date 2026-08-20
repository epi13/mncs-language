# Experimental Fabric validation — 2026-08-20

This record validates the MNCS experiment infrastructure against the narrow
Source Profile 0.1 research scope. It is not compiler assurance, source-profile
promotion, independent evaluation, or evidence that model output is correct.

## Frozen inputs

- `mncs-language`: `fda8e3e`
- Harness: `0.6.9`, local fallback disabled
- Fabric controller: `0.2.0a31`, experiment-certified exact runtime identity
- Windows actor: `collamore02-windows`, `qwen3:8b`
- Fedora actor: `fabric-worker-01`, `granite3.3:2b`
- Control experiment: `exp-25e3bc2cd06b4a0fb3c434babc88a388`
- Turns: four, alternating protocol designer and adversarial reviewer
- Turn wait bound: 240 seconds; experiment bound: 900 seconds

Before starting, `elh experiment-readiness --profile multi-agent --json`
reported every required layer `READY`, both capability inventories were
`CURRENT`, and Fabric classified the controller as
`EXPERIMENT_CERTIFIED_EXACT`. `cargo test --workspace --all-targets` passed 96
tests in the language checkout.

## Preregistered question

Can a two-worker protocol be specified that tests only whether the same
accepted Source Profile 0.1 envelope produces matching semantic, HIR, and SSA
identities on compatible hosts, while keeping host derivation observations
separate?

The experiment explicitly prohibited claims of compiler correctness, universal
equivalence, backend validation, model independence, or Commons acceptance.

## Results

The infrastructure result was **PASS**:

- all four detached Fabric turns completed;
- each turn retained a distinct work, request, job, record, receipt, bundle,
  consumer-context, worker, and output identity;
- durable handoffs crossed Windows to Fedora and back without local fallback;
- no retry, timeout, package mutation, source edit, or Commons publication
  occurred.

The protocol-content result was **FAIL/UNKNOWN** and must not be promoted:

- the Fedora critique invented a second language commit, `fdaf61c`, despite the
  frozen shared input being `fda8e3e`;
- it replaced the actual pinned model identities with `unknown_worker_a` and
  `unknown_worker_b`;
- the next designer propagated those false pins;
- the final reviewer returned `PASS` and claimed immutable exact pins, failing
  to detect the contradiction.

This is useful negative evidence: successful heterogeneous execution and
agreement between agents did not establish protocol correctness. A future run
must provide a machine-generated immutable input manifest to every turn and use
a deterministic validator that rejects changed commit, worker, model, command,
or source-envelope identities before a model verdict can be considered.

## Retained evidence

On the validating controller, Control retains the experiment beneath
`~/.local/state/mncs-control-mcp/experiments/exp-25e3bc2cd06b4a0fb3c434babc88a388/`.
Use the MCP `experiment_status` and `experiment_result` tools to inspect the
bounded state and outputs. Fabric receipts prove execution provenance only; the
model text remains untrusted experimental material.

## Disposition

No language artifact, compiler candidate, source-profile change, or Commons
record is promoted from this run. The strongest supported claim is that the
restored Control → Harness → Fabric path can execute and retain a bounded,
cross-worker, multi-model handoff experiment while preserving enough negative
evidence to reject an unsound semantic conclusion.
