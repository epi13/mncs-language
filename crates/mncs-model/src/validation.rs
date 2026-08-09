use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{Program, Value, SUPPORTED_SCHEMA_VERSION};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationReport {
    pub valid: bool,
    pub errors: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
    pub summary: ValidationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationSummary {
    pub modules: usize,
    pub functions: usize,
    pub contracts: usize,
    pub effects: usize,
    pub evidence_claims: usize,
    pub assumptions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
}

impl Program {
    pub fn validate(&self) -> ValidationReport {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            errors.push(diagnostic(
                "MNCS001",
                "schema_version",
                format!(
                    "unsupported schema version {:?}; expected {:?}",
                    self.schema_version, SUPPORTED_SCHEMA_VERSION
                ),
            ));
        }

        if self.module.trim().is_empty() {
            errors.push(diagnostic(
                "MNCS002",
                "module",
                "module name must not be empty",
            ));
        }

        let assumption_ids = collect_unique_ids(
            self.assumptions.iter().map(|item| item.id.as_str()),
            "assumptions",
            "MNCS003",
            &mut errors,
        );

        let mut function_names = BTreeSet::new();
        for (function_index, function) in self.functions.iter().enumerate() {
            let function_path = format!("functions[{function_index}]");
            if function.name.trim().is_empty() {
                errors.push(diagnostic(
                    "MNCS004",
                    format!("{function_path}.name"),
                    "function name must not be empty",
                ));
            } else if !function_names.insert(function.name.as_str()) {
                errors.push(diagnostic(
                    "MNCS005",
                    format!("{function_path}.name"),
                    format!("duplicate function name {:?}", function.name),
                ));
            }

            validate_named_values(&function.inputs, &function_path, "inputs", &mut errors);
            validate_named_values(&function.outputs, &function_path, "outputs", &mut errors);

            let contract_ids = collect_unique_ids(
                function.contracts.iter().map(|item| item.id.as_str()),
                &format!("{function_path}.contracts"),
                "MNCS006",
                &mut errors,
            );

            let declared_capabilities: BTreeSet<&str> =
                function.capabilities.iter().map(String::as_str).collect();
            if declared_capabilities.len() != function.capabilities.len() {
                errors.push(diagnostic(
                    "MNCS007",
                    format!("{function_path}.capabilities"),
                    "capability declarations must be unique",
                ));
            }

            for (effect_index, effect) in function.effects.iter().enumerate() {
                let effect_path = format!("{function_path}.effects[{effect_index}]");
                if effect.kind.trim().is_empty() || effect.target.trim().is_empty() {
                    errors.push(diagnostic(
                        "MNCS008",
                        effect_path.clone(),
                        "effect kind and target must not be empty",
                    ));
                }
                if effect.capability.trim().is_empty() {
                    errors.push(diagnostic(
                        "MNCS009",
                        format!("{effect_path}.capability"),
                        "every effect must name its authorizing capability",
                    ));
                } else if !declared_capabilities.contains(effect.capability.as_str()) {
                    errors.push(diagnostic(
                        "MNCS010",
                        format!("{effect_path}.capability"),
                        format!(
                            "effect requires undeclared capability {:?}",
                            effect.capability
                        ),
                    ));
                }
            }

            for (index, assumption) in function.assumptions.iter().enumerate() {
                if !assumption_ids.contains(assumption.as_str()) {
                    errors.push(diagnostic(
                        "MNCS011",
                        format!("{function_path}.assumptions[{index}]"),
                        format!("unknown assumption reference {assumption:?}"),
                    ));
                }
            }

            let mut evidence_by_property: BTreeMap<&str, usize> = BTreeMap::new();
            for (evidence_index, evidence) in function.evidence.iter().enumerate() {
                let evidence_path = format!("{function_path}.evidence[{evidence_index}]");
                if !contract_ids.contains(evidence.property.as_str()) {
                    errors.push(diagnostic(
                        "MNCS012",
                        format!("{evidence_path}.property"),
                        format!(
                            "evidence references unknown contract property {:?}",
                            evidence.property
                        ),
                    ));
                }
                if evidence.verifier.trim().is_empty() {
                    errors.push(diagnostic(
                        "MNCS013",
                        format!("{evidence_path}.verifier"),
                        "evidence must identify a verifier",
                    ));
                }
                *evidence_by_property
                    .entry(evidence.property.as_str())
                    .or_default() += 1;
            }

            for contract in &function.contracts {
                if !evidence_by_property.contains_key(contract.id.as_str()) {
                    warnings.push(diagnostic(
                        "MNCS101",
                        format!("{function_path}.contracts.{}", contract.id),
                        "contract has no evidence claim",
                    ));
                }
            }
            if function.contracts.is_empty() {
                warnings.push(diagnostic(
                    "MNCS102",
                    format!("{function_path}.contracts"),
                    "function declares no behavioral contract",
                ));
            }
        }

        if self.functions.is_empty() {
            warnings.push(diagnostic(
                "MNCS103",
                "functions",
                "program contains no functions",
            ));
        }

        ValidationReport {
            valid: errors.is_empty(),
            errors,
            warnings,
            summary: ValidationSummary {
                modules: usize::from(!self.module.trim().is_empty()),
                functions: self.functions.len(),
                contracts: self.functions.iter().map(|f| f.contracts.len()).sum(),
                effects: self.functions.iter().map(|f| f.effects.len()).sum(),
                evidence_claims: self.functions.iter().map(|f| f.evidence.len()).sum(),
                assumptions: self.assumptions.len(),
            },
        }
    }
}

fn validate_named_values(
    values: &[Value],
    function_path: &str,
    collection: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let mut names = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let path = format!("{function_path}.{collection}[{index}]");
        if value.name.trim().is_empty() || value.value_type.trim().is_empty() {
            errors.push(diagnostic(
                "MNCS014",
                path,
                "value name and type must not be empty",
            ));
        } else if !names.insert(value.name.as_str()) {
            errors.push(diagnostic(
                "MNCS015",
                format!("{path}.name"),
                format!("duplicate value name {:?}", value.name),
            ));
        }
    }
}

fn collect_unique_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
    path: &str,
    code: &str,
    errors: &mut Vec<Diagnostic>,
) -> BTreeSet<&'a str> {
    let mut unique = BTreeSet::new();
    for (index, id) in ids.enumerate() {
        if id.trim().is_empty() {
            errors.push(diagnostic(
                code,
                format!("{path}[{index}].id"),
                "identifier must not be empty",
            ));
        } else if !unique.insert(id) {
            errors.push(diagnostic(
                code,
                format!("{path}[{index}].id"),
                format!("duplicate identifier {id:?}"),
            ));
        }
    }
    unique
}

fn diagnostic(
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        code: code.into(),
        path: path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        Assumption, AssumptionConfidence, ContractClause, ContractKind, Effect, EvidenceClaim,
        EvidenceStatus, FailureMode, Function, Program, Value,
    };

    pub(crate) fn valid_program() -> Program {
        Program {
            schema_version: SUPPORTED_SCHEMA_VERSION.to_owned(),
            module: "Banking.Transfer".to_owned(),
            assumptions: vec![Assumption {
                id: "storage_durability".to_owned(),
                statement: "committed writes survive process termination".to_owned(),
                supplied_by: "StorageBackend".to_owned(),
                confidence: AssumptionConfidence::External,
            }],
            functions: vec![Function {
                name: "transfer".to_owned(),
                inputs: vec![Value {
                    name: "amount".to_owned(),
                    value_type: "Money".to_owned(),
                }],
                outputs: vec![],
                contracts: vec![ContractClause {
                    id: "positive_amount".to_owned(),
                    kind: ContractKind::Requires,
                    expression: "amount > 0".to_owned(),
                }],
                effects: vec![Effect {
                    kind: "write".to_owned(),
                    target: "Account.balance".to_owned(),
                    capability: "AccountMutation".to_owned(),
                }],
                capabilities: vec!["AccountMutation".to_owned()],
                assumptions: vec!["storage_durability".to_owned()],
                evidence: vec![EvidenceClaim {
                    property: "positive_amount".to_owned(),
                    verifier: "RangeVerifier".to_owned(),
                    status: EvidenceStatus::Verified,
                    artifact: Some("sha256:example".to_owned()),
                }],
                failure: FailureMode::Atomic,
            }],
        }
    }

    #[test]
    fn accepts_consistent_program() {
        let report = valid_program().validate();
        assert!(report.valid, "{:#?}", report.errors);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn rejects_effect_without_declared_capability() {
        let mut program = valid_program();
        program.functions[0].capabilities.clear();
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNCS010"));
    }

    #[test]
    fn rejects_evidence_for_unknown_property() {
        let mut program = valid_program();
        "missing".clone_into(&mut program.functions[0].evidence[0].property);
        let report = program.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "MNCS012"));
    }

    #[test]
    fn warns_when_contract_lacks_evidence() {
        let mut program = valid_program();
        program.functions[0].evidence.clear();
        let report = program.validate();
        assert!(report.valid);
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.code == "MNCS101"));
    }
}
