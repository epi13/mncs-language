//! Provably terminating structural recursion over finite values (RFC 0047).
//!
//! A direct self-call is admitted only when every recursive call consumes a
//! proper structural subvalue of the function's first parameter, which must
//! have finite (inductive enum) type. The frontend enforces the rule over
//! resolved bindings; this module owns the evidence schema and the
//! independent re-derivation that the obligation layer runs over
//! already-trusted body and binding-table facts. A frontend claim of
//! "decreases" with no verifiable chain fails closed here, exactly like a
//! corrupted or sabotaged record.
//!
//! The re-derivation checks, in order: a non-empty link chain; a concrete
//! (non-generic) function whose first body parameter has finite type;
//! chain-root anchoring at that parameter's value; per-link projection
//! operations with operand/result continuity; argument flow from the last
//! link into the call's recursive-parameter position with the callee being
//! the function itself; finiteness of the shared projection type against
//! the program's declared finite types; and binding-table linkage showing
//! each child was projected from its claimed parent (this is what defeats
//! `let`-alias and shadowing forgeries, which are value-identical to
//! honest chains after elaboration).

use serde::{Deserialize, Serialize};

use crate::body::{BodyOperationKind, BodyType, SequenceBound};
use crate::{Program, SemanticBindingKind, SemanticId};

/// Absolute upper bound on live call depth for one execution. Structural
/// recursion is statically terminating, but native-stack recursion still
/// needs deterministic fuel accounting: every nested call consumes one
/// unit, and exhaustion is a deterministic runtime failure, never
/// unbounded host stacking. Requests may name a smaller budget; nothing
/// may name a larger one.
pub const MODEL_MAX_CALL_DEPTH: u64 = 1024;

/// Evidence property naming an admitted structural-decrease call site.
pub const STRUCTURAL_DECREASE_PROPERTY: &str = "structural-decrease";

/// One match-projection link in a structural-decrease chain: the child
/// value was projected from the parent value by the named operation, and
/// the child binding was resolved from the parent binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralDecreaseLink {
    pub op: String,
    pub parent_binding: SemanticId,
    pub child_binding: SemanticId,
    pub parent_value: String,
    pub child_value: String,
}

/// The admitted-call record: which call site, of which function, with the
/// full projection chain from the recursive parameter to the argument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralDecreaseClaim {
    pub call_op: String,
    pub function: SemanticId,
    pub param_binding: SemanticId,
    pub param_value: String,
    pub arg_binding: SemanticId,
    pub links: Vec<StructuralDecreaseLink>,
}

/// Decode one structural-decrease evidence artifact. Anything
/// unparseable fails closed at the obligation layer, never silently.
pub fn parse_structural_decrease_claim(artifact: &str) -> Result<StructuralDecreaseClaim, String> {
    serde_json::from_str(artifact)
        .map_err(|error| format!("structural-decrease artifact does not decode: {error}"))
}

/// Re-derive an admitted structural decrease from body and binding-table
/// facts. `Ok(())` means the claimed chain exists exactly as recorded;
/// any `Err` names the first broken link. The frontend's binding-level
/// admission is never trusted here: every structural fact is rechecked.
pub fn verify_structural_decrease(
    program: &Program,
    claim: &StructuralDecreaseClaim,
) -> Result<(), String> {
    if claim.links.is_empty() {
        return Err("structural-decrease chain is empty".to_owned());
    }
    let function = program
        .functions
        .iter()
        .find(|candidate| {
            crate::function_id(
                candidate.identity_namespace(&program.module),
                &candidate.name,
            ) == claim.function
        })
        .ok_or_else(|| "structural-decrease names an unknown function".to_owned())?;
    if !function.generic_params.is_empty() {
        return Err("structural-decrease over a generic template never verifies".to_owned());
    }
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| "structural-decrease names a bodyless function".to_owned())?;
    // R1, re-derived: the first body parameter carries finite type, and the
    // claimed parameter value is that parameter.
    let first_param = body
        .parameters
        .first()
        .ok_or_else(|| "structural-decrease function has no parameters".to_owned())?;
    let BodyType::Finite { .. } = &first_param.ty else {
        return Err("structural-decrease recursive parameter is not finite".to_owned());
    };
    if first_param.id != claim.param_value {
        return Err("structural-decrease parameter value is not the first parameter".to_owned());
    }
    // The shared projection type must be a declared finite type of this
    // program; every link projects the same subject type.
    let mut projection_type: Option<SemanticId> = None;
    for link in &claim.links {
        let operation = find_operation(body, &link.op)?;
        let BodyOperationKind::FinitePayloadProject {
            type_identity,
            variant_identity,
            field,
            ..
        } = &operation.kind
        else {
            return Err(format!(
                "structural-decrease link {} is not a payload projection",
                link.op
            ));
        };
        if operation.operands != vec![link.parent_value.clone()] {
            return Err(format!(
                "structural-decrease link {} does not project its claimed parent value",
                link.op
            ));
        }
        let result_id = operation
            .results
            .first()
            .map(|result| result.id.as_str())
            .unwrap_or("");
        if result_id != link.child_value {
            return Err(format!(
                "structural-decrease link {} does not produce its claimed child value",
                link.op
            ));
        }
        match &projection_type {
            None => projection_type = Some(type_identity.clone()),
            Some(identity) if identity == type_identity => {}
            Some(_) => {
                return Err("structural-decrease links project different types".to_owned());
            }
        }
        let finite = program
            .finite_types
            .iter()
            .find(|candidate| &candidate.identity == type_identity)
            .ok_or_else(|| "structural-decrease projects an undeclared finite type".to_owned())?;
        let variant = finite
            .variants
            .iter()
            .find(|candidate| &candidate.identity == variant_identity)
            .ok_or_else(|| "structural-decrease projects an undeclared variant".to_owned())?;
        if !variant.payload.iter().any(|payload| payload.name == *field) {
            return Err("structural-decrease projects an undeclared payload field".to_owned());
        }
    }
    // Value continuity: each link's child feeds the next link's parent, and
    // the chain roots at the recursive parameter's value.
    if claim.links[0].parent_value != claim.param_value {
        return Err(
            "structural-decrease chain does not root at the recursive parameter".to_owned(),
        );
    }
    for pair in claim.links.windows(2) {
        if pair[0].child_value != pair[1].parent_value {
            return Err("structural-decrease chain is discontinuous".to_owned());
        }
    }
    // R4 plus argument flow, re-derived: the call site targets this very
    // function and carries the last descendant in first-operand position.
    let call = find_operation(body, &claim.call_op)?;
    let BodyOperationKind::Call {
        function: callee, ..
    } = &call.kind
    else {
        return Err("structural-decrease call site is not a call operation".to_owned());
    };
    if *callee != claim.function {
        return Err("structural-decrease call site targets another function".to_owned());
    }
    let first_operand = call.operands.first().map(String::as_str).unwrap_or("");
    let last_child = claim
        .links
        .last()
        .map(|link| link.child_value.as_str())
        .unwrap_or("");
    if first_operand != last_child {
        return Err(
            "structural-decrease argument does not flow into recursive position".to_owned(),
        );
    }
    // Binding linkage, re-derived from the binding table: each child was
    // projected from its claimed parent. Value-identical forgeries (a
    // `let` alias or a shadowed rebinding over a descendant spelling
    // shares the honest value graph exactly after elaboration) carry no
    // such linkage and fail here.
    let table = program
        .binding_table
        .as_ref()
        .ok_or_else(|| "structural-decrease needs a binding table".to_owned())?;
    let bindings: std::collections::BTreeMap<&SemanticId, &crate::SemanticBinding> = table
        .bindings
        .iter()
        .map(|binding| (&binding.identity, binding))
        .collect();
    let param = bindings
        .get(&claim.param_binding)
        .ok_or_else(|| "structural-decrease parameter binding is not declared".to_owned())?;
    if param.kind != SemanticBindingKind::Parameter {
        return Err("structural-decrease parameter binding is not a parameter".to_owned());
    }
    if claim.arg_binding
        != claim
            .links
            .last()
            .map(|link| link.child_binding.clone())
            .unwrap_or_else(|| claim.param_binding.clone())
    {
        return Err("structural-decrease argument binding is not the last descendant".to_owned());
    }
    let mut expected_parent = &claim.param_binding;
    for link in &claim.links {
        if &link.parent_binding != expected_parent {
            return Err("structural-decrease binding chain is discontinuous".to_owned());
        }
        let child = bindings.get(&link.child_binding).ok_or_else(|| {
            format!(
                "structural-decrease child binding {} is not declared",
                link.child_binding.0
            )
        })?;
        if child.kind != SemanticBindingKind::Local {
            return Err("structural-decrease child is not a match-payload binding".to_owned());
        }
        if child.projected_from.as_ref() != Some(&link.parent_binding) {
            return Err(format!(
                "structural-decrease child binding {} was not projected from its claimed parent",
                link.child_binding.0
            ));
        }
        expected_parent = &link.child_binding;
    }
    Ok(())
}

fn find_operation(body: &crate::FunctionBody, op: &str) -> Result<crate::BodyOperation, String> {
    body.blocks
        .iter()
        .flat_map(|block| block.operations.iter())
        .find(|operation| operation.id == op)
        .cloned()
        .ok_or_else(|| format!("structural-decrease names an unknown operation {op}"))
}

/// Concrete traversal bounds substituted into one specialized function
/// that exceed `admitted`. Shared by the frontend's post-specialization
/// sweep so the definition/instantiation contract has one implementation.
pub fn specialized_bounds_over_ceiling(body: &crate::FunctionBody, admitted: u32) -> Vec<String> {
    fn over(bound: &SequenceBound, admitted: u32) -> bool {
        match bound {
            SequenceBound::Exact(length) | SequenceBound::UpTo(length) => *length > admitted,
            SequenceBound::Param(_) | SequenceBound::UpToParam(_) => false,
        }
    }
    let mut offenders = Vec::new();
    for block in &body.blocks {
        for operation in &block.operations {
            let bad = match &operation.kind {
                BodyOperationKind::SequenceLength { bound }
                | BodyOperationKind::SequenceProject { bound, .. }
                | BodyOperationKind::SequenceReplace { bound, .. } => over(bound, admitted),
                BodyOperationKind::ViewConstruct {
                    source_bound,
                    view_bound,
                } => over(source_bound, admitted) || over(view_bound, admitted),
                _ => false,
            };
            if bad {
                offenders.push(operation.id.clone());
            }
        }
    }
    for iteration in &body.bounded_iterations {
        if iteration
            .sequence_bound
            .as_ref()
            .is_some_and(|bound| over(bound, admitted))
        {
            offenders.push(iteration.id.clone());
        }
    }
    offenders
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_program(functions: serde_json::Value) -> Program {
        serde_json::from_value(json!({
            "schema_version": "0.2",
            "module": "test.rec",
            "functions": functions,
        }))
        .expect("test program deserializes")
    }

    fn claim(function: &str, param_value: &str) -> StructuralDecreaseClaim {
        StructuralDecreaseClaim {
            call_op: "call".to_owned(),
            function: crate::function_id("test.rec", function),
            param_binding: crate::SemanticId("b-param".to_owned()),
            param_value: param_value.to_owned(),
            arg_binding: crate::SemanticId("b-arg".to_owned()),
            links: vec![StructuralDecreaseLink {
                op: "project".to_owned(),
                parent_binding: crate::SemanticId("b-param".to_owned()),
                child_binding: crate::SemanticId("b-arg".to_owned()),
                parent_value: "v-parent".to_owned(),
                child_value: "v-child".to_owned(),
            }],
        }
    }

    #[test]
    fn empty_chain_fails_closed() {
        let program = test_program(json!([]));
        let mut empty = claim("f", "p0");
        empty.links.clear();
        assert!(verify_structural_decrease(&program, &empty).is_err());
    }

    #[test]
    fn unknown_function_fails_closed() {
        let program = test_program(json!([]));
        assert!(verify_structural_decrease(&program, &claim("missing", "p0")).is_err());
    }

    #[test]
    fn generic_template_never_verifies() {
        let program = test_program(
            json!([{ "name": "f", "generic_params": [{ "name": "N", "kind": "nat" }] }]),
        );
        assert!(verify_structural_decrease(&program, &claim("f", "p0")).is_err());
    }

    #[test]
    fn bodyless_function_fails_closed() {
        let program = test_program(json!([{ "name": "f" }]));
        assert!(verify_structural_decrease(&program, &claim("f", "p0")).is_err());
    }

    #[test]
    fn non_finite_first_parameter_fails_closed() {
        let program = test_program(json!([{
            "name": "f",
            "body": {
                "schema_version": "0.4",
                "entry": "b0",
                "parameters": [{ "id": "p0", "name": "n", "type": { "integer": { "bits": 64, "signed": true } } }],
                "blocks": []
            }
        }]));
        assert!(verify_structural_decrease(&program, &claim("f", "p0")).is_err());
    }

    #[test]
    fn param_value_mismatch_fails_closed() {
        let program = test_program(json!([{
            "name": "f",
            "body": {
                "schema_version": "0.4",
                "entry": "b0",
                "parameters": [{ "id": "p0", "name": "t", "type": { "finite": { "identity": "mncs:0.2:finite-type:test.rec::Tree", "name": "Tree" } } }],
                "blocks": []
            }
        }]));
        assert!(verify_structural_decrease(&program, &claim("f", "other")).is_err());
    }
}
