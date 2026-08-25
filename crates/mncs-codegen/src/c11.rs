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
    CompilerDiagnosticKind, ExecutionRequest, ExecutionStatus, ExecutionValue, Program, SsaModule,
    TargetContractRef, TargetLoweringPlan, TransformationStatus, SSA_SCHEMA_VERSION,
};

use crate::native::{
    argv_from_request, compile_and_run, decode_native_value, probe_clang, probe_gcc,
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
            "trapping_integer",
            "explicit_failure",
            "semantic_bounded_iteration",
            "finite_values",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        [
            "record_values",
            "memory",
            "effects",
            "saturating_integer",
            "widening_integer",
            "undefined_behavior",
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
        "/* MNCS C11 realization 0.1. Not MNCS semantics. No C undefined behavior for integers. */\n#include <stdint.h>\n#include <stdbool.h>\n\n",
    );
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
        for inst in &block.insts {
            let dest = inst_dest(inst);
            if declared.insert(dest.id.clone()) {
                let _ = writeln!(out, "  {} {};", c_type(dest.ty), names.value(&dest.id));
            }
        }
    }
    out.push_str("  int32_t mncs_pc = 0;\n");
    out.push_str("  for (;;) {\n    switch (mncs_pc) {\n");
    for (index, block) in function.blocks.iter().enumerate() {
        let _ = writeln!(out, "    case {index}: {{");
        for inst in &block.insts {
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
            let op = match operator.as_str() {
                "sub" => "-",
                "mul" => "*",
                _ => "+",
            };
            if matches!(operator.as_str(), "div" | "mod") {
                // Division by zero is undefined behavior in C, so every
                // intent guards it explicitly and fails exactly like the
                // reference executors; checked division also guards MIN/-1.
                let slash = if operator == "div" { "/" } else { "%" };
                let overflow_guard = if operator == "div" && signed {
                    format!(
                        "if ({lhs_n} == INT{bits}_MIN && {rhs_n} == -1) {{ *mncs_status = 1; *mncs_value = 0; return; }} "
                    )
                } else {
                    String::new()
                };
                let _ = writeln!(
                    out,
                    "      if ({rhs_n} == 0) {{ *mncs_status = 1; *mncs_value = 0; return; }} {overflow_guard}{dest_n} = ({}){lhs_n} {slash} {rhs_n};",
                    c_type(dest.ty)
                );
                return;
            }
            if matches!(
                intent,
                ArithmeticIntent::Checked | ArithmeticIntent::Trapping
            ) && !promise.decision.permitted
            {
                let (min, max) = if signed {
                    let top = 1i128 << (bits - 1);
                    ((-top).to_string(), (top - 1).to_string())
                } else {
                    ("0".to_owned(), ((1i128 << bits) - 1).to_string())
                };
                let _ = writeln!(
                    out,
                    "      {{ int64_t mncs_wide = (int64_t){lhs_n} {op} (int64_t){rhs_n}; if (mncs_wide > {max} || mncs_wide < {min}) {{ *mncs_status = 1; *mncs_value = 0; return; }} {dest_n} = ({})mncs_wide; }}",
                    c_type(dest.ty)
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
        ScalarInst::FiniteIsVariant {
            dest,
            src,
            discriminant,
        } => {
            let _ = writeln!(
                out,
                "      {} = ({} == {});",
                names.value(&dest.id),
                names.value(src),
                discriminant
            );
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

fn inst_dest(inst: &ScalarInst) -> &crate::scalar::ScalarValue {
    match inst {
        ScalarInst::Const { dest, .. }
        | ScalarInst::Integer { dest, .. }
        | ScalarInst::Boolean { dest, .. }
        | ScalarInst::Compare { dest, .. }
        | ScalarInst::FiniteConstruct { dest, .. }
        | ScalarInst::FiniteIsVariant { dest, .. }
        | ScalarInst::Call { dest, .. } => dest,
    }
}

struct CNames {
    values: BTreeMap<mncs_model::SemanticId, String>,
    blocks: BTreeMap<mncs_model::SemanticId, usize>,
}

impl CNames {
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
                push(&inst_dest(inst).id);
            }
        }
        let blocks = function
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (block.id.clone(), index))
            .collect();
        Self { values, blocks }
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
    // Composite arguments cannot cross this process-argument realization
    // boundary; they fail closed before any code is compiled.
    for value in &request.arguments {
        if !matches!(
            value,
            ExecutionValue::Boolean { .. } | ExecutionValue::Integer { .. }
        ) {
            return execution_failure(
                result,
                ExecutionStatus::Unsupported,
                "composite arguments are outside the current C11 process-boundary envelope; records and payload sums realize through WASM and the research bytecode interpreter",
            );
        }
    }
    let driver = c_driver(&request.target.function, &contract.inputs);
    let argv = match argv_from_request(request) {
        Ok(argv) => argv,
        Err(reason) => return execution_failure(result, ExecutionStatus::Unsupported, &reason),
    };
    match compile_and_run(
        &[("module.c", source.as_str()), ("driver.c", driver.as_str())],
        &compiler,
        &["-std=c11", "-O0", "-Wall"],
        &argv,
    ) {
        Ok((value, status, _)) => {
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
        Err(error) => execution_failure(result, error.status(), error.reason()),
    }
}

pub(crate) use crate::support::process_driver as c_driver;
