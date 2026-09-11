//! Language-owned aggregate field-identity rule (WEB-P-011).
//!
//! MNCS records (and payload-bearing finite variants) are nominal and
//! name-matched: the meaning of a value comes from the association of field
//! names to values, never from the order fields happen to be written in an
//! external corpus. The canonical (name-sorted) field order is the internal
//! normalization only; it is not part of the external value contract.
//!
//! Every execution boundary therefore follows one rule:
//!
//! 1. resolve external fields **by name** against the canonical declaration;
//! 2. reject malformed values explicitly (duplicate, unknown, or missing
//!    fields) with a diagnostic that names the expected and received field
//!    lists;
//! 3. normalize accepted values into canonical order before lowering.
//!
//! A malformed record value is either interpreted correctly by semantic
//! field identity or rejected. It is never silently bound one field's value
//! to another field.

use std::collections::BTreeSet;

use crate::{BodyType, ExecutionValue, Program};

/// How one external field list disagrees with its canonical declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldOrderProblem {
    /// A field name appears more than once in the received value. Checked
    /// first: duplicates are the shape most likely to hide a silent
    /// misbinding, so they always win over unknown/missing reports.
    DuplicateField,
    /// A received field name is not declared by the type.
    UnknownField,
    /// A declared field name has no received value.
    MissingField,
}

/// One field-identity disagreement, carrying both sides for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldOrderError {
    pub problem: FieldOrderProblem,
    pub field: String,
    /// Human context, e.g. `record "Box"` or `finite payload "Opt::Some"`.
    pub context: String,
    /// Canonical declaration order.
    pub expected: Vec<String>,
    /// As-written received order.
    pub received: Vec<String>,
}

impl FieldOrderError {
    /// Stable machine-readable diagnostic: what failed, where, and the
    /// expected-vs-actual field lists. Callers embed this in the
    /// backend-owned failure reason alongside argument identity.
    pub fn describe(&self) -> String {
        let problem = match self.problem {
            FieldOrderProblem::DuplicateField => "duplicate field",
            FieldOrderProblem::UnknownField => "unknown field",
            FieldOrderProblem::MissingField => "missing field",
        };
        format!(
            "MNCS_VALUE_CONTRACT {}: {} {:?}; expected fields [{}], received fields [{}]",
            self.context,
            problem,
            self.field,
            self.expected.join(", "),
            self.received.join(", ")
        )
    }
}

/// Resolve as-written `(name, value)` field pairs against canonical declared
/// names. Returns the indices into `received` in declared (canonical) order,
/// so callers can bind or normalize without cloning through this layer.
///
/// Duplicate received names are reported before unknown names, and unknown
/// names before missing ones, so the most misbinding-prone shape wins.
pub fn order_fields_by_name<V>(
    context: &str,
    declared: &[String],
    received: &[(String, V)],
) -> Result<Vec<usize>, FieldOrderError> {
    let expected: Vec<String> = declared.to_vec();
    let received_names: Vec<String> = received.iter().map(|(name, _)| name.clone()).collect();
    let error = |problem: FieldOrderProblem, field: String| FieldOrderError {
        problem,
        field,
        context: context.to_owned(),
        expected: expected.clone(),
        received: received_names.clone(),
    };
    let mut seen = BTreeSet::new();
    for name in received.iter().map(|(name, _)| name) {
        if !seen.insert(name) {
            return Err(error(FieldOrderProblem::DuplicateField, name.clone()));
        }
    }
    for name in received.iter().map(|(name, _)| name) {
        if !declared.iter().any(|field| field == name) {
            return Err(error(FieldOrderProblem::UnknownField, name.clone()));
        }
    }
    let mut order = Vec::with_capacity(declared.len());
    for name in declared {
        match received.iter().position(|(field, _)| field == name) {
            Some(index) => order.push(index),
            None => return Err(error(FieldOrderProblem::MissingField, name.clone())),
        }
    }
    Ok(order)
}

/// Deepest-first field-identity detail for a rejected aggregate argument:
/// resolves the top-level fields by name, then recurses into nested
/// records, finite payloads, sequences, views, vectors, and masks, so the
/// reported disagreement names the exact field path with expected-vs-
/// received lists. Returns `None` when every resolvable shape agrees (the
/// rejection then comes from a nested scalar) or when a declaration is
/// unknown. Both interpreters share this so their diagnostics agree.
pub fn first_aggregate_mismatch(
    program: &Program,
    value: &ExecutionValue,
    ty: &BodyType,
    path: &str,
) -> Option<String> {
    match (value, ty) {
        (
            ExecutionValue::Record {
                type_identity,
                fields,
                ..
            },
            BodyType::Record { .. },
        ) => {
            let declaration = program
                .record_types
                .iter()
                .find(|decl| &decl.identity == type_identity)?;
            let declared_names: Vec<String> = declaration
                .fields
                .iter()
                .map(|field| field.name.clone())
                .collect();
            let context = format!("record {:?} at {path}", declaration.name);
            match order_fields_by_name(&context, &declared_names, fields) {
                Err(error) => Some(error.describe()),
                Ok(order) => {
                    for (declared_index, received_index) in order.iter().enumerate() {
                        let declared = &declaration.fields[declared_index];
                        let field_ty = BodyType::from_program(program, &declared.field_type);
                        let field_path = format!("{path}.{}", declared.name);
                        if let Some(note) = first_aggregate_mismatch(
                            program,
                            &fields[*received_index].1,
                            &field_ty,
                            &field_path,
                        ) {
                            return Some(note);
                        }
                    }
                    None
                }
            }
        }
        (
            ExecutionValue::Finite {
                type_identity,
                variant_identity,
                discriminant,
                payload,
                ..
            },
            BodyType::Finite { .. },
        ) => {
            if !crate::execution::valid_finite_value(
                program,
                type_identity,
                variant_identity,
                *discriminant,
            ) {
                return Some(format!(
                    "MNCS_VALUE_CONTRACT {path}: finite identity mismatch: \
                     undeclared variant for {type_identity:?} (discriminant {discriminant})"
                ));
            }
            let variant = program
                .finite_types
                .iter()
                .find(|decl| &decl.identity == type_identity)
                .and_then(|finite_type| {
                    finite_type
                        .variants
                        .iter()
                        .find(|variant| &variant.identity == variant_identity)
                })?;
            let declared_names: Vec<String> = variant
                .payload
                .iter()
                .map(|field| field.name.clone())
                .collect();
            let context = format!("finite payload {variant_identity:?} at {path}");
            match order_fields_by_name(&context, &declared_names, payload) {
                Err(error) => Some(error.describe()),
                Ok(order) => {
                    for (declared_index, received_index) in order.iter().enumerate() {
                        let declared = &variant.payload[declared_index];
                        let field_ty = BodyType::from_program(program, &declared.field_type);
                        let field_path = format!("{path}.{}", declared.name);
                        if let Some(note) = first_aggregate_mismatch(
                            program,
                            &payload[*received_index].1,
                            &field_ty,
                            &field_path,
                        ) {
                            return Some(note);
                        }
                    }
                    None
                }
            }
        }
        (
            ExecutionValue::Sequence { values },
            BodyType::Sequence {
                element,
                bound: crate::SequenceBound::Exact(length),
            },
        ) => {
            if values.len() != *length as usize {
                return Some(format!(
                    "MNCS_VALUE_CONTRACT {path}: sequence length mismatch: expected {length} element(s), received {}",
                    values.len()
                ));
            }
            values
                .iter()
                .enumerate()
                .find_map(|(index, element_value)| {
                    first_aggregate_mismatch(
                        program,
                        element_value,
                        element,
                        &format!("{path}[{index}]"),
                    )
                })
        }
        (
            ExecutionValue::Sequence { values },
            BodyType::Sequence {
                element,
                bound: crate::SequenceBound::UpTo(capacity),
            },
        ) => {
            if values.len() > *capacity as usize {
                return Some(format!(
                    "MNCS_VALUE_CONTRACT {path}: view length exceeds capacity: capacity {capacity}, received {}",
                    values.len()
                ));
            }
            values
                .iter()
                .enumerate()
                .find_map(|(index, element_value)| {
                    first_aggregate_mismatch(
                        program,
                        element_value,
                        element,
                        &format!("{path}[{index}]"),
                    )
                })
        }
        // Spellings reconstructed without full type information can leave a
        // nominal value behind a bare name; resolve by record/finite short
        // name so nested diagnostics still fire.
        (ExecutionValue::Record { name, .. }, BodyType::Named(named)) if name == named => {
            let identity = program
                .record_types
                .iter()
                .find(|decl| &decl.name == named)
                .map(|decl| decl.identity.clone())?;
            first_aggregate_mismatch(
                program,
                value,
                &BodyType::Record {
                    identity,
                    name: named.clone(),
                },
                path,
            )
        }
        // Scalar leaves: describe the domain disagreement with the same
        // stable token so every value-contract rejection is machine
        // actionable, not just the aggregate-shape ones.
        (ExecutionValue::Integer { value, ty }, BodyType::Integer(expected)) => {
            if ty == expected && integer_in_domain(*value, *expected) {
                None
            } else {
                Some(format!(
                    "MNCS_VALUE_CONTRACT {path}: scalar mismatch: expected {}{}, received integer {value}",
                    if expected.signed { "i" } else { "u" },
                    expected.bits
                ))
            }
        }
        (ExecutionValue::Boolean { .. }, BodyType::Named(name)) if name == "bool" => None,
        // The interpreters admit booleans for 1-bit unsigned integers.
        (ExecutionValue::Boolean { .. }, BodyType::Integer(integer))
            if integer.bits == 1 && !integer.signed =>
        {
            None
        }
        (ExecutionValue::Byte { value }, BodyType::Byte) => {
            if (0..=255).contains(value) {
                None
            } else {
                Some(format!(
                    "MNCS_VALUE_CONTRACT {path}: scalar mismatch: expected byte, received byte {value}"
                ))
            }
        }
        (ExecutionValue::Float { ty, .. }, BodyType::Float(expected)) => {
            if ty == expected {
                None
            } else {
                Some(format!(
                    "MNCS_VALUE_CONTRACT {path}: scalar mismatch: expected f{}, received f{}",
                    expected.bits, ty.bits
                ))
            }
        }
        (ExecutionValue::Vector { values }, BodyType::Vector { element, lanes }) => {
            if values.len() != *lanes as usize {
                return Some(format!(
                    "MNCS_VALUE_CONTRACT {path}: vector lane mismatch: expected {lanes} lane(s), received {}",
                    values.len()
                ));
            }
            values.iter().enumerate().find_map(|(index, lane)| {
                first_aggregate_mismatch(program, lane, element, &format!("{path}[{index}]"))
            })
        }
        (ExecutionValue::Mask { lanes: bits }, BodyType::Mask { lanes }) => {
            if bits.len() == *lanes as usize {
                None
            } else {
                Some(format!(
                    "MNCS_VALUE_CONTRACT {path}: mask lane mismatch: expected {lanes} lane(s), received {}",
                    bits.len()
                ))
            }
        }
        // Any other pairing disagrees by kind. This arm only fires for
        // values the entry validation already rejected; it guarantees the
        // rejection carries the stable token with both shapes spelled out.
        (_, _) => Some(format!(
            "MNCS_VALUE_CONTRACT {path}: value shape mismatch: expected {}, received {}",
            ty.semantic_name(),
            leaf_shape(value)
        )),
    }
}

/// One-line received-shape summary for mismatch notes.
fn leaf_shape(value: &ExecutionValue) -> String {
    match value {
        ExecutionValue::Integer { value, ty } => {
            format!(
                "integer {value} ({}{})",
                if ty.signed { "i" } else { "u" },
                ty.bits
            )
        }
        ExecutionValue::Float { ty, .. } => format!("float f{}", ty.bits),
        ExecutionValue::Boolean { value } => format!("boolean {value}"),
        ExecutionValue::Byte { value } => format!("byte {value}"),
        ExecutionValue::Finite {
            type_identity,
            discriminant,
            ..
        } => format!("finite {type_identity:?} discriminant {discriminant}"),
        ExecutionValue::Record {
            type_identity,
            fields,
            ..
        } => {
            let names: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();
            format!("record {type_identity:?} fields [{}]", names.join(", "))
        }
        ExecutionValue::Sequence { values } => {
            format!("sequence ({} element(s))", values.len())
        }
        ExecutionValue::Vector { values } => format!("vector ({} lane(s))", values.len()),
        ExecutionValue::Mask { lanes } => format!("mask ({} lane(s))", lanes.len()),
    }
}

/// Integer domain check mirroring the interpreters' range rule.
fn integer_in_domain(value: i128, ty: crate::IntegerType) -> bool {
    if !(1..=126).contains(&ty.bits) {
        return false;
    }
    if ty.signed {
        let Some(top) = 1_i128.checked_shl(u32::from(ty.bits - 1)) else {
            return false;
        };
        (-top..top).contains(&value)
    } else {
        let Some(top) = 1_i128
            .checked_shl(u32::from(ty.bits))
            .and_then(|top| top.checked_sub(1))
        else {
            return false;
        };
        (0..=top).contains(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared() -> Vec<String> {
        ["apple", "mango", "zebra"]
            .iter()
            .map(|name| name.to_string())
            .collect()
    }

    fn received(names: &[&str]) -> Vec<(String, u64)> {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| (name.to_string(), index as u64))
            .collect()
    }

    #[test]
    fn canonical_order_resolves_to_identity_permutation() {
        let order = order_fields_by_name(
            "record \"R\"",
            &declared(),
            &received(&["apple", "mango", "zebra"]),
        )
        .expect("canonical order resolves");
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn declaration_order_resolves_by_name() {
        let values = received(&["zebra", "apple", "mango"]);
        let order =
            order_fields_by_name("record \"R\"", &declared(), &values).expect("any order resolves");
        let resolved: Vec<u64> = order.iter().map(|index| values[*index].1).collect();
        // zebra=0, apple=1, mango=2 rebound into canonical slots.
        assert_eq!(resolved, vec![1, 2, 0]);
    }

    #[test]
    fn duplicate_field_wins_over_other_problems() {
        let error = order_fields_by_name(
            "record \"R\"",
            &declared(),
            &received(&["apple", "apple", "bogus"]),
        )
        .expect_err("duplicates rejected");
        assert_eq!(error.problem, FieldOrderProblem::DuplicateField);
        assert_eq!(error.field, "apple");
    }

    #[test]
    fn unknown_field_names_expected_and_received() {
        let error = order_fields_by_name(
            "record \"R\"",
            &declared(),
            &received(&["apple", "mango", "bogus"]),
        )
        .expect_err("unknown rejected");
        assert_eq!(error.problem, FieldOrderProblem::UnknownField);
        assert_eq!(error.field, "bogus");
        let text = error.describe();
        assert!(
            text.contains("expected fields [apple, mango, zebra]"),
            "{text}"
        );
        assert!(
            text.contains("received fields [apple, mango, bogus]"),
            "{text}"
        );
    }

    #[test]
    fn missing_field_reports_the_absent_name() {
        let error =
            order_fields_by_name("record \"R\"", &declared(), &received(&["apple", "mango"]))
                .expect_err("missing rejected");
        assert_eq!(error.problem, FieldOrderProblem::MissingField);
        assert_eq!(error.field, "zebra");
    }
}
