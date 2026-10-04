use super::*;
use serde::Deserialize;
use mncs_model::{ArithmeticIntent, BackendPromise, IntegerType, SemanticId};
use crate::promises::withheld;
use crate::scalar::{ScalarBlock, ScalarValue};
use std::collections::{BTreeMap, BTreeSet};

/// Verified native SSA normalized into the scalar boundary shared with
/// existing target backends. The native compiler owns the verification; the
/// adapter independently checks signatures, values, edges, and scalar types.
#[derive(Debug, Clone, Deserialize)]
pub struct NativeSsaScalarModule {
    pub schema_version: String,
    pub verification_status: String,
    pub verifier: String,
    pub compiler_source_sha256: String,
    pub stage0_revision: String,
    pub functions: Vec<NativeSsaScalarFunction>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeSsaScalarFunction {
    pub identity: String,
    pub params: Vec<NativeSsaScalarValue>,
    pub result_type: NativeSsaScalarType,
    pub blocks: Vec<NativeSsaScalarBlock>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeSsaScalarValue {
    pub id: String,
    pub ty: NativeSsaScalarType,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NativeSsaScalarType {
    Bool,
    U64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeSsaScalarBlock {
    pub id: u32,
    #[serde(default)]
    pub params: Vec<NativeSsaScalarValue>,
    #[serde(default)]
    pub instructions: Vec<NativeSsaScalarInstruction>,
    pub terminator: NativeSsaScalarTerminator,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeSsaScalarInstruction {
    // u64, not i128: the bool/u64 envelope has no wider constant, and the
    // CLI JSON layer cannot deserialize i128 without arbitrary precision.
    Constant { dest: NativeSsaScalarValue, value: u64 },
    Call { dest: NativeSsaScalarValue, callee: String, args: Vec<String> },
    Integer {
        dest: NativeSsaScalarValue,
        operator: String,
        intent: ArithmeticIntent,
        lhs: String,
        rhs: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeSsaScalarTerminator {
    Return { value: String },
    Jump { target: u32, args: Vec<String> },
    Branch {
        condition: String,
        then_target: u32,
        then_args: Vec<String>,
        else_target: u32,
        else_args: Vec<String>,
    },
}

pub fn emit_verified_native_ssa_c11(native: &NativeSsaScalarModule) -> Result<String, String> {
    if native.schema_version != "mncs.native-scalar-ssa/1"
        || native.verification_status != "pass"
        || native.verifier != "mncs.compiler.ssa.verify"
        || native.compiler_source_sha256.len() != 64
        || native.stage0_revision.trim().is_empty()
    {
        return Err("native SSA artifact lacks a supported schema or verifier PASS binding".into());
    }
    if native.functions.is_empty() {
        return Err("native SSA artifact contains no functions".into());
    }

    let mut signatures = BTreeMap::new();
    for function in &native.functions {
        if !function.identity.starts_with("mncs:0.2:function:")
            || function.blocks.is_empty()
        {
            return Err(format!("malformed native function {}", function.identity));
        }
        let symbol = native_function_symbol(&function.identity)?;
        if signatures.insert(function.identity.clone(), (
            function.params.iter().map(|value| value.ty).collect::<Vec<_>>(),
            function.result_type,
            symbol,
        )).is_some() {
            return Err(format!("duplicate native function {}", function.identity));
        }
    }

    let mut scalar_functions = Vec::new();
    for function in &native.functions {
        let (_, result_type, export_name) = signatures.get(&function.identity).cloned().unwrap();
        let value_id = |id: &str| SemanticId(format!("{}#value:{id}", function.identity));
        let scalar_ty = |ty| match ty {
            NativeSsaScalarType::Bool => ScalarTy::Bool,
            NativeSsaScalarType::U64 => ScalarTy::Int(IntegerType { bits: 64, signed: false }),
        };

        let mut definitions = BTreeMap::new();
        for value in &function.params {
            if definitions.insert(value.id.clone(), value.ty).is_some() {
                return Err(format!("duplicate SSA value {} in {}", value.id, function.identity));
            }
        }
        let mut block_ids = BTreeMap::new();
        for block in &function.blocks {
            if block_ids.insert(block.id, SemanticId(format!("{}#block:{}", function.identity, block.id))).is_some() {
                return Err(format!("duplicate SSA block {} in {}", block.id, function.identity));
            }
            for value in &block.params {
                if definitions.insert(value.id.clone(), value.ty).is_some() {
                    return Err(format!("duplicate SSA value {} in {}", value.id, function.identity));
                }
            }
            for instruction in &block.instructions {
                let dest = match instruction {
                    NativeSsaScalarInstruction::Constant { dest, .. }
                    | NativeSsaScalarInstruction::Call { dest, .. }
                    | NativeSsaScalarInstruction::Integer { dest, .. } => dest,
                };
                if definitions.insert(dest.id.clone(), dest.ty).is_some() {
                    return Err(format!("duplicate SSA value {} in {}", dest.id, function.identity));
                }
            }
        }
        let type_of = |id: &str| definitions.get(id).copied()
            .ok_or_else(|| format!("undefined SSA value {id} in {}", function.identity));

        let mut scalar_blocks = Vec::new();
        let mut decisions = Vec::new();
        for block in &function.blocks {
            let mut instructions = Vec::new();
            for instruction in &block.instructions {
                match instruction {
                    NativeSsaScalarInstruction::Constant { dest, value } => {
                        instructions.push(ScalarInst::Const {
                            dest: ScalarValue { id: value_id(&dest.id), ty: scalar_ty(dest.ty) },
                            value: i128::from(*value),
                        });
                    }
                    NativeSsaScalarInstruction::Call { dest, callee, args } => {
                        let Some((params, returned, symbol)) = signatures.get(callee) else {
                            return Err(format!("unresolved native SSA call target {callee}"));
                        };
                        if params.len() != args.len() || *returned != dest.ty {
                            return Err(format!("call signature mismatch for {callee}"));
                        }
                        for (argument, expected) in args.iter().zip(params) {
                            if type_of(argument)? != *expected {
                                return Err(format!("call argument type mismatch for {callee}"));
                            }
                        }
                        instructions.push(ScalarInst::Call {
                            dest: ScalarValue { id: value_id(&dest.id), ty: scalar_ty(dest.ty) },
                            callee: symbol.clone(),
                            args: args.iter().map(|id| value_id(id)).collect(),
                        });
                    }
                    NativeSsaScalarInstruction::Integer { dest, operator, intent, lhs, rhs } => {
                        match operator.as_str() {
                            "add" | "sub" | "mul" | "div" | "mod" | "and" | "or" | "xor" | "shl" | "shr" => {}
                            _ => return Err(format!("integer operator {operator} is outside the scalar envelope")),
                        }
                        if matches!(intent, ArithmeticIntent::Widening { .. }) {
                            return Err("widening arithmetic intent is outside the scalar envelope".into());
                        }
                        if dest.ty != NativeSsaScalarType::U64 {
                            return Err(format!("integer arithmetic requires a u64 destination in {}", function.identity));
                        }
                        if type_of(lhs)? != NativeSsaScalarType::U64 || type_of(rhs)? != NativeSsaScalarType::U64 {
                            return Err(format!("integer arithmetic requires u64 operands in {}", function.identity));
                        }
                        // The adapter carries no range evidence, so the
                        // no-overflow promise stays withheld: wrapping is
                        // already total, checked/trapping keep their exact
                        // guards, and saturating keeps its clamp. The
                        // withheld decision is recorded for audit.
                        let (promise_kind, reason) = match intent {
                            ArithmeticIntent::Wrapping => (BackendPromise::WrappingArithmetic,
                                "wrapping intent is already conservative arithmetic; no no-overflow promise is implied"),
                            ArithmeticIntent::Saturating => (BackendPromise::NoOverflow,
                                "saturating semantics do not consume a no-overflow backend promise"),
                            _ => (BackendPromise::NoOverflow,
                                "native SSA admission carries no range evidence; guards stay exact"),
                        };
                        let promise = withheld(promise_kind, value_id(&dest.id), reason);
                        decisions.push(promise.decision.clone());
                        instructions.push(ScalarInst::Integer {
                            dest: ScalarValue { id: value_id(&dest.id), ty: scalar_ty(dest.ty) },
                            operator: operator.clone(),
                            intent: *intent,
                            lhs: value_id(lhs),
                            rhs: value_id(rhs),
                            promise: Box::new(promise),
                        });
                    }
                }
            }
            let check_edge = |target: &u32, args: &[String]| -> Result<(SemanticId, Vec<SemanticId>), String> {
                let Some(target_id) = block_ids.get(target) else {
                    return Err(format!("SSA edge targets missing block {target}"));
                };
                let target_block = function.blocks.iter().find(|candidate| candidate.id == *target).unwrap();
                if args.len() != target_block.params.len() {
                    return Err(format!("SSA edge arity mismatch at block {target}"));
                }
                for (argument, parameter) in args.iter().zip(&target_block.params) {
                    if type_of(argument)? != parameter.ty {
                        return Err(format!("SSA edge type mismatch at block {target}"));
                    }
                }
                Ok((target_id.clone(), args.iter().map(|id| value_id(id)).collect()))
            };
            let terminator = match &block.terminator {
                NativeSsaScalarTerminator::Return { value } => {
                    if type_of(value)? != result_type {
                        return Err(format!("return type mismatch in {}", function.identity));
                    }
                    ScalarTerm::Return { value: value_id(value) }
                }
                NativeSsaScalarTerminator::Jump { target, args } => {
                    let (target, args) = check_edge(target, args)?;
                    ScalarTerm::Jump { target, args }
                }
                NativeSsaScalarTerminator::Branch { condition, then_target, then_args, else_target, else_args } => {
                    if type_of(condition)? != NativeSsaScalarType::Bool {
                        return Err(format!("branch condition must be bool in {}", function.identity));
                    }
                    let (then_target, then_args) = check_edge(then_target, then_args)?;
                    let (else_target, else_args) = check_edge(else_target, else_args)?;
                    ScalarTerm::Branch {
                        cond: value_id(condition),
                        then_target, then_args, else_target, else_args,
                    }
                }
            };
            scalar_blocks.push(ScalarBlock {
                id: block_ids[&block.id].clone(),
                params: block.params.iter().map(|value| ScalarValue {
                    id: value_id(&value.id), ty: scalar_ty(value.ty),
                }).collect(),
                insts: instructions,
                term: terminator,
            });
        }
        scalar_functions.push(ScalarFunction {
            export_name,
            params: function.params.iter().map(|value| ScalarValue {
                id: value_id(&value.id), ty: scalar_ty(value.ty),
            }).collect(),
            result: ScalarValue {
                id: SemanticId(format!("{}#result", function.identity)),
                ty: scalar_ty(result_type),
            },
            blocks: scalar_blocks,
            promises: Vec::new(),
            promise_decisions: decisions,
        });
    }

    let identities: BTreeSet<_> = signatures.keys().collect();
    for function in &native.functions {
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            if let NativeSsaScalarInstruction::Call { callee, .. } = instruction {
                if !identities.contains(callee) {
                    return Err(format!("call target {callee} is outside the verified module"));
                }
            }
        }
    }
    let module_decisions: Vec<_> = scalar_functions.iter()
        .flat_map(|function| function.promise_decisions.iter().cloned())
        .collect();
    Ok(emit_module(&ScalarModule {
        functions: scalar_functions,
        unsupported: Vec::new(),
        features: vec!["verified-native-scalar-ssa".into()],
        promise_decisions: module_decisions,
    }))
}

fn native_function_symbol(identity: &str) -> Result<String, String> {
    let body = identity.strip_prefix("mncs:0.2:function:")
        .ok_or_else(|| format!("invalid canonical function identity {identity}"))?;
    let (module, function) = body.rsplit_once("::")
        .filter(|(module, function)| !module.is_empty() && !function.is_empty())
        .ok_or_else(|| format!("invalid canonical function identity {identity}"))?;
    Ok(crate::support::qualified_c_symbol(module, function))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn verified_module() -> NativeSsaScalarModule {
        NativeSsaScalarModule {
            schema_version: "mncs.native-scalar-ssa/1".into(),
            verification_status: "pass".into(),
            verifier: "mncs.compiler.ssa.verify".into(),
            compiler_source_sha256: "a".repeat(64),
            stage0_revision: "pinned-stage0".into(),
            functions: vec![
                NativeSsaScalarFunction {
                    identity: "mncs:0.2:function:dep::echo".into(),
                    params: vec![NativeSsaScalarValue { id: "0".into(), ty: NativeSsaScalarType::U64 }],
                    result_type: NativeSsaScalarType::U64,
                    blocks: vec![NativeSsaScalarBlock {
                        id: 0,
                        params: vec![],
                        instructions: vec![],
                        terminator: NativeSsaScalarTerminator::Return { value: "0".into() },
                    }],
                },
                NativeSsaScalarFunction {
                    identity: "mncs:0.2:function:app::run".into(),
                    params: vec![
                        NativeSsaScalarValue { id: "0".into(), ty: NativeSsaScalarType::Bool },
                        NativeSsaScalarValue { id: "1".into(), ty: NativeSsaScalarType::U64 },
                    ],
                    result_type: NativeSsaScalarType::U64,
                    blocks: vec![
                        NativeSsaScalarBlock {
                            id: 0,
                            params: vec![],
                            instructions: vec![NativeSsaScalarInstruction::Call {
                                dest: NativeSsaScalarValue { id: "2".into(), ty: NativeSsaScalarType::U64 },
                                callee: "mncs:0.2:function:dep::echo".into(),
                                args: vec!["1".into()],
                            }],
                            terminator: NativeSsaScalarTerminator::Branch {
                                condition: "0".into(),
                                then_target: 1,
                                then_args: vec!["2".into()],
                                else_target: 2,
                                else_args: vec!["2".into()],
                            },
                        },
                        NativeSsaScalarBlock {
                            id: 1,
                            params: vec![NativeSsaScalarValue { id: "3".into(), ty: NativeSsaScalarType::U64 }],
                            instructions: vec![],
                            terminator: NativeSsaScalarTerminator::Jump { target: 3, args: vec!["3".into()] },
                        },
                        NativeSsaScalarBlock {
                            id: 2,
                            params: vec![NativeSsaScalarValue { id: "4".into(), ty: NativeSsaScalarType::U64 }],
                            instructions: vec![],
                            terminator: NativeSsaScalarTerminator::Jump { target: 3, args: vec!["4".into()] },
                        },
                        NativeSsaScalarBlock {
                            id: 3,
                            params: vec![NativeSsaScalarValue { id: "5".into(), ty: NativeSsaScalarType::U64 }],
                            instructions: vec![],
                            terminator: NativeSsaScalarTerminator::Return { value: "5".into() },
                        },
                    ],
                },
            ],
        }
    }

    #[test]
    fn native_ssa_c11_executes_imported_call_and_block_parameter_join() {
        let source = emit_verified_native_ssa_c11(&verified_module()).unwrap();
        assert!(source.contains("mncs_dep__echo"));
        assert!(source.contains("mncs_app__run"));

        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("mncs-native-ssa-c11-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let module_path = dir.join("module.c");
        let driver_path = dir.join("driver.c");
        let executable_path = dir.join("run");
        std::fs::write(&module_path, source).unwrap();
        std::fs::write(&driver_path, r#"#include <stdint.h>
extern void mncs_app__run(uint8_t, uint64_t, int32_t *, int64_t *, uint64_t);
int main(void) {
  int32_t status = 0;
  int64_t value = 0;
  mncs_app__run(0, 37, &status, &value, 64);
  if (status || value != 37) return 1;
  mncs_app__run(1, 81, &status, &value, 64);
  if (status || value != 81) return 2;
  return 0;
}
"#).unwrap();

        let compiler = if std::process::Command::new("clang").arg("--version").output().is_ok() {
            "clang"
        } else {
            "cc"
        };
        let compiled = std::process::Command::new(compiler)
            .args(["-std=c11", "-O0"])
            .arg(&module_path)
            .arg(&driver_path)
            .arg("-lm")
            .arg("-o")
            .arg(&executable_path)
            .output()
            .unwrap();
        assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
        let executed = std::process::Command::new(&executable_path).status().unwrap();
        assert!(executed.success());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn native_ssa_c11_refuses_unbound_or_malformed_facts() {
        let mut module = verified_module();
        module.verification_status = "unknown".into();
        assert!(emit_verified_native_ssa_c11(&module).is_err());

        let mut module = verified_module();
        module.functions[1].blocks[0].terminator = NativeSsaScalarTerminator::Jump {
            target: 999,
            args: vec![],
        };
        assert!(emit_verified_native_ssa_c11(&module).is_err());
    }

    fn arithmetic_module() -> NativeSsaScalarModule {
        let mut module = verified_module();
        module.functions.push(NativeSsaScalarFunction {
            identity: "mncs:0.2:function:app::arith".into(),
            params: vec![
                NativeSsaScalarValue { id: "0".into(), ty: NativeSsaScalarType::U64 },
                NativeSsaScalarValue { id: "1".into(), ty: NativeSsaScalarType::U64 },
            ],
            result_type: NativeSsaScalarType::U64,
            blocks: vec![NativeSsaScalarBlock {
                id: 0,
                params: vec![],
                instructions: vec![
                    NativeSsaScalarInstruction::Integer {
                        dest: NativeSsaScalarValue { id: "2".into(), ty: NativeSsaScalarType::U64 },
                        operator: "add".into(),
                        intent: ArithmeticIntent::Checked,
                        lhs: "0".into(),
                        rhs: "1".into(),
                    },
                    NativeSsaScalarInstruction::Integer {
                        dest: NativeSsaScalarValue { id: "3".into(), ty: NativeSsaScalarType::U64 },
                        operator: "mul".into(),
                        intent: ArithmeticIntent::Wrapping,
                        lhs: "2".into(),
                        rhs: "1".into(),
                    },
                ],
                terminator: NativeSsaScalarTerminator::Return { value: "3".into() },
            }],
        });
        module
    }

    #[test]
    fn native_ssa_c11_executes_checked_and_wrapping_integer_arithmetic() {
        let source = emit_verified_native_ssa_c11(&arithmetic_module()).unwrap();
        assert!(source.contains("mncs_app__arith"));
        // Checked intent keeps its exact wide-intermediate guard; the
        // withheld no-overflow promise never trims it.
        assert!(source.contains("mncs_wide"));

        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("mncs-native-ssa-arith-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let module_path = dir.join("module.c");
        let driver_path = dir.join("driver.c");
        let executable_path = dir.join("run");
        std::fs::write(&module_path, source).unwrap();
        std::fs::write(&driver_path, r#"#include <stdint.h>
extern void mncs_app__arith(uint64_t, uint64_t, int32_t *, int64_t *, uint64_t);
int main(void) {
  int32_t status = 0;
  int64_t value = 0;
  /* (6 + 7) * 7 = 91, no overflow anywhere. */
  mncs_app__arith(6, 7, &status, &value, 64);
  if (status || value != 91) return 1;
  /* 6 + 0 then * 0: wrapping multiply by zero stays total. */
  mncs_app__arith(6, 0, &status, &value, 64);
  if (status || value != 0) return 2;
  /* (2^63 + 2) * 2^63 wraps mod 2^64 to 0 with status 0: the wrapping
     multiply must not trap where the checked add did not overflow. */
  mncs_app__arith(2, 0x8000000000000000ULL, &status, &value, 64);
  if (status || value != 0) return 3;
  /* UINT64_MAX + 1 overflows the checked add: exact trap, status 1. */
  mncs_app__arith(0xFFFFFFFFFFFFFFFFULL, 1, &status, &value, 64);
  if (!status) return 4;
  return 0;
}
"#).unwrap();

        let compiler = if std::process::Command::new("clang").arg("--version").output().is_ok() {
            "clang"
        } else {
            "cc"
        };
        let compiled = std::process::Command::new(compiler)
            .args(["-std=c11", "-O0"])
            .arg(&module_path)
            .arg(&driver_path)
            .arg("-lm")
            .arg("-o")
            .arg(&executable_path)
            .output()
            .unwrap();
        assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
        let executed = std::process::Command::new(&executable_path).status().unwrap();
        assert!(executed.success());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn native_ssa_c11_refuses_arithmetic_outside_the_envelope() {
        // Unknown operator.
        let mut module = arithmetic_module();
        module.functions[2].blocks[0].instructions[0] = NativeSsaScalarInstruction::Integer {
            dest: NativeSsaScalarValue { id: "2".into(), ty: NativeSsaScalarType::U64 },
            operator: "pow".into(),
            intent: ArithmeticIntent::Checked,
            lhs: "0".into(),
            rhs: "1".into(),
        };
        assert!(emit_verified_native_ssa_c11(&module).is_err());

        // Widening intent cannot ride the fixed-cell scalar envelope.
        let mut module = arithmetic_module();
        module.functions[2].blocks[0].instructions[0] = NativeSsaScalarInstruction::Integer {
            dest: NativeSsaScalarValue { id: "2".into(), ty: NativeSsaScalarType::U64 },
            operator: "add".into(),
            intent: ArithmeticIntent::Widening { bits: 128 },
            lhs: "0".into(),
            rhs: "1".into(),
        };
        assert!(emit_verified_native_ssa_c11(&module).is_err());

        // Non-u64 destination.
        let mut module = arithmetic_module();
        module.functions[2].blocks[0].instructions[0] = NativeSsaScalarInstruction::Integer {
            dest: NativeSsaScalarValue { id: "2".into(), ty: NativeSsaScalarType::Bool },
            operator: "add".into(),
            intent: ArithmeticIntent::Checked,
            lhs: "0".into(),
            rhs: "1".into(),
        };
        assert!(emit_verified_native_ssa_c11(&module).is_err());
    }
}
