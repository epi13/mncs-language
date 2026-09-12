//! RFC 0007 flagship demonstration, executed as a test:
//!
//! ```text
//! MNCS source program
//!     -> compiler generates real obligation O (integer-overflow, UNKNOWN)
//!     -> untrusted generation builds proof artifact P (Plus 2 3 = 5 by Refl)
//!     -> MNCS kernel and reference checker both return PASS on P
//!     -> PASS evidence bound to O, P, kernel version, assumptions, deps
//!     -> SSA elision consumes the binding (proof-gated transformation)
//!     -> lowering certificate carries the proof identity (nsw/nuw path)
//!     -> mutating any input invalidates reuse
//! ```
//!
//! Every arrow is asserted. The differential suite proves the MNCS kernel
//! agrees with the reference checker on these exact cells across backends.

use std::collections::BTreeMap;

use mncs_codegen::{integer_no_overflow_promise, proof_backed_no_overflow_certificate};
use mncs_compiler::elaborate_program;
use mncs_model::{
    kernel_backed_range_result, parse_proof_corpus, reference_check, ArithmeticIntent,
    EvidenceAuthorityClass, EvidenceFreshness, IntegerOperation, IntegerType, MicroVerifier as _,
    ObligationStatus, ProofArtifact, ProofBinding, ProofCell, ProofTag, ProofVerdict, SemanticId,
    SsaInstructionKind, VerifierMethod, VerifierRequest, PROOF_KERNEL_ID,
};
use mncs_syntax::{parse, SourceArtifactKind, SourceEnvelope};

fn demo_program() -> mncs_model::Program {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/source/proof-demo-no-overflow.mncs"
    ))
    .expect("demo source");
    let envelope = SourceEnvelope::inline(SourceArtifactKind::Program, "demo".to_owned(), text);
    let parsed = parse(&envelope);
    assert!(parsed.is_valid(), "demo parses: {:?}", parsed.diagnostics);
    let program = elaborate_program(&parsed.ast.expect("demo AST")).expect("demo elaborates");
    assert!(program.validate().valid, "demo validates");
    program
}

/// The kernel proof for `2 + 3 = 5`: one shared Succ chain, a closed Plus
/// computation, and a Refl witness. These are the exact cells executed by
/// the differential suite as corpus case `plus-two-three-is-five`.
fn demo_cells() -> Vec<ProofCell> {
    use ProofTag::{Eq, Nat, Plus, Refl, Succ, Zero};
    let mut cells = vec![
        ProofCell::new(Nat, [0, 0, 0, 0]),  // 0
        ProofCell::new(Zero, [0, 0, 0, 0]), // 1
        ProofCell::new(Succ, [1, 0, 0, 0]), // 2 = 1
        ProofCell::new(Succ, [2, 0, 0, 0]), // 3 = 2
        ProofCell::new(Succ, [3, 0, 0, 0]), // 4 = 3
        ProofCell::new(Succ, [4, 0, 0, 0]), // 5 = 4
        ProofCell::new(Succ, [5, 0, 0, 0]), // 6 = 5
        ProofCell::new(Plus, [3, 4, 0, 0]), // 7 = 2 + 3
        ProofCell::new(Eq, [0, 7, 6, 0]),   // 8 Eq Nat (2+3) 5
        ProofCell::new(Refl, [7, 8, 0, 0]), // 9 Refl
    ];
    while cells.len() < mncs_model::PROOF_BUFFER_CAPACITY {
        cells.push(ProofCell::new(Nat, [0, 0, 0, 0]));
    }
    cells
}

struct Demo {
    obligation: SemanticId,
    operation: SemanticId,
    fingerprints: BTreeMap<SemanticId, String>,
    request: VerifierRequest,
    ssa: mncs_model::SsaModule,
    instruction_index: (usize, usize, usize),
    /// Evidence identities for the operand trace: the two Call producers
    /// and the two Constant argument producers behind `id(2) + id(3)`.
    /// They travel in proof-backed certificate dependencies so the
    /// caller-witnessed operand claim stays auditable.
    trace: Vec<SemanticId>,
}

fn setup_demo() -> Demo {
    let program = demo_program();
    let generation = program.generate_obligations();
    let obligation = generation
        .obligations
        .iter()
        .find(|record| {
            record.requirement.0.contains("integer-overflow")
                && record.status == ObligationStatus::Unknown
        })
        .expect("compiler generates an UNKNOWN integer-overflow obligation");
    assert_eq!(obligation.method, "symbolic-Checked");
    let ssa = program.lower_to_ssa().expect("SSA");
    let mut found = None;
    for (function_index, function) in ssa.functions.iter().enumerate() {
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                if let SsaInstructionKind::Integer {
                    operator,
                    operand_type,
                    intent: ArithmeticIntent::Checked,
                } = &instruction.kind
                {
                    if operator == "add"
                        && *operand_type
                            == (IntegerType {
                                bits: 64,
                                signed: true,
                            })
                        && instruction.obligations.contains(&obligation.identity)
                    {
                        found = Some((function_index, block_index, instruction_index));
                    }
                }
            }
        }
    }
    let instruction_index = found.expect("SSA keeps the checked add with its obligation");
    let instruction = &ssa.functions[instruction_index.0].blocks[instruction_index.1].instructions
        [instruction_index.2];
    let operation = instruction
        .semantic_identity
        .clone()
        .expect("semantic operation");
    // Both operands must be exactly `id(2)` and `id(3)` through the same
    // transparent call boundary: the proof is about this exact computation
    // (which evaluates to 2 + 3), not about addition in general. The call
    // boundary is what keeps the obligation UNKNOWN — the compiler's range
    // analysis discharges closed `2 + 3` statically (P1-021) but
    // conservatively does not see through calls — so the pin traces each
    // add input through its Call producer to an exact constant argument.
    let mut constants = Vec::new();
    let mut callees = Vec::new();
    for input in &instruction.inputs {
        let producer = ssa
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find(|candidate| {
                candidate
                    .outputs
                    .iter()
                    .any(|output| &output.identity == input)
            })
            .expect("call producer");
        let SsaInstructionKind::Call { function, .. } = &producer.kind else {
            panic!(
                "demo add operands flow through calls, found {:?}",
                producer.kind
            );
        };
        callees.push(function.clone());
        assert_eq!(
            producer.inputs.len(),
            1,
            "transparent single-argument call: {producer:?}"
        );
        let argument = ssa
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find(|candidate| {
                candidate
                    .outputs
                    .iter()
                    .any(|output| output.identity == producer.inputs[0])
            })
            .expect("constant argument");
        if let SsaInstructionKind::Constant { value, .. } = &argument.kind {
            constants.push(*value);
        }
    }
    constants.sort();
    assert_eq!(constants, vec![2, 3], "demo adds exactly id(2) and id(3)");
    assert_eq!(
        callees.len(),
        2,
        "both operands flow through the call boundary"
    );
    assert_eq!(
        callees[0], callees[1],
        "both operands flow through the same transparent function"
    );
    // Trace identities for proof-backed certificate dependencies: each
    // Call producer plus the Constant producer behind its argument.
    let mut trace = Vec::new();
    for input in &instruction.inputs {
        let producer = ssa
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find(|candidate| {
                candidate
                    .outputs
                    .iter()
                    .any(|output| &output.identity == input)
            })
            .expect("call producer");
        trace.push(producer.identity.clone());
        let argument = ssa
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find(|candidate| {
                candidate
                    .outputs
                    .iter()
                    .any(|output| output.identity == producer.inputs[0])
            })
            .expect("constant argument");
        trace.push(argument.identity.clone());
    }
    let fingerprints = BTreeMap::from([
        (operation.clone(), "demo-operation".to_owned()),
        (obligation.identity.clone(), "demo-obligation".to_owned()),
    ]);
    let request = VerifierRequest {
        schema_version: "0.2".to_owned(),
        obligation: obligation.clone(),
        subject: obligation.subject.clone(),
        scope: program.module.clone(),
        input: mncs_model::VerifierInput::Capability(mncs_model::CapabilityVerifierInput {
            authorized: None,
        }),
        assumptions: Vec::new(),
        dependencies: obligation.dependencies.clone(),
        dependency_fingerprints: fingerprints.clone(),
    };
    Demo {
        obligation: obligation.identity.clone(),
        operation,
        fingerprints,
        request,
        ssa,
        instruction_index,
        trace,
    }
}

fn demo_binding(demo: &Demo) -> ProofBinding {
    let artifact = ProofArtifact::new(
        PROOF_KERNEL_ID,
        demo.obligation.clone(),
        Vec::new(),
        vec![demo.operation.clone()],
        demo_cells(),
        10,
        9,
        8,
    );
    assert!(artifact.identity_is_valid());
    assert_eq!(reference_check(&artifact), ProofVerdict::Pass);
    ProofBinding::bind(&artifact, demo.fingerprints.clone()).expect("binding binds a passing proof")
}

#[test]
fn demo_proof_cells_match_the_executed_corpus_case() {
    // The artifact checked below is byte-identical to the cells the
    // differential suite executes on all five backends.
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/execution/proof-kernel-corpus.json"
    ))
    .expect("curated corpus");
    let cases = parse_proof_corpus(&text).expect("parse curated corpus");
    let plus = cases
        .iter()
        .find(|case| case.id == "plus-two-three-is-five")
        .expect("flagship corpus case");
    assert_eq!(plus.cells, demo_cells());
    assert_eq!((plus.count, plus.proof, plus.proposition), (10, 9, 8));
    assert_eq!(plus.expected, Some(ProofVerdict::Pass));
}

#[test]
fn compiler_generated_obligation_is_discharged_by_a_checked_proof() {
    let demo = setup_demo();
    let binding = demo_binding(&demo);
    assert_eq!(binding.obligation, demo.obligation);
    assert_eq!(binding.kernel, PROOF_KERNEL_ID);
    let operation = IntegerOperation {
        operator: "add".to_owned(),
        operand_type: (IntegerType {
            bits: 64,
            signed: true,
        }),
        left: 2,
        right: 3,
        intent: ArithmeticIntent::Checked,
    };
    let result = kernel_backed_range_result(&demo.request, &operation, &binding);
    assert_eq!(result.status, ObligationStatus::Pass);
    assert_eq!(result.authority, EvidenceAuthorityClass::KernelProof);
    assert_eq!(result.method, VerifierMethod::KernelProof);
    assert_eq!(result.artifact, Some(binding.proof.0.clone()));
}

#[test]
fn proof_gated_elision_consumes_the_binding() {
    let demo = setup_demo();
    let binding = demo_binding(&demo);
    let evidence = mncs_model::VerifierResult {
        schema_version: "0.2".to_owned(),
        obligation: demo.obligation.clone(),
        subject: demo.operation.clone(),
        scope: demo.operation.0.clone(),
        status: ObligationStatus::Pass,
        verifier: mncs_model::DeterministicVerifier::default()
            .identity()
            .clone(),
        method: VerifierMethod::IntegerRange,
        assumptions: Vec::new(),
        dependencies: vec![demo.operation.clone()],
        dependency_fingerprints: BTreeMap::new(),
        artifact: Some("evidence://demo-exact".to_owned()),
        limitations: Vec::new(),
        freshness: EvidenceFreshness::Current,
        authority: EvidenceAuthorityClass::Verifier,
    };
    let decision = demo.ssa.attempt_no_overflow_elision_with_proof(
        &demo.operation,
        &demo.obligation,
        &[evidence],
        &binding,
        &demo.fingerprints,
    );
    assert!(decision.permitted, "{}", decision.reason);
    assert!(decision
        .record
        .backend_promises_permitted
        .contains(&"no-overflow".to_owned()));
    assert!(
        decision.record.evidence_consumed.contains(&binding.proof),
        "transformation record names the consumed proof"
    );
}

#[test]
fn lowering_certificate_carries_the_proof_identity() {
    let demo = setup_demo();
    let binding = demo_binding(&demo);
    let instruction = &demo.ssa.functions[demo.instruction_index.0].blocks
        [demo.instruction_index.1]
        .instructions[demo.instruction_index.2];
    // The operands arrive through calls, so no constant-range certificate
    // exists for this instruction: the promise path withholds, and the
    // proof-backed constructor below is the only minting route.
    let promise = integer_no_overflow_promise(&demo.ssa, instruction);
    assert!(
        !promise.decision.permitted,
        "call-fed operands keep the constant path withheld"
    );
    let witnessed = IntegerOperation {
        operator: "add".to_owned(),
        operand_type: (IntegerType {
            bits: 64,
            signed: true,
        }),
        left: 2,
        right: 3,
        intent: ArithmeticIntent::Checked,
    };
    let certificate = proof_backed_no_overflow_certificate(
        &demo.ssa,
        instruction,
        &demo.obligation,
        &binding,
        &witnessed,
        &demo.trace,
        &demo.fingerprints,
    )
    .expect("proof-backed certificate mints for the witnessed computation");
    assert!(certificate.identity_is_valid());
    assert_eq!(certificate.proof_identity, None, "minted unbound");
    assert!(
        certificate
            .dependencies
            .iter()
            .all(|dependency| std::iter::once(&demo.ssa.identity)
                .chain(demo.trace.iter())
                .chain(std::iter::once(&demo.obligation))
                .chain(std::iter::once(&instruction.identity))
                .any(|expected| expected == dependency)),
        "dependencies carry only the module, instruction, obligation, and trace: {:?}",
        certificate.dependencies
    );
    let bound = certificate
        .with_proof_binding(&binding)
        .expect("matching binding attaches");
    assert!(bound.identity_is_valid());
    assert_eq!(bound.proof_identity, Some(binding.proof.clone()));
    assert_ne!(
        bound.identity, certificate.identity,
        "proof-bound and unbound certificates never share an identity"
    );
    // Fail-closed minting: a foreign obligation, a mismatched operator, and
    // an overflowing witness each mint nothing, even with a valid binding.
    let other = SemanticId("mncs:test:other-obligation".to_owned());
    assert!(
        proof_backed_no_overflow_certificate(
            &demo.ssa,
            instruction,
            &other,
            &binding,
            &witnessed,
            &demo.trace,
            &demo.fingerprints,
        )
        .is_none(),
        "foreign obligation mints nothing"
    );
    let mismatched = IntegerOperation {
        operator: "mul".to_owned(),
        ..witnessed.clone()
    };
    assert!(
        proof_backed_no_overflow_certificate(
            &demo.ssa,
            instruction,
            &demo.obligation,
            &binding,
            &mismatched,
            &demo.trace,
            &demo.fingerprints,
        )
        .is_none(),
        "mismatched operator mints nothing"
    );
    let overflowing = IntegerOperation {
        left: i128::MAX,
        right: 1,
        ..witnessed.clone()
    };
    assert!(
        proof_backed_no_overflow_certificate(
            &demo.ssa,
            instruction,
            &demo.obligation,
            &binding,
            &overflowing,
            &demo.trace,
            &demo.fingerprints,
        )
        .is_none(),
        "overflowing witness mints nothing: the proof alone never grants the range fact"
    );
}

#[test]
fn mutated_inputs_invalidate_proof_reuse() {
    let demo = setup_demo();
    let binding = demo_binding(&demo);
    // A tampered computation (2 + 4 instead of 2 + 3) does not check out, so
    // no binding is issued for it at all.
    let mut tampered = demo_cells();
    tampered[7] = ProofCell::new(ProofTag::Plus, [3, 5, 0, 0]);
    let bad = ProofArtifact::new(
        PROOF_KERNEL_ID,
        demo.obligation.clone(),
        Vec::new(),
        vec![demo.operation.clone()],
        tampered,
        10,
        9,
        8,
    );
    assert!(bad.identity_is_valid(), "fresh seals are well-formed");
    assert_ne!(reference_check(&bad), ProofVerdict::Pass);
    assert!(ProofBinding::bind(&bad, demo.fingerprints.clone()).is_none());
    // Forged identity: mutating a sealed artifact breaks its identity, so a
    // valid proof for other bytes can never be replayed under this one.
    let mut forged = ProofArtifact::new(
        PROOF_KERNEL_ID,
        demo.obligation.clone(),
        Vec::new(),
        vec![demo.operation.clone()],
        demo_cells(),
        10,
        9,
        8,
    );
    forged.cells[7] = ProofCell::new(ProofTag::Plus, [3, 5, 0, 0]);
    assert!(!forged.identity_is_valid());
    assert!(ProofBinding::bind(&forged, demo.fingerprints.clone()).is_none());
    // Changed dependency fingerprints invalidate the good binding.
    let stale: BTreeMap<SemanticId, String> = demo
        .fingerprints
        .keys()
        .map(|key| (key.clone(), "changed".to_owned()))
        .collect();
    assert!(!binding.reusable_if(
        &binding.proof,
        PROOF_KERNEL_ID,
        &demo.obligation,
        &[],
        &stale,
        ProofVerdict::Pass,
    ));
    let evidence = mncs_model::VerifierResult {
        schema_version: "0.2".to_owned(),
        obligation: demo.obligation.clone(),
        subject: demo.operation.clone(),
        scope: demo.operation.0.clone(),
        status: ObligationStatus::Pass,
        verifier: mncs_model::DeterministicVerifier::default()
            .identity()
            .clone(),
        method: VerifierMethod::IntegerRange,
        assumptions: Vec::new(),
        dependencies: vec![demo.operation.clone()],
        dependency_fingerprints: BTreeMap::new(),
        artifact: Some("evidence://demo-exact".to_owned()),
        limitations: Vec::new(),
        freshness: EvidenceFreshness::Current,
        authority: EvidenceAuthorityClass::Verifier,
    };
    let withheld = demo.ssa.attempt_no_overflow_elision_with_proof(
        &demo.operation,
        &demo.obligation,
        std::slice::from_ref(&evidence),
        &binding,
        &stale,
    );
    assert!(!withheld.permitted, "stale dependencies withhold elision");
    // The proof for this obligation does not transfer to another one.
    let other = SemanticId("mncs:test:other-obligation".to_owned());
    let misbound = demo.ssa.attempt_no_overflow_elision_with_proof(
        &demo.operation,
        &other,
        &[evidence],
        &binding,
        &demo.fingerprints,
    );
    assert!(!misbound.permitted, "wrong-obligation reuse is withheld");
    // An overflowing computation fails the conjunction even with a valid
    // binding shape: the proof alone never grants the range fact.
    let overflow = IntegerOperation {
        operator: "add".to_owned(),
        operand_type: (IntegerType {
            bits: 64,
            signed: true,
        }),
        left: i128::MAX,
        right: 1,
        intent: ArithmeticIntent::Checked,
    };
    let denied = kernel_backed_range_result(&demo.request, &overflow, &binding);
    assert_eq!(denied.status, ObligationStatus::Fail);
}
