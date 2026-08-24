use std::collections::BTreeMap;
use std::fmt::Write;

use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    Assumption, AssumptionConfidence, ContractClause, ContractKind, Effect, EvidenceClaim,
    EvidenceStatus, FailureMode, Function, Program, Value,
};

pub const CANONICAL_SCHEMA_VERSION: &str = "0.2";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CanonicalForm {
    pub schema_version: String,
    pub json: String,
    pub fingerprint: String,
}

#[derive(Debug, Error)]
pub enum CanonicalError {
    #[error("unable to serialize canonical semantic form: {0}")]
    Json(#[from] serde_json::Error),
}

impl Program {
    /// Build the deterministic machine representation of this semantic model.
    ///
    /// Object keys are lexicographically ordered by `serde_json`'s default map
    /// representation. Function parameters retain their declared order;
    /// declarations whose meaning is set-like are sorted by their semantic key.
    pub fn canonical_form(&self) -> Result<CanonicalForm, CanonicalError> {
        let json = serde_json::to_string(&canonical_program(self))?;
        let fingerprint = sha256_hex(json.as_bytes());
        Ok(CanonicalForm {
            schema_version: CANONICAL_SCHEMA_VERSION.to_owned(),
            json,
            fingerprint,
        })
    }

    pub fn canonical_json(&self) -> Result<String, CanonicalError> {
        Ok(self.canonical_form()?.json)
    }

    pub fn content_fingerprint(&self) -> Result<String, CanonicalError> {
        Ok(self.canonical_form()?.fingerprint)
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

pub(crate) fn canonical_json_value<T: serde::Serialize>(
    value: &T,
) -> Result<String, CanonicalError> {
    Ok(serde_json::to_string(value)?)
}

fn canonical_program(program: &Program) -> JsonValue {
    let mut fields = vec![
        (
            "assumptions",
            JsonValue::Array(sorted_assumptions(&program.assumptions)),
        ),
        (
            "functions",
            JsonValue::Array(sorted_functions(&program.functions)),
        ),
        ("module", JsonValue::String(program.module.clone())),
        (
            "schema_version",
            JsonValue::String(program.schema_version.clone()),
        ),
    ];
    // Preserve the established canonical fingerprints for pre-0.3 programs.
    // A finite declaration is material only when one is present.
    if !program.finite_types.is_empty() {
        fields.push((
            "finite_types",
            JsonValue::Array(sorted_finite_types(&program.finite_types)),
        ));
    }
    object(fields)
}

fn sorted_finite_types(types: &[crate::FiniteType]) -> Vec<JsonValue> {
    let mut types = types.to_vec();
    types.sort_by(|left, right| left.identity.cmp(&right.identity));
    types
        .into_iter()
        .map(|finite_type| serde_json::to_value(finite_type).expect("finite type is serializable"))
        .collect()
}

pub(crate) fn canonical_function(function: &Function) -> JsonValue {
    let mut result = object([
        ("assumptions", sorted_strings(&function.assumptions)),
        ("capabilities", sorted_strings(&function.capabilities)),
        (
            "contracts",
            JsonValue::Array(sorted_contracts(&function.contracts)),
        ),
        (
            "effects",
            JsonValue::Array(sorted_effects(&function.effects)),
        ),
        (
            "evidence",
            JsonValue::Array(sorted_evidence(&function.evidence)),
        ),
        (
            "failure",
            JsonValue::String(failure_name(&function.failure).to_owned()),
        ),
        (
            "inputs",
            JsonValue::Array(function.inputs.iter().map(canonical_value).collect()),
        ),
        ("name", JsonValue::String(function.name.clone())),
        (
            "outputs",
            JsonValue::Array(function.outputs.iter().map(canonical_value).collect()),
        ),
    ]);
    if let Some(body) = &function.body {
        if let JsonValue::Object(fields) = &mut result {
            fields.insert(
                "body".to_owned(),
                serde_json::from_str(&body.canonical_json().expect("canonical body"))
                    .expect("canonical body JSON"),
            );
        }
    }
    result
}

pub(crate) fn canonical_contract(contract: &ContractClause) -> JsonValue {
    object([
        ("expression", JsonValue::String(contract.expression.clone())),
        ("id", JsonValue::String(contract.id.clone())),
        (
            "kind",
            JsonValue::String(contract_kind_name(&contract.kind).to_owned()),
        ),
    ])
}

pub(crate) fn canonical_assumption(assumption: &Assumption) -> JsonValue {
    object([
        (
            "confidence",
            JsonValue::String(assumption_confidence_name(&assumption.confidence).to_owned()),
        ),
        ("id", JsonValue::String(assumption.id.clone())),
        ("statement", JsonValue::String(assumption.statement.clone())),
        (
            "supplied_by",
            JsonValue::String(assumption.supplied_by.clone()),
        ),
    ])
}

pub(crate) fn canonical_effect(effect: &Effect) -> JsonValue {
    object([
        ("capability", JsonValue::String(effect.capability.clone())),
        ("kind", JsonValue::String(effect.kind.clone())),
        ("target", JsonValue::String(effect.target.clone())),
    ])
}

pub(crate) fn canonical_evidence(evidence: &EvidenceClaim) -> JsonValue {
    object([
        (
            "artifact",
            evidence
                .artifact
                .as_ref()
                .map_or(JsonValue::Null, |value| JsonValue::String(value.clone())),
        ),
        ("property", JsonValue::String(evidence.property.clone())),
        (
            "status",
            JsonValue::String(evidence_status_name(&evidence.status).to_owned()),
        ),
        ("verifier", JsonValue::String(evidence.verifier.clone())),
    ])
}

fn canonical_value(value: &Value) -> JsonValue {
    object([
        ("name", JsonValue::String(value.name.clone())),
        ("type", JsonValue::String(value.value_type.clone())),
    ])
}

fn sorted_assumptions(values: &[Assumption]) -> Vec<JsonValue> {
    let mut values: Vec<_> = values.iter().map(canonical_assumption).collect();
    values.sort_by_key(|value| {
        value
            .get("id")
            .and_then(JsonValue::as_str)
            .unwrap_or("")
            .to_owned()
    });
    values
}

fn sorted_functions(values: &[Function]) -> Vec<JsonValue> {
    let mut values: Vec<_> = values.iter().map(canonical_function).collect();
    values.sort_by_key(|value| {
        value
            .get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or("")
            .to_owned()
    });
    values
}

fn sorted_contracts(values: &[ContractClause]) -> Vec<JsonValue> {
    let mut values: Vec<_> = values.iter().map(canonical_contract).collect();
    values.sort_by_key(|value| {
        value
            .get("id")
            .and_then(JsonValue::as_str)
            .unwrap_or("")
            .to_owned()
    });
    values
}

fn sorted_effects(values: &[Effect]) -> Vec<JsonValue> {
    let mut values: Vec<_> = values.iter().map(canonical_effect).collect();
    values.sort_by_key(|value| {
        (
            value
                .get("kind")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_owned(),
            value
                .get("target")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_owned(),
            value
                .get("capability")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_owned(),
        )
    });
    values
}

fn sorted_evidence(values: &[EvidenceClaim]) -> Vec<JsonValue> {
    let mut values: Vec<_> = values.iter().map(canonical_evidence).collect();
    values.sort_by_key(|value| {
        (
            value
                .get("property")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_owned(),
            value
                .get("verifier")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_owned(),
            value
                .get("status")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_owned(),
            value
                .get("artifact")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_owned(),
        )
    });
    values
}

fn sorted_strings(values: &[String]) -> JsonValue {
    let mut values = values.to_vec();
    values.sort();
    JsonValue::Array(values.into_iter().map(JsonValue::String).collect())
}

fn object(values: impl IntoIterator<Item = (&'static str, JsonValue)>) -> JsonValue {
    let values: BTreeMap<String, _> = values
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect();
    JsonValue::Object(values.into_iter().collect())
}

fn contract_kind_name(value: &ContractKind) -> &'static str {
    match value {
        ContractKind::Requires => "requires",
        ContractKind::Ensures => "ensures",
        ContractKind::Invariant => "invariant",
        ContractKind::Preserves => "preserves",
        ContractKind::Budget => "budget",
    }
}

fn assumption_confidence_name(value: &AssumptionConfidence) -> &'static str {
    match value {
        AssumptionConfidence::External => "external",
        AssumptionConfidence::Observed => "observed",
        AssumptionConfidence::Tested => "tested",
        AssumptionConfidence::Verified => "verified",
    }
}

fn evidence_status_name(value: &EvidenceStatus) -> &'static str {
    match value {
        EvidenceStatus::Claimed => "claimed",
        EvidenceStatus::Tested => "tested",
        EvidenceStatus::Analyzed => "analyzed",
        EvidenceStatus::Verified => "verified",
        EvidenceStatus::ExternallyVerified => "externally_verified",
    }
}

fn failure_name(value: &FailureMode) -> &'static str {
    match value {
        FailureMode::Isolated => "isolated",
        FailureMode::Atomic => "atomic",
        FailureMode::Partial => "partial",
        FailureMode::Compensating => "compensating",
        FailureMode::Fatal => "fatal",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContractClause, ContractKind, Function, Program};

    fn program() -> Program {
        Program {
            schema_version: "0.1".to_owned(),
            module: "demo".to_owned(),
            finite_types: vec![],
            record_types: Vec::new(),
            assumptions: vec![],
            functions: vec![Function {
                name: "f".to_owned(),
                inputs: vec![],
                outputs: vec![],
                contracts: vec![ContractClause {
                    id: "p".to_owned(),
                    kind: ContractKind::Requires,
                    expression: "true".to_owned(),
                }],
                effects: vec![],
                capabilities: vec![],
                assumptions: vec![],
                evidence: vec![],
                failure: FailureMode::default(),
                body: None,
            }],
        }
    }

    #[test]
    fn canonical_form_is_compact_and_hashed_deterministically() {
        let form = program().canonical_form().expect("canonical form");
        assert_eq!(form.schema_version, "0.2");
        assert_eq!(form.json, program().canonical_json().unwrap());
        assert_eq!(form.fingerprint.len(), 64);
        assert!(form.json.starts_with('{'));
    }

    #[test]
    fn set_like_declaration_order_is_non_semantic_but_parameter_order_is_semantic() {
        let mut left = program();
        left.assumptions = vec![
            crate::Assumption {
                id: "b".to_owned(),
                statement: "b".to_owned(),
                supplied_by: "test".to_owned(),
                confidence: crate::AssumptionConfidence::External,
            },
            crate::Assumption {
                id: "a".to_owned(),
                statement: "a".to_owned(),
                supplied_by: "test".to_owned(),
                confidence: crate::AssumptionConfidence::External,
            },
        ];
        left.functions[0].capabilities = vec!["B".to_owned(), "A".to_owned()];
        left.functions[0].assumptions = vec!["b".to_owned(), "a".to_owned()];
        let mut right = left.clone();
        right.assumptions.reverse();
        right.functions[0].capabilities.reverse();
        right.functions[0].assumptions.reverse();
        assert_eq!(
            left.canonical_json().unwrap(),
            right.canonical_json().unwrap()
        );

        let mut changed_parameter_order = left.clone();
        changed_parameter_order.functions[0].inputs = vec![
            crate::Value {
                name: "first".to_owned(),
                value_type: "u8".to_owned(),
            },
            crate::Value {
                name: "second".to_owned(),
                value_type: "u8".to_owned(),
            },
        ];
        let mut reversed_parameters = changed_parameter_order.clone();
        reversed_parameters.functions[0].inputs.reverse();
        assert_ne!(
            changed_parameter_order.canonical_json().unwrap(),
            reversed_parameters.canonical_json().unwrap()
        );
    }

    #[test]
    fn semantic_change_changes_canonical_fingerprint() {
        let before = program();
        let mut after = before.clone();
        "false".clone_into(&mut after.functions[0].contracts[0].expression);
        assert_ne!(
            before.content_fingerprint().unwrap(),
            after.content_fingerprint().unwrap()
        );
    }
}
