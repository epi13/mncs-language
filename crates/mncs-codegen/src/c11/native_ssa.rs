use super::*;
use crate::scalar::{ScalarBlock, ScalarValue};
use mncs_model::{ArithmeticIntent, BackendPromise, IntegerType, SemanticId};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// Unattested scalar SSA presented for structural C11 lowering. The adapter
/// checks signatures, values, edges, and scalar types, but this document is
/// not a compiler proof and cannot establish a source-to-SSA relationship.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSsaScalarModule {
    pub schema_version: String,
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
    Constant {
        dest: NativeSsaScalarValue,
        value: u64,
    },
    Call {
        dest: NativeSsaScalarValue,
        callee: String,
        args: Vec<String>,
    },
    Integer {
        dest: NativeSsaScalarValue,
        operator: String,
        lhs: String,
        rhs: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeSsaScalarTerminator {
    Return {
        value: String,
    },
    Jump {
        target: u32,
        args: Vec<String>,
    },
    Branch {
        condition: String,
        then_target: u32,
        then_args: Vec<String>,
        else_target: u32,
        else_args: Vec<String>,
    },
}

pub fn emit_structurally_checked_native_ssa_c11(
    native: &NativeSsaScalarModule,
) -> Result<String, String> {
    if native.schema_version != "mncs.native-scalar-ssa-structural/1" {
        return Err("input is not the explicitly unattested native scalar SSA schema".into());
    }
    if native.functions.is_empty() {
        return Err("native SSA artifact contains no functions".into());
    }

    let mut signatures = BTreeMap::new();
    for function in &native.functions {
        if !function.identity.starts_with("mncs:0.2:function:") || function.blocks.is_empty() {
            return Err(format!("malformed native function {}", function.identity));
        }
        let symbol = native_function_symbol(&function.identity)?;
        if signatures
            .insert(
                function.identity.clone(),
                (
                    function
                        .params
                        .iter()
                        .map(|value| value.ty)
                        .collect::<Vec<_>>(),
                    function.result_type,
                    symbol,
                ),
            )
            .is_some()
        {
            return Err(format!("duplicate native function {}", function.identity));
        }
    }

    let mut scalar_functions = Vec::new();
    for function in &native.functions {
        let (_, result_type, export_name) = signatures.get(&function.identity).cloned().unwrap();
        let value_id = |id: &str| SemanticId(format!("{}#value:{id}", function.identity));
        let scalar_ty = |ty| match ty {
            NativeSsaScalarType::Bool => ScalarTy::Bool,
            NativeSsaScalarType::U64 => ScalarTy::Int(IntegerType {
                bits: 64,
                signed: false,
            }),
        };

        let mut definitions = BTreeMap::new();
        for value in &function.params {
            if definitions.insert(value.id.clone(), value.ty).is_some() {
                return Err(format!(
                    "duplicate SSA value {} in {}",
                    value.id, function.identity
                ));
            }
        }
        let mut block_ids = BTreeMap::new();
        for block in &function.blocks {
            if block_ids
                .insert(
                    block.id,
                    SemanticId(format!("{}#block:{}", function.identity, block.id)),
                )
                .is_some()
            {
                return Err(format!(
                    "duplicate SSA block {} in {}",
                    block.id, function.identity
                ));
            }
            for value in &block.params {
                if definitions.insert(value.id.clone(), value.ty).is_some() {
                    return Err(format!(
                        "duplicate SSA value {} in {}",
                        value.id, function.identity
                    ));
                }
            }
            for instruction in &block.instructions {
                let dest = match instruction {
                    NativeSsaScalarInstruction::Constant { dest, .. }
                    | NativeSsaScalarInstruction::Call { dest, .. }
                    | NativeSsaScalarInstruction::Integer { dest, .. } => dest,
                };
                if definitions.insert(dest.id.clone(), dest.ty).is_some() {
                    return Err(format!(
                        "duplicate SSA value {} in {}",
                        dest.id, function.identity
                    ));
                }
            }
        }
        let type_of = |id: &str| {
            definitions
                .get(id)
                .copied()
                .ok_or_else(|| format!("undefined SSA value {id} in {}", function.identity))
        };

        let mut scalar_blocks = Vec::new();
        for block in &function.blocks {
            let mut instructions = Vec::new();
            for instruction in &block.instructions {
                match instruction {
                    NativeSsaScalarInstruction::Constant { dest, value } => {
                        if dest.ty == NativeSsaScalarType::Bool && *value > 1 {
                            return Err("native C11 boolean constant must be zero or one".into());
                        }
                        instructions.push(ScalarInst::Const {
                            dest: ScalarValue {
                                id: value_id(&dest.id),
                                ty: scalar_ty(dest.ty),
                            },
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
                            dest: ScalarValue {
                                id: value_id(&dest.id),
                                ty: scalar_ty(dest.ty),
                            },
                            callee: symbol.clone(),
                            args: args.iter().map(|id| value_id(id)).collect(),
                        });
                    }
                    NativeSsaScalarInstruction::Integer {
                        dest,
                        operator,
                        lhs,
                        rhs,
                    } => {
                        if dest.ty != NativeSsaScalarType::U64
                            || type_of(lhs)? != NativeSsaScalarType::U64
                            || type_of(rhs)? != NativeSsaScalarType::U64
                        {
                            return Err(
                                "native C11 integer operation is outside the u64 envelope".into()
                            );
                        }
                        if operator != "add" {
                            return Err(format!(
                                "native C11 integer operator {operator:?} is unsupported"
                            ));
                        }
                        instructions.push(ScalarInst::Integer {
                            dest: ScalarValue {
                                id: value_id(&dest.id),
                                ty: scalar_ty(dest.ty),
                            },
                            operator: operator.clone(),
                            intent: ArithmeticIntent::Wrapping,
                            lhs: value_id(lhs),
                            rhs: value_id(rhs),
                            promise: Box::new(crate::promises::withheld(
                                BackendPromise::WrappingArithmetic,
                                value_id(&dest.id),
                                "structural input carries no overflow proof",
                            )),
                        });
                    }
                }
            }
            let check_edge =
                |target: &u32, args: &[String]| -> Result<(SemanticId, Vec<SemanticId>), String> {
                    let Some(target_id) = block_ids.get(target) else {
                        return Err(format!("SSA edge targets missing block {target}"));
                    };
                    let target_block = function
                        .blocks
                        .iter()
                        .find(|candidate| candidate.id == *target)
                        .unwrap();
                    if args.len() != target_block.params.len() {
                        return Err(format!("SSA edge arity mismatch at block {target}"));
                    }
                    for (argument, parameter) in args.iter().zip(&target_block.params) {
                        if type_of(argument)? != parameter.ty {
                            return Err(format!("SSA edge type mismatch at block {target}"));
                        }
                    }
                    Ok((
                        target_id.clone(),
                        args.iter().map(|id| value_id(id)).collect(),
                    ))
                };
            let terminator = match &block.terminator {
                NativeSsaScalarTerminator::Return { value } => {
                    if type_of(value)? != result_type {
                        return Err(format!("return type mismatch in {}", function.identity));
                    }
                    ScalarTerm::Return {
                        value: value_id(value),
                    }
                }
                NativeSsaScalarTerminator::Jump { target, args } => {
                    let (target, args) = check_edge(target, args)?;
                    ScalarTerm::Jump { target, args }
                }
                NativeSsaScalarTerminator::Branch {
                    condition,
                    then_target,
                    then_args,
                    else_target,
                    else_args,
                } => {
                    if type_of(condition)? != NativeSsaScalarType::Bool {
                        return Err(format!(
                            "branch condition must be bool in {}",
                            function.identity
                        ));
                    }
                    let (then_target, then_args) = check_edge(then_target, then_args)?;
                    let (else_target, else_args) = check_edge(else_target, else_args)?;
                    ScalarTerm::Branch {
                        cond: value_id(condition),
                        then_target,
                        then_args,
                        else_target,
                        else_args,
                    }
                }
            };
            scalar_blocks.push(ScalarBlock {
                id: block_ids[&block.id].clone(),
                params: block
                    .params
                    .iter()
                    .map(|value| ScalarValue {
                        id: value_id(&value.id),
                        ty: scalar_ty(value.ty),
                    })
                    .collect(),
                insts: instructions,
                term: terminator,
            });
        }
        scalar_functions.push(ScalarFunction {
            export_name,
            params: function
                .params
                .iter()
                .map(|value| ScalarValue {
                    id: value_id(&value.id),
                    ty: scalar_ty(value.ty),
                })
                .collect(),
            result: ScalarValue {
                id: SemanticId(format!("{}#result", function.identity)),
                ty: scalar_ty(result_type),
            },
            blocks: scalar_blocks,
            promises: Vec::new(),
            promise_decisions: Vec::new(),
        });
    }

    let identities: BTreeSet<_> = signatures.keys().collect();
    for function in &native.functions {
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            if let NativeSsaScalarInstruction::Call { callee, .. } = instruction {
                if !identities.contains(callee) {
                    return Err(format!(
                        "call target {callee} is outside the verified module"
                    ));
                }
            }
        }
    }
    let source = emit_module(&ScalarModule {
        functions: scalar_functions,
        unsupported: Vec::new(),
        features: vec!["structurally-checked-native-scalar-ssa".into()],
        promise_decisions: Vec::new(),
    });
    Ok(format!(
        "/* Structural C11 lowering only: no compiler proof or source binding is carried by this input. */\n{source}"
    ))
}

fn native_function_symbol(identity: &str) -> Result<String, String> {
    let body = identity
        .strip_prefix("mncs:0.2:function:")
        .ok_or_else(|| format!("invalid canonical function identity {identity}"))?;
    let (module, function) = body
        .rsplit_once("::")
        .filter(|(module, function)| !module.is_empty() && !function.is_empty())
        .ok_or_else(|| format!("invalid canonical function identity {identity}"))?;
    Ok(crate::support::qualified_c_symbol(module, function))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn structural_module() -> NativeSsaScalarModule {
        NativeSsaScalarModule {
            schema_version: "mncs.native-scalar-ssa-structural/1".into(),
            functions: vec![
                NativeSsaScalarFunction {
                    identity: "mncs:0.2:function:dep::echo".into(),
                    params: vec![NativeSsaScalarValue {
                        id: "0".into(),
                        ty: NativeSsaScalarType::U64,
                    }],
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
                        NativeSsaScalarValue {
                            id: "0".into(),
                            ty: NativeSsaScalarType::Bool,
                        },
                        NativeSsaScalarValue {
                            id: "1".into(),
                            ty: NativeSsaScalarType::U64,
                        },
                    ],
                    result_type: NativeSsaScalarType::U64,
                    blocks: vec![
                        NativeSsaScalarBlock {
                            id: 0,
                            params: vec![],
                            instructions: vec![
                                NativeSsaScalarInstruction::Call {
                                    dest: NativeSsaScalarValue {
                                        id: "2".into(),
                                        ty: NativeSsaScalarType::U64,
                                    },
                                    callee: "mncs:0.2:function:dep::echo".into(),
                                    args: vec!["1".into()],
                                },
                                NativeSsaScalarInstruction::Constant {
                                    dest: NativeSsaScalarValue {
                                        id: "6".into(),
                                        ty: NativeSsaScalarType::U64,
                                    },
                                    value: 1,
                                },
                                NativeSsaScalarInstruction::Integer {
                                    dest: NativeSsaScalarValue {
                                        id: "7".into(),
                                        ty: NativeSsaScalarType::U64,
                                    },
                                    operator: "add".into(),
                                    lhs: "2".into(),
                                    rhs: "6".into(),
                                },
                            ],
                            terminator: NativeSsaScalarTerminator::Branch {
                                condition: "0".into(),
                                then_target: 1,
                                then_args: vec!["7".into()],
                                else_target: 2,
                                else_args: vec!["7".into()],
                            },
                        },
                        NativeSsaScalarBlock {
                            id: 1,
                            params: vec![NativeSsaScalarValue {
                                id: "3".into(),
                                ty: NativeSsaScalarType::U64,
                            }],
                            instructions: vec![],
                            terminator: NativeSsaScalarTerminator::Jump {
                                target: 3,
                                args: vec!["3".into()],
                            },
                        },
                        NativeSsaScalarBlock {
                            id: 2,
                            params: vec![NativeSsaScalarValue {
                                id: "4".into(),
                                ty: NativeSsaScalarType::U64,
                            }],
                            instructions: vec![],
                            terminator: NativeSsaScalarTerminator::Jump {
                                target: 3,
                                args: vec!["4".into()],
                            },
                        },
                        NativeSsaScalarBlock {
                            id: 3,
                            params: vec![NativeSsaScalarValue {
                                id: "5".into(),
                                ty: NativeSsaScalarType::U64,
                            }],
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
        let source = emit_structurally_checked_native_ssa_c11(&structural_module()).unwrap();
        assert!(source.starts_with("/* Structural C11 lowering only:"));
        assert!(source.contains("mncs_dep__echo"));
        assert!(source.contains("mncs_app__run"));

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "mncs-native-ssa-c11-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let module_path = dir.join("module.c");
        let driver_path = dir.join("driver.c");
        let executable_path = dir.join("run");
        std::fs::write(&module_path, source).unwrap();
        std::fs::write(
            &driver_path,
            r#"#include <stdint.h>
extern void mncs_app__run(uint8_t, uint64_t, int32_t *, int64_t *, uint64_t);
int main(void) {
  int32_t status = 0;
  int64_t value = 0;
  mncs_app__run(0, 37, &status, &value, 64);
  if (status || value != 38) return 1;
  mncs_app__run(1, 81, &status, &value, 64);
  if (status || value != 82) return 2;
  return 0;
}
"#,
        )
        .unwrap();

        let compiler = if std::process::Command::new("clang")
            .arg("--version")
            .output()
            .is_ok()
        {
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
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = std::process::Command::new(&executable_path)
            .status()
            .unwrap();
        assert!(executed.success());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn native_ssa_c11_refuses_authority_claims_and_malformed_structure() {
        let claims_authority = serde_json::json!({
            "schema_version": "mncs.native-scalar-ssa-structural/1",
            "verification_status": "pass",
            "verifier": "mncs.compiler.ssa.verify",
            "functions": []
        });
        assert!(serde_json::from_value::<NativeSsaScalarModule>(claims_authority).is_err());

        let mut module = structural_module();
        module.schema_version = "mncs.native-scalar-ssa/1".into();
        assert!(emit_structurally_checked_native_ssa_c11(&module).is_err());

        let mut module = structural_module();
        module.functions[1].blocks[0].terminator = NativeSsaScalarTerminator::Jump {
            target: 999,
            args: vec![],
        };
        assert!(emit_structurally_checked_native_ssa_c11(&module).is_err());

        let mut module = structural_module();
        let NativeSsaScalarInstruction::Integer { operator, .. } =
            &mut module.functions[1].blocks[0].instructions[2]
        else {
            panic!("fixture has one scalar integer operation")
        };
        *operator = "div".into();
        assert!(emit_structurally_checked_native_ssa_c11(&module).is_err());

        let mut module = structural_module();
        module.functions[1].blocks[0].instructions.push(
            NativeSsaScalarInstruction::Constant {
                dest: NativeSsaScalarValue {
                    id: "8".into(),
                    ty: NativeSsaScalarType::Bool,
                },
                value: 2,
            },
        );
        assert!(emit_structurally_checked_native_ssa_c11(&module).is_err());
    }
}
