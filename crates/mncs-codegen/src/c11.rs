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
    NativeExecutable, ToolchainIdentity,
};
use crate::scalar::{
    c_type, lower_to_scalar, ScalarFunction, ScalarInst, ScalarModule, ScalarTerm, ScalarTy,
};
use crate::support::{
    artifact_ref, empty_execution, execution_failure, function_names, function_value_contracts,
    unknown, validate_realizable_ssa, validate_selected_ssa,
};
use crate::{BackendAdapter, BackendExecutionResult};

pub const C11_BACKEND_NAME: &str = "mncs-c11";
pub const C11_BACKEND_VERSION: &str = "0.1";
pub const C11_TARGET: &str = "mncs:target:c11-0.1";
pub const C11_FORMAT: &str = "text/x-c; mncs-c11-0.1";
pub const C11_ARTIFACT_KIND: &str = "c11_translation_unit";

pub struct C11Adapter;

/// Stateful C11 preparation retains the generated module, selected compiler,
/// and one compiled executable per language-owned function contract. Calls
/// still run in isolated child processes through the canonical call-file
/// protocol; only preparation and compilation are reused.
pub struct C11StatefulSession<'a> {
    artifact: &'a mncs_model::BackendArtifact,
    compiler: ToolchainIdentity,
    source: String,
    executables: BTreeMap<String, NativeExecutable>,
}

pub fn prepare_stateful_session<'a>(
    artifact: &'a mncs_model::BackendArtifact,
) -> Result<C11StatefulSession<'a>, String> {
    crate::support::backend_matches_identity(artifact, &c11_backend(), C11_ARTIFACT_KIND)?;
    let compiler = probe_clang()
        .or_else(probe_gcc)
        .ok_or_else(|| "unavailable toolchain: neither clang nor gcc is present".to_owned())?;
    let bytes = artifact.bytes()?;
    mncs_model::record_counter("artifact_decode");
    Ok(C11StatefulSession {
        artifact,
        compiler,
        source: String::from_utf8_lossy(&bytes).into_owned(),
        executables: BTreeMap::new(),
    })
}

impl C11StatefulSession<'_> {
    pub fn execute(&mut self, request: &ExecutionRequest) -> BackendExecutionResult {
        let mut result = empty_execution(self.artifact, request);
        // P1-013: a request naming a compiled generic instantiation
        // resolves to the emitted specialization entry; requests without
        // type arguments pass through untouched.
        let (entry_module, entry_function) = match crate::support::resolve_request_entry(
            self.artifact,
            &request.target.module,
            &request.target.function,
            &request.type_arguments,
        ) {
            Ok(entry) => entry,
            Err(reason) => {
                return execution_failure(result, ExecutionStatus::InvalidRequest, reason)
            }
        };
        let Some(contract) = crate::support::entry_value_contract(
            &self.artifact.function_value_contracts,
            &entry_module,
            &entry_function,
        ) else {
            return execution_failure(
                result,
                ExecutionStatus::InvalidRequest,
                "C11 execution requires a language-owned function value contract",
            );
        };
        // Arity gate (P-006): fail closed on a miscounted request before
        // driving the child. Missing arguments surface as an unattributed
        // driver failure and extra arguments are silently ignored without
        // this check. Same message as the LLVM/WASM/Cranelift gates.
        if contract.inputs.len() != request.arguments.len() {
            return execution_failure(
                result,
                ExecutionStatus::InvalidRequest,
                format!(
                    "backend request violates the language-owned value contract: expected {} argument(s), received {}",
                    contract.inputs.len(),
                    request.arguments.len()
                ),
            );
        }
        // Entry symbols are module-qualified (`mncs_<module>__<name>`), so
        // same-named functions from distinct modules lower and execute as
        // distinct natives (ENG-PRESSURE-0017). Refused entrypoints (P1-B02
        // admission) fail closed as Unsupported instead of mislinking.
        let Some(entry) = crate::support::resolve_entry_export(
            &self.artifact.exports,
            &entry_module,
            &entry_function,
        ) else {
            return execution_failure(
                result,
                ExecutionStatus::Unsupported,
                crate::support::unrealized_entry_reason(&entry_module, &entry_function),
            );
        };
        // RFC 0047 §5 uniform fuel: the driver seeds the entry depth from
        // the request budget, so an explicit budget means the same fuel
        // here as on the reference interpreters. The seed joins the cache
        // key because it is baked into the driver source.
        let entry_depth = match crate::support::depth_seed_for_request(request) {
            Ok(seed) => seed,
            Err(reason) => {
                return execution_failure(result, ExecutionStatus::InvalidRequest, reason)
            }
        };
        let driver = c_driver(
            &entry,
            &contract.inputs,
            contract.outputs.first(),
            entry_depth,
        );
        let call_blob = match crate::support::build_call_file(
            &request.arguments,
            &contract.inputs,
            contract.outputs.first(),
            &self.artifact.composite_value_contracts,
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
        let argv = match &call_blob {
            Some(_) => Vec::new(),
            None => match argv_from_request(request) {
                Ok(argv) => argv,
                Err(reason) => {
                    return execution_failure(result, ExecutionStatus::Unsupported, reason)
                }
            },
        };
        // The driver names its entry symbol, so the cache key is the
        // canonical entry identity: two modules may export the same short
        // name with different drivers. The fuel seed joins the key because
        // it is baked into the driver source: reusing a zero-seed
        // executable for a budgeted request would silently grant full fuel.
        let cache_key = format!(
            "{}#depth{entry_depth}",
            crate::support::entry_key(&entry_module, &entry_function)
        );
        if !self.executables.contains_key(&cache_key) {
            let executable = match NativeExecutable::compile_or_reuse(
                &[
                    ("module.c", self.source.as_str()),
                    ("driver.c", driver.as_str()),
                ],
                &self.compiler,
                &["-std=c11", "-O0", "-Wall"],
            ) {
                Ok(executable) => executable,
                Err(error) => return execution_failure(result, error.status(), error.reason()),
            };
            let compiled = executable.was_compiled();
            self.executables.insert(cache_key.clone(), executable);
            if compiled {
                mncs_model::record_counter("backend_compile");
            }
        }
        let executable = self
            .executables
            .get(&cache_key)
            .expect("C11 executable inserted above");
        match executable.run(&argv, call_path.as_deref()) {
            Ok(run) => {
                // WEB-P-012: attribute non-returned observations; never a
                // silent null reason.
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
            Err(error) => execution_failure(result, error.status(), error.reason()),
        }
    }
}

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
            "float conversions use libm trunc behind explicit range traps".to_owned(),
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
            "sequence_or_view_boundary_crossing",
            "nested_composite_sequence_elements",
            "boolean_sequence_elements",
            "vector_or_mask_boundary_crossing",
            "semantic_branchless_select",
            "semantic_integer_vectors",
            "semantic_masks",
            "packed_mask_realization",
            "portable_scalar_vector_fallback",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        [
            "memory",
            "effects",
            "widening_integer",
            "undefined_behavior",
            "unbounded_sequences",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        [C11_ARTIFACT_KIND.to_owned()].into_iter().collect(),
        ["bounded_execution_agreement".to_owned()]
            .into_iter()
            .collect(),
        [
            "external_c_compiler".to_owned(),
            "language_stateful_trace_runner".to_owned(),
        ]
        .into_iter()
        .collect(),
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
    mncs_model::record_counter("backend_lowering");
    if let Err(result) = validate_selected_ssa(ssa, &selected_ssa, "CGC101") {
        return *result;
    }
    if let Err(result) = validate_realizable_ssa(program, ssa, "CGC102") {
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
    // Per-entrypoint admission (P1-B02 partial realization): refused
    // functions no longer poison admitted ones. The artifact realizes the
    // admitted subset (`exports`) and records every refusal in its
    // `unsupported` field — the machine-readable admission report. Only a
    // module with NOTHING realizable still refuses whole-program (CGC301
    // plus per-function CGC302s). On the success path no diagnostics are
    // emitted for refused neighbors; drivers gate entry lookup on
    // `exports` instead.
    if scalar.functions.is_empty() {
        let mut diagnostics = vec![CompilerDiagnostic::new(
            "CGC301",
            CompilerDiagnosticKind::UnavailableBackendCapability,
            "selected SSA is outside the C11 scalar envelope",
        )];
        for reason in &scalar.unsupported {
            diagnostics.push(CompilerDiagnostic::new(
                "CGC302",
                CompilerDiagnosticKind::UnavailableBackendCapability,
                reason.clone(),
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
        ssa.proof_binding_refs(),
        vec![
            "external C compiler".to_owned(),
            "generated C retained as a typed backend artifact".to_owned(),
        ],
        plan.target.evidence.clone(),
        scalar.unsupported.clone(),
        TransformationStatus::Pass,
    )
    .with_function_value_contracts(function_value_contracts(program))
    .with_composite_value_contracts(crate::support::composite_value_contracts(program))
    .with_generic_entrypoints(crate::support::generic_entrypoint_records(program))
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
        "/* MNCS C11 realization 0.2. Not MNCS semantics. No C undefined behavior for integers. */\n#include <stdint.h>\n#include <stdbool.h>\n#include <string.h>\n#include <math.h>\n\n",
    );
    if crate::support::scalar_module_needs_arena_symbols(module) {
        // Shared canonical capacity; the driver declares the same
        // `NATIVE_ARENA_BYTES`, so module and driver always agree.
        let arena = crate::support::NATIVE_ARENA_BYTES;
        out.push_str(&format!(
            "/* Canonical composite cell arena (MNCS cell layout v0.1). Defined\n   whenever the call-file driver may copy an arena image, including\n   mask-only modules that never allocate cells. */\n#define MNCS_ARENA_BYTES ({arena}u)\nunsigned char mncs_arena[MNCS_ARENA_BYTES];\nuint64_t mncs_bump = 0;\n/* Sticky arena-exhaustion flag: allocation arithmetic cannot wrap\n   (every addition is range-checked before it happens), loads and stores\n   bounds-check their address (corrupted or externally restored offsets\n   fail closed instead of touching out-of-bounds memory), and each\n   function converts a set flag into status=1 at its return points.\n   Never SIGSEGV/SIGBUS/poison: exhaustion is a deterministic language\n   runtime failure. Reset at every function entry. */\nuint64_t mncs_failed = 0;\n/* Allocation-cap exhaustion reports distinctly: alloc sites set\n   mncs_exhausted alongside mncs_failed so return points surface status=3\n   (budget_exhausted) with an attributed resource reason instead of a bare\n   status=1. Reset at every function entry alongside mncs_failed. */\nuint64_t mncs_exhausted = 0;\n",
        ));
        if crate::support::scalar_module_uses_cells(module) {
            out.push_str(
                "uint64_t mncs_cell_alloc(uint64_t bytes) {\n  uint64_t base;\n  if (mncs_failed) return 0;\n  if (bytes > MNCS_ARENA_BYTES) { mncs_failed = 1; mncs_exhausted = 1; return 0; }\n  if (mncs_bump > MNCS_ARENA_BYTES) { mncs_failed = 1; mncs_exhausted = 1; return 0; }\n  base = (mncs_bump + 7u) & ~(uint64_t)7u;\n  if (base > MNCS_ARENA_BYTES - bytes) { mncs_failed = 1; mncs_exhausted = 1; return 0; }\n  mncs_bump = base + bytes;\n  return base;\n}\nvoid mncs_slot_store32(unsigned char *a, uint64_t at, uint32_t v) {\n  if (mncs_failed) return;\n  if (at > MNCS_ARENA_BYTES - 4u) { mncs_failed = 1; return; }\n  memcpy(a + at, &v, sizeof v);\n}\nvoid mncs_slot_store64(unsigned char *a, uint64_t at, uint64_t v) {\n  if (mncs_failed) return;\n  if (at > MNCS_ARENA_BYTES - 8u) { mncs_failed = 1; return; }\n  memcpy(a + at, &v, sizeof v);\n}\nuint32_t mncs_slot_load32(const unsigned char *a, uint64_t at) {\n  uint32_t v = 0;\n  if (mncs_failed) return 0;\n  if (at > MNCS_ARENA_BYTES - 4u) { mncs_failed = 1; return 0; }\n  memcpy(&v, a + at, sizeof v);\n  return v;\n}\nuint64_t mncs_slot_load64(const unsigned char *a, uint64_t at) {\n  uint64_t v = 0;\n  if (mncs_failed) return 0;\n  if (at > MNCS_ARENA_BYTES - 8u) { mncs_failed = 1; return 0; }\n  memcpy(&v, a + at, sizeof v);\n  return v;\n}\n",
            );
        }
        out.push('\n');
    }
    for function in &module.functions {
        emit_prototype(&mut out, function);
    }
    out.push('\n');
    let has_arena = crate::support::scalar_module_needs_arena_symbols(module);
    for function in &module.functions {
        emit_function(&mut out, function, has_arena);
        out.push('\n');
    }
    out
}

fn emit_prototype(out: &mut String, function: &ScalarFunction) {
    let params = function
        .params
        .iter()
        .map(|param| c_type(param.ty).to_owned())
        // RFC 0047 call-depth fuel: the prototype must match the definition
        // exactly (hidden trailing `uint64_t mncs_depth`).
        .chain([
            "int32_t*".to_owned(),
            "int64_t*".to_owned(),
            "uint64_t".to_owned(),
        ])
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(out, "void {}({params});", function.export_name);
}

/// Exact float-domain bounds `[lo, hi)` for a guarded float-to-integer
/// conversion (Profile 0.12), as C decimal spellings. Every bound is a
/// power of two (or zero), hence exactly representable in binary64, and
/// the half-open shape matches the reference domain check.
fn c_float_domain_bounds(bits: u16, signed: bool) -> (String, String) {
    let hi: u128 = if signed {
        1_u128 << (bits - 1)
    } else {
        1_u128 << bits
    };
    let lo = if signed {
        format!("-{}.0", 1_u128 << (bits - 1))
    } else {
        "0.0".to_owned()
    };
    (lo, format!("{hi}.0"))
}

fn emit_function(out: &mut String, function: &ScalarFunction, has_arena: bool) {
    let names = CNames::new(function);
    let mut params = function
        .params
        .iter()
        .map(|param| format!("{} {}", c_type(param.ty), names.value(&param.id)))
        .collect::<Vec<_>>();
    params.push("int32_t *mncs_status".to_owned());
    params.push("int64_t *mncs_value".to_owned());
    // RFC 0047 call-depth fuel rides a hidden trailing parameter: every
    // nested call passes one more than it received, so the counter is
    // per-activation (no global to reset, no decrement to balance, exact
    // interpreter equivalence). The driver passes 0 at top level.
    params.push("uint64_t mncs_depth".to_owned());
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
    // RFC 0047 call-depth fuel, checked against the incoming depth so the
    // boundary matches the reference interpreter exactly (incoming depth
    // above the cap fails; depth grows by one per nested call). Exhaustion
    // reports status 3 (BudgetExhausted), observably distinct from the
    // generic failure status 1 the body uses below.
    let _ = writeln!(
        out,
        "  if (mncs_depth > {}u) {{ *mncs_status = 3; *mncs_value = 0; return; }}",
        mncs_model::MODEL_MAX_CALL_DEPTH
    );
    if has_arena {
        // Reset at top-level entry only: exhaustion in one execution must
        // not poison the next call in a reused process or stateful
        // session, but a recursive entry must never clear a flag set by
        // an outer activation (RFC 0047 structural recursion).
        out.push_str("  if (mncs_depth == 0) { mncs_failed = 0; mncs_exhausted = 0; }\n");
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
                if has_arena {
                    // Sticky exhaustion becomes a deterministic language
                    // runtime failure here; a poisoned value never escapes.
                    out.push_str(
                        "      if (mncs_failed) { *mncs_status = mncs_exhausted ? 3 : 1; *mncs_value = 0; return; }\n",
                    );
                }
                out.push_str("      *mncs_status = 0;\n");
                // Float results cross as bit-carried words: a converting
                // store would truncate (and overflow undefined behavior),
                // so doubles memcpy their payload into the i64 cell.
                if matches!(function.result.ty, crate::scalar::ScalarTy::Float) {
                    let _ = writeln!(
                        out,
                        "      memcpy(mncs_value, &{}, sizeof({}));",
                        names.value(value),
                        names.value(value)
                    );
                } else {
                    let _ = writeln!(out, "      *mncs_value = (int64_t){};", names.value(value));
                }
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
                ScalarTy::Mask(_) => 64,
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
                    let expression = if bits > 63 {
                        format!("mncs_w << {count}")
                    } else {
                        format!("(mncs_w << {count}) & {mask}")
                    };
                    let _ = writeln!(out, "      {{ uint64_t mncs_w = (uint64_t)(uint64_t){lhs_n}; mncs_w = {expression}; {dest_n} = ({})mncs_w; }}", c_type(dest.ty));
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
                    // Mask to the declared width BEFORE shifting: the C
                    // variable is signed, so an unmasked cast sign-extends
                    // (a u32 with bit 31 set becomes 0xFFFF_FFFF_xxxxxxxx
                    // and the shift-then-narrow keeps ones the logical
                    // shift must clear).
                    let _ = writeln!(
                        out,
                        "      {dest_n} = ({})(((uint64_t){lhs_n} & {mask}) >> {count});",
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
                // MNCS pins `MIN % -1 == 0` without trapping (only `MIN / -1`
                // traps), so signed remainder guards the same edge into a
                // total zero instead of a SIGFPE. Unsigned operands divide in
                // their own modular domain, not through the signed C type.
                let slash = if operator == "div" { "/" } else { "%" };
                let overflow_guard = if operator == "div" && signed {
                    format!(
                        "if ({lhs_n} == INT{bits}_MIN && {rhs_n} == -1) {{ *mncs_status = 1; *mncs_value = 0; return; }} "
                    )
                } else if operator == "mod" && signed {
                    format!(
                        "if ({lhs_n} == INT{bits}_MIN && {rhs_n} == -1) {{ {dest_n} = ({})0; }} else ",
                        c_type(dest.ty)
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
                // Unsigned MNCS values ride in signed C cells (`int32_t` for
                // widths <= 32, `int64_t` for 64-bit), so the widening must
                // reinterpret through the matching unsigned cell width first:
                // a direct `(unsigned __int128)` of the signed cell would
                // sign-extend and mistrap every operand >= 2^63, while
                // `(uint64_t)` of a negative `int32_t` cell sign-extends to
                // a huge 64-bit value and mistraps every u32 operand >= 2^31
                // (P1-B01: 255*16777216 trapped on C11 only).
                let wide_ty = if signed {
                    "__int128"
                } else {
                    "unsigned __int128"
                };
                // Narrow unsigned cells reinterpret via `(uint32_t)` (C
                // signed-to-unsigned conversion is modular, hence bit-exact);
                // 64-bit unsigned cells reinterpret via `(uint64_t)`.
                let unsigned_narrow = !signed && bits <= 32;
                let (lhs_w, rhs_w) = if signed {
                    (format!("({wide_ty}){lhs_n}"), format!("({wide_ty}){rhs_n}"))
                } else if unsigned_narrow {
                    (
                        format!("({wide_ty})(uint32_t){lhs_n}"),
                        format!("({wide_ty})(uint32_t){rhs_n}"),
                    )
                } else {
                    (
                        format!("({wide_ty})(uint64_t){lhs_n}"),
                        format!("({wide_ty})(uint64_t){rhs_n}"),
                    )
                };
                let _ = writeln!(
                    out,
                    "      {{ {wide_ty} mncs_wide = {lhs_w} {op} {rhs_w}; if (mncs_wide > {max} || mncs_wide < {min}) {{ *mncs_status = 1; *mncs_value = 0; return; }} {dest_n} = ({})mncs_wide; }}",
                    c_type(dest.ty),
                    max = max_text,
                    min = min_text,
                );
            } else if matches!(intent, ArithmeticIntent::Saturating) {
                // Total by definition: compute in the wide domain, then clamp
                // into the declared representable range. Unsigned cells
                // reinterpret through the matching unsigned width for the
                // same reason as the checked path above (P1-B01).
                let wide_ty = if signed {
                    "__int128"
                } else {
                    "unsigned __int128"
                };
                let unsigned_narrow = !signed && bits <= 32;
                let (lhs_w, rhs_w) = if signed {
                    (format!("({wide_ty}){lhs_n}"), format!("({wide_ty}){rhs_n}"))
                } else if unsigned_narrow {
                    (
                        format!("({wide_ty})(uint32_t){lhs_n}"),
                        format!("({wide_ty})(uint32_t){rhs_n}"),
                    )
                } else {
                    (
                        format!("({wide_ty})(uint64_t){lhs_n}"),
                        format!("({wide_ty})(uint64_t){rhs_n}"),
                    )
                };
                let _ = writeln!(
                    out,
                    "      {{ {wide_ty} mncs_wide = {lhs_w} {op} {rhs_w}; if (mncs_wide > {max}) {{ mncs_wide = {max}; }} if (mncs_wide < {min}) {{ mncs_wide = {min}; }} {dest_n} = ({})mncs_wide; }}",
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
        ScalarInst::BooleanCompare {
            dest,
            predicate,
            lhs,
            rhs,
        } => {
            // Normalized 0/1 slots: `==`/`!=` is exact. Unknown predicates
            // are rejected at body validation; the fallback traps loudly
            // rather than emitting a wrong comparison.
            let op = match predicate.as_str() {
                "eq" => "==",
                "ne" => "!=",
                _ => "!==",
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
        ScalarInst::BooleanNot { dest, src } => {
            let _ = writeln!(
                out,
                "      {} = ({})!{};",
                names.value(&dest.id),
                c_type(dest.ty),
                names.value(src)
            );
        }
        ScalarInst::Compare {
            dest,
            predicate,
            operand,
            lhs,
            rhs,
        } => {
            let pred = match predicate.as_str() {
                "eq" => "==",
                "ne" => "!=",
                "lt" => "<",
                "le" => "<=",
                "gt" => ">",
                _ => ">=",
            };
            // C `int64_t` storage is bit-preserving; unsigned MNCS integers
            // still compare in their unsigned domain, matching LLVM `icmp u*`
            // and Cranelift unsigned IntCC.
            let left = names.value(lhs);
            let right = names.value(rhs);
            let compare = if operand.signed || matches!(predicate.as_str(), "eq" | "ne") {
                format!("{left} {pred} {right}")
            } else {
                let unsigned_ty = match operand.bits {
                    8 => "uint8_t",
                    16 => "uint16_t",
                    32 => "uint32_t",
                    _ => "uint64_t",
                };
                format!("({unsigned_ty}){left} {pred} ({unsigned_ty}){right}")
            };
            let _ = writeln!(
                out,
                "      {} = ({})({compare});",
                names.value(&dest.id),
                c_type(dest.ty)
            );
        }
        ScalarInst::FloatConst { dest, bits } => {
            // Bit-exact materialization: memcpy the payload, never a
            // decimal spelling (which would still round correctly, but
            // bits leave no room for doubt).
            let _ = writeln!(
                out,
                "      {{ uint64_t mncs_bits = {bits}ULL; memcpy(&{0}, &mncs_bits, sizeof({0})); }}",
                names.value(&dest.id),
            );
        }
        ScalarInst::Float {
            dest,
            operator,
            lhs,
            rhs,
        } => {
            let op = match operator.as_str() {
                "add" => "+",
                "sub" => "-",
                "mul" => "*",
                "div" => "/",
                _ => {
                    let _ = writeln!(out, "      *mncs_status = 2; *mncs_value = 0; return;");
                    return;
                }
            };
            let dest_n = names.value(&dest.id);
            let lhs_n = names.value(lhs);
            let rhs_n = names.value(rhs);
            // The non-finite trap rule without libm: `x - x == 0` holds
            // exactly for finite values (same operand, no rounding) and is
            // NaN otherwise. Guard both inputs and the result.
            let _ = writeln!(
                out,
                "      {dest_n} = {lhs_n} {op} {rhs_n};\n      if (!(({lhs_n} - {lhs_n}) == 0.0) || !(({rhs_n} - {rhs_n}) == 0.0) || !(({dest_n} - {dest_n}) == 0.0)) {{ *mncs_status = 1; *mncs_value = 0; return; }}"
            );
        }
        ScalarInst::FloatIntrinsic {
            dest,
            function,
            src,
        } => {
            // Same-process libm, guarded like arithmetic: `sin`/`cos` of
            // a finite binary64 is finite, and the guards trap otherwise.
            // `neg` is exact IEEE-754 negation (unary minus, no libm);
            // only the input finiteness guard is load-bearing.
            let call = match function.as_str() {
                "sin" => "sin",
                "cos" => "cos",
                "neg" => "-",
                _ => {
                    let _ = writeln!(out, "      *mncs_status = 2; *mncs_value = 0; return;");
                    return;
                }
            };
            let dest_n = names.value(&dest.id);
            let src_n = names.value(src);
            let _ = writeln!(
                out,
                "      {dest_n} = {call}({src_n});\n      if (!(({src_n} - {src_n}) == 0.0) || !(({dest_n} - {dest_n}) == 0.0)) {{ *mncs_status = 1; *mncs_value = 0; return; }}"
            );
        }
        ScalarInst::FloatCompare {
            dest,
            predicate,
            lhs,
            rhs,
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
                "      if (!(({0} - {0}) == 0.0) || !(({1} - {1}) == 0.0)) {{ *mncs_status = 1; *mncs_value = 0; return; }}\n      {2} = {0} {pred} {1};",
                names.value(lhs),
                names.value(rhs),
                names.value(&dest.id),
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
                // Floats ride bit-carried: a numeric `(uint64_t)double`
                // conversion would truncate. Copy the 64-bit pattern.
                if matches!(names.ty(value), ScalarTy::Float) {
                    let _ = writeln!(
                        out,
                        "      {{ uint64_t mncs_bits; memcpy(&mncs_bits, &{}, sizeof(mncs_bits)); mncs_slot_store64(mncs_arena, {} + {byte_offset}, mncs_bits); }}",
                        names.value(value),
                        names.value(cell),
                    );
                } else {
                    let _ = writeln!(
                        out,
                        "      mncs_slot_store64(mncs_arena, {} + {byte_offset}, (uint64_t){});",
                        names.value(cell),
                        names.value(value)
                    );
                }
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
                // Floats ride bit-carried: a numeric `(double)bits`
                // conversion would reinterpret magnitude. Copy the pattern.
                if matches!(dest.ty, ScalarTy::Float) {
                    let _ = writeln!(
                        out,
                        "      {{ uint64_t mncs_bits = mncs_slot_load64(mncs_arena, {} + {byte_offset}); memcpy(&{}, &mncs_bits, sizeof(mncs_bits)); }}",
                        names.value(cell),
                        names.value(&dest.id),
                    );
                } else {
                    let _ = writeln!(
                        out,
                        "      {} = ({})mncs_slot_load64(mncs_arena, {} + {byte_offset});",
                        names.value(&dest.id),
                        c_type(dest.ty),
                        names.value(cell)
                    );
                }
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
        ScalarInst::Convert {
            dest,
            from,
            to,
            src,
        } => {
            // C casts implement exactly the declared total conversion:
            // narrowing truncates high bits, widening extends by the source
            // signedness (`byte` is unsigned).
            if matches!(to, crate::scalar::ScalarTy::Float) {
                // Integer, byte, and boolean domains convert to binary64
                // exactly rounded by hardware. The ABI carries every
                // 64-bit integer in an `int64_t` cell, so an unsigned
                // 64-bit source must reinterpret through `uint64_t`
                // first: a plain `(double)` cast would convert the cell
                // as signed.
                let source = names.value(src);
                if matches!(
                    from,
                    crate::scalar::ScalarTy::Int(integer) if !integer.signed && integer.bits == 64
                ) {
                    let _ = writeln!(
                        out,
                        "      {} = (double)(uint64_t){};",
                        names.value(&dest.id),
                        source
                    );
                } else {
                    let _ = writeln!(out, "      {} = (double){};", names.value(&dest.id), source);
                }
            } else if matches!(from, crate::scalar::ScalarTy::Float) {
                // Float to integer truncates toward zero and traps on
                // non-finite or out-of-range inputs (the float trap rule).
                // The bounds below are powers of two, hence exact; the
                // half-open shape matches the reference domain check, and
                // ordered comparisons reject NaN. `trunc` needs math.h
                // (linked from the host toolchain).
                let (bits, signed, c_target) = match to {
                    crate::scalar::ScalarTy::Int(integer) => (
                        integer.bits,
                        integer.signed,
                        match (integer.bits, integer.signed) {
                            (64, true) => "int64_t",
                            (64, false) => "uint64_t",
                            (32, true) => "int32_t",
                            (32, false) => "uint32_t",
                            (16, true) => "int16_t",
                            (16, false) => "uint16_t",
                            (8, true) => "int8_t",
                            (8, false) => "uint8_t",
                            _ => {
                                let _ = writeln!(
                                    out,
                                    "      *mncs_status = 2; *mncs_value = 0; return;"
                                );
                                return;
                            }
                        },
                    ),
                    crate::scalar::ScalarTy::Byte => (8, false, "uint8_t"),
                    _ => {
                        // No float-to-bool cast exists in the language.
                        let _ = writeln!(out, "      *mncs_status = 2; *mncs_value = 0; return;");
                        return;
                    }
                };
                let (lo, hi) = c_float_domain_bounds(bits, signed);
                let _ = writeln!(
                    out,
                    "      {{ double mncs_v = {}; double mncs_t = trunc(mncs_v); if (!((mncs_v - mncs_v) == 0.0) || !(mncs_t >= {} && mncs_t < {})) {{ *mncs_status = 1; *mncs_value = 0; return; }} {} = ({})mncs_t; }}",
                    names.value(src),
                    lo,
                    hi,
                    names.value(&dest.id),
                    c_target
                );
            } else {
                let _ = writeln!(
                    out,
                    "      {} = ({}){};",
                    names.value(&dest.id),
                    c_type(dest.ty),
                    names.value(src)
                );
            }
        }
        ScalarInst::Select {
            dest,
            condition,
            when_true,
            when_false,
        } => {
            // Strict booleans are normalized to 0/1. Blend in the unsigned
            // domain so the C source itself contains no conditional operator
            // or candidate-dependent control flow. Native branchlessness is
            // still an artifact-level claim, not inferred from this source.
            // Float arms ride bit-carried: blend the 64-bit patterns, then
            // copy the winner into the `double` (numeric casts would trap
            // or reinterpret the magnitude).
            if matches!(dest.ty, ScalarTy::Float) {
                let _ = writeln!(
                    out,
                    "      {{ uint64_t mncs_mask = 0u - (uint64_t){}; uint64_t mncs_t; uint64_t mncs_f; memcpy(&mncs_t, &{}, sizeof(mncs_t)); memcpy(&mncs_f, &{}, sizeof(mncs_f)); uint64_t mncs_r = (mncs_t & mncs_mask) | (mncs_f & ~mncs_mask); memcpy(&{}, &mncs_r, sizeof(mncs_r)); }}",
                    names.value(condition),
                    names.value(when_true),
                    names.value(when_false),
                    names.value(&dest.id),
                );
            } else {
                let _ = writeln!(
                    out,
                    "      {{ uint64_t mncs_mask = 0u - (uint64_t){}; uint64_t mncs_t = (uint64_t){}; uint64_t mncs_f = (uint64_t){}; {} = ({})((mncs_t & mncs_mask) | (mncs_f & ~mncs_mask)); }}",
                    names.value(condition),
                    names.value(when_true),
                    names.value(when_false),
                    names.value(&dest.id),
                    c_type(dest.ty),
                );
            }
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
            let dest_n = names.value(&dest.id);
            let source_n = names.value(source);
            let index_n = names.value(index);
            if matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. }) {
                let _ = writeln!(
                    out,
                    "      if ((uint64_t){index_n} >= {length}ULL) {{ *mncs_status = 1; *mncs_value = 0; return; }}"
                );
            }
            let _ = writeln!(out, "      {dest_n} = mncs_cell_alloc({}u);", length * 8);
            for lane in 0..*length {
                let offset = u64::from(lane) * 8;
                match element_width {
                    SlotWidth::W32 => {
                        let _ = writeln!(
                            out,
                            "      mncs_slot_store32(mncs_arena, {dest_n} + {offset}u, mncs_slot_load32(mncs_arena, {source_n} + {offset}u));"
                        );
                    }
                    SlotWidth::W64 => {
                        let _ = writeln!(
                            out,
                            "      mncs_slot_store64(mncs_arena, {dest_n} + {offset}u, mncs_slot_load64(mncs_arena, {source_n} + {offset}u));"
                        );
                    }
                }
            }
            match element_width {
                SlotWidth::W32 => {
                    let _ = writeln!(
                        out,
                        "      mncs_slot_store32(mncs_arena, {dest_n} + (uint64_t){index_n} * 8u, (uint32_t){});",
                        names.value(element)
                    );
                }
                SlotWidth::W64 => {
                    // Float elements ride bit-carried (see CellStore).
                    if matches!(names.ty(element), ScalarTy::Float) {
                        let _ = writeln!(
                            out,
                            "      {{ uint64_t mncs_bits; memcpy(&mncs_bits, &{}, sizeof(mncs_bits)); mncs_slot_store64(mncs_arena, {dest_n} + (uint64_t){index_n} * 8u, mncs_bits); }}",
                            names.value(element),
                        );
                    } else {
                        let _ = writeln!(
                            out,
                            "      mncs_slot_store64(mncs_arena, {dest_n} + (uint64_t){index_n} * 8u, (uint64_t){});",
                            names.value(element)
                        );
                    }
                }
            }
        }
        ScalarInst::SequenceCopy {
            dest,
            destination,
            dst_at,
            source,
            src_at,
            len,
            dst_bound: _,
            src_bound,
            evidence,
            dst_length,
            element_width,
            ..
        } => {
            let dest_n = names.value(&dest.id);
            let dst_base_n = names.value(destination);
            let dst_at_n = names.value(dst_at);
            let src_at_n = names.value(src_at);
            let len_n = names.value(len);
            // Resolve the source base and runtime length: exact sources are
            // canonical cells with a static length; views unpack their
            // packed descriptor exactly like SequenceProject does.
            let (src_base, src_len) = match src_bound {
                mncs_model::SequenceBound::Exact(length) => {
                    (names.value(source).to_owned(), format!("{length}ULL"))
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    let view = names.value(source);
                    (
                        format!("(uint64_t)(uint32_t)({view})"),
                        format!("({view} >> 32)"),
                    )
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            };
            if matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. }) {
                let _ = writeln!(
                    out,
                    "      {{ uint64_t c_dend = (uint64_t){dst_at_n} + (uint64_t){len_n}; uint64_t c_send = (uint64_t){src_at_n} + (uint64_t){len_n}; if (c_dend < (uint64_t){dst_at_n} || c_dend > {dst_length}ULL || c_send < (uint64_t){src_at_n} || c_send > (uint64_t)({src_len})) {{ *mncs_status = 1; *mncs_value = 0; return; }} }}"
                );
            }
            let _ = writeln!(
                out,
                "      {dest_n} = mncs_cell_alloc({}u);",
                dst_length * 8
            );
            // Branchless per-lane span select over the static destination
            // bound: lane `j` takes the source window slot exactly when it
            // falls in `[dst_at, dst_at + len)`. The fallback source address
            // is the lane's own destination slot, so every emitted load is
            // in-bounds by construction even for lanes outside the window.
            for lane in 0..*dst_length {
                let offset = u64::from(lane) * 8;
                let _ = writeln!(
                    out,
                    "      {{ uint64_t c_k = {lane}ULL - (uint64_t){dst_at_n}; uint64_t c_in = (((uint64_t){dst_at_n} <= {lane}ULL) & (c_k < (uint64_t){len_n})) ? 1u : 0u; uint64_t c_saddr = c_in ? (({src_base}) + (((uint64_t){src_at_n} + c_k) * 8u)) : ((uint64_t){dst_base_n} + {offset}u);"
                );
                match element_width {
                    SlotWidth::W32 => {
                        let _ = writeln!(
                            out,
                            "      uint32_t c_s = mncs_slot_load32(mncs_arena, c_saddr); uint32_t c_d = mncs_slot_load32(mncs_arena, (uint64_t){dst_base_n} + {offset}u); mncs_slot_store32(mncs_arena, {dest_n} + {offset}u, c_in ? c_s : c_d); }}"
                        );
                    }
                    SlotWidth::W64 => {
                        let _ = writeln!(
                            out,
                            "      uint64_t c_s = mncs_slot_load64(mncs_arena, c_saddr); uint64_t c_d = mncs_slot_load64(mncs_arena, (uint64_t){dst_base_n} + {offset}u); mncs_slot_store64(mncs_arena, {dest_n} + {offset}u, c_in ? c_s : c_d); }}"
                        );
                    }
                }
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
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
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
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
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
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            };
            let base = match source_bound {
                mncs_model::SequenceBound::Exact(_) => names.value(source).to_owned(),
                mncs_model::SequenceBound::UpTo(_) => {
                    format!("(uint64_t)(uint32_t)({})", names.value(source))
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
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
        ScalarInst::ViewNarrow {
            dest,
            source,
            new_cap,
        } => {
            let dest_n = names.value(&dest.id);
            let src_n = names.value(source);
            // The static capacity is the only thing that changes: trap
            // unless the runtime span fits, then alias the descriptor.
            let _ = writeln!(
                out,
                "      {dest_n} = {src_n}; if (({dest_n} >> 32) > {new_cap}ULL) {{ *mncs_status = 1; *mncs_value = 0; return; }}"
            );
        }
        ScalarInst::BoundCheck {
            dest,
            seq,
            index,
            bound,
        } => {
            let dest_n = names.value(&dest.id);
            let index_n = names.value(index);
            // The check is always retained: trap unless the candidate sits
            // below the runtime length, then carry it unchanged. Exact
            // bounds fold to constants; views read the packed descriptor.
            let length = match bound {
                mncs_model::SequenceBound::Exact(length) => format!("{length}ULL"),
                mncs_model::SequenceBound::UpTo(_) => {
                    format!("({} >> 32)", names.value(seq))
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            };
            let _ = writeln!(
                out,
                "      if ((uint64_t){index_n} >= {length}) {{ *mncs_status = 1; *mncs_value = 0; return; }} {dest_n} = {index_n};",
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
                .chain([
                    "mncs_status".to_owned(),
                    "mncs_value".to_owned(),
                    "mncs_depth + 1".to_owned(),
                ])
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "      {callee}({list});");
            out.push_str("      if (*mncs_status != 0) { return; }\n");
            // Float results cross as bit-carried words (see Return): a
            // numeric `(double)*mncs_value` conversion would reinterpret
            // the magnitude. Copy the 64-bit pattern instead.
            if matches!(dest.ty, ScalarTy::Float) {
                let _ = writeln!(
                    out,
                    "      memcpy(&{}, mncs_value, sizeof({}));",
                    names.value(&dest.id),
                    names.value(&dest.id),
                );
            } else {
                let _ = writeln!(
                    out,
                    "      {} = ({})*mncs_value;",
                    names.value(&dest.id),
                    c_type(dest.ty)
                );
            }
        }
    }
}

fn inst_dest(inst: &ScalarInst) -> Option<&crate::scalar::ScalarValue> {
    match inst {
        ScalarInst::Const { dest, .. }
        | ScalarInst::FloatConst { dest, .. }
        | ScalarInst::Float { dest, .. }
        | ScalarInst::FloatCompare { dest, .. }
        | ScalarInst::FloatIntrinsic { dest, .. }
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
        | ScalarInst::SequenceCopy { dest, .. }
        | ScalarInst::SequenceProject { dest, .. }
        | ScalarInst::SequenceLength { dest, .. }
        | ScalarInst::ViewConstruct { dest, .. }
        | ScalarInst::ViewNarrow { dest, .. }
        | ScalarInst::BoundCheck { dest, .. }
        | ScalarInst::Call { dest, .. } => Some(dest),
        // Cell stores produce no value; sequences declare through members.
        ScalarInst::CellStoreDiscriminant { .. } | ScalarInst::CellStore { .. } => None,
        ScalarInst::Sequence(_) => None,
    }
}

/// Emit one slot load at a computed byte address inside the canonical arena.
///
/// Float destinations ride bit-carried (see CellLoad): the 64-bit arena
/// pattern is memcopied into the `double`, never numerically converted.
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
            if matches!(dest_ty, crate::scalar::ScalarTy::Float) {
                let _ = writeln!(
                    out,
                    "      {{ uint64_t mncs_bits = mncs_slot_load64(mncs_arena, {address}); memcpy(&{dest_name}, &mncs_bits, sizeof(mncs_bits)); }}",
                );
            } else {
                let _ = writeln!(
                    out,
                    "      {dest_name} = ({})mncs_slot_load64(mncs_arena, {address});",
                    c_type(dest_ty)
                );
            }
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
    // P1-013: resolve a host-requested generic instantiation to its
    // emitted specialization entry before contract/export lookup.
    let (entry_module, entry_function) = match crate::support::resolve_request_entry(
        artifact,
        &request.target.module,
        &request.target.function,
        &request.type_arguments,
    ) {
        Ok(entry) => entry,
        Err(reason) => return execution_failure(result, ExecutionStatus::InvalidRequest, reason),
    };
    let Some(contract) = crate::support::entry_value_contract(
        &artifact.function_value_contracts,
        &entry_module,
        &entry_function,
    ) else {
        return execution_failure(
            result,
            ExecutionStatus::InvalidRequest,
            "C11 execution requires a language-owned function value contract",
        );
    };
    // Arity gate (P-006): same check as the stateful session path, so
    // frozen-artifact execution refuses miscounted requests identically.
    if contract.inputs.len() != request.arguments.len() {
        return execution_failure(
            result,
            ExecutionStatus::InvalidRequest,
            format!(
                "backend request violates the language-owned value contract: expected {} argument(s), received {}",
                contract.inputs.len(),
                request.arguments.len()
            ),
        );
    }
    // Composite arguments and results cross through the canonical call
    // file; pure scalar calls keep the historical argv-only protocol.
    // The entry symbol is module-qualified (ENG-PRESSURE-0017). Refused
    // entrypoints (P1-B02 admission) fail closed as Unsupported.
    let Some(entry) =
        crate::support::resolve_entry_export(&artifact.exports, &entry_module, &entry_function)
    else {
        return execution_failure(
            result,
            ExecutionStatus::Unsupported,
            crate::support::unrealized_entry_reason(&entry_module, &entry_function),
        );
    };
    // RFC 0047 §5 uniform fuel (see the stateful session above).
    let entry_depth = match crate::support::depth_seed_for_request(request) {
        Ok(seed) => seed,
        Err(reason) => return execution_failure(result, ExecutionStatus::InvalidRequest, reason),
    };
    let driver = c_driver(
        &entry,
        &contract.inputs,
        contract.outputs.first(),
        entry_depth,
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
            // A non-returned native observation carries the driver's
            // attributed reason when one exists; a missing reason is a
            // backend defect, never a silent null (WEB-P-012).
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
                Err(reason) => execution_failure(result, ExecutionStatus::InvalidRequest, reason),
            }
        }
        Err(error) => execution_failure(result, error.status(), error.reason()),
    }
}

pub(crate) use crate::support::process_driver as c_driver;
