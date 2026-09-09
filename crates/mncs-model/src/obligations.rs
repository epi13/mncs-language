use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::body::BodyOperationKind;
use crate::canonical::sha256_hex;
use crate::identity::{assumption_id, capability_id, contract_id, effect_id, function_id};
use crate::ir::effect_obligation_identity;
use crate::{
    Effect, EvidenceFreshness, MachineIntentExpression, ObligationStatus, Program, SemanticId,
};

pub const OBLIGATION_SCHEMA_VERSION: &str = "0.2";

/// Whether a body operation identity names the elaborator's bounded-loop
/// counter decrement. Elaborator temporaries live under the unspellable
/// `$mncs$` namespace, so no source binding can match this shape; matching
/// the hygienic form (rather than a bare `iteration_decrement` prefix, which
/// source code could once spell) keeps the discharge both total and sound.
fn is_iteration_decrement(id: &str) -> bool {
    id.starts_with("$mncs$iteration_decrement$")
}

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
            let namespace = function.identity_namespace(&self.module);
            let function_identity = function_id(namespace, &function.name);
            let declared: BTreeSet<_> = function.capabilities.iter().cloned().collect();
            let mut occurrences = BTreeMap::<String, usize>::new();
            for effect in &function.effects {
                let canonical = serde_json::to_string(&crate::canonical::canonical_effect(effect))
                    .expect("canonical effect");
                let occurrence = occurrences.entry(canonical.clone()).or_default();
                let effect_identity = effect_id(namespace, &function.name, &canonical, *occurrence);
                let capability_identity =
                    capability_id(namespace, &function.name, &effect.capability);
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
                        .map(|assumption| assumption_id(namespace, assumption))
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
                let property = contract_id(namespace, &function.name, &contract.id);
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
                        .map(|assumption| assumption_id(namespace, assumption))
                        .collect(),
                    dependencies: vec![function_identity.clone(), property],
                    freshness: EvidenceFreshness::Current,
                    fallback: Some("retain explicit contract without backend promise".to_owned()),
                });
            }
            if let Some(body) = &function.body {
                // Compiler-known integer literals by value identity, so
                // checked-division facts with literal divisors discharge
                // statically below (ENG-PRESSURE-0008). The table maps a
                // constant result value to its exact value and width; it is
                // generation-time routing over elaborated facts, never proof
                // authority — admission of any proof claim stays in the MNCS
                // proof-kernel path.
                let body_constants = collect_body_constants(body);
                for iteration in &body.bounded_iterations {
                    let subject =
                        crate::identity::iteration_id(namespace, &function.name, &iteration.id);
                    let mut dependencies = vec![function_identity.clone(), subject.clone()];
                    dependencies.extend(iteration.callees.iter().cloned());
                    dependencies.extend(
                        iteration
                            .required_capabilities
                            .iter()
                            .map(|capability| capability_id(namespace, &function.name, capability)),
                    );
                    dependencies.extend(
                        iteration.body_blocks.iter().map(|block| {
                            crate::identity::block_id(namespace, &function.name, block)
                        }),
                    );
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
                        let subject = operation.identity(namespace, &function.name, &block.id);
                        match &operation.kind {
                            BodyOperationKind::RecordConstruct { .. }
                            | BodyOperationKind::RecordProject { .. }
                            | BodyOperationKind::FinitePayloadProject { .. }
                            | BodyOperationKind::BooleanOp { .. }
                            // Byte bitwise/shift/compare, float constants, and
                            // explicit conversions are total by definition;
                            // sequence construction and length observations
                            // cannot fail.
                            | BodyOperationKind::FloatConstant { .. }
                            | BodyOperationKind::ByteBitwise { .. }
                            | BodyOperationKind::ByteShift { .. }
                            | BodyOperationKind::ByteCompare { .. }
                            | BodyOperationKind::Convert { .. }
                            | BodyOperationKind::Select { .. }
                            | BodyOperationKind::VectorConstruct { .. }
                            | BodyOperationKind::VectorSplat { .. }
                            | BodyOperationKind::VectorCompare { .. }
                            | BodyOperationKind::MaskBinary { .. }
                            | BodyOperationKind::MaskNot { .. }
                            | BodyOperationKind::MaskReduce { .. }
                            | BodyOperationKind::VectorExtract { evidence: crate::BoundsEvidence::StaticExact | crate::BoundsEvidence::TraversalDomain, .. }
                            | BodyOperationKind::VectorReplace { evidence: crate::BoundsEvidence::StaticExact | crate::BoundsEvidence::TraversalDomain, .. }
                            | BodyOperationKind::SequenceReplace {
                                evidence: crate::BoundsEvidence::StaticExact,
                                ..
                            }
                            | BodyOperationKind::SequenceReplace {
                                evidence: crate::BoundsEvidence::TraversalDomain,
                                ..
                            }
                            | BodyOperationKind::SequenceConstruct { .. }
                            | BodyOperationKind::SequenceLength { .. } => {}
                            BodyOperationKind::SequenceReplace {
                                evidence: crate::BoundsEvidence::RuntimeChecked { failure: _ },
                                ..
                            } => {
                                let requirement =
                                    requirement_id("sequence-replace-bounds", &subject);
                                obligations.push(ObligationRecord {
                                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                                    identity: body_obligation_id(
                                        "sequence-replace-bounds",
                                        &subject,
                                    ),
                                    subject,
                                    requirement,
                                    status: ObligationStatus::Unknown,
                                    method: "runtime-checked-functional-update".to_owned(),
                                    assumptions: Vec::new(),
                                    dependencies: Vec::new(),
                                    freshness: EvidenceFreshness::Unknown,
                                    fallback: Some(
                                        "checked update with explicit runtime failure"
                                            .to_owned(),
                                    ),
                                });
                            }
                            BodyOperationKind::VectorExtract { evidence: crate::BoundsEvidence::RuntimeChecked { .. }, .. }
                            | BodyOperationKind::VectorReplace { evidence: crate::BoundsEvidence::RuntimeChecked { .. }, .. } => {
                                let requirement = requirement_id("vector-lane-bounds", &subject);
                                obligations.push(ObligationRecord {
                                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                                    identity: body_obligation_id("vector-lane-bounds", &subject),
                                    subject: subject.clone(),
                                    requirement,
                                    status: ObligationStatus::Unknown,
                                    method: "runtime-checked-vector-lane".to_owned(),
                                    assumptions: Vec::new(),
                                    dependencies: vec![subject],
                                    freshness: EvidenceFreshness::Unknown,
                                    fallback: Some("the lane operation is available only on the successful bounds-check path".to_owned()),
                                });
                            }
                            // A runtime-checked projection retains an explicit
                            // bounds obligation; statically established or
                            // traversal-domain evidence discharges it by
                            // semantics instead.
                            BodyOperationKind::SequenceProject {
                                evidence:
                                    crate::BoundsEvidence::RuntimeChecked { failure: _ },
                                ..
                            } => {
                                let requirement =
                                    requirement_id("sequence-index-bounds", &subject);
                                obligations.push(ObligationRecord {
                                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                                    identity: body_obligation_id(
                                        "sequence-index-bounds",
                                        &subject,
                                    ),
                                    subject: subject.clone(),
                                    requirement,
                                    status: ObligationStatus::Unknown,
                                    method: "runtime-checked-sequence-index".to_owned(),
                                    assumptions: Vec::new(),
                                    dependencies: vec![subject],
                                    freshness: EvidenceFreshness::Unknown,
                                    fallback: Some(
                                        "the element is available only on the successful bounds-check path"
                                            .to_owned(),
                                    ),
                                });
                            }
                            BodyOperationKind::SequenceProject { .. } => {}
                            // View range validity is checked at realization;
                            // the obligation stays UNKNOWN until discharged.
                            BodyOperationKind::ViewConstruct { .. } => {
                                let requirement = requirement_id("view-range-valid", &subject);
                                obligations.push(ObligationRecord {
                                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                                    identity: body_obligation_id("view-range-valid", &subject),
                                    subject: subject.clone(),
                                    requirement,
                                    status: ObligationStatus::Unknown,
                                    method: "checked-view-range-construction".to_owned(),
                                    assumptions: Vec::new(),
                                    dependencies: vec![subject],
                                    freshness: EvidenceFreshness::Unknown,
                                    fallback: Some(
                                        "the view value exists only on the successful range-check path"
                                            .to_owned(),
                                    ),
                                });
                            }
                            BodyOperationKind::Float { .. }
                            | BodyOperationKind::FloatCompare { .. }
                            | BodyOperationKind::FloatIntrinsic { .. } => {
                                // Binary64 operators and comparisons trap on
                                // non-finite inputs (and operators on
                                // non-finite results); the trap is the
                                // conservative fallback, exactly like checked
                                // division's zero guard, so the obligation
                                // stays unknown until finiteness is proven
                                // upstream.
                                let requirement = requirement_id("float-finite", &subject);
                                obligations.push(ObligationRecord {
                                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                                    identity: body_obligation_id("float-finite", &subject),
                                    subject: subject.clone(),
                                    requirement,
                                    status: ObligationStatus::Unknown,
                                    method: "symbolic-float-finite".to_owned(),
                                    assumptions: function
                                        .assumptions
                                        .iter()
                                        .map(|assumption| {
                                            assumption_id(namespace, assumption)
                                        })
                                        .collect(),
                                    dependencies: vec![function_identity.clone(), subject],
                                    freshness: EvidenceFreshness::Unknown,
                                    fallback: Some(
                                        "retain the non-finite trap or prove finiteness upstream"
                                            .to_owned(),
                                    ),
                                });
                            }
                            // Checked division/modulo trap on exactly two
                            // singular inputs — a zero divisor, and (signed
                            // division only) `MIN / -1`. Both facts are
                            // represented explicitly with canonical
                            // digest-bound identities instead of one opaque
                            // `integer-overflow` hash, and literal divisors
                            // discharge statically by language semantics
                            // (ENG-PRESSURE-0008). The runtime guards are
                            // retained regardless: discharge records the
                            // static fact, it never removes the check.
                            BodyOperationKind::Integer {
                                operator,
                                operand_type,
                                ..
                            } if operator == "div" || operator == "mod" => {
                                push_division_obligations(
                                    &mut obligations,
                                    function,
                                    &function_identity,
                                    namespace,
                                    &subject,
                                    operation,
                                    operand_type,
                                    &body_constants,
                                );
                            }
                            BodyOperationKind::Integer { intent, .. }
                            | BodyOperationKind::VectorBinary { intent, .. }
                            | BodyOperationKind::VectorReduce { intent, .. } => {
                                let bounded_counter_step =
                                    body.bounded_iterations.iter().any(|iteration| {
                                        iteration.backedge == block.id
                                            && is_iteration_decrement(&operation.id)
                                    });
                                let total_by_semantics = matches!(
                                    intent,
                                    crate::ArithmeticIntent::Wrapping
                                        | crate::ArithmeticIntent::Saturating
                                        | crate::ArithmeticIntent::Widening { .. }
                                );
                                let discharged = bounded_counter_step || total_by_semantics;
                                let requirement = requirement_id("integer-overflow", &subject);
                                obligations.push(ObligationRecord {
                                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                                    identity: body_obligation_id("integer-overflow", &subject),
                                    subject: subject.clone(),
                                    requirement,
                                    status: if discharged {
                                        ObligationStatus::Pass
                                    } else {
                                        ObligationStatus::Unknown
                                    },
                                    method: if bounded_counter_step {
                                        "language-bounded-iteration-counter-decrement".to_owned()
                                    } else if total_by_semantics {
                                        format!("language-explicit-{intent:?}-semantics")
                                    } else {
                                        format!("symbolic-{intent:?}")
                                    },
                                    assumptions: function
                                        .assumptions
                                        .iter()
                                        .map(|assumption| {
                                            assumption_id(namespace, assumption)
                                        })
                                        .collect(),
                                    dependencies: vec![function_identity.clone(), subject],
                                    freshness: if discharged {
                                        EvidenceFreshness::Current
                                    } else {
                                        EvidenceFreshness::Unknown
                                    },
                                    fallback: (!discharged).then(|| {
                                        "retain explicit arithmetic behavior or insert a runtime check"
                                            .to_owned()
                                    }),
                                });
                            }
                            BodyOperationKind::Effect { effect, .. } => {
                                let effect_identity = effect_id(
                                    namespace,
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
                            BodyOperationKind::HostCall {
                                capability,
                                operation,
                            } => {
                                // A host call authorizes exactly like a
                                // declared effect: the obligation passes
                                // when the capability is declared, and the
                                // executor discharges the grant at run time.
                                let granted = Effect {
                                    kind: crate::host_call_effect_kind(operation).to_owned(),
                                    target: String::new(),
                                    capability: capability.clone(),
                                };
                                let effect_identity = effect_id(
                                    namespace,
                                    &function.name,
                                    &serde_json::to_string(&crate::canonical::canonical_effect(
                                        &granted,
                                    ))
                                    .expect("host call effect"),
                                    function
                                        .effects
                                        .iter()
                                        .position(|declared| {
                                            declared.kind == granted.kind
                                                && declared.capability == *capability
                                        })
                                        .unwrap_or(0),
                                );
                                obligations.push(ObligationRecord {
                                    schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
                                    identity: body_obligation_id("effect-authorized", &subject),
                                    subject: subject.clone(),
                                    requirement: requirement_id("effect-authorized", &subject),
                                    status: if function.capabilities.contains(capability) {
                                        ObligationStatus::Pass
                                    } else {
                                        ObligationStatus::Fail
                                    },
                                    method: "host-call-closure".to_owned(),
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
                                        capability_id(namespace, &function.name, capability)
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
                                        namespace,
                                        &function.name,
                                        &block.id,
                                    ),
                                    requirement: requirement.identity.clone(),
                                    status: ObligationStatus::Unknown,
                                    method: "body-machine-intent".to_owned(),
                                    assumptions: Vec::new(),
                                    dependencies: vec![
                                        operation.identity(namespace, &function.name, &block.id),
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

/// Compiler-known integer literals by result value identity, covering one
/// function body. Used only to route literal-operand facts (notably
/// nonzero literal divisors) to static discharge; it decides no proof.
fn collect_body_constants(
    body: &crate::FunctionBody,
) -> BTreeMap<String, (i128, crate::IntegerType)> {
    let mut constants = BTreeMap::new();
    for block in &body.blocks {
        for operation in &block.operations {
            let BodyOperationKind::Constant { value, ty } = &operation.kind else {
                continue;
            };
            let crate::BodyType::Integer(int_ty) = ty else {
                continue;
            };
            for result in &operation.results {
                constants.insert(result.id.clone(), (*value, *int_ty));
            }
        }
    }
    constants
}

/// Push the explicit checked-division obligations for one scalar `div`/`mod`
/// operation (ENG-PRESSURE-0008):
///
/// - `divisor-nonzero`: the divisor is never zero. Discharges statically
///   exactly when the divisor operand is a nonzero integer literal.
/// - `signed-division-overflow`: signed `MIN / -1` (and the `mod`
///   counterpart, conservatively) traps. Discharges statically exactly
///   when the divisor is a nonzero literal other than `-1`, or the
///   dividend is a literal other than the signed minimum.
///
/// Both obligations carry canonical digest-bound identities through the
/// existing constructors, record the same function assumptions as every
/// other body obligation, and keep a runtime-guard fallback while
/// UNKNOWN. Discharge marks a language-semantic fact (a literal value
/// cannot be zero) the way wrapping-intent totality already does; it
/// never removes a runtime check and never admits a proof — proof claims
/// still belong to the MNCS proof-kernel path.
#[allow(clippy::too_many_arguments)]
fn push_division_obligations(
    obligations: &mut Vec<ObligationRecord>,
    function: &crate::Function,
    function_identity: &SemanticId,
    namespace: &str,
    subject: &SemanticId,
    operation: &crate::BodyOperation,
    operand_type: &crate::IntegerType,
    body_constants: &BTreeMap<String, (i128, crate::IntegerType)>,
) {
    let assumptions: Vec<SemanticId> = function
        .assumptions
        .iter()
        .map(|assumption| assumption_id(namespace, assumption))
        .collect();
    let divisor = operation
        .operands
        .get(1)
        .and_then(|operand| body_constants.get(operand))
        .copied();
    let dividend = operation
        .operands
        .first()
        .and_then(|operand| body_constants.get(operand))
        .copied();
    let divisor_nonzero = divisor.is_some_and(|(value, _)| value != 0);
    push_single_obligation(
        obligations,
        function_identity,
        &assumptions,
        subject,
        "divisor-nonzero",
        divisor_nonzero,
        "language-literal-divisor-nonzero",
        "runtime-checked-division",
        Some("retain the zero-divisor runtime guard"),
    );
    if operand_type.signed {
        // Signed minimum for this width; only `MIN / -1` (and, by the same
        // conservative rule, `MIN % -1`) can overflow.
        let minimum = -(1_i128 << (operand_type.bits.saturating_sub(1)));
        let bounds_clear = divisor_nonzero
            && divisor.is_some_and(|(value, _)| {
                value != -1 || dividend.is_some_and(|(dividend, _)| dividend != minimum)
            });
        push_single_obligation(
            obligations,
            function_identity,
            &assumptions,
            subject,
            "signed-division-overflow",
            bounds_clear,
            "language-literal-division-bounds",
            "runtime-checked-signed-division",
            Some("retain the signed-division-overflow runtime guard"),
        );
    }
}

/// Push one body obligation with the pass/unknown method, freshness, and
/// fallback selected by its statically established state.
#[allow(clippy::too_many_arguments)]
fn push_single_obligation(
    obligations: &mut Vec<ObligationRecord>,
    function_identity: &SemanticId,
    assumptions: &[SemanticId],
    subject: &SemanticId,
    kind: &str,
    discharged: bool,
    pass_method: &str,
    unknown_method: &str,
    unknown_fallback: Option<&str>,
) {
    obligations.push(ObligationRecord {
        schema_version: OBLIGATION_SCHEMA_VERSION.to_owned(),
        identity: body_obligation_id(kind, subject),
        subject: subject.clone(),
        requirement: requirement_id(kind, subject),
        status: if discharged {
            ObligationStatus::Pass
        } else {
            ObligationStatus::Unknown
        },
        method: if discharged {
            pass_method.to_owned()
        } else {
            unknown_method.to_owned()
        },
        assumptions: assumptions.to_vec(),
        dependencies: vec![function_identity.clone(), subject.clone()],
        freshness: if discharged {
            EvidenceFreshness::Current
        } else {
            EvidenceFreshness::Unknown
        },
        fallback: (!discharged).then(|| {
            unknown_fallback
                .unwrap_or("retain the runtime guard")
                .to_owned()
        }),
    });
}

#[cfg(test)]
mod tests {
    use crate::validation::tests::valid_program;
    use crate::{
        ArithmeticIntent, BodyBlock, BodyOperation, BodyOperationKind, BodyTerminator, BodyType,
        BodyValue, EvidenceFreshness, Function, FunctionBody, IntegerOperation, IntegerType,
        Intent, MachineIntentExpression, ObligationStatus, Program, Requirement, SemanticId,
        SUPPORTED_SCHEMA_VERSION,
    };

    fn div_program(divisor: Option<i128>, dividend: Option<i128>, signed: bool) -> Program {
        let int_ty = IntegerType { bits: 64, signed };
        let mut operations = Vec::new();
        let mut next = 0_usize;
        let mut constant = |value: i128| {
            let id = format!("c{next}");
            next += 1;
            operations.push(BodyOperation {
                id: id.clone(),
                kind: BodyOperationKind::Constant {
                    value,
                    ty: BodyType::Integer(int_ty),
                },
                operands: Vec::new(),
                results: vec![BodyValue {
                    id: id.clone(),
                    ty: BodyType::Integer(int_ty),
                }],
                contracts: Vec::new(),
                assumptions: Vec::new(),
                machine_intent: None,
                lowering: None,
                portability: None,
            });
            id
        };
        let dividend_id = match dividend {
            Some(value) => constant(value),
            None => "param_x".to_owned(),
        };
        let divisor_id = match divisor {
            Some(value) => constant(value),
            None => "param_y".to_owned(),
        };
        operations.push(BodyOperation {
            id: "div0".to_owned(),
            kind: BodyOperationKind::Integer {
                operator: "div".to_owned(),
                operand_type: int_ty,
                intent: ArithmeticIntent::Checked,
            },
            operands: vec![dividend_id, divisor_id],
            results: vec![BodyValue {
                id: "div0".to_owned(),
                ty: BodyType::Integer(int_ty),
            }],
            contracts: Vec::new(),
            assumptions: Vec::new(),
            machine_intent: None,
            lowering: None,
            portability: None,
        });
        Program {
            schema_version: SUPPORTED_SCHEMA_VERSION.to_owned(),
            module: "test.division".to_owned(),
            dependencies: Vec::new(),
            finite_types: Vec::new(),
            record_types: Vec::new(),
            assumptions: Vec::new(),
            binding_table: None,
            functions: vec![Function {
                name: "probe".to_owned(),
                home_module: None,
                generic_params: Vec::new(),
                inputs: Vec::new(),
                outputs: Vec::new(),
                contracts: Vec::new(),
                effects: Vec::new(),
                capabilities: Vec::new(),
                assumptions: Vec::new(),
                evidence: Vec::new(),
                failure: crate::FailureMode::Isolated,
                body: Some(FunctionBody {
                    schema_version: crate::body::EXECUTABLE_BODY_SCHEMA_VERSION.to_owned(),
                    entry: "entry".to_owned(),
                    parameters: Vec::new(),
                    generic_params: Vec::new(),
                    cycle_policy: crate::BodyCyclePolicy::Legacy,
                    bounded_iterations: Vec::new(),
                    blocks: vec![BodyBlock {
                        id: "entry".to_owned(),
                        parameters: Vec::new(),
                        operations,
                        terminator: BodyTerminator::Return {
                            values: vec!["div0".to_owned()],
                        },
                    }],
                }),
            }],
            generic_specializations: Vec::new(),
        }
    }

    fn obligation_statuses(program: &Program) -> Vec<(String, ObligationStatus, String)> {
        program
            .generate_obligations()
            .obligations
            .iter()
            .map(|obligation| {
                let kind = obligation.identity.0.clone();
                (kind, obligation.status, obligation.method.clone())
            })
            .collect()
    }

    /// Literal nonzero divisors discharge both division facts statically
    /// with canonical digest-bound identities (ENG-PRESSURE-0008).
    #[test]
    fn literal_nonzero_divisor_discharges_both_division_facts() {
        let entries = obligation_statuses(&div_program(Some(255), Some(255), true));
        assert_eq!(entries.len(), 2, "{entries:?}");
        for (identity, status, _) in &entries {
            assert!(
                identity.starts_with("mncs:0.2:obligation:body:divisor-nonzero:")
                    || identity.starts_with("mncs:0.2:obligation:body:signed-division-overflow:"),
                "explicit division kinds, got {identity}"
            );
            assert!(
                !identity.contains("integer-overflow"),
                "no opaque hash: {identity}"
            );
            assert_eq!(*status, ObligationStatus::Pass, "{identity}");
        }
        assert!(entries
            .iter()
            .any(|(_, _, method)| method == "language-literal-divisor-nonzero"));
        assert!(entries
            .iter()
            .any(|(_, _, method)| method == "language-literal-division-bounds"));
    }

    /// Unknown divisors refuse: both facts stay UNKNOWN with runtime-guard
    /// fallbacks (ENG-PRESSURE-0008 refusal case).
    #[test]
    fn unknown_divisor_keeps_both_division_facts_open() {
        let entries = obligation_statuses(&div_program(None, None, true));
        assert_eq!(entries.len(), 2, "{entries:?}");
        for (identity, status, _) in &entries {
            assert_eq!(*status, ObligationStatus::Unknown, "{identity}");
        }
        let generation = div_program(None, None, true).generate_obligations();
        for obligation in &generation.obligations {
            assert!(obligation.fallback.is_some(), "{:?}", obligation.identity);
            assert_eq!(
                obligation.freshness,
                crate::EvidenceFreshness::Unknown,
                "{:?}",
                obligation.identity
            );
        }
    }

    /// A literal zero divisor never discharges, and `x / -1` discharges
    /// only divisor-nonzero: the `MIN / -1` overflow stays open.
    #[test]
    fn zero_and_neg_one_divisors_refuse_precisely() {
        let zero = obligation_statuses(&div_program(Some(0), Some(7), true));
        assert!(
            zero.iter()
                .all(|(_, status, _)| *status == ObligationStatus::Unknown),
            "{zero:?}"
        );
        let neg_one = obligation_statuses(&div_program(Some(-1), None, true));
        let nonzero = neg_one
            .iter()
            .find(|(identity, _, _)| identity.contains("divisor-nonzero"))
            .expect("divisor-nonzero present");
        assert_eq!(nonzero.1, ObligationStatus::Pass);
        let overflow = neg_one
            .iter()
            .find(|(identity, _, _)| identity.contains("signed-division-overflow"))
            .expect("overflow present");
        assert_eq!(overflow.1, ObligationStatus::Unknown);
        // A literal dividend away from MIN closes the edge: `10 / -1`.
        let lit = obligation_statuses(&div_program(Some(-1), Some(10), true));
        assert!(
            lit.iter()
                .all(|(_, status, _)| *status == ObligationStatus::Pass),
            "{lit:?}"
        );
    }

    /// Unsigned division carries no signed-overflow obligation at all.
    #[test]
    fn unsigned_division_has_no_signed_overflow_obligation() {
        let entries = obligation_statuses(&div_program(None, None, false));
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert!(entries[0].0.contains("divisor-nonzero"), "{entries:?}");
        let lit = obligation_statuses(&div_program(Some(256), Some(10), false));
        assert_eq!(lit.len(), 1, "{lit:?}");
        assert_eq!(lit[0].1, ObligationStatus::Pass);
    }

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
