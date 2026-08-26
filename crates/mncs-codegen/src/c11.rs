//! Deterministic C11 realization of selected SSA.
//!
//! Generated C preserves MNCS wrapping/checked integer semantics with
//! defined unsigned modular arithmetic. A C compiler is an external
//! realization tool, not part of the MNCS trust boundary.

use std::collections::BTreeMap;
use std::fmt::Write;

use mncs_model::{
    ArithmeticIntent, BackendCapabilityManifest, BackendConfiguration, BackendEvidence,
    BackendIdentity, BackendResult, CompilerArtifactRef, CompilerDiagnostic,
    CompilerDiagnosticKind, ExecutionRequest, ExecutionStatus, Program, SsaModule,
    TargetContractRef, TargetLoweringPlan, TransformationStatus, SSA_SCHEMA_VERSION,
};

use crate::composite::SlotWidth;
use crate::native::{
    argv_from_request, compile_and_run_with_call_file_full, probe_clang, probe_gcc,
};
use crate::scalar::{
    c_type, lower_to_scalar, ScalarFunction, ScalarInst, ScalarModule, ScalarTerm, ScalarTy,
};
use crate::support::{
    artifact_ref, empty_execution, execution_failure, function_names, function_value_contracts,
    unknown, validate_selected_ssa,
};
use crate::{BackendAdapter, BackendExecutionResult};

pub const C11_BACKEND_NAME: &str = "mncs-c11";
pub const C11_BACKEND_VERSION: &str = "0.1";
pub const C11_TARGET: &str = "mncs:target:c11-0.1";
pub const C11_FORMAT: &str = "text/x-c; mncs-c11-0.1";
pub const C11_ARTIFACT_KIND: &str = "c11_translation_unit";

pub struct C11Adapter;

pub fn c11_backend() -> BackendIdentity {
    BackendIdentity::new(C11_BACKEND_NAME, C11_BACKEND_VERSION)
}

pub fn c11_configuration() -> BackendConfiguration {
    BackendConfiguration {
        backend: c11_backend(),
        options: BTreeMap::from([
            ("standard".to_owned(), "c11".to_owned()),
            ("opt-level".to_owned(), "0".to_owned()),
        ]),
        target_features: ["scalar-ssa", "defined-integer", "no-memory"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        linker_toolchain: probe_clang()
            .or_else(probe_gcc)
            .map(|tool| tool.semantic_id()),
        assumptions: vec![
            "generated C is a realization, not MNCS semantics".to_owned(),
            "signed overflow is never performed; wrapping uses unsigned modular arithmetic"
                .to_owned(),
            "the C compiler is outside the MNCS semantic trust boundary".to_owned(),
            "C struct layout is not a language-owned record representation".to_owned(),
        ],
    }
}

pub fn c11_capabilities() -> BackendCapabilityManifest {
    BackendCapabilityManifest::new(
        c11_backend(),
        "mncs:selected-ssa-to-c11:0.1",
        [C11_TARGET.to_owned()].into_iter().collect(),
        [SSA_SCHEMA_VERSION.to_owned()].into_iter().collect(),
        ["data-layout", "abi", "integer", "trap", "standard"]
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
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        [
            "memory",
            "effects",
            "widening_integer",
            "undefined_behavior",
            "sequence_or_view_boundary_crossing",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        [C11_ARTIFACT_KIND.to_owned()].into_iter().collect(),
        ["bounded_execution_agreement".to_owned()]
            .into_iter()
            .collect(),
        ["external_c_compiler".to_owned()].into_iter().collect(),
        ["status_value_out_pointer_abi".to_owned()]
            .into_iter()
            .collect(),
        "checked/trapping overflow and declared failure set status=1; wrapping is unsigned modular",
        true,
    )
}

pub fn c11_target() -> TargetContractRef {
    TargetContractRef::new(
        C11_TARGET,
        BTreeMap::from([
            (
                "data-layout".to_owned(),
                "logical scalars only; C types stdint.h int32_t/int64_t".to_owned(),
            ),
            (
                "abi".to_owned(),
                "void fn(args..., int32_t* status, int64_t* value)".to_owned(),
            ),
            (
                "integer".to_owned(),
                "wrapping via unsigned; checked via int64_t range test".to_owned(),
            ),
            ("trap".to_owned(), "status=1, no abort, no UB".to_owned()),
            ("standard".to_owned(), "c11".to_owned()),
        ]),
        vec![mncs_model::SemanticId(
            "mncs:target-evidence:c11-0.1:declared-research-contract".to_owned(),
        )],
        vec!["C11 is a portability/bootstrap realization, not native MNCS".to_owned()],
    )
}

pub fn c11_plan(selected_ssa: CompilerArtifactRef) -> TargetLoweringPlan {
    TargetLoweringPlan::with_explicit_facts(
        selected_ssa,
        c11_target(),
        Some(c11_configuration()),
        vec![
            "no language-owned C struct layout".to_owned(),
            "bounded iteration is a switch/goto CFG, not while".to_owned(),
        ],
        vec!["private status/value out-pointer ABI".to_owned()],
        BTreeMap::from([
            (
                "wrapping".to_owned(),
                "unsigned modular add/sub/mul/and/or/xor".to_owned(),
            ),
            (
                "checked".to_owned(),
                "int64_t range test then status=1".to_owned(),
            ),
        ]),
        BTreeMap::from([
            ("overflow".to_owned(), "status=1".to_owned()),
            ("failure_terminator".to_owned(), "status=1".to_owned()),
        ]),
        vec!["optional clang or gcc -std=c11".to_owned()],
        Vec::new(),
        TransformationStatus::Pass,
    )
}

pub fn target_is_c11(target: &TargetContractRef) -> bool {
    target.candidate == C11_TARGET
        && ["data-layout", "abi", "integer", "trap", "standard"]
            .iter()
            .all(|fact| target.facts.contains_key(*fact))
        && !target.evidence.is_empty()
}

impl BackendAdapter for C11Adapter {
    fn capabilities(&self) -> BackendCapabilityManifest {
        c11_capabilities()
    }
    fn target(&self) -> TargetContractRef {
        c11_target()
    }
    fn configuration(&self) -> BackendConfiguration {
        c11_configuration()
    }
    fn plan(&self, selected_ssa: CompilerArtifactRef) -> TargetLoweringPlan {
        c11_plan(selected_ssa)
    }
    fn lower(
        &self,
        program: &Program,
        ssa: &SsaModule,
        selected_ssa: CompilerArtifactRef,
        plan: &TargetLoweringPlan,
    ) -> BackendResult {
        lower_c11(program, ssa, selected_ssa, plan)
    }
    fn execute(
        &self,
        artifact: &mncs_model::BackendArtifact,
        request: &ExecutionRequest,
    ) -> BackendExecutionResult {
        execute_c11(artifact, request)
    }
}

pub fn lower_c11(
    program: &Program,
    ssa: &SsaModule,
    selected_ssa: CompilerArtifactRef,
    plan: &TargetLoweringPlan,
) -> BackendResult {
    if let Err(result) = validate_selected_ssa(ssa, &selected_ssa, "CGC101") {
        return *result;
    }
    if !target_is_c11(&plan.target) {
        return unknown(vec![CompilerDiagnostic::new(
            "CGC201",
            CompilerDiagnosticKind::MissingTargetEvidence,
            "C11 lowering requires explicit data-layout, ABI, integer, trap, and standard facts",
        )]);
    }
    if plan.status != TransformationStatus::Pass {
        return unknown(vec![CompilerDiagnostic::new(
            "CGC202",
            CompilerDiagnosticKind::MissingTargetEvidence,
            "C11 will not realize a non-PASS target plan",
        )]);
    }
    let names = function_names(program, ssa);
    let scalar = lower_to_scalar(program, ssa, &names);
    if !scalar.unsupported.is_empty() || scalar.functions.is_empty() {
        let mut diagnostics = vec![CompilerDiagnostic::new(
            "CGC301",
            CompilerDiagnosticKind::UnavailableBackendCapability,
            "selected SSA is outside the C11 scalar envelope",
        )];
        for reason in scalar.unsupported {
            diagnostics.push(CompilerDiagnostic::new(
                "CGC302",
                CompilerDiagnosticKind::UnavailableBackendCapability,
                reason,
            ));
        }
        return unknown(diagnostics);
    }
    let source = emit_module(&scalar);
    let mut assumptions = plan.assumptions_introduced.clone();
    assumptions.extend(
        scalar
            .functions
            .iter()
            .flat_map(|function| function.promises.clone()),
    );
    let artifact = mncs_model::BackendArtifact::new_with_kind(
        c11_backend(),
        selected_ssa.clone(),
        plan.target.clone(),
        C11_ARTIFACT_KIND,
        C11_FORMAT,
        source.as_bytes(),
        scalar
            .functions
            .iter()
            .map(|function| function.export_name.clone())
            .collect(),
        assumptions.clone(),
        Vec::new(),
        vec![
            "external C compiler".to_owned(),
            "generated C retained as a typed backend artifact".to_owned(),
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
        c11_backend(),
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

fn emit_module(module: &ScalarModule) -> String {
    let mut out = String::from(
        "/* MNCS C11 realization 0.2. Not MNCS semantics. No C undefined behavior for integers. */\n#include <stdint.h>\n#include <stdbool.h>\n#include <string.h>\n\n",
    );
    if module_uses_cells(module) {
        out.push_str(
            "/* Canonical composite cell arena (MNCS cell layout v0.1): 8-byte\n   aligned cells, record field i at byte offset i*8, boxed finite tag\n   in slot 0 with payloads from byte 8. Slot access uses memcpy so no\n   alignment or aliasing assumptions are made. */\n#define MNCS_ARENA_BYTES (4u * 1024u * 1024u)\nunsigned char mncs_arena[MNCS_ARENA_BYTES];\nuint64_t mncs_bump = 0;\nuint64_t mncs_cell_alloc(uint64_t bytes) {\n  uint64_t base = (mncs_bump + 7u) & ~(uint64_t)7u;\n  mncs_bump = base + bytes;\n  return base;\n}\nvoid mncs_slot_store32(unsigned char *a, uint64_t at, uint32_t v) {\n  memcpy(a + at, &v, sizeof v);\n}\nvoid mncs_slot_store64(unsigned char *a, uint64_t at, uint64_t v) {\n  memcpy(a + at, &v, sizeof v);\n}\nuint32_t mncs_slot_load32(const unsigned char *a, uint64_t at) {\n  uint32_t v;\n  memcpy(&v, a + at, sizeof v);\n  return v;\n}\nuint64_t mncs_slot_load64(const unsigned char *a, uint64_t at) {\n  uint64_t v;\n  memcpy(&v, a + at, sizeof v);\n  return v;\n}\n\n",
        );
    }
    for function in &module.functions {
        emit_prototype(&mut out, function);
    }
    out.push('\n');
    for function in &module.functions {
        emit_function(&mut out, function);
        out.push('\n');
    }
    out
}

/// Whether any lowered function manipulates canonical cells.
fn module_uses_cells(module: &ScalarModule) -> bool {
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

fn emit_prototype(out: &mut String, function: &ScalarFunction) {
    let params = function
        .params
        .iter()
        .map(|param| c_type(param.ty).to_owned())
        .chain(["int32_t*".to_owned(), "int64_t*".to_owned()])
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(out, "void {}({params});", function.export_name);
}

fn emit_function(out: &mut String, function: &ScalarFunction) {
    let names = CNames::new(function);
    let mut params = function
        .params
        .iter()
        .map(|param| format!("{} {}", c_type(param.ty), names.value(&param.id)))
        .collect::<Vec<_>>();
    params.push("int32_t *mncs_status".to_owned());
    params.push("int64_t *mncs_value".to_owned());
    let _ = writeln!(
        out,
        "void {}({}) {{",
        function.export_name,
        params.join(", ")
    );
    let mut declared = function
        .params
        .iter()
        .map(|param| param.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    for block in &function.blocks {
        for param in &block.params {
            if declared.insert(param.id.clone()) {
                let _ = writeln!(out, "  {} {};", c_type(param.ty), names.value(&param.id));
            }
        }
        for inst in flatten_insts(&block.insts) {
            if let Some(dest) = inst_dest(inst) {
                if declared.insert(dest.id.clone()) {
                    let _ = writeln!(out, "  {} {};", c_type(dest.ty), names.value(&dest.id));
                }
            }
        }
    }
    out.push_str("  int32_t mncs_pc = 0;\n");
    out.push_str("  for (;;) {\n    switch (mncs_pc) {\n");
    for (index, block) in function.blocks.iter().enumerate() {
        let _ = writeln!(out, "    case {index}: {{");
        for inst in flatten_insts(&block.insts) {
            emit_inst(out, inst, &names);
        }
        match &block.term {
            ScalarTerm::Return { value } => {
                out.push_str("      *mncs_status = 0;\n");
                let _ = writeln!(out, "      *mncs_value = (int64_t){};", names.value(value));
                out.push_str("      return;\n");
            }
            ScalarTerm::Jump { target, args } => {
                emit_transfers(out, function, target, args, &names);
                let _ = writeln!(
                    out,
                    "      mncs_pc = {}; continue;",
                    names.block_index(target)
                );
            }
            ScalarTerm::Branch {
                cond,
                then_target,
                then_args,
                else_target,
                else_args,
            } => {
                let _ = writeln!(out, "      if ({}) {{", names.value(cond));
                emit_transfers(out, function, then_target, then_args, &names);
                let _ = writeln!(
                    out,
                    "        mncs_pc = {}; continue;",
                    names.block_index(then_target)
                );
                out.push_str("      } else {\n");
                emit_transfers(out, function, else_target, else_args, &names);
                let _ = writeln!(
                    out,
                    "        mncs_pc = {}; continue;",
                    names.block_index(else_target)
                );
                out.push_str("      }\n");
            }
            ScalarTerm::Fail => {
                out.push_str("      *mncs_status = 1;\n      *mncs_value = 0;\n      return;\n");
            }
        }
        out.push_str("    }\n");
    }
    out.push_str("    default:\n      *mncs_status = 1;\n      *mncs_value = 0;\n      return;\n    }\n  }\n}\n");
}

fn emit_transfers(
    out: &mut String,
    function: &ScalarFunction,
    target: &mncs_model::SemanticId,
    args: &[mncs_model::SemanticId],
    names: &CNames,
) {
    let Some(block) = function.blocks.iter().find(|block| block.id == *target) else {
        return;
    };
    for (param, arg) in block.params.iter().zip(args) {
        if param.id != *arg {
            let _ = writeln!(
                out,
                "      {} = {};",
                names.value(&param.id),
                names.value(arg)
            );
        }
    }
}

fn emit_inst(out: &mut String, inst: &ScalarInst, names: &CNames) {
    match inst {
        ScalarInst::Const { dest, value } => {
            let _ = writeln!(
                out,
                "      {} = ({}){};",
                names.value(&dest.id),
                c_type(dest.ty),
                value
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
            let bits = match dest.ty {
                ScalarTy::Int(integer) => integer.bits,
                _ => 32,
            };
            let signed = matches!(dest.ty, ScalarTy::Int(integer) if integer.signed);
            let mask = if bits >= 64 {
                "UINT64_MAX".to_owned()
            } else {
                format!("{:#x}ULL", (1u128 << bits) - 1)
            };
            if matches!(operator.as_str(), "and" | "or" | "xor") {
                let op = match operator.as_str() {
                    "and" => "&",
                    "or" => "|",
                    _ => "^",
                };
                let _ = writeln!(
                    out,
                    "      {dest_n} = ({})((uint64_t){lhs_n} {op} (uint64_t){rhs_n});",
                    c_type(dest.ty)
                );
                return;
            }
            // Shifts are total: counts are taken modulo the declared width
            // and every step avoids C undefined behavior explicitly.
            if matches!(operator.as_str(), "shl" | "shr") {
                let modulus = format!("{bits}u");
                let count = format!("((uint32_t){} % {modulus})", rhs_n);
                if operator == "shl" {
                    // Shift in the unsigned domain, then mask into the
                    // declared width; the destination cast restores sign.
                    let _ = writeln!(
                        out,
                        "      {{ uint64_t mncs_w = (uint64_t)(uint64_t){lhs_n}; mncs_w = (bits > 63) ? (mncs_w << {count}) : (((mncs_w << {count})) & {mask}); {dest_n} = ({})mncs_w; }}",
                        c_type(dest.ty),
                    );
                } else if signed {
                    // Arithmetic right shift without relying on C's
                    // implementation-defined `>>` for negatives:
                    // ~logical-shift-of-complement preserves the sign digit.
                    let _ = writeln!(
                        out,
                        "      {dest_n} = ({})(int64_t)(~(uint64_t)(~(int64_t){lhs_n} >> {count}));",
                        c_type(dest.ty)
                    );
                } else {
                    let _ = writeln!(
                        out,
                        "      {dest_n} = ({})((uint64_t){lhs_n} >> {count});",
                        c_type(dest.ty)
                    );
                }
                return;
            }
            let op = match operator.as_str() {
                "sub" => "-",
                "mul" => "*",
                _ => "+",
            };
            if matches!(operator.as_str(), "div" | "mod") {
                // Division by zero is undefined behavior in C, so every
                // intent guards it explicitly and fails exactly like the
                // reference executors; checked division also guards MIN/-1.
                // Unsigned operands divide in their own modular domain, not
                // through the signed C variable type.
                let slash = if operator == "div" { "/" } else { "%" };
                let overflow_guard = if operator == "div" && signed {
                    format!(
                        "if ({lhs_n} == INT{bits}_MIN && {rhs_n} == -1) {{ *mncs_status = 1; *mncs_value = 0; return; }} "
                    )
                } else {
                    String::new()
                };
                let unsigned_cast = |name: &str| {
                    if bits >= 64 {
                        format!("(uint64_t){name}")
                    } else {
                        format!("(uint32_t){name}")
                    }
                };
                let quotient = if signed {
                    format!("{lhs_n} {slash} {rhs_n}")
                } else {
                    format!("{} {slash} {}", unsigned_cast(lhs_n), unsigned_cast(rhs_n))
                };
                let _ = writeln!(
                    out,
                    "      if ({rhs_n} == 0) {{ *mncs_status = 1; *mncs_value = 0; return; }} {overflow_guard}{dest_n} = ({}){quotient};",
                    c_type(dest.ty)
                );
                return;
            }
            let bounds = if signed {
                let top = 1i128 << (bits - 1);
                (-top, top - 1)
            } else {
                (0i128, (1i128 << bits) - 1)
            };
            // Decimal literals are emitted with explicit suffixes (and the
            // signed minimum is built by subtraction) because a bare
            // `-9223372036854775808` would first materialize an unsigned
            // constant that does not fit `long long`.
            let literal = |value: i128| -> String {
                if signed {
                    format!("{value}LL")
                } else {
                    format!("{value}ULL")
                }
            };
            let max_text = literal(bounds.1);
            let min_text = if signed && bounds.0 == -(1i128 << 63) {
                "(-9223372036854775807LL - 1)".to_owned()
            } else {
                literal(bounds.0)
            };
            if matches!(
                intent,
                ArithmeticIntent::Checked | ArithmeticIntent::Trapping
            ) && !promise.decision.permitted
            {
                // The wide intermediate is 128-bit so a 64-bit overflow is
                // still exactly detectable; narrower operands cannot overflow
                // an int128/uint128 intermediate at all, but the guard stays
                // uniform so failure semantics are identical at every width.
                let wide_ty = if signed {
                    "__int128"
                } else {
                    "unsigned __int128"
                };
                let _ = writeln!(
                    out,
                    "      {{ {wide_ty} mncs_wide = ({wide_ty}){lhs_n} {op} ({wide_ty}){rhs_n}; if (mncs_wide > {max} || mncs_wide < {min}) {{ *mncs_status = 1; *mncs_value = 0; return; }} {dest_n} = ({})mncs_wide; }}",
                    c_type(dest.ty),
                    max = max_text,
                    min = min_text,
                );
            } else if matches!(intent, ArithmeticIntent::Saturating) {
                // Total by definition: compute in the wide domain, then clamp
                // into the declared representable range.
                let wide_ty = if signed {
                    "__int128"
                } else {
                    "unsigned __int128"
                };
                let _ = writeln!(
                    out,
                    "      {{ {wide_ty} mncs_wide = ({wide_ty}){lhs_n} {op} ({wide_ty}){rhs_n}; if (mncs_wide > {max}) {{ mncs_wide = {max}; }} if (mncs_wide < {min}) {{ mncs_wide = {min}; }} {dest_n} = ({})mncs_wide; }}",
                    c_type(dest.ty),
                    max = max_text,
                    min = min_text,
                );
            } else {
                let _ = writeln!(
                    out,
                    "      {dest_n} = ({})((uint64_t)((uint64_t){lhs_n} {op} (uint64_t){rhs_n}) & {mask});",
                    c_type(dest.ty)
                );
            }
        }
        ScalarInst::Boolean {
            dest,
            operator,
            lhs,
            rhs,
        } => {
            // Strict MNCS booleans over normalized 0/1 int32 slots.
            let op = match operator.as_str() {
                "and" => "&&",
                _ => "||",
            };
            let _ = writeln!(
                out,
                "      {} = ({})({} {op} {});",
                names.value(&dest.id),
                c_type(dest.ty),
                names.value(lhs),
                names.value(rhs)
            );
        }
        ScalarInst::Compare {
            dest,
            predicate,
            lhs,
            rhs,
            ..
        } => {
            let pred = match predicate.as_str() {
                "eq" => "==",
                "ne" => "!=",
                "lt" => "<",
                "le" => "<=",
                "gt" => ">",
                _ => ">=",
            };
            let _ = writeln!(
                out,
                "      {} = ({})({} {pred} {});",
                names.value(&dest.id),
                c_type(dest.ty),
                names.value(lhs),
                names.value(rhs)
            );
        }
        ScalarInst::FiniteConstruct { dest, discriminant } => {
            let _ = writeln!(out, "      {} = {};", names.value(&dest.id), discriminant);
        }
        ScalarInst::CellAlloc { dest, bytes } => {
            let _ = writeln!(
                out,
                "      {} = mncs_cell_alloc({bytes});",
                names.value(&dest.id)
            );
        }
        ScalarInst::CellStoreDiscriminant { cell, discriminant } => {
            let _ = writeln!(
                out,
                "      mncs_slot_store32(mncs_arena, {}, (uint32_t){});",
                names.value(cell),
                discriminant
            );
        }
        ScalarInst::CellStore {
            cell,
            byte_offset,
            width,
            value,
        } => match width {
            SlotWidth::W32 => {
                let _ = writeln!(
                    out,
                    "      mncs_slot_store32(mncs_arena, {} + {byte_offset}, (uint32_t){});",
                    names.value(cell),
                    names.value(value)
                );
            }
            SlotWidth::W64 => {
                let _ = writeln!(
                    out,
                    "      mncs_slot_store64(mncs_arena, {} + {byte_offset}, (uint64_t){});",
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
            SlotWidth::W32 => {
                let _ = writeln!(
                    out,
                    "      {} = ({})mncs_slot_load32(mncs_arena, {} + {byte_offset});",
                    names.value(&dest.id),
                    c_type(dest.ty),
                    names.value(cell)
                );
            }
            SlotWidth::W64 => {
                let _ = writeln!(
                    out,
                    "      {} = ({})mncs_slot_load64(mncs_arena, {} + {byte_offset});",
                    names.value(&dest.id),
                    c_type(dest.ty),
                    names.value(cell)
                );
            }
        },
        ScalarInst::ByteBitwise {
            dest,
            operator,
            lhs,
            rhs,
        } => {
            let op = match operator.as_str() {
                "and" => "&",
                "or" => "|",
                _ => "^",
            };
            let _ = writeln!(
                out,
                "      {} = ({})((uint8_t)((uint8_t){} {op} (uint8_t){}));",
                names.value(&dest.id),
                c_type(dest.ty),
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
            // Total semantics: the count is taken modulo 8; both shifts are
            // logical because bytes are unsigned by definition.
            let expr = match operator.as_str() {
                "shl" => format!(
                    "(uint8_t)((uint32_t){} << ((uint32_t){} % 8u))",
                    names.value(lhs),
                    names.value(rhs)
                ),
                _ => format!(
                    "(uint8_t)((uint32_t){} >> ((uint32_t){} % 8u))",
                    names.value(lhs),
                    names.value(rhs)
                ),
            };
            let _ = writeln!(
                out,
                "      {} = ({})({expr});",
                names.value(&dest.id),
                c_type(dest.ty)
            );
        }
        ScalarInst::Convert { dest, src, .. } => {
            // C casts implement exactly the declared total conversion:
            // narrowing truncates high bits, widening extends by the source
            // signedness (`byte` is unsigned).
            let _ = writeln!(
                out,
                "      {} = ({}){};",
                names.value(&dest.id),
                c_type(dest.ty),
                names.value(src)
            );
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
            let dest_n = names.value(&dest.id);
            match bound {
                mncs_model::SequenceBound::Exact(length) => {
                    if checked {
                        let _ = writeln!(
                            out,
                            "      if ((uint64_t){index} >= {length}ULL) {{ *mncs_status = 1; *mncs_value = 0; return; }}",
                            index = names.value(index),
                        );
                    }
                    let address = format!(
                        "{} + (uint64_t){} * 8u",
                        names.value(seq),
                        names.value(index)
                    );
                    emit_slot_load(out, dest_n, dest.ty, &address, *width);
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    let view = names.value(seq);
                    if checked {
                        let _ = writeln!(
                            out,
                            "      if ((uint64_t){index} >= ({view} >> 32)) {{ *mncs_status = 1; *mncs_value = 0; return; }}",
                            index = names.value(index),
                        );
                    }
                    let address = format!(
                        "(uint64_t)(uint32_t)({view}) + (uint64_t){} * 8u",
                        names.value(index)
                    );
                    emit_slot_load(out, dest_n, dest.ty, &address, *width);
                }
            }
        }
        ScalarInst::SequenceLength { dest, bound, seq } => {
            let dest_n = names.value(&dest.id);
            match bound {
                mncs_model::SequenceBound::Exact(length) => {
                    let _ = writeln!(out, "      {dest_n} = ({}){length}ULL;", c_type(dest.ty));
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    let _ = writeln!(
                        out,
                        "      {dest_n} = ({})(({} >> 32));",
                        c_type(dest.ty),
                        names.value(seq)
                    );
                }
            }
        }
        ScalarInst::ViewConstruct {
            dest,
            source_bound,
            view_cap,
            source,
            start,
            end,
        } => {
            let dest_n = names.value(&dest.id);
            let start_n = names.value(start);
            let end_n = names.value(end);
            // The source length is static for exact sequences and observed
            // from the packed descriptor for views.
            let source_len = match source_bound {
                mncs_model::SequenceBound::Exact(length) => format!("{length}ULL"),
                mncs_model::SequenceBound::UpTo(_) => {
                    format!("({} >> 32)", names.value(source))
                }
            };
            let base = match source_bound {
                mncs_model::SequenceBound::Exact(_) => names.value(source).to_owned(),
                mncs_model::SequenceBound::UpTo(_) => {
                    format!("(uint64_t)(uint32_t)({})", names.value(source))
                }
            };
            let _ = writeln!(
                out,
                "      if ({start_n} > {end_n} || {end_n} > {source_len} || {end_n} - {start_n} > {view_cap}ULL) {{ *mncs_status = 1; *mncs_value = 0; return; }}",
            );
            let _ = writeln!(
                out,
                "      {dest_n} = (uint64_t)(uint32_t)({base} + {start_n} * 8u) | (((uint64_t)({end_n} - {start_n})) << 32);"
            );
        }
        ScalarInst::FiniteIsVariant {
            dest,
            src,
            discriminant,
        } if names.ty(src).is_cell() => {
            // Boxed finite: compare the canonical cell's tag word.
            let _ = writeln!(
                out,
                "      {} = ((int32_t)mncs_slot_load32(mncs_arena, {}) == (int32_t){});",
                names.value(&dest.id),
                names.value(src),
                discriminant
            );
        }
        ScalarInst::FiniteIsVariant {
            dest,
            src,
            discriminant,
        } => {
            let _ = writeln!(
                out,
                "      {} = ((int32_t){} == (int32_t){});",
                names.value(&dest.id),
                names.value(src),
                discriminant
            );
        }
        ScalarInst::Sequence(insts) => {
            for nested in insts {
                emit_inst(out, nested, names);
            }
        }
        ScalarInst::Call { dest, callee, args } => {
            let list = args
                .iter()
                .map(|arg| names.value(arg).to_owned())
                .chain(["mncs_status".to_owned(), "mncs_value".to_owned()])
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "      {callee}({list});");
            out.push_str("      if (*mncs_status != 0) { return; }\n");
            let _ = writeln!(
                out,
                "      {} = ({})*mncs_value;",
                names.value(&dest.id),
                c_type(dest.ty)
            );
        }
    }
}

fn inst_dest(inst: &ScalarInst) -> Option<&crate::scalar::ScalarValue> {
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
        | ScalarInst::SequenceProject { dest, .. }
        | ScalarInst::SequenceLength { dest, .. }
        | ScalarInst::ViewConstruct { dest, .. }
        | ScalarInst::Call { dest, .. } => Some(dest),
        // Cell stores produce no value; sequences declare through members.
        ScalarInst::CellStoreDiscriminant { .. } | ScalarInst::CellStore { .. } => None,
        ScalarInst::Sequence(_) => None,
    }
}

/// Emit one slot load at a computed byte address inside the canonical arena.
fn emit_slot_load(
    out: &mut String,
    dest_name: &str,
    dest_ty: crate::scalar::ScalarTy,
    address: &str,
    width: SlotWidth,
) {
    match width {
        SlotWidth::W32 => {
            let _ = writeln!(
                out,
                "      {dest_name} = ({})mncs_slot_load32(mncs_arena, {address});",
                c_type(dest_ty)
            );
        }
        SlotWidth::W64 => {
            let _ = writeln!(
                out,
                "      {dest_name} = ({})mncs_slot_load64(mncs_arena, {address});",
                c_type(dest_ty)
            );
        }
    }
}

/// Flatten sequence groups so declaration and emission walk every concrete
/// instruction exactly once, in order.
fn flatten_insts(insts: &[ScalarInst]) -> Vec<&ScalarInst> {
    let mut flat = Vec::new();
    for inst in insts {
        match inst {
            ScalarInst::Sequence(nested) => flat.extend(flatten_insts(nested)),
            other => flat.push(other),
        }
    }
    flat
}

struct CNames {
    values: BTreeMap<mncs_model::SemanticId, String>,
    blocks: BTreeMap<mncs_model::SemanticId, usize>,
    types: BTreeMap<mncs_model::SemanticId, ScalarTy>,
}

impl CNames {
    fn ty(&self, id: &mncs_model::SemanticId) -> ScalarTy {
        self.types.get(id).copied().unwrap_or(ScalarTy::Finite)
    }
}

impl CNames {
    fn new(function: &ScalarFunction) -> Self {
        let mut values = BTreeMap::new();
        let mut types: BTreeMap<mncs_model::SemanticId, ScalarTy> = BTreeMap::new();
        let mut next = 0u32;
        let mut push = |value: &crate::scalar::ScalarValue| {
            values.entry(value.id.clone()).or_insert_with(|| {
                let name = format!("v{next}");
                next += 1;
                name
            });
            types.insert(value.id.clone(), value.ty);
        };
        for param in &function.params {
            push(param);
        }
        for block in &function.blocks {
            for param in &block.params {
                push(param);
            }
            for inst in flatten_insts(&block.insts) {
                if let Some(dest) = inst_dest(inst) {
                    push(dest);
                }
            }
        }
        let blocks = function
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (block.id.clone(), index))
            .collect();
        Self {
            values,
            blocks,
            types,
        }
    }

    fn value(&self, id: &mncs_model::SemanticId) -> &str {
        self.values.get(id).map_or("v_missing", String::as_str)
    }

    fn block_index(&self, id: &mncs_model::SemanticId) -> usize {
        self.blocks.get(id).copied().unwrap_or(0)
    }
}

pub fn execute_c11(
    artifact: &mncs_model::BackendArtifact,
    request: &ExecutionRequest,
) -> BackendExecutionResult {
    let mut result = empty_execution(artifact, request);
    if let Err(reason) =
        crate::support::backend_matches_identity(artifact, &c11_backend(), C11_ARTIFACT_KIND)
    {
        return execution_failure(result, ExecutionStatus::InvalidRequest, reason);
    }
    let Some(compiler) = probe_clang().or_else(probe_gcc) else {
        return execution_failure(
            result,
            ExecutionStatus::Unsupported,
            "unavailable toolchain: neither clang nor gcc is present for C11 execution",
        );
    };
    let source = match artifact.bytes() {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(reason) => return execution_failure(result, ExecutionStatus::InvalidRequest, reason),
    };
    let Some(contract) = artifact
        .function_value_contracts
        .get(&request.target.function)
    else {
        return execution_failure(
            result,
            ExecutionStatus::InvalidRequest,
            "C11 execution requires a language-owned function value contract",
        );
    };
    // Composite arguments and results cross through the canonical call
    // file; pure scalar calls keep the historical argv-only protocol.
    let driver = c_driver(
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
    let argv = match &call_blob {
        Some(_) => Vec::new(),
        None => match argv_from_request(request) {
            Ok(argv) => argv,
            Err(reason) => return execution_failure(result, ExecutionStatus::Unsupported, reason),
        },
    };
    match compile_and_run_with_call_file_full(
        &[("module.c", source.as_str()), ("driver.c", driver.as_str())],
        &compiler,
        &["-std=c11", "-O0", "-Wall"],
        &argv,
        call_path.as_deref(),
    ) {
        Ok((run, _)) => {
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

pub(crate) use crate::support::process_driver as c_driver;
