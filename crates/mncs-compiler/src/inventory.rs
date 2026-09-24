//! Compiler-owned machine inventories.
//!
//! These records are deliberately built from compiler data tables and the
//! already-elaborated semantic model.  They are not source-name scans and
//! they do not encode policy belonging to an application such as Actions or
//! mncs-test.

use std::collections::BTreeSet;

use mncs_model::{
    Effect, GenericParam, GenericParamKind, SemanticId, Value, finite_type_id, function_id,
    module_id, program_id, record_type_id, test_case_id, test_declaration_id,
};
use mncs_syntax::{SourceSpan, source_profile_registry_snapshot};
use serde::Serialize;

use crate::{SourceFrontEndResult, fingerprint};

pub const LANGUAGE_INVENTORY_SCHEMA_VERSION: &str = "mncs.language-inventory/1";
pub const DECLARATION_INVENTORY_SCHEMA_VERSION: &str = "mncs.declaration-inventory/1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InventoryIntrinsic {
    pub name: String,
    pub kind: String,
    /// `None` means the compiler determines arity from an expected type (the
    /// public `vector` constructor has one operand per admitted lane).
    pub arity: Option<usize>,
    pub introduced_profile: String,
    pub effects: Vec<String>,
    pub capability_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompilerOperation {
    pub name: String,
    pub arity: usize,
    pub effect: String,
    pub capability_kind: String,
    pub introduced_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeForm {
    pub name: String,
    pub kind: String,
    pub introduced_profile: String,
    pub bounded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeclarationKind {
    pub name: String,
    pub callable: bool,
    pub introduced_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LanguageInventory {
    pub schema_version: &'static str,
    pub current_profile: String,
    pub profiles: Vec<mncs_syntax::SourceProfileSnapshot>,
    pub syntax_features: Vec<String>,
    pub type_forms: Vec<TypeForm>,
    pub intrinsics: Vec<InventoryIntrinsic>,
    pub effects: Vec<String>,
    pub capabilities: Vec<String>,
    pub compiler_operations: Vec<CompilerOperation>,
    pub declaration_kinds: Vec<DeclarationKind>,
    pub source_identity: String,
    pub inventory_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeclarationInventoryEntry {
    pub identity: SemanticId,
    pub kind: String,
    pub module: String,
    pub name: String,
    pub source_span: Option<SourceSpan>,
    pub exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CallableInventoryEntry {
    pub declaration_identity: SemanticId,
    pub callable_identity: SemanticId,
    /// Per-declaration typed signature identity emitted by elaborated
    /// inventories. Syntax-only projections cannot resolve imported nominal
    /// types and therefore serialize `null` here.
    pub signature_identity: Option<String>,
    pub callable_kind: String,
    pub module: String,
    pub name: String,
    pub qualified_name: String,
    pub source_span: SourceSpan,
    pub profile: String,
    pub generic_params: Vec<GenericParam>,
    pub inputs: Vec<Value>,
    pub outputs: Vec<Value>,
    pub effects: Vec<Effect>,
    pub capabilities: Vec<String>,
    pub exported: bool,
    pub test_case_identity: Option<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeclarationInventory {
    pub schema_version: &'static str,
    pub scope: String,
    pub module: String,
    pub source_artifact_identity: String,
    pub source_profile: String,
    pub subject_identity: SemanticId,
    pub subject_fingerprint: String,
    pub declarations: Vec<DeclarationInventoryEntry>,
    pub callables: Vec<CallableInventoryEntry>,
    pub inventory_identity: String,
}

/// Construct the public source-language inventory from explicit compiler
/// tables.  The table is intentionally boring: adding a public intrinsic is
/// a deliberate compiler change, and changing a private Rust helper name can
/// never publish a new language capability.
pub fn language_inventory() -> LanguageInventory {
    let profiles = source_profile_registry_snapshot();
    let current_profile = profiles
        .iter()
        .rev()
        .find(|profile| profile.status == mncs_syntax::ProfileStatus::Current)
        .map(|profile| profile.version.clone())
        .or_else(|| profiles.last().map(|profile| profile.version.clone()))
        .unwrap_or_else(|| "0.1".to_owned());

    let type_forms = vec![
        type_form("bool", "scalar", "0.1", false),
        type_form("signed_integer", "scalar_family", "0.1", false),
        type_form("unsigned_integer", "scalar_family", "0.1", false),
        type_form("byte", "scalar", "0.7", false),
        type_form("f64", "scalar", "0.12", false),
        type_form("finite", "nominal_sum", "0.2", false),
        type_form("record", "nominal_product", "0.5", false),
        type_form("sequence", "bounded_collection", "0.7", true),
        type_form("vector", "fixed_lane_collection", "0.8", true),
        type_form("mask", "fixed_lane_predicate", "0.8", true),
        type_form("generic_type_parameter", "generic", "0.10", false),
    ];

    let intrinsics = public_intrinsics();
    let compiler_operations = compiler_operations();
    let effects: Vec<String> = compiler_operations
        .iter()
        .map(|operation| operation.effect.clone())
        .chain(["host_read".to_owned(), "host_write".to_owned()])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let capabilities = vec![
        "declared_effect_capability".to_owned(),
        "process_grant".to_owned(),
        "structured_artifact_grant".to_owned(),
        "filesystem_root_grant".to_owned(),
        "crypto_grant".to_owned(),
        "clock_grant".to_owned(),
        "write_grant".to_owned(),
    ];
    let declaration_kinds = vec![
        declaration_kind("module", false, "0.1"),
        declaration_kind("import", false, "0.1"),
        declaration_kind("finite_type", false, "0.2"),
        declaration_kind("record_type", false, "0.5"),
        declaration_kind("function", true, "0.1"),
        declaration_kind("test", true, "0.17"),
    ];
    let syntax_features = profiles
        .iter()
        .flat_map(|profile| profile.features.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let source_material = serde_json::json!({
        "compiler": "mncs-compiler",
        "syntax_registry": profiles.clone(),
        "type_forms": type_forms.clone(),
        "intrinsics": intrinsics.clone(),
        "effects": effects.clone(),
        "capabilities": capabilities.clone(),
        "compiler_operations": compiler_operations.clone(),
        "declaration_kinds": declaration_kinds.clone(),
    });
    let source_identity = format!(
        "sha256:{}",
        mncs_model::sha256_hex(
            serde_json::to_vec(&source_material)
                .expect("language inventory source tables serialize")
                .as_slice()
        )
    );

    let mut inventory = LanguageInventory {
        schema_version: LANGUAGE_INVENTORY_SCHEMA_VERSION,
        current_profile,
        profiles,
        syntax_features,
        type_forms,
        intrinsics,
        effects,
        capabilities,
        compiler_operations,
        declaration_kinds,
        source_identity,
        inventory_identity: String::new(),
    };
    let mut identity_value =
        serde_json::to_value(&inventory).expect("language inventory serializes");
    identity_value
        .as_object_mut()
        .expect("language inventory is an object")
        .remove("inventory_identity");
    inventory.inventory_identity = format!(
        "sha256:{}",
        mncs_model::sha256_hex(
            serde_json::to_vec(&identity_value)
                .expect("language inventory identity serializes")
                .as_slice()
        )
    );
    inventory
}

fn type_form(name: &str, kind: &str, introduced_profile: &str, bounded: bool) -> TypeForm {
    TypeForm {
        name: name.to_owned(),
        kind: kind.to_owned(),
        introduced_profile: introduced_profile.to_owned(),
        bounded,
    }
}

fn declaration_kind(name: &str, callable: bool, introduced_profile: &str) -> DeclarationKind {
    DeclarationKind {
        name: name.to_owned(),
        callable,
        introduced_profile: introduced_profile.to_owned(),
    }
}

fn intrinsic(
    name: &str,
    kind: &str,
    arity: Option<usize>,
    introduced_profile: &str,
    effects: &[&str],
    capability_kind: Option<&str>,
) -> InventoryIntrinsic {
    InventoryIntrinsic {
        name: name.to_owned(),
        kind: kind.to_owned(),
        arity,
        introduced_profile: introduced_profile.to_owned(),
        effects: effects.iter().map(|effect| (*effect).to_owned()).collect(),
        capability_kind: capability_kind.map(str::to_owned),
    }
}

fn public_intrinsics() -> Vec<InventoryIntrinsic> {
    let mut values = vec![
        intrinsic("select", "selection", Some(3), "0.8", &[], None),
        intrinsic("replace", "sequence_update", Some(3), "0.8", &[], None),
        intrinsic("copy_span", "sequence_copy", Some(5), "0.14", &[], None),
        intrinsic("checked_index", "checked_index", Some(2), "0.14", &[], None),
        intrinsic("sin", "float", Some(1), "0.12", &[], None),
        intrinsic("cos", "float", Some(1), "0.12", &[], None),
        intrinsic("neg", "float", Some(1), "0.12", &[], None),
        intrinsic("vector", "vector", None, "0.8", &[], None),
        intrinsic("splat", "vector", Some(1), "0.8", &[], None),
        intrinsic("extract_lane", "vector", Some(2), "0.8", &[], None),
        intrinsic("replace_lane", "vector", Some(3), "0.8", &[], None),
    ];
    for name in [
        "vec_add_wrap",
        "vec_add_checked",
        "vec_add_sat",
        "vec_sub_wrap",
        "vec_sub_checked",
        "vec_sub_sat",
        "vec_mul_wrap",
        "vec_mul_checked",
        "vec_mul_sat",
        "vec_and",
        "vec_or",
        "vec_xor",
        "vec_shl",
        "vec_shr",
        "vec_min",
        "vec_max",
        "vec_eq",
        "vec_ne",
        "vec_lt",
        "vec_le",
        "vec_gt",
        "vec_ge",
        "mask_and",
        "mask_or",
        "mask_xor",
        "mask_not",
        "mask_any",
        "mask_all",
        "mask_none",
        "reduce_sum_wrap",
        "reduce_sum_checked",
        "reduce_min",
        "reduce_max",
    ] {
        let arity = match name {
            "mask_not" | "mask_any" | "mask_all" | "mask_none" | "reduce_sum_wrap"
            | "reduce_sum_checked" | "reduce_min" | "reduce_max" => Some(1),
            _ => Some(2),
        };
        values.push(intrinsic(name, "vector_or_mask", arity, "0.8", &[], None));
    }
    values.sort_by(|left, right| left.name.cmp(&right.name));
    values
}

fn compiler_operations() -> Vec<CompilerOperation> {
    let definitions = [
        ("blob_read", 0, "host_read", "write_grant", "0.8"),
        ("blob_append", 1, "host_write", "write_grant", "0.12"),
        (
            "host_read",
            0,
            "host_read",
            "declared_effect_capability",
            "0.8",
        ),
        ("host_write", 1, "host_write", "write_grant", "0.12"),
        ("clock_read", 0, "clock_read", "clock_grant", "0.8"),
        ("sha256_digest", 1, "sha256_digest", "crypto_grant", "0.8"),
        (
            "structured_digest",
            1,
            "structured_digest",
            "structured_artifact_grant",
            "0.16",
        ),
        ("process_run", 1, "process_run", "process_grant", "0.16"),
        ("process_start", 1, "process_run", "process_grant", "0.18"),
        ("process_observe", 1, "process_run", "process_grant", "0.18"),
        ("process_cancel", 1, "process_run", "process_grant", "0.18"),
        ("process_reap", 1, "process_run", "process_grant", "0.18"),
        (
            "structured_read",
            2,
            "structured_read",
            "structured_artifact_grant",
            "0.18",
        ),
        (
            "structured_read_identity",
            3,
            "structured_read",
            "structured_artifact_grant",
            "0.18",
        ),
        (
            "provider_call",
            2,
            "provider_call",
            "declared_effect_capability",
            "0.18",
        ),
        (
            "structured_write",
            3,
            "structured_write",
            "structured_artifact_grant",
            "0.18",
        ),
        ("ed25519_verify", 3, "ed25519_verify", "crypto_grant", "0.8"),
        (
            "fs_list_count",
            0,
            "fs_list",
            "filesystem_root_grant",
            "0.12",
        ),
        (
            "fs_entry_name_at",
            1,
            "fs_list",
            "filesystem_root_grant",
            "0.12",
        ),
        (
            "fs_entry_kind_at",
            1,
            "fs_list",
            "filesystem_root_grant",
            "0.12",
        ),
        (
            "fs_entry_size_at",
            1,
            "fs_list",
            "filesystem_root_grant",
            "0.18",
        ),
        (
            "fs_entry_mtime_at",
            1,
            "fs_list",
            "filesystem_root_grant",
            "0.18",
        ),
        (
            "fs_generation",
            0,
            "fs_list",
            "filesystem_root_grant",
            "0.12",
        ),
        (
            "fs_read_bytes_at",
            3,
            "fs_read",
            "filesystem_root_grant",
            "0.12",
        ),
        (
            "fs_create_file",
            2,
            "fs_write",
            "filesystem_root_grant",
            "0.16",
        ),
        (
            "fs_write_bytes_at",
            3,
            "fs_write",
            "filesystem_root_grant",
            "0.16",
        ),
        (
            "fs_append_bytes_at",
            2,
            "fs_write",
            "filesystem_root_grant",
            "0.16",
        ),
        ("fs_mkdir", 1, "fs_write", "filesystem_root_grant", "0.16"),
        (
            "fs_delete_at",
            1,
            "fs_write",
            "filesystem_root_grant",
            "0.16",
        ),
        (
            "fs_rename_at",
            2,
            "fs_write",
            "filesystem_root_grant",
            "0.16",
        ),
        ("fs_sync_at", 1, "fs_write", "filesystem_root_grant", "0.16"),
    ];
    definitions
        .into_iter()
        .map(
            |(name, arity, effect, capability_kind, introduced_profile)| CompilerOperation {
                name: name.to_owned(),
                arity,
                effect: effect.to_owned(),
                capability_kind: capability_kind.to_owned(),
                introduced_profile: introduced_profile.to_owned(),
            },
        )
        .collect()
}

/// Build a generic source declaration/callable inventory from the compiler's
/// parsed AST and elaborated semantic identities.  No consumer-specific
/// dispatch or test policy is added here.
pub fn declaration_inventory(front_end: &SourceFrontEndResult) -> Option<DeclarationInventory> {
    let ast = front_end.ast.as_ref()?;
    let program = front_end.program.as_ref()?;
    if !front_end.is_valid() {
        return None;
    }
    let identities = front_end.identities.as_ref()?;
    // Keep the source subject identity compatible with the established
    // test-inventory contract.  Declarations still carry module identities;
    // the inventory subject represents the complete source program whose
    // callable surface is being described.
    let subject_identity = program_id(&program.module);
    let subject_fingerprint = program.production_content_fingerprint().ok()?;
    let mut declarations = vec![DeclarationInventoryEntry {
        identity: subject_identity.clone(),
        kind: "module".to_owned(),
        module: program.module.clone(),
        name: program.module.clone(),
        source_span: Some(ast.span),
        exported: true,
    }];
    for use_decl in &ast.uses {
        declarations.push(DeclarationInventoryEntry {
            identity: module_id(&use_decl.module.text),
            kind: "import".to_owned(),
            module: program.module.clone(),
            name: use_decl.module.text.clone(),
            source_span: Some(use_decl.span),
            exported: false,
        });
    }
    for finite in &program.finite_types {
        declarations.push(DeclarationInventoryEntry {
            identity: finite.identity.clone(),
            kind: "finite_type".to_owned(),
            module: program.module.clone(),
            name: finite.name.clone(),
            source_span: ast
                .finite_types
                .iter()
                .find(|candidate| candidate.name.text == finite.name)
                .map(|candidate| candidate.span),
            exported: true,
        });
    }
    for record in &program.record_types {
        declarations.push(DeclarationInventoryEntry {
            identity: record.identity.clone(),
            kind: "record_type".to_owned(),
            module: program.module.clone(),
            name: record.name.clone(),
            source_span: ast
                .record_types
                .iter()
                .find(|candidate| candidate.name.text == record.name)
                .map(|candidate| candidate.span),
            exported: true,
        });
    }

    let mut callables = Vec::new();
    let abi_functions = mncs_codegen::language_owned_abi_contracts(&program).0;
    for ast_function in &ast.functions {
        let Some(function) = program.functions.iter().find(|candidate| {
            candidate.home_module.is_none() && candidate.name == ast_function.name.text
        }) else {
            continue;
        };
        let callable_identity = function_id(&program.module, &function.name);
        let signature_identity = abi_functions
            .values()
            .find(|entry| entry.function_identity == callable_identity)
            .map(|entry| entry.signature_identity.clone());
        let declaration_identity = if function.is_test {
            test_declaration_id(&program.module, &function.name)
        } else {
            callable_identity.clone()
        };
        let semantic_fingerprint = identities
            .fingerprint(&callable_identity)
            .unwrap_or_default();
        let test_case_identity = function
            .is_test
            .then(|| test_case_id(&program.module, &function.name, semantic_fingerprint));
        declarations.push(DeclarationInventoryEntry {
            identity: declaration_identity.clone(),
            kind: if function.is_test { "test" } else { "function" }.to_owned(),
            module: program.module.clone(),
            name: function.name.clone(),
            source_span: Some(ast_function.span),
            exported: true,
        });
        callables.push(CallableInventoryEntry {
            declaration_identity,
            callable_identity,
            signature_identity,
            callable_kind: if function.is_test { "test" } else { "function" }.to_owned(),
            module: program.module.clone(),
            name: function.name.clone(),
            qualified_name: format!("{}::{}", program.module, function.name),
            source_span: ast_function.span,
            profile: ast.language_version.text.clone(),
            generic_params: function.generic_params.clone(),
            inputs: function.inputs.clone(),
            outputs: function.outputs.clone(),
            effects: function.effects.clone(),
            capabilities: function.capabilities.clone(),
            exported: true,
            test_case_identity,
        });
    }
    declarations.sort_by(|left, right| left.identity.cmp(&right.identity));
    callables.sort_by(|left, right| left.callable_identity.cmp(&right.callable_identity));

    let mut inventory = DeclarationInventory {
        schema_version: DECLARATION_INVENTORY_SCHEMA_VERSION,
        scope: "source_module".to_owned(),
        module: program.module.clone(),
        source_artifact_identity: front_end.envelope.identity.clone(),
        source_profile: ast.language_version.text.clone(),
        subject_identity,
        subject_fingerprint,
        declarations,
        callables,
        inventory_identity: String::new(),
    };
    let mut identity_value = serde_json::to_value(&inventory).ok()?;
    identity_value.as_object_mut()?.remove("inventory_identity");
    inventory.inventory_identity = format!(
        "sha256:{}",
        mncs_model::sha256_hex(serde_json::to_vec(&identity_value).ok()?.as_slice())
    );
    Some(inventory)
}

/// Parse-only declaration inventory used for large library modules whose
/// imported implementation closure is not needed to answer declaration and
/// callable-surface questions.  The parser and identity constructors remain
/// compiler-owned; this is not a host regex fallback.  Full inventories use
/// [`declaration_inventory`] and include elaborated types/evidence.
pub fn declaration_inventory_from_syntax(
    front_end: &SourceFrontEndResult,
) -> Option<DeclarationInventory> {
    let ast = front_end.ast.as_ref()?;
    declaration_inventory_from_ast(&front_end.envelope, ast)
}

/// Build the same parse-only inventory directly from the syntax parser.  This
/// lets tooling inspect a module's declaration surface without elaborating a
/// potentially large import closure.
pub fn declaration_inventory_from_ast(
    envelope: &mncs_syntax::SourceEnvelope,
    ast: &mncs_syntax::AbstractSyntaxTree,
) -> Option<DeclarationInventory> {
    let module = ast.module.text.clone();
    let subject_identity = program_id(&module);
    let subject_fingerprint = fingerprint(ast);
    let mut declarations = vec![DeclarationInventoryEntry {
        identity: subject_identity.clone(),
        kind: "module".to_owned(),
        module: module.clone(),
        name: module.clone(),
        source_span: Some(ast.span),
        exported: true,
    }];
    for use_decl in &ast.uses {
        declarations.push(DeclarationInventoryEntry {
            identity: module_id(&use_decl.module.text),
            kind: "import".to_owned(),
            module: module.clone(),
            name: use_decl.module.text.clone(),
            source_span: Some(use_decl.span),
            exported: false,
        });
    }
    for finite in &ast.finite_types {
        declarations.push(DeclarationInventoryEntry {
            identity: finite_type_id(&module, &finite.name.text),
            kind: "finite_type".to_owned(),
            module: module.clone(),
            name: finite.name.text.clone(),
            source_span: Some(finite.span),
            exported: true,
        });
    }
    for record in &ast.record_types {
        let fields = record
            .fields
            .iter()
            .map(|field| (field.name.text.as_str(), field.value_type.text.as_str()))
            .collect::<Vec<_>>();
        declarations.push(DeclarationInventoryEntry {
            identity: record_type_id(&module, &record.name.text, &fields),
            kind: "record_type".to_owned(),
            module: module.clone(),
            name: record.name.text.clone(),
            source_span: Some(record.span),
            exported: true,
        });
    }

    let mut callables = Vec::new();
    for function in &ast.functions {
        let callable_identity = function_id(&module, &function.name.text);
        let declaration_identity = if function.is_test {
            test_declaration_id(&module, &function.name.text)
        } else {
            callable_identity.clone()
        };
        let declaration_fingerprint = fingerprint(function);
        let test_case_identity = function
            .is_test
            .then(|| test_case_id(&module, &function.name.text, &declaration_fingerprint));
        let generic_params = function
            .generic_params
            .iter()
            .map(|parameter| GenericParam {
                name: parameter.name.text.clone(),
                kind: match parameter
                    .constraint
                    .as_ref()
                    .map(|constraint| constraint.text.as_str())
                {
                    Some("Nat") => GenericParamKind::Nat,
                    _ => GenericParamKind::Type,
                },
            })
            .collect();
        let inputs = function
            .inputs
            .iter()
            .map(|input| Value {
                name: input.name.text.clone(),
                value_type: input.value_type.text.clone(),
            })
            .collect();
        let outputs = function
            .outputs
            .iter()
            .map(|output| Value {
                name: output.name.text.clone(),
                value_type: output.value_type.text.clone(),
            })
            .collect();
        let capabilities = function
            .capabilities
            .iter()
            .map(|capability| capability.text.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let effects = function
            .effects
            .iter()
            .map(|effect| Effect {
                kind: effect.kind.text.clone(),
                target: function.name.text.clone(),
                capability: effect.capability.text.clone(),
            })
            .collect();
        declarations.push(DeclarationInventoryEntry {
            identity: declaration_identity.clone(),
            kind: if function.is_test { "test" } else { "function" }.to_owned(),
            module: module.clone(),
            name: function.name.text.clone(),
            source_span: Some(function.span),
            exported: true,
        });
        callables.push(CallableInventoryEntry {
            declaration_identity,
            callable_identity,
            signature_identity: None,
            callable_kind: if function.is_test { "test" } else { "function" }.to_owned(),
            module: module.clone(),
            name: function.name.text.clone(),
            qualified_name: format!("{module}::{}", function.name.text),
            source_span: function.span,
            profile: ast.language_version.text.clone(),
            generic_params,
            inputs,
            outputs,
            effects,
            capabilities,
            exported: true,
            test_case_identity,
        });
    }
    declarations.sort_by(|left, right| left.identity.cmp(&right.identity));
    callables.sort_by(|left, right| left.callable_identity.cmp(&right.callable_identity));

    let mut inventory = DeclarationInventory {
        schema_version: DECLARATION_INVENTORY_SCHEMA_VERSION,
        scope: "source_module_syntax".to_owned(),
        module,
        source_artifact_identity: envelope.identity.clone(),
        source_profile: ast.language_version.text.clone(),
        subject_identity,
        subject_fingerprint,
        declarations,
        callables,
        inventory_identity: String::new(),
    };
    let mut identity_value = serde_json::to_value(&inventory).ok()?;
    identity_value.as_object_mut()?.remove("inventory_identity");
    inventory.inventory_identity = format!(
        "sha256:{}",
        mncs_model::sha256_hex(serde_json::to_vec(&identity_value).ok()?.as_slice())
    );
    Some(inventory)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mncs_syntax::{SourceArtifactKind, SourceEnvelope};

    #[test]
    fn language_inventory_is_deterministic_and_excludes_private_helper_names() {
        let left = language_inventory();
        let right = language_inventory();
        assert_eq!(left, right);
        assert!(
            left.intrinsics
            .iter()
                .all(|intrinsic| !intrinsic.name.starts_with("elaborate_"))
        );
        assert_eq!(
            left.current_profile,
            left.profiles.last().expect("registry is non-empty").version
        );
    }

    #[test]
    fn declaration_inventory_separates_callable_and_test_identities() {
        let source = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "inventory",
            "mncs 0.17; module inventory.example; fn run(value: i64) -> (result: i64) { return value; } test check() -> (result: i64) { return 1; }",
        );
        let front_end = crate::ReferenceCompiler::default().front_end(source);
        let inventory = declaration_inventory(&front_end).expect("valid inventory");
        let run = inventory
            .callables
            .iter()
            .find(|callable| callable.name == "run")
            .expect("run callable");
        let check = inventory
            .callables
            .iter()
            .find(|callable| callable.name == "check")
            .expect("check callable");
        assert_eq!(run.callable_kind, "function");
        assert_eq!(check.callable_kind, "test");
        assert_ne!(run.declaration_identity, check.declaration_identity);
        assert!(check.test_case_identity.is_some());
    }
}
