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
    TargetContractRef, TargetLoweringPlan, TransformationStatus, MODEL_MAX_CALL_DEPTH,
    SSA_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};

use crate::native::{argv_from_request, host_triple};
use crate::scalar::{
    lower_to_scalar, ScalarFunction, ScalarInst, ScalarModule, ScalarTerm, ScalarTy,
};
use crate::support::{
    artifact_ref, empty_execution, execution_failure, function_names, function_value_contracts,
    unknown, validate_realizable_ssa, validate_selected_ssa,
};
use crate::{BackendAdapter, BackendExecutionResult};
use cranelift_codegen::ir::InstBuilder as _;

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
            "non_host_isa_jit",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        [CRANELIFT_ARTIFACT_KIND.to_owned()].into_iter().collect(),
        ["bounded_execution_agreement".to_owned()]
            .into_iter()
            .collect(),
        [
            "cranelift_host_jit".to_owned(),
            "language_stateful_trace_runner".to_owned(),
        ]
        .into_iter()
        .collect(),
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
                "wrapping iadd/isub/imul; checked via i64 range tests; division guards zero divisor and signed MIN / -1 into the structured failure path; unsigned operands use udiv/urem".to_owned(),
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
                "i64 range test then status=1; division guards zero divisor and signed MIN / -1"
                    .to_owned(),
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
    mncs_model::record_counter("backend_lowering");
    if let Err(result) = validate_selected_ssa(ssa, &selected_ssa, "CGF101") {
        return *result;
    }
    if let Err(result) = validate_realizable_ssa(program, ssa, "CGF102") {
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
        ssa.proof_binding_refs(),
        vec![
            "cranelift host JIT".to_owned(),
            "inspectable CLIF text".to_owned(),
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

/// Reinterpret i64 bits as f64 through one explicit stack slot (Profile
/// 0.12). This Cranelift version exposes only the memory `bitcast`, so
/// value reinterpretation round-trips through a reserved 8-byte slot.
fn jit_bits_to_f64(
    builder: &mut cranelift_frontend::FunctionBuilder,
    slot: cranelift_codegen::ir::StackSlot,
    bits: cranelift_codegen::ir::Value,
) -> cranelift_codegen::ir::Value {
    builder.ins().stack_store(bits, slot, 0);
    builder
        .ins()
        .stack_load(cranelift_codegen::ir::types::F64, slot, 0)
}

/// Reinterpret an f64 value as i64 bits through the reserved slot.
fn jit_f64_to_bits(
    builder: &mut cranelift_frontend::FunctionBuilder,
    slot: cranelift_codegen::ir::StackSlot,
    value: cranelift_codegen::ir::Value,
) -> cranelift_codegen::ir::Value {
    builder.ins().stack_store(value, slot, 0);
    builder
        .ins()
        .stack_load(cranelift_codegen::ir::types::I64, slot, 0)
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
    // RFC 0047 §5: trailing call-depth fuel (machine ABI).
    sig.push("i64".to_owned());
    let _ = writeln!(
        out,
        "function %{}({}) {{",
        function.export_name,
        sig.join(", ")
    );
    for (index, block) in function.blocks.iter().enumerate() {
        let _ = index;
        let params = if index == 0 {
            let mut list = function
                .params
                .iter()
                .map(|param| format!("{}: {}", names.value(&param.id), clif_ty(param.ty)))
                .collect::<Vec<_>>();
            list.push("st: i64".to_owned());
            list.push("val: i64".to_owned());
            list.push("depth: i64".to_owned());
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
        for inst in flatten_scalar(&block.insts) {
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
                "        {} = iconst.{} {}",
                names.value(&dest.id),
                clif_ty(dest.ty),
                clif_constant(*value, dest.ty)
            );
        }
        ScalarInst::FloatConst { dest, bits } => {
            // Bit-carried in the uniform i64 cell; use sites bitcast.
            let _ = writeln!(
                out,
                "        {} = iconst.i64 {}",
                names.value(&dest.id),
                *bits as i64
            );
        }
        ScalarInst::Float {
            dest,
            operator,
            lhs,
            rhs,
        } => {
            let native = match operator.as_str() {
                "add" => "fadd",
                "sub" => "fsub",
                "mul" => "fmul",
                "div" => "fdiv",
                _ => "fadd",
            };
            let dest_n = names.value(&dest.id);
            let lhs_n = names.value(lhs);
            let rhs_n = names.value(rhs);
            // Uniform i64 cells bitcast at use; each operand and the
            // result carries a finiteness guard into the shared failure
            // shape, identical to the division guards.
            let _ = writeln!(out, "        {dest_n}_l = bitcast.f64 {lhs_n}");
            let _ = writeln!(out, "        {dest_n}_r = bitcast.f64 {rhs_n}");
            let _ = writeln!(out, "        {dest_n}_z = f64const 0x0000000000000000");
            for side in ["l", "r"] {
                let _ = writeln!(
                    out,
                    "        {dest_n}_d{side} = fsub {dest_n}_{side}, {dest_n}_{side}"
                );
                let _ = writeln!(
                    out,
                    "        {dest_n}_b{side} = fcmp une {dest_n}_d{side}, {dest_n}_z"
                );
                let _ = writeln!(out, "        brnz {dest_n}_b{side}, fail_fl_{dest_n}");
            }
            let _ = writeln!(out, "        {dest_n}_f = {native} {dest_n}_l, {dest_n}_r");
            let _ = writeln!(out, "        {dest_n}_dr = fsub {dest_n}_f, {dest_n}_f");
            let _ = writeln!(
                out,
                "        {dest_n}_br = fcmp une {dest_n}_dr, {dest_n}_z"
            );
            let _ = writeln!(out, "        brnz {dest_n}_br, fail_fl_{dest_n}");
            let _ = writeln!(out, "        {dest_n} = bitcast.i64 {dest_n}_f");
            let _ = writeln!(out, "        jump ok_fl_{dest_n}");
            let _ = writeln!(out, "    fail_fl_{dest_n}:");
            out.push_str("        v_bad = iconst.i32 1\n        v_z = iconst.i64 0\n");
            out.push_str(
                "        store.i32 v_bad, st\n        store.i64 v_z, val\n        return\n",
            );
            let _ = writeln!(out, "    ok_fl_{dest_n}:");
        }
        ScalarInst::FloatIntrinsic {
            dest,
            function,
            src,
        } => {
            // Text form only; the JIT/AOT builder lowers the same guard
            // and call shape through the declared `sin`/`cos` import.
            let dest_n = names.value(&dest.id);
            let src_n = names.value(src);
            let call = match function.as_str() {
                "sin" => "call sin",
                "cos" => "call cos",
                _ => "call mncs_unknown_float_intrinsic",
            };
            let _ = writeln!(out, "        {dest_n}_l = bitcast.f64 {src_n}");
            let _ = writeln!(out, "        {dest_n}_z = f64const 0x0000000000000000");
            let _ = writeln!(out, "        {dest_n}_d = fsub {dest_n}_l, {dest_n}_l");
            let _ = writeln!(out, "        {dest_n}_b = fcmp une {dest_n}_d, {dest_n}_z");
            let _ = writeln!(out, "        brnz {dest_n}_b, fail_fl_{dest_n}");
            let _ = writeln!(out, "        {dest_n}_f = {call} {dest_n}_l");
            let _ = writeln!(out, "        {dest_n}_dr = fsub {dest_n}_f, {dest_n}_f");
            let _ = writeln!(
                out,
                "        {dest_n}_br = fcmp une {dest_n}_dr, {dest_n}_z"
            );
            let _ = writeln!(out, "        brnz {dest_n}_br, fail_fl_{dest_n}");
            let _ = writeln!(out, "        {dest_n} = bitcast.i64 {dest_n}_f");
            let _ = writeln!(out, "        jump ok_fl_{dest_n}");
            let _ = writeln!(out, "    fail_fl_{dest_n}:");
            out.push_str("        v_bad = iconst.i32 1\n        v_z = iconst.i64 0\n");
            out.push_str(
                "        store.i32 v_bad, st\n        store.i64 v_z, val\n        return\n",
            );
            let _ = writeln!(out, "    ok_fl_{dest_n}:");
        }
        ScalarInst::FloatCompare {
            dest,
            predicate,
            lhs,
            rhs,
        } => {
            let cc = match predicate.as_str() {
                "eq" => "eq",
                "ne" => "ne",
                "lt" => "lt",
                "le" => "le",
                "gt" => "gt",
                _ => "ge",
            };
            let dest_n = names.value(&dest.id);
            let lhs_n = names.value(lhs);
            let rhs_n = names.value(rhs);
            let _ = writeln!(out, "        {dest_n}_l = bitcast.f64 {lhs_n}");
            let _ = writeln!(out, "        {dest_n}_r = bitcast.f64 {rhs_n}");
            let _ = writeln!(out, "        {dest_n}_z = f64const 0x0000000000000000");
            for side in ["l", "r"] {
                let _ = writeln!(
                    out,
                    "        {dest_n}_d{side} = fsub {dest_n}_{side}, {dest_n}_{side}"
                );
                let _ = writeln!(
                    out,
                    "        {dest_n}_b{side} = fcmp une {dest_n}_d{side}, {dest_n}_z"
                );
                let _ = writeln!(out, "        brnz {dest_n}_b{side}, fail_fc_{dest_n}");
            }
            let _ = writeln!(out, "        {dest_n} = fcmp {cc} {dest_n}_l, {dest_n}_r");
            let _ = writeln!(out, "        jump ok_fc_{dest_n}");
            let _ = writeln!(out, "    fail_fc_{dest_n}:");
            out.push_str("        v_bad = iconst.i32 1\n        v_z = iconst.i64 0\n");
            out.push_str(
                "        store.i32 v_bad, st\n        store.i64 v_z, val\n        return\n",
            );
            let _ = writeln!(out, "    ok_fc_{dest_n}:");
        }
        ScalarInst::Boolean {
            dest,
            operator,
            lhs,
            rhs,
        } => {
            let op = match operator.as_str() {
                "and" => "band",
                _ => "bor",
            };
            let _ = writeln!(
                out,
                "        {} = {} {}, {}",
                names.value(&dest.id),
                op,
                names.value(lhs),
                names.value(rhs)
            );
        }
        ScalarInst::BooleanCompare {
            dest,
            predicate,
            lhs,
            rhs,
        } => {
            // Normalized 0/1 i32 cells compare exactly with `icmp eq/ne`,
            // mirroring `Compare`.
            let cond = match predicate.as_str() {
                "eq" => "eq",
                "ne" => "ne",
                _ => "eq",
            };
            let _ = writeln!(
                out,
                "        {} = icmp {cond} {}, {}",
                names.value(&dest.id),
                names.value(lhs),
                names.value(rhs)
            );
        }
        ScalarInst::BooleanNot { dest, src } => {
            // Normalized 0/1 cell: logical not is equality-against-zero,
            // mirroring the proven `icmp_imm` selection-guard form.
            let _ = writeln!(
                out,
                "        {} = icmp_imm eq {}, 0",
                names.value(&dest.id),
                names.value(src)
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
            if matches!(operator.as_str(), "div" | "mod") {
                // Division never relies on native traps: a zero divisor and
                // (for signed division) MIN / -1 are guarded into the shared
                // structured-failure path, identical to the reference
                // executors and to the JIT/AOT builders. Unsigned operands
                // use the unsigned division forms.
                let signed = matches!(dest.ty, ScalarTy::Int(integer) if integer.signed);
                let bits = match dest.ty {
                    ScalarTy::Int(integer) => integer.bits,
                    ScalarTy::Cell | ScalarTy::View | ScalarTy::Mask(_) => 64,
                    _ => 32,
                };
                let (lhs_norm, rhs_norm) =
                    emit_width_normalize(out, lhs_n, rhs_n, bits, signed, dest_n);
                let _ = writeln!(out, "        {dest_n}_dz = icmp eq {rhs_norm}, 0");
                let _ = writeln!(out, "        brnz {dest_n}_dz, fail_div_{dest_n}");
                if operator == "div" && signed {
                    let min = -(1_i128 << (bits - 1));
                    let _ = writeln!(out, "        {dest_n}_m1 = icmp eq {rhs_norm}, -1");
                    let _ = writeln!(out, "        {dest_n}_mn = icmp eq {lhs_norm}, {min}");
                    let _ = writeln!(out, "        {dest_n}_ov = band {dest_n}_m1, {dest_n}_mn");
                    let _ = writeln!(out, "        brnz {dest_n}_ov, fail_div_{dest_n}");
                }
                let native = match (operator.as_str(), signed) {
                    ("div", false) => "udiv",
                    ("mod", false) => "urem",
                    ("mod", true) => "srem",
                    _ => "sdiv",
                };
                let _ = writeln!(out, "        {dest_n} = {native} {lhs_norm}, {rhs_norm}");
                let _ = writeln!(out, "        jump ok_div_{dest_n}");
                let _ = writeln!(out, "    fail_div_{dest_n}:");
                out.push_str("        v_bad = iconst.i32 1\n        v_z = iconst.i64 0\n");
                out.push_str(
                    "        store.i32 v_bad, st\n        store.i64 v_z, val\n        return\n",
                );
                let _ = writeln!(out, "    ok_div_{dest_n}:");
                return;
            }
            if matches!(operator.as_str(), "and" | "or" | "xor") {
                let op = match operator.as_str() {
                    "and" => "band",
                    "or" => "bor",
                    _ => "bxor",
                };
                let _ = writeln!(out, "        {dest_n} = {op} {lhs_n}, {rhs_n}");
                return;
            }
            if matches!(operator.as_str(), "shl" | "shr") {
                // Counts are modulo the declared width inside the uniform
                // i64 cell; normalize operands into the declared domain
                // first so the bit pattern matches the language semantics.
                let bits = match dest.ty {
                    ScalarTy::Int(integer) => integer.bits,
                    ScalarTy::Cell | ScalarTy::View | ScalarTy::Mask(_) => 64,
                    _ => 32,
                };
                let signed = matches!(dest.ty, ScalarTy::Int(integer) if integer.signed);
                let (lhs_v, rhs_v) = emit_width_normalize(out, lhs_n, rhs_n, bits, signed, dest_n);
                let _ = writeln!(out, "        {dest_n}_c = urem.i64 {rhs_v}, {bits}");
                if operator == "shl" {
                    let _ = writeln!(out, "        {dest_n} = ishl {lhs_v}, {dest_n}_c");
                    if bits < 64 {
                        if signed {
                            let shift = 64 - i64::from(bits);
                            let _ =
                                writeln!(out, "        {dest_n} = ishl_imm.i64 {dest_n}, {shift}");
                            let _ =
                                writeln!(out, "        {dest_n} = sshr_imm.i64 {dest_n}, {shift}");
                        } else {
                            let mask = ((1_i128 << bits) - 1) as i64;
                            let _ = writeln!(out, "        {dest_n} = band {dest_n}, {mask}");
                        }
                    }
                } else if signed {
                    let _ = writeln!(out, "        {dest_n} = sshr {lhs_v}, {dest_n}_c");
                } else {
                    let _ = writeln!(out, "        {dest_n} = ushr {lhs_v}, {dest_n}_c");
                }
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
                // Unsigned cells zero-extend (u64 MAX rides as `-1` bits);
                // signed cells sign-extend. Comparisons match the domain.
                let signed = matches!(dest.ty, ScalarTy::Int(integer) if integer.signed);
                let bits = match dest.ty {
                    ScalarTy::Int(integer) => integer.bits,
                    ScalarTy::Cell | ScalarTy::View | ScalarTy::Mask(_) => 64,
                    _ => 32,
                };
                let (min, max) = integer_bounds(bits, signed);
                let ext = if signed { "sextend" } else { "uextend" };
                let (hi_cc, lo_cc) = if signed {
                    ("sgt", "slt")
                } else {
                    ("ugt", "ult")
                };
                let _ = writeln!(out, "        {dest_n}_l = {ext}.i64 {lhs_n}");
                let _ = writeln!(out, "        {dest_n}_r = {ext}.i64 {rhs_n}");
                let _ = writeln!(out, "        {dest_n}_w = {op} {dest_n}_l, {dest_n}_r");
                let _ = writeln!(out, "        {dest_n}_max = iconst.i64 {max}");
                let _ = writeln!(out, "        {dest_n}_min = iconst.i64 {min}");
                let _ = writeln!(
                    out,
                    "        {dest_n}_hi = icmp {hi_cc} {dest_n}_w, {dest_n}_max"
                );
                let _ = writeln!(
                    out,
                    "        {dest_n}_lo = icmp {lo_cc} {dest_n}_w, {dest_n}_min"
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
        ScalarInst::CellAlloc { dest, bytes } => {
            let _ = writeln!(
                out,
                "        {} = call %mncs_cell_alloc({bytes})",
                names.value(&dest.id)
            );
        }
        ScalarInst::CellStoreDiscriminant { cell, discriminant } => {
            let _ = writeln!(
                out,
                "        v_tmp = iconst.i32 {discriminant}\n        call %mncs_slot_store32({}, 0, v_tmp)",
                names.value(cell)
            );
        }
        ScalarInst::CellStore {
            cell,
            byte_offset,
            width,
            value,
        } => match width {
            crate::composite::SlotWidth::W32 => {
                let _ = writeln!(
                    out,
                    "        call %mncs_slot_store32({}, {byte_offset}, {})",
                    names.value(cell),
                    names.value(value)
                );
            }
            crate::composite::SlotWidth::W64 => {
                let _ = writeln!(
                    out,
                    "        call %mncs_slot_store64({}, {byte_offset}, {})",
                    names.value(cell),
                    names.value(value)
                );
            }
        },
        ScalarInst::CellLoad {
            dest,
            cell,
            byte_offset,
            width,
        } => match width {
            crate::composite::SlotWidth::W32 => {
                let _ = writeln!(
                    out,
                    "        {} = load.u32({}, {byte_offset})",
                    names.value(&dest.id),
                    names.value(cell)
                );
            }
            crate::composite::SlotWidth::W64 => {
                let _ = writeln!(
                    out,
                    "        {} = load.i64({}, {byte_offset})",
                    names.value(&dest.id),
                    names.value(cell)
                );
            }
        },
        ScalarInst::Sequence(insts) => {
            for nested in insts {
                emit_clif_inst(out, nested, names);
            }
        }
        ScalarInst::ByteBitwise {
            dest,
            operator,
            lhs,
            rhs,
        } => {
            // Bytes ride zero-extended in the uniform cell, so bitwise ops
            // on the cells are byte-exact.
            let op = match operator.as_str() {
                "and" => "band",
                "or" => "bor",
                _ => "bxor",
            };
            let _ = writeln!(
                out,
                "        {} = {op} {}, {}",
                names.value(&dest.id),
                names.value(lhs),
                names.value(rhs)
            );
        }
        ScalarInst::ByteShift {
            dest,
            operator,
            lhs,
            rhs,
        } => {
            // Total semantics: count modulo 8, logical shifts, result masked
            // back into the byte domain.
            let d = names.value(&dest.id);
            let _ = writeln!(out, "        {d}_c = urem.i64 {}, 8", names.value(rhs));
            if operator == "shl" {
                let _ = writeln!(out, "        {d}_s = ishl {}, {d}_c", names.value(lhs));
            } else {
                let _ = writeln!(out, "        {d}_s = ushr {}, {d}_c", names.value(lhs));
            }
            let _ = writeln!(out, "        {d} = band {d}_s, 255");
        }
        ScalarInst::Convert {
            dest,
            from,
            to,
            src,
            ..
        } => {
            // The source cell is normalized to its own width/signedness, so
            // conversion is renormalization into the target parameters:
            // truncation drops high bits; widening keeps the value exactly.
            // Float edges mirror the JIT builder: int sources convert
            // exactly rounded, float sources guard finite and range-guard
            // with sliver-safe bounds before converting.
            let d = names.value(&dest.id);
            if matches!(to, ScalarTy::Float) {
                let op = if signed_of(*from) {
                    "fcvt_from_sint.f64"
                } else {
                    "fcvt_from_uint.f64"
                };
                let _ = writeln!(out, "        {d} = {op} {}", names.value(src));
            } else {
                // Float sources convert through a guarded intermediate;
                // integer sources renormalize directly.
                let mut source = names.value(src).to_owned();
                if matches!(from, ScalarTy::Float) {
                    let (cc, lo, hi) = float_guard_bounds(*to);
                    let cc_text = match cc {
                        cranelift_codegen::ir::condcodes::FloatCC::LessThan => "flt",
                        _ => "fle",
                    };
                    let conv = if signed_of(*to) { "sint" } else { "uint" };
                    let _ = writeln!(out, "        {d}_v = bitcast.f64 {}", names.value(src));
                    let _ = writeln!(out, "        {d}_bad = {cc_text} {d}_v, {lo:?}");
                    let _ = writeln!(out, "        {d}_bad2 = fge {d}_v, {hi:?}");
                    let _ = writeln!(out, "        {d}_raw = fcvt_to_{conv}.i64 {d}_v");
                    let _ = writeln!(out, "        trapz {d}_bad ; trapz {d}_bad2");
                    source = format!("{d}_raw");
                }
                let bits = bits_of(*to);
                let signed = signed_of(*to);
                if bits >= 64 && !signed {
                    let _ = writeln!(out, "        {d} = {source}");
                } else if signed {
                    let shift = 64 - i64::from(bits);
                    let _ = writeln!(out, "        {d} = ishl_imm.i64 {source}, {shift}");
                    let _ = writeln!(out, "        {d} = sshr_imm.i64 {d}, {shift}");
                } else {
                    let mask = ((1_i128 << bits) - 1) as i64;
                    let _ = writeln!(out, "        {d} = band {source}, {mask}");
                }
            }
        }
        ScalarInst::Select {
            dest,
            condition,
            when_true,
            when_false,
        } => {
            let d = names.value(&dest.id);
            let _ = writeln!(
                out,
                "        {d}_b = icmp_imm ne {}, 0",
                names.value(condition)
            );
            let _ = writeln!(
                out,
                "        {d} = select {d}_b, {}, {}",
                names.value(when_true),
                names.value(when_false)
            );
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
            let d = names.value(&dest.id);
            let checked = matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. });
            if checked {
                let _ = writeln!(out, "        {d}_lim = iconst.i64 {length}");
                let _ = writeln!(
                    out,
                    "        {d}_oob = icmp uge {}, {d}_lim",
                    names.value(index)
                );
                let _ = writeln!(out, "        brnz {d}_oob, fail_{d}");
            }
            let _ = writeln!(out, "        {d} = call %mncs_cell_alloc({})", length * 8);
            for lane in 0..*length {
                let offset = lane * 8;
                match element_width {
                    crate::composite::SlotWidth::W32 => {
                        let _ = writeln!(
                            out,
                            "        {d}_s{lane} = load.u32({}, {offset})",
                            names.value(source)
                        );
                        let _ = writeln!(
                            out,
                            "        call %mncs_slot_store32({d}, {offset}, {d}_s{lane})"
                        );
                    }
                    crate::composite::SlotWidth::W64 => {
                        let _ = writeln!(
                            out,
                            "        {d}_s{lane} = load.i64({}, {offset})",
                            names.value(source)
                        );
                        let _ = writeln!(
                            out,
                            "        call %mncs_slot_store64({d}, {offset}, {d}_s{lane})"
                        );
                    }
                }
            }
            let _ = writeln!(
                out,
                "        {d}_off = ishl_imm.i64 {}, 3",
                names.value(index)
            );
            let _ = writeln!(out, "        {d}_addr = iadd {d}, {d}_off");
            let store = match element_width {
                crate::composite::SlotWidth::W32 => "mncs_slot_store32",
                crate::composite::SlotWidth::W64 => "mncs_slot_store64",
            };
            let _ = writeln!(
                out,
                "        call %{store}({d}_addr, 0, {})",
                names.value(element)
            );
            if checked {
                let _ = writeln!(out, "        jump ok_{d}");
                let _ = writeln!(out, "    fail_{d}:");
                out.push_str("        v_bad = iconst.i32 1\n        v_z = iconst.i64 0\n");
                out.push_str(
                    "        store.i32 v_bad, st\n        store.i64 v_z, val\n        return\n",
                );
                let _ = writeln!(out, "    ok_{d}:");
            }
        }
        ScalarInst::SequenceProject {
            dest,
            seq,
            index,
            bound,
            evidence,
            width,
        } => {
            emit_clif_sequence_project(
                out,
                dest,
                seq,
                index,
                bound.clone(),
                evidence,
                *width,
                names,
            );
        }
        ScalarInst::SequenceLength { dest, bound, seq } => match bound {
            mncs_model::SequenceBound::Exact(length) => {
                let _ = writeln!(
                    out,
                    "        {} = iconst.i64 {length}",
                    names.value(&dest.id)
                );
            }
            mncs_model::SequenceBound::UpTo(_) => {
                let _ = writeln!(
                    out,
                    "        {} = ushr_imm.i64 {}, 32",
                    names.value(&dest.id),
                    names.value(seq)
                );
            }
            mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                unreachable!("generic SequenceBound must be specialized before backend lowering")
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
            let d = names.value(&dest.id);
            let start_n = names.value(start);
            let end_n = names.value(end);
            let source_len = match source_bound {
                mncs_model::SequenceBound::Exact(length) => {
                    let _ = writeln!(out, "        {d}_sl = iconst.i64 {length}");
                    format!("{d}_sl")
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    let _ = writeln!(
                        out,
                        "        {d}_sl = ushr_imm.i64 {}, 32",
                        names.value(source)
                    );
                    format!("{d}_sl")
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            };
            let base = match source_bound {
                mncs_model::SequenceBound::Exact(_) => names.value(source).to_owned(),
                mncs_model::SequenceBound::UpTo(_) => {
                    let _ = writeln!(
                        out,
                        "        {d}_b32 = band {}, 4294967295",
                        names.value(source)
                    );
                    format!("{d}_b32")
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            };
            let _ = writeln!(out, "        {d}_gt = icmp ug {start_n}, {end_n}");
            let _ = writeln!(out, "        {d}_over = icmp ugt {end_n}, {source_len}");
            let _ = writeln!(out, "        {d}_span = isub {end_n}, {start_n}");
            let _ = writeln!(out, "        {d}_cap = icmp ugt {d}_span, {view_cap}");
            let _ = writeln!(out, "        {d}_bad12 = bor {d}_gt, {d}_over");
            let _ = writeln!(out, "        {d}_bad = bor {d}_bad12, {d}_cap");
            let _ = writeln!(out, "        brnz {d}_bad, fail_{d}");
            let _ = writeln!(out, "        {d}_addr = iadd {base}, {start_n}");
            let _ = writeln!(out, "        {d}_lo = band {d}_addr, 4294967295");
            let _ = writeln!(out, "        {d}_hi = ishl_imm.i64 {d}_span, 32");
            let _ = writeln!(out, "        {d} = bor {d}_lo, {d}_hi");
            let _ = writeln!(out, "        jump ok_{d}");
            let _ = writeln!(out, "    fail_{d}:");
            out.push_str("        v_bad = iconst.i32 1\n        v_z = iconst.i64 0\n");
            out.push_str(
                "        store.i32 v_bad, st\n        store.i64 v_z, val\n        return\n",
            );
            let _ = writeln!(out, "    ok_{d}:");
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
                .chain(["st".to_owned(), "val".to_owned(), "depth_next".to_owned()])
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "        call %{callee}({list})");
            // The shared `val` cell is 64 bits; 64-bit results (including
            // bit-carried floats) must load the full word, not the low 32.
            let load = match clif_ty(dest.ty) {
                "i64" => "load.i64",
                _ => "load.i32",
            };
            let _ = writeln!(out, "        {} = {load} val", names.value(&dest.id));
        }
    }
}

/// Encode a validated logical constant into the signed textual representation
/// accepted by Cranelift while preserving the bit pattern of unsigned lanes.
/// The scalar realization keeps every value in an i64 cell, so an unsigned
/// i64 constant above `i64::MAX` must be written as its two's-complement i64
/// spelling rather than silently replaced by zero.
fn clif_constant(value: i128, ty: ScalarTy) -> i64 {
    match ty {
        ScalarTy::Int(integer) if !integer.signed => value as u64 as i64,
        _ => value as i64,
    }
}

fn clif_ty(ty: ScalarTy) -> &'static str {
    match ty {
        ScalarTy::Int(integer) if integer.bits == 64 => "i64",
        ScalarTy::Cell | ScalarTy::View | ScalarTy::Mask(_) => "i64",
        // Floats ride bit-carried in uniform i64 cells; use sites bitcast.
        ScalarTy::Float => "i64",
        _ => "i32",
    }
}

/// Bit width of a scalar realization kind for conversion decisions.
fn bits_of(ty: ScalarTy) -> u16 {
    match ty {
        ScalarTy::Bool | ScalarTy::Finite => 32,
        ScalarTy::Byte => 8,
        ScalarTy::Int(integer) => integer.bits,
        ScalarTy::Cell | ScalarTy::View | ScalarTy::Mask(_) => 64,
        ScalarTy::Float => 64,
    }
}

/// Signedness of a scalar realization kind for widening decisions.
fn signed_of(ty: ScalarTy) -> bool {
    matches!(ty, ScalarTy::Int(integer) if integer.signed)
}

/// Text-CLIF realization of a bounded-sequence projection. Exact sequences
/// are canonical cells; views are packed descriptors whose low half holds
/// the base offset and whose high half holds the runtime length.
#[allow(clippy::too_many_arguments)]
fn emit_clif_sequence_project(
    out: &mut String,
    dest: &crate::scalar::ScalarValue,
    seq: &mncs_model::SemanticId,
    index: &mncs_model::SemanticId,
    bound: mncs_model::SequenceBound,
    evidence: &mncs_model::BoundsEvidence,
    width: crate::composite::SlotWidth,
    names: &ClifNames,
) {
    let d = names.value(&dest.id);
    let seq_n = names.value(seq);
    let idx_n = names.value(index);
    let checked = matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. });
    match bound {
        mncs_model::SequenceBound::Exact(length) => {
            if checked {
                let _ = writeln!(out, "        {d}_lim = iconst.i64 {length}");
                // Out-of-bounds is idx >= length; comparing against
                // length-1 keeps the unsigned comparison wrap-free at zero.
                if length == 0 {
                    let _ = writeln!(out, "        {d}_oob = iconst.i8 1");
                } else {
                    let _ = writeln!(out, "        {d}_lm1 = iconst.i64 {}", length - 1);
                    let _ = writeln!(out, "        {d}_oob = icmp ugt {idx_n}, {d}_lm1");
                }
                let _ = writeln!(out, "        brnz {d}_oob, fail_{d}");
            }
            let _ = writeln!(out, "        {d}_base = {}", seq_n);
            let _ = writeln!(out, "        {d}_off = ishl_imm.i64 {idx_n}, 3");
        }
        mncs_model::SequenceBound::UpTo(_) => {
            if checked {
                let _ = writeln!(out, "        {d}_len = ushr_imm.i64 {seq_n}, 32");
                // Views check against their runtime length; an empty view
                // fails every index.
                let _ = writeln!(out, "        {d}_oob = icmp uge {idx_n}, {d}_len");
                let _ = writeln!(out, "        brnz {d}_oob, fail_{d}");
            }
            let _ = writeln!(out, "        {d}_base = band {seq_n}, 4294967295");
            let _ = writeln!(out, "        {d}_off = ishl_imm.i64 {idx_n}, 3");
        }
        mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
            unreachable!("generic SequenceBound must be specialized before backend lowering")
        }
    }
    if checked {
        let _ = writeln!(out, "        jump ok_{d}");
        let _ = writeln!(out, "    fail_{d}:");
        out.push_str("        v_bad = iconst.i32 1\n        v_z = iconst.i64 0\n");
        out.push_str("        store.i32 v_bad, st\n        store.i64 v_z, val\n        return\n");
        let _ = writeln!(out, "    ok_{d}:");
    }
    match width {
        crate::composite::SlotWidth::W32 => {
            let _ = writeln!(out, "        {d} = load.u32 {d}_base, {d}_off");
        }
        crate::composite::SlotWidth::W64 => {
            let _ = writeln!(out, "        {d} = load.i64 {d}_base, {d}_off");
        }
    }
}

/// Emit width normalization for division operands in CLIF text: every value
/// slot is i64, so narrow operands are canonicalized to their declared width
/// (sign-extended for signed types, masked for unsigned) before the guards
/// and the native division. Returns the normalized operand names.
fn emit_width_normalize(
    out: &mut String,
    lhs_n: &str,
    rhs_n: &str,
    bits: u16,
    signed: bool,
    dest_n: &str,
) -> (String, String) {
    if bits >= 64 {
        return (lhs_n.to_owned(), rhs_n.to_owned());
    }
    let (lhs_v, rhs_v) = (format!("{dest_n}_ln"), format!("{dest_n}_rn"));
    if signed {
        let shift = 64 - i64::from(bits);
        let _ = writeln!(out, "        {lhs_v} = ishl_imm.i64 {lhs_n}, {shift}");
        let _ = writeln!(out, "        {lhs_v} = sshr_imm.i64 {lhs_v}, {shift}");
        let _ = writeln!(out, "        {rhs_v} = ishl_imm.i64 {rhs_n}, {shift}");
        let _ = writeln!(out, "        {rhs_v} = sshr_imm.i64 {rhs_v}, {shift}");
    } else {
        let mask = (1_i128 << bits) - 1;
        let _ = writeln!(out, "        {lhs_v} = band {lhs_n}, {mask}");
        let _ = writeln!(out, "        {rhs_v} = band {rhs_n}, {mask}");
    }
    (lhs_v, rhs_v)
}

/// Flatten sequence groups so naming and emission walk every concrete
/// instruction exactly once.
fn flatten_scalar(insts: &[ScalarInst]) -> Vec<&ScalarInst> {
    let mut flat = Vec::new();
    for inst in insts {
        match inst {
            ScalarInst::Sequence(nested) => flat.extend(flatten_scalar(nested)),
            other => flat.push(other),
        }
    }
    flat
}

fn scalar_dest(inst: &ScalarInst) -> Option<&crate::scalar::ScalarValue> {
    match inst {
        ScalarInst::Const { dest, .. }
        | ScalarInst::FloatConst { dest, .. }
        | ScalarInst::Float { dest, .. }
        | ScalarInst::FloatIntrinsic { dest, .. }
        | ScalarInst::FloatCompare { dest, .. }
        | ScalarInst::Integer { dest, .. }
        | ScalarInst::Boolean { dest, .. }
        | ScalarInst::BooleanCompare { dest, .. }
        | ScalarInst::BooleanNot { dest, .. }
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
            for inst in flatten_scalar(&block.insts) {
                let Some(dest) = scalar_dest(inst) else {
                    continue;
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
    let Some(contract) = crate::support::entry_value_contract(
        &artifact.function_value_contracts,
        &request.target.module,
        &request.target.function,
    ) else {
        return execution_failure(
            result,
            ExecutionStatus::InvalidRequest,
            "Cranelift execution requires a language-owned function value contract",
        );
    };
    // Canonical boundary values (scalar bits or cell roots) shared by the
    // JIT and AOT realizations so both behave identically.
    let (raw_args, arena_image) = match jit_boundary_arguments(
        &payload,
        &contract.inputs,
        contract.outputs.first(),
        &artifact.composite_value_contracts,
        request,
    ) {
        Ok(outcome) => outcome,
        Err(reason) => {
            return execution_failure(result, ExecutionStatus::InvalidRequest, reason);
        }
    };
    if let Some(image) = &arena_image {
        // Install the canonical argument arena before the call.
        with_jit_arena(|arena| {
            arena.clear();
            arena.extend_from_slice(image);
        });
    }
    clear_jit_failure_state();
    // RFC 0047 §5 uniform fuel: seed the entry depth from the request
    // budget so an explicit budget means the same fuel here as on the
    // reference interpreters (zero seed is the historical full cap).
    let entry_depth = match crate::support::depth_seed_for_request(request) {
        Ok(seed) => seed as i64,
        Err(reason) => {
            return execution_failure(result, ExecutionStatus::InvalidRequest, reason);
        }
    };
    // The JIT trampoline table is keyed by module-qualified native symbol
    // (ENG-PRESSURE-0017); resolve the entry the same way here.
    let entry = crate::support::entry_native_symbol(
        &payload.exports,
        &request.target.module,
        &request.target.function,
    );
    match jit_execute_with_arguments(&payload, &entry, &raw_args, entry_depth) {
        Ok((status, value)) => {
            if JIT_OOB.load(std::sync::atomic::Ordering::Relaxed) {
                // A host slot access escaped the installed arena image.
                // Allocation-cap exhaustion is a bounded resource failure
                // with the attributed request/arena detail; anything else
                // (wild address) fails closed as a runtime failure instead
                // of decoding possibly-zeroed cells (WEB-P-012).
                let diagnosis = take_jit_diagnosis();
                if JIT_EXHAUSTED.load(std::sync::atomic::Ordering::Relaxed) {
                    return execution_failure(
                        result,
                        ExecutionStatus::BudgetExhausted,
                        diagnosis.unwrap_or_else(|| {
                            "MNCS_RSRC_EXHAUSTED cranelift JIT canonical arena exhausted".to_owned()
                        }),
                    );
                }
                return execution_failure(
                    result,
                    ExecutionStatus::RuntimeFailure,
                    diagnosis.unwrap_or_else(|| {
                        "cranelift JIT cell access exceeded the arena image; failing closed"
                            .to_owned()
                    }),
                );
            }
            // WEB-P-012: a non-returned JIT status carries an attributed
            // reason instead of a silent null.
            if status != ExecutionStatus::Returned {
                return execution_failure(
                    result,
                    status,
                    format!("cranelift JIT execution ended with status {status:?}"),
                );
            }
            result.status = status;
            result.steps = 1;
            match crate::support::decode_native_observation(
                contract.outputs.first(),
                status,
                value,
                read_jit_arena_hex(arena_image.is_some()).as_deref(),
                &artifact.composite_value_contracts,
            ) {
                Ok(returned) => {
                    result.returned = returned;
                    result
                }
                Err(reason) => execution_failure(result, ExecutionStatus::InvalidRequest, reason),
            }
        }
        Err(reason) => {
            // Host JIT may be denied executable memory by policy. That is an
            // environment restriction, not a lowering or semantic failure;
            // fall back to the AOT object + external-link route when a
            // toolchain is present.
            if reason.contains("readable+executable") || reason.contains("executable memory") {
                match aot_fallback_execute(artifact, &payload, request, &raw_args, &arena_image) {
                    Ok(run) => {
                        // WEB-P-012: attribute non-returned observations;
                        // never a silent null reason.
                        if run.status != ExecutionStatus::Returned {
                            let reason = run.reason.unwrap_or_else(|| {
                                format!(
                                    "native execution ended with status {:?} and no attributed reason",
                                    run.status
                                )
                            });
                            return execution_failure(result, run.status, reason);
                        }
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
                            Err(reason) => {
                                execution_failure(result, ExecutionStatus::InvalidRequest, reason)
                            }
                        }
                    }
                    Err(fallback_error) => execution_failure(
                        result,
                        ExecutionStatus::Unsupported,
                        format!("jit unavailable ({reason}); aot fallback: {fallback_error}"),
                    ),
                }
            } else {
                execution_failure(result, ExecutionStatus::Unsupported, reason)
            }
        }
    }
}

/// AOT fallback: emit a host-ISA ELF object from the same scalar lowering,
/// link it against the cell-runtime process driver, and observe it externally.
fn aot_fallback_execute(
    artifact: &mncs_model::BackendArtifact,
    payload: &CraneliftPayload,
    request: &ExecutionRequest,
    raw_args: &[i64],
    arena_image: &Option<Vec<u8>>,
) -> Result<crate::support::NativeRunView, String> {
    use crate::native::{compile_object_and_run_full, probe_clang, probe_gcc};
    let Some(contract) = crate::support::entry_value_contract(
        &artifact.function_value_contracts,
        &request.target.module,
        &request.target.function,
    ) else {
        return Err(
            "Cranelift AOT execution requires a language-owned function value contract".to_owned(),
        );
    };
    if arena_image.is_none() && contract.inputs.iter().any(crate::support::contract_is_cell) {
        return Err(
            "composite parameters require a canonical call file for this request".to_owned(),
        );
    }
    let names = function_names(&payload.program, &payload.ssa);
    let scalar = lower_to_scalar(&payload.program, &payload.ssa, &names);
    let object = aot_object_bytes(&scalar)?;
    let linker = probe_clang()
        .or_else(probe_gcc)
        .ok_or_else(|| "neither clang nor gcc is present".to_owned())?;
    // The linked object contains every module function, so the driver must
    // define the cell runtime whenever any function manipulates cells.
    // The entry symbol is module-qualified (ENG-PRESSURE-0017).
    let entry = crate::support::entry_native_symbol(
        &payload.exports,
        &request.target.module,
        &request.target.function,
    );
    // RFC 0047 §5 uniform fuel (see `execute_cranelift`).
    let entry_depth = crate::support::depth_seed_for_request(request)?;
    let driver = if crate::support::scalar_module_needs_arena_symbols(&scalar) {
        crate::support::process_driver_cell_runtime(
            &entry,
            &contract.inputs,
            contract.outputs.first(),
            entry_depth,
        )
    } else {
        crate::support::process_driver(
            &entry,
            &contract.inputs,
            contract.outputs.first(),
            entry_depth,
        )
    };
    // The C driver parses scalar words itself (`strtod` for floats), so
    // argv carries the decimal request spellings, not the bit-carried
    // JIT words: re-stringified float bits would parse as huge decimals.
    let argv: Vec<String> = if arena_image.is_some() {
        Vec::new()
    } else {
        crate::native::argv_from_request(request)?
    };
    let call_path = arena_image.as_ref().map(|image| {
        let mut blob = Vec::new();
        blob.extend_from_slice(&0x4d4e435331_u64.to_le_bytes());
        blob.extend_from_slice(&(raw_args.len() as u64).to_le_bytes());
        for (index, argument) in raw_args.iter().enumerate() {
            let kind = if contract
                .inputs
                .get(index)
                .is_some_and(crate::support::contract_is_cell)
            {
                1u64
            } else {
                0u64
            };
            blob.extend_from_slice(&kind.to_le_bytes());
            blob.extend_from_slice(&(*argument as u64).to_le_bytes());
        }
        blob.extend_from_slice(&(image.len() as u64).to_le_bytes());
        blob.extend_from_slice(image);
        let digest = crate::support::sha256_hex(&blob);
        let path = std::env::temp_dir().join(format!("mncs-call-{digest}.bin"));
        let _ = std::fs::write(&path, &blob);
        path
    });
    compile_object_and_run_full(
        "mncs.o",
        &object,
        "driver.c",
        &driver,
        &linker,
        &argv,
        call_path.as_deref(),
    )
    .map_err(|error| format!("{:?}: {}", error.status(), error.reason()))
}

fn jit_execute_with_arguments(
    payload: &CraneliftPayload,
    function_name: &str,
    raw_args: &[i64],
    entry_depth: i64,
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
    jit_scalar(&scalar, function_name, raw_args, entry_depth)
}

fn integer_bounds(bits: u16, signed: bool) -> (i64, i64) {
    if bits == 0 {
        return (0, 0);
    }
    if !signed {
        // u64 values ride in signed i64 cells, so the upper bound is the
        // all-ones bit pattern (`-1` as i64, `2^64 - 1` unsigned). Callers
        // widen through `uextend`, which recovers the true magnitude.
        if bits >= 64 {
            (0, -1)
        } else if bits >= 63 {
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

/// Sliver-safe `(bad-if predicate, lo, hi)` for a guarded float-to-integer
/// conversion (Profile 0.12), as host f64 values. The range check runs on
/// the untruncated value: the upper bound is exact as-is, while the lower
/// bound admits the fractional sliver `(lo - 1, lo)` whose truncation still
/// lands in domain. The i64 sliver is empty, so plain less-than is exact
/// there; narrower signed domains spell `lo - 1` exactly; unsigned uses
/// `v <= -1`, which also admits `-0.0` (truncates to zero, in domain).
fn float_guard_bounds(to: ScalarTy) -> (cranelift_codegen::ir::condcodes::FloatCC, f64, f64) {
    use cranelift_codegen::ir::condcodes::FloatCC;
    let bits = bits_of(to);
    let signed = signed_of(to);
    let hi = 2f64.powi(if signed {
        i32::from(bits) - 1
    } else {
        i32::from(bits)
    });
    if signed && bits == 64 {
        (FloatCC::LessThan, -2f64.powi(63), hi)
    } else if signed {
        (
            FloatCC::LessThanOrEqual,
            -2f64.powi(i32::from(bits) - 1) - 1.0,
            hi,
        )
    } else {
        (FloatCC::LessThanOrEqual, -1.0, hi)
    }
}

/// Host ISA for JIT and AOT realizations (same target facts).
fn host_isa() -> Result<std::sync::Arc<dyn cranelift_codegen::isa::TargetIsa>, String> {
    use cranelift_codegen::settings::{self, Configurable};
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
    isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|error| error.to_string())
}

/// Emit a native ELF object for the host ISA from the scalar module. This is
/// the AOT fallback used where JIT executable-memory policy blocks in-process
/// execution; the object links against the shared process driver.
pub fn aot_object_bytes(scalar: &ScalarModule) -> Result<Vec<u8>, String> {
    use cranelift_object::{ObjectBuilder, ObjectModule};
    let isa = host_isa()?;
    let builder = ObjectBuilder::new(
        isa,
        "mncs".to_string().into_bytes(),
        cranelift_module::default_libcall_names(),
    )
    .map_err(|error| error.to_string())?;
    let mut object_module = ObjectModule::new(builder);
    declare_and_build(&mut object_module, scalar)?;
    let product = object_module.finish();
    let mut written = Vec::new();
    product
        .object
        .write_stream(&mut written)
        .map_err(|error| error.to_string())?;
    Ok(written)
}

/// Declare (once per function build) one of the canonical-cell runtime
/// entry points. All share the uniform i64 signature family:
///   mncs_cell_alloc(bytes) -> offset
///   mncs_slot_store32/64(at, value)
///   mncs_slot_load32/64(at) -> zero-extended value
/// Trigonometry shims (Profile 0.12): same-process libm, exactly like
/// the reference executor. Registered under the plain C library names so
/// AOT objects resolve them from libm at link time with no driver change.
extern "C" fn mncs_sin_shim(x: f64) -> f64 {
    x.sin()
}

/// Trigonometry shims (Profile 0.12): same-process libm, exactly like
/// the reference executor. Registered under the plain C library names so
/// AOT objects resolve them from libm at link time with no driver change.
extern "C" fn mncs_cos_shim(x: f64) -> f64 {
    x.cos()
}

fn module_uses_trig(module: &ScalarModule) -> bool {
    module.functions.iter().any(|function| {
        function.blocks.iter().any(|block| {
            block
                .insts
                .iter()
                .any(|inst| matches!(inst, ScalarInst::FloatIntrinsic { .. }))
        })
    })
}

/// Declare (once per function build) one of the trigonometry entry
/// points. Both share the uniform binary64 signature `sin/cos(f64)`.
fn trig_libcall<M: cranelift_module::Module>(
    module: &mut M,
    func: &mut cranelift_codegen::ir::Function,
    name: &str,
) -> cranelift_codegen::ir::FuncRef {
    use cranelift_codegen::ir::{types, AbiParam, Signature};
    use cranelift_module::Linkage;
    let mut sig = Signature::new(module.target_config().default_call_conv);
    sig.params.push(AbiParam::new(types::F64));
    sig.returns.push(AbiParam::new(types::F64));
    let id = module
        .declare_function(name, Linkage::Import, &sig)
        .expect("declare trig libcall");
    module.declare_func_in_func(id, func)
}

fn cell_libcall<M: cranelift_module::Module>(
    module: &mut M,
    func: &mut cranelift_codegen::ir::Function,
    name: &'static str,
) -> cranelift_codegen::ir::FuncRef {
    use cranelift_codegen::ir::{types, AbiParam, Signature};
    use cranelift_module::Linkage;
    let mut sig = Signature::new(module.target_config().default_call_conv);
    match name {
        "mncs_slot_store32" | "mncs_slot_store64" => {
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
        }
        "mncs_cell_alloc" | "mncs_slot_load32" | "mncs_slot_load64" => {
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
        }
        _ => unreachable!("unknown cell libcall {name}"),
    }
    let id = module
        .declare_function(name, Linkage::Import, &sig)
        .expect("declare cell libcall");
    module.declare_func_in_func(id, func)
}

/// Build every scalar function into any Cranelift module realization.
fn declare_and_build<M>(
    module: &mut M,
    scalar: &ScalarModule,
) -> Result<std::collections::BTreeMap<String, cranelift_module::FuncId>, String>
where
    M: cranelift_module::Module,
{
    use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
    use cranelift_codegen::ir::immediates::Ieee64;
    use cranelift_codegen::ir::{types, AbiParam, BlockArg, InstBuilder, MemFlags, Value};
    use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
    use cranelift_module::{FuncId, Linkage, Module};
    use mncs_model::SemanticId;
    use std::collections::BTreeMap;

    let mut declared: BTreeMap<String, FuncId> = BTreeMap::new();
    for function in &scalar.functions {
        let mut sig = module.make_signature();
        for _ in 0..function.params.len() {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        // RFC 0047 §5: trailing call-depth fuel actually consumed by the
        // machine prologue below.
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
        // RFC 0047 §5: trailing call-depth fuel; mirrors the declare pass.
        ctx.func.signature.params.push(AbiParam::new(types::I64));
        {
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fn_ctx);
            let mut blocks = BTreeMap::new();
            let mut values: BTreeMap<SemanticId, Value> = BTreeMap::new();
            // Value types for boxed-variant decisions during building.
            let mut value_types: BTreeMap<SemanticId, crate::scalar::ScalarTy> = BTreeMap::new();
            {
                fn record_type(
                    value: &crate::scalar::ScalarValue,
                    map: &mut BTreeMap<SemanticId, crate::scalar::ScalarTy>,
                ) {
                    map.insert(value.id.clone(), value.ty);
                }
                for param in &function.params {
                    record_type(param, &mut value_types);
                }
                for block in &function.blocks {
                    for param in &block.params {
                        record_type(param, &mut value_types);
                    }
                    for inst in flatten_scalar(&block.insts) {
                        if let Some(dest) = scalar_dest(inst) {
                            record_type(dest, &mut value_types);
                        }
                    }
                }
            }
            let value_is_cell = |id: &SemanticId| {
                value_types
                    .get(id)
                    .copied()
                    .unwrap_or(crate::scalar::ScalarTy::Finite)
                    .is_cell()
            };
            for block in &function.blocks {
                blocks.insert(block.id.clone(), builder.create_block());
            }
            let fail = builder.create_block();
            // Reserved reinterpret slot for the float bit-carries.
            let f64slot =
                builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                    cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                    8,
                    0,
                ));
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
            let depth = fn_params[function.params.len() + 2];
            // RFC 0047 §5: static call-depth fuel. The host trampoline seeds
            // depth 0 and every same-module call passes depth + 1, so
            // unbounded self-recursion fails closed instead of overflowing
            // the native stack. The check sits in the entry block ahead of
            // the body, so a depth over MODEL_MAX_CALL_DEPTH never executes
            // user code. Exhaustion reports the dedicated status code 3
            // (BudgetExhausted, matching the reference interpreters), never
            // the generic failure code 1: the dedicated `exhausted` block
            // keeps fuel accounting observably distinct from user failure.
            let depth_limit = builder
                .ins()
                .iconst(types::I64, MODEL_MAX_CALL_DEPTH as i64);
            let over_depth = builder
                .ins()
                .icmp(IntCC::UnsignedGreaterThan, depth, depth_limit);
            let depth_ok = builder.create_block();
            let exhausted = builder.create_block();
            builder.ins().brif(
                over_depth,
                exhausted,
                &[] as &[BlockArg],
                depth_ok,
                &[] as &[BlockArg],
            );
            builder.switch_to_block(depth_ok);
            builder.seal_block(depth_ok);
            builder.switch_to_block(exhausted);
            builder.seal_block(exhausted);
            let exhausted_status = builder.ins().iconst(types::I32, 3);
            let exhausted_value = builder.ins().iconst(types::I64, 0);
            builder
                .ins()
                .store(MemFlags::trusted(), exhausted_status, status_ptr, 0);
            builder
                .ins()
                .store(MemFlags::trusted(), exhausted_value, value_ptr, 0);
            builder.ins().return_(&[]);
            // The body loop below emits scalar block 0 into the current
            // block (the historical layout puts the prologue alone in the
            // entry block and the first body in `depth_ok`); restore that
            // position. `entry` itself is filled by the depth brif.
            builder.switch_to_block(depth_ok);
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
                for inst in flatten_scalar(&block.insts) {
                    match inst {
                        ScalarInst::Const { dest, value } => {
                            let produced = builder
                                .ins()
                                .iconst(types::I64, clif_constant(*value, dest.ty));
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::Boolean {
                            dest,
                            operator,
                            lhs,
                            rhs,
                        } => {
                            let left = values[lhs];
                            let right = values[rhs];
                            let produced = match operator.as_str() {
                                "and" => builder.ins().band(left, right),
                                _ => builder.ins().bor(left, right),
                            };
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::BooleanCompare {
                            dest,
                            predicate,
                            lhs,
                            rhs,
                        } => {
                            // Normalized 0/1 I64 cells: `icmp` plus `uextend`
                            // exactly like `Compare`.
                            let cc = match predicate.as_str() {
                                "eq" => IntCC::Equal,
                                "ne" => IntCC::NotEqual,
                                _ => IntCC::Equal,
                            };
                            let flag = builder.ins().icmp(cc, values[lhs], values[rhs]);
                            let produced = builder.ins().uextend(types::I64, flag);
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::BooleanNot { dest, src } => {
                            // Normalized 0/1 I64 cell: equality-against-zero
                            // via proven `icmp_imm`/`uextend` primitives.
                            let flag = builder.ins().icmp_imm(IntCC::Equal, values[src], 0);
                            let produced = builder.ins().uextend(types::I64, flag);
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::FloatConst { dest, bits } => {
                            let produced = builder.ins().iconst(types::I64, *bits as i64);
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::Float {
                            dest,
                            operator,
                            lhs,
                            rhs,
                        } => {
                            // Uniform i64 cells bitcast at use; operands and
                            // result guard finite into the shared `fail`
                            // block, like the division guards.
                            let left = jit_bits_to_f64(&mut builder, f64slot, values[lhs]);
                            let right = jit_bits_to_f64(&mut builder, f64slot, values[rhs]);
                            let zero = builder.ins().f64const(Ieee64::with_bits(0));
                            for value in [left, right] {
                                let diff = builder.ins().fsub(value, value);
                                let bad = builder.ins().fcmp(FloatCC::NotEqual, diff, zero);
                                let cont = builder.create_block();
                                builder.ins().brif(
                                    bad,
                                    fail,
                                    &[] as &[BlockArg],
                                    cont,
                                    &[] as &[BlockArg],
                                );
                                builder.switch_to_block(cont);
                                builder.seal_block(cont);
                            }
                            let computed = match operator.as_str() {
                                "add" => builder.ins().fadd(left, right),
                                "sub" => builder.ins().fsub(left, right),
                                "mul" => builder.ins().fmul(left, right),
                                "div" => builder.ins().fdiv(left, right),
                                // Unreachable: scalar lowering rejects
                                // unknown float operators first.
                                _ => builder.ins().fadd(left, right),
                            };
                            let diff = builder.ins().fsub(computed, computed);
                            let bad = builder.ins().fcmp(FloatCC::NotEqual, diff, zero);
                            let cont = builder.create_block();
                            builder.ins().brif(
                                bad,
                                fail,
                                &[] as &[BlockArg],
                                cont,
                                &[] as &[BlockArg],
                            );
                            builder.switch_to_block(cont);
                            builder.seal_block(cont);
                            let produced = jit_f64_to_bits(&mut builder, f64slot, computed);
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::FloatIntrinsic {
                            dest,
                            function,
                            src,
                        } => {
                            // Uniform i64 cells bitcast at use; the operand
                            // and result guard finite into the shared `fail`
                            // block. The call reaches same-process libm
                            // through the declared import (JIT shims; AOT
                            // resolves `sin`/`cos` from libm at link time).
                            if !matches!(function.as_str(), "sin" | "cos") {
                                let always = builder.ins().iconst(types::I8, 1);
                                let dead = builder.create_block();
                                builder.ins().brif(
                                    always,
                                    fail,
                                    &[] as &[BlockArg],
                                    dead,
                                    &[] as &[BlockArg],
                                );
                                builder.switch_to_block(dead);
                                builder.seal_block(dead);
                            }
                            let input = jit_bits_to_f64(&mut builder, f64slot, values[src]);
                            let zero = builder.ins().f64const(Ieee64::with_bits(0));
                            let diff = builder.ins().fsub(input, input);
                            let bad = builder.ins().fcmp(FloatCC::NotEqual, diff, zero);
                            let cont = builder.create_block();
                            builder.ins().brif(
                                bad,
                                fail,
                                &[] as &[BlockArg],
                                cont,
                                &[] as &[BlockArg],
                            );
                            builder.switch_to_block(cont);
                            builder.seal_block(cont);
                            let callee = trig_libcall(module, builder.func, function.as_str());
                            let call = builder.ins().call(callee, &[input]);
                            let computed = builder.inst_results(call)[0];
                            let diff = builder.ins().fsub(computed, computed);
                            let bad = builder.ins().fcmp(FloatCC::NotEqual, diff, zero);
                            let cont = builder.create_block();
                            builder.ins().brif(
                                bad,
                                fail,
                                &[] as &[BlockArg],
                                cont,
                                &[] as &[BlockArg],
                            );
                            builder.switch_to_block(cont);
                            builder.seal_block(cont);
                            let produced = jit_f64_to_bits(&mut builder, f64slot, computed);
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::FloatCompare {
                            dest,
                            predicate,
                            lhs,
                            rhs,
                        } => {
                            let left = jit_bits_to_f64(&mut builder, f64slot, values[lhs]);
                            let right = jit_bits_to_f64(&mut builder, f64slot, values[rhs]);
                            let zero = builder.ins().f64const(Ieee64::with_bits(0));
                            for value in [left, right] {
                                let diff = builder.ins().fsub(value, value);
                                let bad = builder.ins().fcmp(FloatCC::NotEqual, diff, zero);
                                let cont = builder.create_block();
                                builder.ins().brif(
                                    bad,
                                    fail,
                                    &[] as &[BlockArg],
                                    cont,
                                    &[] as &[BlockArg],
                                );
                                builder.switch_to_block(cont);
                                builder.seal_block(cont);
                            }
                            // Operands are guarded finite, so the ordered
                            // codes never observe NaN.
                            let cc = match predicate.as_str() {
                                "eq" => FloatCC::Equal,
                                "ne" => FloatCC::NotEqual,
                                "lt" => FloatCC::LessThan,
                                "le" => FloatCC::LessThanOrEqual,
                                "gt" => FloatCC::GreaterThan,
                                _ => FloatCC::GreaterThanOrEqual,
                            };
                            let flag = builder.ins().fcmp(cc, left, right);
                            let produced = builder.ins().uextend(types::I64, flag);
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
                            if matches!(operator.as_str(), "div" | "mod") {
                                // Division never relies on native traps. A
                                // zero divisor and, for signed division,
                                // MIN / -1 branch to the structured failure
                                // block; unsigned operands use the unsigned
                                // division forms after width normalization.
                                // This keeps JIT and AOT realizations
                                // semantically identical to the reference
                                // executors instead of killing the host
                                // process on a hardware trap.
                                let integer_ty = match dest.ty {
                                    ScalarTy::Int(integer) => integer,
                                    _ => mncs_model::IntegerType {
                                        bits: 32,
                                        signed: true,
                                    },
                                };
                                let (left_n, right_n) = normalize_width(
                                    &mut builder,
                                    left,
                                    right,
                                    integer_ty.bits,
                                    integer_ty.signed,
                                );
                                let zero = builder.ins().iconst(types::I64, 0);
                                let dz = builder.ins().icmp(IntCC::Equal, right_n, zero);
                                let cont = builder.create_block();
                                builder.ins().brif(
                                    dz,
                                    fail,
                                    &[] as &[BlockArg],
                                    cont,
                                    &[] as &[BlockArg],
                                );
                                builder.switch_to_block(cont);
                                builder.seal_block(cont);
                                if operator == "div" && integer_ty.signed {
                                    let minus_one = builder.ins().iconst(types::I64, -1);
                                    let min = if integer_ty.bits >= 64 {
                                        i64::MIN
                                    } else {
                                        -(1_i64 << (integer_ty.bits - 1))
                                    };
                                    let min = builder.ins().iconst(types::I64, min);
                                    let m1 = builder.ins().icmp(IntCC::Equal, right_n, minus_one);
                                    let mn = builder.ins().icmp(IntCC::Equal, left_n, min);
                                    let ov = builder.ins().band(m1, mn);
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
                                let produced = match (operator.as_str(), integer_ty.signed) {
                                    ("div", false) => builder.ins().udiv(left_n, right_n),
                                    ("mod", false) => builder.ins().urem(left_n, right_n),
                                    ("mod", true) => builder.ins().srem(left_n, right_n),
                                    _ => builder.ins().sdiv(left_n, right_n),
                                };
                                values.insert(dest.id.clone(), produced);
                                continue;
                            }
                            if matches!(operator.as_str(), "and" | "or" | "xor") {
                                let produced = match operator.as_str() {
                                    "and" => builder.ins().band(left, right),
                                    "or" => builder.ins().bor(left, right),
                                    _ => builder.ins().bxor(left, right),
                                };
                                values.insert(dest.id.clone(), produced);
                                continue;
                            }
                            if matches!(operator.as_str(), "shl" | "shr") {
                                // Total shifts in the uniform i64 cell:
                                // normalize into the declared domain, apply
                                // the modulo count, renormalize the result.
                                let bits = match dest.ty {
                                    ScalarTy::Int(integer) => integer.bits,
                                    ScalarTy::Cell | ScalarTy::View | ScalarTy::Mask(_) => 64,
                                    _ => 32,
                                };
                                let signed =
                                    matches!(dest.ty, ScalarTy::Int(integer) if integer.signed);
                                let left_n = normalize_one(&mut builder, left, bits, signed);
                                let width_bits = builder.ins().iconst(types::I64, i64::from(bits));
                                let count = builder.ins().urem(right, width_bits);
                                let mut produced = if operator == "shl" {
                                    builder.ins().ishl(left_n, count)
                                } else if signed {
                                    builder.ins().sshr(left_n, count)
                                } else {
                                    builder.ins().ushr(left_n, count)
                                };
                                if operator == "shl" && bits < 64 {
                                    produced = normalize_one(&mut builder, produced, bits, signed);
                                }
                                values.insert(dest.id.clone(), produced);
                                continue;
                            }
                            let bits = match dest.ty {
                                ScalarTy::Int(integer) => integer.bits,
                                _ => 32,
                            };
                            let signed =
                                matches!(dest.ty, ScalarTy::Int(integer) if integer.signed);
                            if matches!(intent, ArithmeticIntent::Saturating) {
                                // Saturating arithmetic is total by definition:
                                // widen to i128 so the wide result can never
                                // wrap, clamp into the declared range, then
                                // narrow back. This is exact at every width.
                                let wide_ty = types::I128;
                                let (min, max) = integer_bounds(bits, signed);
                                let left_wide = if signed {
                                    builder.ins().sextend(wide_ty, left)
                                } else {
                                    builder.ins().uextend(wide_ty, left)
                                };
                                let right_wide = if signed {
                                    builder.ins().sextend(wide_ty, right)
                                } else {
                                    builder.ins().uextend(wide_ty, right)
                                };
                                let produced_wide = match operator.as_str() {
                                    "sub" => builder.ins().isub(left_wide, right_wide),
                                    "mul" => builder.ins().imul(left_wide, right_wide),
                                    _ => builder.ins().iadd(left_wide, right_wide),
                                };
                                // Every declared range fits an i64 cell, so
                                // boundaries are built there and widened.
                                let max64 = builder.ins().iconst(types::I64, max);
                                let max_v = if signed {
                                    builder.ins().sextend(wide_ty, max64)
                                } else {
                                    builder.ins().uextend(wide_ty, max64)
                                };
                                let over = builder.ins().icmp(
                                    if signed {
                                        IntCC::SignedGreaterThan
                                    } else {
                                        IntCC::UnsignedGreaterThan
                                    },
                                    produced_wide,
                                    max_v,
                                );
                                let min64 = builder.ins().iconst(types::I64, min);
                                let min_v = if signed {
                                    builder.ins().sextend(wide_ty, min64)
                                } else {
                                    builder.ins().uextend(wide_ty, min64)
                                };
                                let under = builder.ins().icmp(
                                    if signed {
                                        IntCC::SignedLessThan
                                    } else {
                                        IntCC::UnsignedLessThan
                                    },
                                    produced_wide,
                                    min_v,
                                );
                                let clamped_high = builder.ins().select(over, max_v, produced_wide);
                                let clamped = builder.ins().select(under, min_v, clamped_high);
                                let narrowed = builder.ins().ireduce(types::I64, clamped);
                                values.insert(dest.id.clone(), narrowed);
                                continue;
                            }
                            let mut produced = match operator.as_str() {
                                "sub" => builder.ins().isub(left, right),
                                "mul" => builder.ins().imul(left, right),
                                _ => builder.ins().iadd(left, right),
                            };
                            let needs_overflow_guard = matches!(
                                intent,
                                ArithmeticIntent::Checked | ArithmeticIntent::Trapping
                            ) && !promise.decision.permitted;
                            if !needs_overflow_guard && bits < 64 {
                                // Narrow wrapping results are normalized back
                                // into their declared width inside the shared
                                // i64 cell so a wrapping edge value (for
                                // example `0 -% 1` in u16) remains
                                // representable instead of surfacing as an
                                // unrelated negative i64. The guard below must
                                // observe the raw value, so normalization is
                                // skipped when a guard will run.
                                if signed {
                                    let shift =
                                        builder.ins().iconst(types::I64, i64::from(64 - bits));
                                    let widened = builder.ins().ishl(produced, shift);
                                    produced = builder.ins().sshr(widened, shift)
                                } else {
                                    let mask = builder
                                        .ins()
                                        .iconst(types::I64, ((1i128 << bits) - 1) as i64);
                                    produced = builder.ins().band(produced, mask)
                                }
                            }
                            if needs_overflow_guard {
                                if bits >= 64 {
                                    // A wrapped i64 result can no longer be
                                    // distinguished from a legal one after
                                    // the fact; recompute in i128 where the
                                    // overflow is exactly detectable.
                                    let wide_ty = types::I128;
                                    let (min, max) = integer_bounds(bits, signed);
                                    let left_wide = if signed {
                                        builder.ins().sextend(wide_ty, left)
                                    } else {
                                        builder.ins().uextend(wide_ty, left)
                                    };
                                    let right_wide = if signed {
                                        builder.ins().sextend(wide_ty, right)
                                    } else {
                                        builder.ins().uextend(wide_ty, right)
                                    };
                                    let produced_wide = match operator.as_str() {
                                        "sub" => builder.ins().isub(left_wide, right_wide),
                                        "mul" => builder.ins().imul(left_wide, right_wide),
                                        _ => builder.ins().iadd(left_wide, right_wide),
                                    };
                                    let max64 = builder.ins().iconst(types::I64, max);
                                    let max_v = if signed {
                                        builder.ins().sextend(wide_ty, max64)
                                    } else {
                                        builder.ins().uextend(wide_ty, max64)
                                    };
                                    let min64 = builder.ins().iconst(types::I64, min);
                                    let min_v = if signed {
                                        builder.ins().sextend(wide_ty, min64)
                                    } else {
                                        builder.ins().uextend(wide_ty, min64)
                                    };
                                    let hi = builder.ins().icmp(
                                        if signed {
                                            IntCC::SignedGreaterThan
                                        } else {
                                            IntCC::UnsignedGreaterThan
                                        },
                                        produced_wide,
                                        max_v,
                                    );
                                    let lo = builder.ins().icmp(
                                        if signed {
                                            IntCC::SignedLessThan
                                        } else {
                                            IntCC::UnsignedLessThan
                                        },
                                        produced_wide,
                                        min_v,
                                    );
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
                                } else {
                                    let (min, max) = integer_bounds(bits, signed);
                                    let max_v = builder.ins().iconst(types::I64, max);
                                    let min_v = builder.ins().iconst(types::I64, min);
                                    let hi = builder.ins().icmp(
                                        IntCC::SignedGreaterThan,
                                        produced,
                                        max_v,
                                    );
                                    let lo =
                                        builder.ins().icmp(IntCC::SignedLessThan, produced, min_v);
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
                        ScalarInst::CellAlloc { dest, bytes } => {
                            let callee = cell_libcall(module, builder.func, "mncs_cell_alloc");
                            let bytes_v = builder.ins().iconst(types::I64, *bytes as i64);
                            let call = builder.ins().call(callee, &[bytes_v]);
                            values.insert(dest.id.clone(), builder.inst_results(call)[0]);
                        }
                        ScalarInst::CellStoreDiscriminant { cell, discriminant } => {
                            let callee = cell_libcall(module, builder.func, "mncs_slot_store32");
                            let zero = builder.ins().iconst(types::I64, 0);
                            let addr = builder.ins().iadd(values[cell], zero);
                            let disc = builder.ins().iconst(types::I64, i64::from(*discriminant));
                            let args = [addr, disc];
                            builder.ins().call(callee, &args);
                        }
                        ScalarInst::CellStore {
                            cell,
                            byte_offset,
                            width,
                            value,
                        } => {
                            let name = match width {
                                crate::composite::SlotWidth::W32 => "mncs_slot_store32",
                                crate::composite::SlotWidth::W64 => "mncs_slot_store64",
                            };
                            let callee = cell_libcall(module, builder.func, name);
                            let offset = builder.ins().iconst(types::I64, *byte_offset as i64);
                            let addr = builder.ins().iadd(values[cell], offset);
                            let args = [addr, values[value]];
                            builder.ins().call(callee, &args);
                        }
                        ScalarInst::CellLoad {
                            dest,
                            cell,
                            byte_offset,
                            width,
                        } => {
                            let name = match width {
                                crate::composite::SlotWidth::W32 => "mncs_slot_load32",
                                crate::composite::SlotWidth::W64 => "mncs_slot_load64",
                            };
                            let callee = cell_libcall(module, builder.func, name);
                            let offset = builder.ins().iconst(types::I64, *byte_offset as i64);
                            let addr = builder.ins().iadd(values[cell], offset);
                            let call = builder.ins().call(callee, &[addr]);
                            // Load libcalls return zero-extended i64 slot
                            // payloads. Restore the semantic lane width and
                            // signedness before later arithmetic/comparison.
                            let raw = builder.inst_results(call)[0];
                            let produced = normalize_one(
                                &mut builder,
                                raw,
                                bits_of(dest.ty),
                                signed_of(dest.ty),
                            );
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::Sequence(_) => {
                            unreachable!("Sequence must be flattened before building")
                        }
                        ScalarInst::FiniteIsVariant {
                            dest,
                            src,
                            discriminant,
                        } if value_is_cell(src) => {
                            // Boxed finite: load the canonical tag word.
                            let callee = cell_libcall(module, builder.func, "mncs_slot_load32");
                            let zero = builder.ins().iconst(types::I64, 0);
                            let addr = builder.ins().iadd(values[src], zero);
                            let call = builder.ins().call(callee, &[addr]);
                            let tag = builder.inst_results(call)[0];
                            let disc = builder.ins().iconst(types::I64, i64::from(*discriminant));
                            let flag = builder.ins().icmp(IntCC::Equal, tag, disc);
                            let produced = builder.ins().uextend(types::I64, flag);
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
                        ScalarInst::ByteBitwise {
                            dest,
                            operator,
                            lhs,
                            rhs,
                        } => {
                            // Bytes ride zero-extended in the uniform i64
                            // cell, so bitwise ops are byte-exact.
                            let produced = match operator.as_str() {
                                "and" => builder.ins().band(values[lhs], values[rhs]),
                                "or" => builder.ins().bor(values[lhs], values[rhs]),
                                _ => builder.ins().bxor(values[lhs], values[rhs]),
                            };
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::ByteShift {
                            dest,
                            operator,
                            lhs,
                            rhs,
                        } => {
                            // Total semantics: count modulo 8, logical
                            // shifts, result masked into the byte domain.
                            let eight = builder.ins().iconst(types::I64, 8);
                            let count = builder.ins().urem(values[rhs], eight);
                            let shifted = if operator == "shl" {
                                builder.ins().ishl(values[lhs], count)
                            } else {
                                builder.ins().ushr(values[lhs], count)
                            };
                            let mask = builder.ins().iconst(types::I64, 255);
                            let produced = builder.ins().band(shifted, mask);
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::Convert {
                            dest,
                            from,
                            to,
                            src,
                            ..
                        } => {
                            let src_v = values[src];
                            // Float edges (Profile 0.12). Integer, byte, and
                            // boolean sources convert to binary64 exactly
                            // rounded; float sources guard finite, then
                            // range-guard with sliver-safe bounds (see the
                            // LLVM backend for the exactness argument), then
                            // convert. The shared renormalization below is
                            // the identity on guarded values.
                            let raw = if matches!(to, ScalarTy::Float) {
                                let converted = if signed_of(*from) {
                                    builder.ins().fcvt_from_sint(types::F64, src_v)
                                } else {
                                    builder.ins().fcvt_from_uint(types::F64, src_v)
                                };
                                jit_f64_to_bits(&mut builder, f64slot, converted)
                            } else if matches!(from, ScalarTy::Float) {
                                if !matches!(to, ScalarTy::Int(_) | ScalarTy::Byte) {
                                    // No float-to-bool cast exists in the
                                    // language: fail closed on a real edge
                                    // so both successors stay reachable.
                                    let always = builder.ins().iconst(types::I8, 1);
                                    let dead = builder.create_block();
                                    builder.ins().brif(
                                        always,
                                        fail,
                                        &[] as &[BlockArg],
                                        dead,
                                        &[] as &[BlockArg],
                                    );
                                    builder.switch_to_block(dead);
                                    builder.seal_block(dead);
                                }
                                let v = jit_bits_to_f64(&mut builder, f64slot, src_v);
                                let zero = builder.ins().f64const(Ieee64::with_bits(0));
                                let diff = builder.ins().fsub(v, v);
                                let bad = builder.ins().fcmp(FloatCC::NotEqual, diff, zero);
                                let cont = builder.create_block();
                                builder.ins().brif(
                                    bad,
                                    fail,
                                    &[] as &[BlockArg],
                                    cont,
                                    &[] as &[BlockArg],
                                );
                                builder.switch_to_block(cont);
                                builder.seal_block(cont);
                                let (lo_cc, lo_f, hi_f) = float_guard_bounds(*to);
                                let lo_c =
                                    builder.ins().f64const(Ieee64::with_bits(lo_f.to_bits()));
                                let hi_c =
                                    builder.ins().f64const(Ieee64::with_bits(hi_f.to_bits()));
                                let bad_lo = builder.ins().fcmp(lo_cc, v, lo_c);
                                let bad_hi =
                                    builder.ins().fcmp(FloatCC::GreaterThanOrEqual, v, hi_c);
                                let bad = builder.ins().bor(bad_lo, bad_hi);
                                let cont = builder.create_block();
                                builder.ins().brif(
                                    bad,
                                    fail,
                                    &[] as &[BlockArg],
                                    cont,
                                    &[] as &[BlockArg],
                                );
                                builder.switch_to_block(cont);
                                builder.seal_block(cont);
                                if signed_of(*to) {
                                    builder.ins().fcvt_to_sint(types::I64, v)
                                } else {
                                    builder.ins().fcvt_to_uint(types::I64, v)
                                }
                            } else {
                                src_v
                            };
                            // Renormalize the source cell into the target
                            // width/signedness: truncation drops high bits,
                            // widening keeps the value exactly.
                            let bits = bits_of(*to);
                            let signed = signed_of(*to);
                            let produced = if bits >= 64 && !signed {
                                raw
                            } else if signed {
                                let shift = builder.ins().iconst(types::I64, i64::from(64 - bits));
                                let widened = builder.ins().ishl(raw, shift);
                                builder.ins().sshr(widened, shift)
                            } else {
                                let mask = builder
                                    .ins()
                                    .iconst(types::I64, ((1i128 << bits) - 1) as i64);
                                builder.ins().band(raw, mask)
                            };
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::Select {
                            dest,
                            condition,
                            when_true,
                            when_false,
                        } => {
                            let flag =
                                builder
                                    .ins()
                                    .icmp_imm(IntCC::NotEqual, values[condition], 0);
                            let produced =
                                builder
                                    .ins()
                                    .select(flag, values[when_true], values[when_false]);
                            values.insert(dest.id.clone(), produced);
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
                            let idx = values[index];
                            if matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. })
                            {
                                let limit = builder.ins().iconst(types::I64, i64::from(*length));
                                let oob = builder.ins().icmp(
                                    IntCC::UnsignedGreaterThanOrEqual,
                                    idx,
                                    limit,
                                );
                                let cont = builder.create_block();
                                builder.ins().brif(
                                    oob,
                                    fail,
                                    &[] as &[BlockArg],
                                    cont,
                                    &[] as &[BlockArg],
                                );
                                builder.switch_to_block(cont);
                                builder.seal_block(cont);
                            }
                            let alloc = cell_libcall(module, builder.func, "mncs_cell_alloc");
                            let bytes = builder.ins().iconst(types::I64, i64::from(*length) * 8);
                            let call = builder.ins().call(alloc, &[bytes]);
                            let allocated = builder.inst_results(call)[0];
                            let (load_name, store_name) = match element_width {
                                crate::composite::SlotWidth::W32 => {
                                    ("mncs_slot_load32", "mncs_slot_store32")
                                }
                                crate::composite::SlotWidth::W64 => {
                                    ("mncs_slot_load64", "mncs_slot_store64")
                                }
                            };
                            let load = cell_libcall(module, builder.func, load_name);
                            let store = cell_libcall(module, builder.func, store_name);
                            for lane in 0..*length {
                                let offset = builder.ins().iconst(types::I64, i64::from(lane) * 8);
                                let source_addr = builder.ins().iadd(values[source], offset);
                                let loaded = builder.ins().call(load, &[source_addr]);
                                let lane_value = builder.inst_results(loaded)[0];
                                let dest_addr = builder.ins().iadd(allocated, offset);
                                builder.ins().call(store, &[dest_addr, lane_value]);
                            }
                            let scaled = builder.ins().ishl_imm(idx, 3);
                            let dest_addr = builder.ins().iadd(allocated, scaled);
                            builder.ins().call(store, &[dest_addr, values[element]]);
                            values.insert(dest.id.clone(), allocated);
                        }
                        ScalarInst::SequenceProject {
                            dest,
                            seq,
                            index,
                            bound,
                            evidence,
                            width,
                        } => {
                            let seq_v = values[seq];
                            let idx_v = values[index];
                            let checked = matches!(
                                evidence,
                                mncs_model::BoundsEvidence::RuntimeChecked { .. }
                            );
                            let (base, offset) = match bound {
                                mncs_model::SequenceBound::Exact(length) => {
                                    if checked {
                                        let limit =
                                            builder.ins().iconst(types::I64, i64::from(*length));
                                        let oob = builder.ins().icmp(
                                            IntCC::UnsignedGreaterThanOrEqual,
                                            idx_v,
                                            limit,
                                        );
                                        let cont = builder.create_block();
                                        builder.ins().brif(
                                            oob,
                                            fail,
                                            &[] as &[BlockArg],
                                            cont,
                                            &[] as &[BlockArg],
                                        );
                                        builder.switch_to_block(cont);
                                        builder.seal_block(cont);
                                    }
                                    (seq_v, builder.ins().ishl_imm(idx_v, 3))
                                }
                                mncs_model::SequenceBound::UpTo(_) => {
                                    if checked {
                                        let len = builder.ins().ushr_imm(seq_v, 32);
                                        let oob = builder.ins().icmp(
                                            IntCC::UnsignedGreaterThanOrEqual,
                                            idx_v,
                                            len,
                                        );
                                        let cont = builder.create_block();
                                        builder.ins().brif(
                                            oob,
                                            fail,
                                            &[] as &[BlockArg],
                                            cont,
                                            &[] as &[BlockArg],
                                        );
                                        builder.switch_to_block(cont);
                                        builder.seal_block(cont);
                                    }
                                    let mask = builder.ins().iconst(types::I64, 4_294_967_295);
                                    (
                                        builder.ins().band(seq_v, mask),
                                        builder.ins().ishl_imm(idx_v, 3),
                                    )
                                }
                                            mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => unreachable!("generic SequenceBound must be specialized before backend lowering"),
};
                            let addr = builder.ins().iadd(base, offset);
                            // Slot access goes through the shared libcalls:
                            // addresses are canonical arena offsets, never
                            // host pointers.
                            let name = match width {
                                crate::composite::SlotWidth::W32 => "mncs_slot_load32",
                                crate::composite::SlotWidth::W64 => "mncs_slot_load64",
                            };
                            let callee = cell_libcall(module, builder.func, name);
                            let call = builder.ins().call(callee, &[addr]);
                            values.insert(dest.id.clone(), builder.inst_results(call)[0]);
                        }
                        ScalarInst::SequenceLength { dest, bound, seq } => {
                            let produced = match bound {
                                mncs_model::SequenceBound::Exact(length) => {
                                    builder.ins().iconst(types::I64, i64::from(*length))
                                }
                                mncs_model::SequenceBound::UpTo(_) => {
                                    builder.ins().ushr_imm(values[seq], 32)
                                }
                                            mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => unreachable!("generic SequenceBound must be specialized before backend lowering"),
};
                            values.insert(dest.id.clone(), produced);
                        }
                        ScalarInst::ViewConstruct {
                            dest,
                            source_bound,
                            view_cap,
                            source,
                            start,
                            end,
                        } => {
                            let start_v = values[start];
                            let end_v = values[end];
                            let source_len = match source_bound {
                                mncs_model::SequenceBound::Exact(length) => {
                                    builder.ins().iconst(types::I64, i64::from(*length))
                                }
                                mncs_model::SequenceBound::UpTo(_) => {
                                    builder.ins().ushr_imm(values[source], 32)
                                }
                                            mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => unreachable!("generic SequenceBound must be specialized before backend lowering"),
};
                            let span = builder.ins().isub(end_v, start_v);
                            let gt = builder
                                .ins()
                                .icmp(IntCC::UnsignedGreaterThan, start_v, end_v);
                            let over =
                                builder
                                    .ins()
                                    .icmp(IntCC::UnsignedGreaterThan, end_v, source_len);
                            let cap = builder.ins().iconst(types::I64, i64::from(*view_cap));
                            let beyond = builder.ins().icmp(IntCC::UnsignedGreaterThan, span, cap);
                            let bad12 = builder.ins().bor(gt, over);
                            let bad = builder.ins().bor(bad12, beyond);
                            let cont = builder.create_block();
                            builder.ins().brif(
                                bad,
                                fail,
                                &[] as &[BlockArg],
                                cont,
                                &[] as &[BlockArg],
                            );
                            builder.switch_to_block(cont);
                            builder.seal_block(cont);
                            let base = match source_bound {
                                mncs_model::SequenceBound::Exact(_) => values[source],
                                mncs_model::SequenceBound::UpTo(_) => {
                                    let mask = builder.ins().iconst(types::I64, 4_294_967_295);
                                    builder.ins().band(values[source], mask)
                                }
                                            mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => unreachable!("generic SequenceBound must be specialized before backend lowering"),
};
                            let scaled = builder.ins().ishl_imm(start_v, 3);
                            let addr = builder.ins().iadd(base, scaled);
                            let lo = builder.ins().band_imm(addr, 4_294_967_295);
                            let hi = builder.ins().ishl_imm(span, 32);
                            let packed = builder.ins().bor(lo, hi);
                            values.insert(dest.id.clone(), packed);
                        }
                        ScalarInst::Call { dest, callee, args } => {
                            let callee_id = declared[callee];
                            let callee_ref = module.declare_func_in_func(callee_id, builder.func);
                            let mut call_args: Vec<Value> =
                                args.iter().map(|arg| values[arg]).collect();
                            call_args.push(status_ptr);
                            call_args.push(value_ptr);
                            // RFC 0047 §5: thread depth + 1 into the callee;
                            // the callee prologue enforces the ceiling.
                            let depth_next = builder.ins().iadd_imm(depth, 1);
                            call_args.push(depth_next);
                            builder.ins().call(callee_ref, &call_args);
                            let status =
                                builder
                                    .ins()
                                    .load(types::I32, MemFlags::trusted(), status_ptr, 0);
                            let failed = builder.ins().icmp_imm(IntCC::NotEqual, status, 0);
                            let cont = builder.create_block();
                            // A failed callee already stored its own status
                            // (1 for failure, 3 for fuel exhaustion) and a
                            // zero value through the shared out-pointers, so
                            // propagate with a bare return: branching to the
                            // shared `fail` block would overwrite an
                            // exhaustion 3 with a generic failure 1.
                            let propagate = builder.create_block();
                            builder.ins().brif(
                                failed,
                                propagate,
                                &[] as &[BlockArg],
                                cont,
                                &[] as &[BlockArg],
                            );
                            builder.switch_to_block(propagate);
                            builder.seal_block(propagate);
                            builder.ins().return_(&[]);
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
    Ok(declared)
}

/// Canonicalize i64 value slots to their declared integer width before
/// division: signed narrow types are sign-extended from their low bits,
/// unsigned narrow types are masked, and 64-bit values pass through. This
/// mirrors the reference executors' typed-slot behavior.
fn normalize_width(
    builder: &mut cranelift_frontend::FunctionBuilder<'_>,
    left: cranelift_codegen::ir::Value,
    right: cranelift_codegen::ir::Value,
    bits: u16,
    signed: bool,
) -> (cranelift_codegen::ir::Value, cranelift_codegen::ir::Value) {
    let left_n = normalize_one(builder, left, bits, signed);
    let right_n = normalize_one(builder, right, bits, signed);
    (left_n, right_n)
}

fn normalize_one(
    builder: &mut cranelift_frontend::FunctionBuilder<'_>,
    value: cranelift_codegen::ir::Value,
    bits: u16,
    signed: bool,
) -> cranelift_codegen::ir::Value {
    use cranelift_codegen::ir::types;
    use cranelift_codegen::ir::InstBuilder;
    if bits >= 64 {
        return value;
    }
    if signed {
        let shift = 64 - i64::from(bits);
        let widened = builder.ins().ishl_imm(value, shift);
        builder.ins().sshr_imm(widened, shift)
    } else {
        let mask = builder.ins().iconst(types::I64, (1_i64 << bits) - 1);
        builder.ins().band(value, mask)
    }
}

/// Process-global canonical arena backing the JIT cell libcalls. The image
/// is installed before each call and read back afterwards, mirroring the
/// call-file protocol used by the AOT and C realizations.
static JIT_ARENA: std::sync::Mutex<Vec<u8>> = std::sync::Mutex::new(Vec::new());

fn with_jit_arena<T>(operation: impl FnOnce(&mut Vec<u8>) -> T) -> T {
    let mut arena = JIT_ARENA.lock().expect("JIT arena mutex");
    operation(&mut arena)
}

/// Set by any JIT host slot access outside the installed arena image.
/// Loads/stores beyond the image cannot trap through the `u64` host-call
/// boundary, so without this flag an exhausted arena would silently compute
/// with dropped writes and zero reads instead of failing closed. The
/// execute path checks the flag after every JIT call and reports
/// `RuntimeFailure` while it is set; it is cleared on each arena install.
static JIT_OOB: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Set by `host_cell_alloc` when the request does not fit the canonical
/// arena cap, distinguishing bounded exhaustion (BudgetExhausted with the
/// attributed request/arena detail) from wild accesses. Cleared on each
/// arena install alongside [`JIT_OOB`].
static JIT_EXHAUSTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Attributed failure detail recorded by the JIT host calls (WEB-P-012):
/// the failed request size and arena state, so exhaustion reports bytes
/// instead of internals. Cleared on each arena install; drained once per
/// failed call by the execute path.
static JIT_DIAG: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn record_jit_diagnosis(message: String) {
    if let Ok(mut slot) = JIT_DIAG.lock() {
        *slot = Some(message);
    }
}

fn take_jit_diagnosis() -> Option<String> {
    JIT_DIAG.lock().ok().and_then(|mut slot| slot.take())
}

fn clear_jit_failure_state() {
    JIT_OOB.store(false, std::sync::atomic::Ordering::Relaxed);
    JIT_EXHAUSTED.store(false, std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut slot) = JIT_DIAG.lock() {
        *slot = None;
    }
}

extern "C" fn host_cell_alloc(bytes: u64) -> u64 {
    with_jit_arena(|arena| {
        let base = (arena.len() as u64 + 7) & !7u64;
        let Some(end) = base.checked_add(bytes) else {
            JIT_EXHAUSTED.store(true, std::sync::atomic::Ordering::Relaxed);
            record_jit_diagnosis(format!(
                "MNCS_RSRC_EXHAUSTED cranelift JIT canonical arena exhausted: requested {bytes} byte(s), {} of {} byte(s) used; bounded loops over large aggregate values allocate one fresh cell per functional update",
                arena.len(),
                crate::support::NATIVE_ARENA_BYTES
            ));
            return u64::MAX;
        };
        if end > crate::support::NATIVE_ARENA_BYTES {
            JIT_EXHAUSTED.store(true, std::sync::atomic::Ordering::Relaxed);
            record_jit_diagnosis(format!(
                "MNCS_RSRC_EXHAUSTED cranelift JIT canonical arena exhausted: requested {bytes} byte(s), {} of {} byte(s) used; bounded loops over large aggregate values allocate one fresh cell per functional update",
                arena.len(),
                crate::support::NATIVE_ARENA_BYTES
            ));
            return u64::MAX;
        }
        arena.resize(end as usize, 0);
        base
    })
}

fn slot_range(arena_len: usize, at: u64, width: usize) -> Option<std::ops::Range<usize>> {
    // Wraparound-safe bounds check: `at + width` overflows for sentinel
    // addresses (notably the u64::MAX allocation-failure sentinel), which
    // previously bypassed the guard and aborted the backend instead of
    // taking the deliberate out-of-bounds path.
    let at = usize::try_from(at).ok()?;
    let end = at.checked_add(width)?;
    if end <= arena_len {
        Some(at..end)
    } else {
        None
    }
}

/// Flag a wild slot access with its offset/width/image detail. A preceding
/// allocation-cap exhaustion keeps its better diagnosis: the first
/// attributed cause wins.
fn flag_slot_oob(arena_len: usize, at: u64, width: usize) {
    JIT_OOB.store(true, std::sync::atomic::Ordering::Relaxed);
    if !JIT_EXHAUSTED.load(std::sync::atomic::Ordering::Relaxed) {
        record_jit_diagnosis(format!(
            "cranelift JIT cell access outside the canonical arena: {width}-byte access at offset {at}, image length {arena_len} byte(s)"
        ));
    }
}

extern "C" fn host_slot_store32(at: u64, value: u64) {
    with_jit_arena(|arena| {
        if let Some(range) = slot_range(arena.len(), at, 4) {
            arena[range].copy_from_slice(&(value as u32).to_le_bytes());
        } else {
            flag_slot_oob(arena.len(), at, 4);
        }
    });
}

extern "C" fn host_slot_store64(at: u64, value: u64) {
    with_jit_arena(|arena| {
        if let Some(range) = slot_range(arena.len(), at, 8) {
            arena[range].copy_from_slice(&value.to_le_bytes());
        } else {
            flag_slot_oob(arena.len(), at, 8);
        }
    });
}

extern "C" fn host_slot_load32(at: u64) -> u64 {
    with_jit_arena(|arena| {
        if let Some(range) = slot_range(arena.len(), at, 4) {
            u32::from_le_bytes(arena[range].try_into().unwrap()) as u64
        } else {
            flag_slot_oob(arena.len(), at, 4);
            0
        }
    })
}

extern "C" fn host_slot_load64(at: u64) -> u64 {
    with_jit_arena(|arena| {
        if let Some(range) = slot_range(arena.len(), at, 8) {
            u64::from_le_bytes(arena[range].try_into().unwrap())
        } else {
            flag_slot_oob(arena.len(), at, 8);
            0
        }
    })
}

/// Whether the scalar module manipulates canonical cells.
fn module_uses_cells(module: &ScalarModule) -> bool {
    crate::support::scalar_module_uses_cells(module)
}

/// Marshal one request into JIT-callable 64-bit arguments plus the canonical
/// arena image that must be installed before the call.
fn jit_boundary_arguments(
    payload: &CraneliftPayload,
    input_contracts: &[mncs_model::BackendValueContract],
    output_contract: Option<&mncs_model::BackendValueContract>,
    composite_contracts: &std::collections::BTreeMap<String, mncs_model::BackendValueContract>,
    request: &ExecutionRequest,
) -> Result<(Vec<i64>, Option<Vec<u8>>), String> {
    let names = function_names(&payload.program, &payload.ssa);
    let lowered = lower_to_scalar(&payload.program, &payload.ssa, &names);
    let uses_cells = crate::support::scalar_module_uses_cells(&lowered);
    jit_boundary_arguments_for_request(
        uses_cells,
        input_contracts,
        output_contract,
        composite_contracts,
        request,
    )
}

fn jit_boundary_arguments_for_request(
    uses_cells: bool,
    input_contracts: &[mncs_model::BackendValueContract],
    output_contract: Option<&mncs_model::BackendValueContract>,
    composite_contracts: &std::collections::BTreeMap<String, mncs_model::BackendValueContract>,
    request: &ExecutionRequest,
) -> Result<(Vec<i64>, Option<Vec<u8>>), String> {
    let needs_call_file = uses_cells
        || request
            .arguments
            .iter()
            .any(crate::support::value_uses_call_file)
        || input_contracts
            .iter()
            .any(crate::support::contract_uses_call_file)
        || output_contract.is_some_and(crate::support::contract_uses_call_file);
    if !needs_call_file {
        // Historical scalar protocol: decimal argv strings. Words carry
        // the full u64 range (`18446744073709551615` for u64::MAX), which
        // does not parse as i64, so round-trip through i128 and keep the
        // low 64 bits (bit-exact for every value the ABI can carry,
        // including negative words for signed arguments). Float words
        // parse as binary64 and cross bit-carried, exactly like every
        // other float cell in the JIT.
        let raw_args = argv_from_request(request)?
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                if input_contracts
                    .get(index)
                    .is_some_and(crate::support::contract_is_float)
                {
                    arg.parse::<f64>()
                        .map(|value| value.to_bits() as i64)
                        .map_err(|error| error.to_string())
                } else {
                    arg.parse::<i128>()
                        .map(|value| value as i64)
                        .map_err(|error| error.to_string())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok((raw_args, None));
    }
    // Same entry validation as the call-file path (WEB-P-011): recursive
    // name-based resolution with expected-vs-received detail.
    for (index, value) in request.arguments.iter().enumerate() {
        if let Some(contract) = input_contracts.get(index) {
            crate::support::check_contract_value(
                contract,
                value,
                composite_contracts,
                &format!("argument {index}"),
            )?;
        }
    }
    let mut writer = crate::composite::ArenaWriter::new(composite_contracts.clone());
    let mut raw_args = Vec::new();
    for (index, value) in request.arguments.iter().enumerate() {
        match writer.encode_argument_with_contract(value, input_contracts.get(index))? {
            crate::composite::BoundaryValue::Bits(bits) => raw_args.push(bits as i64),
            crate::composite::BoundaryValue::Cell(root) => raw_args.push(root as i64),
        }
    }
    Ok((raw_args, Some(writer.into_image())))
}

/// Hex-encode the post-call JIT arena so composite results decode through
/// the same observation path as the C realizations.
fn read_jit_arena_hex(installed: bool) -> Option<String> {
    if !installed {
        return None;
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    with_jit_arena(|arena| {
        if arena.is_empty() {
            return None;
        }
        let mut out = String::with_capacity(arena.len() * 2);
        for byte in arena.iter() {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 15) as usize] as char);
        }
        Some(out)
    })
}

fn jit_scalar(
    scalar: &ScalarModule,
    function_name: &str,
    raw_args: &[i64],
    entry_depth: i64,
) -> Result<(ExecutionStatus, i128), String> {
    let mut session = JitSession::new(scalar)?;
    session.call(function_name, raw_args, entry_depth)
}

struct JitSession {
    module: cranelift_jit::JITModule,
    declared: BTreeMap<String, cranelift_module::FuncId>,
    host_trampolines: BTreeMap<String, cranelift_module::FuncId>,
}

impl JitSession {
    fn new(scalar: &ScalarModule) -> Result<Self, String> {
        use cranelift_jit::{JITBuilder, JITModule};

        let isa = host_isa()?;
        let mut jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        if module_uses_trig(scalar) {
            jit_builder.symbol("sin", mncs_sin_shim as *const u8);
            jit_builder.symbol("cos", mncs_cos_shim as *const u8);
        }
        if module_uses_cells(scalar) {
            jit_builder.symbol("mncs_cell_alloc", host_cell_alloc as *const u8);
            jit_builder.symbol("mncs_slot_store32", host_slot_store32 as *const u8);
            jit_builder.symbol("mncs_slot_store64", host_slot_store64 as *const u8);
            jit_builder.symbol("mncs_slot_load32", host_slot_load32 as *const u8);
            jit_builder.symbol("mncs_slot_load64", host_slot_load64 as *const u8);
        }
        let mut module = JITModule::new(jit_builder);
        let declared = declare_and_build(&mut module, scalar)?;
        let host_trampolines = declare_host_trampolines(&mut module, scalar, &declared)?;
        module
            .finalize_definitions()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            module,
            declared,
            host_trampolines,
        })
    }

    /// Call one finalized export. `entry_depth` seeds RFC 0047 call-depth
    /// fuel: zero for the full model cap, or `MAX - budget` when the
    /// execution request carries an explicit budget (see
    /// `support::depth_seed_for_request`), so budgeted fuel matches the
    /// reference interpreters activation-for-activation.
    fn call(
        &mut self,
        function_name: &str,
        raw_args: &[i64],
        entry_depth: i64,
    ) -> Result<(ExecutionStatus, i128), String> {
        // Module symbols follow the native-symbol rule (`fn main` is
        // defined as `mncs_main`); the request carries the MNCS name.
        let symbol = crate::support::c_symbol(function_name);
        let func_id = self
            .declared
            .get(&symbol)
            .copied()
            .ok_or_else(|| "requested Cranelift export is missing".to_owned())?;
        let ptr = self.module.get_finalized_function(func_id);
        let mut status: i32 = 2;
        let mut value: i64 = 0;
        unsafe {
            match raw_args.len() {
                // RFC 0047 §5: every direct host entry seeds call-depth fuel
                // via the trailing hidden parameter; the callee prologue
                // enforces MODEL_MAX_CALL_DEPTH from there.
                0 => {
                    let f: extern "C" fn(*mut i32, *mut i64, i64) = std::mem::transmute(ptr);
                    f(&mut status, &mut value, entry_depth);
                }
                1 => {
                    let f: extern "C" fn(i64, *mut i32, *mut i64, i64) = std::mem::transmute(ptr);
                    f(raw_args[0], &mut status, &mut value, entry_depth);
                }
                2 => {
                    let f: extern "C" fn(i64, i64, *mut i32, *mut i64, i64) =
                        std::mem::transmute(ptr);
                    f(
                        raw_args[0],
                        raw_args[1],
                        &mut status,
                        &mut value,
                        entry_depth,
                    );
                }
                3 => {
                    let f: extern "C" fn(i64, i64, i64, *mut i32, *mut i64, i64) =
                        std::mem::transmute(ptr);
                    f(
                        raw_args[0],
                        raw_args[1],
                        raw_args[2],
                        &mut status,
                        &mut value,
                        entry_depth,
                    );
                }
                4 => {
                    let f: extern "C" fn(i64, i64, i64, i64, *mut i32, *mut i64, i64) =
                        std::mem::transmute(ptr);
                    f(
                        raw_args[0],
                        raw_args[1],
                        raw_args[2],
                        raw_args[3],
                        &mut status,
                        &mut value,
                        entry_depth,
                    );
                }
                5 => {
                    let f: extern "C" fn(i64, i64, i64, i64, i64, *mut i32, *mut i64, i64) =
                        std::mem::transmute(ptr);
                    f(
                        raw_args[0],
                        raw_args[1],
                        raw_args[2],
                        raw_args[3],
                        raw_args[4],
                        &mut status,
                        &mut value,
                        entry_depth,
                    );
                }
                6 => {
                    let f: extern "C" fn(i64, i64, i64, i64, i64, i64, *mut i32, *mut i64, i64) =
                        std::mem::transmute(ptr);
                    f(
                        raw_args[0],
                        raw_args[1],
                        raw_args[2],
                        raw_args[3],
                        raw_args[4],
                        raw_args[5],
                        &mut status,
                        &mut value,
                        entry_depth,
                    );
                }
                _ => {
                    let trampoline_id =
                        self.host_trampolines.get(&symbol).copied().ok_or_else(|| {
                            "requested Cranelift host trampoline is missing".to_owned()
                        })?;
                    let trampoline = self.module.get_finalized_function(trampoline_id);
                    // The generated wrapper loads the complete argument vector from
                    // caller-owned memory and then invokes the original typed scalar
                    // export. This keeps the host ABI bounded to three pointer-sized
                    // values without imposing an accidental six-argument language limit.
                    // The seed rides a fourth word so wide exports honor the same
                    // request budget as direct calls.
                    let f: extern "C" fn(*const i64, *mut i32, *mut i64, i64) =
                        std::mem::transmute(trampoline);
                    f(raw_args.as_ptr(), &mut status, &mut value, entry_depth);
                }
            }
        }
        // Native status protocol: 0 is returned, 3 is call-depth fuel
        // exhaustion (RFC 0047 §5, matching the reference interpreters'
        // BudgetExhausted), and 1 (plus the 2 initializer, which the callee
        // always overwrites) is generic runtime failure. Exhaustion must
        // never collapse into RuntimeFailure: the codes are observably
        // distinct by backend agreement tests.
        match status {
            0 => Ok((ExecutionStatus::Returned, i128::from(value))),
            3 => Ok((ExecutionStatus::BudgetExhausted, 0)),
            _ => Ok((ExecutionStatus::RuntimeFailure, 0)),
        }
    }
}

/// Generate a host-call wrapper for every scalar export. The wrapper is a
/// backend-private ABI adapter: it receives one pointer to a contiguous array
/// of 64-bit argument cells plus the status/value out pointers, loads exactly
/// the number of cells required by the selected export, and calls the normal
/// scalar function. Language-level arity therefore remains independent from
/// the number of Rust function-pointer arguments we can spell in this source.
fn declare_host_trampolines<M>(
    module: &mut M,
    scalar: &ScalarModule,
    declared: &BTreeMap<String, cranelift_module::FuncId>,
) -> Result<BTreeMap<String, cranelift_module::FuncId>, String>
where
    M: cranelift_module::Module,
{
    use cranelift_codegen::ir::{types, AbiParam, InstBuilder, MemFlags};
    use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
    use cranelift_module::{Linkage, Module};

    let mut trampolines = BTreeMap::new();
    for (index, function) in scalar.functions.iter().enumerate() {
        let name = format!("__mncs_host_trampoline_{index}");
        let mut signature = module.make_signature();
        signature.params.push(AbiParam::new(types::I64)); // argument buffer
        signature.params.push(AbiParam::new(types::I64)); // status out pointer
        signature.params.push(AbiParam::new(types::I64)); // value out pointer
        signature.params.push(AbiParam::new(types::I64)); // entry depth seed
        let id = module
            .declare_function(&name, Linkage::Local, &signature)
            .map_err(|error| error.to_string())?;
        trampolines.insert(function.export_name.clone(), id);

        let mut context = module.make_context();
        context.func.signature.params.clear();
        context
            .func
            .signature
            .params
            .extend(signature.params.clone());
        let target =
            module.declare_func_in_func(declared[&function.export_name], &mut context.func);
        let mut function_context = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut context.func, &mut function_context);
            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            let parameters = builder.block_params(entry).to_vec();
            let argument_buffer = parameters[0];
            let status_ptr = parameters[1];
            let value_ptr = parameters[2];
            let entry_seed = parameters[3];
            let mut arguments = Vec::with_capacity(function.params.len() + 2);
            for argument_index in 0..function.params.len() {
                arguments.push(builder.ins().load(
                    types::I64,
                    MemFlags::trusted(),
                    argument_buffer,
                    (argument_index * std::mem::size_of::<i64>()) as i32,
                ));
            }
            arguments.push(status_ptr);
            arguments.push(value_ptr);
            // RFC 0047 §5: host entry seeds call-depth fuel from the
            // request budget (via the fourth trampoline word); every
            // same-module call adds one from there.
            arguments.push(entry_seed);
            builder.ins().call(target, &arguments);
            builder.ins().return_(&[]);
            builder.seal_all_blocks();
            builder.finalize();
        }
        module
            .define_function(id, &mut context)
            .map_err(|error| error.to_string())?;
        module.clear_context(&mut context);
    }
    Ok(trampolines)
}

/// A stateful trace session keeps the validated Cranelift payload and its
/// finalized host module alive across transitions. The logical state still
/// belongs to the language-owned stateful runner; this object only amortizes
/// backend setup and preserves the physical artifact boundary.
pub struct CraneliftStatefulSession<'a> {
    artifact: &'a mncs_model::BackendArtifact,
    scalar: ScalarModule,
    jit: JitSession,
}

pub fn prepare_stateful_session<'a>(
    artifact: &'a mncs_model::BackendArtifact,
) -> Result<CraneliftStatefulSession<'a>, String> {
    crate::support::backend_matches_identity(
        artifact,
        &cranelift_backend(),
        CRANELIFT_ARTIFACT_KIND,
    )
    .map_err(|reason| reason.to_owned())?;
    let bytes = artifact.bytes().map_err(|reason| reason.to_owned())?;
    mncs_model::record_counter("artifact_decode");
    let payload: CraneliftPayload = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid Cranelift payload: {error}"))?;
    let names = function_names(&payload.program, &payload.ssa);
    let scalar = lower_to_scalar(&payload.program, &payload.ssa, &names);
    if !scalar.unsupported.is_empty() || scalar.functions.is_empty() {
        return Err("Cranelift payload lowers to an unsupported or empty scalar module".to_owned());
    }
    if emit_clif(&scalar) != payload.clif {
        return Err(
            "Cranelift CLIF identity does not match the selected SSA in the payload".to_owned(),
        );
    }
    let jit = JitSession::new(&scalar)?;
    Ok(CraneliftStatefulSession {
        artifact,
        scalar,
        jit,
    })
}

impl CraneliftStatefulSession<'_> {
    pub fn execute(&mut self, request: &ExecutionRequest) -> BackendExecutionResult {
        let mut result = empty_execution(self.artifact, request);
        let Some(contract) = crate::support::entry_value_contract(
            &self.artifact.function_value_contracts,
            &request.target.module,
            &request.target.function,
        ) else {
            return execution_failure(
                result,
                ExecutionStatus::InvalidRequest,
                "Cranelift execution requires a language-owned function value contract",
            );
        };
        let (raw_args, arena_image) = match jit_boundary_arguments_for_request(
            crate::support::scalar_module_uses_cells(&self.scalar),
            &contract.inputs,
            contract.outputs.first(),
            &self.artifact.composite_value_contracts,
            request,
        ) {
            Ok(outcome) => outcome,
            Err(reason) => {
                return execution_failure(result, ExecutionStatus::InvalidRequest, reason);
            }
        };
        if let Some(image) = &arena_image {
            with_jit_arena(|arena| {
                arena.clear();
                arena.extend_from_slice(image);
            });
        }
        // RFC 0047 §5 uniform fuel (see `execute_cranelift`): the retained
        // module is fuel-agnostic, so each call seeds its own entry depth.
        let entry_depth = match crate::support::depth_seed_for_request(request) {
            Ok(seed) => seed as i64,
            Err(reason) => {
                return execution_failure(result, ExecutionStatus::InvalidRequest, reason);
            }
        };
        clear_jit_failure_state();
        // Trampolines are keyed by module-qualified native symbol
        // (ENG-PRESSURE-0017).
        let entry = crate::support::entry_native_symbol(
            &self.artifact.exports,
            &request.target.module,
            &request.target.function,
        );
        match self.jit.call(&entry, &raw_args, entry_depth) {
            Ok((status, value)) => {
                // Same attribution as the one-shot path (WEB-P-012):
                // exhaustion reports bytes with BudgetExhausted, wild
                // accesses fail closed, and no observation goes unattributed.
                if JIT_OOB.load(std::sync::atomic::Ordering::Relaxed) {
                    let diagnosis = take_jit_diagnosis();
                    if JIT_EXHAUSTED.load(std::sync::atomic::Ordering::Relaxed) {
                        return execution_failure(
                            result,
                            ExecutionStatus::BudgetExhausted,
                            diagnosis.unwrap_or_else(|| {
                                "MNCS_RSRC_EXHAUSTED cranelift JIT canonical arena exhausted"
                                    .to_owned()
                            }),
                        );
                    }
                    return execution_failure(
                        result,
                        ExecutionStatus::RuntimeFailure,
                        diagnosis.unwrap_or_else(|| {
                            "cranelift JIT cell access exceeded the arena image; failing closed"
                                .to_owned()
                        }),
                    );
                }
                if status != ExecutionStatus::Returned {
                    return execution_failure(
                        result,
                        status,
                        format!("cranelift JIT execution ended with status {status:?}"),
                    );
                }
                result.status = status;
                result.steps = 1;
                match crate::support::decode_native_observation(
                    contract.outputs.first(),
                    status,
                    value,
                    read_jit_arena_hex(arena_image.is_some()).as_deref(),
                    &self.artifact.composite_value_contracts,
                ) {
                    Ok(returned) => {
                        result.returned = returned;
                        result
                    }
                    Err(reason) => {
                        execution_failure(result, ExecutionStatus::InvalidRequest, reason)
                    }
                }
            }
            Err(reason) => execution_failure(result, ExecutionStatus::Unsupported, reason),
        }
    }
}

#[cfg(test)]
mod slot_range_tests {
    use super::slot_range;

    #[test]
    fn sentinel_and_wrapping_addresses_never_validate() {
        // The u64::MAX allocation-failure sentinel previously wrapped
        // `at + width` past the bounds check and aborted the backend.
        assert_eq!(slot_range(4_194_224, u64::MAX, 8), None);
        assert_eq!(slot_range(4_194_224, u64::MAX, 4), None);
        // Wrapping near-max address: old code computed end 0 and passed.
        assert_eq!(slot_range(4_194_224, u64::MAX - 7, 8), None);
        // Ordinary out-of-bounds stays out-of-bounds.
        assert_eq!(slot_range(100, 93, 8), None);
        assert_eq!(slot_range(100, 97, 4), None);
    }

    #[test]
    fn in_bounds_ranges_validate_exactly() {
        assert_eq!(slot_range(100, 92, 8), Some(92..100));
        assert_eq!(slot_range(100, 0, 4), Some(0..4));
        assert_eq!(slot_range(0, 0, 4), None);
    }
}

#[cfg(test)]
mod guarded_division_tests {
    use super::*;

    fn div_program() -> Program {
        Program::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/executable/checked-div.mncs.json"
        )))
        .unwrap()
    }

    /// Where host policy allows JIT executable memory (as on the Linux and
    /// Windows CI hosts), checked division must classify zero divisor and
    /// MIN / -1 as structured runtime failures inside this process instead
    /// of raising SIGFPE against the compiler. Hosts whose policy denies
    /// executable memory skip honestly; their AOT realization is covered by
    /// the backend-family integration tests.
    #[test]
    fn jit_guarded_division_never_traps_the_host_process() {
        let program = div_program();
        let ssa = program.lower_to_ssa().unwrap();
        let names = vec!["checked_div".to_owned(), "checked_mod".to_owned()];
        let scalar = lower_to_scalar(&program, &ssa, &names);
        assert!(scalar.unsupported.is_empty(), "{:?}", scalar.unsupported);
        let cases = [
            ("checked_div", vec![84_i128, 4], true, 21),
            ("checked_div", vec![-84, 4], true, -21),
            ("checked_div", vec![5, 0], false, 0),
            ("checked_div", vec![i64::MIN as i128, -1], false, 0),
            ("checked_mod", vec![10, 3], true, 1),
            ("checked_mod", vec![7, 0], false, 0),
        ];
        for (function, arguments, returns, expected) in cases {
            let raw_args: Vec<i64> = arguments.iter().map(|value| *value as i64).collect();
            let outcome = jit_scalar(&scalar, function, &raw_args, 0);
            match outcome {
                Err(reason)
                    if reason.contains("readable+executable")
                        || reason.contains("executable memory") =>
                {
                    // JIT denied by host policy; AOT conformance covers it.
                }
                Err(reason) => panic!("unexpected JIT failure: {reason}"),
                Ok((status, value)) => {
                    if returns {
                        assert_eq!(status, ExecutionStatus::Returned);
                        assert_eq!(value, expected);
                    } else {
                        assert_eq!(status, ExecutionStatus::RuntimeFailure);
                    }
                }
            }
        }
    }
}
