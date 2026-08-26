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
                    mncs_model::function_id(
                        function.identity_namespace(&program.module),
                        &function.name,
                    ) == ssa_function.semantic_identity
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
        // Bytes marshal through their unsigned 8-bit domain.
        ExecutionValue::Byte { value } => *value,
        // Composite values never cross a scalar process-argument boundary;
        // callers must reject them before marshalling.
        ExecutionValue::Sequence { .. } => {
            panic!("sequence value cannot marshal to a native argument")
        }
        ExecutionValue::Record { name, .. } => {
            panic!("record value {name} cannot marshal to a native argument")
        }
    }
}

pub(crate) fn argument_argv(value: &ExecutionValue) -> Result<String, String> {
    match value {
        ExecutionValue::Sequence { .. } => {
            Err("sequence value cannot cross the native process argument boundary".to_owned())
        }
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

/// Whether one value contract crosses the boundary as a canonical cell.
pub(crate) fn contract_is_cell(contract: &BackendValueContract) -> bool {
    match contract {
        BackendValueContract::Record { .. } => true,
        BackendValueContract::Finite { payloads, .. } => {
            crate::composite::finite_payloads_declare_payloads(payloads)
        }
        BackendValueContract::Scalar { .. } => false,
    }
}

/// Build the canonical call file for one execution request. Returns `None`
/// when every argument crosses as a plain scalar (no call file needed).
pub(crate) fn build_call_file(
    arguments: &[ExecutionValue],
    input_contracts: &[BackendValueContract],
    output_contract: Option<&BackendValueContract>,
    composite_contracts: &BTreeMap<String, BackendValueContract>,
) -> Result<Option<Vec<u8>>, String> {
    let needs_cells = arguments.iter().any(|value| match value {
        ExecutionValue::Record { .. } => true,
        ExecutionValue::Finite { payload, .. } => !payload.is_empty(),
        _ => false,
    }) || input_contracts.iter().any(contract_is_cell)
        || output_contract.is_some_and(contract_is_cell);
    if !needs_cells {
        // Pure scalar calls keep the historical argv-only protocol.
        return Ok(None);
    }
    let mut writer = crate::composite::ArenaWriter::new(composite_contracts.clone());
    let mut entries: Vec<(u64, u64)> = Vec::new();
    for (index, value) in arguments.iter().enumerate() {
        let contract = input_contracts.get(index);
        let boundary = match contract {
            Some(BackendValueContract::Record { .. }) => writer.encode_argument(value)?,
            Some(BackendValueContract::Finite { payloads, .. }) if !payloads.is_empty() => {
                writer.encode_argument(value)?
            }
            _ => writer.encode_argument(value)?,
        };
        match boundary {
            crate::composite::BoundaryValue::Bits(bits) => {
                entries.push((0, bits));
            }
            crate::composite::BoundaryValue::Cell(root) => {
                entries.push((1, root));
            }
        }
    }
    let image = writer.into_image();
    let mut blob = Vec::new();
    blob.extend_from_slice(&0x4d4e435331_u64.to_le_bytes()); // "MNCS1"
    blob.extend_from_slice(&(entries.len() as u64).to_le_bytes());
    for (kind, payload) in entries {
        blob.extend_from_slice(&kind.to_le_bytes());
        blob.extend_from_slice(&payload.to_le_bytes());
    }
    blob.extend_from_slice(&(image.len() as u64).to_le_bytes());
    blob.extend_from_slice(&image);
    Ok(Some(blob))
}

/// Decode one returned observation into language values, resolving
/// composite results through the finished arena image when present.
pub(crate) fn decode_native_observation(
    output_contract: Option<&BackendValueContract>,
    status: ExecutionStatus,
    value: i128,
    arena_hex: Option<&str>,
    composite_contracts: &BTreeMap<String, BackendValueContract>,
) -> Result<Vec<ExecutionValue>, String> {
    if status != ExecutionStatus::Returned {
        return Ok(Vec::new());
    }
    let composite_output = output_contract.is_some_and(contract_is_cell);
    match (composite_output, arena_hex, output_contract) {
        (true, Some(hex), Some(contract)) => {
            let image = hex_decode_bytes(hex)?;
            let reader = crate::composite::ArenaReader::new(&image, composite_contracts);
            Ok(vec![reader.decode(value as u64, contract)?])
        }
        (true, _, _) => {
            Err("composite result was returned without a decodable arena image".to_owned())
        }
        (false, ..) => match output_contract {
            Some(contract) => backend_output_value_from_i128(contract, value)
                .map(|decoded| vec![decoded])
                .map_err(|error| error.reason()),
            None => Ok(vec![ExecutionValue::Integer {
                value,
                ty: IntegerType {
                    bits: 64,
                    signed: true,
                },
            }]),
        },
    }
}

fn hex_decode_bytes(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("arena hex has odd length".to_owned());
    }
    (0..hex.len() / 2)
        .map(|index| {
            u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
                .map_err(|error| format!("arena hex is malformed: {error}"))
        })
        .collect()
}

/// Full native observation handed back across the process boundary.
pub struct NativeRunView {
    pub status: ExecutionStatus,
    pub value: i128,
    pub arena_hex: Option<String>,
}

/// Driver variant for realizations whose code calls the canonical-cell
/// runtime entry points (mncs_cell_alloc / mncs_slot_*): the driver defines
/// the arena itself instead of importing module symbols.
pub(crate) fn process_driver_cell_runtime(
    function: &str,
    inputs: &[BackendValueContract],
    _output: Option<&BackendValueContract>,
) -> String {
    let base = process_driver_full(function, inputs);
    // Replace the extern declarations with local definitions of the same
    // names so imported cell libcalls resolve against this driver.
    base.replace(
        r#"extern unsigned char mncs_arena[4194304];
extern uint64_t mncs_bump;"#,
        r#"static unsigned char mncs_arena[4194304];
static uint64_t mncs_bump = 0;
uint64_t mncs_cell_alloc(uint64_t bytes) {
  uint64_t base = (mncs_bump + 7u) & ~(uint64_t)7u;
  mncs_bump = base + bytes;
  return base;
}
void mncs_slot_store32(uint64_t at, uint64_t v) {
  uint32_t x = (uint32_t)v;
  memcpy(mncs_arena + at, &x, sizeof x);
}
void mncs_slot_store64(uint64_t at, uint64_t v) {
  memcpy(mncs_arena + at, &v, sizeof v);
}
uint64_t mncs_slot_load32(uint64_t at) {
  uint32_t x;
  memcpy(&x, mncs_arena + at, sizeof x);
  return x;
}
uint64_t mncs_slot_load64(uint64_t at) {
  uint64_t x;
  memcpy(&x, mncs_arena + at, sizeof x);
  return x;
}"#,
    )
}

/// Whether any lowered function in the module manipulates canonical cells.
pub(crate) fn scalar_module_uses_cells(module: &crate::scalar::ScalarModule) -> bool {
    use crate::scalar::ScalarInst;
    module.functions.iter().any(|function| {
        function.params.iter().any(|param| param.ty.is_cell())
            || function.result.ty.is_cell()
            || function.blocks.iter().any(|block| {
                block.insts.iter().any(|inst| match inst {
                    ScalarInst::CellAlloc { .. }
                    | ScalarInst::CellStoreDiscriminant { .. }
                    | ScalarInst::CellStore { .. }
                    | ScalarInst::CellLoad { .. } => true,
                    ScalarInst::Sequence(nested) => nested.iter().any(|nested| {
                        matches!(
                            nested,
                            ScalarInst::CellAlloc { .. }
                                | ScalarInst::CellStoreDiscriminant { .. }
                                | ScalarInst::CellStore { .. }
                                | ScalarInst::CellLoad { .. }
                        )
                    }),
                    _ => false,
                })
            })
    })
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
/// Driver selection: pure scalar exports keep the historical argv-only
/// driver; exports touching canonical cells use the call-file protocol.
pub(crate) fn process_driver(
    function: &str,
    inputs: &[mncs_model::BackendValueContract],
    output: Option<&mncs_model::BackendValueContract>,
) -> String {
    let uses_cells = inputs.iter().any(is_cell_contract) || output.is_some_and(is_cell_contract);
    if !uses_cells {
        return process_driver_scalar_only(function, inputs);
    }
    process_driver_full(function, inputs)
}

fn is_cell_contract(contract: &mncs_model::BackendValueContract) -> bool {
    match contract {
        mncs_model::BackendValueContract::Record { .. } => true,
        mncs_model::BackendValueContract::Finite { payloads, .. } => !payloads.is_empty(),
        mncs_model::BackendValueContract::Scalar { .. } => false,
    }
}

/// Historical argv-only driver for pure scalar exports.
fn process_driver_scalar_only(
    function: &str,
    inputs: &[mncs_model::BackendValueContract],
) -> String {
    fn scalar_c_type(contract: &mncs_model::BackendValueContract) -> &'static str {
        match contract {
            mncs_model::BackendValueContract::Scalar { semantic_type } => {
                match mncs_model::BodyType::from_semantic_name(semantic_type) {
                    mncs_model::BodyType::Integer(ty) if ty.bits == 64 => "int64_t",
                    _ => "int32_t",
                }
            }
            mncs_model::BackendValueContract::Finite { payloads, .. } if payloads.is_empty() => {
                "int32_t"
            }
            _ => "int64_t",
        }
    }
    let parse_and_args = inputs
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            (
                format!(
                    "  long long a{index} = strtoll(argv[{}], 0, 10);",
                    index + 1
                ),
                format!("({})a{index}", scalar_c_type(ty)),
            )
        })
        .collect::<Vec<_>>();
    let parse = parse_and_args
        .iter()
        .map(|(parse, _)| parse.clone())
        .collect::<Vec<_>>()
        .join("\n");
    let proto = inputs
        .iter()
        .map(|contract| scalar_c_type(contract).to_owned())
        .collect::<Vec<_>>()
        .into_iter()
        .chain(["int32_t*".to_owned(), "int64_t*".to_owned()])
        .collect::<Vec<_>>()
        .join(", ");
    let mut call_parts = parse_and_args
        .into_iter()
        .map(|(_, arg)| arg)
        .collect::<Vec<_>>();
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

fn process_driver_full(function: &str, inputs: &[mncs_model::BackendValueContract]) -> String {
    // Parameter types follow each declared contract so the generated
    // prototype matches the module definition exactly (an int32/int64
    // mismatch would be C undefined behavior at every call boundary).
    // Composite parameters receive canonical cell offsets through the
    // MNCS call file; their C type is the offset word.
    fn is_cell(contract: &mncs_model::BackendValueContract) -> bool {
        match contract {
            mncs_model::BackendValueContract::Record { .. } => true,
            mncs_model::BackendValueContract::Finite { payloads, .. } => {
                crate::composite::finite_payloads_declare_payloads(payloads)
            }
            mncs_model::BackendValueContract::Scalar { .. } => false,
        }
    }
    fn scalar_c_type(contract: &mncs_model::BackendValueContract) -> &'static str {
        match contract {
            mncs_model::BackendValueContract::Scalar { semantic_type } => {
                match mncs_model::BodyType::from_semantic_name(semantic_type) {
                    mncs_model::BodyType::Integer(ty) if ty.bits == 64 => "int64_t",
                    _ => "int32_t",
                }
            }
            mncs_model::BackendValueContract::Finite { payloads, .. }
                if !crate::composite::finite_payloads_declare_payloads(payloads) =>
            {
                "int32_t"
            }
            _ => "int64_t",
        }
    }
    fn arg_c_type(contract: &mncs_model::BackendValueContract) -> &'static str {
        if is_cell(contract) {
            "uint64_t"
        } else {
            scalar_c_type(contract)
        }
    }
    let prepare = inputs
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            // Every parameter crosses through the call file in this mode:
            // scalar bits and cell roots share the same entry array.
            let ctype = arg_c_type(ty);
            format!(
                "  {ctype} a{index} = (arg_count > {index}) ? ({ctype})values[{index}] : ({ctype})0;"
            )
        })
        .collect::<Vec<_>>();
    let parse = prepare.join("\n");
    let prepare_legacy = inputs
        .iter()
        .enumerate()
        .map(|(index, _ty)| {
            format!(
                "  long long a{index} = ({index} < scalar_argc) ? strtoll(scalar_argv[{index}], 0, 10) : 0;"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let proto = inputs
        .iter()
        .map(|contract| arg_c_type(contract).to_owned())
        .collect::<Vec<_>>()
        .into_iter()
        .chain(["int32_t*".to_owned(), "int64_t*".to_owned()])
        .collect::<Vec<_>>()
        .join(", ");
    let call_args = (0..inputs.len())
        .map(|index| {
            let ctype = arg_c_type(&inputs[index]);
            format!("({ctype})a{index}")
        })
        .collect::<Vec<_>>();
    let mut call_parts = call_args;
    call_parts.push("&status".to_owned());
    call_parts.push("&value".to_owned());
    let call = call_parts.join(", ");
    let n = inputs.len();
    format!(
        r#"#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern unsigned char mncs_arena[4194304];
extern uint64_t mncs_bump;

void {function}({proto});

static unsigned char *mncs_read_file(const char *path, long *out_len) {{
  FILE *f = fopen(path, "rb");
  if (!f) return 0;
  fseek(f, 0, SEEK_END);
  long len = ftell(f);
  fseek(f, 0, SEEK_SET);
  unsigned char *blob = (unsigned char *)malloc((size_t)len);
  if (!blob) {{ fclose(f); return 0; }}
  if (fread(blob, 1, (size_t)len, f) != (size_t)len) {{ free(blob); fclose(f); return 0; }}
  fclose(f);
  *out_len = len;
  return blob;
}}

int main(int argc, char **argv) {{
  (void)argc;
  int32_t status = 2;
  int64_t value = 0;
  const char *call_path = getenv("MNCS_CALL_FILE");
  if (call_path) {{
    /* Canonical call file v0.1:
       [u64 magic][u64 arg_count]{{ [u64 kind][u64 payload] }}*
       [u64 arena_bytes][arena image]
       kind 0 = scalar bits passed on argv as well, 1 = cell root. */
    long flen = 0;
    unsigned char *blob = mncs_read_file(call_path, &flen);
    if (!blob || flen < 16) {{ free(blob); return 2; }}
    uint64_t magic = 0, arg_count = 0;
    memcpy(&magic, blob, 8);
    memcpy(&arg_count, blob + 8, 8);
    if (magic != 0x4d4e435331ULL) {{ free(blob); return 2; }}
    uint64_t values[{n}] = {{0}};
    const unsigned char *p = blob + 16;
    for (uint64_t i = 0; i < arg_count && (long)(p - blob) + 16 <= flen; i++) {{
      uint64_t kind = 0, payload = 0;
      memcpy(&kind, p, 8); p += 8;
      memcpy(&payload, p, 8); p += 8;
      (void)kind;
      if (i < {n}) values[i] = payload;
    }}
    uint64_t arena_bytes = 0;
    if ((long)(p - blob) + 8 <= flen) {{ memcpy(&arena_bytes, p, 8); p += 8; }}
    if (arena_bytes > sizeof(mncs_arena) || (long)(p - blob) + (long)arena_bytes > flen) {{
      free(blob); return 2;
    }}
    memcpy(mncs_arena, p, arena_bytes);
    mncs_bump = arena_bytes;
    free(blob);
    (void)argc;
    (void)argv;
    {parse}
    {function}({call});
    if (status != 0) {{
      printf("{{\"status\":\"runtime_failure\"}}\n");
      return 0;
    }}
    printf("{{\"status\":\"returned\",\"value\":%lld,\"arena_hex\":\"", (long long)value);
    static const char *hex = "0123456789abcdef";
    for (uint64_t i = 0; i < mncs_bump && i < sizeof(mncs_arena); i++) {{
      putchar(hex[mncs_arena[i] >> 4]);
      putchar(hex[mncs_arena[i] & 15]);
    }}
    printf("\"}}\n");
    return 0;
  }}
  const char **scalar_argv = (const char **)argv + 1;
  int scalar_argc = argc - 1;
  (void)scalar_argv;
  (void)scalar_argc;
{prepare_legacy}
  {function}({call});
  if (status == 0) printf("{{\"status\":\"returned\",\"value\":%lld}}\n", (long long)value);
  else printf("{{\"status\":\"runtime_failure\"}}\n");
  return 0;
}}
"#
    )
}

#[cfg(test)]
mod driver_tests {
    use super::*;

    #[test]
    fn cell_runtime_driver_defines_the_canonical_symbols() {
        let record = BackendValueContract::Record {
            type_identity: mncs_model::SemanticId("T:Pair".to_owned()),
            name: "Pair".to_owned(),
            fields: vec![],
        };
        let driver = process_driver_cell_runtime("f", &[record], None);
        assert!(
            driver.contains("uint64_t mncs_cell_alloc"),
            "alloc symbol defined"
        );
        assert!(driver.contains("mncs_slot_load32"), "load32 defined");
        assert!(
            !driver.contains("extern unsigned char mncs_arena"),
            "no externs remain"
        );
    }
}
