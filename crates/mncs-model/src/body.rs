use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::identity::{block_id, body_id, operation_id, value_id};
use crate::{
    ArithmeticIntent, Diagnostic, Effect, Fact, FailureMode, Function, IntegerType, Intent,
    MachinePreference, Obligation, Program, Requirement, SemanticId,
};

pub const EXECUTABLE_BODY_SCHEMA_VERSION: &str = "0.2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionBody {
    pub schema_version: String,
    pub entry: String,
    pub parameters: Vec<BodyParameter>,
    pub blocks: Vec<BodyBlock>,
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
                    || parameter.ty != BodyType::from_semantic_name(&input.value_type)
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
        let mut operation_ids = BTreeSet::new();
        let mut available = values;
        let mut incoming = Vec::<(String, Vec<String>, Vec<BodyType>)>::new();
        for (block_index, block) in self.blocks.iter().enumerate() {
            let block_path = format!("{function_path}.body.blocks[{block_index}]");
            let mut block_values = BTreeSet::new();
            for parameter in &block.parameters {
                if !block_values.insert(parameter.id.clone())
                    || available.contains_key(&parameter.id)
                {
                    errors.push(body_diagnostic(
                        "MNB008",
                        format!("{block_path}.parameters"),
                        format!("duplicate body value identity {:?}", parameter.id),
                    ));
                }
                available.insert(parameter.id.clone(), parameter.ty.clone());
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
                    if available.contains_key(&result.id) || !block_values.insert(result.id.clone())
                    {
                        errors.push(body_diagnostic(
                            "MNB011",
                            format!("{operation_path}.results"),
                            format!("duplicate body value identity {:?}", result.id),
                        ));
                    }
                    available.insert(result.id.clone(), result.ty.clone());
                }
            }
            validate_terminator(
                &block.terminator,
                &block_path,
                &available,
                &block_ids,
                errors,
                &mut incoming,
            );
        }
        for (target, arguments, argument_types) in incoming {
            if let Some(block) = self.blocks.iter().find(|block| block.id == target) {
                if block.parameters.len() != arguments.len() {
                    errors.push(body_diagnostic(
                        "MNB012",
                        format!("{function_path}.body.blocks.{target}"),
                        "control-flow arguments must match target block parameters",
                    ));
                }
                for (index, (parameter, argument_type)) in block
                    .parameters
                    .iter()
                    .zip(argument_types.iter())
                    .enumerate()
                {
                    if parameter.ty != *argument_type {
                        errors.push(body_diagnostic(
                            "MNB037",
                            format!("{function_path}.body.blocks.{target}.parameters[{index}]"),
                            "control-flow argument type does not match the target parameter",
                        ));
                    }
                }
            }
        }
        for block in &self.blocks {
            if matches!(block.terminator, BodyTerminator::Return { .. }) {
                validate_return_types(
                    &block.terminator,
                    function,
                    &available,
                    function_path,
                    errors,
                );
            }
        }
    }

    fn canonicalized(&self) -> Self {
        let mut canonical = self.clone();
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
            intent: _,
        } => {
            if operator != "add" {
                errors.push(body_diagnostic(
                    "MNB015",
                    format!("{path}.kind"),
                    format!("unsupported symbolic integer operator {:?}", operator),
                ));
            }
            if operation.operands.len() != 2 || operation.results.len() != 1 {
                errors.push(body_diagnostic(
                    "MNB016",
                    path.to_owned(),
                    "integer addition requires two operands and one result",
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
            if operation
                .results
                .first()
                .is_some_and(|result| result.ty != BodyType::Integer(*operand_type))
            {
                errors.push(body_diagnostic(
                    "MNB018",
                    format!("{path}.results"),
                    "integer result type does not match the operation type",
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
    errors: &mut Vec<Diagnostic>,
    incoming: &mut Vec<(String, Vec<String>, Vec<BodyType>)>,
) {
    match terminator {
        BodyTerminator::Return { values } => {
            check_terminator_values(values, path, available, errors);
        }
        BodyTerminator::Branch { target, arguments } => {
            check_target(target, path, block_ids, errors);
            check_terminator_values(arguments, path, available, errors);
            incoming.push((
                target.clone(),
                arguments.clone(),
                argument_types(arguments, available),
            ));
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
            incoming.push((
                then_target.clone(),
                then_arguments.clone(),
                argument_types(then_arguments, available),
            ));
            incoming.push((
                else_target.clone(),
                else_arguments.clone(),
                argument_types(else_arguments, available),
            ));
        }
    }
}

fn argument_types(arguments: &[String], available: &BTreeMap<String, BodyType>) -> Vec<BodyType> {
    arguments
        .iter()
        .filter_map(|argument| available.get(argument).cloned())
        .collect()
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
        let expected = BodyType::from_semantic_name(&function.outputs[index].value_type);
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

fn body_diagnostic(code: &str, path: String, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        path,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FailureMode, Value, SUPPORTED_SCHEMA_VERSION};

    fn executable_program() -> Program {
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
        body.blocks[0].operations[1].operands[0] = "missing".to_owned();
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
        function.outputs[0].value_type = "u32".to_owned();
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
}
