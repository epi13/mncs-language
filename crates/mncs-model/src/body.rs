use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::identity::{block_id, body_id, operation_id, value_id};
use crate::{
    ArithmeticIntent, Diagnostic, Effect, Fact, FailureMode, Function, IntegerType, Intent,
    MachinePreference, Obligation, Program, Requirement, SemanticId,
};

pub const EXECUTABLE_BODY_SCHEMA_VERSION: &str = "0.2";
pub const SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND: u32 = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionBody {
    pub schema_version: String,
    pub entry: String,
    pub parameters: Vec<BodyParameter>,
    #[serde(default, skip_serializing_if = "BodyCyclePolicy::is_legacy")]
    pub cycle_policy: BodyCyclePolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bounded_iterations: Vec<BodyBoundedIteration>,
    pub blocks: Vec<BodyBlock>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyCyclePolicy {
    #[default]
    Legacy,
    BoundedIterationOnly,
}

impl BodyCyclePolicy {
    pub(crate) fn is_legacy(&self) -> bool {
        *self == Self::Legacy
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyBoundedIteration {
    pub id: String,
    pub bound: u32,
    pub state_name: String,
    pub state_type: BodyType,
    pub initial_value: String,
    pub header_state: String,
    pub header_counter: String,
    pub preheader: String,
    pub header: String,
    pub body_entry: String,
    pub backedge: String,
    pub exit: String,
    pub exit_state: String,
    pub body_blocks: Vec<String>,
    pub callees: Vec<SemanticId>,
    pub required_capabilities: Vec<String>,
    pub completion_modes: Vec<BoundedIterationCompletion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundedIterationCompletion {
    EarlyReturn,
    Exhausted,
    Failure,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyParameter {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: BodyType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyValue {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: BodyType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyType {
    Named(String),
    Integer(IntegerType),
    Finite { identity: SemanticId, name: String },
    Record { identity: SemanticId, name: String },
}

impl BodyType {
    pub fn from_semantic_name(name: &str) -> Self {
        let (signed, digits) = if let Some(digits) = name.strip_prefix('i') {
            (true, digits)
        } else if let Some(digits) = name.strip_prefix('u') {
            (false, digits)
        } else {
            return Self::Named(name.to_owned());
        };
        match digits.parse::<u16>() {
            Ok(bits) if bits > 0 => Self::Integer(IntegerType { bits, signed }),
            _ => Self::Named(name.to_owned()),
        }
    }

    pub fn semantic_name(&self) -> String {
        match self {
            Self::Named(name) => name.clone(),
            Self::Integer(integer) => {
                format!("{}{}", if integer.signed { 'i' } else { 'u' }, integer.bits)
            }
            Self::Finite { name, .. } => name.clone(),
            Self::Record { name, .. } => name.clone(),
        }
    }

    fn is_boolean(&self) -> bool {
        matches!(self, Self::Named(name) if name == "bool")
            || matches!(self, Self::Integer(integer) if integer.bits == 1 && !integer.signed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyBlock {
    pub id: String,
    #[serde(default)]
    pub parameters: Vec<BodyValue>,
    pub operations: Vec<BodyOperation>,
    pub terminator: BodyTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyOperation {
    pub id: String,
    pub kind: BodyOperationKind,
    #[serde(default)]
    pub operands: Vec<String>,
    #[serde(default)]
    pub results: Vec<BodyValue>,
    #[serde(default)]
    pub contracts: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub machine_intent: Option<MachineIntentSpec>,
    #[serde(default)]
    pub lowering: Option<LoweringEnvelope>,
    #[serde(default)]
    pub portability: Option<PortabilityEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyOperationKind {
    Constant {
        value: i128,
        #[serde(rename = "type")]
        ty: BodyType,
    },
    Integer {
        operator: String,
        operand_type: IntegerType,
        intent: ArithmeticIntent,
    },
    IntegerCompare {
        predicate: String,
        operand_type: IntegerType,
    },
    /// Strict boolean conjunction/disjunction (Profile 0.6). Both operands
    /// are total values; evaluation is not short-circuited.
    BooleanOp {
        operator: String,
    },
    FiniteConstruct {
        type_identity: SemanticId,
        variant_identity: SemanticId,
        discriminant: u32,
        /// Canonical (sorted) payload field names for payload-bearing
        /// variants (Profile 0.6). `operands[i]` carries the value of
        /// `payload_fields[i]`. Empty (and serialized as absent) for
        /// payload-free variants.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        payload_fields: Vec<String>,
    },
    FiniteIsVariant {
        type_identity: SemanticId,
        variant_identity: SemanticId,
        discriminant: u32,
    },
    /// Project one payload field out of a finite value whose variant carries
    /// a payload (Profile 0.6). The result type is the declared field type;
    /// the operand must be a finite value of the declared type and variant.
    FinitePayloadProject {
        type_identity: SemanticId,
        variant_identity: SemanticId,
        discriminant: u32,
        field: String,
    },
    RecordConstruct {
        type_identity: SemanticId,
        /// Canonical (sorted) field names for the constructed value.
        field_names: Vec<String>,
    },
    RecordProject {
        type_identity: SemanticId,
        field: String,
    },
    Call {
        function: SemanticId,
        function_name: String,
        required_capabilities: Vec<String>,
        effects: Vec<Effect>,
    },
    Effect {
        effect: Effect,
        capability: String,
    },
    RuntimeCheck {
        obligation: SemanticId,
        fact: Fact,
        failure: FailureMode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyTerminator {
    Return {
        values: Vec<String>,
    },
    Branch {
        target: String,
        #[serde(default)]
        arguments: Vec<String>,
    },
    ConditionalBranch {
        condition: String,
        then_target: String,
        #[serde(default)]
        then_arguments: Vec<String>,
        else_target: String,
        #[serde(default)]
        else_arguments: Vec<String>,
    },
    Failure {
        mode: FailureMode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineIntentSpec {
    pub intent: Intent,
    #[serde(default)]
    pub preferences: Vec<MachinePreference>,
    #[serde(default)]
    pub facts: Vec<Fact>,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
    #[serde(default)]
    pub obligations: Vec<Obligation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealizationClass {
    PortableReference,
    RuntimeChecked,
    Specialized(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweringEnvelope {
    pub schema_version: String,
    #[serde(default)]
    pub permitted_realizations: Vec<RealizationClass>,
    #[serde(default)]
    pub prohibited_transformations: Vec<String>,
    #[serde(default)]
    pub required_obligations: Vec<SemanticId>,
    pub fallback: Option<RealizationClass>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortabilityEnvelope {
    pub schema_version: String,
    pub target: PortabilityTarget,
    #[serde(default)]
    pub required_features: Vec<String>,
    #[serde(default)]
    pub integer_widths: Vec<u16>,
    pub abi: Option<String>,
    pub fallback: Option<RealizationClass>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortabilityTarget {
    Portable,
    ArchitectureFamily(String),
}

impl FunctionBody {
    pub fn canonical_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.canonicalized())
    }

    pub fn fingerprint(&self) -> Result<String, serde_json::Error> {
        Ok(crate::canonical::sha256_hex(
            self.canonical_json()?.as_bytes(),
        ))
    }

    pub fn identity(&self, module: &str, function: &str) -> SemanticId {
        body_id(module, function)
    }

    pub fn block_identity(&self, module: &str, function: &str, block: &str) -> SemanticId {
        block_id(module, function, block)
    }

    pub fn validate(
        &self,
        program: &Program,
        function: &Function,
        function_path: &str,
        errors: &mut Vec<Diagnostic>,
    ) {
        if self.schema_version != EXECUTABLE_BODY_SCHEMA_VERSION {
            errors.push(body_diagnostic(
                "MNB001",
                format!("{function_path}.body.schema_version"),
                format!(
                    "unsupported executable body schema {:?}; expected {:?}",
                    self.schema_version, EXECUTABLE_BODY_SCHEMA_VERSION
                ),
            ));
        }
        let mut values = BTreeMap::<String, BodyType>::new();
        let mut parameter_ids = BTreeSet::new();
        if self.parameters.len() != function.inputs.len() {
            errors.push(body_diagnostic(
                "MNB002",
                format!("{function_path}.body.parameters"),
                "body parameters must match the enclosing function inputs",
            ));
        }
        for (index, parameter) in self.parameters.iter().enumerate() {
            let path = format!("{function_path}.body.parameters[{index}]");
            if parameter.id.trim().is_empty() || parameter.name.trim().is_empty() {
                errors.push(body_diagnostic(
                    "MNB003",
                    path.clone(),
                    "body parameter id and name must not be empty",
                ));
            }
            if !parameter_ids.insert(parameter.id.clone()) {
                errors.push(body_diagnostic(
                    "MNB004",
                    format!("{path}.id"),
                    format!("duplicate body value identity {:?}", parameter.id),
                ));
            }
            if let Some(input) = function.inputs.get(index) {
                if parameter.name != input.name
                    || parameter.ty != semantic_body_type(program, &input.value_type)
                {
                    errors.push(body_diagnostic(
                        "MNB005",
                        path,
                        "body parameter name or type does not match the function input",
                    ));
                }
            }
            values.insert(parameter.id.clone(), parameter.ty.clone());
        }

        let block_ids: BTreeSet<_> = self.blocks.iter().map(|block| block.id.clone()).collect();
        if self.entry.trim().is_empty() || !block_ids.contains(&self.entry) {
            errors.push(body_diagnostic(
                "MNB006",
                format!("{function_path}.body.entry"),
                format!("body entry block {:?} does not exist", self.entry),
            ));
        }
        if block_ids.len() != self.blocks.len() {
            errors.push(body_diagnostic(
                "MNB007",
                format!("{function_path}.body.blocks"),
                "body block identities must be unique",
            ));
        }
        let cfg = crate::Cfg::from_body(self, self.identity(&program.module, &function.name));
        for block in &cfg.unreachable {
            errors.push(body_diagnostic(
                "MNB038",
                format!("{function_path}.body.blocks.{block}"),
                "unreachable body blocks are not supported by this executable subset",
            ));
        }
        validate_bounded_iterations(self, function, function_path, &cfg, errors);

        let mut operation_ids = BTreeSet::new();
        let mut definitions = BTreeMap::<String, (Option<String>, BodyType)>::new();
        for parameter in &self.parameters {
            definitions.insert(parameter.id.clone(), (None, parameter.ty.clone()));
        }
        for block in &self.blocks {
            for parameter in &block.parameters {
                if definitions
                    .insert(
                        parameter.id.clone(),
                        (Some(block.id.clone()), parameter.ty.clone()),
                    )
                    .is_some()
                {
                    errors.push(body_diagnostic(
                        "MNB008",
                        format!("{function_path}.body.blocks.{}.parameters", block.id),
                        format!("duplicate body value identity {:?}", parameter.id),
                    ));
                }
            }
            for operation in &block.operations {
                for result in &operation.results {
                    if definitions
                        .insert(
                            result.id.clone(),
                            (Some(block.id.clone()), result.ty.clone()),
                        )
                        .is_some()
                    {
                        errors.push(body_diagnostic(
                            "MNB011",
                            format!("{function_path}.body.blocks.{}.operations", block.id),
                            format!("duplicate body value identity {:?}", result.id),
                        ));
                    }
                }
            }
        }
        let block_parameters = self
            .blocks
            .iter()
            .map(|block| (block.id.clone(), block.parameters.clone()))
            .collect::<BTreeMap<_, _>>();

        for (block_index, block) in self.blocks.iter().enumerate() {
            let block_path = format!("{function_path}.body.blocks[{block_index}]");
            let mut available = values.clone();
            for parameter in &block.parameters {
                available.insert(parameter.id.clone(), parameter.ty.clone());
            }
            for (value, (definition_block, ty)) in &definitions {
                if definition_block.as_deref().is_some_and(|definition| {
                    definition != block.id && cfg.dominates(definition, &block.id)
                }) {
                    available.insert(value.clone(), ty.clone());
                }
            }
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let operation_path = format!("{block_path}.operations[{operation_index}]");
                if operation.id.trim().is_empty() || !operation_ids.insert(operation.id.clone()) {
                    errors.push(body_diagnostic(
                        "MNB009",
                        format!("{operation_path}.id"),
                        format!(
                            "operation identity {:?} is empty or duplicated",
                            operation.id
                        ),
                    ));
                }
                for operand in &operation.operands {
                    if !available.contains_key(operand) {
                        errors.push(body_diagnostic(
                            "MNB010",
                            format!("{operation_path}.operands"),
                            format!("value {:?} is not defined before use", operand),
                        ));
                    }
                }
                validate_operation(
                    program,
                    function,
                    operation,
                    &operation_path,
                    &available,
                    errors,
                );
                for result in &operation.results {
                    available.insert(result.id.clone(), result.ty.clone());
                }
            }
            validate_terminator(
                &block.terminator,
                &block_path,
                &available,
                &block_ids,
                &block_parameters,
                errors,
            );
            if matches!(block.terminator, BodyTerminator::Return { .. }) {
                validate_return_types(
                    &block.terminator,
                    program,
                    function,
                    &available,
                    &block_path,
                    errors,
                );
            }
        }
    }

    fn canonicalized(&self) -> Self {
        let mut canonical = self.clone();
        canonical
            .bounded_iterations
            .sort_by(|left, right| left.id.cmp(&right.id));
        for iteration in &mut canonical.bounded_iterations {
            iteration.body_blocks.sort();
            iteration.body_blocks.dedup();
            iteration.callees.sort();
            iteration.callees.dedup();
            iteration.required_capabilities.sort();
            iteration.required_capabilities.dedup();
            iteration.completion_modes.sort();
            iteration.completion_modes.dedup();
        }
        for block in &mut canonical.blocks {
            for operation in &mut block.operations {
                operation.contracts.sort();
                operation.contracts.dedup();
                operation.assumptions.sort();
                operation.assumptions.dedup();
                if let Some(machine_intent) = &mut operation.machine_intent {
                    machine_intent
                        .preferences
                        .sort_by(|left, right| left.identity.cmp(&right.identity));
                    machine_intent
                        .facts
                        .sort_by(|left, right| left.identity.cmp(&right.identity));
                    machine_intent
                        .requirements
                        .sort_by(|left, right| left.identity.cmp(&right.identity));
                    machine_intent
                        .obligations
                        .sort_by(|left, right| left.identity.cmp(&right.identity));
                }
                if let Some(lowering) = &mut operation.lowering {
                    lowering.required_obligations.sort();
                    lowering.prohibited_transformations.sort();
                }
                if let Some(portability) = &mut operation.portability {
                    portability.required_features.sort();
                    portability.integer_widths.sort();
                }
            }
        }
        canonical
    }
}

fn validate_bounded_iterations(
    body: &FunctionBody,
    function: &Function,
    function_path: &str,
    cfg: &crate::Cfg,
    errors: &mut Vec<Diagnostic>,
) {
    let mut ids = BTreeSet::new();
    let declared_backedges = body
        .bounded_iterations
        .iter()
        .map(|iteration| (iteration.backedge.clone(), iteration.header.clone()))
        .collect::<BTreeSet<_>>();
    if body.cycle_policy == BodyCyclePolicy::BoundedIterationOnly {
        for block in &cfg.blocks {
            for target in &block.successors {
                if cfg.dominates(target, &block.id)
                    && !declared_backedges.contains(&(block.id.clone(), target.clone()))
                {
                    errors.push(body_diagnostic(
                        "MNB060",
                        format!("{function_path}.body.blocks.{}.terminator", block.id),
                        "cyclic control flow requires an exact bounded-iteration backedge",
                    ));
                }
            }
        }
    }
    for (index, iteration) in body.bounded_iterations.iter().enumerate() {
        let path = format!("{function_path}.body.bounded_iterations[{index}]");
        if iteration.id.trim().is_empty() || !ids.insert(iteration.id.clone()) {
            errors.push(body_diagnostic(
                "MNB061",
                format!("{path}.id"),
                "bounded iteration identities must be non-empty and unique per function",
            ));
        }
        if !(1..=SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND).contains(&iteration.bound) {
            errors.push(body_diagnostic(
                "MNB062",
                format!("{path}.bound"),
                format!(
                    "bounded iteration count must be between 1 and {}",
                    SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND
                ),
            ));
        }
        let blocks = [
            &iteration.preheader,
            &iteration.header,
            &iteration.body_entry,
            &iteration.backedge,
            &iteration.exit,
        ];
        if blocks.iter().any(|id| cfg.block(id).is_none()) {
            errors.push(body_diagnostic(
                "MNB063",
                path.clone(),
                "bounded iteration references a missing control-flow region block",
            ));
            continue;
        }
        let header = body
            .blocks
            .iter()
            .find(|block| block.id == iteration.header);
        let exit = body.blocks.iter().find(|block| block.id == iteration.exit);
        let Some(header) = header else { continue };
        let Some(exit) = exit else { continue };
        if header.parameters.len() != 2
            || header.parameters[0].id != iteration.header_state
            || header.parameters[0].ty != iteration.state_type
            || header.parameters[1].id != iteration.header_counter
            || header.parameters[1].ty
                != BodyType::Integer(IntegerType {
                    bits: 64,
                    signed: false,
                })
            || exit.parameters.len() != 1
            || exit.parameters[0].id != iteration.exit_state
            || exit.parameters[0].ty != iteration.state_type
        {
            errors.push(body_diagnostic(
                "MNB064",
                path.clone(),
                "bounded iteration carried state/counter parameters are malformed",
            ));
        }
        if !cfg
            .block(&iteration.backedge)
            .is_some_and(|block| block.successors == vec![iteration.header.clone()])
        {
            errors.push(body_diagnostic(
                "MNB065",
                path.clone(),
                "bounded iteration lost its declared backedge",
            ));
        }
        let required = iteration
            .required_capabilities
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let declared = function
            .capabilities
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if !required.is_subset(&declared) {
            errors.push(body_diagnostic(
                "MNB066",
                path.clone(),
                "bounded iteration cannot expand authority across attempts",
            ));
        }
        let modes = iteration
            .completion_modes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if !modes.contains(&BoundedIterationCompletion::Exhausted)
            || !modes.contains(&BoundedIterationCompletion::EarlyReturn)
        {
            errors.push(body_diagnostic(
                "MNB067",
                path,
                "bounded iteration must retain early-return and exhaustion completion modes",
            ));
        }
    }
}

impl BodyOperation {
    pub fn identity(&self, module: &str, function: &str, block: &str) -> SemanticId {
        operation_id(module, function, block, &self.id)
    }

    pub fn result_identity(
        &self,
        module: &str,
        function: &str,
        block: &str,
        result: &str,
    ) -> SemanticId {
        value_id(module, function, block, &self.id, result)
    }

    pub fn machine_intent_identity(
        &self,
        module: &str,
        function: &str,
        block: &str,
    ) -> Option<SemanticId> {
        self.machine_intent
            .as_ref()
            .map(|_| crate::identity::machine_intent_id(module, function, block, &self.id))
    }
}

fn validate_operation(
    program: &Program,
    function: &Function,
    operation: &BodyOperation,
    path: &str,
    available: &BTreeMap<String, BodyType>,
    errors: &mut Vec<Diagnostic>,
) {
    match &operation.kind {
        BodyOperationKind::Constant { ty, .. } => {
            if operation.operands.is_empty() && operation.results.len() == 1 {
                if operation.results[0].ty != *ty {
                    errors.push(body_diagnostic(
                        "MNB013",
                        format!("{path}.results"),
                        "constant result type does not match the constant type",
                    ));
                }
            } else {
                errors.push(body_diagnostic(
                    "MNB014",
                    path.to_owned(),
                    "constant operations require one result and no operands",
                ));
            }
        }
        BodyOperationKind::Integer {
            operator,
            operand_type,
            intent,
        } => {
            if !matches!(
                operator.as_str(),
                "add" | "sub" | "mul" | "div" | "mod" | "and" | "or" | "xor"
            ) {
                errors.push(body_diagnostic(
                    "MNB015",
                    format!("{path}.kind"),
                    format!("unsupported symbolic integer operator {:?}", operator),
                ));
            }
            if matches!(operator.as_str(), "and" | "or" | "xor")
                && !matches!(
                    &operation.kind,
                    BodyOperationKind::Integer {
                        intent: ArithmeticIntent::Wrapping,
                        ..
                    }
                )
            {
                errors.push(body_diagnostic(
                    "MNB043",
                    format!("{path}.kind.intent"),
                    "bitwise integer operations currently require wrapping intent",
                ));
            }
            if operation.operands.len() != 2 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB016",
                    path.to_owned(),
                    "integer operations require two operands and one result",
                ));
            }
            for operand in &operation.operands {
                if available.get(operand) != Some(&BodyType::Integer(*operand_type)) {
                    errors.push(body_diagnostic(
                        "MNB017",
                        format!("{path}.operands"),
                        "integer operand type does not match the operation type",
                    ));
                }
            }
            let result_type = crate::arithmetic_result_type(operator, *operand_type, *intent);
            if result_type.is_none() {
                errors.push(body_diagnostic(
                    "MNB058",
                    format!("{path}.kind.intent"),
                    "widening arithmetic requires an exact wider result; unsigned widening subtraction is unsupported and widths must be <= 126",
                ));
            }
            if operation.results.first().is_some_and(|result| {
                result_type.is_some_and(|ty| result.ty != BodyType::Integer(ty))
            }) {
                errors.push(body_diagnostic(
                    "MNB018",
                    format!("{path}.results"),
                    "integer result type does not match the arithmetic intent result type",
                ));
            }
        }
        BodyOperationKind::IntegerCompare {
            predicate,
            operand_type,
        } => {
            if !matches!(predicate.as_str(), "eq" | "ne" | "lt" | "le" | "gt" | "ge") {
                errors.push(body_diagnostic(
                    "MNB039",
                    format!("{path}.kind.predicate"),
                    format!("unsupported integer comparison predicate {predicate:?}"),
                ));
            }
            if operation.operands.len() != 2 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB040",
                    path.to_owned(),
                    "integer comparison requires two operands and one result",
                ));
            }
            for operand in &operation.operands {
                if available.get(operand) != Some(&BodyType::Integer(*operand_type)) {
                    errors.push(body_diagnostic(
                        "MNB041",
                        format!("{path}.operands"),
                        "integer comparison operand type does not match the operation type",
                    ));
                }
            }
            if operation
                .results
                .first()
                .is_some_and(|result| !result.ty.is_boolean())
            {
                errors.push(body_diagnostic(
                    "MNB042",
                    format!("{path}.results"),
                    "integer comparison result must have boolean type",
                ));
            }
        }
        BodyOperationKind::BooleanOp { operator } => {
            if !matches!(operator.as_str(), "and" | "or") {
                errors.push(body_diagnostic(
                    "MNB067",
                    format!("{path}.kind"),
                    format!("unsupported boolean operator {:?}", operator),
                ));
            }
            if operation.operands.len() != 2 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB068",
                    path.to_owned(),
                    "boolean operation requires two operands and one result",
                ));
            }
            let bool_type = BodyType::Named("bool".to_owned());
            for (index, operand) in operation.operands.iter().enumerate() {
                if available.get(operand) != Some(&bool_type) {
                    errors.push(body_diagnostic(
                        "MNB069",
                        format!("{path}.inputs[{index}]"),
                        "boolean operands must have bool type",
                    ));
                }
            }
            if operation
                .results
                .first()
                .is_some_and(|result| result.ty != bool_type)
            {
                errors.push(body_diagnostic(
                    "MNB070",
                    format!("{path}.results"),
                    "boolean operation result must have bool type",
                ));
            }
        }
        BodyOperationKind::FiniteConstruct {
            type_identity,
            variant_identity,
            discriminant,
            payload_fields,
        } => {
            if operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB044",
                    path.to_owned(),
                    "finite construction requires exactly one result",
                ));
            }
            let declared = program
                .finite_types
                .iter()
                .find(|item| &item.identity == type_identity);
            let valid_variant = declared.and_then(|item| {
                item.variants.iter().find(|variant| {
                    &variant.identity == variant_identity && variant.discriminant == *discriminant
                })
            });
            if declared.is_none() || valid_variant.is_none() {
                errors.push(body_diagnostic(
                    "MNB045",
                    format!("{path}.kind"),
                    "finite constructor does not identify a declared type/variant/discriminant",
                ));
            }
            // Payload integrity: names must be canonical (sorted, unique) and
            // every declared payload field must be supplied, and nothing more.
            if payload_fields
                .windows(2)
                .any(|window| window[0] >= window[1])
            {
                errors.push(body_diagnostic(
                    "MNB060",
                    format!("{path}.kind.payload_fields"),
                    "payload field names must be canonically sorted and unique",
                ));
            }
            let declared_payload = valid_variant.map(|variant| variant.payload.clone());
            match &declared_payload {
                Some(fields) => {
                    let declared_names: Vec<_> =
                        fields.iter().map(|field| field.name.clone()).collect();
                    if declared_names != *payload_fields
                        || operation.operands.len() != payload_fields.len()
                    {
                        errors.push(body_diagnostic(
                            "MNB061",
                            format!("{path}.kind.payload_fields"),
                            "finite constructor payload does not match the declared variant payload fields",
                        ));
                    }
                }
                None if !payload_fields.is_empty() || !operation.operands.is_empty() => {
                    errors.push(body_diagnostic(
                        "MNB062",
                        format!("{path}.kind"),
                        "variant carries no declared payload but the construction supplies one",
                    ));
                }
                None => {}
            }
            for (index, _operand) in operation.operands.iter().enumerate() {
                let expected_ty = payload_fields
                    .get(index)
                    .and_then(|name| {
                        valid_variant.and_then(|variant| {
                            variant.payload.iter().find(|field| &field.name == name)
                        })
                    })
                    .map(|field| semantic_record_field_type(program, field));
                let actual_ty = operation.results.first().map(|_| ()).and_then(|()| {
                    operation
                        .operands
                        .get(index)
                        .and_then(|operand_id| available.get(operand_id))
                        .cloned()
                });
                if expected_ty.is_some() && actual_ty.is_some() && expected_ty != actual_ty {
                    errors.push(body_diagnostic(
                        "MNB063",
                        format!("{path}.inputs[{index}]"),
                        "payload operand type does not match the declared payload field type",
                    ));
                }
            }
            if let Some(result) = operation.results.first() {
                let expected = declared.map(|item| BodyType::Finite {
                    identity: item.identity.clone(),
                    name: item.name.clone(),
                });
                if expected.as_ref() != Some(&result.ty) {
                    errors.push(body_diagnostic(
                        "MNB046",
                        format!("{path}.results"),
                        "finite constructor result type does not match its nominal type",
                    ));
                }
            }
        }
        BodyOperationKind::FiniteIsVariant {
            type_identity,
            variant_identity,
            discriminant,
        } => {
            if operation.operands.len() != 1 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB047",
                    path.to_owned(),
                    "finite variant test requires one operand and one boolean result",
                ));
            }
            let declared = program
                .finite_types
                .iter()
                .find(|item| &item.identity == type_identity);
            let valid_variant = declared.and_then(|item| {
                item.variants.iter().find(|variant| {
                    &variant.identity == variant_identity && variant.discriminant == *discriminant
                })
            });
            let expected_operand = declared.map(|item| BodyType::Finite {
                identity: item.identity.clone(),
                name: item.name.clone(),
            });
            if declared.is_none()
                || valid_variant.is_none()
                || operation
                    .operands
                    .first()
                    .and_then(|operand| available.get(operand))
                    != expected_operand.as_ref()
                || operation
                    .results
                    .first()
                    .is_some_and(|result| !result.ty.is_boolean())
            {
                errors.push(body_diagnostic(
                    "MNB048",
                    format!("{path}.kind"),
                    "finite variant test is not type/variant/discriminant consistent",
                ));
            }
        }
        BodyOperationKind::FinitePayloadProject {
            type_identity,
            variant_identity,
            discriminant,
            field,
        } => {
            if operation.operands.len() != 1 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB064",
                    path.to_owned(),
                    "payload projection requires one operand and one result",
                ));
            }
            let declared = program
                .finite_types
                .iter()
                .find(|item| &item.identity == type_identity);
            let valid_variant = declared.and_then(|item| {
                item.variants.iter().find(|variant| {
                    &variant.identity == variant_identity && variant.discriminant == *discriminant
                })
            });
            let payload_field = valid_variant.and_then(|variant| {
                variant
                    .payload
                    .iter()
                    .find(|item| item.name == *field)
                    .cloned()
            });
            let expected_operand = declared.map(|item| BodyType::Finite {
                identity: item.identity.clone(),
                name: item.name.clone(),
            });
            if declared.is_none()
                || valid_variant.is_none()
                || payload_field.is_none()
                || operation
                    .operands
                    .first()
                    .and_then(|operand| available.get(operand))
                    != expected_operand.as_ref()
            {
                errors.push(body_diagnostic(
                    "MNB065",
                    format!("{path}.kind"),
                    "payload projection is not consistent with a declared variant payload field",
                ));
            }
            if let (Some(field), Some(result)) = (payload_field, operation.results.first()) {
                if semantic_record_field_type(program, &field) != result.ty {
                    errors.push(body_diagnostic(
                        "MNB066",
                        format!("{path}.results"),
                        "payload projection result type does not match the declared field type",
                    ));
                }
            }
        }
        BodyOperationKind::RecordConstruct {
            type_identity,
            field_names,
        } => {
            if operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB048",
                    path.to_owned(),
                    "record construction requires exactly one result",
                ));
            }
            let declared = program
                .record_types
                .iter()
                .find(|item| &item.identity == type_identity);
            let Some(declared) = declared else {
                errors.push(body_diagnostic(
                    "MNB049",
                    format!("{path}.kind"),
                    "record construction does not identify a declared record type",
                ));
                return;
            };
            // Canonical field order is sorted by field name; the operand list
            // must follow it so one logical construction has one encoding.
            let mut canonical: Vec<String> = declared
                .fields
                .iter()
                .map(|field| field.name.clone())
                .collect();
            canonical.sort();
            let mut supplied = field_names.clone();
            supplied.sort();
            supplied.dedup();
            if &supplied != field_names {
                errors.push(body_diagnostic(
                    "MNB050",
                    format!("{path}.kind.field_names"),
                    "record construction field names are not in canonical order",
                ));
            }
            if supplied != canonical || field_names.len() != operation.operands.len() {
                errors.push(body_diagnostic(
                    "MNB051",
                    format!("{path}.kind"),
                    "record construction fields do not exactly match the declared fields",
                ));
                return;
            }
            for (index, field) in declared.fields.iter().enumerate() {
                let expected_field_type = semantic_record_field_type(program, field);
                let Some(operand) = operation.operands.get(index) else {
                    break;
                };
                let actual = available.get(operand);
                if actual != Some(&expected_field_type) {
                    errors.push(body_diagnostic(
                        "MNB052",
                        format!("{path}.operands[{index}]"),
                        "record field value type does not match the declared field type",
                    ));
                }
            }
            if let Some(result) = operation.results.first() {
                let expected = BodyType::Record {
                    identity: declared.identity.clone(),
                    name: declared.name.clone(),
                };
                if result.ty != expected {
                    errors.push(body_diagnostic(
                        "MNB053",
                        format!("{path}.results"),
                        "record construction result type does not match its nominal record",
                    ));
                }
            }
        }
        BodyOperationKind::RecordProject {
            type_identity,
            field,
        } => {
            if operation.operands.len() != 1 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB054",
                    path.to_owned(),
                    "record projection requires one operand and one result",
                ));
                return;
            }
            let declared = program
                .record_types
                .iter()
                .find(|item| &item.identity == type_identity);
            let Some(declared) = declared else {
                errors.push(body_diagnostic(
                    "MNB049",
                    format!("{path}.kind"),
                    "record projection does not identify a declared record type",
                ));
                return;
            };
            let expected_operand = BodyType::Record {
                identity: declared.identity.clone(),
                name: declared.name.clone(),
            };
            if operation
                .operands
                .first()
                .and_then(|operand| available.get(operand))
                != Some(&expected_operand)
            {
                errors.push(body_diagnostic(
                    "MNB055",
                    format!("{path}.operands[0]"),
                    "record projection operand does not have the projected record type",
                ));
            }
            match declared.fields.iter().find(|item| &item.name == field) {
                None => errors.push(body_diagnostic(
                    "MNB056",
                    format!("{path}.kind.field"),
                    "record projection names a field the record does not declare",
                )),
                Some(declared_field) => {
                    let expected_field_type = semantic_record_field_type(program, declared_field);
                    if operation.results.first().map(|result| &result.ty)
                        != Some(&expected_field_type)
                    {
                        errors.push(body_diagnostic(
                            "MNB057",
                            format!("{path}.results"),
                            "record projection result type does not match the field type",
                        ));
                    }
                }
            }
        }
        BodyOperationKind::Call {
            function: callee_identity,
            function_name,
            required_capabilities,
            effects,
        } => {
            let callee = program.functions.iter().find(|candidate| {
                candidate.name == *function_name
                    && crate::identity::function_id(&program.module, &candidate.name)
                        == *callee_identity
            });
            let Some(callee) = callee else {
                errors.push(body_diagnostic(
                    "MNB049",
                    format!("{path}.kind.function"),
                    "call target does not resolve to an exact function identity",
                ));
                validate_metadata(program, function, operation, path, errors);
                return;
            };
            if operation.operands.len() != callee.inputs.len()
                || operation.results.len() != callee.outputs.len()
            {
                errors.push(body_diagnostic(
                    "MNB050",
                    path.to_owned(),
                    "call arity does not match the callee signature",
                ));
            }
            for (operand, parameter) in operation.operands.iter().zip(&callee.inputs) {
                if available
                    .get(operand)
                    .map(BodyType::semantic_name)
                    .as_deref()
                    != Some(parameter.value_type.as_str())
                {
                    errors.push(body_diagnostic(
                        "MNB051",
                        format!("{path}.operands"),
                        "call argument type does not match the callee parameter",
                    ));
                }
            }
            for (result, output) in operation.results.iter().zip(&callee.outputs) {
                if result.ty.semantic_name() != output.value_type {
                    errors.push(body_diagnostic(
                        "MNB052",
                        format!("{path}.results"),
                        "call result type does not match the callee output",
                    ));
                }
            }
            let mut declared_required = callee.capabilities.clone();
            declared_required.sort();
            let mut recorded_required = required_capabilities.clone();
            recorded_required.sort();
            let mut declared_effects = callee.effects.clone();
            declared_effects.sort_by(|left, right| {
                (&left.kind, &left.target, &left.capability).cmp(&(
                    &right.kind,
                    &right.target,
                    &right.capability,
                ))
            });
            let mut recorded_effects = effects.clone();
            recorded_effects.sort_by(|left, right| {
                (&left.kind, &left.target, &left.capability).cmp(&(
                    &right.kind,
                    &right.target,
                    &right.capability,
                ))
            });
            let caller_authorizes = required_capabilities
                .iter()
                .all(|capability| function.capabilities.contains(capability))
                && effects.iter().all(|effect| {
                    function.effects.iter().any(|caller_effect| {
                        caller_effect.kind == effect.kind
                            && caller_effect.capability == effect.capability
                    })
                });
            if recorded_required != declared_required
                || recorded_effects != declared_effects
                || !caller_authorizes
            {
                errors.push(body_diagnostic(
                    "MNB053",
                    format!("{path}.kind"),
                    "call authority/effect closure is incomplete or was laundered",
                ));
            }
        }
        BodyOperationKind::Effect { effect, capability } => {
            if !operation.results.is_empty() || !operation.operands.is_empty() {
                errors.push(body_diagnostic(
                    "MNB019",
                    path.to_owned(),
                    "effect operations do not currently produce values or consume operands",
                ));
            }
            if capability != &effect.capability || !function.capabilities.contains(capability) {
                errors.push(body_diagnostic(
                    "MNB020",
                    format!("{path}.kind"),
                    "body effect capability does not match a declared function capability",
                ));
            }
            if !function.effects.iter().any(|declared| declared == effect) {
                errors.push(body_diagnostic(
                    "MNB021",
                    format!("{path}.kind"),
                    "body effect is not declared by the enclosing function",
                ));
            }
        }
        BodyOperationKind::RuntimeCheck {
            obligation,
            fact,
            failure: _,
        } => {
            if obligation.0.trim().is_empty() || fact.identity.0.trim().is_empty() {
                errors.push(body_diagnostic(
                    "MNB022",
                    path.to_owned(),
                    "runtime checks require an obligation and fact identity",
                ));
            }
        }
    }
    validate_metadata(program, function, operation, path, errors);
}

fn validate_metadata(
    program: &Program,
    function: &Function,
    operation: &BodyOperation,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    for contract in &operation.contracts {
        if !function.contracts.iter().any(|item| item.id == *contract) {
            errors.push(body_diagnostic(
                "MNB023",
                format!("{path}.contracts"),
                format!("unknown contract reference {:?}", contract),
            ));
        }
    }
    for assumption in &operation.assumptions {
        if !program
            .assumptions
            .iter()
            .any(|item| item.id == *assumption)
            || !function.assumptions.contains(assumption)
        {
            errors.push(body_diagnostic(
                "MNB024",
                format!("{path}.assumptions"),
                format!(
                    "assumption {:?} is not consumed by the function",
                    assumption
                ),
            ));
        }
    }
    if let Some(machine_intent) = &operation.machine_intent {
        if machine_intent.intent.identity.0.trim().is_empty() {
            errors.push(body_diagnostic(
                "MNB025",
                format!("{path}.machine_intent"),
                "machine intent requires a stable identity",
            ));
        }
        let mut requirement_ids = BTreeSet::new();
        for requirement in &machine_intent.requirements {
            if requirement.identity.0.trim().is_empty()
                || !requirement_ids.insert(requirement.identity.clone())
                || requirement.subject.0.trim().is_empty()
            {
                errors.push(body_diagnostic(
                    "MNB026",
                    format!("{path}.machine_intent.requirements"),
                    "machine-intent requirements must have unique identities and subjects",
                ));
            }
        }
        let mut obligation_ids = BTreeSet::new();
        for obligation in &machine_intent.obligations {
            if obligation.identity.0.trim().is_empty()
                || !obligation_ids.insert(obligation.identity.clone())
                || obligation.subject.0.trim().is_empty()
            {
                errors.push(body_diagnostic(
                    "MNB027",
                    format!("{path}.machine_intent.obligations"),
                    "machine-intent obligations must have unique identities and subjects",
                ));
            }
        }
    }
    if let Some(lowering) = &operation.lowering {
        if lowering.schema_version != EXECUTABLE_BODY_SCHEMA_VERSION {
            errors.push(body_diagnostic(
                "MNB028",
                format!("{path}.lowering.schema_version"),
                "unsupported lowering-envelope schema",
            ));
        }
        if lowering.permitted_realizations.is_empty() && lowering.fallback.is_none() {
            errors.push(body_diagnostic(
                "MNB029",
                format!("{path}.lowering"),
                "lowering envelope needs a realization class or conservative fallback",
            ));
        }
    }
    if let Some(portability) = &operation.portability {
        if portability.schema_version != EXECUTABLE_BODY_SCHEMA_VERSION {
            errors.push(body_diagnostic(
                "MNB030",
                format!("{path}.portability.schema_version"),
                "unsupported portability-envelope schema",
            ));
        }
        if portability.integer_widths.contains(&0) {
            errors.push(body_diagnostic(
                "MNB031",
                format!("{path}.portability.integer_widths"),
                "portability integer widths must be nonzero",
            ));
        }
    }
}

fn validate_terminator(
    terminator: &BodyTerminator,
    path: &str,
    available: &BTreeMap<String, BodyType>,
    block_ids: &BTreeSet<String>,
    block_parameters: &BTreeMap<String, Vec<BodyValue>>,
    errors: &mut Vec<Diagnostic>,
) {
    match terminator {
        BodyTerminator::Failure { .. } => {}
        BodyTerminator::Return { values } => {
            check_terminator_values(values, path, available, errors);
        }
        BodyTerminator::Branch { target, arguments } => {
            check_target(target, path, block_ids, errors);
            check_terminator_values(arguments, path, available, errors);
            validate_block_arguments(target, arguments, available, block_parameters, path, errors);
        }
        BodyTerminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => {
            if !available.get(condition).is_some_and(BodyType::is_boolean) {
                errors.push(body_diagnostic(
                    "MNB033",
                    format!("{path}.terminator.condition"),
                    "conditional branch requires a boolean condition",
                ));
            }
            check_target(then_target, path, block_ids, errors);
            check_target(else_target, path, block_ids, errors);
            check_terminator_values(then_arguments, path, available, errors);
            check_terminator_values(else_arguments, path, available, errors);
            validate_block_arguments(
                then_target,
                then_arguments,
                available,
                block_parameters,
                path,
                errors,
            );
            validate_block_arguments(
                else_target,
                else_arguments,
                available,
                block_parameters,
                path,
                errors,
            );
        }
    }
}

fn validate_block_arguments(
    target: &str,
    arguments: &[String],
    available: &BTreeMap<String, BodyType>,
    block_parameters: &BTreeMap<String, Vec<BodyValue>>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let Some(parameters) = block_parameters.get(target) else {
        return;
    };
    if parameters.len() != arguments.len() {
        errors.push(body_diagnostic(
            "MNB012",
            format!("{path}.terminator"),
            "control-flow arguments must match target block parameters",
        ));
    }
    for (index, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
        if available.get(argument) != Some(&parameter.ty) {
            errors.push(body_diagnostic(
                "MNB037",
                format!("{path}.terminator.arguments[{index}]"),
                "control-flow argument type does not match the target parameter",
            ));
        }
    }
}

fn check_terminator_values(
    values: &[String],
    path: &str,
    available: &BTreeMap<String, BodyType>,
    errors: &mut Vec<Diagnostic>,
) {
    for value in values {
        if !available.contains_key(value) {
            errors.push(body_diagnostic(
                "MNB032",
                format!("{path}.terminator"),
                format!("terminator value {:?} is not defined before use", value),
            ));
        }
    }
}

fn check_target(
    target: &str,
    path: &str,
    block_ids: &BTreeSet<String>,
    errors: &mut Vec<Diagnostic>,
) {
    if !block_ids.contains(target) {
        errors.push(body_diagnostic(
            "MNB034",
            format!("{path}.terminator"),
            format!("unknown block target {:?}", target),
        ));
    }
}

fn validate_return_types(
    terminator: &BodyTerminator,
    program: &Program,
    function: &Function,
    available: &BTreeMap<String, BodyType>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let BodyTerminator::Return { values } = terminator else {
        return;
    };
    if values.len() != function.outputs.len() {
        errors.push(body_diagnostic(
            "MNB035",
            format!("{path}.body"),
            "body return arity does not match function outputs",
        ));
        return;
    }
    for (index, value) in values.iter().enumerate() {
        let expected = semantic_body_type(program, &function.outputs[index].value_type);
        if available.get(value) != Some(&expected) {
            errors.push(body_diagnostic(
                "MNB036",
                format!("{path}.body"),
                format!(
                    "return value {:?} does not match declared output type",
                    value
                ),
            ));
        }
    }
}

fn semantic_body_type(program: &Program, name: &str) -> BodyType {
    if let Some(record_type) = program.record_types.iter().find(|item| item.name == name) {
        return BodyType::Record {
            identity: record_type.identity.clone(),
            name: record_type.name.clone(),
        };
    }
    program
        .finite_types
        .iter()
        .find(|finite_type| finite_type.name == name)
        .map(|finite_type| BodyType::Finite {
            identity: finite_type.identity.clone(),
            name: finite_type.name.clone(),
        })
        .unwrap_or_else(|| BodyType::from_semantic_name(name))
}

/// Resolve a declared record field's semantic body type at validation time.
fn semantic_record_field_type(program: &Program, field: &crate::core::RecordField) -> BodyType {
    semantic_body_type(program, &field.field_type)
}

fn body_diagnostic(code: &str, path: String, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        path,
        message: message.into(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{FailureMode, Value, SUPPORTED_SCHEMA_VERSION};

    pub(crate) fn executable_program() -> Program {
        let mut program = crate::validation::tests::valid_program();
        let function = &mut program.functions[0];
        function.inputs = vec![Value {
            name: "a".to_owned(),
            value_type: "i32".to_owned(),
        }];
        function.outputs = vec![Value {
            name: "result".to_owned(),
            value_type: "i32".to_owned(),
        }];
        function.contracts.clear();
        function.effects.clear();
        function.capabilities.clear();
        function.assumptions.clear();
        function.evidence.clear();
        function.failure = FailureMode::Isolated;
        function.body = Some(FunctionBody {
            schema_version: EXECUTABLE_BODY_SCHEMA_VERSION.to_owned(),
            entry: "entry".to_owned(),
            parameters: vec![BodyParameter {
                id: "a".to_owned(),
                name: "a".to_owned(),
                ty: BodyType::Integer(IntegerType {
                    bits: 32,
                    signed: true,
                }),
            }],
            cycle_policy: BodyCyclePolicy::Legacy,
            bounded_iterations: Vec::new(),
            blocks: vec![BodyBlock {
                id: "entry".to_owned(),
                parameters: Vec::new(),
                operations: vec![
                    BodyOperation {
                        id: "one".to_owned(),
                        kind: BodyOperationKind::Constant {
                            value: 1,
                            ty: BodyType::Integer(IntegerType {
                                bits: 32,
                                signed: true,
                            }),
                        },
                        operands: Vec::new(),
                        results: vec![BodyValue {
                            id: "one".to_owned(),
                            ty: BodyType::Integer(IntegerType {
                                bits: 32,
                                signed: true,
                            }),
                        }],
                        contracts: Vec::new(),
                        assumptions: Vec::new(),
                        machine_intent: None,
                        lowering: None,
                        portability: None,
                    },
                    BodyOperation {
                        id: "sum".to_owned(),
                        kind: BodyOperationKind::Integer {
                            operator: "add".to_owned(),
                            operand_type: IntegerType {
                                bits: 32,
                                signed: true,
                            },
                            intent: ArithmeticIntent::Checked,
                        },
                        operands: vec!["a".to_owned(), "one".to_owned()],
                        results: vec![BodyValue {
                            id: "sum".to_owned(),
                            ty: BodyType::Integer(IntegerType {
                                bits: 32,
                                signed: true,
                            }),
                        }],
                        contracts: Vec::new(),
                        assumptions: Vec::new(),
                        machine_intent: None,
                        lowering: None,
                        portability: None,
                    },
                ],
                terminator: BodyTerminator::Return {
                    values: vec!["sum".to_owned()],
                },
            }],
        });
        program
    }

    #[test]
    fn symbolic_body_validates_and_has_deterministic_identity() {
        let program = executable_program();
        let report = program.validate();
        assert!(report.valid, "{:#?}", report.errors);
        let body = program.functions[0].body.as_ref().expect("body");
        assert_eq!(
            body.canonical_json().unwrap(),
            body.canonical_json().unwrap()
        );
        assert_eq!(body.fingerprint().unwrap().len(), 64);
        assert_eq!(
            body.identity(&program.module, &program.functions[0].name),
            body.identity(&program.module, &program.functions[0].name)
        );
        let graph = program.semantic_graph().expect("body graph");
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == crate::EdgeKind::ContainsOperation));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == crate::EdgeKind::ConsumesValue));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == crate::EdgeKind::RequiresObligation));
        let ir = program.lower_to_ir().expect("body IR");
        let operations = &ir.functions[0].blocks[0].operations;
        assert!(operations.iter().any(|operation| {
            matches!(operation.kind, crate::IrOperationKind::Integer { .. })
                && operation.machine_intent.is_some()
        }));
        assert!(ir
            .obligations
            .iter()
            .any(|obligation| obligation.status == crate::ObligationStatus::Unknown));
        assert!(!ir.transformations.is_empty());
        let mut changed = program.clone();
        let changed_body = changed.functions[0].body.as_mut().expect("body");
        let integer = changed_body.blocks[0]
            .operations
            .iter_mut()
            .find(|operation| matches!(operation.kind, BodyOperationKind::Integer { .. }))
            .expect("integer operation");
        if let BodyOperationKind::Integer { intent, .. } = &mut integer.kind {
            *intent = ArithmeticIntent::Wrapping;
        }
        assert_ne!(
            program.functions[0]
                .body
                .as_ref()
                .expect("body")
                .fingerprint()
                .unwrap(),
            changed_body.fingerprint().unwrap()
        );
    }

    #[test]
    fn body_rejects_undefined_values_and_type_mismatches() {
        let mut program = executable_program();
        let body = program.functions[0].body.as_mut().expect("body");
        "missing".clone_into(&mut body.blocks[0].operations[1].operands[0]);
        body.blocks[0].operations[1].results[0].ty = BodyType::Integer(IntegerType {
            bits: 16,
            signed: true,
        });
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB010"));
        assert!(report.errors.iter().any(|error| error.code == "MNB017"));
        assert!(report.errors.iter().any(|error| error.code == "MNB018"));
    }

    #[test]
    fn body_rejects_undeclared_effect_and_bad_return() {
        let mut program = executable_program();
        let function = &mut program.functions[0];
        "u32".clone_into(&mut function.outputs[0].value_type);
        let body = function.body.as_mut().expect("body");
        body.blocks[0].operations[0].kind = BodyOperationKind::Effect {
            effect: Effect {
                kind: "write".to_owned(),
                target: "Store.value".to_owned(),
                capability: "StoreMutation".to_owned(),
            },
            capability: "StoreMutation".to_owned(),
        };
        body.blocks[0].operations[0].operands.clear();
        body.blocks[0].operations[0].results.clear();
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB020"));
        assert!(report.errors.iter().any(|error| error.code == "MNB021"));
        assert!(report.errors.iter().any(|error| error.code == "MNB036"));
        assert_eq!(program.schema_version, SUPPORTED_SCHEMA_VERSION);
    }

    #[test]
    fn body_rejects_unknown_integer_comparison_predicate() {
        let mut program = executable_program();
        let body = program.functions[0].body.as_mut().expect("body");
        let operation = &mut body.blocks[0].operations[1];
        operation.kind = BodyOperationKind::IntegerCompare {
            predicate: "unordered".to_owned(),
            operand_type: IntegerType {
                bits: 32,
                signed: true,
            },
        };
        operation.operands = vec!["a".to_owned(), "one".to_owned()];
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB039"));
    }

    #[test]
    fn body_rejects_non_wrapping_bitwise_intent() {
        let mut program = executable_program();
        let body = program.functions[0].body.as_mut().expect("body");
        let operation = &mut body.blocks[0].operations[1];
        operation.kind = BodyOperationKind::Integer {
            operator: "xor".to_owned(),
            operand_type: IntegerType {
                bits: 32,
                signed: true,
            },
            intent: ArithmeticIntent::Checked,
        };
        operation.operands = vec!["a".to_owned(), "one".to_owned()];
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB043"));
    }

    #[test]
    fn body_requires_the_exact_widening_result_type() {
        let mut program = Program::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/executable/arithmetic-envelope.mncs.json"
        )))
        .expect("arithmetic envelope");
        assert!(program.validate().valid);
        let wide = program.functions[1].body.as_mut().expect("body").blocks[0]
            .operations
            .first_mut()
            .expect("widening operation");
        wide.results[0].ty = BodyType::Integer(IntegerType {
            bits: 15,
            signed: true,
        });
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB018"));
    }

    #[test]
    fn body_rejects_mismatched_block_arguments() {
        let mut program = executable_program();
        let body = program.functions[0].body.as_mut().expect("body");
        body.blocks[0].terminator = BodyTerminator::Branch {
            target: "join".to_owned(),
            arguments: vec!["a".to_owned()],
        };
        body.blocks.push(BodyBlock {
            id: "join".to_owned(),
            parameters: vec![BodyValue {
                id: "joined".to_owned(),
                ty: BodyType::Named("bool".to_owned()),
            }],
            operations: Vec::new(),
            terminator: BodyTerminator::Return {
                values: vec!["joined".to_owned()],
            },
        });
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB037"));
    }

    #[test]
    fn body_rejects_value_from_non_dominating_branch() {
        let mut program = executable_program();
        let function = &mut program.functions[0];
        function.inputs = vec![Value {
            name: "condition".to_owned(),
            value_type: "bool".to_owned(),
        }];
        let body = function.body.as_mut().expect("body");
        body.parameters = vec![BodyParameter {
            id: "condition".to_owned(),
            name: "condition".to_owned(),
            ty: BodyType::Named("bool".to_owned()),
        }];
        body.blocks = vec![
            BodyBlock {
                id: "entry".to_owned(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: BodyTerminator::ConditionalBranch {
                    condition: "condition".to_owned(),
                    then_target: "left".to_owned(),
                    then_arguments: Vec::new(),
                    else_target: "right".to_owned(),
                    else_arguments: Vec::new(),
                },
            },
            BodyBlock {
                id: "left".to_owned(),
                parameters: Vec::new(),
                operations: vec![BodyOperation {
                    id: "left-value".to_owned(),
                    kind: BodyOperationKind::Constant {
                        value: 1,
                        ty: BodyType::Integer(IntegerType {
                            bits: 32,
                            signed: true,
                        }),
                    },
                    operands: Vec::new(),
                    results: vec![BodyValue {
                        id: "left-value".to_owned(),
                        ty: BodyType::Integer(IntegerType {
                            bits: 32,
                            signed: true,
                        }),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                }],
                terminator: BodyTerminator::Branch {
                    target: "join".to_owned(),
                    arguments: Vec::new(),
                },
            },
            BodyBlock {
                id: "right".to_owned(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: BodyTerminator::Branch {
                    target: "join".to_owned(),
                    arguments: Vec::new(),
                },
            },
            BodyBlock {
                id: "join".to_owned(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: BodyTerminator::Return {
                    values: vec!["left-value".to_owned()],
                },
            },
        ];
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB032"));
    }

    #[test]
    fn body_rejects_unreachable_blocks_explicitly() {
        let mut program = executable_program();
        let body = program.functions[0].body.as_mut().expect("body");
        body.blocks.push(BodyBlock {
            id: "dead".to_owned(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: BodyTerminator::Return {
                values: vec!["a".to_owned()],
            },
        });
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNB038"));
    }
}
