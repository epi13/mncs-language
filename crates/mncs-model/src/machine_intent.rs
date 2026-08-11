use serde::{Deserialize, Serialize};

use crate::canonical::sha256_hex;
use crate::{EvidenceFreshness, SemanticId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegerType {
    pub bits: u16,
    pub signed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArithmeticIntent {
    Wrapping,
    Checked,
    Saturating,
    Trapping,
    Widening { bits: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegerOperation {
    pub operator: String,
    pub operand_type: IntegerType,
    pub left: i128,
    pub right: i128,
    pub intent: ArithmeticIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegerEvaluation {
    pub value: Option<i128>,
    pub overflow: bool,
    pub trapped: bool,
    pub result_bits: u16,
}

impl IntegerOperation {
    pub fn identity(&self) -> SemanticId {
        let json = serde_json::to_string(self).expect("integer operation is serializable");
        SemanticId(format!(
            "mncs:0.2:operation:{}:{}",
            self.operator,
            sha256_hex(json.as_bytes())
        ))
    }

    pub fn evaluate(&self) -> IntegerEvaluation {
        let result_bits = match self.intent {
            ArithmeticIntent::Widening { bits } => bits,
            _ => self.operand_type.bits,
        };
        let (minimum, maximum) = limits(result_bits, self.operand_type.signed);
        let raw = match self.operator.as_str() {
            "add" => self.left.checked_add(self.right),
            "sub" => self.left.checked_sub(self.right),
            "mul" => self.left.checked_mul(self.right),
            "and" | "or" | "xor" if matches!(self.intent, ArithmeticIntent::Wrapping) => bitwise(
                self.operator.as_str(),
                self.operand_type,
                self.left,
                self.right,
            ),
            _ => None,
        };
        let overflow = raw
            .as_ref()
            .map(|value| *value < minimum || *value > maximum)
            .unwrap_or(true);
        match self.intent {
            ArithmeticIntent::Wrapping => IntegerEvaluation {
                value: raw
                    .map(|value| wrap(value, self.operand_type.bits, self.operand_type.signed)),
                overflow,
                trapped: false,
                result_bits,
            },
            ArithmeticIntent::Checked => IntegerEvaluation {
                value: if !overflow { raw } else { None },
                overflow,
                trapped: false,
                result_bits,
            },
            ArithmeticIntent::Saturating => IntegerEvaluation {
                value: raw.map(|value| value.clamp(minimum, maximum)),
                overflow,
                trapped: false,
                result_bits,
            },
            ArithmeticIntent::Trapping => IntegerEvaluation {
                value: if !overflow { raw } else { None },
                overflow,
                trapped: overflow,
                result_bits,
            },
            ArithmeticIntent::Widening { .. } => IntegerEvaluation {
                value: if !overflow { raw } else { None },
                overflow,
                trapped: false,
                result_bits,
            },
        }
    }
}

fn bitwise(operator: &str, ty: IntegerType, left: i128, right: i128) -> Option<i128> {
    let (_, maximum) = limits(ty.bits, false);
    let modulus = maximum.checked_add(1)?;
    let left = left.rem_euclid(modulus);
    let right = right.rem_euclid(modulus);
    let value = match operator {
        "and" => left & right,
        "or" => left | right,
        "xor" => left ^ right,
        _ => return None,
    };
    if ty.signed {
        let sign = 1_i128.checked_shl(u32::from(ty.bits - 1))?;
        Some(if value >= sign {
            value - modulus
        } else {
            value
        })
    } else {
        Some(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intent {
    pub identity: SemanticId,
    pub statement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachinePreference {
    pub identity: SemanticId,
    pub statement: String,
}

pub type Preference = MachinePreference;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    pub identity: SemanticId,
    pub subject: SemanticId,
    pub scope: String,
    pub statement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requirement {
    pub identity: SemanticId,
    pub subject: SemanticId,
    pub statement: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ObligationStatus {
    Pass,
    Fail,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Obligation {
    pub identity: SemanticId,
    pub subject: SemanticId,
    pub requirement: SemanticId,
    pub status: ObligationStatus,
    pub freshness: EvidenceFreshness,
    pub method: String,
}

/// A small evidence-bearing memory capability pilot. The exact subject and
/// scope are part of the capability, so alignment evidence cannot be reused
/// for another allocation or region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignmentCapability {
    pub identity: SemanticId,
    pub subject: SemanticId,
    pub scope: String,
    pub alignment: u64,
    pub evidence: Obligation,
}

impl AlignmentCapability {
    pub fn permits(&self, subject: &SemanticId, scope: &str, required_alignment: u64) -> bool {
        self.subject == *subject
            && self.scope == scope
            && self.alignment >= required_alignment
            && self.evidence.subject == *subject
            && self.evidence.freshness == EvidenceFreshness::Current
            && self.evidence.status == ObligationStatus::Pass
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineIntentExpression {
    pub operation: IntegerOperation,
    pub intent: Intent,
    pub preferences: Vec<MachinePreference>,
    pub facts: Vec<Fact>,
    pub requirements: Vec<Requirement>,
    pub obligations: Vec<Obligation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighLevelIrNode {
    pub operation_identity: SemanticId,
    pub operation: IntegerOperation,
    pub intent_identity: SemanticId,
    pub fact_identities: Vec<SemanticId>,
    pub requirement_identities: Vec<SemanticId>,
    pub obligation_identities: Vec<SemanticId>,
}

impl MachineIntentExpression {
    pub fn lower_to_high_level_ir(&self) -> HighLevelIrNode {
        HighLevelIrNode {
            operation_identity: self.operation.identity(),
            operation: self.operation.clone(),
            intent_identity: self.intent.identity.clone(),
            fact_identities: self
                .facts
                .iter()
                .map(|fact| fact.identity.clone())
                .collect(),
            requirement_identities: self
                .requirements
                .iter()
                .map(|requirement| requirement.identity.clone())
                .collect(),
            obligation_identities: self
                .obligations
                .iter()
                .map(|obligation| obligation.identity.clone())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendPromise {
    NoOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendPromiseDecision {
    pub promise: BackendPromise,
    pub permitted: bool,
    pub reason: String,
}

impl BackendPromiseDecision {
    pub fn evaluate(
        promise: BackendPromise,
        subject: &SemanticId,
        obligations: &[Obligation],
    ) -> Self {
        let required = obligations.iter().find(|obligation| {
            obligation.subject == *subject && obligation.requirement.0.ends_with("no-overflow")
        });
        let Some(obligation) = required else {
            return Self {
                promise,
                permitted: false,
                reason: "required obligation is absent".to_owned(),
            };
        };
        let permitted = obligation.status == ObligationStatus::Pass
            && obligation.freshness == EvidenceFreshness::Current;
        let reason = if permitted {
            "exact scoped obligation is current and PASS".to_owned()
        } else {
            format!(
                "obligation is {:?} or {:?}",
                obligation.status, obligation.freshness
            )
        };
        Self {
            promise,
            permitted,
            reason,
        }
    }
}

fn limits(bits: u16, signed: bool) -> (i128, i128) {
    assert!(
        (1..=126).contains(&bits),
        "executable pilot supports 1..=126 bits"
    );
    if signed {
        let top = 1_i128 << (bits - 1);
        (-top, top - 1)
    } else {
        (0, (1_i128 << bits) - 1)
    }
}

fn wrap(value: i128, bits: u16, signed: bool) -> i128 {
    let (_, maximum) = limits(bits, false);
    let modulus = maximum + 1;
    let wrapped = value.rem_euclid(modulus);
    if signed {
        let sign = 1_i128 << (bits - 1);
        if wrapped >= sign {
            wrapped - modulus
        } else {
            wrapped
        }
    } else {
        wrapped
    }
}

impl Intent {
    pub fn identity(&self) -> SemanticId {
        self.identity.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(intent: ArithmeticIntent) -> IntegerOperation {
        IntegerOperation {
            operator: "add".to_owned(),
            operand_type: IntegerType {
                bits: 8,
                signed: true,
            },
            left: 127,
            right: 1,
            intent,
        }
    }

    #[test]
    fn arithmetic_intents_have_distinct_explicit_semantics() {
        assert_eq!(
            operation(ArithmeticIntent::Wrapping).evaluate().value,
            Some(-128)
        );
        assert_eq!(operation(ArithmeticIntent::Checked).evaluate().value, None);
        assert_eq!(
            operation(ArithmeticIntent::Saturating).evaluate().value,
            Some(127)
        );
        assert!(operation(ArithmeticIntent::Trapping).evaluate().trapped);
    }

    #[test]
    fn scalar_integer_operators_have_explicit_edge_behavior() {
        let ty = IntegerType {
            bits: 8,
            signed: false,
        };
        let mut subtraction = operation(ArithmeticIntent::Wrapping);
        subtraction.operator = "sub".to_owned();
        subtraction.operand_type = ty;
        subtraction.left = 0;
        subtraction.right = 1;
        assert_eq!(subtraction.evaluate().value, Some(255));

        let mut multiplication = operation(ArithmeticIntent::Saturating);
        multiplication.operator = "mul".to_owned();
        multiplication.operand_type = ty;
        multiplication.left = 255;
        multiplication.right = 2;
        assert_eq!(multiplication.evaluate().value, Some(255));

        let mut bitwise = operation(ArithmeticIntent::Wrapping);
        bitwise.operator = "xor".to_owned();
        bitwise.operand_type = IntegerType {
            bits: 8,
            signed: true,
        };
        bitwise.left = -1;
        bitwise.right = 1;
        assert_eq!(bitwise.evaluate().value, Some(-2));
    }

    #[test]
    fn lowering_preserves_operation_and_obligation_identity() {
        let operation = operation(ArithmeticIntent::Checked);
        let expression = MachineIntentExpression {
            operation: operation.clone(),
            intent: Intent {
                identity: SemanticId("intent:add-safe".to_owned()),
                statement: "preserve overflow result".to_owned(),
            },
            preferences: vec![MachinePreference {
                identity: SemanticId("preference:branchless".to_owned()),
                statement: "prefer branchless".to_owned(),
            }],
            facts: vec![],
            requirements: vec![Requirement {
                identity: SemanticId("requirement:no-overflow".to_owned()),
                subject: operation.identity(),
                statement: "no overflow".to_owned(),
            }],
            obligations: vec![Obligation {
                identity: SemanticId("obligation:no-overflow".to_owned()),
                subject: operation.identity(),
                requirement: SemanticId("requirement:no-overflow".to_owned()),
                status: ObligationStatus::Unknown,
                freshness: EvidenceFreshness::Current,
                method: "pilot".to_owned(),
            }],
        };
        let ir = expression.lower_to_high_level_ir();
        assert_eq!(ir.operation_identity, operation.identity());
        assert_eq!(
            ir.obligation_identities,
            vec![SemanticId("obligation:no-overflow".to_owned())]
        );
        assert_eq!(
            ir.requirement_identities,
            vec![SemanticId("requirement:no-overflow".to_owned())]
        );
    }

    #[test]
    fn backend_promises_require_current_pass_for_the_exact_subject() {
        let subject = SemanticId("operation:one".to_owned());
        let obligation = Obligation {
            identity: SemanticId("obligation:no-overflow".to_owned()),
            subject: subject.clone(),
            requirement: SemanticId("requirement:no-overflow".to_owned()),
            status: ObligationStatus::Pass,
            freshness: EvidenceFreshness::Current,
            method: "verifier".to_owned(),
        };
        assert!(
            BackendPromiseDecision::evaluate(
                BackendPromise::NoOverflow,
                &subject,
                std::slice::from_ref(&obligation)
            )
            .permitted
        );
        assert!(
            !BackendPromiseDecision::evaluate(
                BackendPromise::NoOverflow,
                &SemanticId("operation:other".to_owned()),
                std::slice::from_ref(&obligation)
            )
            .permitted
        );
        let mut stale = obligation.clone();
        stale.freshness = EvidenceFreshness::Stale;
        assert!(
            !BackendPromiseDecision::evaluate(BackendPromise::NoOverflow, &subject, &[stale])
                .permitted
        );
        let mut unknown = obligation;
        unknown.status = ObligationStatus::Unknown;
        assert!(
            !BackendPromiseDecision::evaluate(BackendPromise::NoOverflow, &subject, &[unknown])
                .permitted
        );
    }

    #[test]
    fn alignment_capability_rejects_mismatched_subject_or_scope() {
        let subject = SemanticId("allocation:a".to_owned());
        let capability = AlignmentCapability {
            identity: SemanticId("capability:alignment:a".to_owned()),
            subject: subject.clone(),
            scope: "call:one".to_owned(),
            alignment: 64,
            evidence: Obligation {
                identity: SemanticId("obligation:aligned".to_owned()),
                subject: subject.clone(),
                requirement: SemanticId("requirement:aligned".to_owned()),
                status: ObligationStatus::Pass,
                freshness: EvidenceFreshness::Current,
                method: "alignment-verifier".to_owned(),
            },
        };
        assert!(capability.permits(&subject, "call:one", 32));
        assert!(!capability.permits(&SemanticId("allocation:b".to_owned()), "call:one", 32));
        assert!(!capability.permits(&subject, "call:two", 32));
    }
}
