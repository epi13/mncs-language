use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::body::BodyOperationKind;
use crate::canonical::sha256_hex;
use crate::identity::{assumption_id, capability_id, contract_id, effect_id, function_id};
use crate::ir::effect_obligation_identity;
use crate::{EvidenceFreshness, MachineIntentExpression, ObligationStatus, Program, SemanticId};

pub const OBLIGATION_SCHEMA_VERSION: &str = "0.2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationRecord {
    pub schema_version: String,
    pub identity: SemanticId,
    pub subject: SemanticId,
    pub requirement: SemanticId,
    pub status: ObligationStatus,
    pub method: String,
    pub assumptions: Vec<SemanticId>,
    pub dependencies: Vec<SemanticId>,
    pub freshness: EvidenceFreshness,
    pub fallback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationGeneration {
    pub schema_version: String,
    pub obligations: Vec<ObligationRecord>,
}

impl Program {
    /// Generate only obligations whose subjects and dependencies are
    /// representable in the current 0.1 semantic model. New obligations are
    /// conservative: they are PASS only for direct language-level closure
    /// checks, never merely because an obligation was requested.
    pub fn generate_obligations(&self) -> ObligationGeneration {
        let mut obligations = Vec::new();
        for function in &self.functions {
            let function_identity = function_id(&self.module, &function.name);
            let declared: BTreeSet<_> = function.capabilities.iter().cloned().collect();
            let mut occurrences = BTreeMap::<String, usize>::new();
            for effect in &function.effects {
                let canonical = serde_json::to_string(&crate::canonical::canonical_effect(effect))
                    .expect("canonical effect");
                let occurrence = occurrences.entry(canonical.clone()).or_default();
                let effect_identity =
                    effect_id(&self.module, &function.name, &canonical, *occurrence);
                let capability_identity =
                    capability_id(&self.module, &function.name, &effect.capability);
                let requirement = requirement_id("effect-authorized", &effect_identity);
                let identity = effect_obligation_identity(&effect_identity);
                let status =
                    if !effect.capability.is_empty() && declared.contains(&effect.capability) {
                        ObligationStatus::Pass
                    } else {
                        ObligationStatus::Fail
                    };
                obligations.push(ObligationRecord {
                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                    identity,
                    subject: effect_identity.clone(),
                    requirement,
                    status,
                    method: "language-effect-closure".to_owned(),
                    assumptions: function
                        .assumptions
                        .iter()
                        .map(|assumption| assumption_id(&self.module, assumption))
                        .collect(),
                    dependencies: vec![
                        function_identity.clone(),
                        effect_identity,
                        capability_identity,
                    ],
                    freshness: EvidenceFreshness::Current,
                    fallback: None,
                });
                *occurrence += 1;
            }
            for contract in &function.contracts {
                let property = contract_id(&self.module, &function.name, &contract.id);
                let evidence_present = function
                    .evidence
                    .iter()
                    .any(|claim| claim.property == contract.id);
                obligations.push(ObligationRecord {
                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                    identity: requirement_id("contract-evidence-bound", &property),
                    subject: property.clone(),
                    requirement: requirement_id("contract-evidence-bound", &property),
                    status: if evidence_present {
                        ObligationStatus::Pass
                    } else {
                        ObligationStatus::Unknown
                    },
                    method: "language-evidence-binding".to_owned(),
                    assumptions: function
                        .assumptions
                        .iter()
                        .map(|assumption| assumption_id(&self.module, assumption))
                        .collect(),
                    dependencies: vec![function_identity.clone(), property],
                    freshness: EvidenceFreshness::Current,
                    fallback: Some("retain explicit contract without backend promise".to_owned()),
                });
            }
            if let Some(body) = &function.body {
                for iteration in &body.bounded_iterations {
                    let subject =
                        crate::identity::iteration_id(&self.module, &function.name, &iteration.id);
                    let mut dependencies = vec![function_identity.clone(), subject.clone()];
                    dependencies.extend(iteration.callees.iter().cloned());
                    dependencies.extend(
                        iteration.required_capabilities.iter().map(|capability| {
                            capability_id(&self.module, &function.name, capability)
                        }),
                    );
                    dependencies.extend(iteration.body_blocks.iter().map(|block| {
                        crate::identity::block_id(&self.module, &function.name, block)
                    }));
                    dependencies.sort();
                    dependencies.dedup();
                    for (kind, status, method, fallback) in [
                        (
                            "iteration-bound-valid",
                            ObligationStatus::Pass,
                            "language-profile-0.4-bound-check",
                            None,
                        ),
                        (
                            "iteration-resource-ceiling",
                            ObligationStatus::Pass,
                            "language-static-iteration-ceiling",
                            None,
                        ),
                        (
                            "iteration-exact-resource-cost",
                            ObligationStatus::Unknown,
                            "no-exact-body-cost-evidence",
                            Some("retain the declared iteration ceiling and runtime budget"),
                        ),
                        (
                            "iteration-authority-closure",
                            ObligationStatus::Pass,
                            "language-iteration-authority-closure",
                            None,
                        ),
                        (
                            "iteration-state-preservation",
                            ObligationStatus::Pass,
                            "language-iteration-state-type-check",
                            None,
                        ),
                        (
                            "iteration-completion-modes",
                            ObligationStatus::Pass,
                            "language-iteration-completion-check",
                            None,
                        ),
                    ] {
                        obligations.push(ObligationRecord {
                            schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                            identity: body_obligation_id(kind, &subject),
                            subject: subject.clone(),
                            requirement: requirement_id(kind, &subject),
                            status,
                            method: method.to_owned(),
                            assumptions: Vec::new(),
                            dependencies: dependencies.clone(),
                            freshness: if status == ObligationStatus::Unknown {
                                EvidenceFreshness::Unknown
                            } else {
                                EvidenceFreshness::Current
                            },
                            fallback: fallback.map(str::to_owned),
                        });
                    }
                }
                for block in &body.blocks {
                    for operation in &block.operations {
                        let subject = operation.identity(&self.module, &function.name, &block.id);
                        match &operation.kind {
                            BodyOperationKind::Integer { intent, .. } => {
                                let bounded_counter_step =
                                    body.bounded_iterations.iter().any(|iteration| {
                                        iteration.backedge == block.id
                                            && operation.id.starts_with("iteration_decrement")
                                    });
                                let requirement = requirement_id("integer-overflow", &subject);
                                obligations.push(ObligationRecord {
                                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                                    identity: body_obligation_id("integer-overflow", &subject),
                                    subject: subject.clone(),
                                    requirement,
                                    status: if bounded_counter_step {
                                        ObligationStatus::Pass
                                    } else {
                                        ObligationStatus::Unknown
                                    },
                                    method: if bounded_counter_step {
                                        "language-bounded-iteration-counter-decrement".to_owned()
                                    } else {
                                        format!("symbolic-{intent:?}")
                                    },
                                    assumptions: function
                                        .assumptions
                                        .iter()
                                        .map(|assumption| {
                                            assumption_id(&self.module, assumption)
                                        })
                                        .collect(),
                                    dependencies: vec![function_identity.clone(), subject],
                                    freshness: if bounded_counter_step {
                                        EvidenceFreshness::Current
                                    } else {
                                        EvidenceFreshness::Unknown
                                    },
                                    fallback: (!bounded_counter_step).then(|| {
                                        "retain explicit arithmetic behavior or insert a runtime check"
                                            .to_owned()
                                    }),
                                });
                            }
                            BodyOperationKind::Effect { effect, .. } => {
                                let effect_identity = effect_id(
                                    &self.module,
                                    &function.name,
                                    &serde_json::to_string(&crate::canonical::canonical_effect(
                                        effect,
                                    ))
                                    .expect("body effect"),
                                    function
                                        .effects
                                        .iter()
                                        .position(|declared| declared == effect)
                                        .unwrap_or(0),
                                );
                                obligations.push(ObligationRecord {
                                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                                    identity: body_obligation_id("effect-authorized", &subject),
                                    subject: subject.clone(),
                                    requirement: requirement_id("effect-authorized", &subject),
                                    status: if function.capabilities.contains(&effect.capability) {
                                        ObligationStatus::Pass
                                    } else {
                                        ObligationStatus::Fail
                                    },
                                    method: "body-effect-closure".to_owned(),
                                    assumptions: Vec::new(),
                                    dependencies: vec![subject, effect_identity],
                                    freshness: EvidenceFreshness::Current,
                                    fallback: None,
                                });
                            }
                            BodyOperationKind::RuntimeCheck { obligation, .. } => {
                                obligations.push(ObligationRecord {
                                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                                    identity: body_obligation_id("runtime-check", &subject),
                                    subject,
                                    requirement: obligation.clone(),
                                    status: ObligationStatus::Unknown,
                                    method: "runtime-check-establishment".to_owned(),
                                    assumptions: Vec::new(),
                                    dependencies: vec![obligation.clone()],
                                    freshness: EvidenceFreshness::Unknown,
                                    fallback: Some(
                                        "the fact is available only on the successful runtime path"
                                            .to_owned(),
                                    ),
                                });
                            }
                            BodyOperationKind::Call {
                                function: callee,
                                required_capabilities,
                                effects,
                                ..
                            } => {
                                let authorized = required_capabilities
                                    .iter()
                                    .all(|capability| function.capabilities.contains(capability))
                                    && effects.iter().all(|effect| {
                                        function.effects.iter().any(|caller_effect| {
                                            caller_effect.kind == effect.kind
                                                && caller_effect.capability == effect.capability
                                        })
                                    });
                                let mut dependencies = vec![
                                    function_identity.clone(),
                                    callee.clone(),
                                    subject.clone(),
                                ];
                                dependencies.extend(required_capabilities.iter().map(
                                    |capability| {
                                        capability_id(&self.module, &function.name, capability)
                                    },
                                ));
                                obligations.push(ObligationRecord {
                                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                                    identity: body_obligation_id(
                                        "call-authority-closure",
                                        &subject,
                                    ),
                                    subject,
                                    requirement: requirement_id("call-authority-closure", callee),
                                    status: if authorized {
                                        ObligationStatus::Pass
                                    } else {
                                        ObligationStatus::Fail
                                    },
                                    method: "language-call-authority-closure".to_owned(),
                                    assumptions: Vec::new(),
                                    dependencies,
                                    freshness: EvidenceFreshness::Current,
                                    fallback: None,
                                });
                            }
                            BodyOperationKind::IntegerCompare { .. }
                            | BodyOperationKind::FiniteConstruct { .. }
                            | BodyOperationKind::FiniteIsVariant { .. } => {}
                            BodyOperationKind::Constant { .. } => {}
                        }
                        if let Some(machine_intent) = &operation.machine_intent {
                            for requirement in &machine_intent.requirements {
                                obligations.push(ObligationRecord {
                                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                                    identity: body_obligation_id(
                                        "machine-intent",
                                        &requirement.identity,
                                    ),
                                    subject: operation.identity(
                                        &self.module,
                                        &function.name,
                                        &block.id,
                                    ),
                                    requirement: requirement.identity.clone(),
                                    status: ObligationStatus::Unknown,
                                    method: "body-machine-intent".to_owned(),
                                    assumptions: Vec::new(),
                                    dependencies: vec![
                                        operation.identity(&self.module, &function.name, &block.id),
                                        requirement.identity.clone(),
                                    ],
                                    freshness: EvidenceFreshness::Unknown,
                                    fallback: Some(
                                        "use the declared conservative realization".to_owned(),
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }
        obligations.sort_by(|left, right| left.identity.cmp(&right.identity));
        ObligationGeneration {
            schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
            obligations,
        }
    }
}

pub fn generate_machine_intent_obligations(
    expression: &MachineIntentExpression,
) -> ObligationGeneration {
    let mut obligations = expression
        .requirements
        .iter()
        .map(|requirement| ObligationRecord {
            schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
            identity: requirement_id("machine-intent", &requirement.identity),
            subject: requirement.subject.clone(),
            requirement: requirement.identity.clone(),
            status: ObligationStatus::Unknown,
            method: "obligation-generation".to_owned(),
            assumptions: Vec::new(),
            dependencies: vec![requirement.subject.clone(), requirement.identity.clone()],
            freshness: EvidenceFreshness::Unknown,
            fallback: Some("use the declared conservative realization".to_owned()),
        })
        .collect::<Vec<_>>();
    obligations.sort_by(|left, right| left.identity.cmp(&right.identity));
    ObligationGeneration {
        schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
        obligations,
    }
}

pub(crate) fn requirement_id(kind: &str, subject: &SemanticId) -> SemanticId {
    SemanticId(format!(
        "mncs:0.2:requirement:{kind}:{}",
        sha256_hex(subject.0.as_bytes())
    ))
}

pub(crate) fn body_obligation_id(kind: &str, subject: &SemanticId) -> SemanticId {
    SemanticId(format!(
        "mncs:0.2:obligation:body:{kind}:{}",
        sha256_hex(subject.0.as_bytes())
    ))
}

#[cfg(test)]
mod tests {
    use crate::validation::tests::valid_program;
    use crate::{
        ArithmeticIntent, EvidenceFreshness, IntegerOperation, IntegerType, Intent,
        MachineIntentExpression, ObligationStatus, Requirement, SemanticId,
    };

    #[test]
    fn language_generation_marks_effect_closure_pass_and_missing_contract_evidence_unknown() {
        let generation = valid_program().generate_obligations();
        assert!(generation
            .obligations
            .iter()
            .any(|obligation| obligation.status == ObligationStatus::Pass));
        assert!(generation
            .obligations
            .iter()
            .any(|obligation| obligation.method == "language-evidence-binding"));
        assert!(generation
            .obligations
            .iter()
            .all(|obligation| obligation.freshness == EvidenceFreshness::Current));
    }

    #[test]
    fn generated_machine_intent_obligations_start_unknown() {
        let operation = IntegerOperation {
            operator: "add".to_owned(),
            operand_type: IntegerType {
                bits: 8,
                signed: true,
            },
            left: 1,
            right: 2,
            intent: ArithmeticIntent::Checked,
        };
        let expression = MachineIntentExpression {
            operation: operation.clone(),
            intent: Intent {
                identity: SemanticId("intent:checked".to_owned()),
                statement: "checked".to_owned(),
            },
            preferences: Vec::new(),
            facts: Vec::new(),
            requirements: vec![Requirement {
                identity: SemanticId("requirement:no-overflow".to_owned()),
                subject: operation.identity(),
                statement: "no overflow".to_owned(),
            }],
            obligations: Vec::new(),
        };
        let generation = crate::generate_machine_intent_obligations(&expression);
        assert_eq!(generation.obligations[0].status, ObligationStatus::Unknown);
        assert_eq!(
            generation.obligations[0].freshness,
            EvidenceFreshness::Unknown
        );
    }
}
