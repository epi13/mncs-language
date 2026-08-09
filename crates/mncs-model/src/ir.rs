use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::canonical::sha256_hex;
use crate::identity::{
    assumption_id, capability_id, contract_id, effect_id, function_id, program_id,
};
use crate::{
    AlignmentCapability, ContractKind, FailureMode, Function, GraphError, HighLevelIrNode,
    MachineIntentExpression, ObligationRecord, Program, SemanticId, ValidationReport,
};

pub const HIGH_LEVEL_IR_SCHEMA_VERSION: &str = "0.3";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IrType {
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrValue {
    pub identity: SemanticId,
    pub semantic_identity: Option<SemanticId>,
    pub ty: IrType,
    pub producer: Option<SemanticId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateRegionKind {
    Local,
    External,
    Storage,
    CapabilityControlled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrStateRegion {
    pub identity: SemanticId,
    pub semantic_identity: Option<SemanticId>,
    pub name: String,
    pub kind: StateRegionKind,
    pub capability: Option<SemanticId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathKind {
    Normal,
    Error,
    Compensation,
    Fatal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailurePathKind {
    Error,
    Compensation,
    Fatal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityUse {
    pub capability: SemanticId,
    pub semantic_declaration: Option<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IrOperationKind {
    FunctionEntry,
    ContractCheck {
        kind: ContractKind,
    },
    Effect,
    MachineIntent,
    Return,
    Failure {
        path: FailurePathKind,
        mode: FailureMode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrOperation {
    pub identity: SemanticId,
    pub semantic_identity: Option<SemanticId>,
    pub kind: IrOperationKind,
    pub inputs: Vec<IrValue>,
    pub outputs: Vec<IrValue>,
    pub state_region: Option<SemanticId>,
    pub effects: Vec<SemanticId>,
    pub capability_uses: Vec<CapabilityUse>,
    pub contracts: Vec<SemanticId>,
    pub assumptions: Vec<SemanticId>,
    pub evidence: Vec<SemanticId>,
    pub machine_intent: Option<MachineIntentLinks>,
    pub obligations: Vec<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrBlock {
    pub identity: SemanticId,
    pub path: PathKind,
    pub operations: Vec<IrOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrTransition {
    pub identity: SemanticId,
    pub from: SemanticId,
    pub to: SemanticId,
    pub path: PathKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrFunction {
    pub identity: SemanticId,
    pub semantic_identity: SemanticId,
    pub inputs: Vec<IrValue>,
    pub outputs: Vec<IrValue>,
    pub blocks: Vec<IrBlock>,
    pub transitions: Vec<IrTransition>,
    pub contracts: Vec<SemanticId>,
    pub assumptions: Vec<SemanticId>,
    pub capabilities: Vec<SemanticId>,
    pub failure: FailureMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceEntry {
    pub semantic_identity: SemanticId,
    pub graph_node: Option<SemanticId>,
    pub ir_identities: Vec<SemanticId>,
    pub related: Vec<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceMap {
    pub entries: Vec<TraceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineIntentLinks {
    pub operation_identity: SemanticId,
    pub intent_identity: SemanticId,
    pub fact_identities: Vec<SemanticId>,
    pub preference_identities: Vec<SemanticId>,
    pub requirement_identities: Vec<SemanticId>,
    pub obligation_identities: Vec<SemanticId>,
    pub capability_identities: Vec<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighLevelIr {
    pub schema_version: String,
    pub semantic_identity: SemanticId,
    pub semantic_fingerprint: String,
    pub functions: Vec<IrFunction>,
    pub state_regions: Vec<IrStateRegion>,
    pub obligations: Vec<ObligationRecord>,
    pub trace: TraceMap,
}

#[derive(Debug, Error)]
pub enum IrError {
    #[error("cannot lower an invalid program to high-level IR")]
    InvalidProgram(ValidationReport),
    #[error("semantic graph construction failed during IR lowering: {0}")]
    Graph(#[from] GraphError),
}

impl HighLevelIr {
    pub fn canonical_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn fingerprint(&self) -> Result<String, serde_json::Error> {
        Ok(sha256_hex(self.canonical_json()?.as_bytes()))
    }

    pub fn trace_for(&self, semantic_identity: &SemanticId) -> Option<&TraceEntry> {
        self.trace
            .entries
            .iter()
            .find(|entry| &entry.semantic_identity == semantic_identity)
    }
}

impl Program {
    pub fn lower_to_ir(&self) -> Result<HighLevelIr, IrError> {
        let report = self.validate();
        if !report.valid {
            return Err(IrError::InvalidProgram(report));
        }
        let graph = self.semantic_graph()?;
        let evidence = self.evidence_manifest()?;
        let obligations = self.generate_obligations().obligations;
        let mut functions = Vec::new();
        let mut state_regions = Vec::new();
        let mut trace = BTreeMap::<SemanticId, TraceEntry>::new();

        let program_identity = program_id(&self.module);
        trace_entry(
            &mut trace,
            &program_identity,
            &graph,
            ir_identity("module", &program_identity, 0),
            Vec::new(),
        );

        for function in &self.functions {
            let semantic_function = function_id(&self.module, &function.name);
            let ir_function = ir_identity("function", &semantic_function, 0);
            trace_entry(
                &mut trace,
                &semantic_function,
                &graph,
                ir_function.clone(),
                Vec::new(),
            );
            let inputs = function
                .inputs
                .iter()
                .enumerate()
                .map(|(index, value)| IrValue {
                    identity: ir_value_identity(&semantic_function, "input", index, &value.name),
                    semantic_identity: None,
                    ty: IrType::Named(value.value_type.clone()),
                    producer: None,
                })
                .collect::<Vec<_>>();
            let outputs = function
                .outputs
                .iter()
                .enumerate()
                .map(|(index, value)| IrValue {
                    identity: ir_value_identity(&semantic_function, "output", index, &value.name),
                    semantic_identity: None,
                    ty: IrType::Named(value.value_type.clone()),
                    producer: None,
                })
                .collect::<Vec<_>>();
            let contracts = function
                .contracts
                .iter()
                .map(|contract| {
                    let identity = contract_id(&self.module, &function.name, &contract.id);
                    trace_entry(
                        &mut trace,
                        &identity,
                        &graph,
                        ir_identity("contract", &identity, 0),
                        Vec::new(),
                    );
                    identity
                })
                .collect::<Vec<_>>();
            let assumptions = function
                .assumptions
                .iter()
                .map(|assumption| assumption_id(&self.module, assumption))
                .collect::<Vec<_>>();
            let capabilities = function
                .capabilities
                .iter()
                .map(|capability| capability_id(&self.module, &function.name, capability))
                .collect::<Vec<_>>();
            for assumption in &assumptions {
                trace_entry(
                    &mut trace,
                    assumption,
                    &graph,
                    ir_identity("assumption", assumption, 0),
                    Vec::new(),
                );
            }
            for capability in &capabilities {
                trace_entry(
                    &mut trace,
                    capability,
                    &graph,
                    ir_identity("capability", capability, 0),
                    Vec::new(),
                );
            }

            let mut normal_operations = Vec::new();
            let entry_identity = ir_identity("operation", &semantic_function, 0);
            normal_operations.push(IrOperation {
                identity: entry_identity,
                semantic_identity: Some(semantic_function.clone()),
                kind: IrOperationKind::FunctionEntry,
                inputs: inputs.clone(),
                outputs: Vec::new(),
                state_region: None,
                effects: Vec::new(),
                capability_uses: Vec::new(),
                contracts: contracts.clone(),
                assumptions: assumptions.clone(),
                evidence: Vec::new(),
                machine_intent: None,
                obligations: Vec::new(),
            });
            for (index, contract) in function.contracts.iter().enumerate() {
                let contract_identity = contract_id(&self.module, &function.name, &contract.id);
                let operation_identity = ir_identity("contract-check", &contract_identity, index);
                let evidence_refs = evidence
                    .evidence
                    .iter()
                    .filter(|record| record.property == contract_identity)
                    .map(|record| record.identity.clone())
                    .collect::<Vec<_>>();
                for evidence_identity in &evidence_refs {
                    trace_entry(
                        &mut trace,
                        evidence_identity,
                        &graph,
                        operation_identity.clone(),
                        vec![contract_identity.clone()],
                    );
                }
                normal_operations.push(IrOperation {
                    identity: operation_identity.clone(),
                    semantic_identity: Some(contract_identity.clone()),
                    kind: IrOperationKind::ContractCheck {
                        kind: contract.kind.clone(),
                    },
                    inputs: inputs.clone(),
                    outputs: Vec::new(),
                    state_region: None,
                    effects: Vec::new(),
                    capability_uses: Vec::new(),
                    contracts: vec![contract_identity.clone()],
                    assumptions: assumptions.clone(),
                    evidence: evidence_refs,
                    machine_intent: None,
                    obligations: Vec::new(),
                });
                trace_entry(
                    &mut trace,
                    &contract_identity,
                    &graph,
                    operation_identity,
                    Vec::new(),
                );
            }
            let mut effect_occurrences = BTreeMap::<String, usize>::new();
            for effect in &function.effects {
                let canonical = serde_json::to_string(&crate::canonical::canonical_effect(effect))
                    .expect("canonical effect");
                let occurrence = effect_occurrences.entry(canonical.clone()).or_default();
                let effect_identity =
                    effect_id(&self.module, &function.name, &canonical, *occurrence);
                let capability_identity =
                    capability_id(&self.module, &function.name, &effect.capability);
                let state_identity = ir_identity("state", &effect_identity, 0);
                state_regions.push(IrStateRegion {
                    identity: state_identity.clone(),
                    semantic_identity: Some(effect_identity.clone()),
                    name: effect.target.clone(),
                    kind: state_region_kind(&effect.kind),
                    capability: Some(capability_identity.clone()),
                });
                let operation_identity = ir_identity("effect", &effect_identity, *occurrence);
                normal_operations.push(IrOperation {
                    identity: operation_identity.clone(),
                    semantic_identity: Some(effect_identity.clone()),
                    kind: IrOperationKind::Effect,
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                    state_region: Some(state_identity),
                    effects: vec![effect_identity.clone()],
                    capability_uses: vec![CapabilityUse {
                        capability: capability_identity.clone(),
                        semantic_declaration: Some(capability_identity),
                    }],
                    contracts: contracts.clone(),
                    assumptions: assumptions.clone(),
                    evidence: Vec::new(),
                    machine_intent: None,
                    obligations: vec![effect_obligation_identity(&effect_identity)],
                });
                trace_entry(
                    &mut trace,
                    &effect_identity,
                    &graph,
                    operation_identity,
                    vec![effect_obligation_identity(&effect_identity)],
                );
                *occurrence += 1;
            }
            let return_identity = ir_identity("return", &semantic_function, 0);
            normal_operations.push(IrOperation {
                identity: return_identity,
                semantic_identity: Some(semantic_function.clone()),
                kind: IrOperationKind::Return,
                inputs: outputs.clone(),
                outputs: Vec::new(),
                state_region: None,
                effects: Vec::new(),
                capability_uses: Vec::new(),
                contracts: contracts.clone(),
                assumptions: assumptions.clone(),
                evidence: Vec::new(),
                machine_intent: None,
                obligations: Vec::new(),
            });
            let normal_block = IrBlock {
                identity: ir_identity("block", &semantic_function, 0),
                path: PathKind::Normal,
                operations: normal_operations,
            };
            let error_block = failure_block(function, &semantic_function, PathKind::Error);
            let mut blocks = vec![normal_block.clone(), error_block.clone()];
            let mut transitions = vec![IrTransition {
                identity: ir_identity("transition", &semantic_function, 0),
                from: normal_block.identity.clone(),
                to: error_block.identity.clone(),
                path: PathKind::Error,
            }];
            if function.failure == FailureMode::Compensating {
                let compensation =
                    failure_block(function, &semantic_function, PathKind::Compensation);
                transitions.push(IrTransition {
                    identity: ir_identity("transition", &semantic_function, 1),
                    from: error_block.identity.clone(),
                    to: compensation.identity.clone(),
                    path: PathKind::Compensation,
                });
                blocks.push(compensation);
            }
            if function.failure == FailureMode::Fatal {
                let fatal = failure_block(function, &semantic_function, PathKind::Fatal);
                transitions.push(IrTransition {
                    identity: ir_identity("transition", &semantic_function, 2),
                    from: error_block.identity.clone(),
                    to: fatal.identity.clone(),
                    path: PathKind::Fatal,
                });
                blocks.push(fatal);
            }
            functions.push(IrFunction {
                identity: ir_function,
                semantic_identity: semantic_function,
                inputs,
                outputs,
                blocks,
                transitions,
                contracts,
                assumptions,
                capabilities,
                failure: function.failure.clone(),
            });
        }
        state_regions.sort_by(|left, right| left.identity.cmp(&right.identity));
        let trace = TraceMap {
            entries: trace
                .into_values()
                .map(|mut entry| {
                    entry.ir_identities.sort();
                    entry.related.sort();
                    entry
                })
                .collect(),
        };
        Ok(HighLevelIr {
            schema_version: HIGH_LEVEL_IR_SCHEMA_VERSION.to_owned(),
            semantic_identity: program_identity,
            semantic_fingerprint: self
                .content_fingerprint()
                .expect("validated canonical form"),
            functions,
            state_regions,
            obligations,
            trace,
        })
    }
}

impl MachineIntentExpression {
    pub fn lower_to_ir_operation(&self) -> IrOperation {
        let node: HighLevelIrNode = self.lower_to_high_level_ir();
        let links = MachineIntentLinks {
            operation_identity: node.operation_identity.clone(),
            intent_identity: node.intent_identity,
            fact_identities: node.fact_identities,
            preference_identities: self
                .preferences
                .iter()
                .map(|preference| preference.identity.clone())
                .collect(),
            requirement_identities: node.requirement_identities,
            obligation_identities: node.obligation_identities,
            capability_identities: Vec::new(),
        };
        IrOperation {
            identity: ir_identity("machine-intent", &links.operation_identity, 0),
            semantic_identity: Some(links.operation_identity.clone()),
            kind: IrOperationKind::MachineIntent,
            inputs: Vec::new(),
            outputs: Vec::new(),
            state_region: None,
            effects: Vec::new(),
            capability_uses: Vec::new(),
            contracts: Vec::new(),
            assumptions: Vec::new(),
            evidence: Vec::new(),
            machine_intent: Some(links.clone()),
            obligations: links.obligation_identities,
        }
    }
}

impl AlignmentCapability {
    pub fn to_ir_capability_use(&self) -> CapabilityUse {
        CapabilityUse {
            capability: self.identity.clone(),
            semantic_declaration: Some(self.subject.clone()),
        }
    }
}

fn trace_entry(
    trace: &mut BTreeMap<SemanticId, TraceEntry>,
    semantic_identity: &SemanticId,
    graph: &crate::SemanticGraph,
    ir_identity: SemanticId,
    related: Vec<SemanticId>,
) {
    let graph_node = graph
        .nodes
        .iter()
        .find(|node| node.identity == *semantic_identity)
        .map(|node| node.identity.clone());
    let entry = trace
        .entry(semantic_identity.clone())
        .or_insert_with(|| TraceEntry {
            semantic_identity: semantic_identity.clone(),
            graph_node,
            ir_identities: Vec::new(),
            related: Vec::new(),
        });
    entry.ir_identities.push(ir_identity);
    entry.related.extend(related);
}

fn failure_block(function: &Function, semantic_function: &SemanticId, path: PathKind) -> IrBlock {
    let path_kind = match path {
        PathKind::Error => FailurePathKind::Error,
        PathKind::Compensation => FailurePathKind::Compensation,
        PathKind::Fatal => FailurePathKind::Fatal,
        PathKind::Normal => FailurePathKind::Error,
    };
    let identity = ir_identity("block", semantic_function, path as usize + 1);
    IrBlock {
        identity: identity.clone(),
        path,
        operations: vec![IrOperation {
            identity: ir_identity("failure", semantic_function, path as usize + 1),
            semantic_identity: Some(semantic_function.clone()),
            kind: IrOperationKind::Failure {
                path: path_kind,
                mode: function.failure.clone(),
            },
            inputs: Vec::new(),
            outputs: Vec::new(),
            state_region: None,
            effects: Vec::new(),
            capability_uses: Vec::new(),
            contracts: Vec::new(),
            assumptions: Vec::new(),
            evidence: Vec::new(),
            machine_intent: None,
            obligations: Vec::new(),
        }],
    }
}

fn state_region_kind(kind: &str) -> StateRegionKind {
    match kind.to_ascii_lowercase().as_str() {
        "write" | "append" | "storage" => StateRegionKind::Storage,
        "read" | "observe" => StateRegionKind::External,
        _ => StateRegionKind::CapabilityControlled,
    }
}

fn ir_identity(kind: &str, semantic_identity: &SemanticId, ordinal: usize) -> SemanticId {
    SemanticId(format!(
        "mncs:0.3:ir:{kind}:{}:{ordinal}",
        sha256_hex(semantic_identity.0.as_bytes())
    ))
}

fn ir_value_identity(function: &SemanticId, role: &str, ordinal: usize, name: &str) -> SemanticId {
    SemanticId(format!(
        "mncs:0.3:ir:value:{}:{role}:{ordinal}:{}",
        sha256_hex(function.0.as_bytes()),
        sha256_hex(name.as_bytes())
    ))
}

pub(crate) fn effect_obligation_identity(effect: &SemanticId) -> SemanticId {
    SemanticId(format!(
        "mncs:0.2:obligation:effect-authorized:{}",
        sha256_hex(effect.0.as_bytes())
    ))
}

#[cfg(test)]
mod tests {
    use crate::validation::tests::valid_program;
    use crate::{
        ArithmeticIntent, EvidenceFreshness, IntegerOperation, IntegerType, Intent,
        MachineIntentExpression, Obligation, ObligationStatus, Requirement, SemanticId,
    };

    #[test]
    fn semantic_program_lowers_to_deterministic_ir_with_failure_and_authority_paths() {
        let program = valid_program();
        let left = program.lower_to_ir().expect("IR");
        let right = program.lower_to_ir().expect("IR");
        assert_eq!(left, right);
        assert_eq!(left.fingerprint().unwrap(), right.fingerprint().unwrap());
        let function = &left.functions[0];
        assert_eq!(function.blocks.len(), 2);
        assert!(function.blocks[0].operations.iter().any(|operation| {
            operation
                .capability_uses
                .iter()
                .any(|usage| usage.semantic_declaration.is_some())
        }));
        assert!(left.trace_for(&function.semantic_identity).is_some());
    }

    #[test]
    fn machine_intent_lowering_keeps_requirements_and_unknown_obligations() {
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
                statement: "retain overflow behavior".to_owned(),
            },
            preferences: vec![],
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
                method: "not-yet-verified".to_owned(),
            }],
        };
        let ir = expression.lower_to_ir_operation();
        let links = ir.machine_intent.expect("links");
        assert_eq!(links.operation_identity, operation.identity());
        assert_eq!(
            links.obligation_identities,
            vec![SemanticId("obligation:no-overflow".to_owned())]
        );
        assert_eq!(ir.obligations, links.obligation_identities);
    }
}
