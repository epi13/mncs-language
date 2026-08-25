use std::collections::{BTreeMap, BTreeSet};

use mncs_model::{
    finite_type_id, finite_variant_id, function_id, record_type_id, ArithmeticIntent,
    ArtifactRepresentation, BodyBlock, BodyBoundedIteration, BodyCyclePolicy, BodyOperation,
    BodyOperationKind, BodyParameter, BodyTerminator, BodyType, BodyValue,
    BoundedIterationCompletion, CompilationStatus, CompilationStudyRequest, CompilationStudyResult,
    CompilerArtifactRef, CompilerNodeProfile, CompilerPassExecutionObservation, ContractClause,
    ContractKind, Effect, FailureMode, FiniteType, FiniteVariant, Function, FunctionBody,
    IntegerType, Program, RecordField, RecordType, SemanticGraph, SemanticId, SemanticIdentities,
    TransformationEdge, TransformationStatus, ValidationReport, Value,
    EXECUTABLE_BODY_SCHEMA_VERSION, SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND,
    SUPPORTED_SCHEMA_VERSION,
};
use mncs_syntax::{
    parse, AbstractSyntaxTree, AstBinaryOp, AstExpr, AstFunction, AstStmt, ConcreteSyntaxTree,
    DiagnosticSeverity, DiagnosticStage, LexedDocument, ParseOutput, SourceArtifactKind,
    SourceDiagnostic, SourceEnvelope, SourceSpan, SpannedText, AST_SCHEMA_VERSION,
    CST_SCHEMA_VERSION, LEXICAL_SCHEMA_VERSION, SOURCE_ENVELOPE_SCHEMA_VERSION,
    SOURCE_PROFILE_VERSION_0_4,
};
use serde::Serialize;

use crate::resolution::{NameResolution, NameResolutionIndex, ResolvedNameKind};
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
    /// Authoritative name resolutions recorded during elaboration. Resolutions
    /// are recorded best-effort, so a partially valid document still exposes
    /// the occurrences that elaboration resolved successfully.
    pub name_resolutions: NameResolutionIndex,
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
        self.front_end_with_resolver(envelope, &NullResolver)
    }

    /// Runs the front end with a module resolver, linking `use` imports at
    /// elaboration time.
    pub fn front_end_with_resolver(
        &self,
        envelope: SourceEnvelope,
        resolver: &dyn ModuleResolver,
    ) -> SourceFrontEndResult {
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
        let mut name_resolutions = NameResolutionIndex::default();
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
                match elaborate_program_with_resolver(tree, resolver) {
                    (Ok(elaborated), resolutions) => {
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
                                    format!("{}: {}", error.path, error.message),
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
                        name_resolutions = NameResolutionIndex::new(resolutions);
                    }
                    (Err(mut errors), resolutions) => {
                        name_resolutions = NameResolutionIndex::new(resolutions);
                        diagnostics.append(&mut errors)
                    }
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
            name_resolutions,
            artifacts,
            diagnostics,
        }
    }

    pub fn run_source_study(
        &self,
        envelope: SourceEnvelope,
        node: CompilerNodeProfile,
    ) -> SourceStudyOutput {
        self.run_source_study_inner(envelope, node, None, &NullResolver)
    }

    /// Runs a source study with a module resolver for `use` imports.
    pub fn run_source_study_with_resolver(
        &self,
        envelope: SourceEnvelope,
        node: CompilerNodeProfile,
        resolver: &dyn ModuleResolver,
    ) -> SourceStudyOutput {
        self.run_source_study_inner(envelope, node, None, resolver)
    }

    pub fn run_source_study_with_backend(
        &self,
        envelope: SourceEnvelope,
        node: CompilerNodeProfile,
        backend_name: &str,
    ) -> SourceStudyOutput {
        self.run_source_study_inner(envelope, node, Some(backend_name), &NullResolver)
    }

    /// Runs a source study against a specific backend with a module resolver
    /// for `use` imports.
    pub fn run_source_study_with_backend_and_resolver(
        &self,
        envelope: SourceEnvelope,
        node: CompilerNodeProfile,
        backend_name: &str,
        resolver: &dyn ModuleResolver,
    ) -> SourceStudyOutput {
        self.run_source_study_inner(envelope, node, Some(backend_name), resolver)
    }

    fn run_source_study_inner(
        &self,
        envelope: SourceEnvelope,
        node: CompilerNodeProfile,
        backend_name: Option<&str>,
        resolver: &dyn ModuleResolver,
    ) -> SourceStudyOutput {
        let front_end = self.front_end_with_resolver(envelope, resolver);
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
    elaborate_program_with_resolutions(ast).0
}

/// Resolves imported module names to source envelopes during elaboration.
///
/// The name identifies the candidate; compatibility is established only by
/// elaborating the resolved module (RFC 0014: names are not compatibility).
/// Resolution misses and unresolvable modules are diagnostics in the
/// importing module, never silent skips.
pub trait ModuleResolver {
    fn resolve(&self, module: &str) -> Option<SourceEnvelope>;
}

/// A resolver that finds nothing. Programs elaborated with it must be
/// self-contained.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullResolver;

impl ModuleResolver for NullResolver {
    fn resolve(&self, _module: &str) -> Option<SourceEnvelope> {
        None
    }
}

/// Elaborates `ast` alone; module imports produce an "unavailable" diagnostic.
pub fn elaborate_program_with_resolutions(
    ast: &AbstractSyntaxTree,
) -> (Result<Program, Vec<SourceDiagnostic>>, Vec<NameResolution>) {
    elaborate_program_with_resolver(ast, &NullResolver)
}

/// Elaborates `ast` together with every module it imports, transitively.
///
/// Linking is elaboration-time: each dependency is elaborated independently
/// against the same resolver, its exported declarations are bound into this
/// module's namespace under collision rules, and functions from dependencies
/// participate in call checking and authority closure exactly as local
/// declarations do. The import graph must be acyclic; cycles are rejected.
///
/// Function, finite-type, and record-type identities stay computed from
/// their *declaring* module, so a declaration keeps one stable identity
/// across every program that binds it.
pub fn elaborate_program_with_resolver(
    ast: &AbstractSyntaxTree,
    resolver: &dyn ModuleResolver,
) -> (Result<Program, Vec<SourceDiagnostic>>, Vec<NameResolution>) {
    let mut diagnostics = Vec::new();
    let mut resolutions = Vec::new();
    let mut elaborated = BTreeMap::new();
    let mut declaration_spans = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    if let Err(mut errors) = elaborate_import_closure(
        ast,
        resolver,
        &mut elaborated,
        &mut declaration_spans,
        &mut visiting,
    ) {
        diagnostics.append(&mut errors);
        return (Err(diagnostics), resolutions);
    }
    match link_module_with_closure(ast, &elaborated, &declaration_spans, &mut resolutions) {
        Ok(program) => (Ok(program), resolutions),
        Err(mut errors) => {
            diagnostics.append(&mut errors);
            (Err(diagnostics), resolutions)
        }
    }
}

#[derive(Debug, Clone)]
struct ImportedModule {
    program: Program,
    declarations: DeclarationSpans,
}

#[derive(Default)]
struct MergedContext {
    finite_types_by_name: BTreeMap<String, FiniteType>,
    record_types_by_name: BTreeMap<String, RecordType>,
    signatures: BTreeMap<String, FunctionSignature>,
    imported_finite_types: Vec<FiniteType>,
    imported_record_types: Vec<RecordType>,
    imported_functions: Vec<Function>,
    imported_declarations: DeclarationSpans,
    /// Transitive dependency namespaces: everything this program was linked
    /// against, not only its own direct `use` targets. Identity validation
    /// accepts declarations anchored to any of these namespaces.
    imported_dependencies: BTreeSet<SemanticId>,
    finite_type_names: BTreeSet<String>,
    record_type_names: BTreeSet<String>,
    function_names: BTreeSet<String>,
}

/// Elaborates the transitive import closure of `ast`, post-order, into
/// `elaborated`. Every module in the closure is linked only after all of its
/// own imports are present in the map.
fn elaborate_import_closure(
    ast: &AbstractSyntaxTree,
    resolver: &dyn ModuleResolver,
    elaborated: &mut BTreeMap<String, Program>,
    declaration_spans: &mut BTreeMap<String, DeclarationSpans>,
    visiting: &mut BTreeSet<String>,
) -> Result<(), Vec<SourceDiagnostic>> {
    let name = ast.module.text.clone();
    if elaborated.contains_key(&name) {
        return Ok(());
    }
    declaration_spans.insert(name.clone(), DeclarationSpans::from_ast(ast));
    if !visiting.insert(name.clone()) {
        return Err(vec![elaboration_diagnostic(
            "MNE171",
            "module import graph contains a cycle",
            ast.module.span,
        )]);
    }
    for use_decl in &ast.uses {
        let dependency_name = use_decl.module.text.clone();
        if dependency_name == name {
            return Err(vec![elaboration_diagnostic(
                "MNE171",
                "a module may not import itself",
                use_decl.module.span,
            )]);
        }
        if elaborated.contains_key(&dependency_name) {
            continue;
        }
        let Some(dependency_envelope) = resolver.resolve(&dependency_name) else {
            return Err(vec![elaboration_diagnostic(
                "MNE173",
                format!("imported module '{dependency_name}' is unavailable to the resolver"),
                use_decl.module.span,
            )]);
        };
        let parsed = mncs_syntax::parse(&dependency_envelope);
        let Some(dependency_ast) = parsed.ast else {
            let codes = parsed
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(vec![elaboration_diagnostic(
                "MNE172",
                format!("imported module '{dependency_name}' failed to parse [{codes}]"),
                use_decl.module.span,
            )]);
        };
        elaborate_import_closure(
            &dependency_ast,
            resolver,
            elaborated,
            declaration_spans,
            visiting,
        )?;
        let dependency_program = link_module_with_closure(
            &dependency_ast,
            elaborated,
            declaration_spans,
            &mut Vec::new(),
        );
        let dependency_program = dependency_program.map_err(|errors| {
            vec![elaboration_diagnostic(
                "MNE172",
                format!(
                    "imported module '{dependency_name}' failed to elaborate [{}]",
                    errors
                        .iter()
                        .map(|diagnostic| diagnostic.code.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                use_decl.module.span,
            )]
        })?;
        elaborated.insert(dependency_name, dependency_program);
    }
    visiting.remove(&name);
    Ok(())
}

/// Links one module against its already-elaborated direct imports.
fn link_module_with_closure(
    ast: &AbstractSyntaxTree,
    elaborated: &BTreeMap<String, Program>,
    declaration_spans: &BTreeMap<String, DeclarationSpans>,
    resolutions: &mut Vec<NameResolution>,
) -> Result<Program, Vec<SourceDiagnostic>> {
    let mut diagnostics = Vec::new();
    let mut imported = Vec::new();
    let mut seen = BTreeSet::new();
    for use_decl in &ast.uses {
        let name = use_decl.module.text.clone();
        if !seen.insert(name.clone()) {
            diagnostics.push(elaboration_diagnostic(
                "MNE170",
                format!("module '{name}' is imported more than once"),
                use_decl.module.span,
            ));
            continue;
        }
        let _ = &name;
        match elaborated.get(&name) {
            Some(program) => imported.push(ImportedModule {
                program: program.clone(),
                declarations: declaration_spans.get(&name).cloned().unwrap_or_default(),
            }),
            None => diagnostics.push(elaboration_diagnostic(
                "MNE173",
                format!("imported module '{name}' is unavailable to the resolver"),
                use_decl.module.span,
            )),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let context = merge_imported_declarations(ast, &imported, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    elaborate_linked_module(ast, &context, resolutions)
}

/// Binds imported declarations into this module's namespace. Collisions fail
/// closed: identical re-exports of the same identity are accepted once, any
/// other name conflict is an error.
fn merge_imported_declarations(
    ast: &AbstractSyntaxTree,
    imported: &[ImportedModule],
    diagnostics: &mut Vec<SourceDiagnostic>,
) -> MergedContext {
    let mut context = MergedContext::default();

    // Local declarations occupy the namespace first.
    for declaration in &ast.finite_types {
        context
            .finite_type_names
            .insert(declaration.name.text.clone());
    }
    for declaration in &ast.record_types {
        context
            .record_type_names
            .insert(declaration.name.text.clone());
    }
    for function in &ast.functions {
        context.function_names.insert(function.name.text.clone());
    }

    // Deterministic merge order: import declaration order, then declaration
    // order within each dependency.
    //
    // Linked dependency programs contain their own imports' declarations, so
    // one identity can arrive through several paths; that is a diamond
    // re-export and binds once. Only a name conflict between DIFFERENT
    // identities fails closed.
    for module in imported {
        context
            .imported_dependencies
            .extend(module.program.dependencies.iter().cloned());
        context
            .imported_dependencies
            .insert(mncs_model::module_id(&module.program.module));
        context
            .imported_declarations
            .merge_missing(&module.declarations);
        for finite_type in &module.program.finite_types {
            // A local declaration sharing a name with an import is always a
            // collision; the module declared both under one namespace.
            if context.finite_type_names.contains(&finite_type.name) {
                diagnostics.push(elaboration_diagnostic(
                    "MNE174",
                    format!(
                        "imported finite type '{}' collides with a local declaration",
                        finite_type.name
                    ),
                    ast.module.span,
                ));
                continue;
            }
            if let Some(existing) = context.finite_types_by_name.get(&finite_type.name) {
                if existing.identity != finite_type.identity {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE174",
                        format!(
                            "imported finite type '{}' collides with an existing binding",
                            finite_type.name
                        ),
                        ast.module.span,
                    ));
                }
                continue;
            }
            context
                .finite_types_by_name
                .insert(finite_type.name.clone(), finite_type.clone());
            context.imported_finite_types.push(finite_type.clone());
        }
        for record_type in &module.program.record_types {
            if context.record_type_names.contains(&record_type.name) {
                diagnostics.push(elaboration_diagnostic(
                    "MNE175",
                    format!(
                        "imported record type '{}' collides with a local declaration",
                        record_type.name
                    ),
                    ast.module.span,
                ));
                continue;
            }
            if let Some(existing) = context.record_types_by_name.get(&record_type.name) {
                if existing.identity != record_type.identity {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE175",
                        format!(
                            "imported record type '{}' collides with an existing binding",
                            record_type.name
                        ),
                        ast.module.span,
                    ));
                }
                continue;
            }
            context
                .record_types_by_name
                .insert(record_type.name.clone(), record_type.clone());
            context.imported_record_types.push(record_type.clone());
        }
        for function in &module.program.functions {
            let candidate_signature = signature_from_function(module, function);
            if context.function_names.contains(&function.name) {
                diagnostics.push(elaboration_diagnostic(
                    "MNE176",
                    format!(
                        "imported function '{}' collides with a local declaration",
                        function.name
                    ),
                    ast.module.span,
                ));
                continue;
            }
            if let Some(existing) = context.signatures.get(&function.name) {
                if existing.identity != candidate_signature.identity {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE176",
                        format!(
                            "imported function '{}' collides with an existing binding",
                            function.name
                        ),
                        ast.module.span,
                    ));
                }
                continue;
            }
            let mut imported_function = function.clone();
            if imported_function.home_module.is_none() {
                // Locals of the dependency become linked imports here;
                // declarations that already carry a home keep it.
                imported_function.home_module = Some(module.program.module.clone());
            }
            // Imported names live in `signatures`, not in the locals-only
            // `function_names` set, so later diamond arrivals compare by
            // identity instead of colliding as locals.
            context
                .signatures
                .insert(function.name.clone(), candidate_signature);
            context.imported_functions.push(imported_function);
        }
    }
    context
}

/// Builds the call-checking signature for a function declared in another
/// module. Identities stay anchored to the declaring module: a function that
/// itself arrived through linking keeps its original home namespace.
fn signature_from_function(module: &ImportedModule, function: &Function) -> FunctionSignature {
    let body_type = |name: &str| body_type_from_name(module, name);
    let inputs = function
        .inputs
        .iter()
        .map(|value| body_type(&value.value_type))
        .collect();
    let output = function.outputs.first().map_or_else(
        || BodyType::Named("invalid".to_owned()),
        |value| body_type(&value.value_type),
    );
    let mut capabilities = function.capabilities.clone();
    capabilities.sort();
    capabilities.dedup();
    let home = function
        .home_module
        .as_deref()
        .unwrap_or(&module.program.module);
    FunctionSignature {
        identity: function_id(home, &function.name),
        inputs,
        output,
        capabilities,
        effects: function.effects.clone(),
    }
}

/// Resolves a type name against the merged namespace without diagnosing:
/// dependency programs are valid by the time merging happens, so unknown
/// names cannot occur here.
fn body_type_from_name(module: &ImportedModule, name: &str) -> BodyType {
    if let Some(finite_type) = module
        .program
        .finite_types
        .iter()
        .find(|decl| decl.name == name)
    {
        return BodyType::Finite {
            identity: finite_type.identity.clone(),
            name: finite_type.name.clone(),
        };
    }
    if let Some(record_type) = module
        .program
        .record_types
        .iter()
        .find(|decl| decl.name == name)
    {
        return BodyType::Record {
            identity: record_type.identity.clone(),
            name: record_type.name.clone(),
        };
    }
    BodyType::from_semantic_name(name)
}

/// Elaborate `ast` alone (no imports) and additionally return every name
/// resolution that elaboration decided. The resolutions are authoritative:
/// each entry is the exact binding decision used to accept the corresponding
/// occurrence.
///
/// Recording is best-effort; when elaboration fails, the resolutions decided
/// before the failure are still returned so tools can navigate partially valid
/// documents without re-implementing binding rules.
fn elaborate_linked_module(
    ast: &AbstractSyntaxTree,
    context: &MergedContext,
    resolutions: &mut Vec<NameResolution>,
) -> Result<Program, Vec<SourceDiagnostic>> {
    let mut diagnostics = Vec::new();

    // A local declaration may not shadow an imported binding under a
    // different identity; identical identities (diamond re-export) are fine
    // and were already folded away during merging.
    for declaration in &ast.finite_types {
        if let Some(imported) = context.finite_types_by_name.get(&declaration.name.text) {
            let local_identity = finite_type_id(&ast.module.text, &declaration.name.text);
            if imported.identity != local_identity {
                diagnostics.push(elaboration_diagnostic(
                    "MNE174",
                    format!(
                        "local finite type '{}' collides with an imported binding of a different identity",
                        declaration.name.text
                    ),
                    declaration.name.span,
                ));
            }
        }
    }
    for declaration in &ast.record_types {
        if context
            .record_types_by_name
            .contains_key(&declaration.name.text)
            || context
                .finite_types_by_name
                .contains_key(&declaration.name.text)
        {
            diagnostics.push(elaboration_diagnostic(
                "MNE175",
                format!(
                    "local record type '{}' collides with an imported binding",
                    declaration.name.text
                ),
                declaration.name.span,
            ));
        }
    }

    let mut declarations = DeclarationSpans::from_ast(ast);
    declarations.merge_missing(&context.imported_declarations);
    record_annotation_type_resolutions(ast, &declarations, resolutions);
    let declared_finite_names: BTreeSet<String> = ast
        .finite_types
        .iter()
        .map(|finite| finite.name.text.clone())
        .collect();
    let declared_record_names: BTreeSet<String> = ast
        .record_types
        .iter()
        .map(|record| record.name.text.clone())
        .collect();
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
            if !variant_names.insert(variant.name.text.clone()) {
                diagnostics.push(elaboration_diagnostic(
                    "MNE121",
                    "finite variant is duplicated and would create an unreachable match arm",
                    variant.name.span,
                ));
                continue;
            }
            // Payload fields (Profile 0.6): canonical order, unique names,
            // types resolvable to supported scalars, finite, or record types.
            let mut payload_field_names = BTreeSet::new();
            let mut payload_fields = Vec::new();
            for field in &variant.fields {
                if !payload_field_names.insert(field.name.text.clone()) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE170",
                        "variant payload field identity is duplicated",
                        field.name.span,
                    ));
                    continue;
                }
                if profile_scalar_supported(&field.value_type.text).is_none()
                    && !declared_finite_names.contains(&field.value_type.text)
                    && !declared_record_names.contains(&field.value_type.text)
                {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE171",
                        "variant payload field type does not name a supported scalar, finite, or declared record type",
                        field.value_type.span,
                    ));
                    continue;
                }
                payload_fields.push(RecordField {
                    name: field.name.text.clone(),
                    field_type: field.value_type.text.clone(),
                });
            }
            payload_fields.sort_by(|left, right| left.name.cmp(&right.name));
            variants.push(FiniteVariant {
                identity: finite_variant_id(
                    &ast.module.text,
                    &declaration.name.text,
                    &variant.name.text,
                ),
                name: variant.name.text.clone(),
                discriminant: discriminant as u32,
                payload: payload_fields,
            });
        }
        finite_types.push(FiniteType {
            identity: finite_type_id(&ast.module.text, &declaration.name.text),
            name: declaration.name.text.clone(),
            variants,
        });
    }
    // Imported types join the module namespace after local declarations;
    // collisions were rejected above, so every import is appended verbatim.
    for imported in &context.imported_finite_types {
        finite_types.push(imported.clone());
    }
    let finite_types_by_name = finite_types
        .iter()
        .map(|finite_type| (finite_type.name.clone(), finite_type.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut record_types = Vec::new();
    let mut record_type_names: BTreeSet<String> = BTreeSet::new();
    for declaration in &ast.record_types {
        if !record_type_names.insert(declaration.name.text.clone()) {
            diagnostics.push(elaboration_diagnostic(
                "MNE150",
                "record type identity is duplicated in this module namespace",
                declaration.name.span,
            ));
            continue;
        }
        if finite_types_by_name.contains_key(&declaration.name.text) {
            diagnostics.push(elaboration_diagnostic(
                "MNE151",
                "record type name collides with a declared finite type",
                declaration.name.span,
            ));
            continue;
        }
        let mut field_names: BTreeSet<String> = BTreeSet::new();
        let mut duplicate_fields = 0_usize;
        for field in &declaration.fields {
            if !field_names.insert(field.name.text.clone()) {
                diagnostics.push(elaboration_diagnostic(
                    "MNE152",
                    "record field identity is duplicated and could not be projected uniquely",
                    field.name.span,
                ));
                duplicate_fields += 1;
            }
        }
        record_types.push((declaration.clone(), duplicate_fields));
    }
    // Field types may name other records regardless of declaration order, so
    // resolution runs after every valid record name is registered.
    let provisional_names = record_types
        .iter()
        .map(|(declaration, _)| declaration.name.text.clone())
        .collect::<BTreeSet<_>>();
    let mut resolved_records = Vec::new();
    for (declaration, duplicate_fields) in &record_types {
        if *duplicate_fields > 0 {
            continue;
        }
        let mut fields = Vec::new();
        for field in &declaration.fields {
            if !provisional_names.contains(&field.value_type.text)
                && !finite_types_by_name.contains_key(&field.value_type.text)
                && profile_scalar_supported(&field.value_type.text).is_none()
            {
                diagnostics.push(elaboration_diagnostic(
                    "MNE153",
                    "record field type does not name a supported scalar, finite, or declared record type",
                    field.value_type.span,
                ));
                continue;
            }
            fields.push(RecordField {
                name: field.name.text.clone(),
                field_type: field.value_type.text.clone(),
            });
        }
        fields.sort_by(|left, right| left.name.cmp(&right.name));
        resolved_records.push(RecordType {
            identity: record_type_id(
                &ast.module.text,
                &declaration.name.text,
                &fields
                    .iter()
                    .map(|field| (field.name.as_str(), field.field_type.as_str()))
                    .collect::<Vec<_>>(),
            ),
            name: declaration.name.text.clone(),
            fields,
        });
    }
    let mut all_records = resolved_records;
    for imported in &context.imported_record_types {
        all_records.push(imported.clone());
    }
    let record_types_by_name = all_records
        .iter()
        .map(|record| (record.name.clone(), record.clone()))
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
    let mut signatures = context.signatures.clone();
    for function in &ast.functions {
        signatures.insert(
            function.name.text.clone(),
            FunctionSignature::from_ast(
                ast,
                function,
                &finite_types_by_name,
                &record_types_by_name,
                &mut diagnostics,
            ),
        );
    }
    reject_recursive_calls(ast, &signatures, &mut diagnostics);
    for function in &ast.functions {
        if functions
            .iter()
            .any(|item: &Function| item.name == function.name.text)
        {
            continue;
        }
        match elaborate_function(
            ast,
            function,
            &finite_types_by_name,
            &record_types_by_name,
            &signatures,
            &declarations,
            resolutions,
        ) {
            Ok(elaborated) => functions.push(elaborated),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if diagnostics.is_empty() {
        let mut all_functions = context.imported_functions.clone();
        all_functions.extend(functions);
        let mut dependencies: Vec<SemanticId> = ast
            .uses
            .iter()
            .map(|use_decl| mncs_model::module_id(&use_decl.module.text))
            .collect();
        dependencies.extend(context.imported_dependencies.iter().cloned());
        dependencies.sort();
        dependencies.dedup();
        Ok(Program {
            schema_version: SUPPORTED_SCHEMA_VERSION.to_owned(),
            module: ast.module.text.clone(),
            dependencies,
            finite_types,
            record_types: all_records,
            assumptions: Vec::new(),
            functions: all_functions,
        })
    } else {
        Err(diagnostics)
    }
}

/// Source spans of every nominal declaration name, used to point name
/// occurrences at the exact declaration they resolve to.
#[derive(Debug, Clone, Default)]
struct DeclarationSpans {
    functions: BTreeMap<String, SourceSpan>,
    finite_types: BTreeMap<String, SourceSpan>,
    finite_variants: BTreeMap<(String, String), SourceSpan>,
    record_types: BTreeMap<String, SourceSpan>,
    record_fields: BTreeMap<(String, String), SourceSpan>,
}

impl DeclarationSpans {
    fn from_ast(ast: &AbstractSyntaxTree) -> Self {
        let mut declarations = Self::default();
        for finite_type in &ast.finite_types {
            declarations
                .finite_types
                .insert(finite_type.name.text.clone(), finite_type.name.span);
            for variant in &finite_type.variants {
                declarations.finite_variants.insert(
                    (finite_type.name.text.clone(), variant.name.text.clone()),
                    variant.name.span,
                );
            }
        }
        for record in &ast.record_types {
            declarations
                .record_types
                .insert(record.name.text.clone(), record.name.span);
            for field in &record.fields {
                declarations.record_fields.insert(
                    (record.name.text.clone(), field.name.text.clone()),
                    field.name.span,
                );
            }
        }
        for function in &ast.functions {
            declarations
                .functions
                .insert(function.name.text.clone(), function.name.span);
        }
        declarations
    }

    fn merge_missing(&mut self, imported: &Self) {
        for (name, span) in &imported.functions {
            self.functions.entry(name.clone()).or_insert(*span);
        }
        for (name, span) in &imported.finite_types {
            self.finite_types.entry(name.clone()).or_insert(*span);
        }
        for (name, span) in &imported.finite_variants {
            self.finite_variants.entry(name.clone()).or_insert(*span);
        }
        for (name, span) in &imported.record_types {
            self.record_types.entry(name.clone()).or_insert(*span);
        }
        for (name, span) in &imported.record_fields {
            self.record_fields.entry(name.clone()).or_insert(*span);
        }
    }
}

/// Record type-annotation occurrences (parameter, output, binding, and carried
/// state types). Annotation positions are syntactically unambiguous; the only
/// decision is whether the annotated name denotes a declared nominal type.
fn record_annotation_type_resolutions(
    ast: &AbstractSyntaxTree,
    declarations: &DeclarationSpans,
    resolutions: &mut Vec<NameResolution>,
) {
    for function in &ast.functions {
        for parameter in function.inputs.iter().chain(&function.outputs) {
            record_annotation(&parameter.value_type, declarations, resolutions);
        }
        record_annotation_block(&function.body.statements, declarations, resolutions);
    }
}

fn record_annotation_block(
    statements: &[AstStmt],
    declarations: &DeclarationSpans,
    resolutions: &mut Vec<NameResolution>,
) {
    for statement in statements {
        match statement {
            AstStmt::Let { value_type, .. } => {
                record_annotation(value_type, declarations, resolutions);
            }
            AstStmt::If {
                then_body,
                else_body,
                ..
            } => {
                record_annotation_block(then_body, declarations, resolutions);
                record_annotation_block(else_body, declarations, resolutions);
            }
            AstStmt::BoundedIteration {
                state_type, body, ..
            } => {
                record_annotation(state_type, declarations, resolutions);
                record_annotation_block(body, declarations, resolutions);
            }
            AstStmt::Fail { .. } | AstStmt::Return { .. } => {}
        }
    }
}

fn annotation_kind(name: &str, declarations: &DeclarationSpans) -> Option<ResolvedNameKind> {
    if declarations.finite_types.contains_key(name) {
        Some(ResolvedNameKind::FiniteType)
    } else if declarations.record_types.contains_key(name) {
        Some(ResolvedNameKind::RecordType)
    } else {
        None
    }
}

fn record_annotation(
    name: &SpannedText,
    declarations: &DeclarationSpans,
    resolutions: &mut Vec<NameResolution>,
) {
    let Some(kind) = annotation_kind(&name.text, declarations) else {
        return;
    };
    let declaration = match kind {
        ResolvedNameKind::FiniteType => declarations.finite_types.get(&name.text),
        _ => declarations.record_types.get(&name.text),
    }
    .copied();
    if let Some(declaration) = declaration {
        resolutions.push(NameResolution {
            occurrence: name.span,
            declaration,
            kind,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn elaborate_function(
    ast: &AbstractSyntaxTree,
    function: &AstFunction,
    finite_types: &BTreeMap<String, FiniteType>,
    record_types: &BTreeMap<String, RecordType>,
    signatures: &BTreeMap<String, FunctionSignature>,
    declarations: &DeclarationSpans,
    resolutions: &mut Vec<NameResolution>,
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
                record_types,
                &mut diagnostics,
            ),
        })
        .collect::<Vec<_>>();
    let output_type = profile_type(
        &function.outputs[0].value_type.text,
        function.outputs[0].value_type.span,
        finite_types,
        record_types,
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
            input.name.span,
            BoundNameKind::Parameter,
            &mut diagnostics,
        );
    }
    let (blocks, bounded_iterations, builder_resolutions) = if function.body.statements.is_empty()
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
        (
            vec![BodyBlock {
                id: "entry".to_owned(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: BodyTerminator::Return {
                    values: vec![returned_name.text.clone()],
                },
            }],
            Vec::new(),
            Vec::new(),
        )
    } else {
        let mut builder = BodyBuilder::new(
            output_type.clone(),
            function.name.text.clone(),
            finite_types,
            record_types,
            signatures,
            declarations,
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
        let builder_resolutions = std::mem::take(&mut builder.resolutions);
        (
            builder.blocks,
            builder.bounded_iterations,
            builder_resolutions,
        )
    };
    resolutions.extend(env.take_resolutions());
    resolutions.extend(builder_resolutions);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let _ = ast;
    let _ = assumptions;
    Ok(Function {
        name: function.name.text.clone(),
        home_module: None,
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
            cycle_policy: if ast.language_version.text == SOURCE_PROFILE_VERSION_0_4 {
                BodyCyclePolicy::BoundedIterationOnly
            } else {
                BodyCyclePolicy::Legacy
            },
            bounded_iterations,
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
        record_types: &BTreeMap<String, RecordType>,
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
                    record_types,
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
                    record_types,
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
                "Source Profiles 0.3 and 0.4 reject recursive call cycles; calls must be acyclic",
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
        AstStmt::BoundedIteration {
            initial,
            body,
            next_value,
            ..
        } => {
            calls_in_expr(initial, calls);
            for statement in body {
                calls_in_statement(statement, calls);
            }
            calls_in_expr(next_value, calls);
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
        AstExpr::RecordLiteral { base, fields, .. } => {
            if let Some(base) = base {
                calls_in_expr(base, calls);
            }
            for (_, value) in fields {
                calls_in_expr(value, calls);
            }
        }
        AstExpr::FieldProject { base, .. } => calls_in_expr(base, calls),
        AstExpr::Name(_)
        | AstExpr::Integer { .. }
        | AstExpr::Boolean { .. }
        | AstExpr::FiniteVariant { .. } => {}
    }
}

struct BindingEnv {
    scopes: Vec<std::collections::BTreeMap<String, (ResolvedBinding, SourceSpan, BoundNameKind)>>,
    resolutions: Vec<NameResolution>,
}

#[derive(Clone)]
struct ResolvedBinding {
    id: String,
    ty: BodyType,
}

/// Lexical role of a bound name; recorded so tools can distinguish parameter,
/// local binding, and carried iteration-state declarations.
#[derive(Debug, Clone, Copy)]
enum BoundNameKind {
    Parameter,
    Binding,
    IterationState,
}

impl BoundNameKind {
    fn resolved_kind(self) -> ResolvedNameKind {
        match self {
            Self::Parameter => ResolvedNameKind::Parameter,
            Self::Binding => ResolvedNameKind::Binding,
            Self::IterationState => ResolvedNameKind::IterationState,
        }
    }
}

impl BindingEnv {
    fn new() -> Self {
        Self {
            scopes: vec![std::collections::BTreeMap::new()],
            resolutions: Vec::new(),
        }
    }

    fn bind(
        &mut self,
        name: String,
        id: String,
        ty: BodyType,
        declaration: SourceSpan,
        kind: BoundNameKind,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) {
        let scope = self.scopes.last_mut().expect("scope stack is non-empty");
        if let Some((_, existing, _)) = scope.get(&name) {
            let _ = existing;
            diagnostics.push(elaboration_diagnostic(
                "MNE110",
                "binding is ambiguous in this lexical scope",
                declaration,
            ));
            return;
        }
        self.resolutions.push(NameResolution {
            occurrence: declaration,
            declaration,
            kind: kind.resolved_kind(),
        });
        scope.insert(name, (ResolvedBinding { id, ty }, declaration, kind));
    }

    fn resolve(
        &mut self,
        name: &str,
        span: SourceSpan,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        for scope in self.scopes.iter().rev() {
            if let Some((id, declaration, kind)) = scope.get(name) {
                self.resolutions.push(NameResolution {
                    occurrence: span,
                    declaration: *declaration,
                    kind: kind.resolved_kind(),
                });
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

    fn binds(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    fn push(&mut self) {
        self.scopes.push(std::collections::BTreeMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    fn take_resolutions(&mut self) -> Vec<NameResolution> {
        std::mem::take(&mut self.resolutions)
    }
}

struct BodyBuilder<'a> {
    blocks: Vec<BodyBlock>,
    bounded_iterations: Vec<BodyBoundedIteration>,
    current: usize,
    next_value: usize,
    next_block: usize,
    output_type: BodyType,
    function: String,
    finite_types: &'a BTreeMap<String, FiniteType>,
    record_types: &'a BTreeMap<String, RecordType>,
    signatures: &'a BTreeMap<String, FunctionSignature>,
    declarations: &'a DeclarationSpans,
    resolutions: Vec<NameResolution>,
    in_iteration: bool,
}

impl<'a> BodyBuilder<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        output_type: BodyType,
        function: String,
        finite_types: &'a BTreeMap<String, FiniteType>,
        record_types: &'a BTreeMap<String, RecordType>,
        signatures: &'a BTreeMap<String, FunctionSignature>,
        declarations: &'a DeclarationSpans,
    ) -> Self {
        Self {
            blocks: vec![BodyBlock {
                id: "entry".to_owned(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: BodyTerminator::Return { values: Vec::new() },
            }],
            bounded_iterations: Vec::new(),
            current: 0,
            next_value: 0,
            next_block: 0,
            output_type,
            function,
            finite_types,
            record_types,
            signatures,
            declarations,
            resolutions: Vec::new(),
            in_iteration: false,
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
                    self.record_types,
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
                    name.span,
                    BoundNameKind::Binding,
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
            AstStmt::BoundedIteration { .. } => {
                self.elaborate_bounded_iteration(statement, env, diagnostics);
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

    fn elaborate_bounded_iteration(
        &mut self,
        statement: &AstStmt,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) {
        let AstStmt::BoundedIteration {
            name,
            bound,
            bound_value,
            state,
            state_type,
            initial,
            body,
            next_state,
            next_value,
            span,
        } = statement
        else {
            unreachable!("bounded iteration helper requires iteration statement")
        };
        if self.in_iteration {
            diagnostics.push(elaboration_diagnostic(
                "MNE147",
                "Source Profile 0.4 does not permit nested bounded iterations",
                *span,
            ));
            return;
        }
        let Ok(bound_u32) = u32::try_from(*bound_value) else {
            diagnostics.push(elaboration_diagnostic(
                "MNE142",
                "iteration bound must be a positive Profile 0.4 integer literal",
                bound.span,
            ));
            return;
        };
        if !(1..=SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND).contains(&bound_u32) {
            diagnostics.push(elaboration_diagnostic(
                "MNE142",
                format!(
                    "iteration bound must be between 1 and {} in Source Profile 0.4",
                    SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND
                ),
                bound.span,
            ));
            return;
        }
        if self
            .bounded_iterations
            .iter()
            .any(|iteration| iteration.id == name.text)
        {
            diagnostics.push(elaboration_diagnostic(
                "MNE146",
                "iteration identity is duplicated in this function",
                name.span,
            ));
            return;
        }
        let carried_type = profile_type(
            &state_type.text,
            state_type.span,
            self.finite_types,
            self.record_types,
            diagnostics,
        );
        let Some(initial_value) =
            self.elaborate_expr(initial, Some(&carried_type), env, diagnostics)
        else {
            return;
        };
        if initial_value.ty != carried_type {
            diagnostics.push(elaboration_diagnostic(
                "MNE144",
                "initial iteration state does not preserve the declared carried type",
                initial.span(),
            ));
            return;
        }
        let preheader = self.blocks[self.current].id.clone();
        let counter_type = BodyType::Integer(IntegerType {
            bits: 64,
            signed: false,
        });
        let bound_id = self.new_value("iteration_bound");
        self.blocks[self.current].operations.push(BodyOperation {
            id: bound_id.clone(),
            kind: BodyOperationKind::Constant {
                value: i128::from(bound_u32),
                ty: counter_type.clone(),
            },
            operands: Vec::new(),
            results: vec![BodyValue {
                id: bound_id.clone(),
                ty: counter_type.clone(),
            }],
            contracts: Vec::new(),
            assumptions: Vec::new(),
            machine_intent: None,
            lowering: None,
            portability: None,
        });
        let header = self.new_block();
        let body_entry = self.new_block();
        let exit = self.new_block();
        let header_state = self.new_value("iteration_state");
        let header_counter = self.new_value("iteration_remaining");
        let header_index = self.index_of(&header);
        self.blocks[header_index].parameters = vec![
            BodyValue {
                id: header_state.clone(),
                ty: carried_type.clone(),
            },
            BodyValue {
                id: header_counter.clone(),
                ty: counter_type.clone(),
            },
        ];
        self.blocks[self.current].terminator = BodyTerminator::Branch {
            target: header.clone(),
            arguments: vec![initial_value.id.clone(), bound_id],
        };
        self.current = header_index;
        let zero_id = self.new_value("iteration_zero");
        self.blocks[self.current].operations.push(BodyOperation {
            id: zero_id.clone(),
            kind: BodyOperationKind::Constant {
                value: 0,
                ty: counter_type.clone(),
            },
            operands: Vec::new(),
            results: vec![BodyValue {
                id: zero_id.clone(),
                ty: counter_type.clone(),
            }],
            contracts: Vec::new(),
            assumptions: Vec::new(),
            machine_intent: None,
            lowering: None,
            portability: None,
        });
        let has_attempt_id = self.new_value("iteration_has_attempt");
        self.blocks[self.current].operations.push(BodyOperation {
            id: has_attempt_id.clone(),
            kind: BodyOperationKind::IntegerCompare {
                predicate: "gt".to_owned(),
                operand_type: IntegerType {
                    bits: 64,
                    signed: false,
                },
            },
            operands: vec![header_counter.clone(), zero_id],
            results: vec![BodyValue {
                id: has_attempt_id.clone(),
                ty: BodyType::Named("bool".to_owned()),
            }],
            contracts: Vec::new(),
            assumptions: Vec::new(),
            machine_intent: None,
            lowering: None,
            portability: None,
        });
        let exit_state = self.new_value("iteration_exhausted_state");
        let exit_index = self.index_of(&exit);
        self.blocks[exit_index].parameters.push(BodyValue {
            id: exit_state.clone(),
            ty: carried_type.clone(),
        });
        self.blocks[self.current].terminator = BodyTerminator::ConditionalBranch {
            condition: has_attempt_id,
            then_target: body_entry.clone(),
            then_arguments: Vec::new(),
            else_target: exit.clone(),
            else_arguments: vec![header_state.clone()],
        };

        let blocks_before_body = self.blocks.len();
        self.current = self.index_of(&body_entry);
        self.in_iteration = true;
        env.push();
        env.bind(
            state.text.clone(),
            header_state.clone(),
            carried_type.clone(),
            state.span,
            BoundNameKind::IterationState,
            diagnostics,
        );
        self.elaborate_statements(body, env, diagnostics);
        if !self.block_is_open() {
            diagnostics.push(elaboration_diagnostic(
                "MNE148",
                "iteration continuation is unreachable; every body path terminated",
                next_value.span(),
            ));
            env.pop();
            self.in_iteration = false;
            return;
        }
        if next_state.text != state.text {
            diagnostics.push(elaboration_diagnostic(
                "MNE143",
                "iteration 'next' transition must update the declared carried state",
                next_state.span,
            ));
        }
        let Some(next) = self.elaborate_expr(next_value, Some(&carried_type), env, diagnostics)
        else {
            env.pop();
            self.in_iteration = false;
            return;
        };
        if next.ty != carried_type {
            diagnostics.push(elaboration_diagnostic(
                "MNE144",
                "iteration next-state value does not preserve the carried type",
                next_value.span(),
            ));
        }
        let one_id = self.new_value("iteration_one");
        self.blocks[self.current].operations.push(BodyOperation {
            id: one_id.clone(),
            kind: BodyOperationKind::Constant {
                value: 1,
                ty: counter_type.clone(),
            },
            operands: Vec::new(),
            results: vec![BodyValue {
                id: one_id.clone(),
                ty: counter_type.clone(),
            }],
            contracts: Vec::new(),
            assumptions: Vec::new(),
            machine_intent: None,
            lowering: None,
            portability: None,
        });
        let decremented = self.new_value("iteration_decrement");
        self.blocks[self.current].operations.push(BodyOperation {
            id: decremented.clone(),
            kind: BodyOperationKind::Integer {
                operator: "sub".to_owned(),
                operand_type: IntegerType {
                    bits: 64,
                    signed: false,
                },
                intent: ArithmeticIntent::Checked,
            },
            operands: vec![header_counter.clone(), one_id],
            results: vec![BodyValue {
                id: decremented.clone(),
                ty: counter_type,
            }],
            contracts: Vec::new(),
            assumptions: Vec::new(),
            machine_intent: None,
            lowering: None,
            portability: None,
        });
        let backedge = self.blocks[self.current].id.clone();
        self.blocks[self.current].terminator = BodyTerminator::Branch {
            target: header.clone(),
            arguments: vec![next.id, decremented],
        };
        env.pop();
        self.in_iteration = false;
        let mut body_blocks = vec![body_entry.clone()];
        body_blocks.extend(
            self.blocks[blocks_before_body..]
                .iter()
                .map(|block| block.id.clone()),
        );
        body_blocks.sort();
        body_blocks.dedup();
        let mut callees = BTreeSet::new();
        let mut required_capabilities = BTreeSet::new();
        for block in self
            .blocks
            .iter()
            .filter(|block| body_blocks.contains(&block.id))
        {
            for operation in &block.operations {
                if let BodyOperationKind::Call {
                    function,
                    required_capabilities: call_capabilities,
                    ..
                } = &operation.kind
                {
                    callees.insert(function.clone());
                    required_capabilities.extend(call_capabilities.iter().cloned());
                }
            }
        }
        self.bounded_iterations.push(BodyBoundedIteration {
            id: name.text.clone(),
            bound: bound_u32,
            state_name: state.text.clone(),
            state_type: carried_type.clone(),
            initial_value: initial_value.id,
            header_state,
            header_counter,
            preheader,
            header,
            body_entry,
            backedge,
            exit: exit.clone(),
            exit_state: exit_state.clone(),
            body_blocks,
            callees: callees.into_iter().collect(),
            required_capabilities: required_capabilities.into_iter().collect(),
            completion_modes: vec![
                BoundedIterationCompletion::EarlyReturn,
                BoundedIterationCompletion::Exhausted,
                BoundedIterationCompletion::Failure,
            ],
        });
        self.current = exit_index;
        env.bind(
            state.text.clone(),
            exit_state,
            carried_type,
            state.span,
            BoundNameKind::IterationState,
            diagnostics,
        );
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
                fields,
                span,
            } => {
                let Some(finite_type) = self.finite_types.get(&type_name.text).cloned() else {
                    // `name.selector` is syntactically ambiguous between a
                    // finite variant constructor and record field projection.
                    // A lexical binding always means projection here; nominal
                    // constructors live in the module type namespace.
                    if env.binds(&type_name.text) {
                        let projected = AstExpr::FieldProject {
                            base: Box::new(AstExpr::Name(type_name.clone())),
                            field: variant.clone(),
                            span: *span,
                        };
                        return self.elaborate_expr(&projected, expected, env, diagnostics);
                    }
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
                if let Some(&declaration) = self.declarations.finite_types.get(&type_name.text) {
                    self.resolutions.push(NameResolution {
                        occurrence: type_name.span,
                        declaration,
                        kind: ResolvedNameKind::FiniteType,
                    });
                }
                if let Some(&declaration) = self
                    .declarations
                    .finite_variants
                    .get(&(type_name.text.clone(), variant.text.clone()))
                {
                    self.resolutions.push(NameResolution {
                        occurrence: variant.span,
                        declaration,
                        kind: ResolvedNameKind::FiniteVariant,
                    });
                }
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
                // Payload construction (Profile 0.6): every declared payload
                // field must be supplied exactly once; operands are emitted
                // in canonical field order.
                let mut supplied = BTreeMap::new();
                for (field_name, field_value) in fields {
                    if !declared_variant
                        .payload
                        .iter()
                        .any(|field| field.name == field_name.text)
                    {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE172",
                            "finite constructor names a payload field the variant does not declare",
                            field_name.span,
                        ));
                        continue;
                    }
                    if supplied
                        .insert(field_name.text.clone(), field_value)
                        .is_some()
                    {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE173",
                            "finite constructor assigns a payload field more than once",
                            field_name.span,
                        ));
                    }
                }
                let mut operands = Vec::new();
                let mut payload_fields = Vec::new();
                for declared_field in &declared_variant.payload {
                    let Some(field_value) = supplied.get(&declared_field.name) else {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE174",
                            "finite constructor omits a declared payload field",
                            *span,
                        ));
                        return None;
                    };
                    let expected_field = profile_type(
                        &declared_field.field_type,
                        field_value.span(),
                        self.finite_types,
                        self.record_types,
                        diagnostics,
                    );
                    let resolved =
                        self.elaborate_expr(field_value, Some(&expected_field), env, diagnostics)?;
                    if resolved.ty != expected_field {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE175",
                            "payload field value type does not match the declared payload field type",
                            field_value.span(),
                        ));
                    }
                    payload_fields.push(declared_field.name.clone());
                    operands.push(resolved.id);
                }
                let id = self.new_value("e");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::FiniteConstruct {
                        type_identity: finite_type.identity.clone(),
                        variant_identity: declared_variant.identity,
                        discriminant: declared_variant.discriminant,
                        payload_fields,
                    },
                    operands,
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
                if let Some(&declaration) = self.declarations.functions.get(&function.text) {
                    self.resolutions.push(NameResolution {
                        occurrence: function.span,
                        declaration,
                        kind: ResolvedNameKind::Function,
                    });
                }
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
                    // A qualified pattern `Type.VARIANT` must name the
                    // subject's own type.
                    if let Some(qualifier) = &arm.type_name {
                        if qualifier.text != *type_name {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE176",
                                "qualified pattern names a different finite type than the match subject",
                                qualifier.span,
                            ));
                            continue;
                        }
                        if let Some(&declaration) =
                            self.declarations.finite_types.get(&qualifier.text)
                        {
                            self.resolutions.push(NameResolution {
                                occurrence: qualifier.span,
                                declaration,
                                kind: ResolvedNameKind::FiniteType,
                            });
                        }
                    }
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
                    if let Some(&declaration) = self
                        .declarations
                        .finite_variants
                        .get(&(type_name.clone(), arm.variant.text.clone()))
                    {
                        self.resolutions.push(NameResolution {
                            occurrence: arm.variant.span,
                            declaration,
                            kind: ResolvedNameKind::FiniteVariant,
                        });
                    }
                    // Payload bindings must cover the variant's declared
                    // payload exactly once each, with no extras.
                    let mut bindings = Vec::new();
                    let mut bound_fields = BTreeSet::new();
                    for (field_name, binding_name) in &arm.bindings {
                        let Some(declared_field) = variant
                            .payload
                            .iter()
                            .find(|field| field.name == field_name.text)
                        else {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE177",
                                "pattern binds a payload field the variant does not declare",
                                field_name.span,
                            ));
                            continue;
                        };
                        if !bound_fields.insert(field_name.text.clone()) {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE178",
                                "pattern binds a payload field more than once",
                                field_name.span,
                            ));
                            continue;
                        }
                        let expected_field = profile_type(
                            &declared_field.field_type,
                            binding_name.span,
                            self.finite_types,
                            self.record_types,
                            diagnostics,
                        );
                        bindings.push((
                            field_name.text.clone(),
                            binding_name.clone(),
                            expected_field,
                        ));
                    }
                    if !arm.ignore_payload && bindings.len() != variant.payload.len() {
                        let supplied: BTreeSet<_> =
                            bindings.iter().map(|(field, _, _)| field.clone()).collect();
                        for field in &variant.payload {
                            if !supplied.contains(&field.name) {
                                diagnostics.push(elaboration_diagnostic(
                                    "MNE179",
                                    "pattern omits a declared payload field; every payload field must be bound",
                                    arm.variant.span,
                                ));
                            }
                        }
                        continue;
                    }
                    if !seen.insert(variant.identity.clone()) {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE139",
                            "duplicate match arm is unreachable",
                            arm.variant.span,
                        ));
                        continue;
                    }
                    resolved_arms.push((variant.clone(), &arm.value, bindings));
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
                        format!(
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
                for (index, ((variant, _, _), arm_id)) in
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
                for ((variant, arm_expr, bindings), arm_id) in resolved_arms.iter().zip(&arm_ids) {
                    self.current = self.index_of(arm_id);
                    env.push();
                    // Bind each payload field to its pattern name via an
                    // explicit payload projection inside this arm's block.
                    for (field_name, binding_name, expected_field) in bindings {
                        let projected = self.new_value(binding_name.text.as_str());
                        self.blocks[self.current].operations.push(BodyOperation {
                            id: projected.clone(),
                            kind: BodyOperationKind::FinitePayloadProject {
                                type_identity: type_identity.clone(),
                                variant_identity: variant.identity.clone(),
                                discriminant: variant.discriminant,
                                field: field_name.clone(),
                            },
                            operands: vec![subject.id.clone()],
                            results: vec![BodyValue {
                                id: projected.clone(),
                                ty: expected_field.clone(),
                            }],
                            contracts: Vec::new(),
                            assumptions: Vec::new(),
                            machine_intent: None,
                            lowering: None,
                            portability: None,
                        });
                        env.bind(
                            binding_name.text.clone(),
                            projected,
                            expected_field.clone(),
                            binding_name.span,
                            BoundNameKind::Binding,
                            diagnostics,
                        );
                    }
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
            AstExpr::RecordLiteral {
                type_name,
                base,
                fields,
                span,
            } => {
                let Some(record_type) = self.record_types.get(&type_name.text).cloned() else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE154",
                        "record literal names an unknown record type",
                        type_name.span,
                    ));
                    return None;
                };
                if let Some(&declaration) = self.declarations.record_types.get(&type_name.text) {
                    self.resolutions.push(NameResolution {
                        occurrence: type_name.span,
                        declaration,
                        kind: ResolvedNameKind::RecordType,
                    });
                }
                for (name, _) in fields {
                    if record_type
                        .fields
                        .iter()
                        .any(|field| field.name == name.text)
                    {
                        if let Some(&declaration) = self
                            .declarations
                            .record_fields
                            .get(&(type_name.text.clone(), name.text.clone()))
                        {
                            self.resolutions.push(NameResolution {
                                occurrence: name.span,
                                declaration,
                                kind: ResolvedNameKind::RecordField,
                            });
                        }
                    }
                }
                let ty = BodyType::Record {
                    identity: record_type.identity.clone(),
                    name: record_type.name.clone(),
                };
                if expected.is_some_and(|expected| expected != &ty) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE155",
                        "record literal does not have the required nominal type",
                        *span,
                    ));
                }
                let mut supplied = BTreeMap::new();
                for (name, value) in fields {
                    if !record_type
                        .fields
                        .iter()
                        .any(|field| field.name == name.text)
                    {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE156",
                            "record literal names a field the record does not declare",
                            name.span,
                        ));
                        continue;
                    }
                    if supplied.insert(name.text.clone(), value).is_some() {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE157",
                            "record literal assigns a field more than once",
                            name.span,
                        ));
                    }
                }
                let base_value = match base {
                    Some(base_expr) => {
                        let value = self.elaborate_expr(base_expr, Some(&ty), env, diagnostics)?;
                        if value.ty != ty {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE158",
                                "functional record update base does not have the literal's record type",
                                base_expr.span(),
                            ));
                            return None;
                        }
                        Some(value)
                    }
                    None => None,
                };
                let mut operands = Vec::new();
                for declared_field in &record_type.fields {
                    let operand_id = if let Some(value) = supplied.get(&declared_field.name) {
                        let expected_field = profile_type(
                            &declared_field.field_type,
                            value.span(),
                            self.finite_types,
                            self.record_types,
                            diagnostics,
                        );
                        let resolved =
                            self.elaborate_expr(value, Some(&expected_field), env, diagnostics)?;
                        if resolved.ty != expected_field {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE159",
                                "record field value type does not match the declared field type",
                                value.span(),
                            ));
                        }
                        resolved.id
                    } else {
                        let Some(resolved_base) = &base_value else {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE160",
                                "record literal omits a declared field and has no functional update base",
                                *span,
                            ));
                            return None;
                        };
                        self.emit_record_project(
                            resolved_base,
                            &record_type,
                            &declared_field.name,
                            *span,
                            diagnostics,
                        )
                        .id
                    };
                    operands.push(operand_id);
                }
                let id = self.new_value("rec");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::RecordConstruct {
                        type_identity: record_type.identity.clone(),
                        field_names: record_type
                            .fields
                            .iter()
                            .map(|field| field.name.clone())
                            .collect(),
                    },
                    operands,
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
            AstExpr::FieldProject { base, field, span } => {
                let subject = self.elaborate_expr(base, None, env, diagnostics)?;
                let BodyType::Record {
                    name: type_name, ..
                } = &subject.ty
                else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE161",
                        "field projection requires a value of a declared record type",
                        base.span(),
                    ));
                    return None;
                };
                let Some(record_type) = self.record_types.get(type_name).cloned() else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE161",
                        "field projection requires a value of a declared record type",
                        base.span(),
                    ));
                    return None;
                };
                let Some(declared_field) = record_type
                    .fields
                    .iter()
                    .find(|candidate| candidate.name == field.text)
                else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE162",
                        "record projection names a field the record does not declare",
                        field.span,
                    ));
                    return None;
                };
                if let Some(&declaration) = self
                    .declarations
                    .record_fields
                    .get(&(record_type.name.clone(), field.text.clone()))
                {
                    self.resolutions.push(NameResolution {
                        occurrence: field.span,
                        declaration,
                        kind: ResolvedNameKind::RecordField,
                    });
                }
                let result_ty = profile_type(
                    &declared_field.field_type,
                    field.span,
                    self.finite_types,
                    self.record_types,
                    diagnostics,
                );
                if expected.is_some_and(|expected| expected != &result_ty) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE163",
                        "projected field does not have the required expression type",
                        *span,
                    ));
                }
                let projected = self.emit_record_project(
                    &subject,
                    &record_type,
                    &field.text,
                    *span,
                    diagnostics,
                );
                if projected.ty != result_ty {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE163",
                        "projected field does not have the required expression type",
                        *span,
                    ));
                }
                Some(projected)
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
                // Strict boolean operators (Profile 0.6): both operands are
                // total bool values; evaluation is not short-circuited.
                if matches!(op, AstBinaryOp::And | AstBinaryOp::Or) {
                    let bool_type = BodyType::Named("bool".to_owned());
                    if left_value.ty != bool_type || right_value.ty != bool_type {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE181",
                            "boolean operator operands must have type bool",
                            expr.span(),
                        ));
                        return None;
                    }
                    let id = self.new_value("b");
                    self.blocks[self.current].operations.push(BodyOperation {
                        id: id.clone(),
                        kind: BodyOperationKind::BooleanOp {
                            operator: match op {
                                AstBinaryOp::And => "and".to_owned(),
                                _ => "or".to_owned(),
                            },
                        },
                        operands: vec![left_value.id, right_value.id],
                        results: vec![BodyValue {
                            id: id.clone(),
                            ty: bool_type.clone(),
                        }],
                        contracts: Vec::new(),
                        assumptions: Vec::new(),
                        machine_intent: None,
                        lowering: None,
                        portability: None,
                    });
                    return Some(ResolvedBinding { id, ty: bool_type });
                }
                let id = self.new_value("v");
                let (kind, result_ty) = match op {
                    AstBinaryOp::Add
                    | AstBinaryOp::Sub
                    | AstBinaryOp::Mul
                    | AstBinaryOp::Div
                    | AstBinaryOp::Mod
                    | AstBinaryOp::AddWrap
                    | AstBinaryOp::SubWrap
                    | AstBinaryOp::MulWrap
                    | AstBinaryOp::AddSat
                    | AstBinaryOp::SubSat
                    | AstBinaryOp::MulSat => {
                        let BodyType::Integer(operand_type) = left_value.ty else {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE120",
                                "arithmetic operands must have an integer type",
                                expr.span(),
                            ));
                            return None;
                        };
                        let operator = match op {
                            AstBinaryOp::Add | AstBinaryOp::AddWrap | AstBinaryOp::AddSat => "add",
                            AstBinaryOp::Sub | AstBinaryOp::SubWrap | AstBinaryOp::SubSat => "sub",
                            AstBinaryOp::Mul | AstBinaryOp::MulWrap | AstBinaryOp::MulSat => "mul",
                            AstBinaryOp::Div => "div",
                            AstBinaryOp::Mod => "mod",
                            _ => "mul",
                        };
                        // Explicit arithmetic intents (Profile 0.6): the
                        // operator token names the total edge semantics, so
                        // the overflow obligation is discharged by semantics
                        // instead of remaining symbolically unresolved.
                        let intent = match op {
                            AstBinaryOp::AddWrap | AstBinaryOp::SubWrap | AstBinaryOp::MulWrap => {
                                ArithmeticIntent::Wrapping
                            }
                            AstBinaryOp::AddSat | AstBinaryOp::SubSat | AstBinaryOp::MulSat => {
                                ArithmeticIntent::Saturating
                            }
                            _ => ArithmeticIntent::Checked,
                        };
                        (
                            BodyOperationKind::Integer {
                                operator: operator.to_owned(),
                                operand_type,
                                intent,
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

    fn emit_record_project(
        &mut self,
        subject: &ResolvedBinding,
        record_type: &RecordType,
        field: &str,
        span: SourceSpan,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> ResolvedBinding {
        let field_type = record_type
            .fields
            .iter()
            .find(|candidate| candidate.name == field)
            .map(|declared| {
                profile_type(
                    &declared.field_type,
                    span,
                    self.finite_types,
                    self.record_types,
                    diagnostics,
                )
            })
            .unwrap_or_else(|| BodyType::Named("invalid".to_owned()));
        let id = self.new_value("proj");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::RecordProject {
                type_identity: record_type.identity.clone(),
                field: field.to_owned(),
            },
            operands: vec![subject.id.clone()],
            results: vec![BodyValue {
                id: id.clone(),
                ty: field_type.clone(),
            }],
            contracts: Vec::new(),
            assumptions: Vec::new(),
            machine_intent: None,
            lowering: None,
            portability: None,
        });
        ResolvedBinding { id, ty: field_type }
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
    record_types: &BTreeMap<String, RecordType>,
    diagnostics: &mut Vec<SourceDiagnostic>,
) -> BodyType {
    if let Some(finite_type) = finite_types.get(name) {
        return BodyType::Finite {
            identity: finite_type.identity.clone(),
            name: finite_type.name.clone(),
        };
    }
    if let Some(record_type) = record_types.get(name) {
        return BodyType::Record {
            identity: record_type.identity.clone(),
            name: record_type.name.clone(),
        };
    }
    profile_scalar_supported(name).unwrap_or_else(|| {
        diagnostics.push(elaboration_diagnostic(
            "MNE105",
            "source profile supports bool, 8/16/32/64-bit integers, declared finite types, and declared record types",
            span,
        ));
        BodyType::Named(name.to_owned())
    })
}

fn profile_scalar_supported(name: &str) -> Option<BodyType> {
    let ty = BodyType::from_semantic_name(name);
    let supported = matches!(&ty, BodyType::Named(named) if named == "bool")
        || matches!(
            &ty,
            BodyType::Integer(IntegerType {
                bits: 8 | 16 | 32 | 64,
                ..
            })
        );
    supported.then_some(ty)
}

fn statement_span(statement: &AstStmt) -> SourceSpan {
    match statement {
        AstStmt::Let { span, .. }
        | AstStmt::If { span, .. }
        | AstStmt::Fail { span, .. }
        | AstStmt::Return { span, .. }
        | AstStmt::BoundedIteration { span, .. } => *span,
    }
}

fn elaboration_diagnostic(
    code: &str,
    message: impl Into<String>,
    span: SourceSpan,
) -> SourceDiagnostic {
    SourceDiagnostic {
        code: code.to_owned(),
        stage: DiagnosticStage::Elaboration,
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        span,
        expected: Vec::new(),
        found: None,
    }
}
