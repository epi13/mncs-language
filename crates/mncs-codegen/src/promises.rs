//! Evidence-gated backend promise decisions used during lowering.
//!
//! Emitting a backend attribute is not a proof. Missing, stale, or
//! wrong-scope evidence withholds the promise and keeps conservative
//! lowering available.

use mncs_model::{
    ArithmeticIntent, BackendPromise, BackendPromiseDecision, Obligation, ObligationRecord,
    SsaInstruction, SsaInstructionKind, SsaModule,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringPromise {
    pub decision: BackendPromiseDecision,
    pub attribute: Option<String>,
}

pub fn integer_no_overflow_promise(
    module: &SsaModule,
    instruction: &SsaInstruction,
) -> LoweringPromise {
    let SsaInstructionKind::Integer { intent, .. } = &instruction.kind else {
        return withheld(
            BackendPromise::NoOverflow,
            "instruction is not integer arithmetic",
        );
    };
    match intent {
        ArithmeticIntent::Wrapping => withheld(
            BackendPromise::WrappingArithmetic,
            "wrapping intent is already the conservative arithmetic; no extra no-overflow promise is implied",
        ),
        ArithmeticIntent::Saturating | ArithmeticIntent::Widening { .. } => withheld(
            BackendPromise::NoOverflow,
            "saturating and widening arithmetic are outside this promise envelope",
        ),
        ArithmeticIntent::Checked | ArithmeticIntent::Trapping => {
            let obligations = related_obligations(module, instruction);
            let subject = instruction
                .semantic_identity
                .clone()
                .unwrap_or_else(|| instruction.identity.clone());
            let decision =
                BackendPromiseDecision::evaluate(BackendPromise::NoOverflow, &subject, &obligations);
            if decision.permitted {
                LoweringPromise {
                    attribute: Some("llvm.nsw".to_owned()),
                    decision,
                }
            } else {
                LoweringPromise {
                    attribute: None,
                    decision,
                }
            }
        }
    }
}

pub fn withheld(promise: BackendPromise, reason: impl Into<String>) -> LoweringPromise {
    LoweringPromise {
        attribute: None,
        decision: BackendPromiseDecision {
            promise,
            permitted: false,
            reason: reason.into(),
        },
    }
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

pub fn promise_summary(promise: &LoweringPromise) -> String {
    let name = format!("{:?}", promise.decision.promise);
    if promise.decision.permitted {
        format!(
            "promise {name} emitted ({}); attribute {:?}",
            promise.decision.reason, promise.attribute
        )
    } else {
        format!("promise {name} withheld: {}", promise.decision.reason)
    }
}
