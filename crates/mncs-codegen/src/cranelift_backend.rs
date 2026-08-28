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

use crate::native::{argv_from_request, host_triple};
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
        let _ = index;
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
                "        {} = iconst.{} {value}",
                names.value(&dest.id),
                clif_ty(dest.ty)
            );
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
        ScalarInst::Convert { dest, to, src, .. } => {
            // The source cell is normalized to its own width/signedness, so
            // conversion is renormalization into the target parameters:
            // truncation drops high bits; widening keeps the value exactly.
            let d = names.value(&dest.id);
            let bits = bits_of(*to);
            let signed = signed_of(*to);
            if bits >= 64 && !signed {
                let _ = writeln!(out, "        {d} = {}", names.value(src));
            } else if signed {
                let shift = 64 - i64::from(bits);
                let _ = writeln!(
                    out,
                    "        {d} = ishl_imm.i64 {}, {shift}",
                    names.value(src)
                );
                let _ = writeln!(out, "        {d} = sshr_imm.i64 {d}, {shift}");
            } else {
                let mask = ((1_i128 << bits) - 1) as i64;
                let _ = writeln!(out, "        {d} = band {}, {mask}", names.value(src));
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
        ScalarTy::Cell | ScalarTy::View | ScalarTy::Mask(_) => "i64",
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
    match jit_execute_with_arguments(&payload, request.target.function.as_str(), &raw_args) {
        Ok((status, value)) => {
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
    let Some(contract) = artifact
        .function_value_contracts
        .get(&request.target.function)
    else {
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
    let driver = if crate::support::scalar_module_needs_arena_symbols(&scalar) {
        crate::support::process_driver_cell_runtime(
            &request.target.function,
            &contract.inputs,
            contract.outputs.first(),
        )
    } else {
        crate::support::process_driver(
            &request.target.function,
            &contract.inputs,
            contract.outputs.first(),
        )
    };
    let argv: Vec<String> = if arena_image.is_some() {
        Vec::new()
    } else {
        raw_args.iter().map(|value| value.to_string()).collect()
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
    jit_scalar(&scalar, function_name, raw_args)
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
    use cranelift_codegen::ir::condcodes::IntCC;
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
                for inst in flatten_scalar(&block.insts) {
                    match inst {
                        ScalarInst::Const { dest, value } => {
                            let produced = builder
                                .ins()
                                .iconst(types::I64, i64::try_from(*value).unwrap_or(0));
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
                        ScalarInst::Convert { dest, to, src, .. } => {
                            // Renormalize the source cell into the target
                            // width/signedness: truncation drops high bits,
                            // widening keeps the value exactly.
                            let bits = bits_of(*to);
                            let signed = signed_of(*to);
                            let src_v = values[src];
                            let produced = if bits >= 64 && !signed {
                                src_v
                            } else if signed {
                                let shift = builder.ins().iconst(types::I64, i64::from(64 - bits));
                                let widened = builder.ins().ishl(src_v, shift);
                                builder.ins().sshr(widened, shift)
                            } else {
                                let mask = builder
                                    .ins()
                                    .iconst(types::I64, ((1i128 << bits) - 1) as i64);
                                builder.ins().band(src_v, mask)
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

extern "C" fn host_cell_alloc(bytes: u64) -> u64 {
    with_jit_arena(|arena| {
        let base = (arena.len() as u64 + 7) & !7u64;
        let end = base + bytes;
        if end > 4 * 1024 * 1024 {
            return u64::MAX;
        }
        arena.resize(end as usize, 0);
        base
    })
}

extern "C" fn host_slot_store32(at: u64, value: u64) {
    with_jit_arena(|arena| {
        let at = at as usize;
        if at + 4 <= arena.len() {
            arena[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
        }
    });
}

extern "C" fn host_slot_store64(at: u64, value: u64) {
    with_jit_arena(|arena| {
        let at = at as usize;
        if at + 8 <= arena.len() {
            arena[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }
    });
}

extern "C" fn host_slot_load32(at: u64) -> u64 {
    with_jit_arena(|arena| {
        let at = at as usize;
        if at + 4 <= arena.len() {
            u32::from_le_bytes(arena[at..at + 4].try_into().unwrap()) as u64
        } else {
            0
        }
    })
}

extern "C" fn host_slot_load64(at: u64) -> u64 {
    with_jit_arena(|arena| {
        let at = at as usize;
        if at + 8 <= arena.len() {
            u64::from_le_bytes(arena[at..at + 8].try_into().unwrap())
        } else {
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
    let _ = lowered.functions.first();
    let uses_cells = crate::support::scalar_module_uses_cells(&lowered);
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
        // Historical scalar protocol: decimal argv strings.
        let raw_args = argv_from_request(request)?
            .iter()
            .map(|arg| arg.parse::<i64>().map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok((raw_args, None));
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
) -> Result<(ExecutionStatus, i128), String> {
    use cranelift_jit::{JITBuilder, JITModule};

    let isa = host_isa()?;
    let mut jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    if module_uses_cells(scalar) {
        jit_builder.symbol("mncs_cell_alloc", host_cell_alloc as *const u8);
        jit_builder.symbol("mncs_slot_store32", host_slot_store32 as *const u8);
        jit_builder.symbol("mncs_slot_store64", host_slot_store64 as *const u8);
        jit_builder.symbol("mncs_slot_load32", host_slot_load32 as *const u8);
        jit_builder.symbol("mncs_slot_load64", host_slot_load64 as *const u8);
    }
    let mut module = JITModule::new(jit_builder);
    let declared = declare_and_build(&mut module, scalar)?;
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
            4 => {
                let f: extern "C" fn(i64, i64, i64, i64, *mut i32, *mut i64) =
                    std::mem::transmute(ptr);
                f(
                    raw_args[0],
                    raw_args[1],
                    raw_args[2],
                    raw_args[3],
                    &mut status,
                    &mut value,
                );
            }
            5 => {
                let f: extern "C" fn(i64, i64, i64, i64, i64, *mut i32, *mut i64) =
                    std::mem::transmute(ptr);
                f(
                    raw_args[0],
                    raw_args[1],
                    raw_args[2],
                    raw_args[3],
                    raw_args[4],
                    &mut status,
                    &mut value,
                );
            }
            6 => {
                let f: extern "C" fn(i64, i64, i64, i64, i64, i64, *mut i32, *mut i64) =
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
                );
            }
            _ => {
                return Err(
                    "Cranelift host trampoline currently supports at most six scalar arguments"
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
            let outcome = jit_scalar(&scalar, function, &raw_args);
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
