//! Untrusted SSA candidate generators.
//!
//! These functions search for rewrites. They must never be treated as
//! validators. A generator result is a candidate artifact, not evidence.

use std::collections::{BTreeMap, BTreeSet};

use mncs_model::{
    ArithmeticIntent, IntegerOperation, SemanticId, SsaFunction, SsaInstructionKind, SsaModule,
    SsaTerminator,
};
use sha2::{Digest, Sha256};

pub fn fold_constants(module: &SsaModule) -> SsaModule {
    let mut transformed = module.clone();
    for function in &mut transformed.functions {
        let constants = collect_constants(function);
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                let SsaInstructionKind::Integer {
                    operator,
                    operand_type,
                    intent,
                } = &instruction.kind
                else {
                    continue;
                };
                if instruction.inputs.len() != 2 {
                    continue;
                }
                let Some(left) = constants.get(&instruction.inputs[0]) else {
                    continue;
                };
                let Some(right) = constants.get(&instruction.inputs[1]) else {
                    continue;
                };
                let evaluation = IntegerOperation {
                    operator: operator.clone(),
                    operand_type: *operand_type,
                    left: *left,
                    right: *right,
                    intent: *intent,
                }
                .evaluate();
                if let Some(value) = evaluation.value {
                    if let Some(output) = instruction.outputs.first() {
                        instruction.kind = SsaInstructionKind::Constant {
                            value,
                            ty: output.ty.clone(),
                        };
                        instruction.inputs.clear();
                    }
                }
            }
        }
    }
    retarget(module, transformed, "constant-fold")
}

pub fn remove_unreachable(module: &SsaModule) -> SsaModule {
    let mut transformed = module.clone();
    for function in &mut transformed.functions {
        let reachable = reachable(function);
        function
            .blocks
            .retain(|block| reachable.contains(&block.identity));
    }
    retarget(module, transformed, "unreachable-remove")
}

pub fn simplify_constant_branches(module: &SsaModule) -> SsaModule {
    let mut transformed = module.clone();
    for function in &mut transformed.functions {
        let constants = collect_constants(function);
        for block in &mut function.blocks {
            if let SsaTerminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } = &block.terminator
            {
                if let Some(value) = constants.get(condition) {
                    block.terminator = if *value != 0 {
                        SsaTerminator::Branch {
                            target: then_target.clone(),
                            arguments: then_arguments.clone(),
                        }
                    } else {
                        SsaTerminator::Branch {
                            target: else_target.clone(),
                            arguments: else_arguments.clone(),
                        }
                    };
                }
            }
        }
    }
    retarget(module, transformed, "branch-simplify")
}

pub fn rewrite_checked_to_wrapping(module: &SsaModule) -> SsaModule {
    let mut transformed = module.clone();
    for function in &mut transformed.functions {
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                if let SsaInstructionKind::Integer { intent, .. } = &mut instruction.kind {
                    if *intent == ArithmeticIntent::Checked {
                        *intent = ArithmeticIntent::Wrapping;
                    }
                }
            }
        }
    }
    retarget(module, transformed, "checked-to-wrapping")
}

fn retarget(before: &SsaModule, mut after: SsaModule, kind: &str) -> SsaModule {
    let fingerprint = after.fingerprint().unwrap_or_default();
    after.identity = SemanticId(format!(
        "mncs:0.4:ssa:candidate:{}:{}",
        kind,
        sha256_hex(format!("{}:{fingerprint}", before.identity.0).as_bytes())
    ));
    after
}

fn collect_constants(function: &SsaFunction) -> BTreeMap<SemanticId, i128> {
    let mut constants = BTreeMap::new();
    for block in &function.blocks {
        for instruction in &block.instructions {
            if let SsaInstructionKind::Constant { value, .. } = &instruction.kind {
                if let Some(output) = instruction.outputs.first() {
                    constants.insert(output.identity.clone(), *value);
                }
            }
        }
    }
    constants
}

fn reachable(function: &SsaFunction) -> BTreeSet<SemanticId> {
    let mut reachable = BTreeSet::new();
    let mut stack = Vec::new();
    if let Some(entry) = function.blocks.first() {
        stack.push(entry.identity.clone());
    }
    while let Some(current) = stack.pop() {
        if !reachable.insert(current.clone()) {
            continue;
        }
        let Some(block) = function
            .blocks
            .iter()
            .find(|block| block.identity == current)
        else {
            continue;
        };
        match &block.terminator {
            SsaTerminator::Branch { target, .. } => stack.push(target.clone()),
            SsaTerminator::ConditionalBranch {
                then_target,
                else_target,
                ..
            } => {
                stack.push(then_target.clone());
                stack.push(else_target.clone());
            }
            SsaTerminator::Return { .. } | SsaTerminator::Failure { .. } => {}
        }
    }
    reachable
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
