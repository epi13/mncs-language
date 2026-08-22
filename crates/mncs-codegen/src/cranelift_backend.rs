//! Cranelift realization of selected SSA.
//!
//! The artifact is deterministic CLIF text. Embedded execution compiles
//! that CLIF with Cranelift for the host ISA. Host ISA is a target fact,
//! not the MNCS machine model.

use std::collections::BTreeMap;
use std::fmt::Write;

use mncs_model::{
    ArithmeticIntent, BackendCapabilityManifest, BackendConfiguration, BackendEvidence,
    BackendIdentity, BackendResult, CompilerArtifactRef, CompilerDiagnostic,
    CompilerDiagnosticKind, ExecutionRequest, ExecutionStatus, Program, SsaModule,
    TargetContractRef, TargetLoweringPlan, TransformationStatus, SSA_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};

use crate::native::{argv_from_request, decode_native_value, host_triple};
use crate::scalar::{
    lower_to_scalar, ScalarFunction, ScalarInst, ScalarModule, ScalarTerm, ScalarTy,
};
use crate::support::{
    artifact_ref, empty_execution, execution_failure, function_names, function_value_contracts,
    unknown, validate_selected_ssa,
};
use crate::{BackendAdapter, BackendExecutionResult};

pub const CRANELIFT_BACKEND_NAME: &str = "mncs-cranelift";
pub const CRANELIFT_BACKEND_VERSION: &str = "0.1";
pub const CRANELIFT_TARGET: &str = "mncs:target:cranelift-0.1";
pub const CRANELIFT_FORMAT: &str = "application/vnd.mncs.cranelift-clif+json; version=0.1";
pub const CRANELIFT_ARTIFACT_KIND: &str = "cranelift_clif";

pub struct CraneliftAdapter;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CraneliftPayload {
    schema_version: String,
    clif: String,
    program: Program,
    ssa: SsaModule,
    exports: Vec<String>,
    arities: BTreeMap<String, usize>,
}

pub fn cranelift_backend() -> BackendIdentity {
    BackendIdentity::new(CRANELIFT_BACKEND_NAME, CRANELIFT_BACKEND_VERSION)
}

pub fn cranelift_configuration() -> BackendConfiguration {
    BackendConfiguration {
        backend: cranelift_backend(),
        options: BTreeMap::from([
            ("isa".to_owned(), host_triple().to_owned()),
            ("opt-level".to_owned(), "none".to_owned()),
        ]),
        target_features: ["scalar-ssa", "host-isa-jit", "no-memory"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        linker_toolchain: None,
        assumptions: vec![
            "Cranelift CLIF/JIT is a realization, not MNCS semantics".to_owned(),
            "JIT uses the host ISA advertised as a target fact, not a language machine model"
                .to_owned(),
            "only the declared scalar selected-SSA envelope is lowered".to_owned(),
        ],
    }
}

pub fn cranelift_capabilities() -> BackendCapabilityManifest {
    BackendCapabilityManifest::new(
        cranelift_backend(),
        "mncs:selected-ssa-to-cranelift:0.1",
        [CRANELIFT_TARGET.to_owned(), host_triple().to_owned()]
            .into_iter()
            .collect(),
        [SSA_SCHEMA_VERSION.to_owned()].into_iter().collect(),
        ["data-layout", "abi", "integer", "trap", "isa"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        [
            "checked_integer",
            "wrapping_integer",
            "trapping_integer",
            "explicit_failure",
            "semantic_bounded_iteration",
            "finite_values",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        [
            "memory",
            "effects",
            "saturating_integer",
            "widening_integer",
            "non_host_isa_jit",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        [CRANELIFT_ARTIFACT_KIND.to_owned()].into_iter().collect(),
        ["bounded_execution_agreement".to_owned()]
            .into_iter()
            .collect(),
        ["cranelift_host_jit".to_owned()].into_iter().collect(),
        ["status_value_out_pointer_abi".to_owned()]
            .into_iter()
            .collect(),
        "checked/trapping overflow and declared failure set status=1",
        true,
    )
}

pub fn cranelift_target() -> TargetContractRef {
    TargetContractRef::new(
        CRANELIFT_TARGET,
        BTreeMap::from([
            (
                "data-layout".to_owned(),
                "Cranelift scalar integers; no language-owned aggregate layout".to_owned(),
            ),
            (
                "abi".to_owned(),
                "native Cranelift systemv; status/value out pointers".to_owned(),
            ),
            (
                "integer".to_owned(),
                "wrapping iadd/isub/imul; checked via i64 range tests".to_owned(),
            ),
            ("trap".to_owned(), "status=1".to_owned()),
            ("isa".to_owned(), host_triple().to_owned()),
        ]),
        vec![mncs_model::SemanticId(
            "mncs:target-evidence:cranelift-0.1:declared-research-contract".to_owned(),
        )],
        vec!["Cranelift host JIT is one realization of selected SSA".to_owned()],
    )
}

pub fn cranelift_plan(selected_ssa: CompilerArtifactRef) -> TargetLoweringPlan {
    TargetLoweringPlan::with_explicit_facts(
        selected_ssa,
        cranelift_target(),
        Some(cranelift_configuration()),
        vec!["Cranelift block parameters realize MNCS SSA block parameters".to_owned()],
        vec!["private status/value out-pointer ABI".to_owned()],
        BTreeMap::from([
            (
                "wrapping".to_owned(),
                "clif iadd/isub/imul/band/bor/bxor".to_owned(),
            ),
            (
                "checked".to_owned(),
                "i64 range test then status=1".to_owned(),
            ),
        ]),
        BTreeMap::from([
            ("overflow".to_owned(), "status=1".to_owned()),
            ("failure_terminator".to_owned(), "status=1".to_owned()),
        ]),
        Vec::new(),
        Vec::new(),
        TransformationStatus::Pass,
    )
}

pub fn target_is_cranelift(target: &TargetContractRef) -> bool {
    target.candidate == CRANELIFT_TARGET
        && ["data-layout", "abi", "integer", "trap", "isa"]
            .iter()
            .all(|fact| target.facts.contains_key(*fact))
        && !target.evidence.is_empty()
}

impl BackendAdapter for CraneliftAdapter {
    fn capabilities(&self) -> BackendCapabilityManifest {
        cranelift_capabilities()
    }
    fn target(&self) -> TargetContractRef {
        cranelift_target()
    }
    fn configuration(&self) -> BackendConfiguration {
        cranelift_configuration()
    }
    fn plan(&self, selected_ssa: CompilerArtifactRef) -> TargetLoweringPlan {
        cranelift_plan(selected_ssa)
    }
    fn lower(
        &self,
        program: &Program,
        ssa: &SsaModule,
        selected_ssa: CompilerArtifactRef,
        plan: &TargetLoweringPlan,
    ) -> BackendResult {
        lower_cranelift(program, ssa, selected_ssa, plan)
    }
    fn execute(
        &self,
        artifact: &mncs_model::BackendArtifact,
        request: &ExecutionRequest,
    ) -> BackendExecutionResult {
        execute_cranelift(artifact, request)
    }
}

pub fn lower_cranelift(
    program: &Program,
    ssa: &SsaModule,
    selected_ssa: CompilerArtifactRef,
    plan: &TargetLoweringPlan,
) -> BackendResult {
    if let Err(result) = validate_selected_ssa(ssa, &selected_ssa, "CGF101") {
        return *result;
    }
    if !target_is_cranelift(&plan.target) {
        return unknown(vec![CompilerDiagnostic::new(
            "CGF201",
            CompilerDiagnosticKind::MissingTargetEvidence,
            "Cranelift lowering requires explicit data-layout, ABI, integer, trap, and isa facts",
        )]);
    }
    if plan.status != TransformationStatus::Pass {
        return unknown(vec![CompilerDiagnostic::new(
            "CGF202",
            CompilerDiagnosticKind::MissingTargetEvidence,
            "Cranelift will not realize a non-PASS target plan",
        )]);
    }
    let names = function_names(program, ssa);
    let scalar = lower_to_scalar(program, ssa, &names);
    if !scalar.unsupported.is_empty() || scalar.functions.is_empty() {
        let mut diagnostics = vec![CompilerDiagnostic::new(
            "CGF301",
            CompilerDiagnosticKind::UnavailableBackendCapability,
            "selected SSA is outside the Cranelift scalar envelope",
        )];
        for reason in scalar.unsupported {
            diagnostics.push(CompilerDiagnostic::new(
                "CGF302",
                CompilerDiagnosticKind::UnavailableBackendCapability,
                reason,
            ));
        }
        return unknown(diagnostics);
    }
    let payload = CraneliftPayload {
        schema_version: "0.1".to_owned(),
        clif: emit_clif(&scalar),
        program: program.clone(),
        ssa: ssa.clone(),
        exports: scalar
            .functions
            .iter()
            .map(|function| function.export_name.clone())
            .collect(),
        arities: scalar
            .functions
            .iter()
            .map(|function| (function.export_name.clone(), function.params.len()))
            .collect(),
    };
    let bytes = match serde_json::to_vec(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            return crate::support::failed(vec![CompilerDiagnostic::new(
                "CGF301",
                CompilerDiagnosticKind::InternalCompilerDefect,
                format!("Cranelift payload serialization failed: {error}"),
            )]);
        }
    };
    let mut assumptions = plan.assumptions_introduced.clone();
    assumptions.extend(
        scalar
            .functions
            .iter()
            .flat_map(|function| function.promises.clone()),
    );
    let artifact = mncs_model::BackendArtifact::new_with_kind(
        cranelift_backend(),
        selected_ssa.clone(),
        plan.target.clone(),
        CRANELIFT_ARTIFACT_KIND,
        CRANELIFT_FORMAT,
        &bytes,
        payload.exports.clone(),
        assumptions.clone(),
        Vec::new(),
        vec![
            "cranelift host JIT".to_owned(),
            "inspectable CLIF text".to_owned(),
        ],
        plan.target.evidence.clone(),
        Vec::new(),
        TransformationStatus::Pass,
    )
    .with_function_value_contracts(function_value_contracts(program));
    let artifact_ref = artifact_ref(&artifact);
    let evidence = BackendEvidence::new(
        cranelift_backend(),
        selected_ssa,
        artifact_ref.clone(),
        assumptions,
        plan.target.evidence.clone(),
        TransformationStatus::Pass,
    );
    BackendResult {
        status: TransformationStatus::Pass,
        artifact: Some(artifact),
        artifact_ref: Some(artifact_ref),
        evidence: Some(evidence),
        diagnostics: Vec::new(),
    }
}

fn emit_clif(module: &ScalarModule) -> String {
    let mut out = String::from("; MNCS Cranelift CLIF 0.1. Not MNCS semantics.\n");
    for function in &module.functions {
        emit_clif_function(&mut out, function);
        out.push('\n');
    }
    out
}

fn emit_clif_function(out: &mut String, function: &ScalarFunction) {
    let names = ClifNames::new(function);
    let mut sig = function
        .params
        .iter()
        .map(|param| clif_ty(param.ty).to_owned())
        .collect::<Vec<_>>();
    sig.push("i64".to_owned());
    sig.push("i64".to_owned());
    let _ = writeln!(
        out,
        "function %{}({}) {{",
        function.export_name,
        sig.join(", ")
    );
    for (index, block) in function.blocks.iter().enumerate() {
        let params = if index == 0 {
            let mut list = function
                .params
                .iter()
                .map(|param| format!("{}: {}", names.value(&param.id), clif_ty(param.ty)))
                .collect::<Vec<_>>();
            list.push("st: i64".to_owned());
            list.push("val: i64".to_owned());
            list.join(", ")
        } else {
            block
                .params
                .iter()
                .map(|param| format!("{}: {}", names.value(&param.id), clif_ty(param.ty)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let _ = writeln!(out, "    {}({params}):", names.block(&block.id));
        for inst in &block.insts {
            emit_clif_inst(out, inst, &names);
        }
        match &block.term {
            ScalarTerm::Return { value } => {
                out.push_str("        v_ok = iconst.i32 0\n");
                let _ = writeln!(out, "        store.i32 v_ok, st");
                let _ = writeln!(out, "        v_ret = sextend.i64 {}", names.value(value));
                out.push_str("        store.i64 v_ret, val\n        return\n");
            }
            ScalarTerm::Jump { target, args } => {
                let list = args
                    .iter()
                    .map(|arg| names.value(arg).to_owned())
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(out, "        jump {}({list})", names.block(target));
            }
            ScalarTerm::Branch {
                cond,
                then_target,
                then_args,
                else_target,
                else_args,
            } => {
                let then_list = then_args
                    .iter()
                    .map(|arg| names.value(arg).to_owned())
                    .collect::<Vec<_>>()
                    .join(", ");
                let else_list = else_args
                    .iter()
                    .map(|arg| names.value(arg).to_owned())
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    out,
                    "        brnz {}, {}({then_list}), {}({else_list})",
                    names.value(cond),
                    names.block(then_target),
                    names.block(else_target)
                );
            }
            ScalarTerm::Fail => {
                out.push_str("        v_bad = iconst.i32 1\n        v_z = iconst.i64 0\n");
                out.push_str(
                    "        store.i32 v_bad, st\n        store.i64 v_z, val\n        return\n",
                );
            }
        }
    }
    out.push_str("}\n");
}

fn emit_clif_inst(out: &mut String, inst: &ScalarInst, names: &ClifNames) {
    match inst {
        ScalarInst::Const { dest, value } => {
            let _ = writeln!(
                out,
                "        {} = iconst.{} {value}",
                names.value(&dest.id),
                clif_ty(dest.ty)
            );
        }
        ScalarInst::Integer {
            dest,
            operator,
            intent,
            lhs,
            rhs,
            promise,
        } => {
            let dest_n = names.value(&dest.id);
            let lhs_n = names.value(lhs);
            let rhs_n = names.value(rhs);
            let ty = clif_ty(dest.ty);
            if matches!(operator.as_str(), "and" | "or" | "xor") {
                let op = match operator.as_str() {
                    "and" => "band",
                    "or" => "bor",
                    _ => "bxor",
                };
                let _ = writeln!(out, "        {dest_n} = {op} {lhs_n}, {rhs_n}");
                return;
            }
            let op = match operator.as_str() {
                "sub" => "isub",
                "mul" => "imul",
                _ => "iadd",
            };
            if matches!(
                intent,
                ArithmeticIntent::Checked | ArithmeticIntent::Trapping
            ) && !promise.decision.permitted
            {
                let max = match dest.ty {
                    ScalarTy::Int(integer) if integer.signed => (1i128 << (integer.bits - 1)) - 1,
                    ScalarTy::Int(integer) => (1i128 << integer.bits) - 1,
                    _ => i128::from(i32::MAX),
                };
                let min = match dest.ty {
                    ScalarTy::Int(integer) if integer.signed => -(1i128 << (integer.bits - 1)),
                    _ => 0,
                };
                let _ = writeln!(out, "        {dest_n}_l = sextend.i64 {lhs_n}");
                let _ = writeln!(out, "        {dest_n}_r = sextend.i64 {rhs_n}");
                let _ = writeln!(out, "        {dest_n}_w = {op} {dest_n}_l, {dest_n}_r");
                let _ = writeln!(out, "        {dest_n}_max = iconst.i64 {max}");
                let _ = writeln!(out, "        {dest_n}_min = iconst.i64 {min}");
                let _ = writeln!(
                    out,
                    "        {dest_n}_hi = icmp sgt {dest_n}_w, {dest_n}_max"
                );
                let _ = writeln!(
                    out,
                    "        {dest_n}_lo = icmp slt {dest_n}_w, {dest_n}_min"
                );
                let _ = writeln!(out, "        {dest_n}_ov = bor {dest_n}_hi, {dest_n}_lo");
                let _ = writeln!(out, "        brnz {dest_n}_ov, fail_{dest_n}");
                let _ = writeln!(out, "        {dest_n} = ireduce.{ty} {dest_n}_w");
                let _ = writeln!(out, "        jump ok_{dest_n}");
                let _ = writeln!(out, "    fail_{dest_n}:");
                out.push_str("        v_bad = iconst.i32 1\n        v_z = iconst.i64 0\n");
                out.push_str(
                    "        store.i32 v_bad, st\n        store.i64 v_z, val\n        return\n",
                );
                let _ = writeln!(out, "    ok_{dest_n}:");
            } else {
                let _ = writeln!(out, "        {dest_n} = {op} {lhs_n}, {rhs_n}");
            }
        }
        ScalarInst::Compare {
            dest,
            predicate,
            operand,
            lhs,
            rhs,
        } => {
            let cond = match (predicate.as_str(), operand.signed) {
                ("eq", _) => "eq",
                ("ne", _) => "ne",
                ("lt", true) => "slt",
                ("le", true) => "sle",
                ("gt", true) => "sgt",
                ("ge", true) => "sge",
                ("lt", false) => "ult",
                ("le", false) => "ule",
                ("gt", false) => "ugt",
                _ => "uge",
            };
            let _ = writeln!(
                out,
                "        {} = icmp {cond} {}, {}",
                names.value(&dest.id),
                names.value(lhs),
                names.value(rhs)
            );
        }
        ScalarInst::FiniteConstruct { dest, discriminant } => {
            let _ = writeln!(
                out,
                "        {} = iconst.i32 {discriminant}",
                names.value(&dest.id)
            );
        }
        ScalarInst::FiniteIsVariant {
            dest,
            src,
            discriminant,
        } => {
            let _ = writeln!(
                out,
                "        {} = icmp eq {}, {discriminant}",
                names.value(&dest.id),
                names.value(src)
            );
        }
        ScalarInst::Call { dest, callee, args } => {
            let list = args
                .iter()
                .map(|arg| names.value(arg).to_owned())
                .chain(["st".to_owned(), "val".to_owned()])
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "        call %{callee}({list})");
            let _ = writeln!(out, "        {} = load.i32 val", names.value(&dest.id));
        }
    }
}

fn clif_ty(ty: ScalarTy) -> &'static str {
    match ty {
        ScalarTy::Int(integer) if integer.bits == 64 => "i64",
        _ => "i32",
    }
}

struct ClifNames {
    values: BTreeMap<mncs_model::SemanticId, String>,
    blocks: BTreeMap<mncs_model::SemanticId, String>,
}

impl ClifNames {
    fn new(function: &ScalarFunction) -> Self {
        let mut values = BTreeMap::new();
        let mut next = 0u32;
        let mut push = |id: &mncs_model::SemanticId| {
            values.entry(id.clone()).or_insert_with(|| {
                let name = format!("v{next}");
                next += 1;
                name
            });
        };
        for param in &function.params {
            push(&param.id);
        }
        for block in &function.blocks {
            for param in &block.params {
                push(&param.id);
            }
            for inst in &block.insts {
                let dest = match inst {
                    ScalarInst::Const { dest, .. }
                    | ScalarInst::Integer { dest, .. }
                    | ScalarInst::Compare { dest, .. }
                    | ScalarInst::FiniteConstruct { dest, .. }
                    | ScalarInst::FiniteIsVariant { dest, .. }
                    | ScalarInst::Call { dest, .. } => dest,
                };
                push(&dest.id);
            }
        }
        let blocks = function
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (block.id.clone(), format!("block{index}")))
            .collect();
        Self { values, blocks }
    }

    fn value(&self, id: &mncs_model::SemanticId) -> &str {
        self.values.get(id).map_or("v_missing", String::as_str)
    }

    fn block(&self, id: &mncs_model::SemanticId) -> &str {
        self.blocks.get(id).map_or("block0", String::as_str)
    }
}

pub fn execute_cranelift(
    artifact: &mncs_model::BackendArtifact,
    request: &ExecutionRequest,
) -> BackendExecutionResult {
    let mut result = empty_execution(artifact, request);
    if let Err(reason) = crate::support::backend_matches_identity(
        artifact,
        &cranelift_backend(),
        CRANELIFT_ARTIFACT_KIND,
    ) {
        return execution_failure(result, ExecutionStatus::InvalidRequest, reason);
    }
    let bytes = match artifact.bytes() {
        Ok(bytes) => bytes,
        Err(reason) => return execution_failure(result, ExecutionStatus::InvalidRequest, reason),
    };
    let payload: CraneliftPayload = match serde_json::from_slice(&bytes) {
        Ok(payload) => payload,
        Err(error) => {
            return execution_failure(
                result,
                ExecutionStatus::InvalidRequest,
                format!("invalid Cranelift payload: {error}"),
            );
        }
    };
    let Some(contract) = artifact
        .function_value_contracts
        .get(&request.target.function)
    else {
        return execution_failure(
            result,
            ExecutionStatus::InvalidRequest,
            "Cranelift execution requires a language-owned function value contract",
        );
    };
    match jit_execute(&payload, request) {
        Ok((status, value)) => {
            result.status = status;
            result.steps = 1;
            match decode_native_value(contract.outputs.first(), status, value) {
                Ok(returned) => {
                    result.returned = returned;
                    result
                }
                Err(error) => execution_failure(result, error.status(), error.reason()),
            }
        }
        Err(reason) => execution_failure(result, ExecutionStatus::Unsupported, reason),
    }
}

fn jit_execute(
    payload: &CraneliftPayload,
    request: &ExecutionRequest,
) -> Result<(ExecutionStatus, i128), String> {
    if payload.clif.trim().is_empty() {
        return Err("Cranelift artifact has empty CLIF".to_owned());
    }
    let names = function_names(&payload.program, &payload.ssa);
    let scalar = lower_to_scalar(&payload.program, &payload.ssa, &names);
    if emit_clif(&scalar) != payload.clif {
        return Err(
            "Cranelift CLIF identity does not match the selected SSA in the payload".to_owned(),
        );
    }
    jit_scalar(&scalar, &request.target.function, request)
}

fn integer_bounds(bits: u16, signed: bool) -> (i64, i64) {
    if bits == 0 {
        return (0, 0);
    }
    if !signed {
        if bits >= 63 {
            (0, i64::MAX)
        } else {
            (0, (1i64 << bits) - 1)
        }
    } else if bits >= 63 {
        (i64::MIN, i64::MAX)
    } else {
        let top = 1i64 << (bits - 1);
        (-top, top - 1)
    }
}

fn jit_scalar(
    scalar: &ScalarModule,
    function_name: &str,
    request: &ExecutionRequest,
) -> Result<(ExecutionStatus, i128), String> {
    use cranelift_codegen::ir::condcodes::IntCC;
    use cranelift_codegen::ir::{types, AbiParam, BlockArg, InstBuilder, MemFlags, Value};
    use cranelift_codegen::settings::{self, Configurable};
    use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
    use cranelift_jit::{JITBuilder, JITModule};
    use cranelift_module::{FuncId, Linkage, Module};
    use mncs_model::SemanticId;
    use std::collections::BTreeMap;

    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|error| error.to_string())?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|error| error.to_string())?;
    let _ = flag_builder.set("enable_verifier", "true");
    let isa_builder =
        cranelift_native::builder().map_err(|error| format!("unsupported target: {error}"))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|error| error.to_string())?;
    let jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let mut module = JITModule::new(jit_builder);
    let mut declared: BTreeMap<String, FuncId> = BTreeMap::new();
    for function in &scalar.functions {
        let mut sig = module.make_signature();
        for _ in 0..function.params.len() {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        let id = module
            .declare_function(&function.export_name, Linkage::Export, &sig)
            .map_err(|error| error.to_string())?;
        declared.insert(function.export_name.clone(), id);
    }
    for function in &scalar.functions {
        let mut ctx = module.make_context();
        let mut fn_ctx = FunctionBuilderContext::new();
        ctx.func.signature.params.clear();
        for _ in 0..function.params.len() {
            ctx.func.signature.params.push(AbiParam::new(types::I64));
        }
        ctx.func.signature.params.push(AbiParam::new(types::I64));
        ctx.func.signature.params.push(AbiParam::new(types::I64));
        {
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fn_ctx);
            let mut blocks = BTreeMap::new();
            let mut values: BTreeMap<SemanticId, Value> = BTreeMap::new();
            for block in &function.blocks {
                blocks.insert(block.id.clone(), builder.create_block());
            }
            let fail = builder.create_block();
            let entry = *blocks
                .get(&function.blocks[0].id)
                .ok_or_else(|| "Cranelift function has no blocks".to_owned())?;
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            let fn_params = builder.block_params(entry).to_vec();
            for (index, param) in function.params.iter().enumerate() {
                values.insert(param.id.clone(), fn_params[index]);
            }
            let status_ptr = fn_params[function.params.len()];
            let value_ptr = fn_params[function.params.len() + 1];
            for block in function.blocks.iter().skip(1) {
                let clif_block = blocks[&block.id];
                for param in &block.params {
                    let value = builder.append_block_param(clif_block, types::I64);
                    values.insert(param.id.clone(), value);
                }
            }
            for (block_index, block) in function.blocks.iter().enumerate() {
                let clif_block = blocks[&block.id];
                if block_index != 0 {
                    builder.switch_to_block(clif_block);
                }
                for inst in &block.insts {
                    match inst {
                        ScalarInst::Const { dest, value } => {
                            let produced = builder
                                .ins()
                                .iconst(types::I64, i64::try_from(*value).unwrap_or(0));
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::Integer {
                            dest,
                            operator,
                            intent,
                            lhs,
                            rhs,
                            promise,
                        } => {
                            let left = values[lhs];
                            let right = values[rhs];
                            if matches!(operator.as_str(), "and" | "or" | "xor") {
                                let produced = match operator.as_str() {
                                    "and" => builder.ins().band(left, right),
                                    "or" => builder.ins().bor(left, right),
                                    _ => builder.ins().bxor(left, right),
                                };
                                values.insert(dest.id.clone(), produced);
                                continue;
                            }
                            let bits = match dest.ty {
                                ScalarTy::Int(integer) => integer.bits,
                                _ => 32,
                            };
                            let signed =
                                matches!(dest.ty, ScalarTy::Int(integer) if integer.signed);
                            let produced = match operator.as_str() {
                                "sub" => builder.ins().isub(left, right),
                                "mul" => builder.ins().imul(left, right),
                                _ => builder.ins().iadd(left, right),
                            };
                            if matches!(
                                intent,
                                ArithmeticIntent::Checked | ArithmeticIntent::Trapping
                            ) && !promise.decision.permitted
                            {
                                let (min, max) = integer_bounds(bits, signed);
                                let max_v = builder.ins().iconst(types::I64, max);
                                let min_v = builder.ins().iconst(types::I64, min);
                                let hi =
                                    builder
                                        .ins()
                                        .icmp(IntCC::SignedGreaterThan, produced, max_v);
                                let lo = builder.ins().icmp(IntCC::SignedLessThan, produced, min_v);
                                let ov = builder.ins().bor(hi, lo);
                                let cont = builder.create_block();
                                builder.ins().brif(
                                    ov,
                                    fail,
                                    &[] as &[BlockArg],
                                    cont,
                                    &[] as &[BlockArg],
                                );
                                builder.switch_to_block(cont);
                                builder.seal_block(cont);
                            }
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::Compare {
                            dest,
                            predicate,
                            operand,
                            lhs,
                            rhs,
                        } => {
                            let cc = match (predicate.as_str(), operand.signed) {
                                ("eq", _) => IntCC::Equal,
                                ("ne", _) => IntCC::NotEqual,
                                ("lt", true) => IntCC::SignedLessThan,
                                ("le", true) => IntCC::SignedLessThanOrEqual,
                                ("gt", true) => IntCC::SignedGreaterThan,
                                ("ge", true) => IntCC::SignedGreaterThanOrEqual,
                                ("lt", false) => IntCC::UnsignedLessThan,
                                ("le", false) => IntCC::UnsignedLessThanOrEqual,
                                ("gt", false) => IntCC::UnsignedGreaterThan,
                                _ => IntCC::UnsignedGreaterThanOrEqual,
                            };
                            let flag = builder.ins().icmp(cc, values[lhs], values[rhs]);
                            let produced = builder.ins().uextend(types::I64, flag);
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::FiniteConstruct { dest, discriminant } => {
                            let produced =
                                builder.ins().iconst(types::I64, i64::from(*discriminant));
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::FiniteIsVariant {
                            dest,
                            src,
                            discriminant,
                        } => {
                            let disc = builder.ins().iconst(types::I64, i64::from(*discriminant));
                            let flag = builder.ins().icmp(IntCC::Equal, values[src], disc);
                            let produced = builder.ins().uextend(types::I64, flag);
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::Call { dest, callee, args } => {
                            let callee_id = declared[callee];
                            let callee_ref = module.declare_func_in_func(callee_id, builder.func);
                            let mut call_args: Vec<Value> =
                                args.iter().map(|arg| values[arg]).collect();
                            call_args.push(status_ptr);
                            call_args.push(value_ptr);
                            builder.ins().call(callee_ref, &call_args);
                            let status =
                                builder
                                    .ins()
                                    .load(types::I32, MemFlags::trusted(), status_ptr, 0);
                            let failed = builder.ins().icmp_imm(IntCC::NotEqual, status, 0);
                            let cont = builder.create_block();
                            builder.ins().brif(
                                failed,
                                fail,
                                &[] as &[BlockArg],
                                cont,
                                &[] as &[BlockArg],
                            );
                            builder.switch_to_block(cont);
                            builder.seal_block(cont);
                            let loaded =
                                builder
                                    .ins()
                                    .load(types::I64, MemFlags::trusted(), value_ptr, 0);
                            values.insert(dest.id.clone(), loaded);
                        }
                    }
                }
                match &block.term {
                    ScalarTerm::Return { value } => {
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder
                            .ins()
                            .store(MemFlags::trusted(), zero, status_ptr, 0);
                        builder
                            .ins()
                            .store(MemFlags::trusted(), values[value], value_ptr, 0);
                        builder.ins().return_(&[]);
                    }
                    ScalarTerm::Jump { target, args } => {
                        let dest = blocks[target];
                        let arg_values: Vec<BlockArg> = args
                            .iter()
                            .map(|arg| BlockArg::Value(values[arg]))
                            .collect();
                        builder.ins().jump(dest, &arg_values);
                    }
                    ScalarTerm::Branch {
                        cond,
                        then_target,
                        then_args,
                        else_target,
                        else_args,
                    } => {
                        let cond_v = values[cond];
                        let nz = builder.ins().icmp_imm(IntCC::NotEqual, cond_v, 0);
                        let then_values: Vec<BlockArg> = then_args
                            .iter()
                            .map(|arg| BlockArg::Value(values[arg]))
                            .collect();
                        let else_values: Vec<BlockArg> = else_args
                            .iter()
                            .map(|arg| BlockArg::Value(values[arg]))
                            .collect();
                        builder.ins().brif(
                            nz,
                            blocks[then_target],
                            &then_values,
                            blocks[else_target],
                            &else_values,
                        );
                    }
                    ScalarTerm::Fail => {
                        builder.ins().jump(fail, &[]);
                    }
                }
            }
            builder.switch_to_block(fail);
            let one = builder.ins().iconst(types::I32, 1);
            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().store(MemFlags::trusted(), one, status_ptr, 0);
            builder.ins().store(MemFlags::trusted(), zero, value_ptr, 0);
            builder.ins().return_(&[]);
            builder.seal_all_blocks();
            builder.finalize();
        }
        let id = declared[&function.export_name];
        module
            .define_function(id, &mut ctx)
            .map_err(|error| error.to_string())?;
        module.clear_context(&mut ctx);
    }
    module
        .finalize_definitions()
        .map_err(|error| error.to_string())?;
    let func_id = declared
        .get(function_name)
        .copied()
        .ok_or_else(|| "requested Cranelift export is missing".to_owned())?;
    let ptr = module.get_finalized_function(func_id);
    let mut status: i32 = 2;
    let mut value: i64 = 0;
    let raw_args: Vec<i64> = argv_from_request(request)
        .iter()
        .map(|arg| arg.parse::<i64>().unwrap_or(0))
        .collect();
    unsafe {
        match raw_args.len() {
            0 => {
                let f: extern "C" fn(*mut i32, *mut i64) = std::mem::transmute(ptr);
                f(&mut status, &mut value);
            }
            1 => {
                let f: extern "C" fn(i64, *mut i32, *mut i64) = std::mem::transmute(ptr);
                f(raw_args[0], &mut status, &mut value);
            }
            2 => {
                let f: extern "C" fn(i64, i64, *mut i32, *mut i64) = std::mem::transmute(ptr);
                f(raw_args[0], raw_args[1], &mut status, &mut value);
            }
            3 => {
                let f: extern "C" fn(i64, i64, i64, *mut i32, *mut i64) = std::mem::transmute(ptr);
                f(
                    raw_args[0],
                    raw_args[1],
                    raw_args[2],
                    &mut status,
                    &mut value,
                );
            }
            _ => {
                return Err(
                    "Cranelift host trampoline currently supports at most three scalar arguments"
                        .to_owned(),
                );
            }
        }
    }
    if status == 0 {
        Ok((ExecutionStatus::Returned, i128::from(value)))
    } else {
        Ok((ExecutionStatus::RuntimeFailure, 0))
    }
}
