//! Orchestration for the existing MNCS semantic -> HIR -> SSA compiler spine.
//!
//! The compiler driver owns sequencing and evidence assembly. The semantic,
//! HIR, SSA, obligation, and provenance implementations remain in
//! `mncs-model`.

mod frontend;

pub use frontend::{elaborate_program, SourceFrontEndResult, SourceStudyOutput};

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use mncs_codegen::{
    configuration_for_backend, lower_with_backend, plan_for_backend, target_for_backend,
};
use mncs_model::{
    ArtifactRepresentation, BackendArtifact, BuildHostIdentity, CompilationEmissions,
    CompilationEvidenceBundle, CompilationRequest, CompilationResult, CompilationStatus,
    CompilationStudyRequest, CompilationStudyResult, CompilerArchitectureContract,
    CompilerArtifactRef, CompilerDiagnostic, CompilerDiagnosticKind, CompilerHostIdentity,
    CompilerImplementationIdentity, CompilerNodeProfile, CompilerPassExecutionObservation,
    CompilerPassIdentity, CompilerStage, CompilerStageContract, CrossHostInvariants,
    ObligationStatus, ObservationModelRef, PassPipelineIdentity, Program, RelationClaim,
    SemanticId, StageAvailability, StageIntegration, TargetContractRef, TargetLoweringPlan,
    TransformationEdge, TransformationStatus, COMPILATION_STUDY_OBSERVATION_INTERPRETATION,
    COMPILATION_STUDY_RESULT_CONTRACT_ID, COMPILER_ARTIFACT_SCHEMA_VERSION, SSA_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const REFERENCE_LANGUAGE_PROFILE: &str = "mncs:language-profile:research-0.1";
pub const REFERENCE_COMPILER_NAME: &str = "mncs-reference-compiler";
pub const REFERENCE_PIPELINE_NAME: &str = "mncs-source-semantic-hir-ssa";

#[derive(Debug, Clone)]
pub struct ReferenceCompiler {
    pub identity: CompilerImplementationIdentity,
    pub pipeline: PassPipelineIdentity,
}

impl Default for ReferenceCompiler {
    fn default() -> Self {
        Self {
            identity: reference_compiler_identity(),
            pipeline: reference_pipeline(),
        }
    }
}

impl ReferenceCompiler {
    pub fn request_for_program(
        &self,
        program: &Program,
        emit: BTreeSet<ArtifactRepresentation>,
        target: Option<TargetContractRef>,
    ) -> CompilationRequest {
        let semantic = program
            .canonical_form()
            .expect("a parsed semantic program is canonicalizable");
        let input = CompilerArtifactRef::new(
            ArtifactRepresentation::Semantic,
            semantic.schema_version,
            semantic.fingerprint,
        );
        let (compiler_host, build_host) = native_host_identities();
        let backend = target.as_ref().and_then(|target| {
            mncs_codegen::backend_names().into_iter().find_map(|name| {
                target_for_backend(name)
                    .filter(|candidate| candidate == target)
                    .and_then(|_| configuration_for_backend(name))
            })
        });
        CompilationRequest::new(
            input,
            SemanticId(REFERENCE_LANGUAGE_PROFILE.to_owned()),
            emit,
            self.identity.clone(),
            compiler_host,
            build_host,
            target,
            None,
            self.pipeline.clone(),
            Vec::new(),
            None,
            backend,
        )
    }

    pub fn request_for_program_with_backend(
        &self,
        program: &Program,
        emit: BTreeSet<ArtifactRepresentation>,
        backend_name: &str,
    ) -> Result<CompilationRequest, Box<CompilerDiagnostic>> {
        let target = target_for_backend(backend_name).ok_or_else(|| {
            Box::new(CompilerDiagnostic::new(
                "CMP006",
                CompilerDiagnosticKind::UnavailableBackendCapability,
                format!("unknown backend adapter {backend_name:?}"),
            ))
        })?;
        let semantic = program
            .canonical_form()
            .expect("a parsed semantic program is canonicalizable");
        let input = CompilerArtifactRef::new(
            ArtifactRepresentation::Semantic,
            semantic.schema_version,
            semantic.fingerprint,
        );
        let (compiler_host, build_host) = native_host_identities();
        Ok(CompilationRequest::new(
            input,
            SemanticId(REFERENCE_LANGUAGE_PROFILE.to_owned()),
            emit,
            self.identity.clone(),
            compiler_host,
            build_host,
            Some(target),
            None,
            self.pipeline.clone(),
            Vec::new(),
            None,
            configuration_for_backend(backend_name),
        ))
    }

    pub fn compile(&self, request: CompilationRequest, program: &Program) -> CompilationResult {
        let mut diagnostics = request.validate();
        if request.compiler != self.identity || request.pipeline != self.pipeline {
            diagnostics.push(CompilerDiagnostic::new(
                "CMP201",
                CompilerDiagnosticKind::InvalidRequest,
                "request compiler or pass pipeline does not match this compiler driver",
            ));
        }
        if !diagnostics.is_empty() {
            return failed_result(&request, diagnostics);
        }

        let semantic = match program.canonical_form() {
            Ok(semantic) => semantic,
            Err(error) => {
                diagnostics.push(CompilerDiagnostic::new(
                    "CMP202",
                    CompilerDiagnosticKind::InternalCompilerDefect,
                    error.to_string(),
                ));
                return failed_result(&request, diagnostics);
            }
        };
        let semantic_ref = CompilerArtifactRef::new(
            ArtifactRepresentation::Semantic,
            semantic.schema_version.clone(),
            semantic.fingerprint.clone(),
        );
        if request.input != semantic_ref {
            let mut diagnostic = CompilerDiagnostic::new(
                "CMP203",
                CompilerDiagnosticKind::InvalidRequest,
                "request input identity does not match the supplied semantic program",
            );
            diagnostic.related_artifacts =
                vec![request.input.identity.clone(), semantic_ref.identity];
            diagnostics.push(diagnostic);
            return failed_result(&request, diagnostics);
        }

        let validation = program.validate();
        if !validation.valid {
            diagnostics.extend(
                validation
                    .errors
                    .into_iter()
                    .map(|error| CompilerDiagnostic {
                        code: error.code,
                        kind: CompilerDiagnosticKind::SemanticInvalidity,
                        path: Some(error.path),
                        message: error.message,
                        related_artifacts: vec![request.input.identity.clone()],
                        obligations: Vec::new(),
                    }),
            );
            return failed_result(&request, diagnostics);
        }

        let hir = match program.lower_to_ir() {
            Ok(hir) => hir,
            Err(error) => {
                diagnostics.push(CompilerDiagnostic::new(
                    "CMP204",
                    CompilerDiagnosticKind::InternalCompilerDefect,
                    format!("validated semantic input failed HIR lowering: {error}"),
                ));
                return failed_result(&request, diagnostics);
            }
        };
        let hir_fingerprint = hir.fingerprint().expect("HIR is serializable");
        let hir_ref = CompilerArtifactRef::new(
            ArtifactRepresentation::Hir,
            hir.schema_version.clone(),
            hir_fingerprint,
        );

        let ssa = match program.lower_to_ssa() {
            Ok(ssa) => ssa,
            Err(error) => {
                diagnostics.push(CompilerDiagnostic::new(
                    "CMP205",
                    CompilerDiagnosticKind::InternalCompilerDefect,
                    format!("validated semantic input failed SSA lowering: {error}"),
                ));
                return failed_result(&request, diagnostics);
            }
        };
        let ssa_fingerprint = ssa.fingerprint().expect("SSA is serializable");
        let ssa_ref = CompilerArtifactRef::new(
            ArtifactRepresentation::Ssa,
            SSA_SCHEMA_VERSION,
            ssa_fingerprint.clone(),
        );
        let selected_ssa_ref = CompilerArtifactRef::new(
            ArtifactRepresentation::SelectedSsa,
            SSA_SCHEMA_VERSION,
            ssa_fingerprint,
        );

        let unresolved_obligations = ssa
            .obligations
            .iter()
            .filter(|obligation| obligation.status == ObligationStatus::Unknown)
            .map(|obligation| obligation.identity.clone())
            .collect::<Vec<_>>();
        let failed_obligations = ssa
            .obligations
            .iter()
            .filter(|obligation| obligation.status == ObligationStatus::Fail)
            .map(|obligation| obligation.identity.clone())
            .collect::<Vec<_>>();
        let lowering_status = if !failed_obligations.is_empty() {
            TransformationStatus::Fail
        } else if !unresolved_obligations.is_empty() {
            TransformationStatus::Unknown
        } else {
            TransformationStatus::Pass
        };

        for obligation in &unresolved_obligations {
            let mut diagnostic = CompilerDiagnostic::new(
                "CMP301",
                CompilerDiagnosticKind::UnresolvedObligation,
                "compilation retained an unresolved obligation and its conservative fallback",
            );
            diagnostic.obligations.push(obligation.clone());
            diagnostics.push(diagnostic);
        }
        for obligation in &failed_obligations {
            let mut diagnostic = CompilerDiagnostic::new(
                "CMP302",
                CompilerDiagnosticKind::FailedTransformationRelation,
                "a required compilation obligation failed",
            );
            diagnostic.obligations.push(obligation.clone());
            diagnostics.push(diagnostic);
        }

        let semantic_to_hir = TransformationEdge::new(
            semantic_ref.clone(),
            hir_ref.clone(),
            self.pass("lower-semantic-to-hir"),
            request.assumptions.clone(),
            ssa.obligations
                .iter()
                .map(|obligation| obligation.identity.clone())
                .collect(),
            Vec::new(),
            lowering_status,
            hir.transformations
                .iter()
                .map(|record| record.identity.clone())
                .collect(),
        );
        let hir_to_ssa = TransformationEdge::new(
            hir_ref.clone(),
            ssa_ref.clone(),
            self.pass("lower-hir-to-ssa"),
            Vec::new(),
            ssa.obligations
                .iter()
                .map(|obligation| obligation.identity.clone())
                .collect(),
            Vec::new(),
            lowering_status,
            ssa.transformations
                .iter()
                .map(|record| record.identity.clone())
                .collect(),
        );
        // The first selection is deliberately conservative: selected SSA is
        // byte-for-byte canonical SSA, so UNKNOWN has not authorized a rewrite.
        let select_ssa = TransformationEdge::new(
            ssa_ref.clone(),
            selected_ssa_ref.clone(),
            self.pass("select-canonical-ssa"),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            TransformationStatus::Pass,
            Vec::new(),
        );

        let mut artifacts = vec![
            semantic_ref.clone(),
            hir_ref.clone(),
            ssa_ref,
            selected_ssa_ref.clone(),
        ];
        let mut edges = vec![semantic_to_hir, hir_to_ssa, select_ssa];
        let mut backend_artifact: Option<BackendArtifact> = None;
        let target_lowering_plan = request.target.clone().map(|target| {
            let plan = if let Some(configuration) = &request.backend {
                if let Some(template) =
                    plan_for_backend(&configuration.backend.name, selected_ssa_ref.clone())
                {
                    TargetLoweringPlan::with_explicit_facts(
                        selected_ssa_ref.clone(),
                        target,
                        Some(configuration.clone()),
                        template.target_layout_assumptions,
                        template.abi_assumptions,
                        template.integer_lowering,
                        template.trap_failure_mapping,
                        template.linker_requirements,
                        template.promises_consumed,
                        TransformationStatus::Pass,
                    )
                } else {
                    TargetLoweringPlan::with_unknown_facts(
                        selected_ssa_ref.clone(),
                        target,
                        request.backend.clone(),
                        vec!["registered backend capability manifest".to_owned()],
                    )
                }
            } else {
                TargetLoweringPlan::with_unknown_facts(
                    selected_ssa_ref.clone(),
                    target,
                    request.backend.clone(),
                    vec![
                        "data-layout evidence".to_owned(),
                        "ABI and calling-convention evidence".to_owned(),
                        "integer and trap mapping evidence".to_owned(),
                    ],
                )
            };
            let plan_fingerprint = fingerprint(&plan);
            let plan_ref = CompilerArtifactRef::new(
                ArtifactRepresentation::TargetLoweringPlan,
                COMPILER_ARTIFACT_SCHEMA_VERSION,
                plan_fingerprint,
            );
            let edge = TransformationEdge::new(
                selected_ssa_ref.clone(),
                plan_ref.clone(),
                self.pass("plan-target-lowering"),
                plan.assumptions_introduced.clone(),
                Vec::new(),
                plan.target.evidence.clone(),
                plan.status,
                Vec::new(),
            );
            artifacts.push(plan_ref);
            edges.push(edge);
            if plan.status != TransformationStatus::Pass {
                diagnostics.push(CompilerDiagnostic::new(
                    "CMP303",
                    CompilerDiagnosticKind::MissingTargetEvidence,
                    "target lowering remains UNKNOWN until layout, ABI, integer, and trap facts are evidenced",
                ));
            }
            plan
        });
        if let Some(plan) = target_lowering_plan.as_ref() {
            if plan.status == TransformationStatus::Pass {
                let backend_name = plan
                    .backend
                    .as_ref()
                    .map(|configuration| configuration.backend.name.as_str())
                    .unwrap_or("");
                let lowered =
                    lower_with_backend(backend_name, program, &ssa, selected_ssa_ref.clone(), plan);
                diagnostics.extend(lowered.diagnostics.clone());
                if let (Some(artifact), Some(artifact_ref), Some(evidence)) =
                    (lowered.artifact, lowered.artifact_ref, lowered.evidence)
                {
                    let edge = TransformationEdge::new(
                        selected_ssa_ref.clone(),
                        artifact_ref.clone(),
                        self.pass("realize-selected-ssa"),
                        artifact.assumptions.clone(),
                        artifact.obligations_generated.clone(),
                        evidence.evidence.clone(),
                        lowered.status,
                        Vec::new(),
                    );
                    artifacts.push(artifact_ref);
                    edges.push(edge);
                    backend_artifact = Some(artifact);
                }
            }
        }

        let target_unresolved = match &target_lowering_plan {
            None => false,
            Some(plan)
                if plan.status == TransformationStatus::Pass && backend_artifact.is_some() =>
            {
                false
            }
            Some(_) => true,
        };
        let status = if !failed_obligations.is_empty() {
            CompilationStatus::Failed
        } else if !unresolved_obligations.is_empty() || target_unresolved {
            CompilationStatus::CompletedWithUnresolvedObligations
        } else {
            CompilationStatus::Completed
        };
        let mut assumptions = request.assumptions.clone();
        if let Some(target) = &request.target {
            assumptions.extend(target.assumptions.clone());
        }
        assumptions.sort();
        assumptions.dedup();
        let mut bundle = CompilationEvidenceBundle {
            schema_version: COMPILER_ARTIFACT_SCHEMA_VERSION.to_owned(),
            identity: SemanticId(String::new()),
            request_identity: request.identity.clone(),
            input: semantic_ref,
            emitted_artifacts: artifacts.clone(),
            transformation_edges: edges,
            compiler: request.compiler.clone(),
            pipeline: request.pipeline.clone(),
            compiler_host: request.compiler_host.clone(),
            build_host: request.build_host.clone(),
            target: request.target.clone(),
            run_environment: request.run_environment.clone(),
            target_lowering_plan: target_lowering_plan.clone(),
            backend: backend_artifact
                .as_ref()
                .map(|artifact| artifact.backend.clone())
                .or_else(|| {
                    request
                        .backend
                        .as_ref()
                        .map(|configuration| configuration.backend.clone())
                }),
            obligations: ssa.obligations.clone(),
            unresolved_obligations,
            evidence: Vec::new(),
            assumptions,
            diagnostics: diagnostics.clone(),
        };
        bundle.seal();
        let integrity = bundle.validate_integrity();
        if !integrity.is_empty() {
            diagnostics.extend(integrity);
            return failed_result(&request, diagnostics);
        }
        let evidence_ref = CompilerArtifactRef::new(
            ArtifactRepresentation::EvidenceBundle,
            COMPILER_ARTIFACT_SCHEMA_VERSION,
            fingerprint(&bundle),
        );
        artifacts.push(evidence_ref);

        let emissions = CompilationEmissions {
            semantic: request
                .emit
                .contains(&ArtifactRepresentation::Semantic)
                .then_some(semantic),
            hir: request
                .emit
                .contains(&ArtifactRepresentation::Hir)
                .then_some(hir),
            ssa: request
                .emit
                .contains(&ArtifactRepresentation::Ssa)
                .then_some(ssa),
            target_lowering_plan: request
                .emit
                .contains(&ArtifactRepresentation::TargetLoweringPlan)
                .then_some(target_lowering_plan)
                .flatten(),
            backend: request
                .emit
                .contains(&ArtifactRepresentation::BackendArtifact)
                .then_some(backend_artifact)
                .flatten(),
        };
        let evidence = request
            .emit
            .contains(&ArtifactRepresentation::EvidenceBundle)
            .then_some(bundle);
        let mut result = CompilationResult {
            schema_version: COMPILER_ARTIFACT_SCHEMA_VERSION.to_owned(),
            identity: SemanticId(String::new()),
            request_identity: request.identity.clone(),
            request,
            status,
            artifacts,
            emissions,
            evidence,
            diagnostics,
        };
        result.seal();
        result
    }

    pub fn run_study(
        &self,
        study: CompilationStudyRequest,
        program: &Program,
    ) -> CompilationStudyResult {
        let request = study.compilation_request;
        let result = self.compile(request.clone(), program);
        let mut unresolved_assumptions = request.assumptions.clone();
        if let Some(target) = &request.target {
            unresolved_assumptions.extend(target.assumptions.clone());
        }
        unresolved_assumptions.sort();
        unresolved_assumptions.dedup();
        let stage_fingerprints = result
            .artifacts
            .iter()
            .map(|artifact| (artifact.representation, artifact.fingerprint.clone()))
            .collect::<BTreeMap<_, _>>();
        let selected_ssa_identity = result
            .artifacts
            .iter()
            .find(|artifact| artifact.representation == ArtifactRepresentation::SelectedSsa)
            .map(|artifact| artifact.identity.clone());
        let semantic_fingerprint = stage_fingerprints
            .get(&ArtifactRepresentation::Semantic)
            .cloned();
        let hir_fingerprint = stage_fingerprints
            .get(&ArtifactRepresentation::Hir)
            .cloned();
        let ssa_fingerprint = stage_fingerprints
            .get(&ArtifactRepresentation::Ssa)
            .cloned();
        let pass_executions = result
            .evidence
            .as_ref()
            .map(|evidence| {
                evidence
                    .transformation_edges
                    .iter()
                    .map(CompilerPassExecutionObservation::from)
                    .collect()
            })
            .unwrap_or_default();
        let unresolved_obligations = result
            .evidence
            .as_ref()
            .map(|evidence| evidence.unresolved_obligations.clone())
            .unwrap_or_default();
        let mut study_result = CompilationStudyResult {
            schema_version: COMPILER_ARTIFACT_SCHEMA_VERSION.to_owned(),
            contract_id: COMPILATION_STUDY_RESULT_CONTRACT_ID.to_owned(),
            identity: SemanticId(String::new()),
            study_request_identity: study.identity.clone(),
            compilation_request_identity: request.identity.clone(),
            run_identity: SemanticId(format!(
                "mncs:compiler:study-run:{}",
                fingerprint(&(&study.identity, &result.identity))
            )),
            node: study.node,
            compiler_identity: request.compiler.identity,
            pipeline_identity: request.pipeline.identity,
            compiler_host_identity: request.compiler_host.identity,
            build_host_identity: request.build_host.identity,
            target_identity: request.target.map(|target| target.identity),
            backend_identity: result
                .evidence
                .as_ref()
                .and_then(|evidence| evidence.backend.as_ref())
                .map(|backend| backend.identity.clone()),
            realization_plan_identity: result
                .emissions
                .target_lowering_plan
                .as_ref()
                .map(|plan| plan.identity.clone()),
            selected_ssa_identity,
            backend_artifact_identity: result
                .emissions
                .backend
                .as_ref()
                .map(|artifact| artifact.identity.clone()),
            backend_artifact_kind: result
                .emissions
                .backend
                .as_ref()
                .map(|artifact| artifact.artifact_kind.clone()),
            compilation_status: result.status,
            stage_fingerprints,
            pass_executions,
            semantic_fingerprint,
            hir_fingerprint,
            ssa_fingerprint,
            compilation_result_identity: result.identity,
            execution_result_references: study.execution_result_references,
            diagnostics: result.diagnostics,
            unresolved_obligations,
            unresolved_assumptions,
            invariants: CrossHostInvariants {
                expected_equal: vec![
                    "compiler_identity".to_owned(),
                    "pipeline_identity".to_owned(),
                    "semantic_fingerprint".to_owned(),
                    "hir_fingerprint".to_owned(),
                    "ssa_fingerprint".to_owned(),
                ],
                expected_distinct: vec![
                    "compiler_host_identity".to_owned(),
                    "build_host_identity".to_owned(),
                    "compilation_request_identity".to_owned(),
                    "compilation_result_identity".to_owned(),
                ],
            },
            interpretation: COMPILATION_STUDY_OBSERVATION_INTERPRETATION.to_owned(),
        };
        study_result.seal();
        study_result
    }

    fn pass(&self, id: &str) -> CompilerPassIdentity {
        self.pipeline
            .passes
            .iter()
            .find(|pass| pass.id == id)
            .unwrap_or_else(|| panic!("reference pipeline is missing pass {id:?}"))
            .clone()
    }
}

pub fn compile(request: CompilationRequest, program: &Program) -> CompilationResult {
    ReferenceCompiler::default().compile(request, program)
}

pub fn reference_compiler_identity() -> CompilerImplementationIdentity {
    CompilerImplementationIdentity::new(REFERENCE_COMPILER_NAME, env!("CARGO_PKG_VERSION"), None)
}

pub fn reference_pipeline() -> PassPipelineIdentity {
    let observation = ObservationModelRef::new(
        "mncs-public-behavior",
        "0.1",
        [
            "return_value",
            "declared_failure",
            "external_effect",
            "authority_use",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    );
    let relation = || RelationClaim {
        kind: "directional_observational_refinement".to_owned(),
        observation_model: observation.clone(),
    };
    PassPipelineIdentity::new(
        REFERENCE_PIPELINE_NAME,
        "0.2",
        vec![
            CompilerPassIdentity::new(
                "lex-source",
                "0.1",
                ArtifactRepresentation::Source,
                ArtifactRepresentation::LexicalTokens,
                relation(),
                vec!["source envelope identity is valid".to_owned()],
                Vec::new(),
                Vec::new(),
                vec!["lossless token reconstruction".to_owned()],
                "token span and reconstruction checks",
                "emit structured lexical diagnostics",
            ),
            CompilerPassIdentity::new(
                "parse-source-to-cst",
                "0.1",
                ArtifactRepresentation::LexicalTokens,
                ArtifactRepresentation::ConcreteSyntaxTree,
                relation(),
                vec!["lexical stream is deterministic".to_owned()],
                Vec::new(),
                Vec::new(),
                vec!["lossless CST reconstruction".to_owned()],
                "source profile parser",
                "retain CST and emit recovery diagnostics",
            ),
            CompilerPassIdentity::new(
                "derive-ast",
                "0.1",
                ArtifactRepresentation::ConcreteSyntaxTree,
                ArtifactRepresentation::AbstractSyntaxTree,
                relation(),
                vec!["CST satisfies the 0.1 source grammar".to_owned()],
                Vec::new(),
                Vec::new(),
                vec!["AST name and span preservation".to_owned()],
                "AST projection checks",
                "emit no AST when parse errors remain",
            ),
            CompilerPassIdentity::new(
                "elaborate-ast-to-semantic",
                "0.1",
                ArtifactRepresentation::AbstractSyntaxTree,
                ArtifactRepresentation::Semantic,
                relation(),
                vec!["artifact kind is program".to_owned()],
                Vec::new(),
                Vec::new(),
                vec!["binding and output type agreement".to_owned()],
                "mncs-model semantic validation",
                "retain source diagnostics and emit no semantic artifact",
            ),
            CompilerPassIdentity::new(
                "construct-semantic-graph",
                "0.1",
                ArtifactRepresentation::Semantic,
                ArtifactRepresentation::SemanticGraph,
                relation(),
                vec!["semantic program validates".to_owned()],
                Vec::new(),
                Vec::new(),
                vec!["graph relationship integrity".to_owned()],
                "semantic graph construction",
                "emit no graph for invalid semantics",
            ),
            CompilerPassIdentity::new(
                "resolve-semantic-identities",
                "0.1",
                ArtifactRepresentation::SemanticGraph,
                ArtifactRepresentation::IdentityMap,
                relation(),
                vec!["semantic graph exists".to_owned()],
                Vec::new(),
                Vec::new(),
                vec!["content identity reproducibility".to_owned()],
                "semantic identity reproduction",
                "emit no identity map for invalid semantics",
            ),
            CompilerPassIdentity::new(
                "analyze-types-and-contracts",
                "0.1",
                ArtifactRepresentation::IdentityMap,
                ArtifactRepresentation::Validation,
                relation(),
                vec!["binding identities resolved".to_owned()],
                Vec::new(),
                Vec::new(),
                vec!["body type and contract validity".to_owned()],
                "mncs-model validation report",
                "reject invalid semantic input before HIR",
            ),
            CompilerPassIdentity::new(
                "lower-semantic-to-hir",
                "0.1",
                ArtifactRepresentation::Semantic,
                ArtifactRepresentation::Hir,
                relation(),
                vec!["semantic validation passes".to_owned()],
                Vec::new(),
                Vec::new(),
                vec!["semantic and machine-intent obligations".to_owned()],
                "mncs-model structural HIR lowering",
                "reject invalid semantic input",
            ),
            CompilerPassIdentity::new(
                "lower-hir-to-ssa",
                "0.1",
                ArtifactRepresentation::Hir,
                ArtifactRepresentation::Ssa,
                relation(),
                vec!["HIR derives from exact semantic input".to_owned()],
                Vec::new(),
                Vec::new(),
                vec!["SSA structural validity".to_owned()],
                "mncs-model SSA validator",
                "retain HIR and reject invalid SSA",
            ),
            CompilerPassIdentity::new(
                "select-canonical-ssa",
                "0.1",
                ArtifactRepresentation::Ssa,
                ArtifactRepresentation::SelectedSsa,
                relation(),
                vec!["SSA structural validity".to_owned()],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                "identity fingerprint check",
                "retain canonical SSA without optimization",
            ),
            CompilerPassIdentity::new(
                "plan-target-lowering",
                "0.1",
                ArtifactRepresentation::SelectedSsa,
                ArtifactRepresentation::TargetLoweringPlan,
                relation(),
                vec!["selected SSA is structurally valid".to_owned()],
                vec![
                    "data layout".to_owned(),
                    "ABI and calling convention".to_owned(),
                    "integer and trap mapping".to_owned(),
                ],
                Vec::new(),
                vec!["target legality".to_owned()],
                "target contract evidence appraisal",
                "report UNKNOWN and emit no backend artifact when facts are missing",
            ),
            CompilerPassIdentity::new(
                "realize-selected-ssa",
                "0.1",
                ArtifactRepresentation::SelectedSsa,
                ArtifactRepresentation::BackendArtifact,
                relation(),
                vec![
                    "selected SSA is structurally valid".to_owned(),
                    "target lowering plan is PASS with explicit facts".to_owned(),
                ],
                vec![
                    "backend-declared layout".to_owned(),
                    "backend-declared ABI".to_owned(),
                    "integer and trap mapping".to_owned(),
                ],
                Vec::new(),
                vec!["supported integer/control-flow subset".to_owned()],
                "selected mncs-codegen backend adapter",
                "emit UNSUPPORTED/UNKNOWN rather than guessing",
            ),
        ],
    )
}

pub fn reference_compiler_architecture() -> CompilerArchitectureContract {
    use CompilerStage as Stage;
    use StageAvailability as Availability;
    use StageIntegration as Integration;

    CompilerArchitectureContract::new(vec![
        CompilerStageContract {
            stage: Stage::SourceIntentRepresentation,
            availability: Availability::Experimental,
            integration: Integration::CompilerDriver,
            input_contracts: vec!["SourceEnvelope 0.1 for any declared artifact kind".to_owned()],
            output_contracts: vec!["content-addressed source artifact".to_owned()],
            deterministic_output_required: true,
            owner: "mncs-language source representation contract".to_owned(),
            decisions: vec![
                "source origin is explicit and is not assumed to be a file".to_owned(),
                "the 0.1 executable slice accepts program artifacts only".to_owned(),
            ],
            blockers: vec!["elaboration profiles for non-program artifact kinds".to_owned()],
        },
        CompilerStageContract {
            stage: Stage::LexicalAnalysis,
            availability: Availability::Experimental,
            integration: Integration::CompilerDriver,
            input_contracts: vec!["SourceEnvelope 0.1".to_owned()],
            output_contracts: vec!["lossless LexedDocument 0.1 with byte spans".to_owned()],
            deterministic_output_required: true,
            owner: "mncs-language lexical contract".to_owned(),
            decisions: vec!["trivia remains in the token stream".to_owned()],
            blockers: vec!["complete language token vocabulary".to_owned()],
        },
        CompilerStageContract {
            stage: Stage::Parsing,
            availability: Availability::Experimental,
            integration: Integration::CompilerDriver,
            input_contracts: vec!["LexedDocument 0.1".to_owned()],
            output_contracts: vec!["ConcreteSyntaxTree 0.1 and recovery diagnostics".to_owned()],
            deterministic_output_required: true,
            owner: "mncs-language parser contract".to_owned(),
            decisions: vec!["accepted 0.1 grammar is intentionally narrow".to_owned()],
            blockers: vec!["multi-statement and specification grammar".to_owned()],
        },
        CompilerStageContract {
            stage: Stage::ConcreteSyntaxTree,
            availability: Availability::Experimental,
            integration: Integration::CompilerDriver,
            input_contracts: vec!["token stream".to_owned()],
            output_contracts: vec!["lossless CST with hierarchical token ranges".to_owned()],
            deterministic_output_required: true,
            owner: "mncs-language source representation contract".to_owned(),
            decisions: vec!["exact source reconstructs from retained tokens".to_owned()],
            blockers: vec!["stable recovery-node contract".to_owned()],
        },
        CompilerStageContract {
            stage: Stage::AbstractSyntaxTree,
            availability: Availability::Experimental,
            integration: Integration::CompilerDriver,
            input_contracts: vec!["lossless CST".to_owned()],
            output_contracts: vec!["typed-name AST 0.1 with source spans".to_owned()],
            deterministic_output_required: true,
            owner: "mncs-language elaboration contract".to_owned(),
            decisions: vec!["AST identity is not semantic identity".to_owned()],
            blockers: vec!["complete expression and specification AST".to_owned()],
        },
        CompilerStageContract {
            stage: Stage::SemanticGraphConstruction,
            availability: Availability::Experimental,
            integration: Integration::CompilerDriver,
            input_contracts: vec!["validated mncs-model Program".to_owned()],
            output_contracts: vec!["SemanticGraph 0.2".to_owned()],
            deterministic_output_required: true,
            owner: "mncs-model semantic graph".to_owned(),
            decisions: vec!["graph is content-addressed and relationship-first".to_owned()],
            blockers: vec!["recursive cross-artifact graph construction".to_owned()],
        },
        CompilerStageContract {
            stage: Stage::IdentityResolution,
            availability: Availability::Experimental,
            integration: Integration::CompilerDriver,
            input_contracts: vec!["semantic entities and relationships".to_owned()],
            output_contracts: vec!["SemanticIdentities 0.2".to_owned()],
            deterministic_output_required: true,
            owner: "mncs-model identity system".to_owned(),
            decisions: vec!["semantic identity is distinct from file identity".to_owned()],
            blockers: vec!["scope and binding contract from RFC 0035".to_owned()],
        },
        CompilerStageContract {
            stage: Stage::TypeAndContractAnalysis,
            availability: Availability::Partial,
            integration: Integration::CompilerDriver,
            input_contracts: vec!["mncs-model Program".to_owned()],
            output_contracts: vec!["ValidationReport".to_owned()],
            deterministic_output_required: true,
            owner: "mncs-model validation and obligation contracts".to_owned(),
            decisions: vec!["invalid semantics cannot reach HIR".to_owned()],
            blockers: vec!["general type, inference, and contract calculus".to_owned()],
        },
        CompilerStageContract {
            stage: Stage::HighLevelIr,
            availability: Availability::Experimental,
            integration: Integration::CompilerDriver,
            input_contracts: vec!["canonical semantic artifact".to_owned()],
            output_contracts: vec!["HighLevelIr 0.3".to_owned()],
            deterministic_output_required: true,
            owner: "mncs-model HIR contract".to_owned(),
            decisions: vec!["effects, obligations, and provenance remain explicit".to_owned()],
            blockers: vec!["complete semantic coverage".to_owned()],
        },
        CompilerStageContract {
            stage: Stage::VerificationPasses,
            availability: Availability::Partial,
            integration: Integration::CompilerDriver,
            input_contracts: vec!["HIR and canonical SSA".to_owned()],
            output_contracts: vec!["TransformationEdge and unresolved obligations".to_owned()],
            deterministic_output_required: true,
            owner: "mncs-language pass and evidence contracts".to_owned(),
            decisions: vec!["UNKNOWN never authorizes stronger lowering".to_owned()],
            blockers: vec!["translation validation and independent checkers".to_owned()],
        },
        CompilerStageContract {
            stage: Stage::LoweringBackendGeneration,
            availability: Availability::Experimental,
            integration: Integration::CompilerDriver,
            input_contracts: vec!["selected SSA and target contract".to_owned()],
            output_contracts: vec![
                "BackendCapabilityManifest, TargetLoweringPlan, and typed backend artifact"
                    .to_owned(),
            ],
            deterministic_output_required: true,
            owner: "mncs-language backend adapter contract".to_owned(),
            decisions: vec![
                "unevidenced target facts remain UNKNOWN".to_owned(),
                "adapter target facts are explicit data, not host defaults".to_owned(),
                "portable WASM, research bytecode, LLVM IR, C11, and Cranelift use the same adapter boundary".to_owned(),
            ],
            blockers: vec!["memory/ABI-rich native lowering and additional ISA families".to_owned()],
        },
        CompilerStageContract {
            stage: Stage::ExecutableArtifact,
            availability: Availability::Experimental,
            integration: Integration::CompilerDriver,
            input_contracts: vec!["identity-bound typed backend artifact".to_owned()],
            output_contracts: vec![
                "adapter-specific bounded research execution observation".to_owned(),
            ],
            deterministic_output_required: true,
            owner: "mncs-language executable and RFC 0031 derivation contracts".to_owned(),
            decisions: vec![
                "research execution is adapter-owned and does not imply a host runtime".to_owned(),
                "body/SSA/backend agreement is empirical bounded agreement, not universal equivalence".to_owned(),
            ],
            blockers: vec![
                "cross-ISA native object packaging as a first-class artifact kind".to_owned(),
                "linker derivation identity beyond external clang/llc observation".to_owned(),
                "independent translation checker remaining trust".to_owned(),
            ],
        },
    ])
}

pub fn native_host_identities() -> (CompilerHostIdentity, BuildHostIdentity) {
    let architecture = std::env::consts::ARCH;
    let operating_system = std::env::consts::OS;
    let environment = native_environment(operating_system);
    (
        CompilerHostIdentity::new(architecture, operating_system, environment, BTreeMap::new()),
        BuildHostIdentity::new(
            architecture,
            operating_system,
            environment,
            format!("rust-{}", env!("CARGO_PKG_RUST_VERSION")),
            BTreeMap::new(),
        ),
    )
}

pub fn native_node_profile(node_identity: impl Into<String>) -> CompilerNodeProfile {
    let operating_system = std::env::consts::OS.to_owned();
    let architecture = std::env::consts::ARCH.to_owned();
    CompilerNodeProfile {
        node_identity: node_identity.into(),
        architecture,
        operating_system: operating_system.clone(),
        environment: native_environment(&operating_system).to_owned(),
        observations: BTreeMap::new(),
    }
}

fn failed_result(
    request: &CompilationRequest,
    diagnostics: Vec<CompilerDiagnostic>,
) -> CompilationResult {
    let mut result = CompilationResult {
        schema_version: COMPILER_ARTIFACT_SCHEMA_VERSION.to_owned(),
        identity: SemanticId(String::new()),
        request_identity: request.identity.clone(),
        request: request.clone(),
        status: CompilationStatus::Failed,
        artifacts: Vec::new(),
        emissions: CompilationEmissions::default(),
        evidence: None,
        diagnostics,
    };
    result.seal();
    result
}

fn native_environment(operating_system: &str) -> &'static str {
    match operating_system {
        "linux" => "native-linux",
        "windows" => "native-windows",
        "macos" => "native-macos",
        _ => "native-unknown",
    }
}

fn fingerprint<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("compiler artifact is serializable");
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use mncs_syntax::{SourceArtifactKind, SourceEnvelope};

    fn program(name: &str) -> Program {
        Program::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/executable/checked-add.mncs.json"
        )))
        .map(|mut program| {
            name.clone_into(&mut program.module);
            program
        })
        .expect("fixture program")
    }

    fn emissions() -> BTreeSet<ArtifactRepresentation> {
        [
            ArtifactRepresentation::Semantic,
            ArtifactRepresentation::Hir,
            ArtifactRepresentation::Ssa,
            ArtifactRepresentation::EvidenceBundle,
        ]
        .into_iter()
        .collect()
    }

    fn source_envelope() -> SourceEnvelope {
        SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "examples.identity",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../examples/source/identity.mncs"
            )),
        )
    }

    fn flagship_envelope() -> SourceEnvelope {
        SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "examples.flagship",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../examples/source/flagship.mncs"
            )),
        )
    }

    fn cre1_envelope() -> SourceEnvelope {
        SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "examples.cre1",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../examples/source/cre1-evidence-combine.mncs"
            )),
        )
    }

    #[test]
    fn source_fixture_traverses_every_front_end_stage_into_hir() {
        let compiler = ReferenceCompiler::default();
        let output = compiler.run_local_source_study(source_envelope());
        assert!(
            output.front_end.is_valid(),
            "{:#?}",
            output.front_end.diagnostics
        );
        assert_eq!(
            output.front_end.cst.reconstruct(),
            output.front_end.envelope.text
        );
        let study = output.study.expect("valid source study");
        assert!(study.identity_is_valid());
        for representation in [
            ArtifactRepresentation::Source,
            ArtifactRepresentation::LexicalTokens,
            ArtifactRepresentation::ConcreteSyntaxTree,
            ArtifactRepresentation::AbstractSyntaxTree,
            ArtifactRepresentation::Semantic,
            ArtifactRepresentation::SemanticGraph,
            ArtifactRepresentation::IdentityMap,
            ArtifactRepresentation::Validation,
            ArtifactRepresentation::Hir,
            ArtifactRepresentation::Ssa,
        ] {
            assert!(
                study.stage_fingerprints.contains_key(&representation),
                "missing {representation:?}"
            );
        }
        let passes = study
            .pass_executions
            .iter()
            .map(|execution| execution.pass_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(passes[0], "lex-source");
        assert!(passes.contains(&"construct-semantic-graph"));
        assert!(passes.contains(&"lower-semantic-to-hir"));
        let hir = output.front_end.program.unwrap().lower_to_ir().unwrap();
        assert_eq!(hir.functions.len(), 1);
        assert_eq!(hir.functions[0].inputs.len(), 1);
    }

    #[test]
    fn source_profile_03_preserves_finite_match_and_call_relations() {
        let compiler = ReferenceCompiler::default();
        let front_end = compiler.front_end(cre1_envelope());
        assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
        let program = front_end.program.expect("elaborated CRE-1 program");
        assert_eq!(program.finite_types.len(), 1);
        assert_eq!(program.finite_types[0].variants.len(), 3);
        let graph = program.semantic_graph().expect("valid semantic graph");
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == mncs_model::EdgeKind::Calls));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == mncs_model::EdgeKind::TestsVariant));
        let hir = program.lower_to_ir().expect("Profile 0.3 HIR");
        assert!(hir
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .any(|block| block.operations.iter().any(|operation| matches!(
                operation.kind,
                mncs_model::IrOperationKind::Call { .. }
            ))));
        let ssa = program.lower_to_ssa().expect("Profile 0.3 SSA");
        assert!(ssa
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .any(
                |block| block.instructions.iter().any(|instruction| matches!(
                    instruction.kind,
                    mncs_model::SsaInstructionKind::Call { .. }
                ))
            ));
        assert!(program
            .generate_obligations()
            .obligations
            .iter()
            .any(|obligation| {
                obligation.requirement.0.contains("call-authority-closure")
                    && obligation.status == ObligationStatus::Pass
            }));
    }

    #[test]
    fn source_profile_04_executes_minimum_and_maximum_accepted_bounds() {
        let compiler = ReferenceCompiler::default();
        for bound in [1, mncs_model::SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND] {
            let source = format!(
                "mncs 0.4; module profile.bounds{bound}; \
                 fn candidate(input: i64) -> (result: i64) {{ \
                   iterate attempts up_to {bound} carrying current: i64 = input {{ \
                     next current = current; \
                   }} \
                   return input; \
                 }}"
            );
            let front_end = compiler.front_end(SourceEnvelope::inline(
                SourceArtifactKind::Program,
                format!("profile.bounds{bound}"),
                source,
            ));
            assert!(
                front_end.is_valid(),
                "bound {bound}: {:#?}",
                front_end.diagnostics
            );
            let program = front_end.program.expect("bounded program");
            let iteration = &program.functions[0]
                .body
                .as_ref()
                .expect("body")
                .bounded_iterations[0];
            assert_eq!(iteration.bound, bound);
            let request = mncs_model::ExecutionRequest {
                schema_version: mncs_model::EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
                target: mncs_model::ExecutionTarget {
                    module: program.module.clone(),
                    function: "candidate".to_owned(),
                },
                arguments: vec![mncs_model::ExecutionValue::Integer {
                    value: 7,
                    ty: mncs_model::IntegerType {
                        bits: 64,
                        signed: true,
                    },
                }],
                step_budget: 10_000,
                policy: mncs_model::ExecutionPolicy::default(),
            };
            let body = mncs_model::execute_with_policy(&program, &request);
            let ssa = mncs_model::execute_ssa(&program, &request);
            assert_eq!(body.status, mncs_model::ExecutionStatus::Returned);
            assert_eq!(ssa.status, mncs_model::ExecutionStatus::Returned);
            assert_eq!(body.returned, ssa.returned);
        }
    }

    #[test]
    fn source_profile_0_2_elaborates_bindings_contracts_and_failure() {
        let compiler = ReferenceCompiler::default();
        let output = compiler.run_local_source_study(flagship_envelope());
        assert!(
            output.front_end.is_valid(),
            "{:#?}",
            output.front_end.diagnostics
        );
        let program = output.front_end.program.expect("elaborated flagship");
        let function = &program.functions[0];
        assert_eq!(function.name, "bounded_step");
        assert!(function
            .contracts
            .iter()
            .any(|clause| clause.id == "n_within_limit"));
        assert!(function
            .capabilities
            .iter()
            .any(|capability| capability == "checked_integer"));
        assert!(function.body.as_ref().unwrap().blocks.len() > 1);
        let study = output.study.expect("flagship source study");
        assert!(study
            .stage_fingerprints
            .contains_key(&ArtifactRepresentation::Ssa));
    }

    #[test]
    fn source_profile_0_2_rejects_type_mismatch_duplicate_and_unresolved_bindings() {
        let compiler = ReferenceCompiler::default();
        let mismatched = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "invalid.type",
            "mncs 0.2; module invalid.type; fn bad(a: i32) -> (result: i32) { let flag: bool = a; return a; }",
        );
        let mismatch = compiler.front_end(mismatched);
        assert!(!mismatch.is_valid());
        assert!(mismatch
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MNE117"));

        let duplicate = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "invalid.duplicate",
            "mncs 0.2; module invalid.duplicate; fn bad(a: i32) -> (result: i32) { let a: i32 = 1; return a; }",
        );
        let duplicate = compiler.front_end(duplicate);
        assert!(!duplicate.is_valid());
        assert!(duplicate
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MNE110"));

        let unresolved = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "invalid.unresolved",
            "mncs 0.2; module invalid.unresolved; fn bad(a: i32) -> (result: i32) { return missing; }",
        );
        let unresolved = compiler.front_end(unresolved);
        assert!(!unresolved.is_valid());
        assert!(unresolved
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MNE102"));

        for (origin, source, code) in [
            (
                "invalid.unsupported-type",
                "mncs 0.2; module invalid.unsupported; fn bad(a: text) -> (result: text) { return a; }",
                "MNE105",
            ),
            (
                "invalid.condition",
                "mncs 0.2; module invalid.condition; fn bad(a: i32) -> (result: i32) { if a { return a; } else { return a; } return a; }",
                "MNE117",
            ),
            (
                "invalid.unreachable",
                "mncs 0.2; module invalid.unreachable; fn bad(a: i32) -> (result: i32) { fail rejected; let b: i32 = a; return b; }",
                "MNE114",
            ),
            (
                "invalid.duplicate-function",
                "mncs 0.2; module invalid.functions; fn bad(a: i32) -> (result: i32) { return a; } fn bad(a: i32) -> (result: i32) { return a; }",
                "MNE104",
            ),
        ] {
            let result = compiler.front_end(SourceEnvelope::inline(
                SourceArtifactKind::Program,
                origin,
                source,
            ));
            assert!(!result.is_valid(), "{origin} unexpectedly passed");
            assert!(
                result
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == code),
                "{origin} did not emit {code}: {:?}",
                result.diagnostics
            );
        }
    }

    #[test]
    fn invalid_source_stops_before_semantic_identity_and_hir() {
        let compiler = ReferenceCompiler::default();
        let envelope = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "invalid.return",
            "mncs 0.1; module invalid.sample; fn id(value: i64) -> (result: i64) { return missing; }",
        );
        let output = compiler.run_local_source_study(envelope);
        assert!(output.study.is_none());
        assert!(output.front_end.program.is_none());
        assert!(output
            .front_end
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MNE102"));
        assert!(output
            .front_end
            .artifact(ArtifactRepresentation::Hir)
            .is_none());
    }

    #[test]
    fn compilation_is_deterministic_and_reuses_existing_ir() {
        let compiler = ReferenceCompiler::default();
        let program = program("compiler-test");
        let request = compiler.request_for_program(&program, emissions(), None);
        let first = compiler.compile(request.clone(), &program);
        let second = compiler.compile(request, &program);
        assert_eq!(first, second);
        assert!(first.identity_is_valid());
        let round_trip: CompilationResult =
            serde_json::from_str(&serde_json::to_string(&first).unwrap()).unwrap();
        assert_eq!(round_trip, first);
        assert!(round_trip.request.identity_is_valid());
        let hir = program.lower_to_ir().unwrap();
        let ssa = program.lower_to_ssa().unwrap();
        assert_eq!(
            first.emissions.hir.as_ref().unwrap().fingerprint().unwrap(),
            hir.fingerprint().unwrap()
        );
        assert_eq!(
            first.emissions.ssa.as_ref().unwrap().fingerprint().unwrap(),
            ssa.fingerprint().unwrap()
        );
    }

    #[test]
    fn invalid_semantics_never_reach_hir_or_ssa() {
        let compiler = ReferenceCompiler::default();
        let program = Program::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/executable/invalid-body-capability.mncs.json"
        )))
        .unwrap();
        let request = compiler.request_for_program(&program, emissions(), None);
        let result = compiler.compile(request, &program);
        assert_eq!(result.status, CompilationStatus::Failed);
        assert!(result.artifacts.is_empty());
        assert!(result.emissions.hir.is_none());
        assert!(result.emissions.ssa.is_none());
        assert!(result
            .diagnostics
            .iter()
            .all(|item| item.kind == CompilerDiagnosticKind::SemanticInvalidity));
    }

    #[test]
    fn unresolved_obligations_are_retained_and_do_not_authorize_rewrites() {
        let compiler = ReferenceCompiler::default();
        let program = program("compiler-unresolved");
        let request = compiler.request_for_program(&program, emissions(), None);
        let result = compiler.compile(request, &program);
        assert_eq!(
            result.status,
            CompilationStatus::CompletedWithUnresolvedObligations
        );
        let evidence = result.evidence.unwrap();
        assert!(!evidence.unresolved_obligations.is_empty());
        assert!(evidence
            .transformation_edges
            .iter()
            .any(|edge| edge.status == TransformationStatus::Unknown));
        let selection = evidence
            .transformation_edges
            .iter()
            .find(|edge| edge.pass.id == "select-canonical-ssa")
            .unwrap();
        assert_eq!(selection.status, TransformationStatus::Pass);
        assert_eq!(selection.input.fingerprint, selection.output.fingerprint);

        let mut laundered = evidence;
        laundered.unresolved_obligations.clear();
        laundered.seal();
        assert!(laundered
            .validate_integrity()
            .iter()
            .any(|diagnostic| diagnostic.code == "CMP103"));
    }

    #[test]
    fn host_change_changes_derivation_but_not_semantic_hir_or_ssa() {
        let compiler = ReferenceCompiler::default();
        let program = program("compiler-host-invariance");
        let native_request = compiler.request_for_program(&program, emissions(), None);
        let (alternate_os, alternate_environment) =
            if native_request.compiler_host.operating_system == "windows" {
                ("linux", "native-linux")
            } else {
                ("windows", "native-windows")
            };
        let alternate_host = CompilerHostIdentity::new(
            "x86_64",
            alternate_os,
            alternate_environment,
            BTreeMap::new(),
        );
        let alternate_request = CompilationRequest::new(
            native_request.input.clone(),
            native_request.language_profile.clone(),
            native_request.emit.clone(),
            native_request.compiler.clone(),
            alternate_host,
            native_request.build_host.clone(),
            None,
            None,
            native_request.pipeline.clone(),
            Vec::new(),
            None,
            None,
        );
        let native = compiler.compile(native_request.clone(), &program);
        let alternate = compiler.compile(alternate_request.clone(), &program);
        assert_ne!(native_request.identity, alternate_request.identity);
        assert_ne!(native.identity, alternate.identity);
        for representation in [
            ArtifactRepresentation::Semantic,
            ArtifactRepresentation::Hir,
            ArtifactRepresentation::Ssa,
        ] {
            let left = native
                .artifacts
                .iter()
                .find(|item| item.representation == representation)
                .unwrap();
            let right = alternate
                .artifacts
                .iter()
                .find(|item| item.representation == representation)
                .unwrap();
            assert_eq!(left, right);
        }
    }

    #[test]
    fn mismatched_input_identity_is_structurally_rejected() {
        let compiler = ReferenceCompiler::default();
        let input_program = program("compiler-input-a");
        let mut request = compiler.request_for_program(&input_program, emissions(), None);
        let different = program("compiler-input-b");
        request.input = compiler
            .request_for_program(&different, emissions(), None)
            .input;
        request.identity = CompilationRequest::new(
            request.input.clone(),
            request.language_profile.clone(),
            request.emit.clone(),
            request.compiler.clone(),
            request.compiler_host.clone(),
            request.build_host.clone(),
            request.target.clone(),
            request.run_environment.clone(),
            request.pipeline.clone(),
            request.assumptions.clone(),
            request.assurance_profile.clone(),
            request.backend.clone(),
        )
        .identity;
        let result = compiler.compile(request, &input_program);
        assert_eq!(result.status, CompilationStatus::Failed);
        assert!(result.diagnostics.iter().any(|item| item.code == "CMP203"));
    }

    #[test]
    fn target_plan_is_unknown_and_preserves_pretarget_fingerprints() {
        let compiler = ReferenceCompiler::default();
        let program = program("compiler-target");
        let baseline = compiler.compile(
            compiler.request_for_program(&program, emissions(), None),
            &program,
        );
        let target = TargetContractRef::new(
            "aarch64-unknown-linux-gnu",
            BTreeMap::new(),
            Vec::new(),
            vec!["target triple is a candidate, not evidence".to_owned()],
        );
        let mut target_emissions = emissions();
        target_emissions.insert(ArtifactRepresentation::TargetLoweringPlan);
        let targeted = compiler.compile(
            compiler.request_for_program(&program, target_emissions, Some(target)),
            &program,
        );
        for representation in [
            ArtifactRepresentation::Semantic,
            ArtifactRepresentation::Hir,
            ArtifactRepresentation::Ssa,
        ] {
            let left = baseline
                .artifacts
                .iter()
                .find(|item| item.representation == representation)
                .unwrap();
            let right = targeted
                .artifacts
                .iter()
                .find(|item| item.representation == representation)
                .unwrap();
            assert_eq!(left.fingerprint, right.fingerprint);
        }
        assert_eq!(
            targeted.emissions.target_lowering_plan.unwrap().status,
            TransformationStatus::Unknown
        );
        assert!(!targeted
            .artifacts
            .iter()
            .any(|artifact| artifact.representation == ArtifactRepresentation::BackendArtifact));
        assert!(!targeted
            .diagnostics
            .iter()
            .any(|item| item.kind == CompilerDiagnosticKind::UnavailableBackendCapability));
    }

    #[test]
    fn portable_wasm_target_emits_a_backend_artifact() {
        let compiler = ReferenceCompiler::default();
        let program = program("compiler-portable-wasm");
        let mut emit = emissions();
        emit.insert(ArtifactRepresentation::TargetLoweringPlan);
        emit.insert(ArtifactRepresentation::BackendArtifact);
        let request = compiler.request_for_program(
            &program,
            emit,
            Some(mncs_codegen::portable_wasm_target()),
        );
        let result = compiler.compile(request, &program);
        assert!(
            result
                .artifacts
                .iter()
                .any(|artifact| artifact.representation == ArtifactRepresentation::BackendArtifact),
            "{:?}",
            result.diagnostics
        );
        assert_eq!(
            result
                .emissions
                .target_lowering_plan
                .as_ref()
                .unwrap()
                .status,
            TransformationStatus::Pass
        );
        assert_eq!(
            result.emissions.backend.as_ref().unwrap().status,
            TransformationStatus::Pass
        );
    }

    #[test]
    fn architecture_contract_covers_every_logical_stage_without_hiding_gaps() {
        let architecture = reference_compiler_architecture();
        assert!(architecture.validate().is_empty());
        assert_eq!(architecture.stages.len(), CompilerStage::ORDERED.len());
        let lexical = architecture
            .stages
            .iter()
            .find(|stage| stage.stage == CompilerStage::LexicalAnalysis)
            .unwrap();
        assert_eq!(lexical.availability, StageAvailability::Experimental);
        assert_eq!(lexical.integration, StageIntegration::CompilerDriver);
        let hir = architecture
            .stages
            .iter()
            .find(|stage| stage.stage == CompilerStage::HighLevelIr)
            .unwrap();
        assert_eq!(hir.integration, StageIntegration::CompilerDriver);
        let executable = architecture
            .stages
            .iter()
            .find(|stage| stage.stage == CompilerStage::ExecutableArtifact)
            .unwrap();
        assert_eq!(executable.availability, StageAvailability::Experimental);
        assert_eq!(executable.integration, StageIntegration::CompilerDriver);
    }

    #[test]
    fn study_result_is_a_language_owned_observation_with_pass_records() {
        let compiler = ReferenceCompiler::default();
        let program = program("compiler-study-contract");
        let request = compiler.request_for_program(&program, emissions(), None);
        let study =
            CompilationStudyRequest::new(native_node_profile("test-node"), request, Vec::new());
        let result = compiler.run_study(study, &program);
        assert!(result.identity_is_valid());
        assert_eq!(result.contract_id, COMPILATION_STUDY_RESULT_CONTRACT_ID);
        assert_eq!(
            result.interpretation,
            COMPILATION_STUDY_OBSERVATION_INTERPRETATION
        );
        assert!(result
            .stage_fingerprints
            .contains_key(&ArtifactRepresentation::Semantic));
        assert!(result
            .pass_executions
            .iter()
            .any(|pass| pass.pass_id == "lower-semantic-to-hir"));
        assert!(!result.unresolved_obligations.is_empty());
        let family_reference = result.family_reference();
        assert_eq!(family_reference.producer, "mncs-language");
        assert_eq!(family_reference.stable_id, result.identity.0);
        assert_eq!(family_reference.content_digest.len(), 64);
        assert_eq!(
            family_reference.compiler.selected_ssa_identity,
            result.selected_ssa_identity
        );
        assert!(family_reference
            .authority_boundary
            .contains("does not certify experiment success"));

        let mut laundered = result;
        "conformant".clone_into(&mut laundered.interpretation);
        laundered.seal();
        assert!(!laundered.identity_is_valid());
    }
}
