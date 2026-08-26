//! Evidence-gated backend promise decisions used during lowering.
//!
//! Emitting a backend attribute is not a proof. Missing, stale, wrong-scope,
//! or non-checkable evidence withholds the promise and keeps conservative
//! lowering available.

use mncs_model::{
    ArithmeticIntent, BackendPromise, BackendPromiseCertificate, BackendPromiseDecision,
    EvidenceFreshness, IntegerOperation, Obligation, ObligationRecord, ObligationStatus,
    SemanticId, SsaInstruction, SsaInstructionKind, SsaModule,
};

const CONSTANT_RANGE_CHECKER: &str = "mncs:verifier:constant-integer-range:0.1";
const CONSTANT_RANGE_METHOD: &str = "exact-constant-integer-range-v0.1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringPromise {
    pub decision: BackendPromiseDecision,
    pub attribute: Option<String>,
}

pub fn integer_no_overflow_promise(
    module: &SsaModule,
    instruction: &SsaInstruction,
) -> LoweringPromise {
    let subject = instruction
        .semantic_identity
        .clone()
        .unwrap_or_else(|| instruction.identity.clone());
    let SsaInstructionKind::Integer { intent, .. } = &instruction.kind else {
        return withheld(
            BackendPromise::NoOverflow,
            subject,
            "instruction is not integer arithmetic",
        );
    };
    match intent {
        ArithmeticIntent::Wrapping => withheld(
            BackendPromise::WrappingArithmetic,
            subject,
            "wrapping intent is already conservative arithmetic; no no-overflow promise is implied",
        ),
        ArithmeticIntent::Saturating | ArithmeticIntent::Widening { .. } => withheld(
            BackendPromise::NoOverflow,
            subject,
            "saturating and widening semantics do not consume a no-overflow backend promise",
        ),
        ArithmeticIntent::Checked | ArithmeticIntent::Trapping => {
            if let Some(certificate) = search_constant_no_overflow_certificate(module, instruction)
            {
                if verify_constant_no_overflow_certificate(module, instruction, &certificate) {
                    let signed = matches!(
                        instruction.kind,
                        SsaInstructionKind::Integer {
                            operand_type: mncs_model::IntegerType { signed: true, .. },
                            ..
                        }
                    );
                    let attribute = if signed { "llvm.nsw" } else { "llvm.nuw" }.to_owned();
                    return LoweringPromise {
                        attribute: Some(attribute.clone()),
                        decision: BackendPromiseDecision {
                            promise: BackendPromise::NoOverflow,
                            subject,
                            permitted: true,
                            reason: "independently re-checked exact constant-range certificate is current and PASS".to_owned(),
                            obligation: Some(certificate.obligation.clone()),
                            evidence_status: Some(ObligationStatus::Pass),
                            evidence_freshness: Some(EvidenceFreshness::Current),
                            attribute: Some(attribute),
                            certificate: Some(certificate),
                        },
                    };
                }
            }
            let obligations = related_obligations(module, instruction);
            let mut decision = BackendPromiseDecision::evaluate(
                BackendPromise::NoOverflow,
                &subject,
                &obligations,
            );
            if decision.permitted {
                decision.permitted = false;
                decision.reason =
                    "current PASS obligation lacks an independently checkable lowering certificate"
                        .to_owned();
            }
            LoweringPromise {
                attribute: None,
                decision,
            }
        }
    }
}

pub fn withheld(
    promise: BackendPromise,
    subject: SemanticId,
    reason: impl Into<String>,
) -> LoweringPromise {
    LoweringPromise {
        attribute: None,
        decision: BackendPromiseDecision {
            promise,
            subject,
            permitted: false,
            reason: reason.into(),
            obligation: None,
            evidence_status: None,
            evidence_freshness: None,
            attribute: None,
            certificate: None,
        },
    }
}

pub fn non_arithmetic_promise_decisions(
    instruction: &SsaInstruction,
) -> Vec<BackendPromiseDecision> {
    let subject = instruction
        .semantic_identity
        .clone()
        .unwrap_or_else(|| instruction.identity.clone());
    [
        (
            BackendPromise::InBounds,
            "selected SSA instruction is not a memory access; no in-bounds promise applies",
        ),
        (
            BackendPromise::Alignment,
            "selected SSA instruction has no address or alignment observation",
        ),
        (
            BackendPromise::NonAliasing,
            "selected SSA instruction has no reference/provenance operands",
        ),
        (
            BackendPromise::Relaxation,
            "no target-specific relaxation was requested or evidenced",
        ),
    ]
    .into_iter()
    .map(|(promise, reason)| withheld(promise, subject.clone(), reason).decision)
    .collect()
}

fn related_obligations(module: &SsaModule, instruction: &SsaInstruction) -> Vec<Obligation> {
    module
        .obligations
        .iter()
        .filter(|record| obligation_applies(record, instruction))
        .map(record_to_obligation)
        .collect()
}

fn obligation_applies(record: &ObligationRecord, instruction: &SsaInstruction) -> bool {
    instruction.obligations.contains(&record.identity)
        || instruction.semantic_identity.as_ref() == Some(&record.subject)
        || record.subject == instruction.identity
}

fn record_to_obligation(record: &ObligationRecord) -> Obligation {
    Obligation {
        identity: record.identity.clone(),
        subject: record.subject.clone(),
        requirement: record.requirement.clone(),
        status: record.status,
        freshness: record.freshness,
        method: record.method.clone(),
    }
}

fn constant_producer<'a>(
    module: &'a SsaModule,
    value: &SemanticId,
) -> Option<(&'a SsaInstruction, i128)> {
    module
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| {
            if !instruction
                .outputs
                .iter()
                .any(|output| &output.identity == value)
            {
                return None;
            }
            match instruction.kind {
                SsaInstructionKind::Constant { value, .. } => Some((instruction, value)),
                _ => None,
            }
        })
}

fn integer_overflow_obligation(module: &SsaModule, subject: &SemanticId) -> Option<SemanticId> {
    module
        .obligations
        .iter()
        .find(|record| {
            record.subject == *subject && record.requirement.0.contains("integer-overflow")
        })
        .map(|record| record.identity.clone())
}

fn search_constant_no_overflow_certificate(
    module: &SsaModule,
    instruction: &SsaInstruction,
) -> Option<BackendPromiseCertificate> {
    let SsaInstructionKind::Integer {
        operator,
        operand_type,
        intent: ArithmeticIntent::Checked | ArithmeticIntent::Trapping,
    } = &instruction.kind
    else {
        return None;
    };
    let (left_instruction, left) = constant_producer(module, instruction.inputs.first()?)?;
    let (right_instruction, right) = constant_producer(module, instruction.inputs.get(1)?)?;
    let evaluation = IntegerOperation {
        operator: operator.clone(),
        operand_type: *operand_type,
        left,
        right,
        intent: ArithmeticIntent::Checked,
    }
    .evaluate();
    if evaluation.overflow || evaluation.value.is_none() {
        return None;
    }
    let subject = instruction
        .semantic_identity
        .clone()
        .unwrap_or_else(|| instruction.identity.clone());
    let obligation = integer_overflow_obligation(module, &subject)?;
    Some(BackendPromiseCertificate::new(
        BackendPromise::NoOverflow,
        module.identity.clone(),
        subject,
        obligation.clone(),
        SemanticId(CONSTANT_RANGE_CHECKER.to_owned()),
        CONSTANT_RANGE_METHOD,
        vec![
            module.identity.clone(),
            instruction.identity.clone(),
            left_instruction.identity.clone(),
            right_instruction.identity.clone(),
            obligation,
        ],
    ))
}

pub fn verify_constant_no_overflow_certificate(
    module: &SsaModule,
    instruction: &SsaInstruction,
    certificate: &BackendPromiseCertificate,
) -> bool {
    if !certificate.identity_is_valid()
        || certificate.promise != BackendPromise::NoOverflow
        || certificate.selected_ssa != module.identity
        || certificate.verifier != SemanticId(CONSTANT_RANGE_CHECKER.to_owned())
        || certificate.method != CONSTANT_RANGE_METHOD
    {
        return false;
    }
    let SsaInstructionKind::Integer {
        operator,
        operand_type,
        intent: ArithmeticIntent::Checked | ArithmeticIntent::Trapping,
    } = &instruction.kind
    else {
        return false;
    };
    let Some(left_value) = instruction.inputs.first() else {
        return false;
    };
    let Some(right_value) = instruction.inputs.get(1) else {
        return false;
    };
    let Some((left_instruction, left)) = constant_producer(module, left_value) else {
        return false;
    };
    let Some((right_instruction, right)) = constant_producer(module, right_value) else {
        return false;
    };
    let evaluation = IntegerOperation {
        operator: operator.clone(),
        operand_type: *operand_type,
        left,
        right,
        intent: ArithmeticIntent::Checked,
    }
    .evaluate();
    if evaluation.overflow || evaluation.value.is_none() {
        return false;
    }
    let subject = instruction
        .semantic_identity
        .clone()
        .unwrap_or_else(|| instruction.identity.clone());
    let Some(obligation) = integer_overflow_obligation(module, &subject) else {
        return false;
    };
    let expected = BackendPromiseCertificate::new(
        BackendPromise::NoOverflow,
        module.identity.clone(),
        subject,
        obligation.clone(),
        SemanticId(CONSTANT_RANGE_CHECKER.to_owned()),
        CONSTANT_RANGE_METHOD,
        vec![
            module.identity.clone(),
            instruction.identity.clone(),
            left_instruction.identity.clone(),
            right_instruction.identity.clone(),
            obligation,
        ],
    );
    *certificate == expected
}

pub fn promise_summary(promise: &LoweringPromise) -> String {
    let name = format!("{:?}", promise.decision.promise);
    if promise.decision.permitted {
        format!(
            "promise {name} emitted ({}); attribute {:?}",
            promise.decision.reason, promise.decision.attribute
        )
    } else {
        format!("promise {name} withheld: {}", promise.decision.reason)
    }
}

/// Branchless-realization promise decisions for a selection instruction.
///
/// Lowering records the request and the current obligation state; it never
/// grants the promise. Only an independent structural verifier inspecting
/// the emitted artifact may report PASS, and even then the final native
/// machine code remains UNKNOWN unless disassembly evidence exists.
pub fn branchless_promise_decisions(
    module: &SsaModule,
    instruction: &SsaInstruction,
) -> Vec<BackendPromiseDecision> {
    if !matches!(instruction.kind, SsaInstructionKind::Select { .. }) {
        return Vec::new();
    }
    let subject = instruction
        .semantic_identity
        .clone()
        .unwrap_or_else(|| instruction.identity.clone());
    let obligations = related_obligations(module, instruction);
    let decision = BackendPromiseDecision::evaluate(
        BackendPromise::BranchlessRealization,
        &subject,
        &obligations,
    );
    vec![decision]
}
