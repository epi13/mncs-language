#!/usr/bin/env python3
"""One-shot cranelift_backend.rs surgery for the AOT + div/mod + booleans work."""

path = "crates/mncs-codegen/src/cranelift_backend.rs"
t = open(path).read()

# --- 1. split jit_scalar into host_isa + aot_object_bytes + declare_and_build ---
old_start = t.find("fn jit_scalar(")
decl_end = t.find("    let mut declared: BTreeMap<String, FuncId> = BTreeMap::new();")
assert old_start > 0 and decl_end > old_start

new_head = '''/// Host ISA for JIT and AOT realizations (same target facts).
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

'''
t = t[:old_start] + new_head + t[decl_end:]

tail_marker = """    module
        .finalize_definitions()
        .map_err(|error| error.to_string())?;
    let func_id = declared"""
tail_idx = t.find(tail_marker)
assert tail_idx > 0
new_tail = '''    Ok(declared)
}

fn jit_scalar(
    scalar: &ScalarModule,
    function_name: &str,
    request: &ExecutionRequest,
) -> Result<(ExecutionStatus, i128), String> {
    use cranelift_jit::{JITBuilder, JITModule};

    let isa = host_isa()?;
    let jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let mut module = JITModule::new(jit_builder);
    let declared = declare_and_build(&mut module, scalar)?;
    module
        .finalize_definitions()
        .map_err(|error| error.to_string())?;
    let func_id = declared'''
t = t[:tail_idx] + new_tail + t[tail_idx + len(tail_marker):]

# --- 2. Boolean arm in emit_clif_inst ---
old_clif_head = '''fn emit_clif_inst(out: &mut String, inst: &ScalarInst, names: &ClifNames) {
    match inst {
        ScalarInst::Const { dest, value } => {'''
new_clif_head = '''fn emit_clif_inst(out: &mut String, inst: &ScalarInst, names: &ClifNames) {
    match inst {
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
        ScalarInst::Const { dest, value } => {'''
assert old_clif_head in t
t = t.replace(old_clif_head, new_clif_head)

# --- 3. div/mod in emit_clif_inst Integer arm ---
old_bitwise = '''            if matches!(operator.as_str(), "and" | "or" | "xor") {
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
            };'''
new_bitwise = '''            if matches!(operator.as_str(), "div" | "mod") {
                // Cranelift sdiv traps on zero and MIN / -1; srem traps on
                // zero. That matches MNCS checked semantics. Narrow division
                // computes through i64 so trap behavior stays well-defined.
                let native = match (operator.as_str(), clif_ty(dest.ty)) {
                    ("mod", "i64") => "srem",
                    (_, "i64") => "sdiv",
                    ("mod", _) => "srem",
                    (_, _) => "sdiv",
                };
                if ty == "i64" {
                    let _ = writeln!(out, "        {dest_n} = {native} {lhs_n}, {rhs_n}");
                } else {
                    let _ = writeln!(out, "        {dest_n}_w = sext.{ty} {lhs_n}");
                    let _ = writeln!(out, "        {dest_n}_x = sext.{ty} {rhs_n}");
                    let _ = writeln!(
                        out,
                        "        {dest_n}_q = {native}.i64 {dest_n}_w, {dest_n}_x"
                    );
                    let _ = writeln!(out, "        {dest_n} = ireduce.i64 {dest_n}_q");
                }
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
            let op = match operator.as_str() {
                "sub" => "isub",
                "mul" => "imul",
                _ => "iadd",
            };'''
assert old_bitwise in t
t = t.replace(old_bitwise, new_bitwise)

# --- 4. inst_dest arms (both sites share this text) ---
old_dest = '''                let dest = match inst {
                    ScalarInst::Const { dest, .. }
                    | ScalarInst::Integer { dest, .. }
                    | ScalarInst::Compare { dest, .. }'''
new_dest = '''                let dest = match inst {
                    ScalarInst::Const { dest, .. }
                    | ScalarInst::Integer { dest, .. }
                    | ScalarInst::Boolean { dest, .. }
                    | ScalarInst::Compare { dest, .. }'''
assert old_dest in t
t = t.replace(old_dest, new_dest)

# --- 5. JIT builder: Boolean + div/mod ---
old_jit = '''                        ScalarInst::Integer {
                            dest,
                            operator,
                            intent,
                            lhs,
                            rhs,
                            promise,
                        } => {
                            let left = values[lhs];
                            let right = values[rhs];
                            if matches!(operator.as_str(), "and" | "or" | "xor") {'''
new_jit = '''                        ScalarInst::Boolean {
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
                                // Cranelift sdiv/srem trap exactly like MNCS
                                // checked division; the host maps traps to
                                // runtime failures.
                                let produced = if operator == "div" {
                                    builder.ins().sdiv(left, right)
                                } else {
                                    builder.ins().srem(left, right)
                                };
                                values.insert(dest.id.clone(), produced);
                                continue;
                            }
                            if matches!(operator.as_str(), "and" | "or" | "xor") {'''
assert old_jit in t
t = t.replace(old_jit, new_jit)

# --- 6. AOT fallback execution path ---
old_exec = '''        Err(reason) => execution_failure(result, ExecutionStatus::Unsupported, reason),
    }
}

fn jit_execute('''
new_exec = '''        Err(reason) => {
            // Host JIT may be denied executable memory by policy. That is an
            // environment restriction, not a lowering or semantic failure;
            // fall back to the AOT object + external-link route when a
            // toolchain is present.
            if reason.contains("readable+executable") || reason.contains("executable memory") {
                match aot_fallback_execute(artifact, &payload, request, contract) {
                    Ok((status, value)) => {
                        result.status = status;
                        result.steps = 1;
                        if status == ExecutionStatus::Returned {
                            result.returned = vec![mncs_model::ExecutionValue::Integer {
                                value,
                                ty: mncs_model::IntegerType {
                                    bits: 64,
                                    signed: true,
                                },
                            }];
                        } else {
                            result.failure = Some(mncs_model::ExecutionFailure {
                                identity: Some(artifact.identity.clone()),
                                reason: "AOT realization reported a runtime failure".to_owned(),
                            });
                        }
                        result
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
/// link it against the shared process driver, and observe it externally.
fn aot_fallback_execute(
    artifact: &mncs_model::BackendArtifact,
    payload: &CraneliftPayload,
    request: &ExecutionRequest,
    contract: &mncs_model::BackendFunctionValueContract,
) -> Result<(ExecutionStatus, i128), String> {
    use crate::native::{compile_object_and_run, probe_clang, probe_gcc};
    let names = function_names(&payload.program, &payload.ssa);
    let scalar = lower_to_scalar(&payload.program, &payload.ssa, &names);
    // Composite arguments cannot cross this process boundary yet; they fail
    // closed exactly like the C11/LLVM drivers instead of mis-typing.
    for value in &request.arguments {
        if !matches!(
            value,
            mncs_model::ExecutionValue::Boolean { .. } | mncs_model::ExecutionValue::Integer { .. }
        ) {
            return Err("composite arguments are outside the current Cranelift AOT process-boundary envelope".to_owned());
        }
    }
    let object = aot_object_bytes(&scalar)?;
    let linker = probe_clang()
        .or_else(probe_gcc)
        .ok_or_else(|| "neither clang nor gcc is present".to_owned())?;
    let driver =
        crate::support::process_driver_i64(&request.target.function, contract.inputs.len());
    let argv = argv_from_request(request).map_err(|error| error)?;
    compile_object_and_run("mncs.o", &object, "driver.c", &driver, &linker, &argv)
}

fn jit_execute('''
assert old_exec in t
t = t.replace(old_exec, new_exec)

open(path, "w").write(t)
print("cranelift surgery complete")
