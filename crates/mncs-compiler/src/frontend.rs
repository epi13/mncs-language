use std::collections::{BTreeMap, BTreeSet};

use mncs_model::{
    finite_type_id, finite_variant_id, function_id, ArithmeticIntent, ArtifactRepresentation,
    BodyBlock, BodyOperation, BodyOperationKind, BodyParameter, BodyTerminator, BodyType,
    BodyValue, CompilationStatus, CompilationStudyRequest, CompilationStudyResult,
    CompilerArtifactRef, CompilerNodeProfile, CompilerPassExecutionObservation, ContractClause,
    ContractKind, Effect, FailureMode, FiniteType, FiniteVariant, Function, FunctionBody,
    IntegerType, Program, SemanticGraph, SemanticId, SemanticIdentities, TransformationEdge,
    TransformationStatus, ValidationReport, Value, EXECUTABLE_BODY_SCHEMA_VERSION,
    SUPPORTED_SCHEMA_VERSION,
};
use mncs_syntax::{
    parse, AbstractSyntaxTree, AstBinaryOp, AstExpr, AstFunction, AstStmt, ConcreteSyntaxTree,
    DiagnosticSeverity, DiagnosticStage, LexedDocument, ParseOutput, SourceArtifactKind,
    SourceDiagnostic, SourceEnvelope, SourceSpan, AST_SCHEMA_VERSION, CST_SCHEMA_VERSION,
    LEXICAL_SCHEMA_VERSION, SOURCE_ENVELOPE_SCHEMA_VERSION,
};
use serde::Serialize;

use crate::{fingerprint, native_node_profile, ReferenceCompiler};

#[derive(Debug, Clone, Serialize)]
pub struct SourceFrontEndResult {
    pub envelope: SourceEnvelope,
    pub lexical: LexedDocument,
    pub cst: ConcreteSyntaxTree,
    pub ast: Option<AbstractSyntaxTree>,
    pub program: Option<Program>,
    pub semantic_graph: Option<SemanticGraph>,
    pub identities: Option<SemanticIdentities>,
    pub validation: Option<ValidationReport>,
    pub artifacts: Vec<CompilerArtifactRef>,
    pub diagnostics: Vec<SourceDiagnostic>,
}

impl SourceFrontEndResult {
    pub fn is_valid(&self) -> bool {
        self.program.is_some()
            && self.semantic_graph.is_some()
            && self.identities.is_some()
            && self.validation.as_ref().is_some_and(|report| report.valid)
            && self
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != DiagnosticSeverity::Error)
    }

    pub fn artifact(&self, representation: ArtifactRepresentation) -> Option<&CompilerArtifactRef> {
        self.artifacts
            .iter()
            .find(|artifact| artifact.representation == representation)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceStudyOutput {
    pub front_end: SourceFrontEndResult,
    pub study: Option<CompilationStudyResult>,
}

impl ReferenceCompiler {
    pub fn front_end(&self, envelope: SourceEnvelope) -> SourceFrontEndResult {
        let ParseOutput {
            lexical,
            cst,
            ast,
            diagnostics,
        } = parse(&envelope);
        let mut artifacts = vec![
            CompilerArtifactRef::new(
                ArtifactRepresentation::Source,
                SOURCE_ENVELOPE_SCHEMA_VERSION,
                fingerprint(&envelope),
            ),
            CompilerArtifactRef::new(
                ArtifactRepresentation::LexicalTokens,
                LEXICAL_SCHEMA_VERSION,
                lexical.fingerprint(),
            ),
            CompilerArtifactRef::new(
                ArtifactRepresentation::ConcreteSyntaxTree,
                CST_SCHEMA_VERSION,
                cst.fingerprint(),
            ),
        ];
        let mut diagnostics = diagnostics;
        let mut program = None;
        let mut validation = None;
        let mut semantic_graph = None;
        let mut identities = None;
        if let Some(tree) = &ast {
            artifacts.push(CompilerArtifactRef::new(
                ArtifactRepresentation::AbstractSyntaxTree,
                AST_SCHEMA_VERSION,
                tree.fingerprint(),
            ));
            if envelope.artifact_kind != SourceArtifactKind::Program {
                diagnostics.push(elaboration_diagnostic(
                    "MNE100",
                    "the 0.1 vertical slice elaborates program artifacts only",
                    tree.span,
                ));
            } else {
                match elaborate_program(tree) {
                    Ok(elaborated) => {
                        let report = elaborated.validate();
                        let canonical = elaborated
                            .canonical_form()
                            .expect("an elaborated program is canonicalizable");
                        artifacts.push(CompilerArtifactRef::new(
                            ArtifactRepresentation::Semantic,
                            canonical.schema_version,
                            canonical.fingerprint,
                        ));
                        if report.valid {
                            let graph = elaborated
                                .semantic_graph()
                                .expect("validated elaboration has a semantic graph");
                            let identity_map = elaborated.semantic_identities();
                            artifacts.push(CompilerArtifactRef::new(
                                ArtifactRepresentation::SemanticGraph,
                                &graph.schema_version,
                                fingerprint(&graph),
                            ));
                            artifacts.push(CompilerArtifactRef::new(
                                ArtifactRepresentation::IdentityMap,
                                &identity_map.schema_version,
                                fingerprint(&identity_map),
                            ));
                            semantic_graph = Some(graph);
                            identities = Some(identity_map);
                        } else {
                            diagnostics.extend(report.errors.iter().map(|error| {
                                elaboration_diagnostic(
                                    &error.code,
                                    &format!("{}: {}", error.path, error.message),
                                    tree.span,
                                )
                            }));
                        }
                        artifacts.push(CompilerArtifactRef::new(
                            ArtifactRepresentation::Validation,
                            SUPPORTED_SCHEMA_VERSION,
                            fingerprint(&report),
                        ));
                        validation = Some(report);
                        program = Some(elaborated);
                    }
                    Err(mut errors) => diagnostics.append(&mut errors),
                }
            }
        }
        SourceFrontEndResult {
            envelope,
            lexical,
            cst,
            ast,
            program,
            semantic_graph,
            identities,
            validation,
            artifacts,
            diagnostics,
        }
    }

    pub fn run_source_study(
        &self,
        envelope: SourceEnvelope,
        node: CompilerNodeProfile,
    ) -> SourceStudyOutput {
        self.run_source_study_inner(envelope, node, None)
    }

    pub fn run_source_study_with_backend(
        &self,
        envelope: SourceEnvelope,
        node: CompilerNodeProfile,
        backend_name: &str,
    ) -> SourceStudyOutput {
        self.run_source_study_inner(envelope, node, Some(backend_name))
    }

    fn run_source_study_inner(
        &self,
        envelope: SourceEnvelope,
        node: CompilerNodeProfile,
        backend_name: Option<&str>,
    ) -> SourceStudyOutput {
        let front_end = self.front_end(envelope);
        let Some(program) = front_end.program.as_ref().filter(|_| front_end.is_valid()) else {
            return SourceStudyOutput {
                front_end,
                study: None,
            };
        };
        let mut emit = [
            ArtifactRepresentation::Semantic,
            ArtifactRepresentation::Hir,
            ArtifactRepresentation::Ssa,
            ArtifactRepresentation::EvidenceBundle,
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        if backend_name.is_some() {
            emit.insert(ArtifactRepresentation::TargetLoweringPlan);
            emit.insert(ArtifactRepresentation::BackendArtifact);
        }
        let request = match backend_name {
            Some(name) => self
                .request_for_program_with_backend(program, emit, name)
                .expect("backend name was validated by the experiment planner"),
            None => self.request_for_program(program, emit, None),
        };
        let study_request = CompilationStudyRequest::new(node, request, Vec::new());
        let mut study = self.run_study(study_request, program);
        let front_end_fingerprints = front_end
            .artifacts
            .iter()
            .map(|artifact| (artifact.representation, artifact.fingerprint.clone()))
            .collect::<BTreeMap<_, _>>();
        study.stage_fingerprints.extend(front_end_fingerprints);
        let mut executions = self.front_end_pass_executions(&front_end);
        executions.append(&mut study.pass_executions);
        study.pass_executions = executions;
        study.compilation_status = if front_end.is_valid() {
            study.compilation_status
        } else {
            CompilationStatus::Failed
        };
        study.seal();
        SourceStudyOutput {
            front_end,
            study: Some(study),
        }
    }

    pub fn run_local_source_study(&self, envelope: SourceEnvelope) -> SourceStudyOutput {
        self.run_source_study(envelope, native_node_profile("local-source-node"))
    }

    fn front_end_pass_executions(
        &self,
        front_end: &SourceFrontEndResult,
    ) -> Vec<CompilerPassExecutionObservation> {
        let stages = [
            (
                "lex-source",
                ArtifactRepresentation::Source,
                ArtifactRepresentation::LexicalTokens,
            ),
            (
                "parse-source-to-cst",
                ArtifactRepresentation::LexicalTokens,
                ArtifactRepresentation::ConcreteSyntaxTree,
            ),
            (
                "derive-ast",
                ArtifactRepresentation::ConcreteSyntaxTree,
                ArtifactRepresentation::AbstractSyntaxTree,
            ),
            (
                "elaborate-ast-to-semantic",
                ArtifactRepresentation::AbstractSyntaxTree,
                ArtifactRepresentation::Semantic,
            ),
            (
                "construct-semantic-graph",
                ArtifactRepresentation::Semantic,
                ArtifactRepresentation::SemanticGraph,
            ),
            (
                "resolve-semantic-identities",
                ArtifactRepresentation::SemanticGraph,
                ArtifactRepresentation::IdentityMap,
            ),
            (
                "analyze-types-and-contracts",
                ArtifactRepresentation::IdentityMap,
                ArtifactRepresentation::Validation,
            ),
        ];
        stages
            .into_iter()
            .filter_map(|(pass_id, input_kind, output_kind)| {
                let input = front_end.artifact(input_kind)?;
                let output = front_end.artifact(output_kind)?;
                let pass = self
                    .pipeline
                    .passes
                    .iter()
                    .find(|pass| pass.id == pass_id)
                    .expect("reference pipeline contains every front-end pass");
                let edge = TransformationEdge::new(
                    input.clone(),
                    output.clone(),
                    pass.clone(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    TransformationStatus::Pass,
                    Vec::new(),
                );
                Some(CompilerPassExecutionObservation::from(&edge))
            })
            .collect()
    }
}

pub fn elaborate_program(ast: &AbstractSyntaxTree) -> Result<Program, Vec<SourceDiagnostic>> {
    let mut diagnostics = Vec::new();
    let mut finite_types = Vec::new();
    let mut finite_type_names = BTreeSet::new();
    for declaration in &ast.finite_types {
        if !finite_type_names.insert(declaration.name.text.clone()) {
            diagnostics.push(elaboration_diagnostic(
                "MNE120",
                "finite type identity is duplicated in this module namespace",
                declaration.name.span,
            ));
            continue;
        }
        let mut variant_names = BTreeSet::new();
        let mut variants = Vec::new();
        for (discriminant, variant) in declaration.variants.iter().enumerate() {
            if !variant_names.insert(variant.text.clone()) {
                diagnostics.push(elaboration_diagnostic(
                    "MNE121",
                    "finite variant is duplicated and would create an unreachable match arm",
                    variant.span,
                ));
                continue;
            }
            variants.push(FiniteVariant {
                identity: finite_variant_id(
                    &ast.module.text,
                    &declaration.name.text,
                    &variant.text,
                ),
                name: variant.text.clone(),
                discriminant: discriminant as u32,
            });
        }
        finite_types.push(FiniteType {
            identity: finite_type_id(&ast.module.text, &declaration.name.text),
            name: declaration.name.text.clone(),
            variants,
        });
    }
    let finite_types_by_name = finite_types
        .iter()
        .map(|finite_type| (finite_type.name.clone(), finite_type.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut functions = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    for function in &ast.functions {
        if !names.insert(function.name.text.clone()) {
            diagnostics.push(elaboration_diagnostic(
                "MNE104",
                "function identity is duplicated in this module namespace",
                function.name.span,
            ));
            continue;
        }
    }
    let signatures = ast
        .functions
        .iter()
        .map(|function| {
            (
                function.name.text.clone(),
                FunctionSignature::from_ast(ast, function, &finite_types_by_name, &mut diagnostics),
            )
        })
        .collect::<BTreeMap<_, _>>();
    reject_recursive_calls(ast, &signatures, &mut diagnostics);
    for function in &ast.functions {
        if functions
            .iter()
            .any(|item: &Function| item.name == function.name.text)
        {
            continue;
        }
        match elaborate_function(ast, function, &finite_types_by_name, &signatures) {
            Ok(elaborated) => functions.push(elaborated),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if diagnostics.is_empty() {
        Ok(Program {
            schema_version: SUPPORTED_SCHEMA_VERSION.to_owned(),
            module: ast.module.text.clone(),
            finite_types,
            assumptions: Vec::new(),
            functions,
        })
    } else {
        Err(diagnostics)
    }
}

fn elaborate_function(
    ast: &AbstractSyntaxTree,
    function: &AstFunction,
    finite_types: &BTreeMap<String, FiniteType>,
    signatures: &BTreeMap<String, FunctionSignature>,
) -> Result<Function, Vec<SourceDiagnostic>> {
    let mut diagnostics = Vec::new();
    if function.outputs.len() != 1 {
        diagnostics.push(elaboration_diagnostic(
            "MNE101",
            "the current source profiles require exactly one named output",
            function.span,
        ));
        return Err(diagnostics);
    }
    let inputs = function
        .inputs
        .iter()
        .map(|input| Value {
            name: input.name.text.clone(),
            value_type: input.value_type.text.clone(),
        })
        .collect::<Vec<_>>();
    let outputs = function
        .outputs
        .iter()
        .map(|output| Value {
            name: output.name.text.clone(),
            value_type: output.value_type.text.clone(),
        })
        .collect::<Vec<_>>();
    let parameters = function
        .inputs
        .iter()
        .map(|input| BodyParameter {
            id: input.name.text.clone(),
            name: input.name.text.clone(),
            ty: profile_type(
                &input.value_type.text,
                input.value_type.span,
                finite_types,
                &mut diagnostics,
            ),
        })
        .collect::<Vec<_>>();
    let output_type = profile_type(
        &function.outputs[0].value_type.text,
        function.outputs[0].value_type.span,
        finite_types,
        &mut diagnostics,
    );
    let mut capabilities = function
        .capabilities
        .iter()
        .map(|capability| capability.text.clone())
        .collect::<Vec<_>>();
    capabilities.sort();
    let mut seen_caps = std::collections::BTreeSet::new();
    for capability in &function.capabilities {
        if !seen_caps.insert(&capability.text) {
            diagnostics.push(elaboration_diagnostic(
                "MNE110",
                "capability declaration is ambiguous in this function scope",
                capability.span,
            ));
        }
    }
    let mut effects = Vec::new();
    for effect in &function.effects {
        if !seen_caps.contains(&effect.capability.text) {
            diagnostics.push(elaboration_diagnostic(
                "MNE111",
                "an effect cannot appear without the capability that authorizes it",
                effect.span,
            ));
            continue;
        }
        effects.push(Effect {
            kind: effect.kind.text.clone(),
            target: function.name.text.clone(),
            capability: effect.capability.text.clone(),
        });
    }
    let contracts = function
        .contracts
        .iter()
        .filter_map(|clause| {
            let kind = match clause.kind.text.as_str() {
                "requires" => ContractKind::Requires,
                "ensures" => ContractKind::Ensures,
                "assumes" => ContractKind::Requires,
                _ => {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE112",
                        "unsupported contract clause",
                        clause.span,
                    ));
                    return None;
                }
            };
            Some(ContractClause {
                id: clause.name.text.clone(),
                kind,
                expression: clause.name.text.clone(),
            })
        })
        .collect();
    let assumptions = function
        .contracts
        .iter()
        .filter(|clause| clause.kind.text == "assumes")
        .map(|clause| clause.name.text.clone())
        .collect::<Vec<_>>();
    let mut env = BindingEnv::new();
    for (input, parameter) in function.inputs.iter().zip(&parameters) {
        env.bind(
            input.name.text.clone(),
            input.name.text.clone(),
            parameter.ty.clone(),
            input.span,
            &mut diagnostics,
        );
    }
    let blocks = if function.body.statements.is_empty()
        && matches!(function.body.returned_value, AstExpr::Name(_))
    {
        let AstExpr::Name(returned_name) = &function.body.returned_value else {
            unreachable!("guarded above")
        };
        let returned = env.resolve(&returned_name.text, returned_name.span, &mut diagnostics);
        if let Some(returned) = returned {
            if returned.ty != output_type {
                diagnostics.push(elaboration_diagnostic(
                    "MNE103",
                    "returned value type does not match the declared output type",
                    returned_name.span,
                ));
            }
        }
        vec![BodyBlock {
            id: "entry".to_owned(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: BodyTerminator::Return {
                values: vec![returned_name.text.clone()],
            },
        }]
    } else {
        let mut builder = BodyBuilder::new(
            output_type.clone(),
            function.name.text.clone(),
            finite_types,
            signatures,
        );
        builder.elaborate_statements(&function.body.statements, &mut env, &mut diagnostics);
        if let Some(returned) = builder.elaborate_expr(
            &function.body.returned_value,
            Some(&output_type),
            &mut env,
            &mut diagnostics,
        ) {
            builder.finish_return(
                returned,
                function.body.returned_value.span(),
                &mut diagnostics,
            );
        }
        builder.blocks
    };
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let _ = ast;
    let _ = assumptions;
    Ok(Function {
        name: function.name.text.clone(),
        inputs,
        outputs,
        contracts,
        effects,
        capabilities,
        assumptions: function
            .contracts
            .iter()
            .filter(|clause| clause.kind.text == "assumes")
            .map(|clause| clause.name.text.clone())
            .collect(),
        evidence: Vec::new(),
        failure: FailureMode::Isolated,
        body: Some(FunctionBody {
            schema_version: EXECUTABLE_BODY_SCHEMA_VERSION.to_owned(),
            entry: "entry".to_owned(),
            parameters,
            blocks,
        }),
    })
}

#[derive(Clone)]
struct FunctionSignature {
    identity: SemanticId,
    inputs: Vec<BodyType>,
    output: BodyType,
    capabilities: Vec<String>,
    effects: Vec<Effect>,
}

impl FunctionSignature {
    fn from_ast(
        ast: &AbstractSyntaxTree,
        function: &AstFunction,
        finite_types: &BTreeMap<String, FiniteType>,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Self {
        let inputs = function
            .inputs
            .iter()
            .map(|input| {
                profile_type(
                    &input.value_type.text,
                    input.value_type.span,
                    finite_types,
                    diagnostics,
                )
            })
            .collect();
        let output = function.outputs.first().map_or_else(
            || BodyType::Named("invalid".to_owned()),
            |output| {
                profile_type(
                    &output.value_type.text,
                    output.value_type.span,
                    finite_types,
                    diagnostics,
                )
            },
        );
        let mut capabilities = function
            .capabilities
            .iter()
            .map(|capability| capability.text.clone())
            .collect::<Vec<_>>();
        capabilities.sort();
        capabilities.dedup();
        let effects = function
            .effects
            .iter()
            .map(|effect| Effect {
                kind: effect.kind.text.clone(),
                target: function.name.text.clone(),
                capability: effect.capability.text.clone(),
            })
            .collect();
        Self {
            identity: function_id(&ast.module.text, &function.name.text),
            inputs,
            output,
            capabilities,
            effects,
        }
    }
}

fn reject_recursive_calls(
    ast: &AbstractSyntaxTree,
    signatures: &BTreeMap<String, FunctionSignature>,
    diagnostics: &mut Vec<SourceDiagnostic>,
) {
    let graph = ast
        .functions
        .iter()
        .map(|function| {
            let mut calls = BTreeSet::new();
            calls_in_expr(&function.body.returned_value, &mut calls);
            for statement in &function.body.statements {
                calls_in_statement(statement, &mut calls);
            }
            calls.retain(|callee| signatures.contains_key(callee));
            (function.name.text.clone(), calls)
        })
        .collect::<BTreeMap<_, _>>();
    for function in &ast.functions {
        let start = function.name.text.as_str();
        let mut visiting = vec![(start, vec![start])];
        let mut recursive = false;
        while let Some((current, path)) = visiting.pop() {
            for callee in graph.get(current).into_iter().flatten() {
                if callee == start {
                    recursive = true;
                    break;
                }
                if !path.iter().any(|seen| seen == callee) {
                    let mut next_path = path.clone();
                    next_path.push(callee);
                    visiting.push((callee, next_path));
                }
            }
            if recursive {
                break;
            }
        }
        if recursive {
            diagnostics.push(elaboration_diagnostic(
                "MNE130",
                "Source Profile 0.3 rejects recursive call cycles; calls must be acyclic",
                function.name.span,
            ));
        }
    }
}

fn calls_in_statement(statement: &AstStmt, calls: &mut BTreeSet<String>) {
    match statement {
        AstStmt::Let { value, .. } | AstStmt::Return { value, .. } => calls_in_expr(value, calls),
        AstStmt::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            calls_in_expr(condition, calls);
            for statement in then_body.iter().chain(else_body) {
                calls_in_statement(statement, calls);
            }
        }
        AstStmt::Fail { .. } => {}
    }
}

fn calls_in_expr(expr: &AstExpr, calls: &mut BTreeSet<String>) {
    match expr {
        AstExpr::Call {
            function,
            arguments,
            ..
        } => {
            calls.insert(function.text.clone());
            for argument in arguments {
                calls_in_expr(argument, calls);
            }
        }
        AstExpr::Binary { left, right, .. } => {
            calls_in_expr(left, calls);
            calls_in_expr(right, calls);
        }
        AstExpr::Match { value, arms, .. } => {
            calls_in_expr(value, calls);
            for arm in arms {
                calls_in_expr(&arm.value, calls);
            }
        }
        AstExpr::Name(_)
        | AstExpr::Integer { .. }
        | AstExpr::Boolean { .. }
        | AstExpr::FiniteVariant { .. } => {}
    }
}

struct BindingEnv {
    scopes: Vec<std::collections::BTreeMap<String, (ResolvedBinding, SourceSpan)>>,
}

#[derive(Clone)]
struct ResolvedBinding {
    id: String,
    ty: BodyType,
}

impl BindingEnv {
    fn new() -> Self {
        Self {
            scopes: vec![std::collections::BTreeMap::new()],
        }
    }

    fn bind(
        &mut self,
        name: String,
        id: String,
        ty: BodyType,
        span: SourceSpan,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) {
        let scope = self.scopes.last_mut().expect("scope stack is non-empty");
        if let Some((_, existing)) = scope.get(&name) {
            let _ = existing;
            diagnostics.push(elaboration_diagnostic(
                "MNE110",
                "binding is ambiguous in this lexical scope",
                span,
            ));
            return;
        }
        scope.insert(name, (ResolvedBinding { id, ty }, span));
    }

    fn resolve(
        &self,
        name: &str,
        span: SourceSpan,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        for scope in self.scopes.iter().rev() {
            if let Some((id, _)) = scope.get(name) {
                return Some(id.clone());
            }
        }
        diagnostics.push(elaboration_diagnostic(
            "MNE102",
            "name does not resolve to a unique binding in the current lexical environment",
            span,
        ));
        None
    }

    fn push(&mut self) {
        self.scopes.push(std::collections::BTreeMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }
}

struct BodyBuilder<'a> {
    blocks: Vec<BodyBlock>,
    current: usize,
    next_value: usize,
    next_block: usize,
    output_type: BodyType,
    function: String,
    finite_types: &'a BTreeMap<String, FiniteType>,
    signatures: &'a BTreeMap<String, FunctionSignature>,
}

impl<'a> BodyBuilder<'a> {
    fn new(
        output_type: BodyType,
        function: String,
        finite_types: &'a BTreeMap<String, FiniteType>,
        signatures: &'a BTreeMap<String, FunctionSignature>,
    ) -> Self {
        Self {
            blocks: vec![BodyBlock {
                id: "entry".to_owned(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: BodyTerminator::Return { values: Vec::new() },
            }],
            current: 0,
            next_value: 0,
            next_block: 0,
            output_type,
            function,
            finite_types,
            signatures,
        }
    }

    fn elaborate_statements(
        &mut self,
        statements: &[AstStmt],
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) {
        for statement in statements {
            if !self.block_is_open() {
                diagnostics.push(elaboration_diagnostic(
                    "MNE114",
                    "statement is unreachable after a terminal return or failure",
                    statement_span(statement),
                ));
                break;
            }
            self.elaborate_statement(statement, env, diagnostics);
        }
    }

    fn elaborate_statement(
        &mut self,
        statement: &AstStmt,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) {
        match statement {
            AstStmt::Let {
                name,
                value_type,
                value,
                span,
            } => {
                let declared = profile_type(
                    &value_type.text,
                    value_type.span,
                    self.finite_types,
                    diagnostics,
                );
                let Some(produced) = self.elaborate_expr(value, Some(&declared), env, diagnostics)
                else {
                    return;
                };
                if produced.ty != declared {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE115",
                        "binding initializer type does not match its declared type",
                        *span,
                    ));
                }
                env.bind(
                    name.text.clone(),
                    produced.id.clone(),
                    declared,
                    *span,
                    diagnostics,
                );
            }
            AstStmt::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let bool_type = BodyType::Named("bool".to_owned());
                let Some(cond) = self.elaborate_expr(condition, Some(&bool_type), env, diagnostics)
                else {
                    return;
                };
                if cond.ty != bool_type {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE116",
                        "if condition must have type bool",
                        condition.span(),
                    ));
                    return;
                }
                let then_id = self.new_block();
                let else_id = self.new_block();
                let join_id = self.new_block();
                self.blocks[self.current].terminator = BodyTerminator::ConditionalBranch {
                    condition: cond.id,
                    then_target: then_id.clone(),
                    then_arguments: Vec::new(),
                    else_target: else_id.clone(),
                    else_arguments: Vec::new(),
                };
                self.current = self.index_of(&then_id);
                env.push();
                self.elaborate_statements(then_body, env, diagnostics);
                if self.block_is_open() {
                    self.blocks[self.current].terminator = BodyTerminator::Branch {
                        target: join_id.clone(),
                        arguments: Vec::new(),
                    };
                }
                env.pop();
                self.current = self.index_of(&else_id);
                env.push();
                self.elaborate_statements(else_body, env, diagnostics);
                if self.block_is_open() {
                    self.blocks[self.current].terminator = BodyTerminator::Branch {
                        target: join_id.clone(),
                        arguments: Vec::new(),
                    };
                }
                env.pop();
                self.current = self.index_of(&join_id);
            }
            AstStmt::Fail { mode, span } => {
                let failure = match mode.text.as_str() {
                    "isolated" => FailureMode::Isolated,
                    "atomic" => FailureMode::Atomic,
                    "fatal" => FailureMode::Fatal,
                    _ => {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE113",
                            "unsupported failure mode",
                            *span,
                        ));
                        FailureMode::Isolated
                    }
                };
                self.blocks[self.current].terminator = BodyTerminator::Failure { mode: failure };
            }
            AstStmt::Return { value, span } => {
                if let Some(resolved) =
                    self.elaborate_expr(value, Some(&self.output_type.clone()), env, diagnostics)
                {
                    if resolved.ty != self.output_type {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE103",
                            "returned value type does not match the declared output type",
                            *span,
                        ));
                    }
                    self.blocks[self.current].terminator = BodyTerminator::Return {
                        values: vec![resolved.id],
                    };
                }
            }
        }
    }

    fn elaborate_expr(
        &mut self,
        expr: &AstExpr,
        expected: Option<&BodyType>,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        match expr {
            AstExpr::Name(name) => {
                let resolved = env.resolve(&name.text, name.span, diagnostics)?;
                if expected.is_some_and(|expected| expected != &resolved.ty) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE117",
                        "resolved name does not have the required expression type",
                        name.span,
                    ));
                }
                Some(resolved)
            }
            AstExpr::Integer { value, text } => {
                let ty = match expected {
                    Some(BodyType::Integer(integer)) => BodyType::Integer(*integer),
                    Some(_) => {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE118",
                            "integer literal cannot satisfy a non-integer type",
                            text.span,
                        ));
                        BodyType::Integer(IntegerType {
                            bits: 64,
                            signed: true,
                        })
                    }
                    None => BodyType::Integer(IntegerType {
                        bits: 64,
                        signed: true,
                    }),
                };
                let id = self.new_value("c");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::Constant {
                        value: *value,
                        ty: ty.clone(),
                    },
                    operands: Vec::new(),
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: ty.clone(),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                let _ = text;
                Some(ResolvedBinding { id, ty })
            }
            AstExpr::Boolean { value, text: _ } => {
                let ty = BodyType::Named("bool".to_owned());
                if expected.is_some_and(|expected| expected != &ty) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE122",
                        "boolean literal cannot satisfy the required expression type",
                        expr.span(),
                    ));
                }
                let id = self.new_value("b");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::Constant {
                        value: i128::from(*value),
                        ty: ty.clone(),
                    },
                    operands: Vec::new(),
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: ty.clone(),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                Some(ResolvedBinding { id, ty })
            }
            AstExpr::FiniteVariant {
                type_name,
                variant,
                span,
            } => {
                let Some(finite_type) = self.finite_types.get(&type_name.text).cloned() else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE123",
                        "finite constructor names an unknown nominal type",
                        type_name.span,
                    ));
                    return None;
                };
                let Some(declared_variant) = finite_type
                    .variants
                    .iter()
                    .find(|candidate| candidate.name == variant.text)
                    .cloned()
                else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE124",
                        "finite constructor names an unknown variant",
                        variant.span,
                    ));
                    return None;
                };
                let ty = BodyType::Finite {
                    identity: finite_type.identity.clone(),
                    name: finite_type.name.clone(),
                };
                if expected.is_some_and(|expected| expected != &ty) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE125",
                        "finite constructor does not have the required nominal type",
                        *span,
                    ));
                }
                let id = self.new_value("e");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::FiniteConstruct {
                        type_identity: finite_type.identity.clone(),
                        variant_identity: declared_variant.identity,
                        discriminant: declared_variant.discriminant,
                    },
                    operands: Vec::new(),
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: ty.clone(),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                Some(ResolvedBinding { id, ty })
            }
            AstExpr::Call {
                function,
                arguments,
                span,
            } => {
                let Some(signature) = self.signatures.get(&function.text).cloned() else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE131",
                        "call target does not resolve to a function in this module",
                        function.span,
                    ));
                    return None;
                };
                if arguments.len() != signature.inputs.len() {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE132",
                        "call argument arity does not match the callee signature",
                        *span,
                    ));
                    return None;
                }
                let mut operands = Vec::new();
                for (source_argument, parameter_type) in arguments.iter().zip(&signature.inputs) {
                    let argument = self.elaborate_expr(
                        source_argument,
                        Some(parameter_type),
                        env,
                        diagnostics,
                    )?;
                    if &argument.ty != parameter_type {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE133",
                            "call argument type does not match the callee parameter",
                            source_argument.span(),
                        ));
                    }
                    operands.push(argument.id);
                }
                let caller = self
                    .signatures
                    .get(&self.function)
                    .expect("current function signature");
                let authority_closed = signature
                    .capabilities
                    .iter()
                    .all(|capability| caller.capabilities.contains(capability))
                    && signature.effects.iter().all(|callee_effect| {
                        caller.effects.iter().any(|caller_effect| {
                            caller_effect.kind == callee_effect.kind
                                && caller_effect.capability == callee_effect.capability
                        })
                    });
                if !authority_closed {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE134",
                        "call cannot manufacture or launder the callee's required authority/effects",
                        *span,
                    ));
                    return None;
                }
                if expected.is_some_and(|expected| expected != &signature.output) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE135",
                        "call result does not have the required expression type",
                        *span,
                    ));
                }
                let id = self.new_value("call");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::Call {
                        function: signature.identity.clone(),
                        function_name: function.text.clone(),
                        required_capabilities: signature.capabilities.clone(),
                        effects: signature.effects.clone(),
                    },
                    operands,
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: signature.output.clone(),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                Some(ResolvedBinding {
                    id,
                    ty: signature.output.clone(),
                })
            }
            AstExpr::Match { value, arms, span } => {
                let subject = self.elaborate_expr(value, None, env, diagnostics)?;
                let BodyType::Finite {
                    identity: type_identity,
                    name: type_name,
                } = &subject.ty
                else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE136",
                        "match subject must have a declared finite type",
                        value.span(),
                    ));
                    return None;
                };
                let finite_type = self
                    .finite_types
                    .get(type_name)
                    .expect("finite body type came from declaration")
                    .clone();
                let Some(result_type) = expected.cloned() else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE137",
                        "match result requires an expected type in Source Profile 0.3",
                        *span,
                    ));
                    return None;
                };
                let diagnostic_count = diagnostics.len();
                let mut seen = BTreeSet::new();
                let mut resolved_arms = Vec::new();
                for arm in arms {
                    let Some(variant) = finite_type
                        .variants
                        .iter()
                        .find(|variant| variant.name == arm.variant.text)
                    else {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE138",
                            "match arm names a variant outside the subject's finite type",
                            arm.variant.span,
                        ));
                        continue;
                    };
                    if !seen.insert(variant.identity.clone()) {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE139",
                            "duplicate match arm is unreachable",
                            arm.variant.span,
                        ));
                        continue;
                    }
                    resolved_arms.push((variant.clone(), &arm.value));
                }
                let missing = finite_type
                    .variants
                    .iter()
                    .filter(|variant| !seen.contains(&variant.identity))
                    .map(|variant| variant.name.clone())
                    .collect::<Vec<_>>();
                if !missing.is_empty() {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE140",
                        &format!(
                            "non-exhaustive match; missing variants: {}",
                            missing.join(", ")
                        ),
                        *span,
                    ));
                }
                if diagnostics.len() != diagnostic_count || resolved_arms.is_empty() {
                    return None;
                }

                let dispatch_start = self.current;
                let join_id = self.new_block();
                let arm_ids = resolved_arms
                    .iter()
                    .map(|_| self.new_block())
                    .collect::<Vec<_>>();
                let test_ids = (1..resolved_arms.len())
                    .map(|_| self.new_block())
                    .collect::<Vec<_>>();
                let mut dispatch = dispatch_start;
                for (index, ((variant, _), arm_id)) in
                    resolved_arms.iter().zip(&arm_ids).enumerate()
                {
                    self.current = dispatch;
                    if index + 1 == resolved_arms.len() {
                        self.blocks[self.current].terminator = BodyTerminator::Branch {
                            target: arm_id.clone(),
                            arguments: Vec::new(),
                        };
                    } else {
                        let condition = self.new_value("match");
                        self.blocks[self.current].operations.push(BodyOperation {
                            id: condition.clone(),
                            kind: BodyOperationKind::FiniteIsVariant {
                                type_identity: type_identity.clone(),
                                variant_identity: variant.identity.clone(),
                                discriminant: variant.discriminant,
                            },
                            operands: vec![subject.id.clone()],
                            results: vec![BodyValue {
                                id: condition.clone(),
                                ty: BodyType::Named("bool".to_owned()),
                            }],
                            contracts: Vec::new(),
                            assumptions: Vec::new(),
                            machine_intent: None,
                            lowering: None,
                            portability: None,
                        });
                        let next_test = test_ids[index].clone();
                        self.blocks[self.current].terminator = BodyTerminator::ConditionalBranch {
                            condition,
                            then_target: arm_id.clone(),
                            then_arguments: Vec::new(),
                            else_target: next_test.clone(),
                            else_arguments: Vec::new(),
                        };
                        dispatch = self.index_of(&next_test);
                    }
                }
                let result_id = self.new_value("match_result");
                let join_index = self.index_of(&join_id);
                self.blocks[join_index].parameters.push(BodyValue {
                    id: result_id.clone(),
                    ty: result_type.clone(),
                });
                for ((_, arm_expr), arm_id) in resolved_arms.iter().zip(&arm_ids) {
                    self.current = self.index_of(arm_id);
                    env.push();
                    if let Some(value) =
                        self.elaborate_expr(arm_expr, Some(&result_type), env, diagnostics)
                    {
                        if value.ty != result_type {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE141",
                                "match arms must produce the same expected type",
                                arm_expr.span(),
                            ));
                        }
                        if self.block_is_open() {
                            self.blocks[self.current].terminator = BodyTerminator::Branch {
                                target: join_id.clone(),
                                arguments: vec![value.id],
                            };
                        }
                    }
                    env.pop();
                }
                self.current = join_index;
                Some(ResolvedBinding {
                    id: result_id,
                    ty: result_type,
                })
            }
            AstExpr::Binary {
                op, left, right, ..
            } => {
                let arithmetic =
                    matches!(op, AstBinaryOp::Add | AstBinaryOp::Sub | AstBinaryOp::Mul);
                let operand_expected = arithmetic.then_some(expected).flatten();
                let left_value = self.elaborate_expr(left, operand_expected, env, diagnostics)?;
                let right_value =
                    self.elaborate_expr(right, Some(&left_value.ty), env, diagnostics)?;
                if left_value.ty != right_value.ty {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE119",
                        "binary operands must have the same type",
                        expr.span(),
                    ));
                    return None;
                }
                let id = self.new_value("v");
                let (kind, result_ty) = match op {
                    AstBinaryOp::Add | AstBinaryOp::Sub | AstBinaryOp::Mul => {
                        let BodyType::Integer(operand_type) = left_value.ty else {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE120",
                                "arithmetic operands must have an integer type",
                                expr.span(),
                            ));
                            return None;
                        };
                        let operator = match op {
                            AstBinaryOp::Add => "add",
                            AstBinaryOp::Sub => "sub",
                            _ => "mul",
                        };
                        (
                            BodyOperationKind::Integer {
                                operator: operator.to_owned(),
                                operand_type,
                                intent: ArithmeticIntent::Checked,
                            },
                            BodyType::Integer(operand_type),
                        )
                    }
                    _ => {
                        let BodyType::Integer(operand_type) = left_value.ty else {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE121",
                                "comparison operands must have an integer type",
                                expr.span(),
                            ));
                            return None;
                        };
                        let predicate = match op {
                            AstBinaryOp::Eq => "eq",
                            AstBinaryOp::Ne => "ne",
                            AstBinaryOp::Lt => "lt",
                            AstBinaryOp::Le => "le",
                            AstBinaryOp::Gt => "gt",
                            AstBinaryOp::Ge => "ge",
                            _ => "eq",
                        };
                        (
                            BodyOperationKind::IntegerCompare {
                                predicate: predicate.to_owned(),
                                operand_type,
                            },
                            BodyType::Named("bool".to_owned()),
                        )
                    }
                };
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind,
                    operands: vec![left_value.id, right_value.id],
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: result_ty.clone(),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                Some(ResolvedBinding { id, ty: result_ty })
            }
        }
    }

    fn finish_return(
        &mut self,
        value: ResolvedBinding,
        span: SourceSpan,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) {
        if value.ty != self.output_type {
            diagnostics.push(elaboration_diagnostic(
                "MNE103",
                "returned value type does not match the declared output type",
                span,
            ));
        }
        if self.block_is_open() {
            self.blocks[self.current].terminator = BodyTerminator::Return {
                values: vec![value.id],
            };
        }
    }

    fn new_value(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}{}", self.next_value);
        self.next_value += 1;
        id
    }

    fn new_block(&mut self) -> String {
        self.next_block += 1;
        let id = format!("b{}", self.next_block);
        self.blocks.push(BodyBlock {
            id: id.clone(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: BodyTerminator::Return { values: Vec::new() },
        });
        id
    }

    fn index_of(&self, id: &str) -> usize {
        self.blocks
            .iter()
            .position(|block| block.id == id)
            .expect("block exists")
    }

    fn block_is_open(&self) -> bool {
        matches!(
            &self.blocks[self.current].terminator,
            BodyTerminator::Return { values } if values.is_empty()
        )
    }
}

fn profile_type(
    name: &str,
    span: SourceSpan,
    finite_types: &BTreeMap<String, FiniteType>,
    diagnostics: &mut Vec<SourceDiagnostic>,
) -> BodyType {
    if let Some(finite_type) = finite_types.get(name) {
        return BodyType::Finite {
            identity: finite_type.identity.clone(),
            name: finite_type.name.clone(),
        };
    }
    let ty = BodyType::from_semantic_name(name);
    let supported = matches!(&ty, BodyType::Named(named) if named == "bool")
        || matches!(
            &ty,
            BodyType::Integer(IntegerType {
                bits: 8 | 16 | 32 | 64,
                ..
            })
        );
    if !supported {
        diagnostics.push(elaboration_diagnostic(
            "MNE105",
            "source profile supports bool, 8/16/32/64-bit integers, and declared finite types",
            span,
        ));
    }
    ty
}

fn statement_span(statement: &AstStmt) -> SourceSpan {
    match statement {
        AstStmt::Let { span, .. }
        | AstStmt::If { span, .. }
        | AstStmt::Fail { span, .. }
        | AstStmt::Return { span, .. } => *span,
    }
}

fn elaboration_diagnostic(code: &str, message: &str, span: SourceSpan) -> SourceDiagnostic {
    SourceDiagnostic {
        code: code.to_owned(),
        stage: DiagnosticStage::Elaboration,
        severity: DiagnosticSeverity::Error,
        message: message.to_owned(),
        span,
        expected: Vec::new(),
        found: None,
    }
}
