//! Deterministic LLVM IR realization of selected SSA.
//!
//! LLVM IR generation does not establish correctness. Optional clang/llc
//! compilation is an external realization step outside the MNCS trust
//! boundary.

use std::collections::BTreeMap;
use std::fmt::Write;

use mncs_model::{
    ArithmeticIntent, BackendCapabilityManifest, BackendConfiguration, BackendEvidence,
    BackendIdentity, BackendResult, CompilerArtifactRef, CompilerDiagnostic,
    CompilerDiagnosticKind, ExecutionRequest, ExecutionStatus, Program, SsaModule,
    TargetContractRef, TargetLoweringPlan, TransformationStatus, SSA_SCHEMA_VERSION,
};

use crate::native::{
    argv_from_request, compile_and_run, decode_native_value, host_matches_triple, host_triple,
    probe_clang, probe_llc,
};
use crate::scalar::{
    abi_bits, llvm_type, lower_to_scalar, ScalarBlock, ScalarFunction, ScalarInst, ScalarModule,
    ScalarTerm, ScalarTy, ScalarValue,
};
use crate::support::{
    artifact_ref, empty_execution, execution_failure, function_names, function_value_contracts,
    unknown, validate_selected_ssa,
};
use crate::{BackendAdapter, BackendExecutionResult};

pub const LLVM_BACKEND_NAME: &str = "mncs-llvm-ir";
pub const LLVM_BACKEND_VERSION: &str = "0.1";
pub const LLVM_TARGET: &str = "mncs:target:llvm-ir-0.1";
pub const LLVM_FORMAT: &str = "text/x-llvm-ir; mncs-llvm-ir-0.1";
pub const LLVM_ARTIFACT_KIND: &str = "llvm_ir";
pub const LLVM_DEFAULT_TRIPLE: &str = "x86_64-unknown-linux-gnu";
pub const LLVM_DEFAULT_DATALAYOUT: &str =
    "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128";

pub struct LlvmAdapter;

pub fn llvm_backend() -> BackendIdentity {
    BackendIdentity::new(LLVM_BACKEND_NAME, LLVM_BACKEND_VERSION)
}

pub fn llvm_configuration() -> BackendConfiguration {
    BackendConfiguration {
        backend: llvm_backend(),
        options: BTreeMap::from([
            ("emit".to_owned(), "llvm_ir".to_owned()),
            ("opt-level".to_owned(), "0".to_owned()),
            ("triple".to_owned(), LLVM_DEFAULT_TRIPLE.to_owned()),
        ]),
        target_features: ["scalar-ssa", "explicit-overflow", "no-memory"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        linker_toolchain: probe_clang().map(|tool| tool.semantic_id()),
        assumptions: vec![
            "LLVM IR is a realization, not MNCS semantics".to_owned(),
            "external clang/llc are outside the MNCS trust boundary".to_owned(),
            "status/value out-pointer ABI is private to this realization".to_owned(),
            "logical records/products, if present, are not a physical LLVM layout commitment"
                .to_owned(),
        ],
    }
}

pub fn llvm_capabilities() -> BackendCapabilityManifest {
    BackendCapabilityManifest::new(
        llvm_backend(),
        "mncs:selected-ssa-to-llvm-ir:0.1",
        [LLVM_TARGET.to_owned(), LLVM_DEFAULT_TRIPLE.to_owned()]
            .into_iter()
            .collect(),
        [SSA_SCHEMA_VERSION.to_owned()].into_iter().collect(),
        ["data-layout", "abi", "integer", "trap", "triple"]
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
            "host_effects",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        [LLVM_ARTIFACT_KIND.to_owned(), "native_object".to_owned()]
            .into_iter()
            .collect(),
        ["bounded_execution_agreement".to_owned()]
            .into_iter()
            .collect(),
        ["external_clang_or_llc".to_owned()].into_iter().collect(),
        ["status_value_out_pointer_abi".to_owned()]
            .into_iter()
            .collect(),
        "checked/trapping overflow and declared failure return status=1; wrapping uses LLVM wrapping add/sub/mul",
        true,
    )
}

pub fn llvm_target() -> TargetContractRef {
    TargetContractRef::new(
        LLVM_TARGET,
        BTreeMap::from([
            ("data-layout".to_owned(), LLVM_DEFAULT_DATALAYOUT.to_owned()),
            (
                "abi".to_owned(),
                "void fn(args..., i32* status, i64* value); status 0=returned 1=failure".to_owned(),
            ),
            (
                "integer".to_owned(),
                "wrapping=llvm wrapping; checked/trapping=llvm.*.with.overflow then status=1; nsw only with current PASS evidence".to_owned(),
            ),
            (
                "trap".to_owned(),
                "failure terminators and overflow return the private failure status rather than UB".to_owned(),
            ),
            ("triple".to_owned(), LLVM_DEFAULT_TRIPLE.to_owned()),
            ("opt-level".to_owned(), "0".to_owned()),
        ]),
        vec![mncs_model::SemanticId(
            "mncs:target-evidence:llvm-ir-0.1:declared-research-contract".to_owned(),
        )],
        vec![
            "LLVM target triple/data layout are realization facts, not the MNCS machine model"
                .to_owned(),
            "x86-64, AArch64, and Raspberry Pi-class ARM are target realizations, not language variants".to_owned(),
        ],
    )
}

pub fn llvm_plan(selected_ssa: CompilerArtifactRef) -> TargetLoweringPlan {
    TargetLoweringPlan::with_explicit_facts(
        selected_ssa,
        llvm_target(),
        Some(llvm_configuration()),
        vec![
            "scalar two's-complement integers".to_owned(),
            "no language-owned aggregate layout".to_owned(),
            "bounded iteration remains selected-SSA-bound; LLVM uses ordinary CFG/phis".to_owned(),
        ],
        vec![
            "private status/value out-pointer ABI".to_owned(),
            "one LLVM function per MNCS function".to_owned(),
        ],
        BTreeMap::from([
            ("wrapping".to_owned(), "llvm add/sub/mul/and/or/xor".to_owned()),
            (
                "checked".to_owned(),
                "llvm.*.with.overflow then status=1 unless a current no-overflow promise is permitted"
                    .to_owned(),
            ),
            (
                "unsupported".to_owned(),
                "saturating, widening, effects, memory".to_owned(),
            ),
        ]),
        BTreeMap::from([
            ("overflow".to_owned(), "status=1".to_owned()),
            ("failure_terminator".to_owned(), "status=1".to_owned()),
        ]),
        vec![
            "optional clang/llc for object/executable realization".to_owned(),
            if llc_object_available() {
                "llc and clang are present on this host for optional native object realization"
                    .to_owned()
            } else {
                "llc/clang native object realization is optional and host-dependent".to_owned()
            },
        ],
        Vec::new(),
        TransformationStatus::Pass,
    )
}

pub fn target_is_llvm(target: &TargetContractRef) -> bool {
    target.candidate == LLVM_TARGET
        && ["data-layout", "abi", "integer", "trap", "triple"]
            .iter()
            .all(|fact| target.facts.contains_key(*fact))
        && !target.evidence.is_empty()
}

impl BackendAdapter for LlvmAdapter {
    fn capabilities(&self) -> BackendCapabilityManifest {
        llvm_capabilities()
    }
    fn target(&self) -> TargetContractRef {
        llvm_target()
    }
    fn configuration(&self) -> BackendConfiguration {
        llvm_configuration()
    }
    fn plan(&self, selected_ssa: CompilerArtifactRef) -> TargetLoweringPlan {
        llvm_plan(selected_ssa)
    }
    fn lower(
        &self,
        program: &Program,
        ssa: &SsaModule,
        selected_ssa: CompilerArtifactRef,
        plan: &TargetLoweringPlan,
    ) -> BackendResult {
        lower_llvm(program, ssa, selected_ssa, plan)
    }
    fn execute(
        &self,
        artifact: &mncs_model::BackendArtifact,
        request: &ExecutionRequest,
    ) -> BackendExecutionResult {
        execute_llvm(artifact, request)
    }
}

pub fn lower_llvm(
    program: &Program,
    ssa: &SsaModule,
    selected_ssa: CompilerArtifactRef,
    plan: &TargetLoweringPlan,
) -> BackendResult {
    if let Err(result) = validate_selected_ssa(ssa, &selected_ssa, "CGL101") {
        return *result;
    }
    if !target_is_llvm(&plan.target) {
        return unknown(vec![CompilerDiagnostic::new(
            "CGL201",
            CompilerDiagnosticKind::MissingTargetEvidence,
            "LLVM lowering requires explicit data-layout, ABI, integer, trap, and triple facts",
        )]);
    }
    if plan.status != TransformationStatus::Pass {
        return unknown(vec![CompilerDiagnostic::new(
            "CGL202",
            CompilerDiagnosticKind::MissingTargetEvidence,
            "target lowering plan is not PASS; the LLVM adapter will not invent target facts",
        )]);
    }
    let names = function_names(program, ssa);
    let scalar = lower_to_scalar(program, ssa, &names);
    if !scalar.unsupported.is_empty() || scalar.functions.is_empty() {
        let mut diagnostics = vec![CompilerDiagnostic::new(
            "CGL301",
            CompilerDiagnosticKind::UnavailableBackendCapability,
            "selected SSA is outside the LLVM scalar envelope",
        )];
        for reason in scalar.unsupported {
            diagnostics.push(CompilerDiagnostic::new(
                "CGL302",
                CompilerDiagnosticKind::UnavailableBackendCapability,
                reason,
            ));
        }
        return unknown(diagnostics);
    }
    let ir = emit_module(&scalar, plan);
    let mut assumptions = plan.assumptions_introduced.clone();
    assumptions.extend(
        scalar
            .functions
            .iter()
            .flat_map(|function| function.promises.clone()),
    );
    assumptions.push(format!(
        "llvm opt-level {}",
        plan.target
            .facts
            .get("opt-level")
            .map_or("0", String::as_str)
    ));
    let artifact = mncs_model::BackendArtifact::new_with_kind(
        llvm_backend(),
        selected_ssa.clone(),
        plan.target.clone(),
        LLVM_ARTIFACT_KIND,
        LLVM_FORMAT,
        ir.as_bytes(),
        scalar
            .functions
            .iter()
            .map(|function| function.export_name.clone())
            .collect(),
        assumptions.clone(),
        Vec::new(),
        vec![
            "external clang compilation".to_owned(),
            "optional llc object emission".to_owned(),
            "no embedded LLVM interpreter".to_owned(),
        ],
        plan.target.evidence.clone(),
        Vec::new(),
        TransformationStatus::Pass,
    )
    .with_function_value_contracts(function_value_contracts(program));
    let artifact_ref = artifact_ref(&artifact);
    let evidence = BackendEvidence::new(
        llvm_backend(),
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

fn emit_module(module: &ScalarModule, plan: &TargetLoweringPlan) -> String {
    let triple = plan
        .target
        .facts
        .get("triple")
        .cloned()
        .unwrap_or_else(|| LLVM_DEFAULT_TRIPLE.to_owned());
    let layout = plan
        .target
        .facts
        .get("data-layout")
        .cloned()
        .unwrap_or_else(|| LLVM_DEFAULT_DATALAYOUT.to_owned());
    let mut out = String::new();
    let _ = writeln!(
        out,
        "; MNCS LLVM IR realization 0.1\n; not MNCS semantics; selected SSA identity is retained in the artifact envelope"
    );
    let _ = writeln!(out, "source_filename = \"mncs-llvm-ir\"");
    let _ = writeln!(out, "target datalayout = \"{layout}\"");
    let _ = writeln!(out, "target triple = \"{triple}\"");
    out.push('\n');
    for width in ["i8", "i16", "i32", "i64"] {
        for op in ["sadd", "uadd", "ssub", "usub", "smul", "umul"] {
            let _ = writeln!(
                out,
                "declare {{{width}, i1}} @llvm.{op}.with.overflow.{width}({width}, {width})"
            );
        }
    }
    out.push('\n');
    for function in &module.functions {
        emit_function(&mut out, function);
        out.push('\n');
    }
    out
}

fn emit_function(out: &mut String, function: &ScalarFunction) {
    let names = NameMap::new(function);
    let mut split = 0u32;
    let mut params = function
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| format!("{} %arg{index}", llvm_type(param.ty)))
        .collect::<Vec<_>>();
    params.push("ptr %mncs_status".to_owned());
    params.push("ptr %mncs_value".to_owned());
    let _ = writeln!(
        out,
        "define void @{}({}) {{",
        function.export_name,
        params.join(", ")
    );
    out.push_str("entry:\n");
    for value in names.all_values() {
        let _ = writeln!(out, "  %{}_slot = alloca {}", value.0, llvm_type(value.1));
    }
    for (index, param) in function.params.iter().enumerate() {
        let _ = writeln!(
            out,
            "  store {} %arg{index}, ptr %{}_slot",
            llvm_type(param.ty),
            names.value(&param.id)
        );
    }
    if let Some(first) = function.blocks.first() {
        let _ = writeln!(out, "  br label %{}", names.block(&first.id));
    }
    for block in &function.blocks {
        emit_block(out, function, block, &names, &mut split);
    }
    out.push_str("mncs_fail:\n");
    out.push_str("  store i32 1, ptr %mncs_status\n");
    out.push_str("  store i64 0, ptr %mncs_value\n");
    out.push_str("  ret void\n");
    out.push_str("}\n");
}

fn emit_block(
    out: &mut String,
    function: &ScalarFunction,
    block: &ScalarBlock,
    names: &NameMap,
    split: &mut u32,
) {
    let label = names.block(&block.id);
    let _ = writeln!(out, "{label}:");
    for inst in &block.insts {
        emit_inst(out, inst, names, split);
    }
    match &block.term {
        ScalarTerm::Return { value } => {
            let loaded = load_value(out, names, value, "retv", split);
            let ty = names.ty(value);
            let bits = abi_bits(ty);
            if bits < 64 {
                let _ = writeln!(
                    out,
                    "  %ret_ext_{} = sext {} %{loaded} to i64",
                    names.value(value),
                    llvm_type(ty)
                );
                let _ = writeln!(
                    out,
                    "  store i64 %ret_ext_{}, ptr %mncs_value",
                    names.value(value)
                );
            } else {
                let _ = writeln!(out, "  store i64 %{loaded}, ptr %mncs_value");
            }
            out.push_str("  store i32 0, ptr %mncs_status\n");
            out.push_str("  ret void\n");
        }
        ScalarTerm::Jump { target, args } => {
            emit_transfers(out, function, target, args, names, split);
            let _ = writeln!(out, "  br label %{}", names.block(target));
        }
        ScalarTerm::Branch {
            cond,
            then_target,
            then_args,
            else_target,
            else_args,
        } => {
            let loaded = load_value(out, names, cond, "cnd", split);
            let _ = writeln!(
                out,
                "  %c_{} = icmp ne {} %{loaded}, 0",
                names.value(cond),
                llvm_type(names.ty(cond))
            );
            *split += 1;
            let then_l = format!("br{split}_then");
            let else_l = format!("br{split}_else");
            let _ = writeln!(
                out,
                "  br i1 %c_{}, label %{then_l}, label %{else_l}",
                names.value(cond)
            );
            let _ = writeln!(out, "{then_l}:");
            emit_transfers(out, function, then_target, then_args, names, split);
            let _ = writeln!(out, "  br label %{}", names.block(then_target));
            let _ = writeln!(out, "{else_l}:");
            emit_transfers(out, function, else_target, else_args, names, split);
            let _ = writeln!(out, "  br label %{}", names.block(else_target));
        }
        ScalarTerm::Fail => out.push_str("  br label %mncs_fail\n"),
    }
}

fn emit_transfers(
    out: &mut String,
    function: &ScalarFunction,
    target: &mncs_model::SemanticId,
    args: &[mncs_model::SemanticId],
    names: &NameMap,
    split: &mut u32,
) {
    let Some(block) = function.blocks.iter().find(|block| block.id == *target) else {
        return;
    };
    for (param, arg) in block.params.iter().zip(args) {
        let loaded = load_value(out, names, arg, "xfer", split);
        let _ = writeln!(
            out,
            "  store {} %{loaded}, ptr %{}_slot",
            llvm_type(param.ty),
            names.value(&param.id)
        );
    }
}

fn load_value(
    out: &mut String,
    names: &NameMap,
    id: &mncs_model::SemanticId,
    prefix: &str,
    split: &mut u32,
) -> String {
    *split += 1;
    let tmp = format!("{prefix}{split}_{}", names.value(id));
    let _ = writeln!(
        out,
        "  %{tmp} = load {}, ptr %{}_slot",
        llvm_type(names.ty(id)),
        names.value(id)
    );
    tmp
}

fn store_dest(out: &mut String, names: &NameMap, dest: &ScalarValue, tmp: &str) {
    let _ = writeln!(
        out,
        "  store {} %{tmp}, ptr %{}_slot",
        llvm_type(dest.ty),
        names.value(&dest.id)
    );
}

fn emit_inst(out: &mut String, inst: &ScalarInst, names: &NameMap, split: &mut u32) {
    match inst {
        ScalarInst::Const { dest, value } => {
            *split += 1;
            let tmp = format!("k{split}");
            let _ = writeln!(
                out,
                "  %{tmp} = add {} {}, 0",
                llvm_type(dest.ty),
                llvm_const(*value, dest.ty)
            );
            store_dest(out, names, dest, &tmp);
        }
        ScalarInst::Integer {
            dest,
            operator,
            intent,
            lhs,
            rhs,
            promise,
        } => {
            let ty = llvm_type(dest.ty);
            let llvm_op = match operator.as_str() {
                "add" => "add",
                "sub" => "sub",
                "mul" => "mul",
                "and" => "and",
                "or" => "or",
                "xor" => "xor",
                _ => "add",
            };
            let left = load_value(out, names, lhs, "lhs", split);
            let right = load_value(out, names, rhs, "rhs", split);
            let nsw =
                promise.decision.permitted && matches!(operator.as_str(), "add" | "sub" | "mul");
            if matches!(
                intent,
                ArithmeticIntent::Checked | ArithmeticIntent::Trapping
            ) && matches!(operator.as_str(), "add" | "sub" | "mul")
                && !nsw
            {
                emit_checked(out, dest, operator, &left, &right, names, split);
            } else {
                *split += 1;
                let tmp = format!("bin{split}");
                let flags = if nsw { " nsw" } else { "" };
                let _ = writeln!(out, "  %{tmp} = {llvm_op}{flags} {ty} %{left}, %{right}");
                store_dest(out, names, dest, &tmp);
            }
        }
        ScalarInst::Compare {
            dest,
            predicate,
            operand,
            lhs,
            rhs,
        } => {
            let left = load_value(out, names, lhs, "lhs", split);
            let right = load_value(out, names, rhs, "rhs", split);
            let pred = llvm_pred(predicate, *operand);
            let ty = llvm_type(ScalarTy::Int(*operand));
            let _ = writeln!(
                out,
                "  %cmp_{} = icmp {pred} {ty} %{left}, %{right}",
                names.value(&dest.id)
            );
            let tmp = format!("cmpz_{}", names.value(&dest.id));
            let _ = writeln!(
                out,
                "  %{tmp} = zext i1 %cmp_{} to {}",
                names.value(&dest.id),
                llvm_type(dest.ty)
            );
            store_dest(out, names, dest, &tmp);
        }
        ScalarInst::FiniteConstruct { dest, discriminant } => {
            *split += 1;
            let tmp = format!("fc{split}");
            let _ = writeln!(out, "  %{tmp} = add i32 {discriminant}, 0");
            store_dest(out, names, dest, &tmp);
        }
        ScalarInst::FiniteIsVariant {
            dest,
            src,
            discriminant,
        } => {
            let srcv = load_value(out, names, src, "src", split);
            let _ = writeln!(
                out,
                "  %cmp_{} = icmp eq i32 %{srcv}, {discriminant}",
                names.value(&dest.id)
            );
            let tmp = format!("isz_{}", names.value(&dest.id));
            let _ = writeln!(
                out,
                "  %{tmp} = zext i1 %cmp_{} to {}",
                names.value(&dest.id),
                llvm_type(dest.ty)
            );
            store_dest(out, names, dest, &tmp);
        }
        ScalarInst::Call { dest, callee, args } => {
            let mut loaded = Vec::new();
            for arg in args {
                let loaded_arg = load_value(out, names, arg, "ca", split);
                loaded.push(format!("{} %{loaded_arg}", llvm_type(names.ty(arg))));
            }
            loaded.push("ptr %mncs_status".to_owned());
            loaded.push("ptr %mncs_value".to_owned());
            *split += 1;
            let tmp = format!("call{split}");
            let _ = writeln!(out, "  call void @{callee}({})", loaded.join(", "));
            let _ = writeln!(out, "  %{tmp}_st = load i32, ptr %mncs_status");
            let _ = writeln!(out, "  %{tmp}_fail = icmp ne i32 %{tmp}_st, 0");
            let _ = writeln!(
                out,
                "  br i1 %{tmp}_fail, label %mncs_fail, label %{tmp}_ok"
            );
            let _ = writeln!(out, "{tmp}_ok:");
            let _ = writeln!(out, "  %{tmp}_raw = load i64, ptr %mncs_value");
            let dest_ty = llvm_type(dest.ty);
            if dest_ty == "i64" {
                let _ = writeln!(out, "  %{tmp}_v = add i64 %{tmp}_raw, 0");
            } else {
                let _ = writeln!(out, "  %{tmp}_v = trunc i64 %{tmp}_raw to {dest_ty}");
            }
            store_dest(out, names, dest, &format!("{tmp}_v"));
        }
    }
}

fn emit_checked(
    out: &mut String,
    dest: &ScalarValue,
    operator: &str,
    lhs: &str,
    rhs: &str,
    names: &NameMap,
    split: &mut u32,
) {
    *split += 1;
    let ty = llvm_type(dest.ty);
    let signed = matches!(dest.ty, ScalarTy::Int(integer) if integer.signed)
        || matches!(dest.ty, ScalarTy::Bool);
    let kind = match (operator, signed) {
        ("add", true) => "sadd",
        ("add", false) => "uadd",
        ("sub", true) => "ssub",
        ("sub", false) => "usub",
        ("mul", true) => "smul",
        _ => "umul",
    };
    let tmp = format!("ov{split}");
    let _ = writeln!(
        out,
        "  %{tmp} = call {{{ty}, i1}} @llvm.{kind}.with.overflow.{ty}({ty} %{lhs}, {ty} %{rhs})"
    );
    let _ = writeln!(out, "  %{tmp}_v = extractvalue {{{ty}, i1}} %{tmp}, 0");
    let _ = writeln!(out, "  %{tmp}_f = extractvalue {{{ty}, i1}} %{tmp}, 1");
    let _ = writeln!(out, "  br i1 %{tmp}_f, label %mncs_fail, label %{tmp}_ok");
    let _ = writeln!(out, "{tmp}_ok:");
    store_dest(out, names, dest, &format!("{tmp}_v"));
}

fn llvm_const(value: i128, ty: ScalarTy) -> String {
    match ty {
        ScalarTy::Int(integer) if integer.bits == 64 => value.to_string(),
        _ => i32::try_from(value).unwrap_or(value as i32).to_string(),
    }
}

fn llvm_pred(predicate: &str, ty: mncs_model::IntegerType) -> &'static str {
    match (predicate, ty.signed) {
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
    }
}

struct NameMap {
    values: BTreeMap<mncs_model::SemanticId, String>,
    types: BTreeMap<mncs_model::SemanticId, ScalarTy>,
    blocks: BTreeMap<mncs_model::SemanticId, String>,
}

impl NameMap {
    fn new(function: &ScalarFunction) -> Self {
        let mut values = BTreeMap::new();
        let mut types = BTreeMap::new();
        let mut next = 0u32;
        let mut push = |value: &ScalarValue| {
            if values.contains_key(&value.id) {
                return;
            }
            values.insert(value.id.clone(), format!("v{next}"));
            types.insert(value.id.clone(), value.ty);
            next += 1;
        };
        for param in &function.params {
            push(param);
        }
        for block in &function.blocks {
            for param in &block.params {
                push(param);
            }
            for inst in &block.insts {
                match inst {
                    ScalarInst::Const { dest, .. }
                    | ScalarInst::Integer { dest, .. }
                    | ScalarInst::Compare { dest, .. }
                    | ScalarInst::FiniteConstruct { dest, .. }
                    | ScalarInst::FiniteIsVariant { dest, .. }
                    | ScalarInst::Call { dest, .. } => push(dest),
                }
            }
        }
        let blocks = function
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (block.id.clone(), format!("b{index}")))
            .collect();
        Self {
            values,
            types,
            blocks,
        }
    }

    fn value(&self, id: &mncs_model::SemanticId) -> &str {
        self.values.get(id).map_or("undef", String::as_str)
    }

    fn block(&self, id: &mncs_model::SemanticId) -> &str {
        self.blocks.get(id).map_or("mncs_fail", String::as_str)
    }

    fn ty(&self, id: &mncs_model::SemanticId) -> ScalarTy {
        self.types.get(id).copied().unwrap_or(ScalarTy::Finite)
    }

    fn all_values(&self) -> Vec<(String, ScalarTy)> {
        self.values
            .iter()
            .map(|(id, name)| {
                (
                    name.clone(),
                    self.types.get(id).copied().unwrap_or(ScalarTy::Finite),
                )
            })
            .collect()
    }
}

pub fn execute_llvm(
    artifact: &mncs_model::BackendArtifact,
    request: &ExecutionRequest,
) -> BackendExecutionResult {
    let mut result = empty_execution(artifact, request);
    if let Err(reason) =
        crate::support::backend_matches_identity(artifact, &llvm_backend(), LLVM_ARTIFACT_KIND)
    {
        return execution_failure(result, ExecutionStatus::InvalidRequest, reason);
    }
    let triple = artifact
        .target
        .facts
        .get("triple")
        .map_or(LLVM_DEFAULT_TRIPLE, String::as_str);
    if !host_matches_triple(triple) {
        return execution_failure(
            result,
            crate::native::NativeError::UnsupportedTarget(format!(
                "LLVM artifact triple {triple} does not match host {}",
                host_triple()
            ))
            .status(),
            crate::native::NativeError::UnsupportedTarget(format!(
                "LLVM artifact triple {triple} does not match host {}",
                host_triple()
            ))
            .reason(),
        );
    }
    let Some(clang) = probe_clang() else {
        return execution_failure(
            result,
            ExecutionStatus::Unsupported,
            "unavailable toolchain: clang is not present for external LLVM execution",
        );
    };
    let ir = match artifact.bytes() {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(reason) => {
            return execution_failure(result, ExecutionStatus::InvalidRequest, reason);
        }
    };
    let Some(contract) = artifact
        .function_value_contracts
        .get(&request.target.function)
    else {
        return execution_failure(
            result,
            ExecutionStatus::InvalidRequest,
            "LLVM execution requires a language-owned function value contract",
        );
    };
    if contract.inputs.len() != request.arguments.len() {
        return execution_failure(
            result,
            ExecutionStatus::InvalidRequest,
            "backend request violates the language-owned value contract",
        );
    }
    let driver = llvm_driver(&request.target.function, contract.inputs.len());
    let args = argv_from_request(request);
    match compile_and_run(
        &[("module.ll", ir.as_str()), ("driver.c", driver.as_str())],
        &clang,
        &["-Wno-override-module", "-O0", "-no-pie"],
        &args,
    ) {
        Ok((value, status, toolchain)) => {
            result.failure = None;
            result.status = status;
            result.steps = 1;
            match decode_native_value(Some(&contract.outputs[0]), status, value) {
                Ok(returned) => {
                    result.returned = returned;
                    let _ = toolchain;
                    result
                }
                Err(error) => execution_failure(result, error.status(), error.reason()),
            }
        }
        Err(error) => execution_failure(result, error.status(), error.reason()),
    }
}

fn llvm_driver(function: &str, arity: usize) -> String {
    let mut args = Vec::new();
    let mut parse = Vec::new();
    for index in 0..arity {
        args.push(format!("a{index}"));
        parse.push(format!(
            "  long long a{index} = strtoll(argv[{}], 0, 10);",
            index + 1
        ));
    }
    let proto_args = (0..arity)
        .map(|_| "int32_t")
        .chain(["int32_t*", "int64_t*"])
        .collect::<Vec<_>>()
        .join(", ");
    let call_args = args
        .iter()
        .map(|name| format!("(int32_t){name}"))
        .chain(["&status".to_owned(), "&value".to_owned()])
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
void {function}({proto_args});
int main(int argc, char **argv) {{
  (void)argc;
{}
  int32_t status = 2;
  int64_t value = 0;
  {function}({call_args});
  if (status == 0) {{
    printf("{{\"status\":\"returned\",\"value\":%lld}}\n", (long long)value);
  }} else {{
    printf("{{\"status\":\"runtime_failure\"}}\n");
  }}
  return 0;
}}
"#,
        parse.join("\n")
    )
}

pub fn llc_object_available() -> bool {
    probe_llc().is_some() && probe_clang().is_some()
}
