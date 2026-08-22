//! Independent translation validators.
//!
//! The `generate` module produces candidate SSA. It is not a validator and
//! must not be treated as evidence of validity. The `validate` functions do
//! not call `generate`; they re-derive facts from the subject artifacts.
//!
//! Residual trust: validators share `mncs-model` integer evaluation and
//! `mncs-codegen` research execution with the compiler family. They do not
//! share transformation algorithms with the generators.

pub mod generate;

use std::collections::{BTreeMap, BTreeSet};

use mncs_codegen::{compare_body_ssa_and_backend, selected_ssa_ref, LayeredExecutionStatus};
use mncs_model::{
    execute_ssa_module, ArtifactRepresentation, BackendArtifact, CompilerArtifactRef,
    EvidenceFreshness, ExecutionCorpus, ExecutionStatus, IntegerOperation, Program, RelationClaim,
    SemanticId, SsaInstructionKind, SsaModule, SsaTerminator, TranslationCounterexample,
    TranslationJudgement, TranslationValidationResult, VerifierIdentity, VerifierIndependence,
    SSA_SCHEMA_VERSION,
};

pub const VALIDATOR_VERSION: &str = "0.1";
pub const RESIDUAL_TRUST: &str = "shares mncs-model integer evaluation and mncs-codegen research execution with the compiler; does not share transformation algorithms with candidate generators";

pub fn validator(name: &str) -> VerifierIdentity {
    VerifierIdentity {
        identity: SemanticId(format!("mncs:validator:{name}:{VALIDATOR_VERSION}")),
        name: name.to_owned(),
        version: VALIDATOR_VERSION.to_owned(),
        independence: VerifierIndependence::IndependentImplementation,
    }
}

pub fn observational_relation() -> RelationClaim {
    RelationClaim {
        kind: "observational-equivalence".to_owned(),
        observation_model: mncs_model::ObservationModelRef::new(
            "mncs-public-behavior",
            "0.1",
            ["return_value".to_owned(), "declared_failure".to_owned()]
                .into_iter()
                .collect(),
        ),
    }
}

pub fn ssa_ref(module: &SsaModule) -> CompilerArtifactRef {
    CompilerArtifactRef::new(
        ArtifactRepresentation::Ssa,
        SSA_SCHEMA_VERSION,
        module.fingerprint().expect("SSA is serializable"),
    )
}

pub fn validate_constant_folding(
    before: &SsaModule,
    after: &SsaModule,
) -> TranslationValidationResult {
    let mut counterexample = None;
    let mut judgement = TranslationJudgement::Pass;
    let mut limitations = Vec::new();
    for function in &before.functions {
        let after_function = after
            .functions
            .iter()
            .find(|candidate| candidate.semantic_identity == function.semantic_identity);
        let Some(after_function) = after_function else {
            judgement = TranslationJudgement::Unknown;
            limitations.push("candidate dropped a function the validator cannot match".to_owned());
            continue;
        };
        let constants = collect_constants(function);
        for block in &function.blocks {
            for instruction in &block.instructions {
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
                let Some(expected) = evaluation.value else {
                    if after_has_constant(
                        after_function,
                        instruction.semantic_identity.as_ref(),
                        None,
                    ) {
                        judgement = TranslationJudgement::Fail;
                        counterexample = Some(TranslationCounterexample {
                            case_id: instruction.identity.0.clone(),
                            reason: "constant folding produced a value for an overflowing checked operation".to_owned(),
                            arguments: Vec::new(),
                            expected: None,
                            observed: None,
                        });
                    } else {
                        limitations.push(
                            "overflowing checked operation cannot be folded; UNKNOWN does not authorize folding".to_owned(),
                        );
                        if judgement == TranslationJudgement::Pass {
                            judgement = TranslationJudgement::Unknown;
                        }
                    }
                    continue;
                };
                match after_constant_value(after_function, instruction.semantic_identity.as_ref()) {
                    Some(observed) if observed == expected => {}
                    Some(observed) => {
                        judgement = TranslationJudgement::Fail;
                        counterexample = Some(TranslationCounterexample {
                            case_id: instruction.identity.0.clone(),
                            reason: format!(
                                "folded constant {observed} does not match exact overflow semantics ({expected})"
                            ),
                            arguments: Vec::new(),
                            expected: None,
                            observed: None,
                        });
                    }
                    None => {
                        limitations.push(
                            "foldable constant operation was not folded; that is not a FAIL"
                                .to_owned(),
                        );
                    }
                }
            }
        }
    }
    result(
        "constant-folding-exact-overflow",
        "constant-folding",
        before,
        after,
        judgement,
        counterexample,
        limitations,
    )
}

pub fn validate_unreachable_removal(
    before: &SsaModule,
    after: &SsaModule,
) -> TranslationValidationResult {
    let mut judgement = TranslationJudgement::Pass;
    let mut counterexample = None;
    let mut limitations = Vec::new();
    for function in &before.functions {
        let Some(after_function) = after
            .functions
            .iter()
            .find(|candidate| candidate.semantic_identity == function.semantic_identity)
        else {
            judgement = TranslationJudgement::Unknown;
            continue;
        };
        let reachable = reachable_blocks(function);
        let after_ids = after_function
            .blocks
            .iter()
            .map(|block| block.identity.clone())
            .collect::<BTreeSet<_>>();
        for block in &function.blocks {
            if reachable.contains(&block.identity) && !after_ids.contains(&block.identity) {
                judgement = TranslationJudgement::Fail;
                counterexample = Some(TranslationCounterexample {
                    case_id: block.identity.0.clone(),
                    reason: "candidate removed a reachable block".to_owned(),
                    arguments: Vec::new(),
                    expected: None,
                    observed: None,
                });
            }
        }
        for block in &after_function.blocks {
            if !function
                .blocks
                .iter()
                .any(|item| item.identity == block.identity)
            {
                judgement = TranslationJudgement::Unknown;
                limitations
                    .push("candidate introduced a block the validator cannot relate".to_owned());
            }
        }
    }
    result(
        "unreachable-block-removal",
        "unreachable-removal",
        before,
        after,
        judgement,
        counterexample,
        limitations,
    )
}

pub fn validate_branch_simplification(
    before: &SsaModule,
    after: &SsaModule,
) -> TranslationValidationResult {
    let mut judgement = TranslationJudgement::Pass;
    let mut counterexample = None;
    let mut limitations = Vec::new();
    for function in &before.functions {
        let constants = collect_constants(function);
        let Some(after_function) = after
            .functions
            .iter()
            .find(|candidate| candidate.semantic_identity == function.semantic_identity)
        else {
            judgement = TranslationJudgement::Unknown;
            continue;
        };
        for (index, block) in function.blocks.iter().enumerate() {
            let SsaTerminator::ConditionalBranch {
                condition,
                then_target,
                else_target,
                ..
            } = &block.terminator
            else {
                continue;
            };
            let Some(value) = constants.get(condition).copied() else {
                continue;
            };
            let expected = if value != 0 { then_target } else { else_target };
            match after_function
                .blocks
                .get(index)
                .map(|block| &block.terminator)
            {
                Some(SsaTerminator::Branch { target, .. }) if target == expected => {}
                Some(SsaTerminator::ConditionalBranch { .. }) => {
                    limitations.push(
                        "constant condition was not simplified; that is not a FAIL".to_owned(),
                    );
                }
                Some(_) => {
                    judgement = TranslationJudgement::Fail;
                    counterexample = Some(TranslationCounterexample {
                        case_id: block.identity.0.clone(),
                        reason: "constant branch was rewritten to the wrong successor".to_owned(),
                        arguments: Vec::new(),
                        expected: None,
                        observed: None,
                    });
                }
                None => {
                    judgement = TranslationJudgement::Unknown;
                    limitations.push("candidate block alignment is unknown".to_owned());
                }
            }
        }
    }
    result(
        "constant-branch-simplification",
        "branch-simplification",
        before,
        after,
        judgement,
        counterexample,
        limitations,
    )
}

pub fn validate_checked_elision(
    program: &Program,
    before: &SsaModule,
    after: &SsaModule,
    corpus: &ExecutionCorpus,
) -> TranslationValidationResult {
    let mut changed = false;
    for (before_fn, after_fn) in before.functions.iter().zip(&after.functions) {
        for (before_block, after_block) in before_fn.blocks.iter().zip(&after_fn.blocks) {
            for (before_inst, after_inst) in before_block
                .instructions
                .iter()
                .zip(&after_block.instructions)
            {
                if let (
                    SsaInstructionKind::Integer {
                        intent: mncs_model::ArithmeticIntent::Checked,
                        ..
                    },
                    SsaInstructionKind::Integer {
                        intent: mncs_model::ArithmeticIntent::Wrapping,
                        ..
                    },
                ) = (&before_inst.kind, &after_inst.kind)
                {
                    changed = true;
                }
            }
        }
    }
    if !changed {
        return result(
            "checked-to-wrapping-elision",
            "checked-elision",
            before,
            after,
            TranslationJudgement::Unknown,
            None,
            vec!["no checked-to-wrapping rewrite was observed".to_owned()],
        );
    }
    for case_ in &corpus.cases {
        let checked = execute_ssa_module(program, before, &case_.request);
        let wrapping = execute_ssa_module(program, after, &case_.request);
        if !statuses_agree(&checked, wrapping.status, &wrapping.returned) {
            let counterexample = Some(TranslationCounterexample {
                case_id: case_.id.clone(),
                reason: "checked and wrapping observations disagree over the declared corpus"
                    .to_owned(),
                arguments: case_.request.arguments.clone(),
                expected: Some(checked.returned),
                observed: Some(wrapping.returned),
            });
            return result(
                "checked-to-wrapping-elision",
                "checked-elision",
                before,
                after,
                TranslationJudgement::Fail,
                counterexample,
                Vec::new(),
            );
        }
    }
    result(
        "checked-to-wrapping-elision",
        "checked-elision",
        before,
        after,
        TranslationJudgement::Unknown,
        None,
        vec![
            "corpus agreement is empirical bounded agreement, not a no-overflow proof".to_owned(),
            "UNKNOWN does not authorize dropping checked overflow".to_owned(),
        ],
    )
}

pub fn validate_backend_lowering(
    program: &Program,
    ssa: &SsaModule,
    artifact: &BackendArtifact,
    corpus: &ExecutionCorpus,
) -> TranslationValidationResult {
    let comparison = compare_body_ssa_and_backend(program, ssa, artifact, corpus);
    let (judgement, counterexample, limitations) = match comparison.status {
        LayeredExecutionStatus::ConsistentOverCorpus => (
            TranslationJudgement::Pass,
            None,
            vec![
                "PASS is empirical bounded agreement over the declared corpus".to_owned(),
                "this is not a proof of universal semantic equivalence".to_owned(),
            ],
        ),
        LayeredExecutionStatus::MismatchDetected => (
            TranslationJudgement::Fail,
            comparison
                .mismatches
                .first()
                .map(|mismatch| TranslationCounterexample {
                    case_id: mismatch.case_id.clone(),
                    reason: mismatch.reason.clone(),
                    arguments: Vec::new(),
                    expected: None,
                    observed: None,
                }),
            Vec::new(),
        ),
        LayeredExecutionStatus::Unsupported => (
            TranslationJudgement::Unknown,
            None,
            vec!["backend or reference execution is unsupported on this corpus".to_owned()],
        ),
        LayeredExecutionStatus::InvalidInput => (
            TranslationJudgement::Unknown,
            None,
            vec!["corpus or artifact is invalid".to_owned()],
        ),
    };
    TranslationValidationResult::new(
        validator("backend-lowering-bounded-agreement"),
        selected_ssa_ref(ssa),
        CompilerArtifactRef::new(
            ArtifactRepresentation::BackendArtifact,
            artifact.schema_version.clone(),
            artifact.bytes_sha256.clone(),
        ),
        observational_relation(),
        format!("backend-realization:{}", artifact.backend.name),
        judgement,
        vec![mncs_model::LAYERED_EXECUTION_COMPARISON_INTERPRETATION.to_owned()],
        artifact.evidence_dependencies.clone(),
        BTreeMap::new(),
        counterexample,
        limitations,
        RESIDUAL_TRUST,
        EvidenceFreshness::Current,
    )
}

pub fn validate_stale_dependency(
    mut result: TranslationValidationResult,
) -> TranslationValidationResult {
    result.freshness = EvidenceFreshness::Stale;
    result
        .limitations
        .push("a relevant artifact, backend, or validator dependency changed".to_owned());
    result.seal();
    result
}

fn statuses_agree(
    left: &mncs_model::SsaExecutionResult,
    right_status: ExecutionStatus,
    right_returned: &[mncs_model::ExecutionValue],
) -> bool {
    left.status == right_status
        && match left.status {
            ExecutionStatus::Returned => left.returned == right_returned,
            ExecutionStatus::RuntimeFailure | ExecutionStatus::BudgetExhausted => true,
            ExecutionStatus::Unsupported | ExecutionStatus::InvalidRequest => false,
        }
}

fn result(
    validator_name: &str,
    kind: &str,
    before: &SsaModule,
    after: &SsaModule,
    judgement: TranslationJudgement,
    counterexample: Option<TranslationCounterexample>,
    limitations: Vec<String>,
) -> TranslationValidationResult {
    TranslationValidationResult::new(
        validator(validator_name),
        ssa_ref(before),
        ssa_ref(after),
        observational_relation(),
        kind,
        judgement,
        Vec::new(),
        vec![before.identity.clone(), after.identity.clone()],
        BTreeMap::from([
            (
                before.identity.clone(),
                before.fingerprint().unwrap_or_default(),
            ),
            (
                after.identity.clone(),
                after.fingerprint().unwrap_or_default(),
            ),
        ]),
        counterexample,
        limitations,
        RESIDUAL_TRUST,
        EvidenceFreshness::Current,
    )
}

fn collect_constants(function: &mncs_model::SsaFunction) -> BTreeMap<SemanticId, i128> {
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

fn after_constant_value(
    function: &mncs_model::SsaFunction,
    semantic: Option<&SemanticId>,
) -> Option<i128> {
    for block in &function.blocks {
        for instruction in &block.instructions {
            if instruction.semantic_identity.as_ref() == semantic {
                if let SsaInstructionKind::Constant { value, .. } = &instruction.kind {
                    return Some(*value);
                }
            }
        }
    }
    None
}

fn after_has_constant(
    function: &mncs_model::SsaFunction,
    semantic: Option<&SemanticId>,
    _unused: Option<()>,
) -> bool {
    after_constant_value(function, semantic).is_some()
}

fn reachable_blocks(function: &mncs_model::SsaFunction) -> BTreeSet<SemanticId> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::{
        fold_constants, remove_unreachable, rewrite_checked_to_wrapping, simplify_constant_branches,
    };
    use mncs_codegen::{lower_selected_ssa, portable_wasm_plan, selected_ssa_ref};
    use mncs_model::{
        ExecutionCase, ExecutionPolicy, ExecutionRequest, ExecutionTarget, ExecutionValue,
        IntegerType,
    };

    fn program() -> Program {
        Program::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/executable/checked-add.mncs.json"
        )))
        .unwrap()
    }

    fn corpus() -> ExecutionCorpus {
        ExecutionCorpus {
            schema_version: mncs_model::EXECUTION_CORPUS_SCHEMA_VERSION.to_owned(),
            name: "checked-add".to_owned(),
            properties: Vec::new(),
            cases: vec![
                ExecutionCase {
                    id: "sum".to_owned(),
                    request: request(20, 22),
                    expected: None,
                },
                ExecutionCase {
                    id: "overflow".to_owned(),
                    request: request(i128::from(i32::MAX), 1),
                    expected: None,
                },
            ],
        }
    }

    fn request(a: i128, b: i128) -> ExecutionRequest {
        ExecutionRequest {
            schema_version: mncs_model::EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
            target: ExecutionTarget {
                module: "Examples.Executable".to_owned(),
                function: "checked_add".to_owned(),
            },
            arguments: vec![
                ExecutionValue::Integer {
                    value: a,
                    ty: IntegerType {
                        bits: 32,
                        signed: true,
                    },
                },
                ExecutionValue::Integer {
                    value: b,
                    ty: IntegerType {
                        bits: 32,
                        signed: true,
                    },
                },
            ],
            step_budget: 64,
            policy: ExecutionPolicy::default(),
        }
    }

    #[test]
    fn constant_folding_pass_fail_and_unknown_are_distinct() {
        let program = Program::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/executable/const-add.mncs.json"
        )))
        .unwrap();
        let before = program.lower_to_ssa().unwrap();
        let folded = fold_constants(&before);
        let pass = validate_constant_folding(&before, &folded);
        assert!(matches!(
            pass.judgement,
            TranslationJudgement::Pass | TranslationJudgement::Unknown
        ));
        let mut invalid = folded.clone();
        for function in &mut invalid.functions {
            for block in &mut function.blocks {
                for instruction in &mut block.instructions {
                    if let SsaInstructionKind::Constant { value, .. } = &mut instruction.kind {
                        *value = value.saturating_add(1);
                    }
                }
            }
        }
        let fail = validate_constant_folding(&before, &invalid);
        if folded.fingerprint().ok() != before.fingerprint().ok() {
            assert_eq!(fail.judgement, TranslationJudgement::Fail);
            assert!(fail.counterexample.is_some());
        }
        assert!(!fail.authorizes_promotion() || fail.judgement != TranslationJudgement::Fail);
        let unknown = validate_constant_folding(&before, &before);
        assert!(!unknown.authorizes_promotion() || unknown.judgement == TranslationJudgement::Pass);
    }

    #[test]
    fn unreachable_removal_fails_when_a_live_block_disappears() {
        let program = program();
        let before = program.lower_to_ssa().unwrap();
        let after = remove_unreachable(&before);
        let pass = validate_unreachable_removal(&before, &after);
        assert_ne!(pass.judgement, TranslationJudgement::Fail);
        let mut invalid = after;
        if let Some(function) = invalid.functions.first_mut() {
            function.blocks.clear();
        }
        let fail = validate_unreachable_removal(&before, &invalid);
        assert_eq!(fail.judgement, TranslationJudgement::Fail);
    }

    #[test]
    fn checked_elision_fails_on_overflow_and_does_not_promote_unknown() {
        let program = program();
        let before = program.lower_to_ssa().unwrap();
        let after = rewrite_checked_to_wrapping(&before);
        let fail = validate_checked_elision(&program, &before, &after, &corpus());
        assert_eq!(fail.judgement, TranslationJudgement::Fail);
        assert!(fail.counterexample.is_some());
        assert!(!fail.authorizes_promotion());
        let no_overflow = ExecutionCorpus {
            schema_version: mncs_model::EXECUTION_CORPUS_SCHEMA_VERSION.to_owned(),
            name: "no-overflow".to_owned(),
            properties: Vec::new(),
            cases: vec![ExecutionCase {
                id: "small".to_owned(),
                request: request(1, 2),
                expected: None,
            }],
        };
        let unknown = validate_checked_elision(&program, &before, &after, &no_overflow);
        assert_eq!(unknown.judgement, TranslationJudgement::Unknown);
        assert!(!unknown.authorizes_promotion());
    }

    #[test]
    fn backend_lowering_pass_is_empirical_bounded_agreement() {
        let program = program();
        let ssa = program.lower_to_ssa().unwrap();
        let selected = selected_ssa_ref(&ssa);
        let artifact = lower_selected_ssa(
            &program,
            &ssa,
            selected.clone(),
            &portable_wasm_plan(selected),
        )
        .artifact
        .unwrap();
        let result = validate_backend_lowering(&program, &ssa, &artifact, &corpus());
        assert_eq!(result.judgement, TranslationJudgement::Pass);
        assert!(result
            .limitations
            .iter()
            .any(|item| item.contains("empirical bounded agreement")));
        let stale = validate_stale_dependency(result);
        assert_eq!(stale.freshness, EvidenceFreshness::Stale);
        assert!(!stale.authorizes_promotion());
    }

    #[test]
    fn generator_is_not_a_validator() {
        let program = program();
        let before = program.lower_to_ssa().unwrap();
        let generated = rewrite_checked_to_wrapping(&before);
        assert_ne!(
            generated.fingerprint().unwrap(),
            before.fingerprint().unwrap()
        );
        let validation = validate_checked_elision(&program, &before, &generated, &corpus());
        assert_eq!(validation.judgement, TranslationJudgement::Fail);
    }

    #[test]
    fn branch_simplification_unknown_is_not_pass_when_unsupported() {
        let program = program();
        let before = program.lower_to_ssa().unwrap();
        let after = simplify_constant_branches(&before);
        let result = validate_branch_simplification(&before, &after);
        if result.judgement == TranslationJudgement::Unknown {
            assert!(!result.authorizes_promotion());
        }
    }
}
