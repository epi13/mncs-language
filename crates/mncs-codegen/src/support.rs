//! Shared adapter helpers that must stay backend-neutral.

use std::collections::BTreeMap;
use std::fmt::Write;

use mncs_model::{
    ArtifactRepresentation, BackendArtifact, BackendFunctionValueContract, BackendIdentity,
    BackendResult, BackendValueContract, BodyType, CompilerArtifactRef, CompilerDiagnostic,
    CompilerDiagnosticKind, ExecutionFailure, ExecutionRequest, ExecutionStatus, ExecutionValue,
    IntegerType, Program, SequenceBound, SsaModule, TransformationStatus,
    BACKEND_ARTIFACT_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};

use crate::BackendExecutionResult;

/// Canonical native cell-arena capacity in bytes, shared by every native
/// realization (C11, LLVM IR, the AOT cell runtime, and the Cranelift JIT
/// host runtime) so flagship-scale admission fits everywhere with headroom:
/// measured peak is 6,020,616 bytes for `binding_reusable` over the
/// 29-cell flagship (`admit-reuse-flagship`). The bump allocators still
/// carry no in-band overflow guard, so genuine exhaustion stays a loud
/// runner-level failure, never a silent verdict.
pub(crate) const NATIVE_ARENA_BYTES: u64 = 16 * 1024 * 1024;

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

pub(crate) fn validate_realizable_ssa(
    program: &Program,
    ssa: &SsaModule,
    code: &str,
) -> Result<(), Box<BackendResult>> {
    let report = ssa.validate_lowering_boundary(program);
    if report.valid {
        mncs_model::record_counter("backend_validation");
        return Ok(());
    }
    let diagnostics = report
        .errors
        .into_iter()
        .map(|error| {
            let mut diagnostic = CompilerDiagnostic::new(
                code,
                CompilerDiagnosticKind::InternalCompilerDefect,
                error.message,
            );
            diagnostic.path = Some(error.path);
            diagnostic
        })
        .collect();
    Err(Box::new(failed(diagnostics)))
}

/// Native symbol for one MNCS function, derived from its canonical
/// (declaring-module, name) identity.
///
/// Every lowered function — root or imported, generic specialization or
/// plain — owns a distinct native symbol, so two legal modules may each
/// define the same local name and link into one program without backend
/// redefinition failures (ENG-PRESSURE-0017). The spelling is a pure
/// function of the canonical identity: `mncs_<namespace>__<name>` with
/// injective escaping (`_` doubles, any other non-alphanumeric byte folds
/// to a single `_`), so distinct identities never spell the same symbol
/// and C never sees a bare `main`. Drivers and module emission share this
/// exact function, so they agree by construction. Execution requests keep
/// addressing entries by `(module, function)`; the execute path derives
/// the same symbol from the request target.
pub(crate) fn qualified_c_symbol(namespace: &str, name: &str) -> String {
    format!(
        "mncs_{}__{}",
        mangle_symbol_part(namespace),
        mangle_symbol_part(name)
    )
}

/// Injective mangle for one namespace/name part: `_` escapes to `__` first
/// (so pre-existing underscores stay distinguishable), then every remaining
/// non-alphanumeric byte (notably the `.` module separators) folds to a
/// single `_`. The output is always a valid C identifier tail.
fn mangle_symbol_part(part: &str) -> String {
    part.replace('_', "__")
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

/// Map key addressing one function value contract by canonical identity.
/// Distinct from the native symbol on purpose: contracts are looked up by
/// entry identity, symbols name emitted code.
pub(crate) fn entry_key(namespace: &str, name: &str) -> String {
    format!("{namespace}::{name}")
}

/// One lowered function's declaration facts: canonical identity plus the
/// emitted native symbol. Built once per lowering so name-based selections
/// (PTX kernel entries) and entry resolution share the module's mapping.
pub(crate) struct EntryDecl {
    pub namespace: String,
    pub name: String,
    pub symbol: String,
}

/// Declaration facts for every SSA function in lowering order. Functions
/// without a program declaration (unreachable for a valid pipeline) carry
/// their fallback export spelling under an empty namespace.
pub(crate) fn entry_decls(program: &Program, ssa: &mncs_model::SsaModule) -> Vec<EntryDecl> {
    let names = function_names(program, ssa);
    ssa.functions
        .iter()
        .zip(names)
        .map(|(ssa_function, symbol)| {
            match program.functions.iter().find(|function| {
                mncs_model::function_id(
                    function.identity_namespace(&program.module),
                    &function.name,
                ) == ssa_function.semantic_identity
            }) {
                Some(function) => EntryDecl {
                    namespace: function.identity_namespace(&program.module).to_owned(),
                    name: function.name.clone(),
                    symbol,
                },
                None => EntryDecl {
                    namespace: String::new(),
                    name: symbol.clone(),
                    symbol,
                },
            }
        })
        .collect()
}

/// Resolve one logical kernel-entry selection to a physical export symbol:
/// an exact symbol, a `module::name` identity, or a short name that is
/// unambiguous in this program. Ambiguous or unknown entries resolve to
/// `None` so callers fail closed with a structured diagnostic as before.
pub(crate) fn resolve_kernel_entry(entry: &str, decls: &[EntryDecl]) -> Option<String> {
    if let Some(exact) = decls.iter().find(|decl| decl.symbol == entry) {
        return Some(exact.symbol.clone());
    }
    if let Some((namespace, name)) = entry.split_once("::") {
        return decls
            .iter()
            .find(|decl| decl.namespace == namespace && decl.name == name)
            .map(|decl| decl.symbol.clone());
    }
    let mut matches = decls.iter().filter(|decl| decl.name == entry);
    match (matches.next(), matches.next()) {
        (Some(only), None) => Some(only.symbol.clone()),
        _ => None,
    }
}

pub(crate) fn function_names(program: &Program, ssa: &SsaModule) -> Vec<String> {
    ssa.functions
        .iter()
        .map(|ssa_function| {
            match program.functions.iter().find(|function| {
                mncs_model::function_id(
                    function.identity_namespace(&program.module),
                    &function.name,
                ) == ssa_function.semantic_identity
            }) {
                Some(function) => {
                    qualified_c_symbol(function.identity_namespace(&program.module), &function.name)
                }
                None => export_name(&ssa_function.semantic_identity.0),
            }
        })
        .collect()
}

pub(crate) fn export_name(identity: &str) -> String {
    c_symbol(
        &identity
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
            .collect::<String>(),
    )
}

/// Native symbol for an MNCS function base name.
///
/// All MNCS-generated function symbols live under the `mncs_` namespace so
/// they cannot collide with libc/libm (`trunc`, `min`, `exp`, `log`, ...),
/// compiler/runtime support symbols, or platform names. `fn main` is the
/// natural MNCS entry shape but C reserves `main`, so it maps to
/// `mncs_main` like every other function. Names already under `mncs_` keep
/// their spelling (idempotent); everything else gains the prefix. The C11,
/// LLVM, and Cranelift drivers map through this same function, so module
/// and driver always agree. This is hygienic by construction, not a
/// blacklist: any present or future libc name is namespaced away.
pub(crate) fn c_symbol(name: &str) -> String {
    if name == "main" || name == "mncs_main" {
        "mncs_main".to_owned()
    } else if let Some(stripped) = name.strip_prefix("mncs_") {
        // Already namespaced. Re-prefix only if the remainder would still
        // collide with the runtime helper surface is unnecessary: the full
        // `mncs_` spelling is already reserved for generated symbols, and
        // keeping it stable preserves artifact determinism.
        format!("mncs_{stripped}")
    } else {
        format!("mncs_{name}")
    }
}

/// Logical composite types (records and payload-bearing finite variants) used
/// by this program's signatures, keyed by semantic type name. Attached to
/// artifacts so execution can marshal composite values without the program.
pub(crate) fn composite_value_contracts(
    program: &Program,
) -> BTreeMap<String, BackendValueContract> {
    let mut composites = BTreeMap::new();
    for record in &program.record_types {
        let contract = BackendValueContract::Record {
            type_identity: record.identity.clone(),
            name: record.name.clone(),
            fields: record
                .fields
                .iter()
                .map(|field| (field.name.clone(), field.field_type.clone()))
                .collect(),
        };
        composites.insert(record.name.clone(), contract.clone());
        composites.insert(record.identity.to_string(), contract);
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
        let contract = BackendValueContract::Finite {
            type_identity: finite.identity.clone(),
            variants: finite
                .variants
                .iter()
                .map(|variant| (variant.discriminant, variant.identity.clone()))
                .collect(),
            payloads,
        };
        composites.insert(finite.name.clone(), contract.clone());
        composites.insert(finite.identity.to_string(), contract);
    }
    composites
}

/// Classify one declared semantic type into the language-owned backend
/// value contract used at process and interpreter boundaries.
pub(crate) fn value_contract_for(program: &Program, name: &str) -> BackendValueContract {
    if let Some(record_type) = program
        .record_types
        .iter()
        .find(|record| record.name == name || record.identity.0 == name)
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
    if let Some(finite_type) = program
        .finite_types
        .iter()
        .find(|finite_type| finite_type.name == name || finite_type.identity.0 == name)
    {
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
        return BackendValueContract::Finite {
            type_identity: finite_type.identity.clone(),
            variants: finite_type
                .variants
                .iter()
                .map(|variant| (variant.discriminant, variant.identity.clone()))
                .collect(),
            payloads,
        };
    }
    match BodyType::from_semantic_name(name) {
        BodyType::Sequence {
            element,
            bound: SequenceBound::Exact(length),
        } => BackendValueContract::Sequence {
            semantic_type: name.to_owned(),
            element: element.semantic_name(),
            length,
        },
        BodyType::Sequence {
            element,
            bound: SequenceBound::UpTo(capacity),
        } => BackendValueContract::View {
            semantic_type: name.to_owned(),
            element: element.semantic_name(),
            capacity,
        },
        BodyType::Vector { element, lanes } => BackendValueContract::Vector {
            semantic_type: name.to_owned(),
            element: element.semantic_name(),
            lanes,
        },
        BodyType::Mask { lanes } => BackendValueContract::Mask {
            semantic_type: name.to_owned(),
            lanes,
        },
        _ => BackendValueContract::Scalar {
            semantic_type: name.to_owned(),
        },
    }
}

pub(crate) fn function_value_contracts(
    program: &Program,
) -> BTreeMap<String, BackendFunctionValueContract> {
    let mut contracts = BTreeMap::new();
    for function in &program.functions {
        let contract = BackendFunctionValueContract {
            inputs: function
                .inputs
                .iter()
                .map(|value| value_contract_for(program, &value.value_type))
                .collect(),
            outputs: function
                .outputs
                .iter()
                .map(|value| value_contract_for(program, &value.value_type))
                .collect(),
        };

        // Linked declarations can share a short name with a root-module
        // entrypoint.  The process boundary addresses an entrypoint by its
        // exported short name, so the root declaration must own that ABI
        // slot; otherwise an imported generic declaration can overwrite the
        // wrapper contract and reject a valid composite request on strict
        // backends.  Imported-only names remain available as before.
        //
        // Every declaration additionally owns its canonical `module::name`
        // slot, so entry resolution can disambiguate same-named functions
        // from distinct modules (ENG-PRESSURE-0017) instead of inheriting
        // whichever short-name shape won the slot above.
        if function.home_module.is_none() {
            contracts.insert(function.name.clone(), contract.clone());
        } else {
            contracts
                .entry(function.name.clone())
                .or_insert(contract.clone());
        }
        contracts.insert(
            entry_key(function.identity_namespace(&program.module), &function.name),
            contract,
        );
    }
    contracts
}

/// Language-owned value contract for one execution entry, resolved by
/// canonical `(module, function)` identity first and by legacy short name
/// second. The qualified slot always names the requested declaration
/// exactly; the short-name fallback keeps requests against artifacts
/// emitted before qualified symbols (and single-module programs, where the
/// two agree) working.
pub(crate) fn entry_value_contract<'a>(
    contracts: &'a std::collections::BTreeMap<String, mncs_model::BackendFunctionValueContract>,
    module: &str,
    function: &str,
) -> Option<&'a mncs_model::BackendFunctionValueContract> {
    contracts
        .get(&entry_key(module, function))
        .or_else(|| contracts.get(function))
}

/// Native code symbol for one execution entry. Prefers the canonical
/// qualified spelling; falls back to the legacy short spelling only when
/// the artifact at hand does not export the qualified symbol (artifacts
/// emitted before qualified lowering, loaded from disk via
/// `experiment execute`). The fallback keeps old artifacts executable;
/// freshly lowered programs always take the qualified branch.
pub(crate) fn entry_native_symbol(exports: &[String], module: &str, function: &str) -> String {
    let qualified = qualified_c_symbol(module, function);
    if exports.iter().any(|export| export == &qualified) {
        qualified
    } else {
        c_symbol(function)
    }
}

/// Whether one emitted module (WASM export list or decoded function table)
/// carries an entry under its qualified spelling.
pub(crate) fn exports_contain(exports: &[String], module: &str, function: &str) -> bool {
    let qualified = qualified_c_symbol(module, function);
    exports.iter().any(|export| export == &qualified)
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
        // Floats marshal through their bit pattern (lossless: i128 holds
        // every u64). The argv front door uses the decimal spelling
        // instead; this arm serves bit-exact callers.
        ExecutionValue::Float { bits, .. } => i128::from(*bits),
        // Collection and record values cross through the canonical call
        // file, never as a scalar argv word. Reaching this arm is a
        // driver-selection bug, not a representation choice.
        ExecutionValue::Sequence { .. }
        | ExecutionValue::Vector { .. }
        | ExecutionValue::Mask { .. } => {
            panic!("collection value cannot marshal to a native argv word")
        }
        ExecutionValue::Record { name, .. } => {
            panic!("record value {name} cannot marshal to a native argv word")
        }
    }
}

pub(crate) fn argument_argv(value: &ExecutionValue) -> Result<String, String> {
    match value {
        ExecutionValue::Sequence { .. }
        | ExecutionValue::Vector { .. }
        | ExecutionValue::Mask { .. }
        | ExecutionValue::Record { .. } => Err(
            "composite or collection value requires the canonical call-file boundary".to_owned(),
        ),
        ExecutionValue::Finite { payload, .. } if !payload.is_empty() => Err(
            "composite or collection value requires the canonical call-file boundary".to_owned(),
        ),
        // Float words are shortest round-trip decimals (exact through
        // `strtod`); boundary values are finite by the trap rule, so the
        // spelling always parses.
        ExecutionValue::Float { bits, .. } => Ok(format!("{}", f64::from_bits(*bits))),
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

/// Whether one value contract crosses the boundary as a canonical cell
/// offset. Views and masks are packed 64-bit words, not cell roots.
pub(crate) fn contract_is_cell(contract: &BackendValueContract) -> bool {
    match contract {
        BackendValueContract::Record { .. }
        | BackendValueContract::Sequence { .. }
        | BackendValueContract::Vector { .. } => true,
        BackendValueContract::Finite { payloads, .. } => {
            crate::composite::finite_payloads_declare_payloads(payloads)
        }
        BackendValueContract::Scalar { .. }
        | BackendValueContract::View { .. }
        | BackendValueContract::Mask { .. } => false,
    }
}

/// Whether one value contract crosses the boundary as a binary64 float
/// word (decimal argv spelling, bit-carried JIT word).
pub(crate) fn contract_is_float(contract: &BackendValueContract) -> bool {
    matches!(
        contract,
        BackendValueContract::Scalar { semantic_type }
            if matches!(
                mncs_model::BodyType::from_semantic_name(semantic_type),
                mncs_model::BodyType::Float(float) if float.is_supported()
            )
    )
}

/// Whether executing this contract requires the canonical arena image.
/// Views store their elements in the arena even though the ABI word is a
/// packed descriptor, not a cell root. Masks are packed bits and do not.
pub(crate) fn contract_needs_arena(contract: &BackendValueContract) -> bool {
    contract_is_cell(contract) || matches!(contract, BackendValueContract::View { .. })
}

/// Whether a contract crosses the native process boundary through the
/// canonical call-file protocol rather than decimal argv words.
///
/// Masks do not occupy arena cells, but they are still not argv scalars:
/// they travel as packed 64-bit words in the same call-file entry array
/// as cell roots and view descriptors. `argument_argv` currently rejects
/// them, so forcing them onto argv has no encoder.
pub(crate) fn contract_uses_call_file(contract: &BackendValueContract) -> bool {
    contract_needs_arena(contract) || matches!(contract, BackendValueContract::Mask { .. })
}

/// Whether a request value must use the call-file protocol.
pub(crate) fn value_uses_call_file(value: &ExecutionValue) -> bool {
    match value {
        ExecutionValue::Record { .. }
        | ExecutionValue::Sequence { .. }
        | ExecutionValue::Vector { .. }
        | ExecutionValue::Mask { .. } => true,
        ExecutionValue::Finite { payload, .. } => !payload.is_empty(),
        _ => false,
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
    let needs_call_file = arguments.iter().any(value_uses_call_file)
        || input_contracts.iter().any(contract_uses_call_file)
        || output_contract.is_some_and(contract_uses_call_file);
    if !needs_call_file {
        // Pure scalar calls keep the historical argv-only protocol.
        return Ok(None);
    }
    let mut writer = crate::composite::ArenaWriter::new(composite_contracts.clone());
    let mut entries: Vec<(u64, u64)> = Vec::new();
    for (index, value) in arguments.iter().enumerate() {
        let contract = input_contracts.get(index);
        let boundary = writer.encode_argument_with_contract(value, contract)?;
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
    let needs_arena = output_contract.is_some_and(contract_needs_arena);
    match (needs_arena, arena_hex, output_contract) {
        (true, Some(hex), Some(contract)) => {
            let image = hex_decode_bytes(hex)?;
            let reader = crate::composite::ArenaReader::new(&image, composite_contracts);
            Ok(vec![reader.decode_boundary(value as u64, contract)?])
        }
        (true, _, _) => Err(
            "collection or composite result was returned without a decodable arena image"
                .to_owned(),
        ),
        (false, _, Some(contract)) => match contract {
            BackendValueContract::Mask { lanes, .. } => {
                Ok(vec![crate::composite::unpack_mask(value as u64, *lanes)])
            }
            _ => backend_output_value_from_i128(contract, value)
                .map(|decoded| vec![decoded])
                .map_err(|error| error.reason()),
        },
        (false, _, None) => Ok(vec![ExecutionValue::Integer {
            value,
            ty: IntegerType {
                bits: 64,
                signed: true,
            },
        }]),
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
    // Same native-symbol rule as the scalar driver: module and driver
    // must agree on `mncs_main`.
    let base = process_driver_full(&c_symbol(function), inputs);
    // Replace the extern declarations with local definitions of the same
    // names so imported cell libcalls resolve against this driver.
    let pattern = format!(
        "extern unsigned char mncs_arena[{NATIVE_ARENA_BYTES}];\nextern uint64_t mncs_bump;"
    );
    let prefix = format!(
        "static unsigned char mncs_arena[{NATIVE_ARENA_BYTES}];\nstatic uint64_t mncs_bump = 0;"
    );
    let replacement = format!(
        "{prefix}\n\
         uint64_t mncs_cell_alloc(uint64_t bytes) {{\n  \
           uint64_t base = (mncs_bump + 7u) & ~(uint64_t)7u;\n  \
           mncs_bump = base + bytes;\n  \
           return base;\n\
         }}\n\
         void mncs_slot_store32(uint64_t at, uint64_t v) {{\n  \
           uint32_t x = (uint32_t)v;\n  \
           memcpy(mncs_arena + at, &x, sizeof x);\n\
         }}\n\
         void mncs_slot_store64(uint64_t at, uint64_t v) {{\n  \
           memcpy(mncs_arena + at, &v, sizeof v);\n\
         }}\n\
         uint64_t mncs_slot_load32(uint64_t at) {{\n  \
           uint32_t x;\n  \
           memcpy(&x, mncs_arena + at, sizeof x);\n  \
           return x;\n\
         }}\n\
         uint64_t mncs_slot_load64(uint64_t at) {{\n  \
           uint64_t x;\n  \
           memcpy(&x, mncs_arena + at, sizeof x);\n  \
           return x;\n\
         }}"
    );
    let replaced = base.replace(&pattern, &replacement);
    debug_assert!(
        replaced.len() != base.len(),
        "cell-runtime driver must carry the canonical arena declarations"
    );
    replaced
}

/// Whether the native process driver will need `mncs_arena` / `mncs_bump`.
/// Call-file copies an arena image even when the image is empty, so mask-only
/// and view-only modules still require the symbols even if they never
/// allocate cells.
pub(crate) fn scalar_module_needs_arena_symbols(module: &crate::scalar::ScalarModule) -> bool {
    use crate::scalar::ScalarTy;
    scalar_module_uses_cells(module)
        || module.functions.iter().any(|function| {
            function
                .params
                .iter()
                .any(|param| matches!(param.ty, ScalarTy::View | ScalarTy::Mask(_)))
                || matches!(function.result.ty, ScalarTy::View | ScalarTy::Mask(_))
        })
}

fn scalar_inst_uses_cells(inst: &crate::scalar::ScalarInst) -> bool {
    use crate::scalar::ScalarInst;
    match inst {
        ScalarInst::CellAlloc { .. }
        | ScalarInst::CellStoreDiscriminant { .. }
        | ScalarInst::CellStore { .. }
        | ScalarInst::CellLoad { .. }
        | ScalarInst::SequenceReplace { .. } => true,
        // Sequence projection lowers to canonical slot loads
        // (`mncs_slot_load32/64`) on every native backend, so a module that
        // only indexes into sequences or views still needs the cell helpers
        // in its prelude. Omitting them produced undeclared-function C
        // failures (and unresolved-symbol JIT failures) for pure
        // index-into-view modules.
        ScalarInst::SequenceProject { .. } => true,
        ScalarInst::Sequence(nested) => nested.iter().any(scalar_inst_uses_cells),
        _ => false,
    }
}

/// Whether any lowered function in the module manipulates canonical cells.
pub(crate) fn scalar_module_uses_cells(module: &crate::scalar::ScalarModule) -> bool {
    module.functions.iter().any(|function| {
        function.params.iter().any(|param| param.ty.is_cell())
            || function.result.ty.is_cell()
            || function
                .blocks
                .iter()
                .any(|block| block.insts.iter().any(scalar_inst_uses_cells))
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
    // The driver declares and calls the module symbol, so it maps through
    // the same native-symbol rule as lowering (notably `main`).
    let symbol = c_symbol(function);
    let uses_call_file =
        inputs.iter().any(contract_uses_call_file) || output.is_some_and(contract_uses_call_file);
    if !uses_call_file {
        return process_driver_scalar_only(&symbol, inputs);
    }
    process_driver_full(&symbol, inputs)
}

fn uses_uint64_abi(contract: &mncs_model::BackendValueContract) -> bool {
    contract_is_cell(contract)
        || matches!(
            contract,
            mncs_model::BackendValueContract::View { .. }
                | mncs_model::BackendValueContract::Mask { .. }
        )
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
                    mncs_model::BodyType::Float(ty) if ty.is_supported() => "double",
                    _ => "int32_t",
                }
            }
            mncs_model::BackendValueContract::Finite { payloads, .. } if payloads.is_empty() => {
                "int32_t"
            }
            _ => "int64_t",
        }
    }
    fn is_float_contract(contract: &mncs_model::BackendValueContract) -> bool {
        matches!(
            contract,
            mncs_model::BackendValueContract::Scalar { semantic_type }
                if matches!(
                    mncs_model::BodyType::from_semantic_name(semantic_type),
                    mncs_model::BodyType::Float(ty) if ty.is_supported()
                )
        )
    }
    let parse_and_args = inputs
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            // Float words parse with `strtod` (shortest round-trip decimal
            // is exact); integers keep the `strtoull` bit-exact path.
            let parse = if is_float_contract(ty) {
                format!("  double a{index} = strtod(argv[{}], 0);", index + 1)
            } else {
                // Full-range argv words: `strtoll` saturates u64 values
                // above i64::MAX to LLONG_MAX, so full-range integers
                // parse with `strtoull` and narrow by cast (bit-exact on
                // two's-complement targets, including negative words,
                // which wrap around and cast back exactly).
                format!(
                    "  unsigned long long a{index} = strtoull(argv[{}], 0, 10);",
                    index + 1
                )
            };
            (parse, format!("({})a{index}", scalar_c_type(ty)))
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
    fn scalar_c_type(contract: &mncs_model::BackendValueContract) -> &'static str {
        match contract {
            mncs_model::BackendValueContract::Scalar { semantic_type } => {
                match mncs_model::BodyType::from_semantic_name(semantic_type) {
                    mncs_model::BodyType::Integer(ty) if ty.bits == 64 => "int64_t",
                    mncs_model::BodyType::Float(ty) if ty.is_supported() => "double",
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
    fn is_float_contract(contract: &mncs_model::BackendValueContract) -> bool {
        matches!(
            contract,
            mncs_model::BackendValueContract::Scalar { semantic_type }
                if matches!(
                    mncs_model::BodyType::from_semantic_name(semantic_type),
                    mncs_model::BodyType::Float(ty) if ty.is_supported()
                )
        )
    }
    fn arg_c_type(contract: &mncs_model::BackendValueContract) -> &'static str {
        if uses_uint64_abi(contract) {
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
            // scalar bits and cell roots share the same entry array. Float
            // entries are bit patterns, so they reinterpret (never convert)
            // into doubles.
            if is_float_contract(ty) {
                format!(
                    "  double a{index} = (arg_count > {index}) ? mncs_bits_to_double(values[{index}]) : 0.0;"
                )
            } else {
                let ctype = arg_c_type(ty);
                format!(
                    "  {ctype} a{index} = (arg_count > {index}) ? ({ctype})values[{index}] : ({ctype})0;"
                )
            }
        })
        .collect::<Vec<_>>();
    let parse = prepare.join("\n");
    let prepare_legacy = inputs
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            if is_float_contract(ty) {
                format!(
                    "  double a{index} = ({index} < scalar_argc) ? strtod(scalar_argv[{index}], 0) : 0.0;"
                )
            } else {
                format!(
                    "  unsigned long long a{index} = ({index} < scalar_argc) ? strtoull(scalar_argv[{index}], 0, 10) : 0;"
                )
            }
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

extern unsigned char mncs_arena[{NATIVE_ARENA_BYTES}];
extern uint64_t mncs_bump;

void {function}({proto});

static double mncs_bits_to_double(uint64_t bits) {{
  double value = 0.0;
  memcpy(&value, &bits, 8);
  return value;
}}

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
    use mncs_model::{FiniteType, FiniteVariant};

    #[test]
    fn full_semantic_finite_identity_resolves_to_a_finite_contract() {
        let identity = mncs_model::SemanticId("mncs:0.2:finite-type:example::Status".to_owned());
        let program = Program {
            schema_version: mncs_model::SUPPORTED_SCHEMA_VERSION.to_owned(),
            module: "example".to_owned(),
            dependencies: Vec::new(),
            finite_types: vec![FiniteType {
                identity: identity.clone(),
                name: "Status".to_owned(),
                variants: vec![FiniteVariant {
                    identity: mncs_model::SemanticId(
                        "mncs:0.2:finite-variant:example::Status::Pass".to_owned(),
                    ),
                    name: "Pass".to_owned(),
                    discriminant: 0,
                    payload: Vec::new(),
                }],
            }],
            record_types: Vec::new(),
            assumptions: Vec::new(),
            binding_table: None,
            functions: Vec::new(),
            generic_specializations: Vec::new(),
        };

        assert!(matches!(
            value_contract_for(&program, identity.as_str()),
            BackendValueContract::Finite { .. }
        ));
        assert!(composite_value_contracts(&program).contains_key(identity.as_str()));
    }

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

    #[test]
    fn mask_only_signatures_use_the_call_file_protocol() {
        let mask = BackendValueContract::Mask {
            semantic_type: "mask<4>".to_owned(),
            lanes: 4,
        };
        assert!(
            contract_uses_call_file(&mask),
            "masks are packed call-file words"
        );
        assert!(
            !contract_needs_arena(&mask),
            "masks do not occupy arena cells"
        );
        let driver = process_driver("any_of", std::slice::from_ref(&mask), None);
        assert!(
            driver.contains("MNCS_CALL_FILE"),
            "mask-only inputs must not fall back to argv"
        );
        let out_driver = process_driver("bits", &[], Some(&mask));
        assert!(
            out_driver.contains("MNCS_CALL_FILE"),
            "mask-only results must not fall back to argv"
        );
        let packed = crate::composite::pack_mask(&[true, false, false, true]);
        let mask_value = ExecutionValue::Mask {
            lanes: vec![true, false, false, true].into(),
        };
        assert!(value_uses_call_file(&mask_value));
        let blob = build_call_file(
            &[mask_value],
            std::slice::from_ref(&mask),
            None,
            &BTreeMap::new(),
        )
        .expect("call file")
        .expect("mask-only still emits a call file");
        assert!(blob.len() >= 16, "header present");
        let kind = u64::from_le_bytes(blob[16..24].try_into().unwrap());
        let payload = u64::from_le_bytes(blob[24..32].try_into().unwrap());
        assert_eq!(kind, 0, "mask travels as packed bits, not a cell root");
        assert_eq!(payload, packed);
    }

    #[test]
    fn index_only_module_reports_cell_use_for_its_prelude() {
        // A module whose only cell-touching operation is indexing into a
        // view still needs the canonical slot helpers: every native backend
        // lowers SequenceProject to mncs_slot_load32/64. Missing helpers
        // surfaced as undeclared-function C failures and unresolved-symbol
        // JIT failures for pure index-into-view modules.
        use crate::scalar::{
            ScalarBlock, ScalarFunction, ScalarInst, ScalarModule, ScalarTerm, ScalarTy,
            ScalarValue,
        };
        use mncs_model::{BoundsEvidence, FailureMode, SemanticId, SequenceBound};
        let sid = |name: &str| SemanticId(name.to_owned());
        let project = ScalarInst::SequenceProject {
            dest: ScalarValue {
                id: sid("v"),
                ty: ScalarTy::Byte,
            },
            seq: sid("window"),
            index: sid("at"),
            bound: SequenceBound::UpTo(8),
            evidence: BoundsEvidence::RuntimeChecked {
                failure: FailureMode::Fatal,
            },
            width: crate::composite::SlotWidth::W32,
        };
        let module = ScalarModule {
            functions: vec![ScalarFunction {
                export_name: "pick".to_owned(),
                params: vec![ScalarValue {
                    id: sid("window"),
                    ty: ScalarTy::View,
                }],
                result: ScalarValue {
                    id: sid("v"),
                    ty: ScalarTy::Byte,
                },
                blocks: vec![ScalarBlock {
                    id: sid("entry"),
                    params: Vec::new(),
                    insts: vec![project],
                    term: ScalarTerm::Return { value: sid("v") },
                }],
                promises: Vec::new(),
                promise_decisions: Vec::new(),
            }],
            unsupported: Vec::new(),
            features: Vec::new(),
            promise_decisions: Vec::new(),
        };
        assert!(
            scalar_module_uses_cells(&module),
            "index-into-view needs the cell-helper prelude"
        );
        assert!(
            scalar_module_needs_arena_symbols(&module),
            "index-into-view needs arena symbols"
        );
    }
}
