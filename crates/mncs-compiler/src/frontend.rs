use std::collections::{BTreeMap, BTreeSet};

use mncs_model::{
    ArithmeticIntent, ArtifactRepresentation, BodyBlock, BodyOperation, BodyOperationKind,
    BodyParameter, BodyTerminator, BodyType, BodyValue, CompilationStatus, CompilationStudyRequest,
    CompilationStudyResult, CompilerArtifactRef, CompilerNodeProfile,
    CompilerPassExecutionObservation, ContractClause, ContractKind, Effect, FailureMode, Function,
    FunctionBody, IntegerType, Program, SemanticGraph, SemanticIdentities, TransformationEdge,
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
        let front_end = self.front_end(envelope);
        let Some(program) = front_end.program.as_ref().filter(|_| front_end.is_valid()) else {
            return SourceStudyOutput {
                front_end,
                study: None,
            };
        };
        let emit = [
            ArtifactRepresentation::Semantic,
            ArtifactRepresentation::Hir,
            ArtifactRepresentation::Ssa,
            ArtifactRepresentation::EvidenceBundle,
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let request = self.request_for_program(program, emit, None);
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
    let mut functions = Vec::new();
    for function in &ast.functions {
        match elaborate_function(ast, function) {
            Ok(elaborated) => functions.push(elaborated),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if diagnostics.is_empty() {
        Ok(Program {
            schema_version: SUPPORTED_SCHEMA_VERSION.to_owned(),
            module: ast.module.text.clone(),
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
            ty: BodyType::from_semantic_name(&input.value_type.text),
        })
        .collect::<Vec<_>>();
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
    for input in &function.inputs {
        env.bind(
            input.name.text.clone(),
            input.name.text.clone(),
            input.span,
            &mut diagnostics,
        );
    }
    let blocks = if function.body.statements.is_empty() {
        let returned = env.resolve(
            &function.body.returned_value.text,
            function.body.returned_value.span,
            &mut diagnostics,
        );
        if let Some(returned) = returned {
            if let Some(input) = function
                .inputs
                .iter()
                .find(|input| input.name.text == returned)
            {
                if input.value_type.text != function.outputs[0].value_type.text {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE103",
                        "returned value type does not match the declared output type",
                        function.body.returned_value.span,
                    ));
                }
            }
        }
        vec![BodyBlock {
            id: "entry".to_owned(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: BodyTerminator::Return {
                values: vec![function.body.returned_value.text.clone()],
            },
        }]
    } else {
        let mut builder = BodyBuilder::new();
        builder.elaborate_statements(&function.body.statements, &mut env, &mut diagnostics);
        if let Some(returned) = env.resolve(
            &function.body.returned_value.text,
            function.body.returned_value.span,
            &mut diagnostics,
        ) {
            builder.finish_return(returned);
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

struct BindingEnv {
    scopes: Vec<std::collections::BTreeMap<String, (String, SourceSpan)>>,
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
        scope.insert(name, (id, span));
    }

    fn resolve(
        &self,
        name: &str,
        span: SourceSpan,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<String> {
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

struct BodyBuilder {
    blocks: Vec<BodyBlock>,
    current: usize,
    next_value: usize,
    next_block: usize,
}

impl BodyBuilder {
    fn new() -> Self {
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
        }
    }

    fn elaborate_statements(
        &mut self,
        statements: &[AstStmt],
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) {
        for statement in statements {
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
                let Some(produced) = self.elaborate_expr(value, env, diagnostics) else {
                    return;
                };
                env.bind(name.text.clone(), produced.clone(), *span, diagnostics);
                let _ = value_type;
            }
            AstStmt::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let Some(cond) = self.elaborate_expr(condition, env, diagnostics) else {
                    return;
                };
                let then_id = self.new_block();
                let else_id = self.new_block();
                let join_id = self.new_block();
                self.blocks[self.current].terminator = BodyTerminator::ConditionalBranch {
                    condition: cond,
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
                if let Some(resolved) = env.resolve(&value.text, *span, diagnostics) {
                    self.blocks[self.current].terminator = BodyTerminator::Return {
                        values: vec![resolved],
                    };
                }
            }
        }
    }

    fn elaborate_expr(
        &mut self,
        expr: &AstExpr,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<String> {
        match expr {
            AstExpr::Name(name) => env.resolve(&name.text, name.span, diagnostics),
            AstExpr::Integer { value, text } => {
                let id = self.new_value("c");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::Constant {
                        value: *value,
                        ty: BodyType::Integer(IntegerType {
                            bits: 64,
                            signed: true,
                        }),
                    },
                    operands: Vec::new(),
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: BodyType::Integer(IntegerType {
                            bits: 64,
                            signed: true,
                        }),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                let _ = text;
                Some(id)
            }
            AstExpr::Binary {
                op, left, right, ..
            } => {
                let left_id = self.elaborate_expr(left, env, diagnostics)?;
                let right_id = self.elaborate_expr(right, env, diagnostics)?;
                let id = self.new_value("v");
                let (kind, result_ty) = match op {
                    AstBinaryOp::Add | AstBinaryOp::Sub | AstBinaryOp::Mul => {
                        let operator = match op {
                            AstBinaryOp::Add => "add",
                            AstBinaryOp::Sub => "sub",
                            _ => "mul",
                        };
                        (
                            BodyOperationKind::Integer {
                                operator: operator.to_owned(),
                                operand_type: IntegerType {
                                    bits: 64,
                                    signed: true,
                                },
                                intent: ArithmeticIntent::Checked,
                            },
                            BodyType::Integer(IntegerType {
                                bits: 64,
                                signed: true,
                            }),
                        )
                    }
                    _ => {
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
                                operand_type: IntegerType {
                                    bits: 64,
                                    signed: true,
                                },
                            },
                            BodyType::Named("bool".to_owned()),
                        )
                    }
                };
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind,
                    operands: vec![left_id, right_id],
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: result_ty,
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                Some(id)
            }
        }
    }

    fn finish_return(&mut self, value: String) {
        if self.block_is_open() {
            self.blocks[self.current].terminator = BodyTerminator::Return {
                values: vec![value],
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
