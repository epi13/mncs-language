//! Shared adapter helpers that must stay backend-neutral.

use std::collections::BTreeMap;
use std::fmt::Write;

use mncs_model::{
    ArtifactRepresentation, BackendArtifact, BackendFunctionValueContract, BackendIdentity,
    BackendResult, BackendValueContract, CompilerArtifactRef, CompilerDiagnostic,
    CompilerDiagnosticKind, ExecutionFailure, ExecutionRequest, ExecutionStatus, ExecutionValue,
    IntegerType, Program, SsaModule, TransformationStatus, BACKEND_ARTIFACT_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};

use crate::BackendExecutionResult;

pub(crate) fn failed(diagnostics: Vec<CompilerDiagnostic>) -> BackendResult {
    BackendResult {
        status: TransformationStatus::Fail,
        artifact: None,
        artifact_ref: None,
        evidence: None,
        diagnostics,
    }
}

pub(crate) fn unknown(diagnostics: Vec<CompilerDiagnostic>) -> BackendResult {
    BackendResult {
        status: TransformationStatus::Unknown,
        artifact: None,
        artifact_ref: None,
        evidence: None,
        diagnostics,
    }
}

pub(crate) fn validate_selected_ssa(
    ssa: &SsaModule,
    selected_ssa: &CompilerArtifactRef,
    code: &str,
) -> Result<(), Box<BackendResult>> {
    if selected_ssa.representation != ArtifactRepresentation::SelectedSsa
        || !selected_ssa.identity_is_valid()
    {
        return Err(Box::new(failed(vec![CompilerDiagnostic::new(
            code,
            CompilerDiagnosticKind::InvalidRequest,
            "backend lowering requires an exact selected SSA identity",
        )])));
    }
    let expected = ssa.fingerprint().unwrap_or_default();
    if selected_ssa.fingerprint != expected {
        return Err(Box::new(failed(vec![CompilerDiagnostic::new(
            code,
            CompilerDiagnosticKind::InvalidRequest,
            "selected SSA fingerprint does not match the supplied SSA module",
        )])));
    }
    Ok(())
}

pub(crate) fn function_names(program: &Program, ssa: &SsaModule) -> Vec<String> {
    ssa.functions
        .iter()
        .map(|ssa_function| {
            program
                .functions
                .iter()
                .find(|function| {
                    mncs_model::function_id(&program.module, &function.name)
                        == ssa_function.semantic_identity
                })
                .map(|function| function.name.clone())
                .unwrap_or_else(|| export_name(&ssa_function.semantic_identity.0))
        })
        .collect()
}

pub(crate) fn export_name(identity: &str) -> String {
    identity
        .rsplit(':')
        .next()
        .unwrap_or(identity)
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// Logical composite types (records and payload-bearing finite variants) used
/// by this program's signatures, keyed by semantic type name. Attached to
/// artifacts so execution can marshal composite values without the program.
pub(crate) fn composite_value_contracts(
    program: &Program,
) -> BTreeMap<String, BackendValueContract> {
    let mut composites = BTreeMap::new();
    for record in &program.record_types {
        composites.insert(
            record.name.clone(),
            BackendValueContract::Record {
                type_identity: record.identity.clone(),
                name: record.name.clone(),
                fields: record
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), field.field_type.clone()))
                    .collect(),
            },
        );
    }
    for finite in &program.finite_types {
        // Include every declared finite type: payload-free ones are needed as
        // field-type references inside other composites.
        let payloads: BTreeMap<u32, Vec<(String, String)>> = finite
            .variants
            .iter()
            .map(|variant| {
                (
                    variant.discriminant,
                    variant
                        .payload
                        .iter()
                        .map(|field| (field.name.clone(), field.field_type.clone()))
                        .collect(),
                )
            })
            .collect();
        composites.insert(
            finite.name.clone(),
            BackendValueContract::Finite {
                type_identity: finite.identity.clone(),
                variants: finite
                    .variants
                    .iter()
                    .map(|variant| (variant.discriminant, variant.identity.clone()))
                    .collect(),
                payloads,
            },
        );
    }
    composites
}

pub(crate) fn function_value_contracts(
    program: &Program,
) -> BTreeMap<String, BackendFunctionValueContract> {
    let value_contract = |name: &str| {
        if let Some(record_type) = program
            .record_types
            .iter()
            .find(|record| record.name == name)
        {
            return BackendValueContract::Record {
                type_identity: record_type.identity.clone(),
                name: record_type.name.clone(),
                fields: record_type
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), field.field_type.clone()))
                    .collect(),
            };
        }
        program
            .finite_types
            .iter()
            .find(|finite_type| finite_type.name == name)
            .map_or_else(
                || BackendValueContract::Scalar {
                    semantic_type: name.to_owned(),
                },
                |finite_type| {
                    // A type is boxed when ANY variant carries a payload;
                    // every variant then gets a layout entry (maybe empty).
                    let payloads = finite_type
                        .variants
                        .iter()
                        .filter(|_| {
                            finite_type
                                .variants
                                .iter()
                                .any(|variant| !variant.payload.is_empty())
                        })
                        .map(|variant| {
                            (
                                variant.discriminant,
                                variant
                                    .payload
                                    .iter()
                                    .map(|field| (field.name.clone(), field.field_type.clone()))
                                    .collect::<Vec<_>>(),
                            )
                        })
                        .collect();
                    BackendValueContract::Finite {
                        type_identity: finite_type.identity.clone(),
                        variants: finite_type
                            .variants
                            .iter()
                            .map(|variant| (variant.discriminant, variant.identity.clone()))
                            .collect(),
                        payloads,
                    }
                },
            )
    };
    program
        .functions
        .iter()
        .map(|function| {
            (
                function.name.clone(),
                BackendFunctionValueContract {
                    inputs: function
                        .inputs
                        .iter()
                        .map(|value| value_contract(&value.value_type))
                        .collect(),
                    outputs: function
                        .outputs
                        .iter()
                        .map(|value| value_contract(&value.value_type))
                        .collect(),
                },
            )
        })
        .collect()
}

pub(crate) fn artifact_ref(artifact: &BackendArtifact) -> CompilerArtifactRef {
    CompilerArtifactRef::new(
        ArtifactRepresentation::BackendArtifact,
        BACKEND_ARTIFACT_SCHEMA_VERSION,
        artifact.bytes_sha256.clone(),
    )
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

pub(crate) fn empty_execution(
    artifact: &BackendArtifact,
    request: &ExecutionRequest,
) -> BackendExecutionResult {
    BackendExecutionResult {
        schema_version: crate::BACKEND_EXECUTION_RESULT_SCHEMA_VERSION.to_owned(),
        status: ExecutionStatus::InvalidRequest,
        target: request.target.clone(),
        backend: artifact.backend.clone(),
        artifact_identity: Some(artifact.identity.clone()),
        artifact_sha256: Some(artifact.bytes_sha256.clone()),
        returned: Vec::new(),
        steps: 0,
        effects: Vec::new(),
        failure: None,
    }
}

pub(crate) fn execution_failure(
    mut result: BackendExecutionResult,
    status: ExecutionStatus,
    reason: impl Into<String>,
) -> BackendExecutionResult {
    result.status = status;
    result.failure = Some(ExecutionFailure {
        identity: result.artifact_identity.clone(),
        reason: reason.into(),
    });
    result
}

pub(crate) fn argument_bits(value: &ExecutionValue) -> i128 {
    match value {
        ExecutionValue::Integer { value, .. } => *value,
        ExecutionValue::Boolean { value } => i128::from(*value),
        ExecutionValue::Finite { discriminant, .. } => i128::from(*discriminant),
        // Record values never cross a scalar process-argument boundary;
        // callers must reject them before marshalling.
        ExecutionValue::Record { name, .. } => {
            panic!("record value {name} cannot marshal to a native argument")
        }
    }
}

pub(crate) fn argument_argv(value: &ExecutionValue) -> Result<String, String> {
    match value {
        ExecutionValue::Record { name, .. } => Err(format!(
            "record value {name} cannot cross the native process argument boundary"
        )),
        other => Ok(argument_bits(other).to_string()),
    }
}

pub(crate) fn backend_matches_identity(
    artifact: &BackendArtifact,
    expected: &BackendIdentity,
    kind: &str,
) -> Result<(), String> {
    if !artifact.identity_is_valid() {
        return Err("backend artifact identity is stale or laundered".to_owned());
    }
    if artifact.backend != *expected {
        return Err("artifact backend identity does not match this adapter".to_owned());
    }
    if artifact.artifact_kind != kind {
        return Err(format!(
            "artifact kind {} is not {kind}",
            artifact.artifact_kind
        ));
    }
    Ok(())
}

pub(crate) fn backend_output_value_from_i128(
    contract: &BackendValueContract,
    value: i128,
) -> Result<ExecutionValue, crate::native::NativeError> {
    crate::backend_output_value(
        contract,
        ExecutionValue::Integer {
            value,
            ty: IntegerType {
                bits: 64,
                signed: true,
            },
        },
    )
    .map_err(crate::native::NativeError::InvalidOutput)
}

pub(crate) fn integer_fits(value: i128, ty: IntegerType) -> bool {
    if ty.signed {
        let bound = 1_i128 << (ty.bits - 1);
        (-bound..bound).contains(&value)
    } else {
        value >= 0 && value < (1_i128 << ty.bits)
    }
}

/// Process driver for the Cranelift realization whose ABI passes every
/// parameter as a 64-bit register value regardless of logical width.
pub(crate) fn process_driver_i64(function: &str, arity: usize) -> String {
    let parse_and_args = (0..arity)
        .map(|index| {
            (
                format!(
                    "  long long a{index} = strtoll(argv[{}], 0, 10);",
                    index + 1
                ),
                format!("a{index}"),
            )
        })
        .collect::<Vec<_>>();
    let parse = parse_and_args
        .iter()
        .map(|(parse, _)| parse.clone())
        .collect::<Vec<_>>()
        .join("\n");
    let call_args = parse_and_args
        .into_iter()
        .map(|(_, arg)| arg)
        .collect::<Vec<_>>();
    let mut proto_parts: Vec<String> = (0..arity).map(|_| "int64_t".to_owned()).collect();
    proto_parts.push("int32_t*".to_owned());
    proto_parts.push("int64_t*".to_owned());
    let proto = proto_parts.join(", ");
    let mut call_parts = call_args;
    call_parts.push("&status".to_owned());
    call_parts.push("&value".to_owned());
    let call = call_parts.join(", ");
    format!(
        r#"#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
void {function}({proto});
int main(int argc, char **argv) {{
  (void)argc;
{parse}
  int32_t status = 2;
  int64_t value = 0;
  {function}({call});
  if (status == 0) printf("{{\"status\":\"returned\",\"value\":%lld}}\n", (long long)value);
  else printf("{{\"status\":\"runtime_failure\"}}\n");
  return 0;
}}
"#
    )
}

pub(crate) fn process_driver(
    function: &str,
    inputs: &[mncs_model::BackendValueContract],
) -> String {
    // Parameter types follow each declared scalar so the generated prototype
    // matches the module definition exactly (an int32/int64 mismatch would be
    // C undefined behavior at every call boundary). Composite parameters never
    // reach this driver: they fail closed in the execute path above.
    fn scalar_c_type(contract: &mncs_model::BackendValueContract) -> &'static str {
        match contract {
            mncs_model::BackendValueContract::Scalar { semantic_type } => {
                match mncs_model::BodyType::from_semantic_name(semantic_type) {
                    mncs_model::BodyType::Integer(ty) if ty.bits == 64 => "int64_t",
                    _ => "int32_t",
                }
            }
            mncs_model::BackendValueContract::Finite { payloads, .. }
                if payloads.is_empty() =>
            {
                "int32_t"
            }
            _ => "int64_t",
        }
    }
    let parse_and_args = inputs
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            let parse = format!(
                "  long long a{index} = strtoll(argv[{}], 0, 10);",
                index + 1
            );
            let arg = format!("({})a{index}", scalar_c_type(ty));
            (parse, arg)
        })
        .collect::<Vec<_>>();
    let parse = parse_and_args
        .iter()
        .map(|(parse, _)| parse.clone())
        .collect::<Vec<_>>()
        .join("\n");
    let call_args = parse_and_args
        .iter()
        .map(|(_, arg)| arg.clone())
        .collect::<Vec<_>>();
    let proto = inputs
        .iter()
        .map(scalar_c_type)
        .map(str::to_owned)
        .collect::<Vec<_>>()
        .into_iter()
        .chain(["int32_t*".to_owned(), "int64_t*".to_owned()])
        .collect::<Vec<_>>()
        .join(", ");
    let mut call_parts = call_args;
    call_parts.push("&status".to_owned());
    call_parts.push("&value".to_owned());
    let call = call_parts.join(", ");
    format!(
        r#"#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
void {function}({proto});
int main(int argc, char **argv) {{
  (void)argc;
{parse}
  int32_t status = 2;
  int64_t value = 0;
  {function}({call});
  if (status == 0) printf("{{\"status\":\"returned\",\"value\":%lld}}\n", (long long)value);
  else printf("{{\"status\":\"runtime_failure\"}}\n");
  return 0;
}}
"#
    )
}
