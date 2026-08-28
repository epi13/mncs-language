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
    argv_from_request, compile_and_run_with_call_file_full, host_matches_triple, host_triple,
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
            "saturating_integer",
            "trapping_integer",
            "explicit_failure",
            "semantic_bounded_iteration",
            "finite_values",
            "canonical_composite_cells",
            "byte_operations",
            "explicit_scalar_conversion",
            "integer_shifts",
            "bounded_sequences_internal",
            "bounded_views_internal",
            "sequence_or_view_boundary_crossing",
            "nested_composite_sequence_elements",
            "boolean_sequence_elements",
            "vector_or_mask_boundary_crossing",
            "semantic_branchless_select",
            "semantic_integer_vectors",
            "semantic_masks",
            "packed_mask_realization",
            "scalarized_vector_realization",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        [
            "memory",
            "effects",
            "unbounded_sequences",
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
                "wrapping=llvm wrapping; checked/trapping=llvm.*.with.overflow then status=1; nsw/nuw only with a current independently rechecked certificate".to_owned(),
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
    let ir = emit_llvm_module(&scalar, plan);
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
    .with_function_value_contracts(function_value_contracts(program))
    .with_composite_value_contracts(crate::support::composite_value_contracts(program))
    .with_promise_decisions(scalar.promise_decisions.clone());
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

pub(crate) fn emit_llvm_module(module: &ScalarModule, plan: &TargetLoweringPlan) -> String {
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
    if crate::support::scalar_module_needs_arena_symbols(module) {
        // Canonical composite cell arena (MNCS cell layout v0.1): 8-byte
        // aligned cells; slot access is plain i32/i64 load/store at 8-byte
        // strides so alignment holds by construction.
        out.push_str("; Canonical composite cell arena (v0.1)\n");
        out.push_str("@mncs_arena = global [4194304 x i8] zeroinitializer\n");
        out.push_str("@mncs_bump = global i64 0\n\n");
    }
    for width in ["i8", "i16", "i32", "i64"] {
        for op in ["sadd", "uadd", "ssub", "usub", "smul", "umul"] {
            let _ = writeln!(
                out,
                "declare {{{width}, i1}} @llvm.{op}.with.overflow.{width}({width}, {width})"
            );
        }
    }
    for width in ["i8", "i16", "i32", "i64"] {
        for op in ["sadd", "uadd", "ssub", "usub"] {
            let _ = writeln!(
                out,
                "declare {width} @llvm.{op}.sat.{width}({width}, {width})"
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
    for inst in flatten_scalar_insts(&block.insts) {
        emit_inst(out, inst, names, split);
    }
    match &block.term {
        ScalarTerm::Return { value } => {
            let loaded = load_value(out, names, value, "retv", split);
            let ty = names.ty(value);
            let bits = abi_bits(ty);
            if bits < 64 {
                // Unsigned narrow values extend by zero so a wrapping edge
                // value (u16 `0xFFFF`) decodes as its declared magnitude.
                let ext = if matches!(ty, ScalarTy::Int(integer) if !integer.signed) {
                    "zext"
                } else {
                    "sext"
                };
                let _ = writeln!(
                    out,
                    "  %ret_ext_{} = {ext} {} %{loaded} to i64",
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
                "div" => {
                    if matches!(dest.ty, ScalarTy::Int(integer) if !integer.signed) {
                        "udiv"
                    } else {
                        "sdiv"
                    }
                }
                "mod" => {
                    if matches!(dest.ty, ScalarTy::Int(integer) if !integer.signed) {
                        "urem"
                    } else {
                        "srem"
                    }
                }
                "and" => "and",
                "or" => "or",
                "xor" => "xor",
                // Total shifts: counts are modulo width; signed shr is
                // arithmetic (ashr), unsigned logical (lshr).
                "shl" => "shl",
                "shr" => {
                    if matches!(dest.ty, ScalarTy::Int(integer) if integer.signed) {
                        "ashr"
                    } else {
                        "lshr"
                    }
                }
                _ => "add",
            };
            let left = load_value(out, names, lhs, "lhs", split);
            let right = load_value(out, names, rhs, "rhs", split);
            let overflow_flag = promise
                .decision
                .permitted
                .then_some(promise.attribute.as_deref())
                .flatten()
                .and_then(|attribute| attribute.strip_prefix("llvm."));
            let no_overflow =
                overflow_flag.is_some() && matches!(operator.as_str(), "add" | "sub" | "mul");
            if matches!(operator.as_str(), "div" | "mod") {
                // Division by zero and MIN / -1 are immediate undefined
                // behavior in LLVM: guard them into the shared fail block so
                // failure statuses match the reference executors.
                emit_checked_division(out, dest, operator, &left, &right, names, split);
            } else if matches!(intent, ArithmeticIntent::Saturating) {
                // Saturating arithmetic is total by definition. Add/sub use
                // the saturating intrinsics; multiplication has no saturating
                // intrinsic, so it computes with an exact overflow flag and
                // selects the signed or unsigned clamp boundary.
                if let Some(clamped) =
                    emit_saturating(out, dest, operator, &left, &right, names, split)
                {
                    store_dest(out, names, dest, &clamped);
                }
            } else if matches!(
                intent,
                ArithmeticIntent::Checked | ArithmeticIntent::Trapping
            ) && matches!(operator.as_str(), "add" | "sub" | "mul")
                && !no_overflow
            {
                emit_checked(out, dest, operator, &left, &right, names, split);
            } else {
                *split += 1;
                let tmp = format!("bin{split}");
                let flags = overflow_flag.map_or(String::new(), |flag| format!(" {flag}"));
                let _ = writeln!(out, "  %{tmp} = {llvm_op}{flags} {ty} %{left}, %{right}");
                store_dest(out, names, dest, &tmp);
            }
        }
        ScalarInst::Boolean {
            dest,
            operator,
            lhs,
            rhs,
        } => {
            // Booleans share the i32 scalar slots; operands are normalized 0/1.
            let left = load_value(out, names, lhs, "lhs", split);
            let right = load_value(out, names, rhs, "rhs", split);
            let op = match operator.as_str() {
                "and" => "and",
                _ => "or",
            };
            *split += 1;
            let tmp = format!("bool{split}");
            let bool_ty = llvm_type(dest.ty);
            let _ = writeln!(out, "  %{tmp} = {op} {bool_ty} %{left}, %{right}");
            store_dest(out, names, dest, &tmp);
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
        ScalarInst::Sequence(insts) => {
            for nested in insts {
                emit_inst(out, nested, names, split);
            }
        }
        ScalarInst::FiniteConstruct { dest, discriminant } => {
            *split += 1;
            let tmp = format!("fc{split}");
            let _ = writeln!(out, "  %{tmp} = add i32 {discriminant}, 0");
            store_dest(out, names, dest, &tmp);
        }
        ScalarInst::CellAlloc { dest, bytes } => {
            // Bump allocation against the module arena, 8-byte aligned. The
            // destination slot already exists from the entry alloca block.
            let d = names.value(&dest.id);
            *split += 1;
            let bump = format!("bump{split}");
            let bumped = format!("bb{split}");
            let aligned = format!("al{split}");
            let newbump = format!("nb{split}");
            let _ = writeln!(out, "  %{bump} = load i64, ptr @mncs_bump");
            let _ = writeln!(out, "  %{bumped} = add i64 %{bump}, 7");
            let _ = writeln!(out, "  %{aligned} = and i64 %{bumped}, -8");
            let _ = writeln!(out, "  %{newbump} = add i64 %{aligned}, {bytes}");
            let _ = writeln!(out, "  store i64 %{newbump}, ptr @mncs_bump");
            let _ = writeln!(out, "  store i64 %{aligned}, ptr %{d}_slot");
        }
        ScalarInst::CellStoreDiscriminant { cell, discriminant } => {
            *split += 1;
            let c = names.value(cell);
            let cellv = format!("cs_cell{split}");
            let addr = format!("cs_addr{split}");
            let gep = format!("cs_gep{split}");
            let _ = writeln!(out, "  %{cellv} = load i64, ptr %{c}_slot");
            let _ = writeln!(out, "  %{addr} = add i64 %{cellv}, 0");
            let _ = writeln!(
                out,
                "  %{gep} = getelementptr inbounds [4194304 x i8], ptr @mncs_arena, i64 0, i64 %{addr}"
            );
            let _ = writeln!(out, "  store i32 {discriminant}, ptr %{gep}");
        }
        ScalarInst::CellStore {
            cell,
            byte_offset,
            width,
            value,
        } => {
            *split += 1;
            let c = names.value(cell);
            let v = names.value(value);
            let cellv = format!("st_cell{split}");
            let addr = format!("st_addr{split}");
            let gep = format!("st_gep{split}");
            // The slot payload keeps the value's own realization width;
            // narrow scalars (bytes, i8/i16) must not be reloaded as i32.
            let raw_ty = slot_payload_ty(*width, names.ty(value));
            let raw = format!("st_raw{split}");
            match width {
                crate::composite::SlotWidth::W32 => {
                    let _ = writeln!(out, "  %{cellv} = load i64, ptr %{c}_slot");
                    let _ = writeln!(out, "  %{addr} = add i64 %{cellv}, {byte_offset}");
                    let _ = writeln!(
                        out,
                        "  %{gep} = getelementptr inbounds [4194304 x i8], ptr @mncs_arena, i64 0, i64 %{addr}"
                    );
                    let _ = writeln!(out, "  %{raw} = load {raw_ty}, ptr %{v}_slot");
                    let _ = writeln!(out, "  store {raw_ty} %{raw}, ptr %{gep}");
                }
                crate::composite::SlotWidth::W64 => {
                    let _ = writeln!(out, "  %{cellv} = load i64, ptr %{c}_slot");
                    let _ = writeln!(out, "  %{addr} = add i64 %{cellv}, {byte_offset}");
                    let _ = writeln!(
                        out,
                        "  %{gep} = getelementptr inbounds [4194304 x i8], ptr @mncs_arena, i64 0, i64 %{addr}"
                    );
                    let _ = writeln!(out, "  %{raw} = load i64, ptr %{v}_slot");
                    let _ = writeln!(out, "  store i64 %{raw}, ptr %{gep}");
                }
            }
        }
        ScalarInst::CellLoad {
            dest,
            cell,
            byte_offset,
            width,
        } => {
            *split += 1;
            let d = names.value(&dest.id);
            let c = names.value(cell);
            let cellv = format!("ld_cell{split}");
            let addr = format!("ld_addr{split}");
            let gep = format!("ld_gep{split}");
            let raw_ty = slot_payload_ty(*width, dest.ty);
            match width {
                crate::composite::SlotWidth::W32 => {
                    let _ = writeln!(out, "  %{cellv} = load i64, ptr %{c}_slot");
                    let _ = writeln!(out, "  %{addr} = add i64 %{cellv}, {byte_offset}");
                    let _ = writeln!(
                        out,
                        "  %{gep} = getelementptr inbounds [4194304 x i8], ptr @mncs_arena, i64 0, i64 %{addr}"
                    );
                    let _ = writeln!(out, "  %{d}_v32 = load {raw_ty}, ptr %{gep}");
                    let _ = writeln!(out, "  store {raw_ty} %{d}_v32, ptr %{d}_slot");
                }
                crate::composite::SlotWidth::W64 => {
                    let _ = writeln!(out, "  %{cellv} = load i64, ptr %{c}_slot");
                    let _ = writeln!(out, "  %{addr} = add i64 %{cellv}, {byte_offset}");
                    let _ = writeln!(
                        out,
                        "  %{gep} = getelementptr inbounds [4194304 x i8], ptr @mncs_arena, i64 0, i64 %{addr}"
                    );
                    let _ = writeln!(out, "  %{d}_v64 = load i64, ptr %{gep}");
                    let _ = writeln!(out, "  store i64 %{d}_v64, ptr %{d}_slot");
                }
            }
        }
        ScalarInst::ByteBitwise {
            dest,
            operator,
            lhs,
            rhs,
        } => {
            // Bytes realize as unsigned 8-bit cells; bitwise ops are total.
            let left = load_value(out, names, lhs, "lhs", split);
            let right = load_value(out, names, rhs, "rhs", split);
            let op = match operator.as_str() {
                "and" => "and",
                "or" => "or",
                _ => "xor",
            };
            *split += 1;
            let tmp = format!("bw{split}");
            let _ = writeln!(out, "  %{tmp} = {op} i8 %{left}, %{right}");
            store_dest(out, names, dest, &tmp);
        }
        ScalarInst::ByteShift {
            dest,
            operator,
            lhs,
            rhs,
        } => {
            // Total semantics: the u64 count is taken modulo 8; shifts are
            // logical because bytes are unsigned by definition.
            let left = load_value(out, names, lhs, "lhs", split);
            let right = load_value(out, names, rhs, "rhs", split);
            *split += 1;
            let modn = format!("shmod{split}");
            let _ = writeln!(out, "  %{modn} = urem i64 %{right}, 8");
            let count = if llvm_type(dest.ty) == "i64" {
                modn
            } else {
                *split += 1;
                let trunc = format!("shtr{split}");
                let _ = writeln!(out, "  %{trunc} = trunc i64 %{modn} to i8");
                trunc
            };
            let op = if operator == "shl" { "shl" } else { "lshr" };
            *split += 1;
            let tmp = format!("sh{split}");
            let _ = writeln!(
                out,
                "  %{tmp} = {op} {} %{left}, %{count}",
                llvm_type(dest.ty),
                count = count
            );
            store_dest(out, names, dest, &tmp);
        }
        ScalarInst::Convert {
            dest,
            from,
            to,
            src,
        } => {
            // Explicit total conversion: narrowing truncates high bits,
            // widening extends by the source signedness (`byte` is unsigned).
            let srcv = load_value(out, names, src, "src", split);
            let from_ty = llvm_type(*from);
            let to_ty = llvm_type(*to);
            let converted = if from_ty == to_ty {
                srcv
            } else {
                *split += 1;
                let tmp = format!("cv{split}");
                if bits_of(*from) < bits_of(*to) {
                    if signed_of(*from) {
                        let _ = writeln!(out, "  %{tmp} = sext {from_ty} %{srcv} to {to_ty}");
                    } else {
                        let _ = writeln!(out, "  %{tmp} = zext {from_ty} %{srcv} to {to_ty}");
                    }
                } else {
                    let _ = writeln!(out, "  %{tmp} = trunc {from_ty} %{srcv} to {to_ty}");
                }
                tmp
            };
            store_dest(out, names, dest, &converted);
        }
        ScalarInst::Select {
            dest,
            condition,
            when_true,
            when_false,
        } => {
            let condition_ty = llvm_type(names.ty(condition));
            let condition = load_value(out, names, condition, "selc", split);
            let when_true = load_value(out, names, when_true, "selt", split);
            let when_false = load_value(out, names, when_false, "self", split);
            *split += 1;
            let tag = *split;
            let ty = llvm_type(dest.ty);
            let _ = writeln!(out, "  %selb{tag} = icmp ne {condition_ty} %{condition}, 0");
            let _ = writeln!(
                out,
                "  %sel{tag} = select i1 %selb{tag}, {ty} %{when_true}, {ty} %{when_false}"
            );
            store_dest(out, names, dest, &format!("sel{tag}"));
        }
        ScalarInst::SequenceReplace {
            dest,
            source,
            index,
            element,
            evidence,
            length,
            element_width,
            ..
        } => {
            let idx = load_value(out, names, index, "ridx", split);
            let source = load_value(out, names, source, "rsrc", split);
            if matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. }) {
                *split += 1;
                let tag = *split;
                let _ = writeln!(out, "  %roob{tag} = icmp uge i64 %{idx}, {length}");
                let _ = writeln!(
                    out,
                    "  br i1 %roob{tag}, label %mncs_fail, label %roob{tag}_ok"
                );
                let _ = writeln!(out, "roob{tag}_ok:");
            }
            *split += 1;
            let tag = *split;
            let d = names.value(&dest.id);
            let bytes = u64::from(*length) * 8;
            let _ = writeln!(out, "  %rbump{tag} = load i64, ptr @mncs_bump");
            let _ = writeln!(out, "  %rbb{tag} = add i64 %rbump{tag}, 7");
            let _ = writeln!(out, "  %ral{tag} = and i64 %rbb{tag}, -8");
            let _ = writeln!(out, "  %rnb{tag} = add i64 %ral{tag}, {bytes}");
            let _ = writeln!(out, "  store i64 %rnb{tag}, ptr @mncs_bump");
            let _ = writeln!(out, "  store i64 %ral{tag}, ptr %{d}_slot");
            let raw_ty = slot_payload_ty(*element_width, names.ty(element));
            for lane in 0..*length {
                let offset = u64::from(lane) * 8;
                let _ = writeln!(out, "  %rsaddr{tag}_{lane} = add i64 %{source}, {offset}");
                let _ = writeln!(out, "  %rdaddr{tag}_{lane} = add i64 %ral{tag}, {offset}");
                let _ = writeln!(out, "  %rsgep{tag}_{lane} = getelementptr inbounds [4194304 x i8], ptr @mncs_arena, i64 0, i64 %rsaddr{tag}_{lane}");
                let _ = writeln!(out, "  %rdgep{tag}_{lane} = getelementptr inbounds [4194304 x i8], ptr @mncs_arena, i64 0, i64 %rdaddr{tag}_{lane}");
                let _ = writeln!(
                    out,
                    "  %rslot{tag}_{lane} = load {raw_ty}, ptr %rsgep{tag}_{lane}"
                );
                let _ = writeln!(
                    out,
                    "  store {raw_ty} %rslot{tag}_{lane}, ptr %rdgep{tag}_{lane}"
                );
            }
            let element = load_value(out, names, element, "relem", split);
            *split += 1;
            let store_tag = *split;
            let _ = writeln!(out, "  %rioff{store_tag} = shl i64 %{idx}, 3");
            let _ = writeln!(
                out,
                "  %riaddr{store_tag} = add i64 %ral{tag}, %rioff{store_tag}"
            );
            let _ = writeln!(out, "  %rige{store_tag} = getelementptr inbounds [4194304 x i8], ptr @mncs_arena, i64 0, i64 %riaddr{store_tag}");
            let _ = writeln!(out, "  store {raw_ty} %{element}, ptr %rige{store_tag}");
        }
        ScalarInst::SequenceProject {
            dest,
            seq,
            index,
            bound,
            evidence,
            width,
        } => {
            let checked = matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. });
            let idx = load_value(out, names, index, "idx", split);
            let seqv = load_value(out, names, seq, "seq", split);
            // Bounds check exactly when validity was not established
            // statically or by traversal semantics.
            if checked {
                *split += 1;
                let tag = *split;
                match bound {
                    mncs_model::SequenceBound::Exact(length) => {
                        let _ = writeln!(out, "  %oob{tag} = icmp uge i64 %{idx}, {length}");
                    }
                    mncs_model::SequenceBound::UpTo(_) => {
                        let _ = writeln!(out, "  %vlen{tag} = lshr i64 %{seqv}, 32");
                        let _ = writeln!(out, "  %oob{tag} = icmp uge i64 %{idx}, %vlen{tag}");
                    }
                }
                let _ = writeln!(
                    out,
                    "  br i1 %oob{tag}, label %mncs_fail, label %oob{tag}_ok"
                );
                let _ = writeln!(out, "oob{tag}_ok:");
            }
            // Address computation: exact sequences are canonical cells;
            // views unpack their base offset from the descriptor's low half.
            let base = match bound {
                mncs_model::SequenceBound::Exact(_) => seqv,
                mncs_model::SequenceBound::UpTo(_) => {
                    *split += 1;
                    let base = format!("vbase{split}");
                    let _ = writeln!(out, "  %base32{split} = trunc i64 %{seqv} to i32");
                    let _ = writeln!(out, "  %{base} = zext i32 %base32{split} to i64");
                    base
                }
            };
            *split += 1;
            let off = format!("soff{split}");
            let _ = writeln!(out, "  %idxw{split} = shl i64 %{idx}, 3");
            let _ = writeln!(out, "  %{off} = add i64 %{base}, %idxw{split}");
            let gep = format!("sgep{split}");
            let _ = writeln!(
                out,
                "  %{gep} = getelementptr inbounds [4194304 x i8], ptr @mncs_arena, i64 0, i64 %{off}"
            );
            let d = names.value(&dest.id);
            let elem_ty = slot_payload_ty(*width, dest.ty);
            match width {
                crate::composite::SlotWidth::W32 => {
                    let _ = writeln!(out, "  %{d}_v32 = load {elem_ty}, ptr %{gep}");
                    let _ = writeln!(out, "  store {elem_ty} %{d}_v32, ptr %{d}_slot");
                }
                crate::composite::SlotWidth::W64 => {
                    let _ = writeln!(out, "  %{d}_v64 = load i64, ptr %{gep}");
                    let _ = writeln!(out, "  store i64 %{d}_v64, ptr %{d}_slot");
                }
            }
        }
        ScalarInst::SequenceLength { dest, bound, seq } => match bound {
            mncs_model::SequenceBound::Exact(length) => {
                *split += 1;
                let tmp = format!("sl{split}");
                let _ = writeln!(out, "  %{tmp} = add i64 {length}, 0");
                store_dest(out, names, dest, &tmp);
            }
            mncs_model::SequenceBound::UpTo(_) => {
                let seqv = load_value(out, names, seq, "seq", split);
                *split += 1;
                let tmp = format!("sl{split}");
                let _ = writeln!(out, "  %{tmp} = lshr i64 %{seqv}, 32");
                store_dest(out, names, dest, &tmp);
            }
        },
        ScalarInst::ViewConstruct {
            dest,
            source_bound,
            view_cap,
            source,
            start,
            end,
        } => {
            let startv = load_value(out, names, start, "start", split);
            let endv = load_value(out, names, end, "end", split);
            // The source length is static for exact sequences and observed
            // from the packed descriptor for views.
            let source_len = match source_bound {
                mncs_model::SequenceBound::Exact(length) => {
                    *split += 1;
                    let tmp = format!("srcl{split}");
                    let _ = writeln!(out, "  %{tmp} = add i64 {length}, 0");
                    tmp
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    let srcv = load_value(out, names, source, "src", split);
                    *split += 1;
                    let tmp = format!("srcl{split}");
                    let _ = writeln!(out, "  %{tmp} = lshr i64 %{srcv}, 32");
                    tmp
                }
            };
            // Range validity: start <= end <= len(source), end-start <= cap.
            *split += 1;
            let tag = *split;
            let _ = writeln!(out, "  %bad1{tag} = icmp ugt i64 %{startv}, %{endv}");
            let _ = writeln!(out, "  %bad2{tag} = icmp ugt i64 %{endv}, %{source_len}");
            let _ = writeln!(out, "  %span{tag} = sub i64 %{endv}, %{startv}");
            let _ = writeln!(out, "  %bad3{tag} = icmp ugt i64 %span{tag}, {view_cap}");
            let _ = writeln!(out, "  %bad12{tag} = or i1 %bad1{tag}, %bad2{tag}");
            let _ = writeln!(out, "  %bad{tag} = or i1 %bad12{tag}, %bad3{tag}");
            let _ = writeln!(
                out,
                "  br i1 %bad{tag}, label %mncs_fail, label %vr{tag}_ok"
            );
            let _ = writeln!(out, "vr{tag}_ok:");
            // Pack (base + start*8) | ((end-start) << 32) into one i64.
            let base = match source_bound {
                mncs_model::SequenceBound::Exact(_) => {
                    load_value(out, names, source, "srcb", split)
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    let srcv = load_value(out, names, source, "srcb", split);
                    *split += 1;
                    let b = format!("vb{split}");
                    let _ = writeln!(out, "  %vb32{split} = trunc i64 %{srcv} to i32");
                    let _ = writeln!(out, "  %{b} = zext i32 %vb32{split} to i64");
                    b
                }
            };
            *split += 1;
            let addr = format!("va{split}");
            let _ = writeln!(out, "  %vsx{split} = shl i64 %{startv}, 3");
            let _ = writeln!(out, "  %{addr} = add i64 %{base}, %vsx{split}");
            *split += 1;
            let packed = format!("vp{split}");
            let _ = writeln!(out, "  %pa{split} = and i64 %{addr}, 4294967295");
            let _ = writeln!(out, "  %pl{split} = shl i64 %span{tag}, 32");
            let _ = writeln!(out, "  %{packed} = or i64 %pa{split}, %pl{split}");
            store_dest(out, names, dest, &packed);
        }
        ScalarInst::FiniteIsVariant {
            dest,
            src,
            discriminant,
        } if names.ty(src).is_cell() => {
            // Boxed finite: compare the canonical cell's tag word.
            let srcv = load_value(out, names, src, "src", split);
            *split += 1;
            let tagptr = format!("tag{split}");
            let tag = format!("tagv{split}");
            let _ = writeln!(
                out,
                "  %{tagptr} = getelementptr inbounds [4194304 x i8], ptr @mncs_arena, i64 0, i64 %{srcv}"
            );
            let _ = writeln!(out, "  %{tag} = load i32, ptr %{tagptr}");
            let _ = writeln!(
                out,
                "  %cmp_{} = icmp eq i32 %{tag}, {discriminant}",
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
        ScalarInst::FiniteIsVariant {
            dest,
            src,
            discriminant,
        } => {
            // Unboxed finite: the value itself is the bare discriminant.
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

/// Saturating add/sub/mul: total clamping arithmetic. Returns the temp name
/// holding the clamped result. Add/sub lower to the saturating intrinsics;
/// multiplication has no saturating intrinsic, so it computes with an exact
/// overflow flag and selects between the true product and the clamp boundary.
fn emit_saturating(
    out: &mut String,
    dest: &ScalarValue,
    operator: &str,
    lhs: &str,
    rhs: &str,
    _names: &NameMap,
    split: &mut u32,
) -> Option<String> {
    let ty = llvm_type(dest.ty);
    let width = ty
        .trim_start_matches('i')
        .parse::<u32>()
        .unwrap_or(32)
        .min(64);
    let signed = matches!(dest.ty, ScalarTy::Int(integer) if integer.signed)
        || matches!(dest.ty, ScalarTy::Bool);
    let (min_const, max_const): (i128, i128) = if signed {
        let top = 1_i128 << (width - 1);
        (-top, top - 1)
    } else {
        (0, (1_i128 << width) - 1)
    };
    *split += 1;
    let tmp = format!("sat{split}");
    match (operator, signed) {
        ("add", true) => {
            let _ = writeln!(
                out,
                "  %{tmp} = call {ty} @llvm.sadd.sat.{ty}({ty} %{lhs}, {ty} %{rhs})"
            );
        }
        ("sub", true) => {
            let _ = writeln!(
                out,
                "  %{tmp} = call {ty} @llvm.ssub.sat.{ty}({ty} %{lhs}, {ty} %{rhs})"
            );
        }
        ("add", false) => {
            let _ = writeln!(
                out,
                "  %{tmp} = call {ty} @llvm.uadd.sat.{ty}({ty} %{lhs}, {ty} %{rhs})"
            );
        }
        ("sub", false) => {
            let _ = writeln!(
                out,
                "  %{tmp} = call {ty} @llvm.usub.sat.{ty}({ty} %{lhs}, {ty} %{rhs})"
            );
        }
        _ => {
            // Multiplication: exact overflow flag, then select the boundary
            // in the overflowing direction. A signed product overflows toward
            // MIN exactly when the operand signs differ.
            let kind = if signed { "smul" } else { "umul" };
            let pair = format!("mulov{tmp}");
            let _ = writeln!(
                out,
                "  %{pair} = call {{{ty}, i1}} @llvm.{kind}.with.overflow.{ty}({ty} %{lhs}, {ty} %{rhs})"
            );
            let _ = writeln!(out, "  %{pair}_v = extractvalue {{{ty}, i1}} %{pair}, 0");
            let _ = writeln!(out, "  %{pair}_f = extractvalue {{{ty}, i1}} %{pair}, 1");
            if signed {
                let _ = writeln!(out, "  %{}_a = icmp slt {ty} %{lhs}, 0", pair);
                let _ = writeln!(out, "  %{}_b = icmp slt {ty} %{rhs}, 0", pair);
                let _ = writeln!(out, "  %{}_dir = xor i1 %{}_a, %{}_b", pair, pair, pair);
                let _ = writeln!(
                    out,
                    "  %{tmp}_b = select i1 %{pair}_dir, {ty} {min_const}, {ty} {max_const}"
                );
            } else {
                // Unsigned products can only overflow upward.
                let _ = writeln!(out, "  %{tmp}_b = add {ty} {max_const}, 0");
            }
            let _ = writeln!(
                out,
                "  %{tmp}_r = select i1 %{pair}_f, {ty} %{tmp}_b, {ty} %{pair}_v"
            );
            return Some(format!("{tmp}_r"));
        }
    }
    Some(tmp)
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

/// Guarded division/remainder: a zero divisor always fails; signed division
/// by MIN / -1 fails under checked intent. Guards branch to mncs_fail so
/// statuses match the reference executors exactly.
fn emit_checked_division(
    out: &mut String,
    dest: &ScalarValue,
    operator: &str,
    lhs: &str,
    rhs: &str,
    names: &NameMap,
    split: &mut u32,
) {
    let ty = llvm_type(dest.ty);
    *split += 1;
    let tag = *split;
    let _ = writeln!(out, "  %dz{tag} = icmp eq {ty} %{rhs}, 0");
    let _ = writeln!(out, "  br i1 %dz{tag}, label %mncs_fail, label %dz{tag}_ok");
    let _ = writeln!(out, "dz{tag}_ok:");
    let signed = matches!(dest.ty, ScalarTy::Int(integer) if integer.signed);
    if operator == "div" && signed {
        let min = match dest.ty {
            ScalarTy::Int(integer) => {
                let magnitude = 1_i128 << (integer.bits.max(1) - 1);
                format!("-{magnitude}")
            }
            _ => "-2147483648".to_owned(),
        };
        *split += 1;
        let inner = *split;
        let _ = writeln!(out, "  %m1{inner} = icmp eq {ty} %{rhs}, -1");
        let _ = writeln!(out, "  %mn{inner} = icmp eq {ty} %{lhs}, {min}");
        let _ = writeln!(out, "  %ov{inner} = and i1 %m1{inner}, %mn{inner}");
        let _ = writeln!(
            out,
            "  br i1 %ov{inner}, label %mncs_fail, label %dv{inner}_ok"
        );
        let _ = writeln!(out, "dv{inner}_ok:");
    }
    let native = match (operator, signed) {
        ("div", false) => "udiv",
        ("mod", false) => "urem",
        ("mod", _) => "srem",
        _ => "sdiv",
    };
    *split += 1;
    let tmp = format!("q{}", *split);
    let _ = writeln!(out, "  %{tmp} = {native} {ty} %{lhs}, %{rhs}");
    store_dest(out, names, dest, &tmp);
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

/// Flatten sequence groups so registration and emission see every concrete
/// instruction exactly once.
fn flatten_scalar_insts(insts: &[ScalarInst]) -> Vec<&ScalarInst> {
    let mut flat = Vec::new();
    for inst in insts {
        match inst {
            ScalarInst::Sequence(nested) => flat.extend(flatten_scalar_insts(nested)),
            other => flat.push(other),
        }
    }
    flat
}

fn scalar_inst_dest(inst: &ScalarInst) -> Option<&ScalarValue> {
    match inst {
        ScalarInst::Const { dest, .. }
        | ScalarInst::Integer { dest, .. }
        | ScalarInst::Boolean { dest, .. }
        | ScalarInst::Compare { dest, .. }
        | ScalarInst::FiniteConstruct { dest, .. }
        | ScalarInst::CellAlloc { dest, .. }
        | ScalarInst::CellLoad { dest, .. }
        | ScalarInst::FiniteIsVariant { dest, .. }
        | ScalarInst::ByteBitwise { dest, .. }
        | ScalarInst::ByteShift { dest, .. }
        | ScalarInst::Convert { dest, .. }
        | ScalarInst::Select { dest, .. }
        | ScalarInst::SequenceReplace { dest, .. }
        | ScalarInst::SequenceProject { dest, .. }
        | ScalarInst::SequenceLength { dest, .. }
        | ScalarInst::ViewConstruct { dest, .. }
        | ScalarInst::Call { dest, .. } => Some(dest),
        ScalarInst::CellStoreDiscriminant { .. } | ScalarInst::CellStore { .. } => None,
        ScalarInst::Sequence(_) => None,
    }
}

/// The IR type carried by one canonical slot payload: W64 slots hold full
/// words; W32 slots keep the value's own (narrow) realization width so a
/// byte stays a byte across the arena boundary.
fn slot_payload_ty(width: crate::composite::SlotWidth, ty: ScalarTy) -> &'static str {
    match width {
        crate::composite::SlotWidth::W64 => "i64",
        crate::composite::SlotWidth::W32 => match ty {
            ScalarTy::Byte => "i8",
            ScalarTy::Int(integer) if integer.bits <= 8 => "i8",
            ScalarTy::Int(integer) if integer.bits <= 16 => "i16",
            _ => "i32",
        },
    }
}

/// Bit width of a scalar realization kind for conversion decisions.
fn bits_of(ty: ScalarTy) -> u16 {
    match ty {
        ScalarTy::Bool | ScalarTy::Finite => 32,
        ScalarTy::Byte => 8,
        ScalarTy::Int(integer) => integer.bits,
        ScalarTy::Cell | ScalarTy::View | ScalarTy::Mask(_) => 64,
    }
}

/// Signedness of a scalar realization kind for widening decisions.
fn signed_of(ty: ScalarTy) -> bool {
    matches!(ty, ScalarTy::Int(integer) if integer.signed)
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
            for inst in flatten_scalar_insts(&block.insts) {
                if let Some(dest) = scalar_inst_dest(inst) {
                    push(dest);
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
    // Composite arguments and results cross through the canonical call
    // file; pure scalar calls keep the historical argv-only protocol.
    let driver = llvm_driver(
        &request.target.function,
        &contract.inputs,
        contract.outputs.first(),
    );
    let call_blob = match crate::support::build_call_file(
        &request.arguments,
        &contract.inputs,
        contract.outputs.first(),
        &artifact.composite_value_contracts,
    ) {
        Ok(blob) => blob,
        Err(reason) => {
            return execution_failure(result, ExecutionStatus::InvalidRequest, reason);
        }
    };
    let call_path = call_blob.as_ref().map(|bytes| {
        let digest = crate::support::sha256_hex(bytes);
        let path = std::env::temp_dir().join(format!("mncs-call-{digest}.bin"));
        let _ = std::fs::write(&path, bytes);
        path
    });
    // In call-file mode every value crosses canonically; argv stays empty.
    let args = match &call_blob {
        Some(_) => Vec::new(),
        None => match argv_from_request(request) {
            Ok(args) => args,
            Err(reason) => return execution_failure(result, ExecutionStatus::Unsupported, reason),
        },
    };
    match compile_and_run_with_call_file_full(
        &[("module.ll", ir.as_str()), ("driver.c", driver.as_str())],
        &clang,
        &["-Wno-override-module", "-O0", "-no-pie"],
        &args,
        call_path.as_deref(),
    ) {
        Ok((run, _toolchain)) => {
            result.failure = None;
            result.status = run.status;
            result.steps = 1;
            match crate::support::decode_native_observation(
                contract.outputs.first(),
                run.status,
                run.value,
                run.arena_hex.as_deref(),
                &artifact.composite_value_contracts,
            ) {
                Ok(returned) => {
                    result.returned = returned;
                    result
                }
                Err(reason) => execution_failure(result, ExecutionStatus::InvalidRequest, reason),
            }
        }
        Err(error) => execution_failure(result, error.status(), error.reason()),
    }
}

fn llvm_driver(
    function: &str,
    inputs: &[mncs_model::BackendValueContract],
    output: Option<&mncs_model::BackendValueContract>,
) -> String {
    // The shared C driver template matches the LLVM function ABI exactly:
    // typed scalar parameters, uint64_t cell offsets, and status/value
    // out-pointers. The module realizes the same arena symbols as C11.
    crate::support::process_driver(function, inputs, output)
}

pub fn llc_object_available() -> bool {
    probe_llc().is_some() && probe_clang().is_some()
}
